# Phase 2 — start prompt

**Status: done (2026-10-02), merged in PR #2 and the review follow-up PR #3.** The prompt below is kept as a record of how the phase was briefed. Do not run it again. Implementation refinements are in D13 and the supported range in D3 (still **proposed**, awaiting the user) in [`docs/decisions.md`](../../decisions.md).

---

```text
You are implementing Phase 2 (`uniform_water_sort` + `water_sort_cli stats`) of jepa_games.

Repo: github.com/vinnipukh/jepa_games (private). Start from main, branch phase-2-uniform.
Phase 1 (water_sort_core) is merged: Generator trait (seed: u64, variant, fresh_seed, generate,
assemble), evaluate(), GenConfig, sampling, seed, solver, canon, metrics, puzzle codes.

READ FIRST: roadmap.md (Phase 2), docs/README.md, docs/decisions.md (D1-D12),
docs/phases/phase-2-uniform/PLAN.md.

KEY REQUIREMENTS: Uniform generator (ChaCha20, core fisher_yates, standard layout, evaluate,
continue the same stream on rejection); a traced variant returning rejection counts so the CLI
never duplicates the loop; golden vectors (20 seeds x 3 configs, 64-bit values as hex);
chi-square uniformity over the enumerated accepted set for 2x2x1 and 3x3x1 with a fixed seed
list and a negative control that must fail; exact symmetry check for D2; `water_sort_cli stats`
grid with per-reason rejection rates, attempts, opt_moves, solver timing, states, symmetric
fraction; memory-aware threading and early stop; PROPOSE D3 with a SUPPORTED table in core.

DONE WHEN: uniformity passes (control fails), solutions replay, same seed -> same puzzle, stats
report committed, D3 proposed with evidence, CI green; then open a PR.
```
