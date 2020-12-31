# Akri Webhook

## Certificate

```bash
DIR=${PWD}/secrets
FILENAME=${DIR}/localhost

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
cargo run -- \
  --tls-crt-file=${FILENAME}.crt \
  --tls-key-file=${FILENAME}.key \
  --port=8443
```
