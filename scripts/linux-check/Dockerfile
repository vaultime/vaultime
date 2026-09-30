# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Ubuntu image with the Tauri build dependencies and rustup. check.sh installs
# the Rust toolchain pinned in rust-toolchain.toml when it runs.
FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_HOME=/opt/cargo \
    RUSTUP_HOME=/opt/rustup \
    PATH=/opt/cargo/bin:$PATH

RUN apt-get update -qq \
 && apt-get install -y -qq curl ca-certificates build-essential pkg-config \
      libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
 && rm -rf /var/lib/apt/lists/*

RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none

COPY check.sh /usr/local/bin/check.sh
