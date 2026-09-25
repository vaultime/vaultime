#!/usr/bin/env bash
# Runs the Linux CI checks locally in Docker, useful when developing on Windows.
# Build output and the cargo registry live in Docker volumes, so reruns are fast.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd -W 2>/dev/null || pwd)"

docker build -q -t vaultime-linux-check "$repo/scripts/linux-check" >/dev/null

MSYS_NO_PATHCONV=1 docker run --rm \
  --mount "type=bind,source=$repo,target=/src" \
  -e CARGO_TARGET_DIR=/target \
  -v vaultime-linux-target:/target \
  -v vaultime-linux-registry:/opt/cargo/registry \
  -v vaultime-linux-toolchains:/opt/rustup \
  vaultime-linux-check bash /usr/local/bin/check.sh
