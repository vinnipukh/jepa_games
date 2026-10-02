# Decision log

Each entry has a status: **decided**, **proposed** (default the plans assume until confirmed), or **pending** (blocks something).

---

## D1 — Turan generator = true random generation — decided (2026-10-02)

The Turan generator uses the **same shuffle algorithm** as the uniform generator (Fisher-Yates + solver rejection). The only difference is the randomness source: uniform uses a seeded ChaCha20 PRNG, and Turan draws every random decision from a true/entropy source with no seed.

Consequences:

- **No `u64` seed exists** for Turan puzzles. Reproducibility comes from recording the random bytes consumed (the *entropy tape*) and replaying them. See D7.
- **Expected outcome is a null result.** A good CSPRNG cannot be told apart from true randomness by any feasible test. Fisher-Yates driven by either source samples the same distribution. The Phase 3 / Phase 7 comparisons are therefore *equivalence* tests: they should find no difference, and finding one would point to a bug (biased sampling, a broken entropy source, or a pipeline difference). The plans treat it this way. If the research question was meant to be "does the generator distribution matter", Turan needs a different distribution (for example, reverse-scramble from a solved state), and D1 should be revisited.
- Entropy source, in order of preference:
  1. OS entropy via `getrandom` (BCryptGenRandom/ProcessPrng on Windows, `getrandom(2)` on Linux, `crypto.getRandomValues` in the browser). Strictly speaking this is a CSPRNG continuously reseeded from hardware entropy, which is the standard meaning of "true random" in software.
  2. Optional `--source rdseed` (raw x86 hardware entropy). This needs `unsafe` intrinsics, so it is isolated in the `turan_water_sort` crate behind a feature flag and is not available on WASM.

## D2 — Uniform over which space? — decided (2026-10-02): labeled configurations

"Labeled" refers to the sample space: tube positions and color ids are treated as distinguishable. It involves no manual annotation; Fisher-Yates produces it directly.

Two candidate target distributions:

- **Labeled-uniform.** Every concrete arrangement (which tube holds what, which actual color is where) is equally likely. Plain Fisher-Yates gives this with no extra work.
- **Canonical-uniform.** Every *structurally distinct* puzzle (up to tube reordering and color renaming) is equally likely.

The two differ only through class sizes. A canonical class contains `(n_tubes! · n_colors!) / |Aut|` labeled arrangements (counting only the arrangements that fit the fixed layout with empty tubes last), where `|Aut|` is the number of symmetries that leave the puzzle unchanged, for example two identical tubes. Labeled-uniform therefore under-samples highly symmetric puzzles by a factor of `|Aut|`. For more than about 4 colors almost every puzzle has `|Aut| = 1`, so the two distributions are practically identical. The gap matters only for tiny configurations.

Decision: labeled-uniform as the target, plus a measurement in Phase 2.3 of the fraction of generated puzzles with `|Aut| > 1` for each configuration. If that fraction is non-negligible in the supported range, add an optional `--canonical-uniform` mode that accepts a sample with probability `1/|Aut|`.

## D3 — Supported configuration range — pending (Phase 2.3)

Set from measurements. Proposed criterion: solver p99 < 1 s and state-limit hit rate < 0.1 % in release mode on the CI runner.

## D4 — Move limit `k · opt_moves` — proposed: k = 4

Generous enough that a random-ish policy is not truncated before it has a chance. To be revisited after the RL baseline.

## D5 — Star coefficients — proposed: 0.10 / 0.25 / 0.50, stored as per-mille integers (100 / 250 / 500)

Integers are used because floating point gives wrong thresholds: `0.10 * 30.0 = 3.0000000000000004`, so `ceil` returns 4 instead of 3. Thresholds are computed with integer arithmetic: `ceil(c · opt / 1000) = (c · opt + 999) / 1000`.

## D6 — CI = GitHub Actions — decided (2026-10-02)

Matrix: `ubuntu-latest` + `windows-latest`. Separate jobs for wasm32 and Python as those phases arrive.

## D7 — `Generator` trait amended — proposed

The roadmap's `generate(&self, params, seed: u64)` cannot express a seedless true-random generator. Amended form:

```rust
pub enum Provenance {
    Seed(u64),                         // uniform: ChaCha20Rng::seed_from_u64
    EntropyTape { tape: Vec<u8> },     // turan: bytes consumed by the accepted attempt
}

pub trait Generator {
    const ID: &'static str;
    const VERSION: u32;
    fn generate(&self, params: &Params, cfg: &GenConfig) -> Result<GeneratedPuzzle, GenError>;
    fn replay(&self, params: &Params, provenance: &Provenance) -> Result<State, GenError>;
}
```

`GeneratedPuzzle.seed: u64` becomes `provenance: Provenance`. Uniform keeps a convenience `generate_from_seed(params, seed)`. Separately, every puzzle gets a **puzzle code**, a compact base32 encoding of params + initial state. It works for both generators and is what the web "open puzzle" box accepts.

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

`rand`'s `shuffle` / `gen_range` algorithms can change between crate versions and can depend on `usize` width (64-bit native vs 32-bit wasm32). The core crate implements its own `bounded_u32` (Lemire's method with rejection on `next_u32`) and Fisher-Yates. Both generators use this exact code, so only the RNG source differs (D1).

## D11 — Generation never uses wall-clock limits — proposed

Solver timeouts during generation are by **expanded-state count only**. A time limit would make accept/reject depend on machine speed and break reproducibility from a seed. Time limits remain available for interactive use (web, CLI `solve`).
