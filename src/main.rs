use actix_web::{post, web, App, HttpResponse, HttpServer, Responder};
use akri_shared::akri::configuration::KubeAkriConfig;
use clap::Arg;
use k8s_openapi::apimachinery::pkg::runtime::RawExtension;
use openapi::models::{
    V1AdmissionRequest as AdmissionRequest, V1AdmissionResponse as AdmissionResponse,
    V1AdmissionReview as AdmissionReview, V1Status as Status,
};
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslFiletype, SslMethod};
use rustls::internal::pemfile::{certs, rsa_private_keys};
use rustls::{NoClientAuth, ServerConfig};
use serde_json::Value;
use std::fs::File;

type CertPair = (String, String);

fn get_config((key, crt): CertPair) -> ServerConfig {
    use std::io::BufReader;

    let mut crt = BufReader::new(File::open(crt.to_owned()).unwrap());
    let mut key = BufReader::new(File::open(key.to_owned()).unwrap());

    let mut config = ServerConfig::new(NoClientAuth::new());
    let cert_chain = certs(&mut crt).unwrap();
    let mut keys = rsa_private_keys(&mut key).unwrap();
    config.set_single_cert(cert_chain, keys.remove(0)).unwrap();

    config
}
fn get_builder((key, crt): CertPair) -> SslAcceptorBuilder {
    let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
    builder.set_private_key_file(key, SslFiletype::PEM).unwrap();
    builder.set_certificate_chain_file(crt).unwrap();

    builder
}
fn check(
    v: &serde_json::Value,
    deserialized: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    if deserialized == &serde_json::Value::Null {
        return Err(None.ok_or(format!("no matching value in `deserialized`"))?);
    }

    match v {
        serde_json::Value::Object(o) => {
            for (key, value) in o {
                println!("[check] key: {}", key);
                if key == "creationTimestamp" {
                    println!("[check] creationTimestamp deserialized: {:?}", deserialized);
                    return Ok(());
                }
                if let Err(e) = check(&value, &deserialized[key]) {
                    return Err(None.ok_or(format!(
                        "input key ({:?}) not equal to parsed: ({:?})",
                        key, e
                    ))?);
                }
            }
            Ok(())
        }
        serde_json::Value::Array(s) => {
            for (pos, _e) in s.iter().enumerate() {
                if let Err(e) = check(&s[pos], &deserialized[pos]) {
                    return Err(None.ok_or(format!(
                        "input index ({:?}) not equal to parsed: ({:?})",
                        pos, e
                    ))?);
                }
            }
            Ok(())
        }
        serde_json::Value::String(s) => match deserialized {
            serde_json::Value::String(ds) => {
                if s != ds {
                    Err(None.ok_or(format!("input ({:?}) not equal to parsed ({:?})", s, ds))?)
                } else {
                    Ok(())
                }
            }
            _ => Err(None.ok_or(format!(
                "input ({:?}) not equal to parsed ({:?})",
                s, deserialized
            ))?),
        },
        serde_json::Value::Bool(b) => match deserialized {
            serde_json::Value::Bool(db) => {
                if b != db {
                    Err(None.ok_or(format!("input ({:?}) not equal to parsed ({:?})", b, db))?)
                } else {
                    Ok(())
                }
            }
            _ => Err(None.ok_or(format!(
                "input ({:?}) not equal to parsed ({:?})",
                b, deserialized
            ))?),
        },
        serde_json::Value::Number(n) => match deserialized {
            serde_json::Value::Number(dn) => {
                if n != dn {
                    Err(None.ok_or(format!("input ({:?}) not equal to parsed ({:?})", n, dn))?)
                } else {
                    Ok(())
                }
            }
            _ => Err(None.ok_or(format!(
                "input ({:?}) not equal to parsed ({:?})",
                n, deserialized
            ))?),
        },
        _ => Err(None.ok_or(format!("what is this? {:?}", "boooo!"))?),
    }
}

fn validate_configuration(rqst: &AdmissionRequest) -> AdmissionResponse {
    match &rqst.object {
        Some(raw) => {
            // RawExtension represents the embedded request
            let x: RawExtension = serde_json::from_value(raw.clone()).expect("RawExtension");
            // TODO(dazwilkin) Is there a more direct way to convert this? pkg/convert ??
            // Marshal it back to a string
            let y = serde_json::to_string(&x).expect("success");
            // Unmarshal `raw` into Akri Configuration
            let c: KubeAkriConfig = serde_json::from_str(y.as_str()).expect("success");
            // Marshal it back to bytes
            let reserialized = serde_json::to_string(&c).expect("bytes");
            println!("researialized: {:?}", reserialized);
            // Unmarshal the result to untyped (Value)
            let deserialized: Value = serde_json::from_str(&reserialized).expect("untyped JSON");

            // Unmarshal `raw` into untyped (Value)
            let v: Value = serde_json::from_value(raw.clone()).expect("RawExtension");

            // Do they match?
            match check(&v, &deserialized) {
                Ok(_) => AdmissionResponse::new(true, rqst.uid.to_owned()),
                Err(e) => AdmissionResponse {
                    allowed: false,
                    audit_annotations: None,
                    patch: None,
                    patch_type: None,
                    status: Some(Status {
                        api_version: None,
                        code: None,
                        details: None,
                        kind: None,
                        message: Some(e.to_string()),
                        metadata: None,
                        reason: None,
                        status: None,
                    }),
                    uid: rqst.uid.to_owned(),
                    warnings: None,
                },
            }
        }
        None => AdmissionResponse {
            allowed: false,
            audit_annotations: None,
            patch: None,
            patch_type: None,
            status: Some(Status {
                api_version: None,
                code: None,
                details: None,
                kind: None,
                message: Some("AdmissionRequest object contains no data".to_owned()),
                metadata: None,
                reason: None,
                status: None,
            }),
            uid: rqst.uid.to_owned(),
            warnings: None,
        },
    }
}

#[post("/validate")]
async fn validate(rqst: web::Json<AdmissionReview>) -> impl Responder {
    match &rqst.request {
        Some(rqst) => {
            let resp = validate_configuration(&rqst);
            let resp: AdmissionReview = AdmissionReview {
                api_version: Some("admission.k8s.io/v1".to_owned()),
                kind: Some("AdmissionReview".to_owned()),
                request: None,
                response: Some(resp),
            };
            let body = serde_json::to_string(&resp).expect("Valid AdmissionReview");
            return HttpResponse::Ok().body(body);
        }
        None => {
            return HttpResponse::BadRequest().body("");
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let matches = clap::App::new("Akri Webhook")
        .version("0.0.1")
        .author("Daz Wilkin <daz.wilkin@gmail.com>")
        .arg(
            Arg::new("crt_file")
                .long("tls-crt-file")
                .takes_value(true)
                .about("TLS Certificate file"),
        )
        .arg(
            Arg::new("key_file")
                .long("tls-key-file")
                .takes_value(true)
                .about("TLS private key file"),
        )
        .arg(
            Arg::new("port")
                .long("port")
                .takes_value(true)
                .about("Webhook port"),
        )
        .arg(
            Arg::new("logtostderr")
                .long("logtostderr")
                .takes_value(false)
                .about("Redundant: included for consistency with Golang variant"),
        )
        .arg(
            Arg::new("v")
                .long("v")
                .takes_value(true)
                .about("Redudnant: included for consistency with Golang variant"),
        )
        .get_matches();

    let crt_file = matches.value_of("crt_file").expect("TLS certificate file");
    let key_file = matches.value_of("key_file").expect("TLS certificate file");

    let port = matches
        .value_of("port")
        .unwrap_or("8443")
        .parse::<u16>()
        .expect("valid port [0-65535]");

    // Debugging :-(
    // let crt_file = "/home/dazwilkin/Projects/akri/webhook/rust/akri-webhook/secrets/localhost.crt";
    // let key_file = "/home/dazwilkin/Projects/akri/webhook/rust/akri-webhook/secrets/localhost.key";
    // let port: u16 = 8443;

    let endpoint = format!("0.0.0.0:{}", port);
    println!("Started HTTPd: {}", endpoint);

    // Cargo.toml: actix-web = { version = "3.3.2", features = ["rustls"] }
    // let config = get_config((key_file.to_owned(), crt_file.to_owned()));
    // HttpServer::new(|| App::new().service(validate))
    //     .bind_rustls(ENDPOINT.to_owned(), config)?
    //     .run()
    //     .await

    // Cargo.toml: actix-web = { version = "3.3.2", features = ["openssl"] }
    let builder = get_builder((key_file.to_owned(), crt_file.to_owned()));
    HttpServer::new(|| App::new().service(validate))
        .bind_openssl(endpoint, builder)?
        .run()
        .await
}
