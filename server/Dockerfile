FROM rust:alpine3.18 as builder

WORKDIR /app

# install openssl
RUN apk add --no-cache musl-dev openssl-dev
ENV OPENSSL_DIR=/usr

# run build
COPY . .
RUN cargo build --release --bin serene

FROM alpine:3.18 as runner

WORKDIR /app

# copy files over
COPY --from=builder /app/target/release/serene /usr/bin/serene

# install required utilities
RUN apk add git pacman

CMD serene
