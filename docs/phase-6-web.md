# Phase 6 — Web UI (WASM)

## Goal

A browser game for human players that runs the same Rust core and generators compiled to WASM. Same seed or puzzle code → same puzzle as in Python.

## Design

```
web/
├── Cargo.toml        # cdylib, wasm-bindgen, getrandom (wasm_js backend)
├── src/lib.rs        # #[wasm_bindgen] API
└── app/              # Vite + TypeScript frontend
    ├── index.html
    ├── src/main.ts
    ├── src/worker.ts # generation + solving off the main thread
    └── src/ui/*.ts
```

- **Rust API** (`wasm-bindgen`): `generate(generator, params, seed?) -> PuzzleJs`, `from_code(code)`, a `Session` wrapper (`pour`, `undo`, `restart`, `moves_counted`, `is_solved`, `stars`), `legal_moves`, `export_trajectory()`. All rule enforcement and move counting go through core's `Session` (Phase 1.5), never through TypeScript.
- **getrandom on wasm32:** `getrandom` with the `wasm_js` feature and `--cfg getrandom_backend="wasm_js"` in `.cargo/config.toml` for the wasm target. Without it the build fails.
- **Web Worker:** generation includes solving, which can take noticeable time for larger configurations. Running it in a worker keeps the UI responsive. The UI shows a spinner, and generation uses a wall-clock limit only as a UI guard. The accepted puzzle is still determined by the state-count limit (D11).
- **Frontend:** plain TypeScript + DOM/SVG tubes, no framework. A small surface, easy to keep the logic out of it.

## Screens

- **Play:** tubes (click source, then target; invalid target → shake animation, no move counted), move counter, Undo, Restart, New puzzle, generator selector (uniform / turan), parameters (limited to the supported range, D3), puzzle id display (seed for uniform, puzzle code for both) with copy button, "Open puzzle" input accepting a seed (uniform) or a puzzle code.
- **Complete:** player moves, `opt_moves`, stars (1–5), "show optimal solution" replay, export trajectory, next puzzle.
- **Shareable URL:** `?code=<puzzle_code>` or `?gen=uniform&seed=<seed>&c=6&k=4&e=2`.

## Rules (from Phase 1.5)

The move counter is the total number of pours on this puzzle. Undo restores the state but not the counter. Restart restores the initial state and keeps the counter. Stars are computed from the counter at completion.

## Trajectory export (optional)

JSON in the Phase 5.3 row format with `source = "human"`, plus a session id (random, no personal data), timestamps per move, and undo/restart events as separate rows. The data needed by the JEPA dataset is the effective `(state, action, next_state)`, which is derived on import. Download-only for now, no server.

## Hosting

GitHub Pages deployed by a workflow on pushes to `main` (optional, needs a public repo or Pages enabled).

## Tasks

1. [ ] `web` crate with wasm-bindgen API; wasm32 CI build job
2. [ ] `wasm-bindgen-test` (Node) running the Phase 1 golden vectors + rollout parity
3. [ ] Vite app: play screen, session wiring
4. [ ] Worker generation
5. [ ] Completion screen + stars + solution replay
6. [ ] Seed / code open, shareable URL
7. [ ] Trajectory export (optional) + Python importer in `logger.py`
8. [ ] Pages deploy (optional)

## Tests

- wasm golden vectors: seed → puzzle code, state → canonical hash, rollouts. These are the same files Python checks, so equality across all three targets follows.
- A manual test checklist for the UI (undo/restart counting, stars at boundaries, invalid move feedback).

## Acceptance

Roadmap gate: the same seed opens the same puzzle on the web and in Python, enforced by the shared golden vectors in both the wasm and Python CI jobs, plus a manual spot check of several seeds and codes.
