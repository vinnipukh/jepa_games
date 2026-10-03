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

- **Rust API** (`wasm-bindgen`): `generate(generator, params, seed?, strategy?, layout?) -> PuzzleJs`, `from_code(code)`, a `Session` wrapper (`pour`, `undo`, `restart`, `moves_counted`, `is_solved`, `stars`), `legal_moves`, `export_trajectory()`. All rule enforcement and move counting go through core's `Session` (Phase 1.5), never through TypeScript.
- **getrandom on wasm32:** `getrandom` 0.4 needs only its `wasm_js` feature (no `--cfg` flag any more, D20). Without it the build fails.
- **Time on wasm32:** `std::time::SystemTime::now()` panics on `wasm32-unknown-unknown`. The web crate passes `js_sys::Date::now()` (ms → ns) into `fresh_seed(now_nanos)` for Turan. Core never reads the clock (D1).
- **Web Worker:** generation includes solving, which can take noticeable time for larger configurations. Running it in a worker keeps the UI responsive. The UI shows a spinner, and generation uses a wall-clock limit only as a UI guard. The accepted puzzle is still determined by the state-count limit (D11).
- **Frontend:** plain TypeScript + DOM/SVG tubes, no framework. A small surface, easy to keep the logic out of it.

## Screens

- **Play:** tubes (click source, then target; invalid target → shake animation, no move counted), move counter, Undo, Restart, New puzzle, generator selector (uniform / turan, plus a strategy selector for Turan), layout selector (standard / distributed, D14), parameters (limited to the supported range, D3), puzzle id display (generator + seed, and the puzzle code) with copy button, "Open puzzle" input accepting a generator + seed or a puzzle code.
- **Complete:** player moves, `opt_moves`, stars (1–5), "show optimal solution" replay, export trajectory, next puzzle.
- **Shareable URL:** `?code=<puzzle_code>` or `?gen=uniform&seed=<seed>&c=6&k=4&e=2` (plus `&layout=distributed` when not standard; Turan adds `&strategy=scramble&steps=<n>`). Distributed puzzles start with half-empty tubes; the tube drawing must show partial fills clearly.

## Rules (from Phase 1.5)

The move counter is the total number of pours on this puzzle. Undo restores the state but not the counter. Restart restores the initial state and keeps the counter. Stars are computed from the counter at completion.

## Trajectory export (optional)

JSON in the Phase 5.3 row format with `source = "human"`, plus a session id (random, no personal data), timestamps per move, and undo/restart events as separate rows. The data needed by the JEPA dataset is the effective `(state, action, next_state)`, which is derived on import. Download-only for now, no server.

## Hosting

GitHub Pages deployed by a workflow on pushes to `main` (optional, needs a public repo or Pages enabled).

## Tasks

1. [x] `web` crate with wasm-bindgen API; wasm32 CI build job
2. [x] `wasm-bindgen-test` (Node) running the Phase 1 golden vectors + rollout parity
3. [x] Vite app: play screen, session wiring
4. [x] Worker generation
5. [x] Completion screen + stars + solution replay
6. [x] Seed / code open, shareable URL
7. [x] Trajectory export (optional) + Python importer in `logger.py`
8. [ ] Pages deploy (optional): not set up; the repo is private, so it is the user's decision (D20)

## Tests

- wasm golden vectors: seed → puzzle code, state → canonical hash, rollouts. These are the same files Python checks, so equality across all three targets follows.
- A manual test checklist for the UI (undo/restart counting, stars at boundaries, invalid move feedback).

## Manual UI checklist

Run `web/build.sh`, then `npm run dev` (or `npm run build && npm run preview`) in `web/app`. Checked on 2026-10-03 in Chromium (Playwright-driven, screenshots reviewed):

- [x] Invalid target: the target tube shakes, the status names the rule ("target tube is full"), the counter does not change.
- [x] Clicking an empty tube as the source shakes it; clicking the selected tube again deselects it.
- [x] Undo restores the state and does not decrement the counter; restart restores the start, keeps the counter and disables undo.
- [x] Stars at boundaries: a wasted pour + undo, then the optimal solution gives `opt + 1` moves and the star count core computes (4 stars at opt 19 and opt 21; 3 stars for 12 moves at opt 10, thresholds 1/3/5); the threshold table matches `star_moves` (opt 19: 19 / 20–21 / 22–24 / 25–29 / 30+).
- [x] "Show optimal solution" replays the stored solution; "Next puzzle" loads a fresh puzzle with the counter at 0.
- [x] Distributed puzzles: half-empty tubes read clearly (capacity ticks), 10+ tubes wrap to two rows, "color numbers" labels each unit.
- [x] Shareable URL in the address bar after every load; `?code=` and `?gen=…` open the same puzzle; a pasted link or a lower-case dashed code (`041g-60bq-x600`) opens it; bad links (bad code, unsupported params, `pour_walk` on standard) show an error and load a default puzzle.
- [x] Export trajectory downloads a JSON file that `logger.import_web_exports` accepts (it is `python/tests/fixtures/human_export_uniform_4x4_2.json`).
- [x] Spot check web vs Python (`jepa_water_sort.generate` with the same seed / code): `uniform 3x3_1 seed 0`, `uniform 6x4_2 seed deadbeef`, `uniform distributed 11x4_2 seed ffffffffffffffff tier hard`, `turan scramble(steps=40) 6x4_2 seed 2a`, `turan pour_walk distributed 8x5_2 seed 123456789abcdef0`, `turan reverse_search 12x3_1 seed 7`, `turan constrained distributed 9x4_1 seed 7`, `?code=041G60BQX600`: all eight puzzle codes identical.

## Acceptance

Roadmap gate: the same seed opens the same puzzle on the web and in Python, enforced by the shared golden vectors in both the wasm and Python CI jobs, plus a manual spot check of several seeds and codes.
