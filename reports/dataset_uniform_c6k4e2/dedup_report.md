# Dataset report: fisher_yates / 6 colors, capacity 4, 2 empty (standard)

Generator `uniform` v1 (`uniform:standard`), master seed `88f5e8812f916163` (fresh), seed_i = splitmix64(master_seed ^ i). `{"min_opt":1,"max_attempts":10000,"max_states":5000000,"metrics":{"random_rollouts":64,"rollout_cap_factor":4,"dead_end_ratios":false}}`. Tool: water_sort_cli 0.1.0+a61b1da8fb64. Wall time 667.8 s on 4 threads.

## Duplicates

| generated | failed | duplicates dropped | duplicate rate | hash collisions | records |
|---:|---:|---:|---:|---:|---:|
| 1000000 | 0 | 71 | 0.0071 % | 0 | 999929 |

Duplicates are records whose canonical form (tube order and color names ignored, D8) equals an earlier record's; the lowest `record_id` is kept. Dropped ids are listed in `duplicates.csv`. The saturation curve shows how the duplicate count grows with the number of generated puzzles: a rate that rises with `count` means the configuration's puzzle space is being exhausted.

| generated | duplicates | rate |
|---:|---:|---:|
| 10 | 0 | 0.0000 % |
| 100 | 0 | 0.0000 % |
| 1000 | 0 | 0.0000 % |
| 10000 | 0 | 0.0000 % |
| 100000 | 2 | 0.0020 % |
| 1000000 | 71 | 0.0071 % |

## Split

Rule: bucket = `canonical_hash % 100`; ranges 80,10,10.

| split | buckets | records | share |
|---|---|---:|---:|
| train | [0, 80) | 799272 | 79.93 % |
| val | [80, 90) | 100101 | 10.01 % |
| test | [90, 100) | 100556 | 10.06 % |

## Tiers (D16)

| split | easy | medium | hard |
|---|---:|---:|---:|
| train | 291283 (36.4 %) | 205470 (25.7 %) | 302519 (37.8 %) |
| val | 36471 (36.4 %) | 25725 (25.7 %) | 37905 (37.9 %) |
| test | 36610 (36.4 %) | 25886 (25.7 %) | 38060 (37.8 %) |
| all | 364364 (36.4 %) | 257081 (25.7 %) | 378484 (37.9 %) |

## Files

| file | split | records | bytes | sha256 |
|---|---|---:|---:|---|
| `train.parquet` | train | 799272 | 50073917 | `46c3c73650c15f093fe3ea2f5261b629f7fd5f47250cb95af90a54e1da533a37` |
| `val.parquet` | val | 100101 | 6304767 | `03c088d07831664f9e84d421cb6d791916774edf54326d6937444e127ed7255c` |
| `test.parquet` | test | 100556 | 6338925 | `5de2b3f6a69ccf8a573a0efed5f5226d32acb682822bd513181b6bd28dac79cd` |
