# Phase 4 — start prompt

Run this after Phase 3 is merged. The cross-generator parts (leakage check, Turan datasets) need it.

---

```text
You are implementing Phase 4 (dataset and storage) of the jepa_games project: one command that
produces a large deduplicated puzzle dataset, its train/val/test split and a duplicate /
leakage report.

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-4-dataset`.

STATE OF THE PROJECT
- Toolchain pinned to 1.99.0 (rust-toolchain.toml; install rustup if `cargo` is missing). CI:
  ubuntu + windows (fmt, clippy -D warnings, test) and ubuntu `heavy-tests` (release, --ignored).
- Merged: water_sort_core (Phase 1), uniform_water_sort + `water_sort_cli stats` (Phase 2),
  turan_water_sort + `stats --generator turan` / `compare` (Phase 3). Both generators implement
  the core Generator trait (generate / generate_traced / generate_observed, fresh_seed) and
  return GeneratedPuzzle (state, seed, generator_id/version/variant, opt_moves, solution,
  metrics, canonical_hash, puzzle_code, attempts). Read the source before writing code.

READ FIRST, in this order:
1. roadmap.md (Phase 4 section)
2. docs/README.md (cross-cutting rules)
3. docs/decisions.md (all entries override the roadmap; note D3 supported range, D8 exact
   canonical hash, D11 no wall-clock limits, and the Phase 3 decisions about Turan defaults)
4. docs/phases/phase-4-dataset/PLAN.md (your detailed plan: record schema, seed derivation,
   ordered parallel writer, dedup, split, leakage, validate)

SCOPE: `water_sort_cli` subcommands `generate`, `dedup-report`, `leakage`, `validate`, plus the
record type (put it in the CLI crate, or in a small new `water_sort_dataset` crate if Python
will need it in Phase 5; decide and record as a D-entry). No Python, no web.

KEY REQUIREMENTS (details in PLAN.md)
- Record schema exactly as in PLAN.md (generator_variant, seed for both generators, gen_config
  JSON, puzzle_code, state as fixed-size binary, solution as action indices, canonical_hash,
  flattened metrics, split, created_at, tool_version with git commit). JSONL (`--format jsonl`)
  and Parquet (default; arrow + parquet crates, zstd, 64k row groups).
- Seeds: seed_i = splitmix64(master_seed ^ i); master seed from the generator's fresh_seed when
  omitted; manifest.json records everything (params, gen_config, generator id/version/variant,
  master seed, counts, split ranges, file list with sha256, tool version, times).
- rayon over index chunks, results written IN INDEX ORDER through a bounded channel to one
  writer: output must be byte-identical across RAYON_NUM_THREADS values (excluding created_at /
  timestamps, which must come from a single run-level clock reading or be excluded from the
  determinism check; decide and document).
- Reject params outside water_sort_core::is_supported unless `--allow-unsupported`.
- Dedup on canonical_hash, keep lowest record_id, confirm duplicates by comparing canonical
  encodings; report counts and the duplicate rate.
- Split: bucket = canonical_hash % 100 -> [0,80) train, [80,90) val, [90,100) test
  (configurable, recorded in the manifest); `split` column plus optional per-split files.
- `leakage --a <dir> --a-split test --b <dir> --b-split train` with `--exclude-from` to drop
  overlapping records from a train set.
- `validate <dir>`: replay every solution to solved in opt_moves, recompute canonical_hash,
  decode puzzle_code == state, and re-generate a sampled 1 % from seed. Run it in CI on a small
  fixture dataset.
- Run the 1M-puzzle acceptance run on a configuration from the supported range (propose one,
  e.g. 6 colors / capacity 4 / 2 empty, record wall time and file sizes in the report and a
  D-entry with status proposed). Do not commit the dataset itself (data/ is gitignored); commit
  the manifest and the dedup report.

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits).
- After every task: cargo fmt, clippy -D warnings, cargo test --workspace --locked pass.
- Tick the checkboxes in docs/phases/phase-4-dataset/PLAN.md and the roadmap Phase 4 items;
  update the status in docs/README.md when done.
- If the plan is wrong or infeasible, STOP and report with evidence before deviating. Record
  accepted deviations as the next free D-number in docs/decisions.md.

DONE WHEN: 1M puzzles, the duplicate report and the split files are produced with a single
command; `validate` passes on that output; two runs with the same master seed and different
RAYON_NUM_THREADS give identical data; CI green on ubuntu, windows and heavy-tests. Then push
the branch, open a PR to main and report back.
```
