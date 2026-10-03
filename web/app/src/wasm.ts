// The Rust core compiled to WASM (web/pkg, built by web/build.sh). Every rule, move count,
// star and generation runs there; this app only renders and forwards clicks.
import init from '../../pkg/water_sort_web.js';

export * from '../../pkg/water_sort_web.js';

let ready: Promise<unknown> | null = null;

/** Loads the wasm module once. */
export function loadWasm(): Promise<unknown> {
  ready ??= init();
  return ready;
}
