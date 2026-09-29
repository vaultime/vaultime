#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Builds the Linux bundles on Ubuntu 22.04 in Docker, then installs and starts
# them on several distros. Output lands in dist-linux/.
#
#   scripts/linux-package.sh            build and test
#   scripts/linux-package.sh --test     only test the bundles in dist-linux/
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd -W 2>/dev/null || pwd)"
out="$repo/dist-linux"
export MSYS_NO_PATHCONV=1

if [ "${1:-}" != "--test" ]; then
  docker build -q -t vaultime-linux-build "$repo/packaging/linux" >/dev/null
  mkdir -p "$out"

  # The sources are copied into the container so the Windows node_modules and
  # target folders stay untouched.
  docker run --rm \
    --mount "type=bind,source=$repo,target=/src,readonly" \
    --mount "type=bind,source=$out,target=/out" \
    -v vaultime-linux-bundle-target:/build/apps/desktop/src-tauri/target \
    -v vaultime-linux-registry:/opt/cargo/registry \
    -v vaultime-linux-npm:/root/.npm \
    -v vaultime-linux-bundle-rustup:/opt/rustup \
    vaultime-linux-build bash -c '
      set -euo pipefail
      tar -C /src --exclude=node_modules --exclude=target --exclude=dist \
        --exclude=dist-linux --exclude=.git -cf - . | tar -C /build -xf -
      cd /build/apps/desktop/src-tauri && rustup toolchain install >/dev/null
      cd /build/apps/desktop && npm ci --no-audit --no-fund >/dev/null
      npx tauri build 2>&1 | grep -E "Finished|Error|error|warning: unused" || true
      rm -f /out/*
      cp src-tauri/target/release/bundle/deb/*.deb \
         src-tauri/target/release/bundle/rpm/*.rpm \
         src-tauri/target/release/bundle/appimage/*.AppImage /out/
    '
fi

deb=$(cd "$out" && ls *.deb)
rpm=$(cd "$out" && ls *.rpm)
appimage=$(cd "$out" && ls *.AppImage)
status=0

smoke() {
  docker run --rm \
    --mount "type=bind,source=$out,target=/pkg,readonly" \
    --mount "type=bind,source=$repo/packaging/linux/smoke-test.sh,target=/smoke-test.sh,readonly" \
    "$1" bash /smoke-test.sh "/pkg/$2" || status=1
}

smoke debian:12 "$deb"
smoke ubuntu:24.04 "$deb"
smoke fedora:latest "$rpm"
smoke archlinux:latest "$appimage"
smoke opensuse/tumbleweed "$appimage"

if [ "$status" -eq 0 ]; then echo "all Linux packages passed"; else echo "Linux package checks FAILED"; fi
exit "$status"
