use actix_web::{post, web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use akri_shared::akri::configuration::Configuration;
use clap::Arg;
// use k8s_openapi::apimachinery::pkg::apis::meta::v1;
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
    match v {
        serde_json::Value::Object(o) => {
            for (key, value) in o {
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

fn validateConfiguration(rqst: &AdmissionRequest) -> AdmissionResponse {
    let resp = AdmissionResponse::new(false, rqst.uid);

    match rqst.object {
        Some(raw) => {
            // Unmarshal `raw` into Akri Configuration
            let c: Configuration = serde_json::from_str(&raw.to_string()[..]).expect("valid JSON");
            // Marshal it back to bytes
            let reserialized = serde_json::to_string(&c).expect("bytes");
            // Unmarshal the result to untyped (Value)
            let deserialized: Value = serde_json::from_str(&reserialized).expect("untyped JSON");

            // Unmarshal `raw` into untyped (Value)
            let v: Value = serde_json::from_str(&raw.to_string()[..]).expect("Valid JSON");

            // Do they match?
            match check(&v, &deserialized) {
                Ok(x) => {
                    resp.allowed = true;
                    resp
                }
                Err(e) => {
                    let status = Status::new();
                    status.message = Some("AdmissionRequest object contains no data".to_owned());
                    resp.status = Some(status);
                    resp
                }
            }
        }
        None => {
            let status = Status::new();
            status.message = Some("AdmissionRequest object contains no data".to_owned());
            resp.status = Some(status);
            return resp;
        }
    }
}

#[post("/validate")]
async fn validate(rqst: web::Json<AdmissionReview>) -> impl Responder {
    match &rqst.request {
        Some(rqst) => {
            let resp = validateConfiguration(&rqst);
            let resp = serde_json::to_string(&resp).expect("valid");
            return HttpResponse::Ok().body(resp);
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
        .get_matches();

    let crt_file = matches.value_of("crt_file").expect("TLS certificate file");
    let key_file = matches.value_of("key_file").expect("TLS certificate file");

    let port = matches
        .value_of("port")
        .unwrap_or("8443")
        .parse::<u16>()
        .expect("valid port [0-65535]");

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
