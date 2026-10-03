#!/usr/bin/env bash
# Prints the wasm-bindgen version locked in Cargo.lock. wasm-bindgen-cli (and its test runner)
# must match it exactly.
set -euo pipefail
cd "$(dirname "$0")/.."
awk '/^name = "wasm-bindgen"$/ { getline; gsub(/"/, "", $3); print $3; exit }' Cargo.lock
