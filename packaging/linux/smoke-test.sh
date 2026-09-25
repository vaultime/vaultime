#!/usr/bin/env bash
# Installs a Vaultime package on the current distro and checks that the app
# starts, migrates its database and keeps running. Meant for throwaway
# containers, run as root.
#
#   smoke-test.sh <package>   a .deb, .rpm or .AppImage file
set -euo pipefail

# Work on a copy, the package may sit on a read-only mount.
work=$(mktemp -d)
cp "$1" "$work/"
package="$work/$(basename "$1")"
. /etc/os-release
echo "== $PRETTY_NAME, $(basename "$package")"

case "$ID" in
  debian|ubuntu)
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -qq
    apt-get install -y -qq xvfb xauth >/dev/null
    [[ "$package" == *.deb ]] && apt-get install -y -qq "$package" >/dev/null
    ;;
  fedora)
    dnf install -y -q xorg-x11-server-Xvfb xorg-x11-xauth xvfb-run >/dev/null
    [[ "$package" == *.rpm ]] && dnf install -y -q "$package" >/dev/null
    ;;
  opensuse*)
    zypper -q -n install xvfb-run >/dev/null
    [[ "$package" == *.rpm ]] && zypper -q -n --no-gpg-checks install "$package" >/dev/null
    ;;
  arch)
    # AppImages rely on the libraries every desktop has, which gtk3 brings in.
    pacman -Sy --noconfirm --needed -q xorg-server-xvfb xorg-xauth gtk3 >/dev/null
    ;;
  *)
    echo "unsupported distro: $ID" >&2
    exit 2
    ;;
esac

if [[ "$package" == *.AppImage ]]; then
  chmod +x "$package"
  app=("$package" --appimage-extract-and-run)
else
  app=(vaultime)
fi

# Containers cannot create the nested namespaces WebKit's sandbox needs.
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1
export LIBGL_ALWAYS_SOFTWARE=1

log_dir="${XDG_DATA_HOME:-$HOME/.local/share}/com.vaultime.app/logs"
out=$(mktemp)
xvfb-run -a "${app[@]}" >"$out" 2>&1 &
runner=$!

started=0
for _ in $(seq 1 60); do
  if grep -qs "tracking engine started" "$log_dir"/*.log; then
    started=1
    break
  fi
  sleep 1
done

# Process ids of the running app. Minimal images lack pgrep.
app_pids() {
  for dir in /proc/[0-9]*; do
    [ "$(cat "$dir/comm" 2>/dev/null)" = vaultime ] && echo "${dir#/proc/}"
  done
  return 0
}

# Give the window and webview time to come up, then make sure nothing died.
sleep 10
alive=0
[ -n "$(app_pids)" ] && alive=1

kill "$runner" $(app_pids) 2>/dev/null || true

if [ "$started" -ne 1 ] || [ "$alive" -ne 1 ] || grep -qiE "panicked|crashed|segmentation" "$out"; then
  echo "smoke test FAILED (started=$started alive=$alive)"
  echo "--- app output"; tail -n 40 "$out"
  echo "--- app log"; tail -n 40 "$log_dir"/*.log 2>/dev/null || true
  exit 1
fi

echo "smoke test passed"
