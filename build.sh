#!/usr/bin/env bash

REGISTRY="ghcr.io"
PREFIX="akri-webhook"
REPO="${REGISTRY}/dazwilkin/${PREFIX}"
TAG="$(git rev-parse HEAD)"

IMAGE=${REPO}:${TAG}

# Create unique image for repo:tag
docker build --tag=${IMAGE} --file=./Dockerfile .
docker push ${IMAGE}

# Create language identifying repo:lang
LANG="rust"
docker tag ${IMAGE} ${REPO}:${LANG}
docker push ${REPO}:${LANG}