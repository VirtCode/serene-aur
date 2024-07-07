FROM rust:alpine3.18 as builder

WORKDIR /app

RUN apk add --no-cache musl-dev
# install nettle for signing
RUN apk add --no-cache nettle-dev clang pkgconfig

# install sqlx cli
RUN cargo install sqlx-cli --no-default-features --features rustls,sqlite

# run build
COPY . .
ENV DATABASE_URL=sqlite:///app/database.db

# we have to disable static linking, otherwise sequoia won't build under alpine with nettle backend
ENV RUSTFLAGS -Ctarget-feature=-crt-static

RUN cargo sqlx database setup --source server/migrations
RUN cargo build --release --bin serene

FROM alpine:3.18 as runner

WORKDIR /app

# copy files over
COPY --from=builder /app/target/release/serene /usr/bin/serene

# install required utilities and libraries
RUN apk add git pacman binutils nettle

# create local user
RUN adduser -D --uid 1000 user

CMD serene
