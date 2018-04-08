# Dockerfile for running answeredthis

FROM rustlang/rust:nightly

RUN apt-get update && apt-get install -y pkg-config libssl-dev sqlite3 libsqlite3-dev

ADD . /app
WORKDIR /app
EXPOSE 80

CMD RUST_BACKTRACE=1 APP_PRODUCTION=1 ./answeredthis
