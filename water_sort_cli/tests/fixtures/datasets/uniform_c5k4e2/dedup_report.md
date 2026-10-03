# Dataset report: fisher_yates / 5 colors, capacity 4, 2 empty (standard)

Generator `uniform` v1 (`uniform:standard`), master seed `00005eed20261003` (given), seed_i = splitmix64(master_seed ^ i). `{"min_opt":1,"max_attempts":10000,"max_states":5000000,"metrics":{"random_rollouts":64,"rollout_cap_factor":4,"dead_end_ratios":false}}`. Tool: fixture. Wall time 3.4 s on 2 threads.

## Duplicates

| generated | failed | duplicates dropped | duplicate rate | hash collisions | records |
|---:|---:|---:|---:|---:|---:|
| 200 | 0 | 0 | 0.0000 % | 0 | 200 |

Duplicates are records whose canonical form (tube order and color names ignored, D8) equals an earlier record's; the lowest `record_id` is kept. Dropped ids are listed in `duplicates.csv`. The saturation curve shows how the duplicate count grows with the number of generated puzzles: a rate that rises with `count` means the configuration's puzzle space is being exhausted.

| generated | duplicates | rate |
|---:|---:|---:|
| 10 | 0 | 0.0000 % |
| 100 | 0 | 0.0000 % |
| 200 | 0 | 0.0000 % |

## Split

Rule: bucket = `canonical_hash % 100`; ranges 80,10,10.

| split | buckets | records | share |
|---|---|---:|---:|
| train | [0, 80) | 160 | 80.00 % |
| val | [80, 90) | 23 | 11.50 % |
| test | [90, 100) | 17 | 8.50 % |

## Tiers (D16)

| split | easy | medium | hard |
|---|---:|---:|---:|
| train | 61 (38.1 %) | 37 (23.1 %) | 62 (38.8 %) |
| val | 11 (47.8 %) | 6 (26.1 %) | 6 (26.1 %) |
| test | 7 (41.2 %) | 4 (23.5 %) | 6 (35.3 %) |
| all | 79 (39.5 %) | 47 (23.5 %) | 74 (37.0 %) |

## Files

| file | split | records | bytes | sha256 |
|---|---|---:|---:|---|
| `train.parquet` | train | 160 | 18374 | `f93edb0e7d31885c13b8c727ca35670530b4fc3c7f05fb5310e517ee70c5bdc6` |
| `val.parquet` | val | 23 | 10685 | `92187f6df8594aa0738fec4d15c1585d95ebedd4099fd5a3fec48aee087ccda3` |
| `test.parquet` | test | 17 | 10381 | `ccedd9feb0fe51fec20fc37faea08b50b7a38cd124a5b3f22a65e9f4b988bd20` |
