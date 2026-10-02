# Phase 6 — start prompt

Run this after Phase 5 is merged. The acceptance check compares the web against Python.

---

```text
You are implementing Phase 6 (browser game, Rust compiled to WASM) of the jepa_games project.

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-6-web`. Read CLAUDE.md at the repo root first (commands, rules, workflow).

STATE OF THE PROJECT
- Rust workspace, toolchain pinned to 1.99.0 (install rustup if missing; add the
  wasm32-unknown-unknown target). CI: ubuntu + windows Rust jobs, ubuntu heavy-tests, Python job.
- Merged: water_sort_core (rules, Session move counting, stars, solver, canon, puzzle codes,
  golden vectors), uniform_water_sort, turan_water_sort, water_sort_cli, the Phase 4 dataset
  format, and the Phase 5 Python package with golden rollouts
  (water_sort_core/tests/golden/rollouts.json). Read the source before writing code.

READ FIRST, in this order:
1. roadmap.md (Phase 6 section)
2. docs/README.md (cross-cutting rules: one source of game rules, bit-exact reproducibility)
3. docs/decisions.md (D1: core never reads the clock; D5 stars; D11 no wall-clock limits in
   generation; D3 supported range)
4. docs/phases/phase-6-web/PLAN.md (your detailed plan)
5. docs/phases/phase-5-python/PLAN.md section 5.3 (trajectory row format for the export)

SCOPE: new `web/` workspace member (cdylib, wasm-bindgen) and a Vite + TypeScript frontend in
web/app/. All rule enforcement and move counting go through core's Session; TypeScript only
renders and forwards clicks.

KEY REQUIREMENTS (details in PLAN.md)
- wasm-bindgen API: generate(generator, params, seed?, strategy?, layout?), from_code(code), Session
  wrapper (pour, undo, restart, moves_counted, is_solved, stars), legal_moves,
  export_trajectory(). 64-bit seeds and hashes cross the JS boundary as BigInt or hex strings,
  never as JS numbers.
- getrandom 0.4 on wasm32: configure its wasm_js backend (check the exact feature / cfg name for
  the version in Cargo.lock and set it in .cargo/config.toml for the wasm target only).
- Time: std::time::SystemTime::now() panics on wasm32-unknown-unknown. Pass js_sys::Date::now()
  (ms -> ns) into fresh_seed(now_nanos) for Turan. Core never reads the clock.
- Generation and solving run in a Web Worker. A wall-clock timeout is allowed only as a UI guard;
  the accepted puzzle is still decided by the state-count limit (D11).
- Screens: play (tubes, click source then target, invalid target = shake and no move counted,
  move counter, undo, restart, new puzzle, generator + strategy + layout (standard / distributed,
  D14; draw half-empty tubes clearly) selector, params limited to
  is_supported, seed + puzzle code display with copy, open by generator + seed or puzzle code),
  completion (player moves, opt_moves, stars, optimal-solution replay, export, next), shareable
  URL (?code=... or ?gen=...&seed=...&c=..&k=..&e=.., Turan adds strategy and steps).
- Undo/restart rules from Phase 1.5: undo does not decrement the counter, restart keeps it.
- Optional trajectory export (source = "human") in the Phase 5.3 format, download only, plus a
  Python importer in the logger.
- Tests: wasm-bindgen-test (Node) running the same golden vectors and rollouts that Rust and
  Python check; add a wasm CI job (build + wasm-bindgen-test). A short manual UI checklist in
  PLAN.md (undo/restart counting, star boundaries, invalid move feedback).
- Hosting on GitHub Pages is optional: the repo is private, so Pages may need a paid plan; ask
  the user before setting it up.

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits).
- After every task: cargo fmt, clippy -D warnings, cargo test --workspace --locked pass, plus the
  wasm build and wasm tests.
- Tick the checkboxes in docs/phases/phase-6-web/PLAN.md and the roadmap Phase 6 items; update
  the status in docs/README.md when done.
- If the plan is wrong or infeasible, STOP and report with evidence. Record accepted deviations
  as the next free D-number in docs/decisions.md.

DONE WHEN: the same seed or puzzle code opens the same puzzle on the web and in Python (shared
golden vectors pass in the wasm and Python CI jobs, plus a manual spot check); all CI jobs green.
Then push the branch, open a PR to main and report back.
```
