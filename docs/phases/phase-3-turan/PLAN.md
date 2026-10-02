# Phase 3 — `turan_water_sort` + distributed layout

## Goal

1. A second generator that produces a **different kind of puzzle** from uniform at minimal code cost (D1). It is time-seeded and reproducible, and built around a strategy enum so that more generation types are cheap to add later.
2. A second **layout** in **both** generators (D14): besides the standard layout (full tubes + whole empty tubes), a *distributed* layout where the free space is spread over the tubes, so a puzzle can start with half-empty tubes and no fully empty one.

The phase ends with a side-by-side comparison of the two generators, per layout.

## Layouts (D14)

Free space is always a whole number of tubes: there are `n_colors × capacity` units in `n_tubes × capacity` slots, so `n_empty × capacity` slots are free. `n_empty` (1 or 2, D3) is therefore "how many tubes' worth of free space", not necessarily the number of empty tubes.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Layout {
    /// The first `n_colors` tubes full, the last `n_empty` tubes empty (the classic game).
    #[default]
    Standard,
    /// Any fill heights `h_i ∈ [0, capacity]` with `Σ h_i = n_colors × capacity`, units packed
    /// from the bottom. Includes the standard layout's height vectors as special cases.
    Distributed,
}
```

`Layout` lives in `water_sort_core` (it describes states, not one generator). Both generators take it as a field: `Uniform { layout }`, `Turan { strategy, layout }`. `variant()` includes it, e.g. `fisher_yates(layout=distributed)`, `scramble(steps=40,layout=distributed)`. `Layout::Standard` must reproduce today's output **bit for bit**: the Phase 2 golden vectors must still pass unchanged, so `Uniform::VERSION` stays 1.

Core additions:
- `State::from_heights(params, heights, units)`: fills each tube bottom to top with `heights[i]` units taken in order.
- `Layout::is_valid(state)` / `State::layout_matches`: for tests.
- Exhaustive `distributed_fills(params)` next to `standard_fills`, for the uniformity test.
- `sample_heights(rng, params) -> Vec<u8>`: exactly uniform over all valid height vectors. Use a DP over `(tube index, units left)` counting completions (counts fit in `u64` for `MAX_TUBES = 16`, `MAX_CAP = 8`), then sample tube by tube with a `bounded_u64` built like `bounded_u32` (Lemire on `next_u64`, no `usize`, D10). Not rejection on random heights, because that acceptance rate is tiny for large configurations.

Why the distributed uniform is still *labeled-uniform* (D2): for every height vector, the number of distinct unit arrangements is the same multinomial `(n_colors·capacity)! / capacity!^n_colors`. So "uniform height vector, then Fisher-Yates over the units" is uniform over all labeled distributed states.

Puzzle codes already encode every cell (EMPTY included), so they cover distributed states with no format change. Test the round trip anyway.

## What stays the same across generators

Same `water_sort_core` rules, solver, `GenConfig`, filters (`min_opt`, not already solved), canonical hash, star metric, and the same RNG type (`ChaCha20Rng::seed_from_u64`). The shared rejection loop is `core::attempt_loop` (D13); do not re-implement it.

| | uniform | turan |
|---|---|---|
| fresh seed | 64 bits from `getrandom` | `splitmix64(now_nanos ^ splitmix64(counter))` |
| construction | Fisher-Yates fill (+ uniform heights for `Distributed`) | strategy: `Scramble` (default) or `Constrained` |
| layout | `Standard` (default) or `Distributed` | `Standard` (default) or `Distributed` |
| reproducible from | `(seed, params, cfg, layout)` | `(seed, params, cfg, strategy, layout)` |

## Design

```rust
pub enum TuranStrategy {
    /// Random walk of reverse pours from a solved state.
    Scramble { steps: u32, max_extra_steps: u32 },
    /// Fisher-Yates fill, rejecting any tube with two vertically adjacent same-color units.
    Constrained,
}

pub struct Turan { pub strategy: TuranStrategy, pub layout: Layout }

impl Generator for Turan {
    const ID: &'static str = "turan";
    const VERSION: u32 = 1;
    fn variant(&self) -> String;            // "scramble(steps=40,layout=standard)", "constrained(layout=distributed)"
    fn fresh_seed(&self, now_nanos: u64) -> Result<u64, GenError>;   // never fails for turan
    fn generate_observed<O: Observer>(&self, params: &Params, seed: u64, cfg: &GenConfig, observer: &mut O) -> Result<GeneratedPuzzle, GenError>;
    // generate, generate_traced, assemble are provided by the trait (D13); the loop is core::attempt_loop
}
```

### Seed

- `fresh_seed(now_nanos)` uses `water_sort_core::seed::time_seed(now_nanos, counter)` with a process-wide `static COUNTER: AtomicU64`.
- The caller supplies `now_nanos`: `SystemTime::now()` natively and `Date.now() * 1e6` on the web. Core never reads the clock (D1).
- An explicitly supplied seed always wins, for replay and datasets.

### Strategy `Scramble` (default)

Uses `water_sort_core::reverse_moves` / `unapply` (Phase 1.2b). Algorithm:

1. Start from `State::solved(params)`.
2. Apply `steps` random reverse moves, each chosen with `bounded_u32` over the list from `reverse_moves(state)` (fixed order). Exclude the move that exactly undoes the previous one.
3. Layout step:
   - **`Standard`:** keep applying random reverse moves until the state has exactly `n_colors` full tubes and `n_empty` empty tubes, in any positions, up to `max_extra_steps`; otherwise the attempt counts as rejected. Then permute tubes so the empty ones are last.
   - **`Distributed`:** no extra step. The walk already ends in an arbitrary height vector.
4. Shuffle the color labels (Fisher-Yates over `0..n_colors`) and the tube order (the full tubes only for `Standard`, all tubes for `Distributed`). This is a symmetry of the game, not a move, and it keeps labels symmetric (D2).
5. Validate with `evaluate`: reject if solved, if the solver times out (state-count limit only, D11), or if `opt_moves < min_opt`. `Unsolvable` cannot happen (reached by reverse moves from solved); `debug_assert` it.
6. Rejection continues the same RNG stream; `attempts` counts every try.

How a failed return-to-layout attempt is reported inside `attempt_loop` (the closure must return a `State`) is an implementation choice. Pick the cleanest one that keeps attempts counted and record it in D15.

Difficulty knob: `steps`. Phase 3 measures `steps → opt_moves` per layout to choose defaults.

### Strategy `Constrained` (optional, second)

The uniform construction for the chosen layout, plus one rejection rule: no tube contains two vertically adjacent units of the same color.

## Measurements and comparison

- `stats` gains `--layout standard|distributed` (default `standard`) for both generators, and `--generator turan --strategy scramble --steps N`. Criterion and report format unchanged (D3).
- **D3 for `Distributed`:** the supported table was measured for `Standard`. Run `stats --generator uniform --layout distributed --empty 1..=2` over the same grid. Commit `reports/uniform_distributed_stats.{csv,md}`, and propose the distributed limits as an addition to D3 (status proposed, user decides). `is_supported` gains a layout argument or a second table. Choose the API and record it.
- `water_sort_cli compare --a <gen spec> --b <gen spec> ...` writes a report, e.g. `reports/uniform_vs_turan_standard.md` and `reports/uniform_vs_turan_distributed.md`:
  - the Phase 2.3 measurement table for both, side by side;
  - histograms: `opt_moves`, color changes, segments, random-policy stuck/capped rates, `states_expanded`;
  - two-sample tests (chi-square on histograms, KS on continuous metrics) to document the size of the difference;
  - overlap: the fraction of canonical hashes shared between equal-size samples.
- A `steps` sweep (e.g. 10, 20, 40, 80, 160) per layout: mean/p50/p99 `opt_moves`, rejection rate, and (standard) the extra-step distribution.
- Also compare `uniform standard` vs `uniform distributed`, to measure what the layout alone changes.

## Tasks

1. [ ] Core: `Layout`, `State::from_heights`, `bounded_u64`, `sample_heights` (DP, exact), `distributed_fills`, layout checks + tests
2. [ ] Uniform: `layout` field, `Distributed` construction; Phase 2 golden vectors unchanged; new golden vectors for `Distributed`
3. [ ] Uniformity chi-square for `Distributed` on small configs (2×2×1, 3×3×1) via `distributed_fills`, with a failing negative control: heights drawn tube by tube, each uniform over its feasible range given the units left. That is biased. Do not use "independent heights, reject if the sum is wrong" as the control: it is exactly uniform.
4. [ ] `TuranStrategy`, `Turan { strategy, layout }`, `fresh_seed`
5. [ ] `Scramble` construction for both layouts + label/tube shuffle
6. [ ] Golden test: 20 fixed seeds × 3 configs × each (strategy, layout)
7. [ ] `stats`: `--layout`, `--generator turan --strategy ...`
8. [ ] Distributed measurement grid for uniform; propose distributed limits (D3 addendum)
9. [ ] `steps` sweep per layout; choose default `steps` per supported configuration (D15, proposed)
10. [ ] `compare` subcommand + reports
11. [ ] (optional) `Constrained` strategy

## Tests

- No game logic outside `water_sort_core`. Generators only use core's public API; checked in review.
- `Layout::Standard` output is bit-identical to Phase 2 (existing golden vectors pass unchanged).
- Same `(seed, params, cfg, strategy, layout)` → identical `GeneratedPuzzle`, on Linux and Windows CI.
- `sample_heights` is exactly uniform: chi-square over all height vectors on small params, plus a DP-count check against brute-force enumeration.
- Two `fresh_seed` calls in a tight loop never return the same value, even at the same `now_nanos`.
- 10k generated puzzles per (generator, layout): the solution replays to solved in `opt_moves`, every state matches its layout, and Turan never yields `Unsolvable`.
- Puzzle-code round trip for distributed states.
- `Constrained`: no generated tube contains adjacent same-color units.

## Acceptance

Roadmap conditions: implements `Generator`, no rules outside core, the same solver and `opt_moves`, deterministic for a given seed, Phase 2.3 measurements side by side with uniform, distribution difference measured (`opt_moves` histogram, color-change histogram, random-policy failure rate). Plus: both layouts in both generators, distributed uniformity test passing with a failing control, comparison reports and proposed `steps` defaults and distributed limits committed.

## Risks / open points

- Returning to the standard layout (Scramble step 3) may need many extra steps for some configurations. Measure the extra-step distribution. If it is too expensive, fall back to a final "compact" pass, or make `Distributed` the Scramble default and report it.
- The distributed layout may change solver cost (no fully empty tube at the start, more partially open tubes). The distributed grid measures this; the supported limits may differ from the standard ones.
- A random walk mixes slowly. Small `steps` gives puzzles close to solved, which `min_opt` rejects. The sweep picks `steps` large enough to keep the rejection rate low.
