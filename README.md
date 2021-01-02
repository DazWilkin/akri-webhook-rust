# Akri Webhook

## Dependencies

The package depends on:

|Crate|Description|
|-----|-----------|
|akri|It requires Akri's `./shared/confiugration.rs` for `KubeAkriConfig`|
|openapi|It requires OpenAPI-generated `AdmissionReview`, `AdmissionRequest` and `AdmissionResponse` objects|


## OpenAPI

I used bergeron's OpenAPI (Swagger) files:

https://github.com/kubernetes/kubernetes/issues/84081#issuecomment-701686042

See the issue for more details.

I used IBM's [Eclipse Codewind tool for OpenAPI](https://marketplace.visualstudio.com/items?itemName=IBM.codewind-openapi-tools#:~:text=The%20Eclipse%20Codewind%20tool%20for,work%20without%20the%20Codewind%20extension.) for Visual Studio Code to generate Rust sources.

## Kubernetes `RawExtension`

`RawExtension` is used to payload Kubernetes resources. The Webhook `AdmissionRequest` includes an `Object` property that represents the Kubernetes resource (i.e. `akri.sh/v0/Confiugration`) being payloaded. This needs special handling to unmarshal.

In the case of Rust:

https://docs.rs/k8s-openapi/0.3.0/i686-pc-windows-msvc/k8s_openapi/v1_10/apimachinery/pkg/runtime/struct.RawExtension.html

The `k8s.io.api.admission.v1.swagger.json` includes:

```JSON
"object": {
    "description": "Object is the object from the incoming request.",
    "$ref": "#/definitions/runtime.RawExtension"
},
```

And the `v1_admission_request.rs` includes:

```Rust
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct V1AdmissionRequest {
    ...
    /// RawExtension is used to hold extensions in external versions.  To use this, make a field which has RawExtension as its type in your external, versioned struct, and Object in your internal struct. You also need to register your various plugin types.  // Internal package: type MyAPIObject struct {  runtime.TypeMeta `json:\",inline\"`  MyPlugin runtime.Object `json:\"myPlugin\"` } type PluginA struct {  AOption string `json:\"aOption\"` }  // External package: type MyAPIObject struct {  runtime.TypeMeta `json:\",inline\"`  MyPlugin runtime.RawExtension `json:\"myPlugin\"` } type PluginA struct {  AOption string `json:\"aOption\"` }  // On the wire, the JSON will look something like this: {  \"kind\":\"MyAPIObject\",  \"apiVersion\":\"v1\",  \"myPlugin\": {   \"kind\":\"PluginA\",   \"aOption\":\"foo\",  }, }  So what happens? Decode first uses json or yaml to unmarshal the serialized data into your external MyAPIObject. That causes the raw JSON to be stored, but not unpacked. The next step is to copy (using pkg/conversion) into the internal struct. The runtime package's DefaultScheme has conversion functions installed which will unpack the JSON stored in RawExtension, turning it into the correct object type, and storing it in the Object. (TODO: In the case where the object is of an unknown type, a runtime.Unknown object will be created and stored.)
    #[serde(rename = "object", skip_serializing_if = "Option::is_none")]
    pub object: Option<serde_json::Value>,
}
```

Which got me as far as:

```rust
fn validate_configuration(rqst: &AdmissionRequest) -> AdmissionResponse {
    match &rqst.object {
        Some(object) => {
            let raw: RawExtension = serde_json::from_value(object.clone()).expect("RawExtension");
            let raw_string = serde_json::to_string(&raw).expect("success");
            let c: KubeAkriConfig = serde_json::from_str(raw_string.as_str()).expect("success");
```


## Certificate

```bash
DIR="${PWD}/secrets"
SERVICE="localhost"
FILENAME="${DIR}/${SERVICE}"

openssl req \
-x509 \
-newkey rsa:2048 \
-keyout ${FILENAME}.key \
-out ${FILENAME}.crt \
-nodes \
-days 365 \
-subj "/CN=localhost"
```

## Run

```bash
PORT="8443"

cargo run -- \
  --tls-crt-file=${FILENAME}.crt \
  --tls-key-file=${FILENAME}.key \
  --port=${PORT}
```

## Docker

```bash
PORT="8443"

docker run \
--rm --interactive --tty \
--volume=${PWD}/secrets:/secrets \
--publish=8443:8443 \
ghcr.io/dazwilkin/akri-webhook:$(git rev-parse HEAD) \
  --tls-crt-file=/secrets/${SERVICE}.crt \
  --tls-key-file=/secrets/${SERVICE}.key \
  --port=${PORT}
```

## Test

Then, from another shell:

```bash
DIR="${PWD}/secrets"
SERVICE="localhost"
FILENAME="${DIR}/${SERVICE}"
PORT="8443"

for TEST in "good" "bad"
do
   RESP=$(curl \
   --silent \
   --insecure \
   --cert ${FILENAME}.crt \
   --key ${FILENAME}.key \
   --request POST \
   --header "Content-Type: application/json" \
   --data "@./JSON/admissionreview.v1.rqst.${TEST}.json" \
   https://localhost:${PORT}/validate)
   printf "${TEST}: ${RESP}\n"
done
```

## Kubernetes

See the instructions for [Kubernetes](https://github.com/DazWilkin/akri-webhook#kubernetes) on [`akri-webhook`](https://github.com/DazWilkin/akri-webhook) the Golang implementation.

The Kubernetes functionality is mostly unchanged. You'll need to ensure you use the correct image and you'll need to comment the `klog` flags referenced in the args (i.e. comment out or remove `--logtostderr` and `--v=2`):

```YAML
containers:
- name: webhook
    image: ghcr.io/dazwilkin/akri-webhook@[[CORRECT-SHA256]]
    imagePullPolicy: Always
    args:
    - --tls-crt-file=/secrets/tls.crt
    - --tls-key-file=/secrets/tls.key
    - --port=8443
#    - --logtostderr
#    - --v=2
```

## Debugging

```bash
kubectl run dnsutils \
--rm \
--stdin \
--tty \
--image=gcr.io/kubernetes-e2e-test-images/dnsutils:1.3 \
-- sh
nslookup ${SERVICE}.${NAMESPACE}.svc
```

Should resolve and should match:

```bash
ENDPOINT=$(\
  kubectl get service/${SERVICE} \
  --namespace=${NAMESPACE} \
  --output=jsonpath="{.spec.clusterIP}") && echo ${ENDPOINT}
```

And:

```bash
openssl s_client \
-connect ${ENDPOINT} \
--servername ${SERVICE}.${NAMESPACE}.svc
```
