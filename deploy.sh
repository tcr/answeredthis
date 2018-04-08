#!/bin/bash

set -e
set -x

DOKKU_URL=sandbox.edit.io
DOKKU_NAME=answeredthis

rm -rf ./dist/deploy/static || true
cp -rf ./static ./dist/deploy/static

docker build \
    -f ./dist/build/Dockerfile dist/build/ \
    -t answeredthis-build

#    -v $(pwd)/dist/build/rustup-toolchain-cache:/usr/local/rustup/toolchains \

docker run --rm \
    -v $(pwd)/dist/build/cargo-git-cache:/usr/local/cargo/git \
    -v $(pwd)/dist/build/cargo-registry-cache:/usr/local/cargo/registry \
    -v $(pwd):/app \
    -w /app/answeredthis \
    -t -i answeredthis-build \
    cargo build --release --target=x86_64-unknown-linux-gnu --bin answeredthis # --features 'standalone'

cp ./target/x86_64-unknown-linux-gnu/release/answeredthis ./dist/deploy/answeredthis

tar c ./dist/deploy/. | bzip2 | ssh root@$DOKKU_URL "bunzip2 > /tmp/mercutio.tar"
ssh root@$DOKKU_URL "cat /tmp/mercutio.tar | dokku tar:in $DOKKU_NAME"
