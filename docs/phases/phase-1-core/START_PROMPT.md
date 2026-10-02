# Phase 1 — start prompt

**Status: done (2026-10-02), merged in PR #1.** The prompt below is kept as a record of how the phase was briefed. Do not run it again. Implementation refinements are recorded in D12 in [`docs/decisions.md`](../../decisions.md).

---

```text
You are implementing Phase 1 (`water_sort_core`) of the jepa_games project.

Repo: github.com/vinnipukh/jepa_games (private), branch main. Phase 0 is done: Cargo workspace
with 4 crates, toolchain 1.99.0, workspace lints (unsafe_code = forbid, clippy pedantic warn),
GitHub Actions CI on ubuntu + windows (fmt, clippy -D warnings, test).

READ FIRST: roadmap.md (Phase 1), docs/README.md, docs/decisions.md (overrides the roadmap),
docs/phases/phase-1-core/PLAN.md.

SCOPE: only water_sort_core (plus workspace Cargo.toml / CI edits it needs).

Key decisions: fixed-array Copy State; exact canonical_full (D8) + approximate solver_key;
xxh3 hash; A* with h = segments - n_colors (D9) and deterministic tie-breaking; state-count
limits only for generation (D11); integer star thresholds (D5) and a Session type; own
bounded_u32 + fisher_yates (D10); seed.rs with pure splitmix64 / time_seed; reverse moves
(PLAN 1.2b) with a round-trip proptest; Generator trait per D7; rand_core only.

WORK METHOD: branch phase-1-core, one commit per task, fmt/clippy/test green after each, add a
release-mode `heavy-tests` CI job for #[ignore] tests, golden vectors in
water_sort_core/tests/golden/, record deviations in docs/decisions.md, stop and report with a
counterexample if the plan is wrong.

DONE WHEN: unit tests + proptests (conservation, solution replay, canonical invariance,
canonical == brute force for n_colors <= 6, heuristic admissibility, reverse round trip, puzzle
code round trip), A* == BFS on 10k puzzles, golden vectors committed, CI green; then open a PR.
```
