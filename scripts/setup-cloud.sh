#!/usr/bin/env bash
# Prepares a fresh (cloud) machine for jepa_games development.
# Idempotent: safe to run at the start of every session.
#
#   scripts/setup-cloud.sh            Rust only (phases 1-4)
#   scripts/setup-cloud.sh --python   + uv and Python 3.12 (phase 5, 7)
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
fi

if $want_web; then
  rustup target add wasm32-unknown-unknown
  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    echo "==> installing wasm-bindgen-cli (match the wasm-bindgen version in Cargo.lock once it exists)"
    cargo install wasm-bindgen-cli --locked
  fi
  command -v node >/dev/null 2>&1 || echo "warning: node not found; phase 6 needs Node.js" >&2
fi

echo "==> quick check"
cargo build --workspace --locked
echo "setup done"
