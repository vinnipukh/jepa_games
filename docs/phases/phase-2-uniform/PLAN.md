# Phase 2 — `uniform_water_sort`

## Goal

A seeded, reproducible generator that samples uniformly from solvable, unsolved labeled configurations with `opt_moves ≥ min_opt` (D2). Plus the measurements that fix the supported configuration range (D3).

## 2.1 Generation

```rust
pub struct Uniform;
impl Generator for Uniform {
    const ID: &'static str = "uniform";
    const VERSION: u32 = 1;
    fn variant(&self) -> String { "fisher_yates".into() }
    fn fresh_seed(&self, _now_nanos: u64) -> u64;   // 64 bits from getrandom
    fn generate(&self, p: &Params, seed: u64, cfg: &GenConfig) -> Result<GeneratedPuzzle, GenError>;
}
```

Algorithm:

1. Seed is externally supplied, or 64 bits from `getrandom` when absent. The seed is always recorded.
2. `rng = ChaCha20Rng::seed_from_u64(seed)`.
3. Loop `attempt = 1..=max_attempts`:
   1. `units = [0,0,0,0, 1,1,1,1, …]` (`n_colors × capacity`).
   2. `core::sampling::fisher_yates(&mut rng, &mut units)` (D10).
   3. Fill tubes `0..n_colors` bottom to top, in order; tubes `n_colors..` stay empty.
   4. Reject if solved. Otherwise solve with state-count limits only (D11).
   5. Reject if `Unsolvable`, `Timeout`, or `opt_moves < min_opt`. Otherwise accept.
   6. Rejection continues the **same** RNG stream, so `(seed, params, cfg, VERSION)` fully determines the result.
4. Fill `GeneratedPuzzle`: `seed`, `generator_variant`, `attempts`, metrics, hash, puzzle code.
5. `max_attempts` exhausted → `GenError::TooManyAttempts`.

Reproducing a puzzle means calling `generate(params, seed, cfg)` again. This re-runs the solver, because rejection decisions are part of the stream, so it must use the same `GenConfig`. Decoding the stored puzzle code is the cheap path.

Note: the **solver limits are part of the generator's identity**. Changing `max_states` can turn a previously rejected `Timeout` into an acceptance and shift every later puzzle. `GenConfig` is therefore stored in every record (Phase 4), and the default config is versioned along with `VERSION`.

## 2.2 Uniformity validation

What is tested: the empirical distribution over accepted labeled states equals the uniform distribution over the set `A` = {labeled states from the fixed layout that are unsolved, solvable, and have `opt ≥ min_opt`}.

Configurations:

| Config | Raw arrangements | Purpose |
|---|---|---|
| 2 colors, cap 2, 1 empty | 6 | smoke test, sanity of enumeration |
| 3 colors, cap 3, 1 empty | 9!/(3!³) = 1,680 | real test with statistical power |

Procedure:

1. Enumerate all multiset permutations, fill them, and filter to `A` with BFS.
2. Draw `N = 50 × |A|` samples from the generator using a **fixed list of seeds**. The test is deterministic, so it never flakes in CI.
3. Pearson chi-square, `df = |A| − 1`. Pass if `p > 0.001`.
4. Negative control: run the same test on a deliberately biased shuffle (an off-by-one Fisher-Yates) and assert that it **fails**. This shows the test actually has power.

The 3-color test is `#[ignore]` and runs in the release-mode heavy-tests job.

Also measured here, for D2: the fraction of samples with `|Aut| > 1` for each configuration.

## 2.3 Measurements — `water_sort_cli stats`

```
water_sort_cli stats --generator uniform \
    --colors 2..=12 --capacity 3..=5 --empty 1..=3 \
    --samples 1000 --max-states 5e6 --out reports/uniform_stats.{csv,md}
```

Per `(n_colors, capacity, n_empty)` cell:

- rejection rate, split into unsolvable / timeout / below `min_opt` / already solved;
- attempts: mean, p50, p99;
- `opt_moves`: histogram, mean, p50, p99;
- solver: wall time p50/p99, `states_expanded` p50/p99, peak memory estimate;
- `|Aut| > 1` fraction (D2).

Parallelized with rayon over samples (seeds `base_seed + i`). Results are deterministic apart from the timing columns.

The supported range (D3) is the set of cells meeting the criterion (proposed: solver p99 < 1 s per attempt and per accepted puzzle, whole-generation p99 < 1 s, timeout rate < 0.1 %, attempts p99 ≤ `max_attempts` / 10). It goes into `docs/decisions.md` and into a `SUPPORTED` table in `water_sort_core`, which the CLI, Python, and web check against.

## Tasks

1. [x] `Uniform` generator (`Generator` impl, `fresh_seed` via `getrandom`)
2. [x] Golden test: 20 fixed seeds × 3 configs → expected puzzle codes (committed)
3. [x] Enumeration helper (`water_sort_core::standard_fills`, public so the uniform crate's tests can use it)
4. [x] Chi-square tests + negative control
5. [x] `water_sort_cli` skeleton (clap) + `stats` subcommand
6. [x] Run the measurement grid, write the report, decide D3 (and D2 follow-up) — report committed; D3 **proposed** (awaiting user decision), D2 follow-up recorded in D3

## Tests

- Same seed → identical `GeneratedPuzzle`, compared field by field, on Linux and Windows CI.
- For 10k random seeds: replaying `solution` on `state` reaches solved in exactly `opt_moves` moves.
- Uniformity chi-square with its negative control.

## Acceptance

Roadmap gate: uniformity test passes; every generated puzzle's solution replays to solved; same seed → same puzzle. Plus: the stats report is committed and D3 is decided.

## Risks / open points

- At large `n_colors` with `n_empty = 1`, many random fills may be unsolvable, so rejection rates could be very high. The measurement shows this; such cells are simply left out of the supported range.
