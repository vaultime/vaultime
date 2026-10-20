#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Uploads the installers of a release to the download server, points the
# latest links at them and writes the updater's latest.json. The release
# workflow runs it once every check passed.
#
#   scripts/publish-downloads.sh [--check] <version> <folder with the release files>
#
# The folder holds what the builds made: the installers and their .sig
# files. latest.json is written into it, for the GitHub release as well.
# --check stops before the upload, so a manual run tests everything else.
# Needs rsync, jq and an SSH key that signs in as the upload user. The
# server limits that user to its download folder, so remote paths are
# relative to it.
set -euo pipefail
shopt -s nullglob

check=false
if [ "${1:-}" = --check ]; then
  check=true
  shift
fi
usage="usage: publish-downloads.sh [--check] <version> <folder>"
version=${1:?$usage}
version=${version#v}
source=${2:?$usage}
host=${VAULTIME_RELEASE_HOST:-vaultime-release@vaultime.codfishcloud.de}
base=${VAULTIME_DOWNLOADS_URL:-https://vaultime.codfishcloud.de/downloads}
notes=${VAULTIME_RELEASE_NOTES:-}

# Exactly one file matches each pattern, or the release is incomplete.
one() {
  # shellcheck disable=SC2206 # the pattern is meant to expand
  local matches=("$source"/$1)
  if [ ${#matches[@]} -ne 1 ]; then
    echo "expected one $1 in $source, found ${#matches[@]}" >&2
    return 1
  fi
  echo "${matches[0]}"
}

# The installers by their names, which carry the version they were built
# as, so a tag that does not match the build stops here.
declare -A installers=(
  [setup]="*_${version}_*-setup.exe"
  [msi]="*_${version}_*.msi"
  [deb]="*_${version}_*.deb"
  [rpm]="*-${version}-*.rpm"
  [appimage]="*_${version}_*.AppImage"
)
# Stable names for the download page. The server makes uploaded links
# harmless, so these are copies.
declare -A aliases=(
  [setup]=Vaultime-setup.exe
  [msi]=Vaultime.msi
  [deb]=Vaultime.deb
  [rpm]=Vaultime.rpm
  [appimage]=Vaultime.AppImage
)
# The installer each updater platform gets, as tauri-action named them.
declare -A platforms=(
  [windows-x86_64]=setup
  [windows-x86_64-nsis]=setup
  [windows-x86_64-msi]=msi
  [linux-x86_64]=appimage
  [linux-x86_64-appimage]=appimage
  [linux-x86_64-deb]=deb
  [linux-x86_64-rpm]=rpm
)

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$version" "$stage/latest"

manifest=$(jq -n --arg version "$version" --arg notes "$notes" \
  --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{version: $version, notes: $notes, pub_date: $date, platforms: {}}')
for kind in "${!installers[@]}"; do
  file=$(one "${installers[$kind]}")
  name=${file##*/}
  signature=$(one "$name.sig")
  # The signed comment names the file, so a signature of another file or
  # version is caught before an updater refuses it.
  if ! base64 -d "$signature" | grep -q "^trusted comment: .*file:$name"$'\t'"version:$version\$"; then
    echo "$name.sig does not sign $name $version" >&2
    exit 1
  fi
  cp "$file" "$signature" "$stage/$version/"
  cp "$file" "$stage/latest/${aliases[$kind]}"
  for platform in "${!platforms[@]}"; do
    [ "${platforms[$platform]}" = "$kind" ] || continue
    manifest=$(jq --arg platform "$platform" --arg url "$base/$version/$name" \
      --rawfile signature "$signature" \
      '.platforms[$platform] = {signature: ($signature | rtrimstr("\n")), url: $url}' <<<"$manifest")
  done
done
jq -S . <<<"$manifest" >"$source/latest.json"
cp "$source/latest.json" "$stage/latest.json"
echo "latest.json for $version: $(jq -r '.platforms | keys | join(", ")' "$source/latest.json")"

if $check; then
  echo "checked $version, nothing uploaded"
  exit 0
fi

# Files first, then the links, then the updater, so nothing points at a file
# that is not there yet.
rsync -rt --chmod=D755,F644 "$stage/$version/" "$host:$version/"
rsync -rt --delete --chmod=D755,F644 "$stage/latest/" "$host:latest/"
rsync -t --chmod=F644 "$stage/latest.json" "$host:latest.json"
echo "published $version to $base"
