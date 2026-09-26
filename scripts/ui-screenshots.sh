#!/usr/bin/env bash
# Screenshots of the UI with sample data, for reviewing design changes.
#
#   cd apps/desktop && VITE_MOCK_IPC=1 npx vite build --outDir dist-mock
#   npx vite preview --outDir dist-mock --port 4173 &
#   bash ../../scripts/ui-screenshots.sh <output-folder> [page ...]
#
# Set HEIGHT to capture more of a long page, the default is 900.
# Uses the Microsoft Edge that ships with Windows, in headless mode.
set -euo pipefail

out=${1:?usage: ui-screenshots.sh <output-folder> [page ...]}
shift
pages=("$@")
if [ ${#pages[@]} -eq 0 ]; then
  pages=(library journal cloud settings library/game-1)
fi

edge="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
height=${HEIGHT:-900}
mkdir -p "$out"
out_win=$(cygpath -w "$out" 2>/dev/null || echo "$out")

for page in "${pages[@]}"; do
  name=$(echo "$page" | tr '/?=&' '----')
  "$edge" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --window-size="1440,$height" --virtual-time-budget=4000 \
    --screenshot="$out_win\\$name.png" "http://localhost:4173/$page" >/dev/null 2>&1
  echo "$out/$name.png"
done
