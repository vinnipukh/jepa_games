# Dataset report: pour_walk(steps=160,layout=distributed) / 4 colors, capacity 3, 1 empty (distributed)

Generator `turan` v1 (`turan:walk:160:distributed`), master seed `00005eed20261003` (given), seed_i = splitmix64(master_seed ^ i). `{"min_opt":1,"max_attempts":10000,"max_states":5000000,"metrics":{"random_rollouts":64,"rollout_cap_factor":4,"dead_end_ratios":false}}`. Tool: fixture. Wall time 0.8 s on 2 threads.

## Duplicates

| generated | failed | duplicates dropped | duplicate rate | hash collisions | records |
|---:|---:|---:|---:|---:|---:|
| 120 | 0 | 1 | 0.8333 % | 0 | 119 |

Duplicates are records whose canonical form (tube order and color names ignored, D8) equals an earlier record's; the lowest `record_id` is kept. Dropped ids are listed in `duplicates.csv`. The saturation curve shows how the duplicate count grows with the number of generated puzzles: a rate that rises with `count` means the configuration's puzzle space is being exhausted.

| generated | duplicates | rate |
|---:|---:|---:|
| 10 | 0 | 0.0000 % |
| 100 | 1 | 1.0000 % |
| 120 | 1 | 0.8333 % |

## Split

Rule: bucket = `canonical_hash % 100`; ranges 80,10,10.

| split | buckets | records | share |
|---|---|---:|---:|
| train | [0, 80) | 101 | 84.87 % |
| val | [80, 90) | 10 | 8.40 % |
| test | [90, 100) | 8 | 6.72 % |

## Tiers (D16)

| split | easy | medium | hard |
|---|---:|---:|---:|
| train | 34 (33.7 %) | 22 (21.8 %) | 45 (44.6 %) |
| val | 6 (60.0 %) | 2 (20.0 %) | 2 (20.0 %) |
| test | 2 (25.0 %) | 2 (25.0 %) | 4 (50.0 %) |
| all | 42 (35.3 %) | 26 (21.8 %) | 51 (42.9 %) |

## Files

| file | split | records | bytes | sha256 |
|---|---|---:|---:|---|
| `puzzles.jsonl` | all | 119 | 107145 | `f9c90fdecb08bce97bb707a75ca74fb03a7fe608a0e9cd74501145d34bb0db74` |
