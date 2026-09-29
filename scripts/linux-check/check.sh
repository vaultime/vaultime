#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Runs the Linux CI checks for both Rust crates inside the container.
set -uo pipefail
status=0

run() {
  echo "== $*"
  "$@" || status=1
}

mkdir -p /src/apps/desktop/dist

cd /src/apps/desktop/src-tauri
rustup toolchain install >/dev/null
run cargo fmt --check
run cargo clippy --locked --all-targets -q -- -D warnings
run cargo test --locked -q

cd /src/apps/api
run cargo fmt --check
run cargo clippy --locked --all-targets -q -- -D warnings
run cargo test --locked -q

if [ "$status" -eq 0 ]; then echo "Linux checks passed"; else echo "Linux checks FAILED"; fi
exit "$status"
