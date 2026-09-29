#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later

# Screenshots of the UI with sample data, for reviewing design changes.
#
#   cd apps/desktop && VITE_MOCK_IPC=1 npx vite build --outDir dist-mock
#   npx vite preview --outDir dist-mock --port 4173 &
#   bash ../../scripts/ui-screenshots.sh <output-folder> [page ...]
#
# WIDTH and HEIGHT set the window, 1440 by 900 by default. The app's smallest
# window is 900 by 600.
# Uses the Microsoft Edge that ships with Windows, in headless mode.
set -euo pipefail

out=${1:?usage: ui-screenshots.sh <output-folder> [page ...]}
shift
pages=("$@")
if [ ${#pages[@]} -eq 0 ]; then
  pages=(library journal cloud settings library/game-1)
fi

edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
width=${WIDTH:-1440}
height=${HEIGHT:-900}
mkdir -p "$out"
out_win=$(cygpath -w "$out" 2>/dev/null || echo "$out")

for page in "${pages[@]}"; do
  name=$(echo "$page" | tr '/?=&' '----')
  "$edge" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size="$width,$height" --virtual-time-budget=4000 \
    --screenshot="$out_win\\$name.png" "http://localhost:4173/$page" >/dev/null 2>&1
  echo "$out/$name.png"
done
