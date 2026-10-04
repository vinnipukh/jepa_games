# jepa_games

A Water Sort puzzle game with two puzzle generators, plus the infrastructure to train a JEPA
world model on it.

**Play:** https://vinnipukh.github.io/jepa_games/

- **`water_sort_core`** (Rust): the game rules, an optimal solver, star rating, canonical
  hashing and the shared generator machinery. Every other part calls into it, so the rules
  exist exactly once.
- **Generators:** `uniform_water_sort` (uniform over valid fills) and `turan_water_sort`
  (strategy-based: reverse search, scramble, pour walk, constrained). Every puzzle is exactly
  reproducible from its seed on every target.
- **`water_sort_cli`:** datasets (Parquet / JSONL with dedup, splits and manifests), statistics
  and generator comparisons.
- **`python/`** (`jepa_water_sort`): PyO3 binding, Gymnasium environment, vector environment
  and trajectory logger.
- **`web/`:** the browser game, the same Rust code compiled to WebAssembly with a small
  TypeScript front end. A seed or puzzle code opens the same puzzle there as in Python.
- **`python/jepa`** (Phase 7): the JEPA world model, latent planners (beam, MCTS, CEM), baselines
  (random, greedy, solver, DDQN) and the evaluation pipeline.

## Results (JEPA, `default` preset)

6 colors × capacity 4 × 2 empty tubes, uniform generator, standard layout; one training seed,
500 test puzzles, move limit 4 × optimal. Trained on an AMD RX 7900 XT (ROCm torch on Windows):
JEPA ≈ 40 min, DDQN ≈ 9 min. Full report: [`reports/jepa_eval_default.md`](reports/jepa_eval_default.md).

| policy | solved (uniform test) | stars | solved (Turan test, zero-shot) | stars |
|---|---|---|---|---|
| solver (optimal) | 100.0 % | 5.00 | 100.0 % | 5.00 |
| **JEPA + MCTS** | 97.2 % | 4.77 | – | – |
| **JEPA + beam** | 97.0 % | 4.76 | 99.4 % | 4.87 |
| JEPA + CEM | 96.4 % | 4.76 | – | – |
| DDQN | 96.6 % | 4.40 | 98.8 % | 4.64 |
| greedy | 77.0 % | 3.22 | 86.6 % | 3.74 |
| random | 72.6 % | 0.92 | 85.4 % | 1.08 |

- The world model is not collapsed: the detached probe decodes states exactly (100 %), and the
  1/2/3-step latent rollouts are 98.7 / 97.8 / 96.2 % exact.
- Acceptance: probe exact match > 95 % **met**, planner > greedy **met**, effective rank > 50 % of
  the latent dim **not met** (19.5 %; the probes are perfect, so the threshold is probably too high
  for this puzzle size).
- Beam, MCTS and CEM differ by less than the noise (about ±1.6 points at 500 puzzles); search
  depth and revisit checks change nothing, so the value head carries the planning signal. Scoring
  by the solved flag alone drops to 56 %.
- The Turan test set (6 × 4 × 2, standard, 500 puzzles) is *easier* than the uniform one (random
  and greedy both score higher), so the numbers are not a harder-distribution result. Models were
  trained on uniform only. Not done yet: multiple seeds, other generators and layouts (the `full`
  preset).

## Quick start

```bash
scripts/setup-cloud.sh --python --web        # Rust toolchain, Python package, wasm tooling
cargo test --workspace --locked              # Rust tests
(cd python && uv run --no-project python -m pytest)
web/build.sh && (cd web/app && npm run dev)  # play in the browser
```

More: [`CLAUDE.md`](CLAUDE.md) (commands and rules), [`roadmap.md`](roadmap.md),
[`docs/README.md`](docs/README.md) (phase plans and status), [`docs/decisions.md`](docs/decisions.md)
(decision log), [`web/README.md`](web/README.md), [`python/README.md`](python/README.md).

## License

[MIT](LICENSE).
