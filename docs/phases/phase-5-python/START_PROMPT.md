# Phase 5 — start prompt

Run this after Phase 4 is merged. Phases 1–3 are enough for the binding and the env, but the logger and the data interface follow the Phase 4 record format.

---

```text
You are implementing Phase 5 (Python binding + Gymnasium environment + trajectory logger) of
the jepa_games project.

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-5-python`. Read CLAUDE.md at the repo root first (commands, rules, workflow).

STATE OF THE PROJECT
- Rust workspace, toolchain pinned to 1.99.0 (install rustup if `cargo` is missing). CI:
  ubuntu + windows (fmt, clippy -D warnings, test) and ubuntu `heavy-tests`.
- Merged: water_sort_core (rules, solver, canon, stars + Session, sampling, seed, metrics,
  puzzle codes, golden vectors in water_sort_core/tests/golden/), uniform_water_sort,
  turan_water_sort (strategies), water_sort_cli (stats, compare, generate, validate, ...),
  and the Phase 4 dataset record format. Read the source before writing code.

READ FIRST, in this order:
1. roadmap.md (Phase 5 section)
2. docs/README.md (cross-cutting rules: one source of game rules, bit-exact reproducibility
   across targets, golden vectors)
3. docs/decisions.md (all entries override the roadmap; D4 move limit k = 4, D5 stars)
4. docs/phases/phase-5-python/PLAN.md (your detailed plan)
5. docs/phases/phase-4-dataset/PLAN.md (record schema the logger must line up with)

SCOPE: new `python/` workspace member (PyO3 cdylib built with maturin) and the
`jepa_water_sort` Python package (binding, env, vector env, policies, logger, tests), plus a
Python CI job. No game logic in Python: every rule goes through the Rust core.

KEY REQUIREMENTS (details in PLAN.md)
- PyO3 + maturin, abi3 wheels (abi3-py310). Use `uv venv --python 3.12` for development. Pick the
  newest PyO3 / numpy crate versions that support the pinned Rust toolchain; record the choice.
- API: generate(generator, params, seed=None, config=None, strategy=None, layout="standard"), solve, step,
  legal_moves, action_mask, stars, canonical_hash, State/Puzzle pyclasses (frozen, picklable via
  puzzle code, to_numpy/from_numpy), batch_step and batch_generate releasing the GIL
  (py.allow_threads + rayon). Map Rust errors to Python exceptions (illegal move -> ValueError,
  stars below optimum -> a dedicated exception).
- WaterSortEnv(generator="uniform"|"turan", params=..., layout="standard"|"distributed" (D14),
  strategy=..., move_limit_k=4, ...):
  observation Box(0,1,(n_tubes, capacity, n_colors+1), int8) one-hot; action Discrete(n_tubes^2)
  with info["action_mask"] and an action_masks() method; illegal action = no state change,
  reward -1, info["illegal"]=True, counts toward the move limit; reward -1 per move with optional
  potential-based shaping gamma*Phi(s') - Phi(s), Phi = -color_changes; termination on solved /
  dead end, truncation at k * opt_moves; reset(seed) reproducible for both generators;
  options={"puzzle_seed": ...} and {"puzzle_code": ...}.
- Native vector env backed by batch_step with autoreset; also works with Gymnasium Sync/Async
  vector envs.
- Trajectory logger: rows (puzzle_id, source, step, state, action, next_state, done, illegal);
  sources optimal / random / epsilon / human; collect only from a given split's puzzles; Parquet
  shards via pyarrow (~1M transitions per shard) with a shard manifest; optional .npz export.
- Parity: Rust writes golden rollouts (water_sort_core/tests/golden/rollouts.json: puzzle code,
  action sequence, expected states / rewards / flags); pytest replays them through `step` and
  through the env with exact equality. Also check the existing Phase 1-3 golden vectors from
  Python.

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits).
- After every task: cargo fmt, clippy -D warnings, cargo test --workspace --locked, and pytest pass.
- Add a CI job on ubuntu + windows: uv -> maturin develop --release -> pytest.
- Tick the checkboxes in docs/phases/phase-5-python/PLAN.md and the roadmap Phase 5 items;
  update the status in docs/README.md when done.
- If the plan is wrong or infeasible, STOP and report with evidence. Record accepted deviations
  as the next free D-number in docs/decisions.md.

DONE WHEN: gymnasium.utils.env_checker.check_env passes; a Python rollout gives exactly the same
result as Rust `step` (golden rollouts); batch_step matches looped step on 10k transitions;
Python CI green on both OSes alongside the existing jobs. Then push the branch, open a PR to
main and report back.
```
