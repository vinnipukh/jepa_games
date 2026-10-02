# Phase 7 — JEPA preparation (Python, PyTorch)

## Goal

Train an action-conditioned JEPA world model on Water Sort trajectories, use it for latent-space planning, and evaluate it against baselines, broken down by difficulty and generator. This phase is outside the Rust side. It consumes the Phase 4 datasets and Phase 5 trajectories and env.

## Layout

```
python/jepa/          # separate package from the binding; depends on jepa_water_sort
├── data.py           # Parquet shard loader → one-hot tensors, (s, a, s', done)
├── models.py         # Encoder, Predictor, TargetEncoder (EMA), IDM, Probe, SolvedHead
├── train.py          # training loop, config via hydra or plain dataclasses
├── monitor.py        # variance, effective rank, probe accuracy
├── plan.py           # beam search, MCTS, CEM over latents
├── baselines.py      # random, greedy, DQN/DDQN (masked), solver
└── eval.py           # evaluation matrix + report
```

## 7.1 Model

- **Input:** the one-hot `(n_tubes, capacity, n_colors + 1)` tensor.
- **Encoder `E`:** a per-tube encoder (shared MLP or 1D conv over slots) followed by a set/transformer layer across tubes **without positional tube embeddings**, which makes it permutation-equivariant over tubes. Water Sort is symmetric under tube reordering, and building that in saves data. Tube-wise latents are pooled to `z`; the tube-wise latents are kept for the predictor.
- **Predictor `P(z_t, a_t)`:** the action is `(from, to)`, so it is injected as learned "source" / "target" flags on the corresponding tube tokens, not as a flat `Discrete(n²)` embedding. This keeps it equivariant and generalizes across `n_tubes`.
- **Target:** EMA target encoder (momentum about 0.996 → 1.0, cosine schedule), with stop-gradient. Loss: smooth-L1 or cosine between `P(z_t, a_t)` and `sg(E_target(s_{t+1}))`, plus multi-step rollout loss (k = 1..3) for planning stability.
- **IDM head `(z_t, z_{t+1}) → a_t`:** cross-entropy over legal actions only (masked), weight about 0.1. Cheap anti-collapse regularizer.
- **Probe head:** predicts per-cell color from tube latents. It always trains on detached latents for monitoring, and optionally also as an auxiliary loss with gradients (an ablation flag).
- **Solved head:** binary classifier `z → is_solved`. Used as the planning goal (7.3).

## 7.2 Collapse monitoring (every epoch, logged to TensorBoard / W&B)

- Per-dimension std of `z` over a fixed held-out batch (alert if the mean is below 0.01).
- Effective rank (exp of the entropy of normalized singular values) of the batch embedding matrix.
- Probe accuracy (detached probe) on held-out states: cell accuracy and full-state exact match.
- IDM accuracy on held-out transitions.

## 7.3 Planning

- **Goal:** solved states are not unique (any tube can hold any color), so there is no single goal latent. Primary approach: maximize `SolvedHead(ẑ)`. Alternative: the distance to the nearest canonical solved state's latent, which the permutation-equivariant encoder makes well defined up to pooling.
- **Search:** beam search over latent rollouts with legal-action masks taken from the **real** env at the root. Deeper nodes use a learned legality head or the predictor alone, as an ablation. MCTS with value = solved-head probability. CEM over action sequences as an alternative.
- Plans execute with replanning every step (MPC style) in the real `WaterSortEnv`.

## 7.4 Baselines

- Random legal policy.
- Greedy: the legal move minimizing `color_changes` after the move, ties broken by `segments` and then randomly.
- DQN / DDQN with action masking (masked argmax, masked targets) on the Phase 5.2 reward, optionally with potential shaping.
- Solver: the optimal upper bound (stars = 5, ratio = 1.0 by definition).

## 7.5 Evaluation

- Metrics: solve rate, mean stars (Phase 1.5 function through the binding), mean `moves / opt_moves` (solved episodes only, and also reported with failures at the move limit).
- Breakdown: by `opt_moves` bucket (quartiles of the test set) and by generator.
- **Cross-evaluation matrix:** {train uniform, train turan} × {test uniform, test turan}, with cross-generator leakage removed (Phase 4). Both generators cover the same puzzles (standard layout) with different distributions (D1, Phase 3), so the off-diagonal cells measure distribution-shift generalization. Report results matched by `opt_moves` bucket, so a gap is not just a difficulty difference.
- Optional: a sweep over Turan `steps` as a controllable shift axis, from near-solved puzzles to well-mixed ones.
- **Layout axis (D14):** run the matrix per layout (standard, distributed), plus {train standard} × {test distributed} and the reverse for each generator, to measure transfer across starting layouts.
- All evaluation uses fixed test puzzles (puzzle codes) and fixed policy seeds; 3 training seeds per model, reported as mean ± std.

## Data interface requirements (on earlier phases)

- Phase 4: test sets per generator with leakage exclusion; puzzle codes in every record.
- Phase 5: Parquet trajectory shards with source labels; native vector env fast enough for planning evaluation (target ≥ 100k steps/s batched).

## Tasks

1. [ ] Data loader + small trajectory dataset (optimal + random + ε = 0.2)
2. [ ] Encoder / predictor / EMA, single-step loss; monitoring
3. [ ] IDM + probe + solved head; collapse ablations
4. [ ] Baselines (random, greedy, solver), then DQN/DDQN
5. [ ] Planning (beam first, then MCTS / CEM)
6. [ ] Evaluation matrix + report

## Acceptance (proposed; the roadmap defines none for this phase)

- No collapse: effective rank > 50 % of the latent dim, and detached probe exact-match > 95 % on held-out states.
- The planner beats greedy in solve rate on the test set of the default configuration.
- The full evaluation matrix is produced by one command.
