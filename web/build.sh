#!/usr/bin/env bash
# Builds the wasm module and its JS bindings into web/pkg (gitignored), which the Vite app in
# web/app imports.
#
#   web/build.sh            release build (default)
#   web/build.sh --dev      debug build (faster to compile, much slower to run)
set -euo pipefail
cd "$(dirname "$0")/.."

profile=release
flag=--release
if [ "${1:-}" = "--dev" ]; then
  profile=debug
  flag=
fi

want=$(scripts/wasm-bindgen-version.sh)
have=$(wasm-bindgen --version 2>/dev/null | awk '{print $2}' || true)
if [ "$have" != "$want" ]; then
  echo "error: wasm-bindgen-cli $want is required (found '${have:-none}')." >&2
  echo "       cargo install wasm-bindgen-cli --version $want --locked" >&2
  exit 1
fi

cargo build $flag --locked --target wasm32-unknown-unknown -p water_sort_web
wasm-bindgen --target web --out-dir web/pkg \
  "target/wasm32-unknown-unknown/$profile/water_sort_web.wasm"
echo "wrote web/pkg"
