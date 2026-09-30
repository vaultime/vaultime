#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Uploads the installers of a release to the download server and points the
# latest links and the updater at them. The release workflow runs it once
# every check passed.
#
#   scripts/publish-downloads.sh <version> <folder with the release files>
#
# The folder holds what tauri-action attached to the draft release: the
# installers, their .sig files and latest.json. Needs rsync, jq and an SSH
# key that signs in as the upload user. The server limits that user to its
# download folder, so remote paths are relative to it.
set -euo pipefail
shopt -s nullglob

version=${1:?usage: publish-downloads.sh <version> <folder>}
version=${version#v}
source=${2:?usage: publish-downloads.sh <version> <folder>}
host=${VAULTIME_RELEASE_HOST:-vaultime-release@vaultime.codfishcloud.de}
base=${VAULTIME_DOWNLOADS_URL:-https://vaultime.codfishcloud.de/downloads}

# Exactly one file matches each pattern, or the release is incomplete.
one() {
  local matches=("$source"/$1)
  if [ ${#matches[@]} -ne 1 ]; then
    echo "expected one $1 in $source, found ${#matches[@]}" >&2
    return 1
  fi
  echo "${matches[0]}"
}

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir "$stage/$version" "$stage/latest"

# Stable names for the download page. The server makes uploaded links
# harmless, so these are copies.
declare -A aliases=(
  ["*-setup.exe"]=Vaultime-setup.exe
  ["*.msi"]=Vaultime.msi
  ["*.deb"]=Vaultime.deb
  ["*.rpm"]=Vaultime.rpm
  ["*.AppImage"]=Vaultime.AppImage
)
for pattern in "${!aliases[@]}"; do
  file=$(one "$pattern")
  signature=$(one "$pattern.sig")
  cp "$file" "$signature" "$stage/$version/"
  cp "$file" "$stage/latest/${aliases[$pattern]}"
done

# The installer an updater platform gets. The links in latest.json can be
# release asset ids instead of file names, so the platform decides.
installer_for() {
  case "$1" in
    windows-*-msi) one "*.msi" ;;
    windows-*) one "*-setup.exe" ;;
    linux-*-deb) one "*.deb" ;;
    linux-*-rpm) one "*.rpm" ;;
    linux-*) one "*.AppImage" ;;
    *) echo "no installer known for the platform $1" >&2; return 1 ;;
  esac
}

# The updater downloads from this version's folder.
manifest=$(one latest.json)
jq -e --arg version "$version" '.version | ltrimstr("v") == $version' "$manifest" >/dev/null \
  || { echo "latest.json is not for $version" >&2; exit 1; }
for platform in windows-x86_64 linux-x86_64; do
  jq -e --arg platform "$platform" '.platforms | has($platform)' "$manifest" >/dev/null \
    || { echo "latest.json has no $platform update" >&2; exit 1; }
done
cp "$manifest" "$stage/latest.json"
for platform in $(jq -r '.platforms | keys[]' "$manifest"); do
  installer=$(installer_for "$platform")
  name=${installer##*/}
  # The signature proves the platform and the file belong together.
  jq -e --arg platform "$platform" --rawfile signature "$installer.sig" \
    '.platforms[$platform].signature == ($signature | rtrimstr("\n"))' "$manifest" >/dev/null \
    || { echo "latest.json signs another file than $name for $platform" >&2; exit 1; }
  jq --arg platform "$platform" --arg url "$base/$version/$name" \
    '.platforms[$platform].url = $url' "$stage/latest.json" >"$stage/latest.next"
  mv "$stage/latest.next" "$stage/latest.json"
done

# Files first, then the links, then the updater, so nothing points at a file
# that is not there yet.
rsync -rt --chmod=D755,F644 "$stage/$version/" "$host:$version/"
rsync -rt --delete --chmod=D755,F644 "$stage/latest/" "$host:latest/"
rsync -t --chmod=F644 "$stage/latest.json" "$host:latest.json"
echo "published $version to $base"
