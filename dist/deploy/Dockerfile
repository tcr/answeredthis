# Dockerfile for running answeredthis

FROM rustlang/rust:nightly

RUN apt-get update && apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev

ADD . /app
WORKDIR /app
EXPOSE 8000

CMD RUST_BACKTRACE=1 LD_LIBRARY_PATH=. ./answeredthis
