#!/usr/bin/env bash
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

# The updater downloads from this version's folder.
manifest=$(one latest.json)
jq --arg base "$base/$version" \
  '.platforms |= map_values(.url = ($base + "/" + (.url | split("/") | last)))' \
  "$manifest" >"$stage/latest.json"
jq -e --arg version "$version" '.version | ltrimstr("v") == $version' "$stage/latest.json" >/dev/null \
  || { echo "latest.json is not for $version" >&2; exit 1; }
for url in $(jq -r '.platforms[].url' "$stage/latest.json"); do
  [ -f "$stage/$version/${url##*/}" ] || { echo "latest.json names ${url##*/}, which is missing" >&2; exit 1; }
done

# Files first, then the links, then the updater, so nothing points at a file
# that is not there yet.
rsync -rt --chmod=D755,F644 "$stage/$version/" "$host:$version/"
rsync -rt --delete --chmod=D755,F644 "$stage/latest/" "$host:latest/"
rsync -t --chmod=F644 "$stage/latest.json" "$host:latest.json"
echo "published $version to $base"
