# Phase 4 acceptance: 1M-puzzle dataset

Configuration (proposed, D18): uniform generator, standard layout, 6 colors / capacity 4 / 2 empty, `min_opt` 1, default `GenConfig`. Machine: the 4-core cloud session machine (the one that measured the D3 distributed addendum), release build at commit `a61b1da`.

## The single command

```
RAYON_NUM_THREADS=4 water_sort_cli generate --generator uniform --count 1e6 \
    --colors 6 --capacity 4 --empty 2 --split-files --out data/uniform_c6k4e2
```

No `--master-seed`, so it came from uniform's `fresh_seed` (OS entropy): `88f5e8812f916163`. Output (not committed, `data/` is gitignored): `train.parquet`, `val.parquet`, `test.parquet`, `duplicates.csv`, `manifest.json`, `dedup_report.md`. The manifest and report are committed in [`dataset_uniform_c6k4e2/`](dataset_uniform_c6k4e2/).

| | value |
|---|---|
| indices generated | 1 000 000 |
| failures (`TooManyAttempts`) | 0 |
| duplicates dropped | 71 (0.0071 %; 2 at 100k) |
| records | 999 929 |
| split train / val / test | 799 272 / 100 101 / 100 556 |
| tiers easy / medium / hard | 36.4 / 25.7 / 37.9 % (the same in every split) |
| wall time | 667.8 s (11 min 8 s), 4 threads, about 1500 puzzles/s |
| file sizes | train 50.1 MB, val 6.3 MB, test 6.3 MB; 62.7 MB in all, about 63 bytes per record |
| peak memory | not measured; bounded by design (2 windows of 16 384 records in flight, one dedup entry per record) |

Tiers are uniform's cut points (D16), measured on 1000 puzzles, so on 1M they come out at 36 / 26 / 38 % rather than exact thirds: `opt_moves` is discrete and the cut points are the nearest observed values.

## Validation

```
water_sort_cli validate data/uniform_c6k4e2          # --regen-rate 0.01 by default
999929 records checked, 9931 regenerated from their seed: OK      (11.1 s, 4 threads)
```

## Determinism across thread counts

Second run with the same master seed and the first run's `created_at`, on 2 threads:

```
RAYON_NUM_THREADS=2 water_sort_cli generate --generator uniform --count 1e6 \
    --colors 6 --capacity 4 --empty 2 --split-files \
    --master-seed 0x88f5e8812f916163 --created-at 2026-10-03T09:56:56.568667Z \
    --out data/uniform_c6k4e2_t2
```

1302.1 s. `train.parquet`, `val.parquet`, `test.parquet` and `duplicates.csv` are byte-identical to the 4-thread run (`cmp`, and equal sha256 in both manifests). The manifests differ only in `master_seed_source` (`fresh` vs `given`), `started_at`, `finished_at`, `wall_time_secs` and `threads`.

## Cross-generator leakage

Turan with its default strategy (`turan:search:10000:depth=300:standard`) at the same configuration, 20 000 indices, master seed `2026100300000001`, 2 threads: 232.5 s, 385 duplicates dropped (1.93 %; 1.05 % at 10k), 19 615 records, tiers easy / medium / hard 94.4 / 4.5 / 1.1 % (D16: reverse search rarely reaches uniform's upper tiers).

| A | B | shared puzzles | share of B |
|---|---|---:|---:|
| uniform 1M, test | Turan 20k, train | 0 | 0 % |
| uniform 1M, all | Turan 20k, all | 2 | 0.010 % |

At a small configuration, 4 × 4 × 2 (uniform 50 000 indices → 40 157 records, 19.7 % duplicates; Turan 10 000 → 4593, 54.1 % duplicates):

| A | B | shared puzzles | share of B |
|---|---|---:|---:|
| uniform test | Turan train | 0 | 0 % |
| uniform test | Turan all | 188 | 4.09 % |
| uniform all | Turan all | 1694 | 36.9 % |

Test and train never share a puzzle, even across generators, because the split is the same function of the canonical hash in every dataset (D17). Overlap appears only when a side uses all its records, the same split on both sides, or different ranges. `leakage --a <uniform> --a-split test --b <turan> --b-split all --exclude-from b --out <dir>` removed the 188 shared puzzles at 4 × 4 × 2, and the result passes `validate`.
