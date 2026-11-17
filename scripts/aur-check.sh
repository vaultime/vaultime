#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds packaging/aur/PKGBUILD in an Arch Linux container, writes its
# .SRCINFO, checks both with namcap, installs the package and checks that
# the program finds every library and leaves its updates to the AUR, which
# needs a release that reads the package manager file.
# Needs Docker. Run it after changing pkgver and sha256sums.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && { pwd -W 2>/dev/null || pwd; })"

MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$repo/packaging/aur:/aur" \
  archlinux:latest bash -euo pipefail -c '
    pacman -Syu --noconfirm --needed base-devel namcap >/dev/null
    useradd -m builder
    cp -r /aur /home/builder/aur
    chown -R builder /home/builder/aur
    cd /home/builder/aur
    source PKGBUILD
    pacman -S --noconfirm --needed "${depends[@]}" >/dev/null
    su builder -c "makepkg --printsrcinfo > .SRCINFO && makepkg --noconfirm >/dev/null"
    cp .SRCINFO /aur/.SRCINFO
    namcap PKGBUILD ./*.pkg.tar.zst
    pacman -U --noconfirm ./*.pkg.tar.zst >/dev/null
    missing=$(ldd /usr/bin/vaultime | grep "not found" || true)
    if [ -n "$missing" ]; then
      echo "missing libraries: $missing"
      exit 1
    fi
    test "$(cat /usr/lib/vaultime/package-manager)" = "the AUR"
    if ! grep -qa lib/vaultime/package-manager /usr/bin/vaultime; then
      echo "vaultime $pkgver does not read the package manager file yet, publish from the first release that does"
      exit 1
    fi
    echo "$pkgname $pkgver builds, installs and finds its libraries"
  '
