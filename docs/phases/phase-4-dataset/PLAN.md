# Phase 4 — Dataset and storage

## Goal

One command produces a large deduplicated puzzle dataset, its train/val/test split, and a duplicate/leakage report. It must be deterministic for uniform, and recorded faithfully for Turan.

## Record schema

| field | type | note |
|---|---|---|
| `record_id` | u64 | index within the run |
| `generator_id`, `generator_version` | str, u32 | |
| `params` | struct{n_colors, capacity, n_empty} | |
| `gen_config` | JSON string | min_opt, max_attempts, solver limits (Phase 2 note) |
| `generator_variant` | str | e.g. `fisher_yates`, `scramble(steps=40)` |
| `seed` | u64 | both generators |
| `puzzle_code` | str | |
| `state` | fixed_size_binary(n_tubes × capacity) | row-major, bottom → top, EMPTY = 255 |
| `opt_moves` | u32 | |
| `solution` | list<u16> | action index `from * n_tubes + to` |
| `canonical_hash` | u64 | `canonical_full` (D8) |
| `attempts` | u32 | |
| `metrics.*` | flattened columns | |
| `split` | enum train/val/test | |
| `created_at` | timestamp (UTC) | |
| `tool_version` | str | crate version + git commit |

Written as JSONL (debugging, `--format jsonl`) or Parquet (default; `arrow` + `parquet` crates, zstd compression, row groups of 64k).

## `generate` command

```
water_sort_cli generate --generator uniform --count 1000000 \
    --colors 6 --capacity 4 --empty 2 --min-opt 10 \
    --master-seed 0x... --out data/uniform_c6k4e2/ [--format parquet|jsonl]
```

- **Seed derivation (both generators):** `seed_i = splitmix64(master_seed ⊕ i)`. Any record can be regenerated on its own, and the master seed is recorded in a `manifest.json`. When `--master-seed` is omitted, it comes from the generator's `fresh_seed`: OS entropy for uniform, the time seed for Turan.
- **Parallelism:** rayon over chunks of indices. Each chunk is generated independently and then written **in index order** through a bounded channel to a single writer. The output is byte-identical regardless of thread count, and memory stays bounded at 1M records.
- **Turan:** the same pipeline, plus `--strategy scramble --steps N` (recorded in `generator_variant` and the manifest).
- **Manifest:** params, gen_config, generator id/version, counts, master seed, tool version, start/end time, split ranges, file list with sha256.

## Dedup

- Key: `canonical_hash` (exact canonical form, D8). Keep the lowest `record_id`, drop the rest, and log the dropped ids.
- At 1M records a `HashSet<u64>` is about 16 MB. The chance of a 64-bit hash collision in that set is about `n²/2⁶⁵ ≈ 3·10⁻⁸`, which is negligible. Puzzles flagged as duplicates are confirmed by comparing canonical encodings, so a collision could only cause a false *non*-duplicate, never a wrong drop.
- Report: duplicate count, duplicate rate, and how it changes with `count` (a sign of how saturated the configuration's puzzle space is).

## Split

- `bucket = canonical_hash % 100`: `[0, 80)` train, `[80, 90)` val, `[90, 100)` test. Ranges are configurable and recorded in the manifest.
- Because the hash is canonical, all tube/color permutations of a puzzle fall in the same bucket.
- Written as a `split` column, plus optional separate files per split (`--split-files`).

## Leakage check

```
water_sort_cli leakage --a data/uniform_c6k4e2 --a-split test --b data/turan_c6k4e2 --b-split train
```

Reports the number and fraction of shared canonical hashes. Within one generator the split rule makes this zero by construction. Across generators, overlap is expected for small configurations and must be removed for clean cross-evaluation: `--exclude-from` drops those records from the train set.

## Other subcommands

- `validate <dir>`: re-check every record (replay the solution to solved in `opt_moves` moves, recompute `canonical_hash`, decode the puzzle code and compare it with `state`, re-generate from `seed` and compare; that last check is sampled, by default 1 %, because it re-runs the solver). Run in CI on a small fixture dataset.
- `dedup-report <dir>`: standalone report.

## Tasks

1. [ ] Record struct + serde + JSONL writer
2. [ ] Parquet writer (arrow schema above)
3. [ ] `generate` with seed derivation, ordered parallel writer, manifest
4. [ ] Dedup + report
5. [ ] Split column + split files
6. [ ] `leakage`, `validate`
7. [ ] 1M-record run; record wall time and file sizes in the manifest/report

## Acceptance

Roadmap gate: 1M puzzles, duplicate report, and split files produced with a single command. Plus: `validate` passes on that output, and two runs with the same master seed and different `RAYON_NUM_THREADS` give byte-identical Parquet.

## Risks / open points

- Generation time at 1M depends on the configuration (Phase 2.3). Pick a dataset configuration from the supported range where 1M finishes in reasonable time on the target machine.
