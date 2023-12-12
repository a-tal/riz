#!/usr/bin/env bash

docker build \
  --pull \
  -t riz:build \
  -f docker/build.dockerfile \
  . || { echo "failed to prep build container"; exit 1; }

docker run \
  --rm \
  -it \
  -v "$(pwd):/src/riz" \
  --workdir "/src/riz" \
  -v "$HOME/.cargo/registry:/usr/local/cargo/registry" \
  riz:build || { echo "riz build failure"; exit 1; }

docker build \
  --build-arg UID=$(id -u) \
  -f docker/run.dockerfile \
  -t riz:dev \
  . || { echo "riz container build failure"; exit 1; }

docker kill riz-api > /dev/null 2>&1
docker rm riz-api > /dev/null 2>&1

docker run -d \
  --restart=always \
  -p 8080:8080 \
  --name riz-api \
  --hostname riz-api \
  -v "${PWD}/data":/data \
  riz:dev || { echo "failed to start container"; exit 1; }
