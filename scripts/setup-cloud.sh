#!/usr/bin/env bash
# Prepares a fresh (cloud) machine for jepa_games development.
# Idempotent: safe to run at the start of every session.
#
#   scripts/setup-cloud.sh            Rust only (phases 1-4)
#   scripts/setup-cloud.sh --python   + uv, Python 3.12 and python/.venv with jepa_water_sort (phase 5, 7)
#   scripts/setup-cloud.sh --web      + wasm32 target and wasm-bindgen-cli (phase 6; needs Node)
set -euo pipefail

cd "$(dirname "$0")/.."

want_python=false
want_web=false
for arg in "$@"; do
  case "$arg" in
    --python) want_python=true ;;
    --web) want_web=true ;;
    *) echo "unknown option: $arg" >&2; exit 2 ;;
  esac
done

if ! command -v rustup >/dev/null 2>&1; then
  echo "==> installing rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none
fi
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

echo "==> installing the toolchain pinned in rust-toolchain.toml"
rustup toolchain install
rustc --version
cargo --version

echo "==> fetching crates"
cargo fetch --locked

if $want_python; then
  if ! command -v uv >/dev/null 2>&1; then
    echo "==> installing uv"
    curl -LsSf https://astral.sh/uv/install.sh | sh
    export PATH="$HOME/.local/bin:$PATH"
  fi
  uv python install 3.12
  echo "==> building the jepa_water_sort Python package into python/.venv"
  (
    cd python
    [ -d .venv ] || uv venv --python 3.12
    uv pip install maturin -r pyproject.toml --extra test
    uv run --no-project maturin develop --release --locked
  )
fi

if $want_web; then
  rustup target add wasm32-unknown-unknown
  want=$(scripts/wasm-bindgen-version.sh)
  if [ "$(wasm-bindgen --version 2>/dev/null | awk '{print $2}')" != "$want" ]; then
    echo "==> installing wasm-bindgen-cli $want (the version in Cargo.lock)"
    cargo install wasm-bindgen-cli --version "$want" --locked
  fi
  if [ ! -f web/app/package-lock.json ]; then
    :
  elif command -v npm >/dev/null 2>  if command -v npm >/dev/null 2>&1; then1; then
    echo "==> installing the web app's npm packages"
    (cd web/app && npm ci)
  else
    echo "warning: node/npm not found; phase 6 needs Node.js >= 20" >&2
  fi
fi

echo "==> quick check"
cargo build --workspace --locked
echo "setup done"
