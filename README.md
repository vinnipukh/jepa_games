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
- **Phase 7** (next): the JEPA itself.

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
