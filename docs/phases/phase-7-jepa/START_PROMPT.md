# Phase 7 — start prompt

Run this after Phases 4 and 5 are merged. Phase 6 is not required. Training needs a GPU machine for anything beyond small smoke runs.

---

```text
You are implementing Phase 7 (JEPA world model, planning, baselines, evaluation; Python +
PyTorch) of the jepa_games project.

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-7-jepa`. Read CLAUDE.md at the repo root first (commands, rules, workflow).

STATE OF THE PROJECT
- Rust side complete: water_sort_core, uniform and turan generators, water_sort_cli (stats,
  compare, generate, dedup-report, leakage, validate).
- Phase 4: Parquet puzzle datasets with canonical-hash splits, manifests, leakage exclusion.
- Phase 5: `jepa_water_sort` Python package (PyO3): generate / solve / step / action_mask /
  stars / canonical_hash, WaterSortEnv + native vector env, Parquet trajectory shards labeled
  optimal / random / epsilon / human. Read the source and the manifests before writing code.

READ FIRST, in this order:
1. roadmap.md (Phase 7 section)
2. docs/README.md
3. docs/decisions.md (D1: uniform and turan cover the same puzzles with different
   distributions, so cross-evaluation is a real distribution-shift test; D4 move limit; D5 stars)
4. docs/phases/phase-7-jepa/PLAN.md (your detailed plan)
5. docs/phases/phase-4-dataset/PLAN.md and docs/phases/phase-5-python/PLAN.md (data interface)

SCOPE: new Python package python/jepa/ (depends on jepa_water_sort): data loading, models,
training, collapse monitoring, planning, baselines, evaluation. No changes to game rules; any
game logic needed comes from jepa_water_sort.

KEY REQUIREMENTS (details in PLAN.md)
- Data: Parquet shards -> one-hot (n_tubes, capacity, n_colors+1) tensors of (s, a, s', done);
  trajectory splits inherit the puzzle split; start with a small dataset (optimal + random +
  epsilon = 0.2) for the default configuration.
- Model: permutation-equivariant encoder over tubes (shared per-tube encoder + set/transformer
  layer without tube position embeddings); predictor conditioned on (from, to) as source/target
  flags on tube tokens; EMA target encoder with stop-gradient; single- and multi-step (k = 1..3)
  latent prediction loss; masked inverse-dynamics head; detached state probe (optional aux loss
  as an ablation flag); solved-classifier head used as the planning goal.
- Collapse monitoring every epoch: per-dim std, effective rank, probe accuracy (cell and exact
  match), IDM accuracy; log to TensorBoard (W&B optional, ask the user before using any external
  service).
- Planning: beam search over latent rollouts with real legal-action masks at the root, MCTS
  with value = solved probability, CEM as an alternative; MPC-style replanning in the real env.
- Baselines: random legal, greedy (minimize color_changes, tie-break segments), masked DQN/DDQN
  on the Phase 5 reward, solver upper bound.
- Evaluation: solve rate, mean stars (via jepa_water_sort.stars), mean moves / opt_moves, broken
  down by opt_moves bucket and generator; the cross-evaluation matrix {train uniform, train
  turan} x {test uniform, test turan} with leakage removed and results matched by opt_moves
  bucket; optional Turan steps sweep as a shift axis; the layout axis (D14): the matrix per
  layout plus train-standard -> test-distributed and the reverse; fixed test puzzles (puzzle codes) and
  policy seeds; 3 training seeds per model, mean +- std. One command produces the full matrix.
- Reproducibility: config dataclasses (or hydra) saved with every run, seeds logged, small CPU
  smoke test that trains a few steps and runs one evaluation in CI (keep it under a few minutes).

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits).
- Keep Rust CI green; add pytest for the jepa package (data loader shapes, model forward passes,
  planner on a toy model, evaluation metrics against hand-computed values).
- Tick the checkboxes in docs/phases/phase-7-jepa/PLAN.md and the roadmap Phase 7 items; update
  the status in docs/README.md when done.
- Ask the user before long or expensive training runs (state the expected GPU hours). Record
  accepted deviations and the final hyperparameters as D-entries in docs/decisions.md.

DONE WHEN (proposed in PLAN.md): no collapse (effective rank > 50 % of latent dim, detached probe
exact match > 95 % on held-out states); the planner beats greedy in solve rate on the default
configuration's test set; the full evaluation matrix is produced by one command and its report
committed. Then push the branch, open a PR to main and report back.
```
