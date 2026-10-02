# Phase 3 — start prompt

Copy the block below into a new coding-agent session.

---

```text
You are implementing Phase 3 (`turan_water_sort`) of the jepa_games project: a second puzzle
generator that produces a DIFFERENT KIND of puzzle from uniform at minimal code cost.

Repo: github.com/vinnipukh/jepa_games (private). Start from an up-to-date `main` and create
branch `phase-3-turan`.

STATE OF THE PROJECT
- Phase 0: Cargo workspace, toolchain pinned to 1.99.0 (rust-toolchain.toml; if `cargo` is
  missing, install rustup and the pinned toolchain is picked up automatically), lints
  (unsafe_code forbid, clippy pedantic), CI: ubuntu + windows (fmt, clippy -D warnings, test) and
  ubuntu `heavy-tests` (`cargo test --release --workspace --locked -- --ignored`).
- Phase 1 (`water_sort_core`, D12) and Phase 2 (`uniform_water_sort`, `water_sort_cli stats`,
  D13) are merged. Core already provides everything Turan needs:
    Generator trait: ID, VERSION, variant(), fresh_seed(now_nanos) -> Result<u64, GenError>,
      required generate_observed<O: Observer>(params, seed, cfg, observer); provided
      generate / generate_traced / assemble::<R>.
    attempt_loop(cfg, observer, candidate) - the shared rejection loop (use it; never re-write it).
    reverse_moves(&State) -> Vec<ReverseMove> (fixed order), unapply(&State, ReverseMove),
      ReverseMove::forward(), is_valid_reverse.
    State::solved / from_tubes / from_fill / tube / is_tube_full / is_tube_empty / segments /
      color_changes, solver::replay, bounded_u32, fisher_yates, seed::{splitmix64, time_seed},
      is_symmetric, SUPPORTED / is_supported.
  uniform_water_sort/src/lib.rs is the reference implementation of a generator; mirror its shape.
  Read the actual source before writing code. Do not duplicate anything core provides.

READ FIRST, in this order:
1. roadmap.md (Phase 3 section; the "Decided (D1)" box)
2. docs/README.md (cross-cutting rules)
3. docs/decisions.md (D1-D13 override the roadmap; D1 defines Turan, D3 the supported range)
4. docs/phases/phase-3-turan/PLAN.md (your detailed plan)
5. docs/phases/phase-2-uniform/PLAN.md and reports/uniform_stats.md (what you compare against)

SCOPE: `turan_water_sort`, Turan support in `water_sort_cli` (`stats`, new `compare`), and small
core additions only where they clearly belong in core (e.g. a tube-permutation helper). No
dataset `generate`, no Python, no web.

KEY REQUIREMENTS (details in PLAN.md)
- Turan: ID "turan", VERSION 1. `TuranStrategy { Scramble { steps, max_extra_steps }, Constrained }`;
  `variant()` like "scramble(steps=40)" / "constrained".
- Seed (D1): fresh_seed(now_nanos) = time_seed(now_nanos, COUNTER.fetch_add(1)) with a
  process-wide AtomicU64. Core never reads the clock; the CLI passes SystemTime::now(). An
  explicit seed always wins. RNG = ChaCha20Rng::seed_from_u64(seed), as in uniform.
- Scramble candidate (inside attempt_loop's closure, continuing ONE rng stream):
  1. start from State::solved(params);
  2. `steps` random reverse moves chosen with bounded_u32 over reverse_moves(state), excluding
     the exact undo of the previous reverse move;
  3. keep applying random reverse moves until the state is in the standard layout (exactly
     n_colors full tubes + n_empty empty tubes, any positions), at most max_extra_steps; if that
     fails, the attempt yields a candidate that evaluate() rejects or the closure signals a
     rejected attempt - pick the cleanest design that keeps attempts counted, record it in D14;
  4. permute tubes so the empty tubes are last, then shuffle color labels and the order of the
     full tubes with fisher_yates (a symmetry of the game, not a move);
  5. evaluate() validates as for uniform. Unsolvable must be impossible: debug_assert it.
- Constrained (second, optional): uniform fill + reject any tube with two vertically adjacent
  same-color units. Only after Scramble is complete and green.
- Determinism: same (params, seed, cfg, strategy) -> identical GeneratedPuzzle on Linux and
  Windows. Golden vectors: 20 seeds x 3 configs x each strategy in
  turan_water_sort/tests/golden/*.json (64-bit values as 16-digit hex strings; regenerate with
  WATER_SORT_BLESS=1, same convention as Phase 1/2).
- Tests: 10k seeds replay to solved in opt_moves (heavy-tests); every output is standard layout;
  no Unsolvable; two fresh_seed calls at the same now_nanos differ; Constrained never yields
  adjacent same-color units.
- CLI: `stats --generator turan --strategy scramble --steps N` (extend the existing GeneratorKind
  enum; reuse the stats machinery, D3 criterion and report format unchanged). New
  `compare --a uniform --b turan:scramble(steps=N) ...` writing reports/uniform_vs_turan.md:
  side-by-side Phase 2.3 table, histograms (opt_moves, color changes, segments, random stuck /
  capped rates, states_expanded), chi-square / KS two-sample tests documenting the difference,
  and canonical-hash overlap between equal-size samples.
- `steps` sweep (e.g. 10, 20, 40, 80, 160) on the supported configurations: mean/p50/p99
  opt_moves, rejection rate, extra-step distribution of the return-to-standard-layout step.
  Choose a default `steps` per configuration and record it (D14, status proposed).
- D3 is decided: only 1 or 2 empty tubes (MAX_SUPPORTED_EMPTY), 2 is the primary configuration.
  Run every sweep / comparison with `--empty 1..=2` (the stats default) over the SUPPORTED cells.
  Do not change D3 or the SUPPORTED table.

WORK METHOD
- One focused commit per task in PLAN.md (conventional commits: feat/test/chore/docs).
- After every task: cargo fmt, clippy -D warnings, cargo test --workspace --locked pass.
- Tick the checkboxes in docs/phases/phase-3-turan/PLAN.md and the roadmap Phase 3 items; update
  the status in docs/README.md when done.
- If the plan is wrong or infeasible (e.g. returning to the standard layout is too expensive),
  STOP and report with measurements before deviating. Record accepted deviations as D14+ in
  docs/decisions.md.
- Commit generated reports; put large raw outputs under data/ (gitignored).

DONE WHEN: Turan implements Generator with no rules outside core; same seed -> same puzzle
(golden vectors); all outputs solvable, standard layout, solution replays; stats + compare
reports committed; steps defaults proposed; CI green on ubuntu, windows and heavy-tests. Then
push the branch, open a PR to main summarizing what was built, the measured distribution
difference and any decisions recorded, and report back.
```
