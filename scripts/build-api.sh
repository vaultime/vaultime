#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Builds the API release binary for Ubuntu 24.04 in Docker into dist-api/.
# deploy-api.sh and cloud-e2e.sh run it. Build output and the cargo registry
# live in Docker volumes, so reruns are fast.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
repo_mount="$(cd "$repo" && pwd -W 2>/dev/null || pwd)"
mkdir -p "$repo/dist-api"

docker build -q -t vaultime-linux-check "$repo/scripts/linux-check" >/dev/null
# A failed build stops here, so an older binary in the cached target folder
# is never copied out.
MSYS_NO_PATHCONV=1 docker run --rm \
  --mount "type=bind,source=$repo_mount,target=/src,readonly" \
  --mount "type=bind,source=$repo_mount/dist-api,target=/out" \
  -e CARGO_TARGET_DIR=/target \
  -v vaultime-api-target:/target \
  -v vaultime-linux-registry:/opt/cargo/registry \
  -v vaultime-linux-toolchains:/opt/rustup \
  vaultime-linux-check bash -c '
    set -euo pipefail
    cd /src/apps/api
    rustup toolchain install >/dev/null
    cargo build --release --locked -q
    cp /target/release/vaultime-api /out/
  '
