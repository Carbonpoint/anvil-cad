#!/usr/bin/env bash
# Regenerate the pictures of the Anvil window used by the documents.
#
# Needs no display and no GPU: the pictures come from the software
# renderer behind the `devtools` feature (crates/anvil-ui/src/shot.rs).
#
#   scripts/ui_shots.sh            writes into docs/images
#   scripts/ui_shots.sh /tmp/out   writes somewhere else
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="${1:-$root/docs/images}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$out"

cargo build --quiet -p anvil-ui --features devtools --example shot
shot="$root/target/debug/examples/shot"

# name | arguments
shots=(
  "window|--sample demo --size 1500x950 --theme light"
  "window_dark|--sample demo --size 1500x950 --theme dark"
  "selected_feature|--sample wb2 --size 1500x950 --theme light --select 1"
  "ribbon_solid|--sample demo --size 1500x950 --theme light --crop 0,0,1500,290"
  "ribbon_field|--sample demo --size 1500x950 --theme light --crop 280,210,470,80"
  "part_navigator|--sample kettle --size 1500x950 --theme light --crop 0,275,230,600"
  "settings_window|--sample demo --size 1500x950 --theme light --settings --crop 10,10,215,265"
  "laptop|--sample wb2 --size 1366x768 --theme light --text 20"
)

for s in "${shots[@]}"; do
  name="${s%%|*}"
  args="${s#*|}"
  # shellcheck disable=SC2086
  "$shot" $args --out "$tmp/$name.ppm"
  python3 "$root/scripts/ppm2png.py" "$tmp/$name.ppm" "$out/ui_$name.png"
  echo "wrote $out/ui_$name.png"
done
