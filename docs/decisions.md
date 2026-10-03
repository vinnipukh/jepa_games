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

## D17 — Phase 4 implementation refinements — decided (2026-10-03)

Recorded while implementing the dataset pipeline (`water_sort_cli generate`, `dedup-report`, `leakage`, `validate`). None of these changes what a generator produces.

- **Where the record type lives.** In `water_sort_cli`, which gains a library target (`water_sort_cli::dataset`, plus the shared `args`), not in a new crate. Phase 5 and 7 read datasets with `pyarrow`, so the Parquet schema is the contract and the Python binding needs no Rust dependency on the dataset code (which pulls in `arrow` / `parquet`). The library target also lets integration tests call the pipeline directly.
- **Schema details.** As in the plan, with: `params` a struct of three `uint8`; `state` `fixed_size_binary(n_tubes × capacity)`; `tier` a nullable string (null only with `--allow-unsupported`, where no tier table exists); enum columns (`layout`, `tier`, `split`) as strings (Parquet dictionary-encodes them); the metrics flattened as `metrics_*` columns, without `metrics.opt_moves` (it equals `opt_moves`). `created_at` is `timestamp[us, UTC]`: nanoseconds do not convert to Python's `datetime`, so the run's clock reading is truncated to whole microseconds. In JSONL, seeds and hashes are 16-digit hex strings (rule 8) and `created_at` is RFC 3339.
- **`generator_variant` is the generator's own `variant()`.** The plan's example `fisher_yates(layout=standard)` is not used: uniform's standard-layout variant stays `fisher_yates` (D15), so the Phase 2 golden files are unchanged. The `layout` column (and the manifest) carry the layout for every generator.
- **`created_at` and determinism.** One clock reading per run (the start), shared by every record, so the data do not depend on timing. `--created-at <RFC 3339>` pins it; two runs with the same master seed and the same `created_at` are byte-identical whatever the thread count or chunking (tested with 1–4 threads and several chunk sizes, at 10k in the heavy tests, and at 1M, see D18). The manifest's `started_at` / `finished_at` / `wall_time_secs` / `threads` describe the run and are excluded from the comparison; the manifest's file list (sha256 per file) is the comparison.
- **Parallel writer.** Chunks of 256 indices run on a rayon pool (`--threads`, else `RAYON_NUM_THREADS`, else all cores); 64 chunks form a window that is collected in index order and handed through a bounded channel (2 windows) to one writer thread, which writes while the next window is generated. Parquet: zstd level 3, one row group per 64k records, `created_by` left at the `parquet` crate's default.
- **Dedup happens while writing.** `--count` is the number of indices generated; duplicates (same canonical form as an earlier record, confirmed by comparing the exact canonical encodings) are dropped, so a dataset holds `count − duplicates − failures` records. Dropped ids are listed in `duplicates.csv` with the id they duplicate. Memory is one hash-map entry plus one canonical encoding per kept record (about 50 MB at 1M records, 6 × 4 × 2). The report adds a saturation curve (duplicates at every power of ten of generated puzzles).
- **Failures.** An index whose generation hits `max_attempts` is listed in the manifest's `failures` and skipped; any other generator error (invalid params, a strategy that does not support the layout) would fail every index and aborts the run.
- **Directory layout.** `manifest.json`, `dedup_report.md`, `duplicates.csv`, and `puzzles.<ext>` or, with `--split-files`, `train.<ext>` / `val.<ext>` / `test.<ext>`. Every record keeps its `split` column either way. The manifest stores the generator as a spec string (`turan:search:10000:depth=300:standard`, the `compare` syntax with every parameter spelled out), which `validate` parses to regenerate records. `tool_version` is `water_sort_cli <version>+<git commit>`; `build.rs` embeds the commit (`WATER_SORT_GIT_COMMIT` overrides it).
- **`leakage`.** `--a-split` / `--b-split` take `train`, `val`, `test` or `all` (defaults `test` / `train`). Shared puzzles are matched by canonical hash and confirmed by encoding. `--exclude-from a|b --out <dir>` writes a copy of that side's dataset without its shared records (only in its selected split), with the same format and file layout, and records the exclusion (source and other dataset, their manifest sha256, splits, count removed) in the new manifest. The copy passes `validate`.
- **Finding: cross-generator leakage between different splits is zero by construction.** The split is the same function of the canonical hash in every dataset, so with equal ranges uniform's test split and Turan's train split cannot share a puzzle (measured 0 at 4 × 4 × 2 and 6 × 4 × 2). Overlap needs one side to use all its records, the same split on both sides, or different ranges: at 4 × 4 × 2, 188 of 4593 Turan puzzles (all splits) are in uniform's test split, 1694 in uniform's whole 50k dataset; at 6 × 4 × 2, 2 of 19 615 Turan puzzles are anywhere in uniform's 1M. Phase 7 cross-evaluation that trains on a whole dataset of one generator must run `leakage --b-split all --exclude-from b` first; training on the train split alone needs nothing.
- **`validate`** checks every record against the manifest and core: generator identity, params, `gen_config`, `created_at`, seed rule, state validity, puzzle code, canonical hash, solution replay to solved in exactly `opt_moves` moves, the `GenConfig` band, tier, split (and the split of the file it is in), `metrics_random_rollouts`; across records: increasing ids per file, ids below `count` and not failed, no canonical form twice, counts per file / split / tier, file sha256 and sizes. `--regen-rate` (default 0.01) regenerates a deterministic sample (by `splitmix64(record_id ^ salt)`) from its seed and compares the whole record. A build whose generator `VERSION` or variant differs from the manifest is reported, since its regenerations would not match.
- **CI fixture.** Two small datasets are committed under `water_sort_cli/tests/fixtures/datasets/` (uniform 5 × 4 × 2 Parquet with split files, 200 indices; Turan pour walk 4 × 3 × 1 distributed JSONL, 120 indices). A test regenerates them and requires byte-identical record files on Linux and Windows (`WATER_SORT_BLESS=1` rewrites them), a test validates them with every record regenerated, and the CI `rust` job also runs `water_sort_cli validate` on both.

## D18 — Phase 4 dataset configuration and acceptance run — decided (2026-10-03)

Evidence: [`reports/dataset_phase4.md`](../reports/dataset_phase4.md), with the manifest and duplicate report in [`reports/dataset_uniform_c6k4e2/`](../reports/dataset_uniform_c6k4e2/).

- **Proposed primary dataset configuration:** uniform, standard layout, 6 colors / capacity 4 / 2 empty (`n_empty = 2` is the primary configuration, D3), `min_opt` 1 and the default `GenConfig`, 80 / 10 / 10 split, all tiers kept (tiers are applied when sampling, D16). Reasons: comfortably inside the supported range (generation p99 9 ms, D3 report), 1M puzzles in 11 minutes on 4 cores, a puzzle space large enough that duplicates stay negligible (71 in 1M), and `opt_moves` 17.9 on average (p99 21), long enough for planning to matter while staying cheap for Phase 5 trajectory logging. Larger configurations (8 × 4 × 2: gen p50 11 ms, about 4× slower; 10 × 4 × 2: about 17×) are the natural next datasets if Phase 7 needs harder puzzles.
- **Measured at 1M:** 999 929 records after dedup, 667.8 s on 4 threads, 62.7 MB of Parquet (train 50.1, val 6.3, test 6.3 MB), `validate` 11.1 s with 9931 records regenerated. A second run with 2 threads, the same master seed and the same `created_at` gives byte-identical record files.
- **Decided (2026-10-03, user):** 6 × 4 × 2 standard uniform is the dataset configuration for Phases 5 and 7. No larger dataset is generated now; [`docs/datasets.md`](datasets.md) documents how to make one (larger sizes, `--tier hard`). Turan datasets use the default reverse search (`turan:search:10000:depth=300`); the other strategies (`pour-walk` distributed only, `scramble`, `constrained`) stay available through `--strategy` (user, 2026-10-03).

## D19 — Phase 5 implementation refinements — decided (2026-10-03)

Recorded while implementing the Python binding, `WaterSortEnv`, the native vector env and the trajectory logger. None of these changes what a generator produces; every existing golden file is unchanged.

- **Versions.** PyO3 0.29.3 and numpy (rust-numpy) 0.29.0, the newest releases (both MSRV 1.83, well below the pinned 1.99), `abi3-py310` (one wheel per OS for Python ≥ 3.10), maturin ≥ 1.9 (developed with 1.15), Gymnasium ≥ 1.0 (developed with 1.3), pyarrow ≥ 15. PyO3 0.26 renamed `allow_threads` to `Python::detach`; the batch functions, `solve` and `generate` call it. The crate `python/` (package `jepa_water_sort`, module `jepa_water_sort._native`) is a workspace member, so fmt, clippy and `Cargo.lock` cover it; it has no Rust tests (they would link libpython) and is tested from pytest. `.cargo/config.toml` sets `PYO3_BUILD_EXTENSION_MODULE=1`, so a plain `cargo build --workspace` does not link libpython on Linux/macOS either (maturin sets it too).
- **Episode rules live in core.** The plan put the env logic in `env.py`. Reward, illegal actions, termination and truncation are game rules, so they are `water_sort_core::episode` (`episode_step`, `EpisodeRules`, `move_limit`, `is_dead_end`, `potential`), exposed as `env_step` / `batch_env_step`. The env, the native vector env, the logger and the golden rollouts all call it (rule 1). Details:
  - The reward is exactly `-1 + (γ·Φ(s') − Φ(s))` in f64, `Φ = −color_changes`. Shaping also applies to illegal self-loops (`(γ − 1)·Φ(s)`), which keeps it potential-based on every transition; with shaping off an illegal action is plain −1, as planned. A solved state has `Φ = 0`.
  - Solved is checked first, then dead end, then truncation, so the last allowed move that solves is `terminated`, not `truncated`.
  - `dead_end_check` asks the solver with a state budget (default 100 000); a solver timeout is not a dead end.
- **Binding API additions.** `Params` (also accepted as a tuple), `GenConfig` (its `to_json` / `from_json` is the manifests' `gen_config_json`), `Puzzle` (all record fields plus `layout`, `tier`, `solution_actions`), `SolveResult` (`status` is `solvable` / `unsolvable` / `timeout`), `canonical`, `tier`, `splitmix64`, `time_seed`, `variant`, `fresh_seed`, `batch_action_mask`. `step` takes `(from, to)` or an action index. Strategies are strings: a name (`reverse_search`, `scramble`, `pour_walk`, `constrained`) optionally with the arguments of its `variant()` string, e.g. `"scramble(steps=40)"`; a full variant string reads back as itself. `State.to_numpy()` is the dataset `state` encoding reshaped to `(n_tubes, capacity)` (255 = empty); `from_numpy` infers `n_colors` from the number of units.
- **`batch_step`** returns `(next_states, units_moved)` with `units_moved = 0` for an illegal action instead of raising, so one bad row does not discard a batch (every legal pour moves at least one unit). Malformed states and out-of-range actions still raise `ValueError`, naming the first bad row.
- **`WaterSortEnv` defaults.** Turan's default strategy is `ReverseSearch` (D16 changed `TuranStrategy::default()` after the plan said `scramble`). The env's default `GenConfig` is the core default with `random_rollouts = 0`: rollouts only fill the metrics, so every seed gives the same puzzle as in a dataset, and resets are cheaper. `info["seed"]` and `info["canonical_hash"]` are `numpy.uint64` (a Python int ≥ 2^63 breaks Gymnasium's vector-env info batching); `info["solvable"]` (with `compute_solvability`) is the solver status string. Extra: `move_limit` and, after a step, `illegal`, `dead_end`, `solved`, `units_moved`; `options={"puzzle": Puzzle}` besides `puzzle_seed` / `puzzle_code`. Registered as `jepa_water_sort/WaterSort-v0`.
- **`WaterSortVectorEnv`** uses Gymnasium's default next-step autoreset and gives sub-env `i` its own `np_random` seeded with `seed + i`, as `SyncVectorEnv` seeds its copies, so both produce identical observations, rewards, flags and infos (tested step for step, also against `AsyncVectorEnv`). `AsyncVectorEnv` should use `context="spawn"` (or `forkserver`): forking after the rayon pool has started can deadlock the children.
- **Logger.** Row schema `puzzle_id, generator_id, canonical_hash, record_id, split, source, episode, step, state, action, next_state, done, truncated, illegal, solved`: the planned unit row plus the parts of `puzzle_id` as columns, the dataset `record_id` and `split`, and `truncated` / `solved` so `done` can be split into its causes. `puzzle_id = "<generator_id>:<canonical hash, 16 hex digits>"`; `state` / `next_state` are `fixed_size_binary(n_tubes × capacity)` like the dataset column. A shard closes at the first episode end after `shard_size` (default 1 000 000) transitions, so episodes never span shards. Episode randomness is `SeedSequence([policy_seed, record_id, repeat])`, so a collection is byte-reproducible and order-independent. The "dataset manifest hash" is the sha256 of `manifest.json`. `collect` re-checks each record's split against `canonical_hash % 100`, so a trajectory set from one split cannot contain another split's puzzle. `optimal` replays the record's stored solution. A fifth source `greedy` (one-step heuristic lookahead) was added for baselines. `human` import takes `{puzzle_code, actions, [generator_id, opt_moves, record_id, split]}` per game; Phase 6 defines the real export and may adjust it.
- **serde_json `float_roundtrip`** is enabled workspace-wide: without it serde_json reads some f64 values one ULP off, and the golden rollouts' shaped rewards must read back exactly. Parsing only becomes exact; no output changes.
- **Branch.** The cloud session was bound to branch `ccr-ebe28a1c-e1yd7d`, so Phase 5 was developed there instead of `phase-5-python`.
- **Measured** (4-core cloud VM, 6 × 4 × 2 uniform): native vector env 31k steps/s at 64 envs and 71k at 1024 vs 15k / 18k for `SyncVectorEnv` (random legal actions; puzzle generation on autoreset dominates). Trajectory collection on 5000 train puzzles, single thread: optimal 65k transitions/s, ε = 0.1 18k/s (re-solving after deviations), random 85k/s, greedy 39k/s; about 15 bytes per transition in zstd Parquet. Within `4 · opt_moves`, random play solved 75 % of the puzzles and greedy 76 %.
- **Decided (2026-10-03, user):** all of the above accepted as proposed, including the 100 000-state dead-end budget and the provisional `human` import format (Phase 6 may adjust it). D4 (`k = 4`) stays proposed until the RL baseline (Phase 7).

## D20 — Phase 6 implementation refinements — proposed (2026-10-03)

Recorded while implementing the `water_sort_web` crate (wasm-bindgen), the Vite + TypeScript game in `web/app` and the human trajectory export. None of this changes what a generator produces; every golden file is unchanged and is now also checked on wasm32.

- **getrandom 0.4 needs no cfg flag.** The plan expected `--cfg getrandom_backend="wasm_js"`; getrandom 0.4.3 selects its JS backend from the `wasm_js` feature alone. `water_sort_web` enables it (it depends on wasm-bindgen anyway, the case getrandom's docs allow); feature unification turns it on for `uniform_water_sort::fresh_seed`. `.cargo/config.toml` only sets the wasm test runner (`wasm-bindgen-test-runner`). wasm-bindgen 0.2.129; `scripts/wasm-bindgen-version.sh` reads the locked version, and the setup script and CI install exactly that wasm-bindgen-cli.
- **Shared strategy parser.** The Turan strategy spec parser (`"scramble(steps=40)"`, full `variant()` strings) moved from the Python binding to `turan_water_sort::parse_strategy`, so Python and the web share it. Python behavior is unchanged.
- **JS API.** snake_case names as in the plan and the Python package: `generate(generator, params, seed?, strategy?, layout?, tier?)`, `generate_with_config(..., config_json)` (golden tests; no supported-range check), `from_code(code, max_states?)`, `Session` (`pour` returns units moved, 0 = illegal and not counted; `why_illegal`, `undo`, `restart`, `moves_counted`, `can_undo`, `is_solved`, `stars`, `cells`, `legal_moves`, `state_code`, `export_trajectory`), `Puzzle` getters, `variant`, `fresh_seed`, `strategies`, `supported_rows`, `stars`, `star_moves`, `canonical_hash`, `splitmix64`, `web_config_json`. Seeds and hashes are 16-digit hex strings (accepted: 1–16 hex digits, optional `0x`), never JS numbers; solutions and legal moves are action indices `from * n_tubes + to`. The worker hands a puzzle to the page as `Puzzle.to_json()`; `Puzzle.from_json` re-checks that the code decodes, the hash matches and the solution solves it.
- **The web's generation settings** (proposed): `GenConfig::default()` with `random_rollouts = 0`, as Python's `WaterSortEnv` (rollouts only fill metrics, so a seed gives the same puzzle as `jepa_water_sort.generate` with its default config). `generate` refuses params outside the supported range of the layout (D3), so a URL cannot start an unbounded generation. A Turan seed comes from `Date.now()` (ms → ns) and the process counter; uniform's from `crypto.getRandomValues`.
- **Difficulty tiers in the UI: uniform only** (proposed). The tier cut points come from uniform's `opt_moves` distribution (D16). Measured in wasm (Node): several Turan strategies never or rarely reach some bands, e.g. `constrained` 6×4×2 easy and medium found no puzzle within 10 000 attempts (6 s each), `scramble` 6×4×2 hard failed for 1 of 3 seeds, and `reverse_search` distributed 8×5×2 hard did not finish within 60 s. Uniform tiers at the largest supported configurations take at most 1.5 s. Turan puzzles show the tier they fall into.
- **Puzzles opened by code** are solved with a 5 000 000-state budget (the generation default, never a time limit). If the solver gives up, the puzzle is still playable, without stars. The layout is inferred from the heights (standard if they are standard).
- **UI time guard** (proposed): 60 s. A worker request still running then is abandoned (the worker is terminated and restarted) and reported; it never decides which puzzle is accepted (D11). Untiered generation at the largest supported configurations takes at most 0.2 s in wasm for every generator and strategy.
- **Human trajectory export** (format `jepa_water_sort.human_trajectory`, version 1): the puzzle's identity (`puzzle_id`, generator, version, variant, layout, seed, params, `GenConfig`, code, hash, `opt_moves`, tier), a random 64-bit `session_id`, start and export time, `moves_counted`, `solved`, `stars`, and one row per pour, illegal pour, undo and restart. Rows have the Phase 5.3 columns (`state` / `next_state` as cell arrays, 255 = empty; `action` null for undo/restart; `episode` 0, `record_id` / `split` null) plus `event`, `t_ms` (since the start), `units_moved`, `moves_counted`. Out-of-range clicks are not recorded (the UI cannot produce them).
- **Import:** `logger.import_web_exports` (CLI `import-human`) replays every row through core and rejects the whole import on any disagreement (state, next state, legality, units, move count, hash, format). Each run of pours between undos and restarts is one episode of effective transitions, replayed with `replay_actions` (no move limit, so `truncated` is always false; illegal pours kept and flagged). The manifest lists the sessions. D19's provisional `import_human` (action lists) stays for scripted input.
- **wasm tests** compile the golden files in with `include_str!` (no file system on wasm32-unknown-unknown); a native test keeps the list of generator files complete. They run in release (3.5 s) in the CI `wasm` job, which also builds `web/pkg` and typechecks and builds the app (Node 22, Vite 8.3, TypeScript 7.0).
- **Hosting:** GitHub Pages is not set up. The repo is private, so Pages may need a paid plan; the user decides. `vite build` uses relative asset paths, so `web/app/dist` works from any sub-path.
- **Branch.** The cloud session was bound to branch `ccr-9eabdd07-gdj3xu`, so Phase 6 was developed there instead of `phase-6-web`.
- **Measured:** wasm module 432 kB (145 kB gzip), app JS 22 kB. Manual spot check: eight seeds / codes across both generators, all strategies and both layouts gave identical puzzle codes on the web and in Python (`docs/phases/phase-6-web/PLAN.md`, manual checklist).
