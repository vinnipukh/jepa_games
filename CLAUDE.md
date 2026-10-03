# jepa_games — agent guide

Water Sort puzzle game with two puzzle generators, plus the infrastructure to train a JEPA world model on it. Rust workspace (game core, generators, CLI, PyO3 binding) plus the `jepa_water_sort` Python package (Gymnasium env, vector env, trajectory logger); the WASM web UI and the JEPA itself come in later phases.

## Where things are

- `roadmap.md`: the original spec, with decided items marked inline.
- `docs/decisions.md`: decision log (D1, D2, ...). **Overrides the roadmap** wherever they differ. Read it before designing anything.
- `docs/README.md`: phase index with status, plus cross-cutting rules.
- `docs/phases/phase-N-<name>/PLAN.md`: detailed plan per phase. `START_PROMPT.md`: the prompt that starts that phase.
- `docs/HANDOFF.md`: how to continue development (cloud sessions).
- `docs/datasets.md`: how to generate datasets, including harder ones.
- `reports/`: committed measurement reports (`water_sort_cli stats` / `compare` output).
- Crates: `water_sort_core` (all game rules, solver, canonical hash, stars, sampling, `Generator` trait), `uniform_water_sort`, `turan_water_sort`, `water_sort_cli` (binary + library: `args`, `dataset` = Phase 4 record schema, Parquet/JSONL, generate, dedup, split, leakage, validate; D17), `python/` (PyO3 crate + `jepa_water_sort` package: binding, `WaterSortEnv`, `WaterSortVectorEnv`, policies, dataset reader, trajectory logger; D19).

## Commands

```bash
scripts/setup-cloud.sh                                   # fresh machine: rustup + pinned toolchain
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --release --workspace --locked -- --ignored   # heavy tests (CI job `heavy-tests`)
WATER_SORT_BLESS=1 cargo test ...                        # regenerate golden vectors (review the diff!)
cargo run --release -p water_sort_cli -- stats --help
cargo run --release -p water_sort_cli -- generate --help   # datasets; also dedup-report, leakage, validate

# Python (from python/; scripts/setup-cloud.sh --python does the first three)
uv venv --python 3.12
uv pip install maturin -r pyproject.toml --extra test
uv run --no-project maturin develop --release --locked     # rebuild after every Rust change
uv run --no-project python -m pytest
uv run --no-project python -m jepa_water_sort.logger collect --help   # trajectories
```

The toolchain is pinned in `rust-toolchain.toml` (1.99.0). `Cargo.lock` is committed; always build with `--locked`.

## Rules that must not be broken

1. **One source of game rules.** Moves, solving, stars, hashing and layouts live only in `water_sort_core`. Generators, the CLI, Python and the web call into it; they never re-implement rules.
2. **Bit-exact reproducibility.** `(generator, VERSION, variant, params, seed, GenConfig)` fully determines a puzzle on every target (native, wasm32, Python). Golden vectors in `*/tests/golden/*.json` enforce it. Never change what an existing seed produces without bumping the generator's `VERSION` and recording a decision.
3. **No clock or nondeterminism in generation.** Core never reads the clock (callers pass `now_nanos`). Solver limits during generation are state counts only, never wall time (D11). No `HashMap` iteration order or thread scheduling may influence results.
4. **Stable hashes.** Never persist `DefaultHasher` output. Canonical hash = xxh3-64 over the exact canonical form (D8).
5. **Own sampling primitives.** Use `bounded_u32` / `fisher_yates` from core, not `rand`'s `gen_range` / `shuffle` (D10).
6. **Shared rejection loop.** Generators use `water_sort_core::attempt_loop`; do not copy the loop (D13).
7. `unsafe` is forbidden workspace-wide.
8. 64-bit values in JSON (seeds, hashes) are 16-digit hex strings, because JavaScript cannot read them exactly.

## Domain facts worth knowing

- Every color has exactly `capacity` units, so free space is always `n_empty × capacity` slots. Supported: `n_empty` 1 or 2 only (D3, `MAX_SUPPORTED_EMPTY`), and `n_colors` limited per `SUPPORTED`. 0 empty tubes has no legal move; 3 is excluded.
- Two layouts (D14): `Standard` (full tubes + whole empty tubes) and `Distributed` (free space spread over tubes, half-empty tubes allowed), in both generators (`water_sort_core::Layout`, Phase 3). Distributed limits: `SUPPORTED_DISTRIBUTED` / `is_supported_in` (D3 addendum, decided).
- Uniform = labeled-uniform over accepted fills (D2). Turan = time-seeded strategies (D1); default `ReverseSearch` (I2A-style search over reverse pours keeping the best-scoring state), plus `Scramble`, `PourWalk` (distributed only) and `Constrained` (D16). A reverse walk alone cannot make more than `n_colors × (capacity − 1)` progressing steps (D16).
- Difficulty tiers easy / medium / hard: `water_sort_core::Tier`, cut points from uniform's `opt_moves` distribution per configuration and layout (D16).
- Environment rules (D19): reward, illegal actions (no state change, −1, counted), termination and truncation are `water_sort_core::episode`; Python only builds observations and `info`. Golden rollouts (`water_sort_core/tests/golden/rollouts.json`) are replayed exactly from pytest.
- Datasets (D17): record `i` has seed `splitmix64(master_seed ^ i)`; dedup by canonical form keeps the lowest `record_id`; split = `canonical_hash % 100` (80/10/10); every record stores its tier. `manifest.json` describes a dataset directory, `validate` re-checks it. Two runs with the same master seed and `--created-at` are byte-identical for any thread count.

## Workflow

- One phase per branch and PR: branch `phase-N-<name>` from an up-to-date `main`, PR into `main`. Small follow-ups go on their own branch and PR.
- One focused commit per plan task, conventional commit messages (`feat`, `fix`, `test`, `docs`, `chore`, `ci`).
- After every task, fmt, clippy and tests must pass locally. CI (ubuntu, windows, heavy-tests) must be green before merging.
- Tick the plan's checkboxes and the roadmap items as you go; update the phase status in `docs/README.md` at the end.
- If a plan turns out wrong or infeasible, stop and report with evidence (counterexample, measurement) before deviating. Accepted deviations become a new numbered entry in `docs/decisions.md`.
- Things that are the user's decision, which you only propose: supported ranges (D3 and addenda), defaults chosen from measurements, anything that publishes or costs money (GitHub Pages, external services such as W&B, long GPU runs). Ask first.
- Commit generated reports to `reports/`; large raw outputs go under `data/` (gitignored).
