# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: AGPL-3.0-or-later

# The Vaultime server. Build it with this folder as the context:
#   docker build -t vaultime-server apps/api

# The Rust of rust-toolchain.toml at the root of the repository.
FROM rust:1.98-slim-trixie AS builder
WORKDIR /build

# Dependencies build in a layer of their own, so a change to the sources only
# rebuilds the server itself.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
 && echo "fn main() {}" > src/main.rs \
 && cargo build --release --locked \
 && rm -r src

COPY src src
COPY migrations migrations
# The copied sources can be older than the placeholder build.
RUN touch src/main.rs \
 && cargo build --release --locked

FROM debian:trixie-slim

LABEL org.opencontainers.image.source="https://github.com/vaultime/vaultime" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later" \
      org.opencontainers.image.title="Vaultime server" \
      org.opencontainers.image.description="Self-hosted backup server for the Vaultime playtime tracker"

# A fixed id, so a folder mounted at /data can be handed to it.
RUN groupadd --gid 10001 vaultime \
 && useradd --uid 10001 --gid vaultime --no-create-home --no-log-init \
      --home-dir /nonexistent --shell /usr/sbin/nologin vaultime \
 && mkdir /data \
 && chown vaultime:vaultime /data

COPY --from=builder /build/target/release/vaultime-api /usr/local/bin/vaultime-api
COPY LICENSE /usr/share/doc/vaultime-server/LICENSE

ENV VAULTIME_API_BIND=0.0.0.0:8080 \
    VAULTIME_BACKUP_ROOT=/data
VOLUME /data
EXPOSE 8080
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/vaultime-api"]
