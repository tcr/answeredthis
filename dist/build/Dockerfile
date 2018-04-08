# Dockerfile for building x86_64-unknown-linux binaries

FROM rustlang/rust:nightly

RUN apt-get update && apt-get install -y cmake build-essential
RUN apt-get update && apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev
