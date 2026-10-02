# Phase 3 — start prompt

Copy the block below into a new coding-agent session.

---

```text
You are implementing Phase 3 of the jepa_games project:
(1) `turan_water_sort`, a second puzzle generator that produces a DIFFERENT KIND of puzzle from
uniform at minimal code cost, and
(2) a second LAYOUT in BOTH generators: besides the standard layout (full tubes + whole empty
tubes), a distributed layout where the free space is spread over the tubes (half-empty tubes).

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-3-turan`. Read CLAUDE.md at the repo root first (commands, rules, workflow).

STATE OF THE PROJECT
- Phase 0: Cargo workspace, toolchain pinned to 1.99.0 (rust-toolchain.toml; run
  scripts/setup-cloud.sh if `cargo` is missing), lints (unsafe_code forbid, clippy pedantic),
  CI: ubuntu + windows (fmt, clippy -D warnings, test) and ubuntu `heavy-tests`
  (`cargo test --release --workspace --locked -- --ignored`).
- Phase 1 (`water_sort_core`, D12) and Phase 2 (`uniform_water_sort`, `water_sort_cli stats`,
  D13) are merged. D3 is decided: only 1 or 2 empty tubes (MAX_SUPPORTED_EMPTY), measured for the
  standard layout. Core already provides:
    Generator trait: ID, VERSION, variant(), fresh_seed(now_nanos) -> Result<u64, GenError>,
      required generate_observed<O: Observer>(params, seed, cfg, observer); provided
      generate / generate_traced / assemble::<R>.
    attempt_loop(cfg, observer, candidate) - the shared rejection loop (use it; never re-write it).
    reverse_moves(&State) -> Vec<ReverseMove> (fixed order), unapply, ReverseMove::forward,
      is_valid_reverse.
    State::solved / from_tubes / from_fill / sorted_units / tube / height / is_tube_full /
      is_tube_empty / segments / color_changes, solver::replay, bounded_u32, fisher_yates,
      seed::{splitmix64, time_seed}, standard_fills, is_symmetric, SUPPORTED / is_supported.
  uniform_water_sort/src/lib.rs is the reference implementation of a generator; mirror its shape.
  Read the actual source before writing code. Do not duplicate anything core provides.

READ FIRST, in this order:
1. CLAUDE.md
2. roadmap.md (Phase 3 section; the "Decided (D1)" box)
3. docs/README.md (cross-cutting rules)
4. docs/decisions.md (D1-D14 override the roadmap; D1 defines Turan, D3 the supported range,
   D14 the two layouts)
5. docs/phases/phase-3-turan/PLAN.md (your detailed plan; follow its task order)
6. docs/phases/phase-2-uniform/PLAN.md and reports/uniform_stats.md (what you compare against)

SCOPE: Layout support in water_sort_core and uniform_water_sort, the new turan_water_sort,
Turan + layout support in `water_sort_cli` (`stats`, new `compare`). No dataset `generate`, no
Python, no web.

KEY REQUIREMENTS (details in PLAN.md)
- Layout (D14) in core: `enum Layout { Standard (default), Distributed }`. Distributed = any
  heights h_i in [0, capacity] with sum = n_colors * capacity, units packed from the bottom.
  Add State::from_heights, bounded_u64 (Lemire on next_u64, no usize), sample_heights (EXACTLY
  uniform over valid height vectors via DP completion counts; not rejection),
  distributed_fills (exhaustive, for tests), layout checks.
- Uniform: `Uniform { layout }`. Standard output must stay BIT-IDENTICAL to Phase 2: the existing
  golden vectors must pass unchanged and VERSION stays 1. Distributed = sample_heights, then
  fisher_yates over sorted_units, then from_heights; variant "fisher_yates(layout=distributed)".
  Distributed uniformity chi-square on 2x2x1 and 3x3x1 via distributed_fills, with a negative
  control that must FAIL (heights drawn tube by tube, each uniform over its feasible range; NOT
  "independent heights + reject", which is exactly uniform).
- Turan: ID "turan", VERSION 1. `Turan { strategy, layout }`,
  `TuranStrategy { Scramble { steps, max_extra_steps }, Constrained }`; variant() like
  "scramble(steps=40,layout=standard)".
- Seed (D1): fresh_seed(now_nanos) = time_seed(now_nanos, COUNTER.fetch_add(1)) with a
  process-wide AtomicU64. Core never reads the clock; the CLI passes SystemTime::now(). An
  explicit seed always wins. RNG = ChaCha20Rng::seed_from_u64(seed).
- Scramble candidate (inside attempt_loop's closure, continuing ONE rng stream): start from
  State::solved; `steps` random reverse moves via bounded_u32 over reverse_moves (excluding the
  exact undo of the previous one); Standard: continue until exactly n_colors full + n_empty empty
  tubes (at most max_extra_steps, else a rejected attempt) and move the empties last;
  Distributed: no extra step; then shuffle color labels and tube order (full tubes only for
  Standard, all tubes for Distributed) with fisher_yates; evaluate() validates; debug_assert no
  Unsolvable. Record how failed layout returns are counted as D15.
- Constrained (optional, last): the uniform construction for the chosen layout + reject tubes
  with two vertically adjacent same-color units.
- Determinism: same (params, seed, cfg, strategy, layout) -> identical GeneratedPuzzle on Linux
  and Windows. Golden vectors: 20 seeds x 3 configs x each (generator, strategy, layout) under
  <crate>/tests/golden/*.json (64-bit values as 16-digit hex strings; regenerate with
  WATER_SORT_BLESS=1).
- Tests: 10k seeds per (generator, layout) replay to solved in opt_moves (heavy-tests); every
  output matches its layout; Turan never Unsolvable; fresh_seed differs at equal now_nanos;
  puzzle-code round trip for distributed states; Constrained never yields adjacent same colors.
- CLI: `stats --layout standard|distributed`, `stats --generator turan --strategy scramble
  --steps N` (extend the existing GeneratorKind; reuse the stats machinery; D3 criterion and
  report format unchanged; `--empty 1..=2` is the default). New `compare --a <spec> --b <spec>`
  writing reports/uniform_vs_turan_standard.md and reports/uniform_vs_turan_distributed.md
  (side-by-side table, histograms of opt_moves / color changes / segments / random stuck +
  capped rates / states_expanded, chi-square and KS two-sample tests, canonical-hash overlap),
  plus uniform-standard vs uniform-distributed.
- Distributed supported range: run the stats grid for uniform with --layout distributed, commit
  reports/uniform_distributed_stats.{csv,md}, and PROPOSE distributed limits as a D3 addendum
  (status proposed; the user decides). Give is_supported a layout-aware form and record the API.
  Do not change the decided standard-layout D3 table.
- `steps` sweep (10, 20, 40, 80, 160) per layout on the supported configurations; propose default
  `steps` per configuration (D15, proposed).

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits: feat/test/chore/docs).
- After every task: cargo fmt, clippy -D warnings, cargo test --workspace --locked pass.
- Tick the checkboxes in docs/phases/phase-3-turan/PLAN.md and the roadmap Phase 3 items; update
  the status in docs/README.md when done.
- If the plan is wrong or infeasible (e.g. returning to the standard layout is too expensive),
  STOP and report with measurements before deviating. Record accepted deviations as D15+ in
  docs/decisions.md.
- Commit generated reports; put large raw outputs under data/ (gitignored). Long measurement
  grids: run in release with a bounded --threads and report wall time.

DONE WHEN: both layouts in both generators; Standard output unchanged (Phase 2 golden vectors
pass); Turan implements Generator with no rules outside core; same seed -> same puzzle (golden
vectors); distributed uniformity test passes with a failing control; all outputs solvable and
matching their layout; stats, distributed grid and compare reports committed; steps defaults and
distributed limits proposed; CI green on ubuntu, windows and heavy-tests. Then push the branch,
open a PR to main summarizing what was built, the measured differences and the decisions
recorded, and report back.
```
