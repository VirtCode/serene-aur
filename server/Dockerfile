FROM rustlang/rust:nightly-alpine3.22 as builder

WORKDIR /app

RUN apk add --no-cache musl-dev
# install nettle for signing
RUN apk add --no-cache nettle-dev clang clang-libclang pkgconfig
# install pacman for libalpm
RUN apk add --no-cache pacman-dev
# install git for dynamic version
RUN apk add --no-cache git

# install sqlx cli
RUN cargo install sqlx-cli --no-default-features --features rustls,sqlite

# run build
COPY . .
ENV DATABASE_URL=sqlite:///app/database.db

# we have to disable static linking, otherwise sequoia won't build under alpine with nettle backend
ENV RUSTFLAGS -Ctarget-feature=-crt-static

RUN cargo sqlx database setup --source server/migrations
RUN cargo build --release --bin serene

FROM alpine:3.22 as runner

WORKDIR /app

# copy files over
COPY --from=builder /app/target/release/serene /usr/bin/serene

# install required utilities and libraries
RUN apk add --no-cache git binutils nettle pacman

# create local user
RUN adduser -D --uid 1000 user

CMD serene
