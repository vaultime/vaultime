#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Draws the picture that links to the website and the repository show, from
# assets/social-preview.html, into assets/social-preview.png at 1280 by 640,
# the size GitHub asks for.
#
#   bash scripts/social-preview.sh
#
# Needs `npm ci` in apps/desktop for the fonts. Uses the Microsoft Edge that
# ships with Windows, in headless mode, and serves the page over HTTP, as
# Edge loads no fonts from file URLs.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
port=${PORT:-4175}
# How long the page may render before the capture, in milliseconds.
render_budget_ms=4000
out="$repo/assets/social-preview.png"

python=$(command -v python3 || command -v python)
"$python" -m http.server "$port" --bind 127.0.0.1 --directory "$repo" >/dev/null 2>&1 &
server=$!
trap 'kill "$server"' EXIT
curl -fsS --retry 20 --retry-connrefused --retry-delay 1 -o /dev/null \
  "http://127.0.0.1:$port/assets/social-preview.html"

out_win=$(cygpath -w "$out" 2>/dev/null || echo "$out")
"$edge" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
  --window-size=1280,640 --virtual-time-budget="$render_budget_ms" \
  --screenshot="$out_win" "http://127.0.0.1:$port/assets/social-preview.html" >/dev/null 2>&1
echo "$out"
