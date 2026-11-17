#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Points the winget manifests in packaging/winget at a published release:
# its version, the hash of its setup.exe and its date. Needs gh. Check the
# result with `winget validate --manifest packaging/winget` before sending it
# to microsoft/winget-pkgs.
set -euo pipefail

version=${1:?usage: winget-manifest.sh <version>}
repo="$(cd "$(dirname "$0")/.." && pwd)"
setup="Vaultime_${version}_x64-setup.exe"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

gh release download "v$version" --repo vaultime/vaultime -p "$setup" -D "$work"
sha=$(sha256sum "$work/$setup" | cut -d' ' -f1 | tr '[:lower:]' '[:upper:]')
date=$(gh release view "v$version" --repo vaultime/vaultime --json publishedAt -q '.publishedAt[:10]')

for manifest in "$repo"/packaging/winget/*.yaml; do
  sed -i -E \
    -e "s/^PackageVersion: .*/PackageVersion: $version/" \
    -e "s|/download/v[^/]+/Vaultime_[^/]+_x64-setup\.exe|/download/v$version/$setup|" \
    -e "s/^(    InstallerSha256: ).*/\1$sha/" \
    -e "s/^ReleaseDate: .*/ReleaseDate: $date/" \
    -e "s|/releases/tag/v[^/]+$|/releases/tag/v$version|" \
    "$manifest"
done
echo "winget manifests point at $version ($sha)"
