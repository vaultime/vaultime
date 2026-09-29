#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Installs the packages needed to build the Linux bundles on Ubuntu 22.04.
# Building on the oldest supported base keeps the glibc floor at 2.35, so the
# packages also run on Debian 12, Fedora, Arch, openSUSE and newer Ubuntu.
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive

apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  build-essential ca-certificates curl file git pkg-config xz-utils \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev \
  >/dev/null
