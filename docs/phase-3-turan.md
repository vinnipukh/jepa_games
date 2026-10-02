# Phase 3 — `turan_water_sort`

## Goal

A second generator that produces a **different kind of puzzle** from uniform at minimal code cost (D1). It is time-seeded and reproducible, and built around a strategy enum so that more generation types are cheap to add later. The phase ends with a side-by-side comparison with uniform.

## What stays the same as uniform

Same `water_sort_core` rules, solver, `GenConfig`, filters (`min_opt`, not already solved), canonical hash, star metric, and the same RNG type (`ChaCha20Rng::seed_from_u64`). What differs is the **seed source** and the **construction step**:

| | uniform | turan |
|---|---|---|
| fresh seed | 64 bits from `getrandom` | `splitmix64(now_nanos ^ splitmix64(counter))` |
| construction | Fisher-Yates fill | strategy: `Scramble` (default) or `Constrained` |
| reproducible from | `(seed, params, cfg)` | `(seed, params, cfg, strategy)` |

## Design

```rust
pub enum TuranStrategy {
    /// Random walk of reverse pours from a solved state.
    Scramble { steps: u32, max_extra_steps: u32 },
    /// Fisher-Yates fill, rejecting any tube with two vertically adjacent same-color units.
    Constrained,
}

pub struct Turan { pub strategy: TuranStrategy }

impl Generator for Turan {
    const ID: &'static str = "turan";
    const VERSION: u32 = 1;
    fn variant(&self) -> String;            // "scramble(steps=40)", "constrained"
    fn fresh_seed(&self, now_nanos: u64) -> u64;
    fn generate(&self, params: &Params, seed: u64, cfg: &GenConfig) -> Result<GeneratedPuzzle, GenError>;
}
```

### Seed

- `fresh_seed(now_nanos)` uses `water_sort_core::seed::time_seed(now_nanos, counter)` with a process-wide `static COUNTER: AtomicU64`.
- The caller supplies `now_nanos`: `SystemTime::now()` natively and `Date.now() * 1e6` on the web. Core never reads the clock (D1).
- An explicitly supplied seed always wins, for replay and datasets.

### Strategy `Scramble` (default)

Uses `water_sort_core::moves::reverse_moves` / `unapply` (Phase 1.2b). Algorithm:

1. Start from the solved standard layout: tube `i` full of color `i` for `i < n_colors`, and the last `n_empty` tubes empty.
2. Apply `steps` random reverse moves, each chosen with `bounded_u32` over the list from `reverse_moves(state)` (fixed order). Exclude the move that exactly undoes the previous one, to avoid wasted back-and-forth.
3. **Return to the standard layout.** Keep applying random reverse moves until the state has exactly `n_colors` full tubes and `n_empty` empty tubes, in any positions. Up to `max_extra_steps`; otherwise the attempt counts as rejected.
   - Reason: uniform always produces this layout. If Turan matches it, both generators cover the same set of puzzles and differ only in how likely each one is, which keeps the cross-evaluation clean.
   - Empty tubes can end up in any position during the walk. They are moved to the back by a tube permutation (step 4), which is a symmetry of the game, so it does not count as a move.
4. Shuffle the color labels (Fisher-Yates over `0..n_colors`) and the order of the full tubes. This removes any trace of the fixed starting assignment, so labels stay symmetric, as in the labeled space (D2).
5. Validate as uniform does: reject if solved, if the solver times out (state-count limit only, D11), or if `opt_moves < min_opt`. `Unsolvable` cannot happen, because the state was reached by reverse moves from a solved state, and the generator asserts this as a debug invariant.
6. Rejection continues the same RNG stream; `attempts` counts every try.

Difficulty knob: `steps`. `opt_moves ≤` the number of forward moves needed to undo the walk, but it is usually much smaller. Phase 3 measures `steps → opt_moves` to choose defaults.

### Strategy `Constrained` (optional, second)

The uniform construction, plus one rejection rule: no tube contains two vertically adjacent units of the same color. About 10 lines on top of the shared `fisher_yates`. A mild variation, kept because it is almost free.

## Comparison with uniform

`water_sort_cli compare --a uniform --b turan[:scramble(steps=N)] ...` writes `reports/uniform_vs_turan.md`:

- the Phase 2.3 measurement table for both, side by side;
- histograms: `opt_moves`, color changes, segments, random-policy stuck/capped rates, `states_expanded`;
- two-sample tests (chi-square on histograms, KS on continuous metrics) to document the size of the difference;
- a `steps` sweep (e.g. 10, 20, 40, 80, 160): mean/p50/p99 `opt_moves` and rejection rate for each value;
- overlap: the fraction of Turan canonical hashes that also occur in an equal-size uniform sample (expected to be small for supported configurations).

## Tasks

1. [ ] `TuranStrategy`, `Turan`, `fresh_seed` (needs `core::seed`)
2. [ ] `Scramble` construction + standard-layout return + label/tube shuffle
3. [ ] Validation loop, `GeneratedPuzzle` fill, `variant()`
4. [ ] Golden test: 20 fixed seeds × 3 configs → expected puzzle codes
5. [ ] `stats` support for `--generator turan --strategy ...`
6. [ ] `steps` sweep; choose default `steps` per supported configuration
7. [ ] `compare` subcommand + report
8. [ ] (optional) `Constrained` strategy

## Tests

- No game logic outside `water_sort_core`. The crate only uses core's public API, including `reverse_moves` / `unapply`; checked in review.
- Same `(seed, params, cfg, strategy)` → identical `GeneratedPuzzle`, on Linux and Windows CI.
- Two `fresh_seed` calls in a tight loop never return the same value, even at the same `now_nanos` (counter test).
- 10k generated puzzles: the solution replays to solved in `opt_moves`, every state is in standard layout, and none are `Unsolvable`.
- `Constrained`: no generated tube contains adjacent same-color units.

## Acceptance

Roadmap conditions: implements `Generator`, no rules outside core, the same solver and `opt_moves`, deterministic for a given seed, Phase 2.3 measurements side by side with uniform, distribution difference measured (`opt_moves` histogram, color-change histogram, random-policy failure rate). Plus: comparison report and chosen `steps` defaults committed.

## Risks / open points

- Returning to the standard layout (step 3) may need many extra steps for some configurations. Measure the extra-step distribution. If it is too expensive, alternatives are (a) a final "compact" pass that pours partial tubes forward and then re-scrambles, or (b) allowing non-standard layouts as a separate strategy.
- A random walk mixes slowly. Small `steps` gives puzzles close to solved, which the `min_opt` filter rejects. The sweep picks `steps` large enough to keep the rejection rate low.
