# Decision log

Each entry has a status: **decided**, **proposed** (default the plans assume until confirmed), or **pending** (blocks something).

---

## D1 — Turan generator = time-seeded, strategy-based construction — decided (2026-10-02)

Goal: variation in puzzle *generation types* at minimal code cost. It is not a randomness-source experiment.

History: first decided as "same algorithm as uniform, true random source". That was dropped because a different randomness source produces the same puzzle distribution, so it gives no variation. It also forced a seedless "entropy tape" provenance design, now removed.

The current design:

- **Seed: time-based.** `seed = splitmix64(now_unix_nanos ^ splitmix64(counter))`, where `counter` is a process-wide `AtomicU64` incremented once per puzzle. Without the counter, puzzles generated in parallel within the same clock tick would get identical seeds. The seed is recorded like uniform's, so every Turan puzzle is reproducible from `(seed, params, config)`. A time seed is *not* true randomness. It is a convenient seed choice for a normal PRNG.
- **RNG: `ChaCha20Rng::seed_from_u64(seed)`**, the same as uniform.
- **Construction: a strategy enum**, so new generation types cost one enum arm and touch nothing outside `turan_water_sort`:
  - `Scramble { steps }` (first, default): start from a solved state and apply random *reverse* pours. It is solvable by construction and gives strongly different statistics from uniform (longer same-color runs, a different difficulty profile).
  - `Constrained` (second, optional): uniform Fisher-Yates plus rejection of any fill with two vertically adjacent same-color units.
- **Consequence for evaluation:** the two generators now produce different distributions over the *same* set of puzzles (both use the standard layout, see Phase 3). The Phase 7 cross-evaluation (train on one, test on the other) is a real generalization test.

The time source is passed into core as a number (`now_nanos: u64`). Core never calls the clock, so it stays pure and WASM-safe: on `wasm32-unknown-unknown`, `std::time::SystemTime::now()` panics, so the web build supplies `Date.now()`.

## D2 — Uniform over which space? — decided (2026-10-02): labeled configurations

"Labeled" refers to the sample space: tube positions and color ids are treated as distinguishable. It involves no manual annotation; Fisher-Yates produces it directly.

Two candidate target distributions:

- **Labeled-uniform.** Every concrete arrangement (which tube holds what, which actual color is where) is equally likely. Plain Fisher-Yates gives this with no extra work.
- **Canonical-uniform.** Every *structurally distinct* puzzle (up to tube reordering and color renaming) is equally likely.

The two differ only through class sizes. A canonical class contains `(n_tubes! · n_colors!) / |Aut|` labeled arrangements (counting only the arrangements that fit the fixed layout with empty tubes last), where `|Aut|` is the number of symmetries that leave the puzzle unchanged, for example two identical tubes. Labeled-uniform therefore under-samples highly symmetric puzzles by a factor of `|Aut|`. For more than about 4 colors almost every puzzle has `|Aut| = 1`, so the two distributions are practically identical. The gap matters only for tiny configurations.

Decision: labeled-uniform as the target, plus a measurement in Phase 2.3 of the fraction of generated puzzles with `|Aut| > 1` for each configuration. If that fraction is non-negligible in the supported range, add an optional `--canonical-uniform` mode that accepts a sample with probability `1/|Aut|`.

## D3 — Supported configuration range — decided (2026-10-02): 1–2 empty tubes, table below

Set from measurements. Criterion: solver p99 < 1 s, time to generate one puzzle p99 < 1 s, state-limit hit rate < 0.1 %, and enough attempt headroom, all in release mode.

Evidence: [`reports/uniform_stats.md`](../reports/uniform_stats.md) (`water_sort_cli stats`, uniform generator, 1000 puzzles per cell, `max_states` 5e6, `max_attempts` 10 000, `min_opt` 1, base seed 20261002). Measured on a 14-core Windows 11 desktop with 8 worker threads, **not** on the CI runner; timings under parallel load are pessimistic compared with a single interactive solve. A cell counts as supported when all of these hold:

- every sample generated within `max_attempts` (no `TooManyAttempts`);
- solver p99 < 1 s, both per attempt (all outcomes) and over the accepted puzzles;
- p99 time to generate one puzzle (all attempts plus metrics) < 1 s;
- timeout rate < 0.1 % of attempts;
- attempts p99 ≤ `max_attempts` / 10 (≤ 1000). The tail of a 1M-puzzle batch (Phase 4) must not reach `TooManyAttempts`.

The last two conditions were added at PR #2 review. The first version checked only per-attempt solver time, which is blind to `n_empty = 1`. There, each attempt is cheap, but a puzzle can take thousands of attempts. For example, 9 × 5 × 1 had a whole-generation p99 of 1.69 s, and 11 × 4 × 1 had an attempts p99 of 3503 out of 10 000. The report was re-scored from the same measurements; no re-run was needed.

Supported range (`n_colors` from 2 up to):

| capacity \ `n_empty` | 1 | 2 | 3 (measured, excluded) |
|---|---|---|---|
| 3 | 12 | 12 | 10 |
| 4 | 9 | 11 | 8 |
| 5 | 7 | 9 | 6 |

In every row the supported cells are a prefix of `n_colors`. The table is `water_sort_core::SUPPORTED` (`is_supported(&params)`), and a CLI test checks it against the committed report cell by cell.

What limits each row:

- **`n_empty = 1`: rejection, not solving.** Unsolvable fills dominate: 99.8 % of attempts at 12 colors / capacity 4, and over 99.98 % at 10 colors / capacity 5. Each attempt is cheap (p99 ≈ 2 ms), but the number of attempts per puzzle explodes. The attempts-headroom condition bounds this row: 10 × 4 × 1 needs 1366 attempts at p99, and 8 × 5 × 1 needs 1180. Beyond those, samples start to exceed the 10 000-attempt limit outright (12 × 4 × 1, 10 × 5 × 1).
- **`n_empty = 2, 3`: solver time.** Rejections are essentially zero, but A* cost grows quickly with colors, and faster with more empty tubes (more branching). Past the bound, p99 goes over 1 s first, then timeouts appear (≈ 0.2 % at 12 × 3 × 3 and 10 × 4 × 3, 3.3 % at 11 × 4 × 3, 11 % at 10 × 5 × 3).
- **Borderline:** 12 × 4 × 2 measured p99 = 1003 ms, so it is just outside. On an unloaded machine it would probably pass. Raising it is a judgment call for the user.
- **A third empty tube buys little.** With the same fills, the `opt_moves` histograms for `n_empty` 2 and 3 are nearly identical, while solver cost is 10–40× higher. `n_empty = 3` configurations are unlikely to be worth using for datasets.

**Decided (2026-10-02, user):** only `n_empty` 1 and 2 are supported (`MAX_SUPPORTED_EMPTY = 2`), with the limits above. `n_empty = 0` has no legal move at all. `n_empty = 3` is excluded by policy even where it passed the criterion: it barely changes `opt_moves` but costs 10–40× more solver time, and 75 % of the Phase 2.3 measurement time. The primary configuration is `n_empty = 2` (the classic game); `n_empty = 1` is an optional harder variant. Sweeps and comparisons use `--empty 1..=2` (the `stats` default). The 12 × 4 × 2 borderline cell stays out.

D2 follow-up (same report): the fraction of generated puzzles with a nontrivial symmetry is large only for 2 colors (13–35 %) and 3 colors (0.5–10 %). From 4 colors on it is ≤ 9 % at capacity 3 and ≤ 1.6 % at capacity 4 and 5, and it falls toward 0 as colors increase. Recommendation: no `--canonical-uniform` mode for now. The labeled/canonical gap matters only for tiny configurations.

### D3 addendum — distributed layout range — decided (2026-10-02, user)

Evidence: [`reports/uniform_distributed_stats.md`](../reports/uniform_distributed_stats.md) (`stats --generator uniform --layout distributed`, same grid, criterion and seeds as above, 1000 puzzles per cell; 15 min wall time). It ran on the cloud session machine (4 cores, 3 worker threads), **not** on the desktop that measured the standard table. Calibration on the same machine ([`reports/uniform_standard_calibration_cloud.csv`](../reports/uniform_standard_calibration_cloud.csv), standard layout, same seeds): 12 × 4 × 2 solver p99 783 ms vs 1003 ms on the desktop, so cloud timings are scaled by 1.28 before applying the 1 s limit.

`SUPPORTED_DISTRIBUTED` (`n_colors` from 2 up to), accepted as proposed:

| capacity \ `n_empty` | 1 | 2 |
|---|---|---|
| 3 | 12 | 12 |
| 4 | 9 | 11 |
| 5 | **8** (standard 7) | **8** (standard 9) |

- **`n_empty = 1`: still rejection-bound, but less.** Spreading the free space makes somewhat fewer fills unsolvable (8 × 5 × 1: 99.35 % vs 99.59 % standard, attempts p99 724 vs 1180), which buys one color at capacity 5. The attempts-headroom condition (machine-independent) sets the bound: 8 × 5 × 1 needs 724 attempts at p99, 9 × 5 × 1 needs 2477.
- **`n_empty = 2`: solver-bound, and costlier than standard.** On the same machine the distributed solver p99 is 1.2–3× the standard one (11 × 4 × 2: 536 vs 362 ms; 9 × 5 × 2: 829 vs 279 ms). Scaled: 12 × 4 × 2 → 1157 ms (out), 9 × 5 × 2 → 1061 ms (out), 11 × 4 × 2 → 686 ms, 8 × 5 × 2 → 414 ms. Unscaled, the cloud report marks 12 × 4 × 2 and 9 × 5 × 2 as passing; they are left out because the standard table was set on the slower machine.
- API: `SUPPORTED_DISTRIBUTED`, `supported_rows(layout)`, `is_supported_in(&params, layout)`. `is_supported(&params)` is unchanged and means the standard layout. A CLI test checks that every proposed distributed cell passes the criterion in the committed report.

## D4 — Move limit `k · opt_moves` — proposed: k = 4

Generous enough that a random-ish policy is not truncated before it has a chance. To be revisited after the RL baseline.

## D5 — Star coefficients — proposed: 0.10 / 0.25 / 0.50, stored as per-mille integers (100 / 250 / 500)

Integers are used because floating point gives wrong thresholds: `0.10 * 30.0 = 3.0000000000000004`, so `ceil` returns 4 instead of 3. Thresholds are computed with integer arithmetic: `ceil(c · opt / 1000) = (c · opt + 999) / 1000`.

## D6 — CI = GitHub Actions — decided (2026-10-02)

Matrix: `ubuntu-latest` + `windows-latest`. Separate jobs for wasm32 and Python as those phases arrive.

## D7 — `Generator` trait: roadmap shape + config, variant and fresh seed — decided (2026-10-02)

The roadmap's `seed: u64` shape stays, since both generators are seeded (D1). Additions:

```rust
pub trait Generator {
    const ID: &'static str;          // "uniform", "turan"
    const VERSION: u32;              // bump when the algorithm changes
    /// Human-readable sub-type, recorded with every puzzle, e.g. "fisher_yates", "scramble(steps=40)".
    fn variant(&self) -> String;
    /// A new seed when the caller supplies none: OS entropy for uniform, time + counter for turan.
    fn fresh_seed(&self, now_nanos: u64) -> u64;
    fn generate(&self, params: &Params, seed: u64, cfg: &GenConfig) -> Result<GeneratedPuzzle, GenError>;
}
```

- `GenConfig` (`min_opt`, `max_attempts`, `SolverLimits`) is part of the reproducibility key. Changing the solver limits can change which attempt is accepted (Phase 2).
- `GeneratedPuzzle` gains `generator_variant: String` and `puzzle_code: String`, and returns a `Result` rather than panicking.
- Every puzzle gets a **puzzle code**, a compact base32 encoding of params + initial state. It opens a puzzle without re-running the generator and is accepted by the web "open puzzle" box next to `(generator, seed)`.

## D8 — Canonical hash must be exact — proposed

The roadmap's "sort tubes, then relabel colors by first appearance" is **not** a canonical form. Relabeling changes the tube order, and two color-permuted copies of one puzzle can end up with different encodings. The roadmap's own proptest ("invariant under color permutation") would fail.

- `canonical_full` = lexicographic minimum of the sorted-tube encoding over **all** color relabelings, computed with backtracking and invariant pruning. It is exact, and is used for dataset dedup and splits.
- `solver_key` = sort, relabel, sort again. Fast but not exact. That is fine for the solver, because a missed merge only costs speed, never correctness.
- The hash function is a fixed, platform-independent function (xxh3-64 over the canonical byte encoding).

## D9 — Stronger admissible heuristic — proposed

`h = total_segments − n_colors`, where a segment is a maximal same-color run within a tube.

- Admissible: a pour changes the segment count by −1 (merge onto a matching top), 0, or +1 (partial pour onto empty), and a solved state has exactly `n_colors` segments.
- Dominates the roadmap's color-change count: `changes = segments − nonempty_tubes`, and `nonempty_tubes ≥ n_colors` always holds, because `n_colors · capacity` units need at least `n_colors` tubes.
- It also captures "one color split across several tubes", which the color-change count ignores.

## D10 — Own sampling primitives — proposed

`rand`'s `shuffle` / `gen_range` algorithms can change between crate versions and can depend on `usize` width (64-bit native vs 32-bit wasm32). The core crate implements its own `bounded_u32` (Lemire's method with rejection on `next_u32`) and Fisher-Yates. Both generators use these primitives, so their results are bit-identical on native, wasm32 and Python.

## D11 — Generation never uses wall-clock limits — proposed

Solver timeouts during generation are by **expanded-state count only**. A time limit would make accept/reject depend on machine speed and break reproducibility from a seed. Time limits remain available for interactive use (web, CLI `solve`).

## D12 — Phase 1 implementation refinements — decided (2026-10-02)

Recorded while implementing `water_sort_core`. None of these changes a decision above; they fix details the plans left open or stated loosely.

- **`canonical_full` algorithm.** D8's definition (minimum over all color relabelings of the sorted-tube encoding) is kept exactly. The plan's sketch, "colors with unique signatures are fixed by signature order", would give a valid canonical form but *not* that minimum, so it would fail the planned brute-force test. Instead: for a fixed relabeling, sorting minimizes the concatenation over tube orders, and for a fixed tube order, first-appearance labeling minimizes over relabelings, so the minimum equals the minimum over tube orders of the first-appearance labeled concatenation. That is built tube by tube, branching only over distinct tube contents that tie for the smallest next block. Highly symmetric puzzles (e.g. 8 copies of a 2-color pattern) made this exponential (3.6 s), so it also prunes with automorphisms found from equal leaves, as nauty does; the worst measured case is now about 12 ms in release. Checked against the `n!` brute force for `n_colors ≤ 6`, including symmetric layouts.
- **`max_states` counts stored states**, not expanded ones, so it bounds solver memory. `states_expanded` is still reported separately.
- **`GenConfig` holds `max_states`, not a `SolverLimits`.** `GenConfig::solver_limits()` always returns `max_time: None`, so D11 holds by construction.
- **`Generator::fresh_seed` returns `Result<u64, GenError>`.** Uniform's OS-entropy seed can fail (`GenError::Entropy`); D7 already listed that error.
- **Metrics RNG.** Core depends only on `rand_core`, so `compute_metrics::<R>` is generic over the RNG and both generators pass `ChaCha20Rng`, seeded from the canonical hash as planned. Rollouts and dead-end ratios run on `canonical_full(state)`, which makes all metrics except `states_expanded` invariant under tube and color permutation. Timeouts are left out of the dead-end ratio denominators.
- **Solution replay** matches each search edge to a real move with the exact `canonical_full`, because the approximate `solver_key` can give symmetric states different keys.
- **Golden vectors** store 64-bit values (seeds, hashes) as 16-digit hex strings, because JSON numbers above 2^53 are not exact in JavaScript.

## D13 — Phase 2 implementation refinements — decided (2026-10-02)

Recorded while implementing `uniform_water_sort` and `water_sort_cli stats`. None of these changes the generated distribution.

- **Shared rejection loop and observer.** The roadmap loop (draw, validate, continue the same stream on rejection) lives once in core as `attempt_loop(cfg, observer, candidate)`, so uniform and turan cannot drift apart. The `Generator` trait gains a required `generate_observed<O: Observer>(params, seed, cfg, observer)`; `generate` and `generate_traced` (which returns the puzzle plus `RejectionCounts`) are provided on top of it. An `Observer` gets `before_attempt` / `after_attempt(&Evaluation)` hooks; it only watches, so it cannot change which attempt is accepted. The stats command times the solver through these hooks, which keeps the clock out of core (D1, D11).
- **`evaluate_counted`** returns the `evaluate` outcome together with `states_expanded` for every outcome (rejections included), which the stats need. `evaluate` is unchanged.
- **`stats` measurement rules.** Solver time and `states_expanded` are reported per attempt (every outcome), plus a p99 over accepted puzzles only; D3 requires both p99s below 1 s. Samples run in fixed batches of 64 independent of the thread count, so every non-timing column is deterministic. To bound the run time, a cell stops early after ≥ 10 timeouts with a timeout rate above 1 % (10× the D3 limit), after 10 samples that hit `max_attempts`, or after a 900 s budget; larger `n_colors` in the same `(capacity, n_empty)` row are then skipped. Every stopped or skipped cell is unsupported. Each cell uses the same seed list `splitmix64(base_seed ^ i)`, so cells with the same `(n_colors, capacity)` share their first fills; this shows up as nearly identical `opt_moves` histograms for `n_empty` 2 and 3, where nothing is rejected (an extra empty tube rarely shortens the optimum).
- **D3 criterion tightened at review** (see D3): it also requires a whole-generation p99 < 1 s and attempts p99 ≤ `max_attempts / 10` (`ATTEMPTS_HEADROOM`). The Markdown report gains a `gen ms p50 / p99` column.
- **`standard_fills` is public in core** (not a test-only helper), so the uniform crate's tests can enumerate the accepted set with the same `evaluate`.
- **The 2-color uniformity smoke test uses its own negative control.** Its accepted set (4 states) is so symmetric that the off-by-one bound (Sattolo) still hits it uniformly (chi² ≈ 0). It uses a second off-by-one bug instead (the loop starts at `len - 2`, so the last unit never moves). The 3-color test keeps the Sattolo control.

## D14 — Two layouts in both generators: standard and distributed — decided (2026-10-02)

User request: puzzles may start with half-empty tubes, in **both** generators.

- Free space is always a whole number of tubes. Every color has exactly `capacity` units (a solved tube is full and single-colored), so `n_tubes × capacity − n_colors × capacity = n_empty × capacity` slots are free. "0 empty tubes with a half-empty tube" is impossible. What is possible is spreading 1 or 2 tubes' worth of free space over several tubes. `n_empty` keeps meaning "tubes' worth of free space" (1 or 2, D3).
- `Layout::Standard` (default): the first `n_colors` tubes full, the last `n_empty` empty. Its output stays bit-identical to Phase 2.
- `Layout::Distributed`: any fill heights `h_i ∈ [0, capacity]` summing to `n_colors × capacity`, units packed from the bottom. Uniform draws the height vector exactly uniformly (DP counts, `bounded_u64`) and then Fisher-Yates over the units. Every height vector has the same number of unit arrangements, so this stays labeled-uniform (D2). Turan's `Scramble` skips the return-to-standard step.
- `Layout` lives in core and is a field of both generators. It appears in `variant()` and therefore in every record (Phase 4), the Python env (Phase 5), the web UI selector (Phase 6) and the evaluation axes (Phase 7).
- Comparisons are made within a layout: uniform-standard vs turan-standard, and uniform-distributed vs turan-distributed. Uniform-standard vs uniform-distributed measures the layout alone.
- The D3 limits were measured for `Standard`. Phase 3 measures `Distributed` and proposes its limits as a D3 addendum; the user decides.

## D15 — Phase 3 implementation refinements and Turan defaults — decided (2026-10-02)

Recorded while implementing layouts, `turan_water_sort`, and the `stats --layout`, `compare` and `sweep` commands.

- **Construction rejections.** A strategy can give up on a candidate before solving (Scramble: the standard layout was not reached; Constrained: adjacent same-color units). Core gains `try_attempt_loop(cfg, observer, FnMut() -> Option<State>)`; `None` is an attempt rejected as `Rejection::Construction` with no solver run (`states_expanded` 0). It counts towards `max_attempts` and `GeneratedPuzzle::attempts`, the observer sees it like any rejection, and `RejectionCounts` gains `construction`. `attempt_loop` is now a wrapper around it, so uniform's output is unchanged.
- **Scramble details.** Each step draws with `bounded_u32` from `reverse_moves(state)`, excluding the exact inverse of the previous pour (`ReverseMove::new(prev.to, prev.from, prev.count)`, the only pour that restores the previous state) unless it is the only move. Then the color labels are shuffled, then the tube order (Standard: empties moved last, order of the others kept, then the full tubes shuffled; Distributed: all tubes). `Unsolvable` is `debug_assert`ed away through an observer wrapper.
- **Finding: reverse walks are absorbed early.** A pour always moves the whole top run that fits, so a state whose every non-empty tube shows a single top unit on a different color (or a one-unit tube) has *no* predecessor: no reverse move exists. Random reverse walks reach such a state after about `opt_moves` steps (median 12 at 4 × 4 × 2, 25 at 11 × 4 × 2, 18 at 12 × 3 × 1), and every walk with one empty tube ends there. With two empty tubes 6–30 % of walks are never absorbed but ping-pong one unit between a one-unit tube and an empty one. A walk with no reverse move left simply ends (the state is still solvable by construction). Consequences ([`reports/turan_steps_sweep.md`](../reports/turan_steps_sweep.md)):
  - `steps` only matters up to about 20–40: mean `opt_moves` at `steps = 40` is within 2 % of the largest value over 10–160 for every supported configuration and both layouts. It is a cap, not a difficulty knob, and Turan puzzles are shorter than uniform ones (see the compare reports).
  - **Standard:** the walk almost never needs extra steps. It ends in the standard layout only if the absorbing state already is one; otherwise it is stuck. 56–95 % of attempts are construction rejections (more at larger capacity), attempts p99 ≤ 104, cheap because no solver runs. The largest extra-step count in the whole sweep was 23.
- **`variant()` strings.** `scramble(steps=S,max_extra_steps=M,layout=standard)`, `scramble(steps=S,layout=distributed)` (M has no effect there), `constrained(layout=L)`. Uniform keeps `fisher_yates` for the standard layout (so the Phase 2 golden files are byte-identical) and uses `fisher_yates(layout=distributed)` otherwise. `Layout` serializes as `"standard"` / `"distributed"`.
- **Constrained** uses the uniform construction of the layout and accepts iff `state.segments() == n_units` (every unit is its own segment), so no new rule enters the generator. One color in the standard layout can never pass; it runs into `TooManyAttempts`.
- **Seeds.** `Turan::fresh_seed(now) = time_seed(now, SEED_COUNTER.fetch_add(1))`, never fails.
- **CLI.** `stats` takes `--generator uniform|turan`, `--layout`, `--strategy`, `--steps`, `--max-extra-steps`; its default output is `reports/<slug>_stats` (`uniform`, `uniform_distributed`, `turan_scramble_standard`, ...). The CSV format is unchanged; the Markdown report adds the variant line. Generator specs for `compare` are `uniform[:distributed]`, `turan[:scramble|constrained][:<steps>][:extra=<M>][:standard|distributed]`.
- **Defaults — decided (2026-10-02, user):** `TuranStrategy::DEFAULT_STEPS = 40` for every supported configuration and both layouts (the sweep's smallest saturating value is 10–40; one value keeps records simple and costs nothing, since walks are absorbed first). `DEFAULT_MAX_EXTRA_STEPS = 100` (max needed: 23; larger values only waste time on ping-pong walks, e.g. 10 000 made 4 × 4 × 2 generation 4× slower).
- **Follow-up — accepted (2026-10-02, user), before Phase 4:** because `steps` saturates, the Scramble difficulty knob is weak. Alternatives, each a new `TuranStrategy` arm or a `VERSION` bump: (a) steer walks away from absorbing states (never pick a reverse move into a state with no reverse move) so `steps` keeps mixing; (b) make Distributed the Scramble default (no construction rejections at all). Phase 3 implements the plan's algorithm as written; (a) is added in a follow-up PR as a new strategy arm, so existing seeds keep their puzzles.

## D16 — Turan difficulty follow-up: opt band, `PourWalk`, `ReverseSearch`, difficulty tiers — decided (2026-10-02)

Follow-up (a) of D15. Reports: [`reports/turan_mixing_probe.md`](../reports/turan_mixing_probe.md) (why (a) fails), [`reports/turan_difficulty_d16.md`](../reports/turan_difficulty_d16.md) (what replaced it, with sources), the two sweeps and three compares linked from there.

- **Finding: no reverse walk can make `steps` a difficulty knob.** Let `E(s) = Σ (top_run − 1)` over non-empty tubes. No reverse move increases `E`, and the solved state has `E₀ = n_colors × (capacity − 1)`, so a reverse walk makes at most `E₀` progressing steps; after that it can only move single units back and forth. This holds whatever rule picks the moves. Measured: the planned non-absorbing rule (and a depth-2 lookahead) leaves mean `opt_moves` identical at 40, 160 and 640 steps, and the standard-layout return then never succeeds with two empty tubes (`TooManyAttempts`). The planned `ScrambleMixing` arm was therefore not added. `water_sort_core::has_reverse_move` (does a state have a predecessor) stays, with tests.
- **Approach (user, 2026-10-02):** follow what other puzzle generators and the RL literature do instead:
  - **A. `GenConfig::max_opt: Option<u32>`.** Together with `min_opt` it gives a target `opt_moves` band (new `Rejection::AboveMaxOpt`, `RejectionCounts::above_max_opt`). `None` (the default) is left out of the JSON and `serde(default)` reads old JSON, so every existing golden file and record is unchanged. CLI: `--max-opt` on `stats` and `compare`. The stats CSV gains a trailing `rate_above_max_opt` column, and its Markdown column becomes `outside band %`.
  - **B. `TuranStrategy::PourWalk { steps }`** (`pour_walk(steps=S,layout=distributed)`): a random walk on the undirected pour graph. Each step draws with `bounded_u32` from the legal pours followed by the reverse moves (fixed orders), excluding the exact undo of the previous step unless nothing else is left; then labels and all tubes are shuffled. It can end unsolvable or solved, and validation rejects those. **Distributed only:** in the standard layout, a random state rarely has standard heights (0.5–2 % with two empty tubes), so a return step would take hundreds of extra steps and do the mixing itself. `Layout::Standard` returns the new `GenError::UnsupportedLayout`.
  - **C. `TuranStrategy::ReverseSearch { max_depth, max_states }`** (`reverse_search(max_depth=D,max_states=N,layout=L)`): the I2A / Boxoban Sokoban generator (Weber, Racanière et al. 2017).
    - Search: depth-first over reverse moves from the solved state. Each node's moves are shuffled with `fisher_yates`, states already visited are skipped (a `HashSet` used only for membership), paths stop at `max_depth`, and the search stops after `max_states` distinct states.
    - Score: every visited state gets color switches along its path × `heuristic` (`segments − n_colors`). A state scores 0 if it has a sorted tube, or in the standard layout if its heights are not standard.
    - Result: the first highest-scoring state, then labels and tubes shuffled. If nothing scores above 0, the attempt is a construction rejection. Solvable by construction.
  - CLI: `--strategy pour-walk|reverse-search`, `--search-depth`, `--search-states` on `stats`. Compare specs `turan:walk:<steps>:distributed` and `turan:search:<states>[:depth=D]:<layout>`. On `sweep`, `--strategy` (default `scramble`, so the Phase 3 report reproduces unchanged); for reverse search, `--steps` sweeps the state budget. Non-scramble sweeps use the layout's own supported table and list uniform's mean next to each configuration.
  - `Turan::VERSION` stays 1. `Scramble` and `Constrained` output is bit-identical (the existing golden files pass unchanged). New golden files cover `pour_walk` (distributed) and `reverse_search` (both layouts) with the default parameters. The 10k replay heavy test covers both arms.
- **Measured** (`reports/turan_difficulty_d16.md`, same 9 configurations and seeds as the Phase 3 compares):
  - **PourWalk:** mean `opt_moves` levels off at 92–100 % of uniform's between 80 and 640 steps (later for larger configurations). At 160 steps it reaches 69–100 % (lowest with capacity 5 and one empty tube, which mixes slowest), with KS D from uniform 0.07–0.7 on the compare configurations, no repeated puzzles, and generation p99 ≤ 390 ms over the whole supported range.
  - **ReverseSearch 10k:** 1.5–4 moves harder than Scramble. It matches uniform at the smallest configurations and stays far below at large ones (10x4x2: 22.6 vs 31.3). Generation p99 is 20–160 ms. A 100k budget adds only 1–3 moves at 10× the cost. Small configurations repeat puzzles (6x3x1: 30–35 % distinct out of 1000).
  - **Bands:** uniform reaches each of its own `opt_moves` tertiles in 3–8 attempts. Scramble and ReverseSearch almost never reach uniform's medium or hard tier beyond the smallest configurations, so a band on them can only select within their own range. PourWalk 160 puts 1–22 % of its puzzles in uniform's hard tier.
- **Defaults — decided (2026-10-02, user):** all accepted as proposed below; `TuranStrategy::default()` is now `ReverseSearch { 300, 10_000 }` (the golden test's `scramble` entries now name `TuranStrategy::scramble(40)`; their output is unchanged), and difficulty tiers are implemented (next item). The proposals were:
  - `TuranStrategy::DEFAULT_WALK_STEPS = 160`: one value for every configuration. It is near the saturation point for most configurations and keeps PourWalk partly distinct from uniform; 640 would make it practically uniform.
  - `DEFAULT_SEARCH_DEPTH = 300` (I2A's value) and `DEFAULT_SEARCH_STATES = 10_000`, where extra budget stops paying off.
  - `max_opt` stays `None` by default. Difficulty tiers (Boxoban-style easy / medium / hard) are better applied when Phase 4 samples from the dataset, using the recorded `opt_moves`, than at generation time, because a generation-time band only selects within one generator's range.
  - **Default strategy:** propose switching `TuranStrategy::default()` from `Scramble` to `ReverseSearch { 300, 10_000 }`. It is the RL literature's standard generator for this kind of puzzle, harder than Scramble while still distinct from uniform, works in both layouts, and needs about 1 attempt. Costs: about 10× Scramble's generation time (roughly 2–3 h for 1M puzzles on 3 threads) and repeats at the smallest configurations. If accepted, the golden test's `scramble` entries must name `TuranStrategy::scramble(40)` explicitly, because they currently use `default()`. Alternative: keep `Scramble` as the default and offer the other two as options.
- **Difficulty tiers — decided (2026-10-02, user).** `water_sort_core::Tier` (`easy` / `medium` / `hard`), modeled on Boxoban's unfiltered / medium / hard sets but defined by `opt_moves` rather than agent success (agent-based tiers can come in Phase 7).
  - **Cut points:** fixed per supported configuration and layout from uniform's measured distribution (`reports/uniform_stats.csv`, `reports/uniform_distributed_stats.csv`, 1000 puzzles each). `easy_max < medium_max` are the observed `opt_moves` values whose cumulative shares are nearest to 1/3 and 2/3 (the sum of both distances is minimized). Tiers therefore mean the same difficulty for every generator; Turan's narrower range simply has few medium or hard puzzles at larger configurations.
  - **Coverage:** every tier holds at least 12 % of uniform's puzzles. Plain tertile quantiles were rejected, because on these discrete distributions they leave an empty medium tier at 2x3x1 distributed. The tables live in core (`TIERS_STANDARD`, `TIERS_DISTRIBUTED`, `tier_row`). A CLI test recomputes them from the committed reports, and a core test checks that they cover exactly the supported range.
  - **API:** `Tier::of(params, layout, opt_moves)` classifies a puzzle; `Tier::opt_band` gives the `(min_opt, max_opt)` band for `GenConfig`.
  - **CLI:** `--tier easy|medium|hard` on `stats` and `compare`; it conflicts with `--min-opt` and `--max-opt`. Unsupported cells are skipped by `stats` and refused by `compare`.
  - **Phase 4:** records store `tier`, and tiers are applied when sampling the dataset (see the Phase 4 plan).
