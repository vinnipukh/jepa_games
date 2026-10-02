# Phase 1 — `water_sort_core`

## Goal

The single implementation of the game: state, rules, canonical hashing, an optimal solver, the star metric, shared sampling primitives, the `Generator` trait, and difficulty metrics. Everything later depends on this crate being correct, so it carries the heaviest test load.

## Module layout

```
water_sort_core/src/
├── lib.rs          # re-exports, #![forbid(unsafe_code)]
├── params.rs       # Params, validation
├── state.rs        # State, EMPTY, is_solved, puzzle code encode/decode
├── moves.rs        # Move, is_legal, apply, legal_moves, action_mask
├── canon.rs        # canonical_tubes, solver_key, canonical_full, canonical_hash
├── solver/
│   ├── mod.rs      # SolveResult, SolverLimits, solve()
│   ├── bfs.rs
│   └── astar.rs
├── stars.rs        # StarConfig, stars()
├── sampling.rs     # bounded_u32, fisher_yates, RecordingRng, TapeRng
├── generator.rs    # Generator trait, GeneratedPuzzle, Provenance, GenConfig, GenError
└── metrics.rs      # DifficultyMetrics, compute_metrics()
```

## 1.1 State representation

```rust
pub const MAX_TUBES: usize = 16;
pub const MAX_CAP: usize = 8;
pub const EMPTY: u8 = u8::MAX;

pub struct Params { pub n_colors: u8, pub capacity: u8, pub n_empty: u8 } // defaults: capacity 4, n_empty 2

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct State {
    params: Params,
    cells: [[u8; MAX_CAP]; MAX_TUBES], // bottom → top, EMPTY above the fill height
    heights: [u8; MAX_TUBES],          // cached fill height per tube
}
```

- Fixed arrays, no heap. `State` is `Copy`-sized (about 150 bytes), cheap to clone in the solver, and has the same layout on every target.
- One concrete type for all parameter sets, with runtime checks against `MAX_*`. This avoids const generics leaking into the PyO3/WASM bindings.
- `Params::validate`: `n_colors ≥ 1`, `capacity ≥ 2`, `n_colors + n_empty ≤ MAX_TUBES`, `capacity ≤ MAX_CAP`.
- `is_solved`: every tube is either empty or full and single-colored.
- **Puzzle code**: params + cells packed (`ceil(log2(n_colors + 1))` bits per cell), base32 (Crockford) encoded. Used by the web UI and logs (D7).

## 1.2 Move rules

```rust
pub struct Move { pub from: u8, pub to: u8 }
```

- `is_legal(s, m)`: `from != to`, `from` non-empty, `to` not full, and `to` either empty or with the same top color as `from`.
- `apply(s, m) -> (State, u8 /*units moved*/)`: moves `min(top_run_len(from), free(to))` units.
- `legal_moves(s) -> impl Iterator<Item = Move>` in a fixed `(from, to)` order. Determinism matters for random rollouts.
- `action_mask(s) -> Vec<bool>` of length `n_tubes²`, with index `from * n_tubes + to`. Returned as a `Vec` because `n_tubes` is a runtime value; the binding layers convert it.
- Solver-only pruning (never in the game):
  - Pouring a full single-color tube anywhere is useless. Pouring an entire single-color tube into an empty tube is the roadmap's redundant move.
  - When there are several empty tubes, only pour into the lowest-indexed one. They are interchangeable.

## 1.3 Canonical hash

See D8 for why the roadmap's two-step procedure is not exact.

- `canonical_tubes(s)`: tubes sorted lexicographically by `(cells, EMPTY-padded)`. Tube-order symmetry only.
- `solver_key(s)`: sort → relabel by first appearance → sort again. Fast and sound, but not exact.
- `canonical_full(s)`: exact. Find the color relabeling σ that minimizes `sort(σ(tubes))` in lexicographic order:
  1. Compute a permutation-invariant signature per color: the sorted multiset of `(tube-content signature, depth)` positions it occupies. Colors with unique signatures are fixed by signature order.
  2. Backtrack only over groups of colors with tied signatures, pruning any partial assignment whose partial encoding is already larger than the best found.
  3. In typical puzzles almost all signatures are unique, so this costs little more than a sort.
- `canonical_hash(s) = xxh3_64(encode(canonical_full(s)))`. The encoding includes params, so hashes from different configurations never collide by construction.

## 1.4 Solver

```rust
pub struct SolverLimits { pub max_states: u64, pub max_time: Option<Duration> } // max_time: interactive only (D11)
pub enum SolveResult {
    Solvable { opt_moves: u32, solution: Vec<Move>, states_expanded: u64 },
    Unsolvable { states_expanded: u64 },
    Timeout { states_expanded: u64 },
}
```

- **BFS**: reference implementation for small puzzles and tests. Closed set keyed by `solver_key`.
- **A\***: heuristic `h = segments − n_colors` (D9; admissible and consistent, so the first time the goal is popped it is optimal). Open list is a binary heap ordered by `(f, −g, insertion counter)`. The counter makes tie-breaking deterministic. The closed set is a `HashMap<Key, (g, parent_key, move)>` for path reconstruction, using a fixed-seed hasher (FxHash) so iteration order is deterministic.
- An exhausted search proves `Unsolvable`, because the state space is finite.
- Solutions are found on canonical keys but must be **replayed on the original labeled state**. The parent chain stores real moves taken from real (non-canonicalized) states.
- IDA\* only if A\* memory becomes the bottleneck in Phase 2.3.

## 1.5 Star metric

```rust
pub struct StarConfig { pub c4_permille: u32, pub c3_permille: u32, pub c2_permille: u32 } // 100/250/500
pub enum StarError { BelowOptimal { player: u32, opt: u32 } }
pub fn stars(player_moves: u32, opt: u32, cfg: &StarConfig) -> Result<u8, StarError>;
```

- Integer `ceil` (D5): `ceil_pm(c, opt) = (c * opt + 999) / 1000`.
- `t4 = max(1, ceil_pm(c4, opt))`, `t3 = max(t4 + 1, ceil_pm(c3, opt))`, `t2 = max(t3 + 1, ceil_pm(c2, opt))`.
- `player_moves < opt` → `Err(BelowOptimal)`, never a panic.
- Move counting (total pours; undo does not decrement, restart does not reset) is enforced by a `Session` type in core that the web and Python both use, so the rule is not re-implemented.

## 1.6 Generator trait

As amended in D7: `generate(params, cfg)`, `replay(params, provenance)`, a `Provenance` enum, and `GeneratedPuzzle` with `provenance` and `puzzle_code` fields in place of `seed`. `GenConfig` holds `min_opt`, `max_attempts`, and `SolverLimits`.

## 1.6b Sampling primitives (D10)

- `bounded_u32(rng: &mut impl RngCore, n: u32) -> u32`: Lemire's nearly-divisionless method using only `next_u32`, with no `usize`.
- `fisher_yates(rng, &mut [u8])`: the classic downward loop, `j = bounded_u32(rng, i + 1)`.
- `RecordingRng<R>`: wraps any `RngCore` and records every byte it hands out (for Turan).
- `TapeRng`: replays a recorded tape and returns an error if exhausted.

## 1.7 Difficulty metrics

```rust
pub struct DifficultyMetrics {
    pub opt_moves: u32,
    pub states_expanded: u64,
    pub color_changes: u32,
    pub segments: u32,
    pub random_rollouts: u32,
    pub random_stuck_rate: f32,      // reached a state with no legal moves
    pub random_capped_rate: f32,     // hit step cap (k · opt) without solving or getting stuck
    pub dead_end_ratio_d1: Option<f32>,
    pub dead_end_ratio_d2: Option<f32>,
}
```

- Random rollouts use a ChaCha20 RNG seeded from `canonical_hash`, so the metric is deterministic for both generators.
- "Dead end" in rollouts means no legal moves. Unsolvable states that still have moves cannot be detected cheaply, so they show up as `capped`.
- Dead-end ratios: deduplicate the depth-1 and depth-2 states by `solver_key`, solve each one, and report the fraction unsolvable. It is expensive (dozens of solves), so it is **opt-in** via `GenConfig`.

## Tasks (in order)

1. [ ] `params.rs`, `state.rs` + unit tests (solved detection, validation)
2. [ ] `moves.rs` + unit tests (every legality branch, partial pour, pour onto empty)
3. [ ] `stars.rs` + table tests (including `opt = 30`, `opt = 1`, below-optimal), `Session`
4. [ ] `canon.rs` + brute-force reference for `n_colors ≤ 6` (min over all `n!` relabelings)
5. [ ] `solver/bfs.rs`
6. [ ] `solver/astar.rs` + BFS-equivalence test
7. [ ] `sampling.rs` + golden vectors (fixed seed → fixed shuffle output)
8. [ ] `generator.rs` types
9. [ ] `metrics.rs`
10. [ ] Puzzle code encode/decode
11. [ ] Criterion benches for `apply`, `canonical_full`, and solve on reference puzzles

## Tests

- **Unit:** rules, solver on hand-made puzzles (including known unsolvable ones), stars table.
- **Proptest:**
  - unit counts per color are preserved after every legal move;
  - applying the returned solution reaches a solved state in exactly `opt_moves` moves;
  - `canonical_full` (and its hash) is invariant under random tube permutation × random color permutation;
  - `canonical_full` matches the brute-force minimum for `n_colors ≤ 6`;
  - `h(s) ≤ true distance` for random states (checked with BFS);
  - puzzle code round-trips.
- **Equivalence:** 10,000 random small puzzles (`n_colors` 2–4, capacity 3–4, `n_empty` 1–2), with A\* `opt_moves` equal to BFS. Marked `#[ignore]` and run in the release-mode heavy-tests CI job.
- **Golden vectors:** `tests/golden/*.json` with seed → shuffled cells, state → canonical hash, and state → solution. Reused by the WASM and Python test suites.

## Acceptance

The roadmap gate (unit tests, the three named proptests), plus: A\*≡BFS on 10k puzzles, `canonical_full` ≡ brute force, golden vectors committed.

## Risks / open points

- Exact canonicalization in the worst case (many tied colors) is exponential. It is bounded in practice. If it shows up in benchmarks, cap the backtracking and fall back to the brute-force orbit for small `n`.
- A\* memory at large `n_colors` decides the supported range (D3).
