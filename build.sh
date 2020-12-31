#!/usr/bin/env bash

REGISTRY="ghcr.io"
PREFIX="akri-webhook"
REPO="${REGISTRY}/dazwilkin/${PREFIX}"
TAG=$(git rev-parse HEAD)

IMAGE=${REPO}:${TAG}

docker build --tag=${IMAGE} --file=./Dockerfile .
docker push ${IMAGE}
