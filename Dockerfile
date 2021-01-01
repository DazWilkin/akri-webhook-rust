ARG PROJECT="akri-webhook"

FROM rustlang/rust:nightly-slim as builder

ARG PROJECT

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl-dev \
    pkg-config

RUN USER=root cargo new --bin ${PROJECT}

WORKDIR /${PROJECT}

# For: akri_shared::akri::configuration::KubeAkriConfig;
COPY ./shared ./shared

# Saves repeatedly building the dependencies
# Because the project doesn't use main, add it here, purely as a throwaway
COPY ./Cargo.toml ./Cargo.toml
RUN echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm src/*.rs

RUN ls -l ./target/release/deps

# Replace hyphens with underscores in ${PROJECT}
RUN rm ./target/release/deps/$(echo ${PROJECT} | tr '-' '_')*

COPY ./src/main.rs ./src

RUN cargo build --release


FROM debian:buster-slim as runtime

ARG PROJECT

LABEL org.label-schema.docker.dockerfile="./Dockerfile" \
    org.label-schema.url="https://github.com/DazWilkin/akri-webhook-rust/" \
    org.label-schema.vcs-ref=$VCS_REF \
    org.label-schema.vcs-url="https://github.com/DazWilkin/akri-webhook-rust.git" \
    org.label-schema.vcs-type="Git"

WORKDIR /bin

# Copy from builder and rename to 'server'
COPY --from=builder /${PROJECT}/target/release/${PROJECT} /server

RUN apt update \
    && apt install -y \
    ca-certificates \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

ENTRYPOINT ["/server"]
CMD ["--tls-crt-file=/path/to/crt","--tls-key-file=/path/to/key","--port=8443"]
