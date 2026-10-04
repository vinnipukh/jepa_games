# JEPA world model: evaluation report

Generated 2026-10-04 09:34 UTC by `python.exe -m jepa.pipeline --preset default --device cuda --extra-puzzles 500 --stages eval report --report reports/jepa_eval_default.md` (preset `default`; torch 2.9.1+rocm7.2.1, device cuda, AMD Radeon RX 7900 XT).

Puzzles: 6 colors × capacity 4 × 2 empty; generators uniform; layouts standard. Training seeds [0] (mean ± std over seeds). Test: the 500 lowest record ids of each test split, move limit 4 · opt_moves, policy seed 0. Planner: beam (depth 4, width 16, score `value`, legality `head`, revisit check `real`).

`opt_moves` buckets (quartiles of all test puzzles pooled): ≤17, 18, 19, >19; pooled weights 38 %, 23 %, 24 %, 15 %.

## Acceptance (default configuration)

| criterion | value | met |
|---|---|---|
| effective rank > 50 % of the latent dim | 19.5 % | **no** |
| detached probe exact match > 95 % (held-out states) | 100.0 % | yes |
| planner solve rate > greedy (uniform_standard test) | 97.0 % vs 77.0 % | yes |

## World model health (collapse monitor, final epoch, held-out val states)

| train set | eff. rank / dim | z std | probe cell | probe exact | IDM acc | legality exact | 1-step exact | 2-step exact | 3-step exact | solved recall | value MAE |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `uniform_standard` | 19.5 % | 0.61 | 100.0 % | 100.0 % | 100.0 % | 100.0 % | 98.7 % | 97.8 % | 96.2 % | 100.0 % | 0.29 |

*k-step exact*: the probe decoding of the k-step latent rollout equals the real state. *value MAE*: distance-to-go error in moves (unsolvable states count as the cap).

## Policies on the default test set (`uniform_standard`)

`jepa-beam` is the planner of the matrix; the other `jepa-*` rows are planner variants (see `PLANNER_VARIANTS` in `jepa/pipeline.py`) on the first 500 test puzzles.

| policy | puzzles | solve rate | mean stars | moves/opt (solved) | moves/opt (all) |
|---|---|---|---|---|---|
| random | 500 | 72.6 % | 0.92 | 1.92 | 2.49 |
| greedy | 500 | 77.0 % | 3.22 | 1.06 | 1.74 |
| ddqn (train `uniform_standard`) | 500 | 96.6 % | 4.40 | 1.03 | 1.13 |
| jepa-beam (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-d1 (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-d2 (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-inconsistent (train `uniform_standard`) | 500 | 96.2 % | 4.75 | 1.00 | 1.12 |
| jepa-beam-model-revisits (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-no-revisits (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-probe-legality (train `uniform_standard`) | 500 | 97.0 % | 4.76 | 1.01 | 1.09 |
| jepa-beam-solved-score (train `uniform_standard`) | 500 | 56.4 % | 0.86 | 2.14 | 2.95 |
| jepa-cem (train `uniform_standard`) | 500 | 96.4 % | 4.76 | 1.00 | 1.11 |
| jepa-mcts (train `uniform_standard`) | 500 | 97.2 % | 4.77 | 1.01 | 1.09 |
| solver | 500 | 100.0 % | 5.00 | 1.00 | 1.00 |

Solve rate by `opt_moves` bucket:

| policy | ≤17 | 18 | 19 | >19 |
|---|---|---|---|---|
| random | 80.7 % | 69.8 % | 69.7 % | 60.3 % |
| greedy | 82.3 % | 77.6 % | 71.4 % | 71.2 % |
| ddqn | 97.9 % | 95.7 % | 95.0 % | 97.3 % |
| jepa-beam | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-d1 | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-d2 | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-inconsistent | 99.0 % | 97.4 % | 95.0 % | 89.0 % |
| jepa-beam-model-revisits | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-no-revisits | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-probe-legality | 99.5 % | 98.3 % | 95.8 % | 90.4 % |
| jepa-beam-solved-score | 63.5 % | 56.9 % | 53.8 % | 41.1 % |
| jepa-cem | 99.0 % | 97.4 % | 95.8 % | 89.0 % |
| jepa-mcts | 99.0 % | 95.7 % | 97.5 % | 94.5 % |
| solver | 100.0 % | 100.0 % | 100.0 % | 100.0 % |

## Cross-evaluation matrix (jepa-beam)

Rows: training set; columns: test set. Each cell: solve rate, then the bucket-matched solve rate in brackets, mean ± std over seeds. Leaked test puzzles (in the model's training puzzles) are removed per cell; their count is listed below the table.

| train \ test | `uniform_standard` |
|---|---|
| `uniform_standard` | 97.0 % (97.0 %) |
| *greedy* | 77.0 % (77.0 %) |
| *random* | 72.6 % (72.6 %) |

Leaked puzzles removed: none.

Mean stars:

| train \ test | `uniform_standard` |
|---|---|
| `uniform_standard` | 4.76 |

## By `opt_moves` bucket, per test set

Test `uniform_standard`:

| policy | ≤17 | 18 | 19 | >19 | puzzles |
|---|---|---|---|---|---|
| ddqn (train `uniform_standard`) | 97.9 % | 95.7 % | 95.0 % | 97.3 % | 500 |
| greedy | 82.3 % | 77.6 % | 71.4 % | 71.2 % | 500 |
| jepa-beam (train `uniform_standard`) | 99.5 % | 98.3 % | 95.8 % | 90.4 % | 500 |
| random | 80.7 % | 69.8 % | 69.7 % | 60.3 % | 500 |
| solver | 100.0 % | 100.0 % | 100.0 % | 100.0 % | 500 |
