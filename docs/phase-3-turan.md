# Phase 3 — `turan_water_sort`

## Goal

A generator identical to uniform in algorithm, but driven by true / OS entropy with no seed (D1). Reproducibility comes from recording the entropy consumed. The phase ends with a side-by-side statistical comparison with uniform.

## What "the only difference is the generator" means here

Both generators call the same `core::sampling::fisher_yates`, the same fill layout, the same solver with the same `GenConfig`, and the same filters. The single variable is the `RngCore` passed in:

| | uniform | turan |
|---|---|---|
| RNG | `ChaCha20Rng::seed_from_u64(seed)` | `RecordingRng<EntropyRng>` |
| provenance | `Seed(u64)` | `EntropyTape { tape }` |
| reproducible from | seed (+ `GenConfig`) | tape, or puzzle code |

## Design

```rust
pub struct Turan { pub source: EntropySource }
pub enum EntropySource { Os, #[cfg(feature = "rdseed")] Rdseed }
impl Generator for Turan { const ID: &str = "turan"; const VERSION: u32 = 1; ... }
```

- **`EntropyRng`**: implements `RngCore` by reading from `getrandom` into a 4 KiB buffer and refilling it when empty. Buffering keeps the syscall count low. Each byte is still fresh OS entropy, not expanded from a seed. `getrandom` failure → `GenError::Entropy`, never a panic.
- **Tape recording**: for each attempt, wrap the source in a fresh `RecordingRng`. Only the **accepted attempt's** tape is stored (about `4 × n_colors × capacity` bytes plus rejection-sampling extras; under 1 KiB for supported configs). The tapes of rejected attempts are dropped, and `attempts` still counts them.
- **Replay**: `replay(params, EntropyTape(t))` runs `fisher_yates(TapeRng::new(t))` once and fills. No solver is needed, unlike uniform replay, because only the accepted attempt is on the tape. A replay test asserts the result equals the stored state.
- **Optional `rdseed` feature**: x86_64 only. Uses `core::arch::x86_64::_rdseed64_step` with retry on carry-flag failure. It needs `unsafe`, so the lint is relaxed only in that module, with a safety comment. It is off by default and unavailable on WASM.
- **WASM**: `getrandom` with the `wasm_js` backend maps to `crypto.getRandomValues`. Same code path.

## Comparison with uniform (equivalence study)

Because a CSPRNG and true entropy should give the same distribution, the hypothesis being tested is **no difference**. A plain significance test cannot confirm that, since failing to reject is not evidence of equality. The analysis therefore uses:

1. **Exact small-config test**: on the 3-color/cap-3/1-empty set `A` from Phase 2.2, a chi-square goodness-of-fit for Turan against uniform-over-`A`, the same test uniform passes.
2. **Two-sample tests on supported configs** (100k puzzles each): chi-square on the `opt_moves` histogram and the color-change histogram, and a Kolmogorov–Smirnov test on `states_expanded`.
3. **Equivalence (TOST)** on the means of `opt_moves`, `color_changes`, and `random_stuck_rate`, with margins set before running (proposed: ±1 % of the uniform mean).
4. **Entropy-source health**: run the recorded tapes through a basic battery (byte frequency, runs, serial correlation). A failure here explains any difference found in steps 1–3.

Output: `water_sort_cli compare --a uniform --b turan ...` writes `reports/uniform_vs_turan.md`, with the Phase 2.3 measurement table side by side plus the test results above.

**Interpretation:** a significant difference means a bug, either in the entropy source or in a pipeline difference between the two generators. It does not mean a property of "true randomness". This framing carries into Phase 7.5, where the train-uniform/test-turan cross-evaluation is expected to show no gap and acts as a control for the evaluation pipeline.

## Tasks

1. [ ] `EntropyRng` (buffered `getrandom`) + error mapping
2. [ ] `Turan` generator using `RecordingRng`, storing the accepted tape
3. [ ] `replay` + round-trip test (generate → replay → identical state)
4. [ ] `stats` support for `--generator turan`
5. [ ] `compare` subcommand + report
6. [ ] (optional) `rdseed` feature

## Tests

- No game logic outside `water_sort_core`: grep-based CI check that the crate only depends on core's public API, plus code review.
- For 10k generated puzzles: the solution replays to solved; `replay(tape)` gives the stored state.
- Different runs produce different puzzles (sanity check: 1,000 generations show no duplicate tape).
- Exact small-config chi-square passes (`#[ignore]`, heavy job).

## Acceptance

Roadmap conditions: implements `Generator`, no rules outside core, same solver and `opt_moves`, deterministic given its provenance (the roadmap's "deterministic for a given seed" is reinterpreted as "deterministic given its tape", D7), measurements side by side with uniform, distribution difference measured. Plus: the comparison report is committed.

## Risks / open points

- If the intended research question was "does the puzzle distribution affect the learned world model", this design cannot answer it (see D1). Revisit before Phase 7 at the latest.
