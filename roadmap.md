# jepa_games — Roadmap

This document is the development plan for the `jepa_games` project. The project consists of a Water Sort game with two different puzzle generation approaches, plus the infrastructure needed to train a JEPA-based world model on it. Phases should be implemented in order; do not move to the next phase until the current phase's acceptance criteria are met.

## Core principles

Both versions (`uniform_water_sort` and `turan_water_sort`) take the game rules, solver, star metric and canonical hash from the shared `water_sort_core` crate. The only difference between the versions is the puzzle generator. This guarantees that when models trained on the two versions are compared, any difference comes from the generator alone.

The web game humans play and the environment the model trains in run the same Rust code (WASM for the web, PyO3 for Python). Game rules are never written a second time anywhere.

Every puzzle must be exactly reproducible from a seed. The seed is drawn from OS entropy and recorded, then generation runs on a deterministic PRNG (ChaCha20 or PCG64).

## Folder structure

```
jepa_games/
├── Cargo.toml                  # workspace
├── roadmap.md
├── water_sort_core/            # state, move rules, solver, star metric, canonical hash, Generator trait
├── uniform_water_sort/         # Fisher-Yates + rejection sampling generator
├── turan_water_sort/           # Turan generator: time-seeded, strategy-based (reverse scramble), see Phase 3
├── docs/                       # per-phase plans + decision log
├── water_sort_cli/             # batch generation, validation, statistics commands
├── python/                     # PyO3 binding (maturin) + Gymnasium env + logger
└── web/                        # WASM UI
```

---

## Phase 0 — Workspace setup

- [x] Create the Cargo workspace and empty crates
- [x] Dependencies: `rand`, `rand_chacha`, `getrandom`, `rayon`, `serde`, `serde_json` (current majors: `rand_core`/`rand_chacha` 0.10, `getrandom` 0.4; see `Cargo.lock`)
- [x] CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` (GitHub Actions, Ubuntu + Windows)
- [x] `#![forbid(unsafe_code)]` in `water_sort_core` (also forbidden workspace-wide)

**Acceptance criterion:** `cargo test --workspace` passes with empty tests, CI is green.

> **Status: done (2026-10-02).** Private repo `vinnipukh/jepa_games`, toolchain pinned to 1.99.0. Plan: [docs/phase-0-workspace.md](docs/phase-0-workspace.md).

Detailed plans for every phase live in [docs/](docs/README.md). Decisions made after this roadmap was written are recorded in [docs/decisions.md](docs/decisions.md) (D1–D11) and override the text below where they differ.

---

## Phase 1 — `water_sort_core`

### 1.1 State representation

- [ ] Parameters: `n_colors`, `capacity` (default 4), `n_empty` (default 2). Number of tubes = `n_colors + n_empty`.
- [ ] Tube contents stored bottom to top in a fixed-size array; colors as `u8`, with a separate value for an empty cell.
- [ ] Solved state definition: every non-empty tube is full and single-colored.

### 1.2 Move rules

- [ ] A move is a single pour (`from`, `to`).
- [ ] Validity: `from` is not empty, `from != to`, `to` is empty or the top color of `to` matches the top color of `from`, and `to` has at least one free slot.
- [ ] Transfer: all contiguous same-colored units at the top of `from` are moved, as many as fit in `to`.
- [ ] Functions `legal_moves(state)` and `action_mask(state) -> [bool; n_tubes * n_tubes]`.
- [ ] Redundant move filter (inside the solver only, not in the game): pouring an entire single-colored tube into an empty tube.

### 1.3 Canonical hash

- [ ] Tube order symmetry: sort the tubes before hashing.
- [ ] Color permutation symmetry: relabel colors by order of first appearance (over the sorted tubes).
- [ ] Two separate functions: `canonical_tubes(state)` (tube order only) and `canonical_full(state)` (tube order + color). The solver uses the latter, and so does dataset duplicate detection.
- **Decision D8:** sort-then-relabel is not an exact canonical form (relabeling changes the sort order). `canonical_full` is the exact minimum over color relabelings and is used for dedup and splits. The solver uses a cheaper approximate `solver_key`. The hash is xxh3-64 over a fixed encoding.

### 1.4 Solver

- [ ] BFS (optimal; for small puzzles and as a test reference).
- [ ] A* or IDA* with an admissible heuristic. Heuristic: the number of adjacent color changes within tubes. A legal pour never creates a new color change and removes at most one, so this value never exceeds the remaining optimal distance.
- [ ] Output: `Solvable { opt_moves, solution: Vec<Move>, states_expanded }`, `Unsolvable { states_expanded }`, or `Timeout`.
- [ ] Time and state-count limits are configurable.
- **Decision D9:** heuristic changed to `segments − n_colors`. It is still admissible and never weaker than the color-change count.
- **Decision D11:** generation uses the state-count limit only. A wall-clock limit would make accept/reject depend on machine speed.
- [ ] Test: on 10,000 random small puzzles, A*/IDA* must return the same `opt_moves` as BFS.

### 1.5 Star metric

- [ ] `stars(player_moves, opt) -> u8`, with `extra = player_moves - opt`:
  - 5★: `extra == 0`
  - 4★: `extra <= t4`, `t4 = max(1, ceil(0.10 * opt))`
  - 3★: `extra <= t3`, `t3 = max(t4 + 1, ceil(0.25 * opt))`
  - 2★: `extra <= t2`, `t2 = max(t3 + 1, ceil(0.50 * opt))`
  - 1★: solved but exceeded `t2`
- [ ] Coefficients (0.10 / 0.25 / 0.50) are read from config.
- **Decision D5:** coefficients are stored as per-mille integers (100 / 250 / 500) and thresholds use integer `ceil`. In floating point, `0.10 * 30 = 3.0000000000000004`, which gives 4 instead of 3.
- [ ] The case `player_moves < opt` returns a separate error/warning rather than panicking (it indicates a solver bug).
- [ ] The counted value is the total number of pours made on that puzzle. Undo does not decrement the counter, restart does not reset it.

### 1.6 Generator trait

```rust
pub trait Generator {
    const ID: &'static str;      // "uniform", "turan"
    const VERSION: u32;          // bump when the generator algorithm changes
    fn generate(&self, params: &Params, seed: u64) -> GeneratedPuzzle;
}

pub struct GeneratedPuzzle {
    pub state: State,
    pub seed: u64,
    pub generator_id: &'static str,
    pub generator_version: u32,
    pub opt_moves: u32,
    pub solution: Vec<Move>,
    pub metrics: DifficultyMetrics,
    pub canonical_hash: u64,
    pub attempts: u32,           // how many attempts rejection sampling needed
}
```

**Decision D7:** the trait keeps `seed: u64` and adds `variant()` (sub-type, e.g. `scramble(steps=40)`), `fresh_seed(now_nanos)`, a `GenConfig` argument and a `Result` return. `GeneratedPuzzle` also gets `generator_variant` and a `puzzle_code` (compact encoding of the initial state). **Decision D10:** core provides its own `bounded_u32` + Fisher-Yates so results are bit-identical on native, wasm32 and Python. A **reverse move** (un-pour) is added to the rules for the Turan scramble.

### 1.7 Difficulty metrics

- [ ] `opt_moves`
- [ ] `states_expanded` (number of states the solver explored)
- [ ] Number of color changes in the initial state
- [ ] Random policy failure rate: out of N random (legal moves only) rollouts, how many hit a dead end
- [ ] Dead-end ratio: what fraction of the states reachable 1 and 2 moves from the start are unsolvable

**Acceptance criterion:** Unit tests for rules, solver and the star function; property-based tests (`proptest`) for "unit counts are preserved after every legal move", "applying the solution sequence reaches a solved state", and "the canonical hash is invariant under tube permutation and color permutation".

---

## Phase 2 — `uniform_water_sort`

### 2.1 Generation

- [ ] Seed: 64 bits from OS entropy via `getrandom` (or an externally supplied seed).
- [ ] PRNG: `ChaCha20Rng::seed_from_u64(seed)`.
- [ ] Shuffle the multiset of `n_colors * capacity` units with Fisher-Yates, fill the first `n_colors` tubes, leave the remaining `n_empty` tubes empty.
- [ ] Validate with the solver. If unsolvable or timed out, regenerate by continuing from the same PRNG stream.
- [ ] Filters: discard already-solved states, discard states with `opt_moves < min_opt` (min_opt from config).

### 2.2 Uniformity validation

- [ ] For a small configuration (e.g. 2 colors, capacity 2, 1 empty tube), enumerate all solvable states that pass the filters.
- [ ] Draw enough samples from the generator and run a chi-square test. The test runs in CI.
- [ ] Note: Fisher-Yates is uniform over labeled configurations (where tube order matters). It is not uniform over canonical classes, because the classes have different sizes. **Decided (D2): labeled-uniform is the target.**

### 2.3 Measurements

- [ ] For each (`n_colors`, `capacity`, `n_empty`) combination: rejection rate, mean number of attempts, `opt_moves` distribution, solver time.
- [ ] Reported via the `water_sort_cli stats` command.
- [ ] Configurations where the solver cannot find the optimal solution in reasonable time are excluded from the supported range.

**Acceptance criterion:** The uniformity test passes; replaying the solution of every generated puzzle reaches a solved state; the same seed always produces the same puzzle.

---

## Phase 3 — `turan_water_sort`

> **Decided (D1, 2026-10-02).** Turan is a time-seeded, strategy-based generator. The goal is variation in generation type at minimal code cost.
> - **Seed:** `splitmix64(now_nanos ^ splitmix64(counter))`. The atomic counter prevents identical seeds within one clock tick. The seed is recorded, so every puzzle is reproducible.
> - **RNG:** `ChaCha20Rng`, the same as uniform.
> - **Default strategy `Scramble { steps }`:** random reverse pours from a solved state, then a return to the standard layout (full tubes + empty tubes). Solvable by construction, with a different distribution from uniform over the same set of puzzles.
> - **Optional strategy `Constrained`:** Fisher-Yates that rejects vertically adjacent same-color units.
>
> Plan: [docs/phase-3-turan.md](docs/phase-3-turan.md).

Whatever the definition turns out to be, the following conditions apply:

- [ ] Implements the `Generator` trait and contains no game rules outside `water_sort_core`.
- [ ] Every generated puzzle is validated by the same solver, and `opt_moves` is computed the same way.
- [ ] Deterministic for a given seed.
- [ ] The measurements in Phase 2.3 are also produced for this generator and reported side by side with uniform.
- [ ] The distribution difference from uniform is measured: `opt_moves` histogram, color-change histogram, random policy failure rate.

---

## Phase 4 — Dataset and storage

- [ ] Puzzle record format: all `GeneratedPuzzle` fields + `params` + creation time. JSONL (for debugging) and Parquet (for bulk data).
- [ ] `water_sort_cli generate --generator uniform --count N --params ... --out ...` command, parallelized with `rayon`.
- [ ] Duplicate detection via the `canonical_full` hash.
- [ ] Train/val/test split is done on the canonical hash (`hash % 100` ranges), so color or tube permutations of the same puzzle never land in different sets.
- [ ] Cross-generator leakage check: are there shared canonical hashes between the uniform test set and the turan train set?

**Acceptance criterion:** Generating 1 million puzzles, the duplicate report and the split files are produced with a single command.

---

## Phase 5 — Python binding and Gymnasium environment

### 5.1 Binding

- [ ] `jepa_water_sort` Python package built with `maturin` + PyO3.
- [ ] Exposed API: `generate(generator, params, seed)`, `solve(state)`, `step(state, move)`, `legal_moves`, `action_mask`, `stars`, `canonical_hash`.
- [ ] Batch functions (batch step, batch generate) release the GIL.

### 5.2 Gymnasium env

- [ ] `WaterSortEnv(generator="uniform" | "turan", params=...)`, `reset(seed=None)`.
- [ ] Observation: `(n_tubes, capacity, n_colors + 1)` one-hot tensor.
- [ ] Action: `Discrete(n_tubes * n_tubes)`, invalid moves masked via `info["action_mask"]`.
- [ ] `info` contains: `opt_moves`, `moves_so_far`, `seed`, `canonical_hash`, solvability status (optional, expensive).
- [ ] Episode end: solved, dead end reached (optional check), or move limit (`k * opt_moves`).
- [ ] Reward (for the RL baseline only; JEPA does not use reward): −1 per move, optional potential-based shaping `γΦ(s') − Φ(s)` with Φ = −(number of color changes). Non-potential shaping is not used.
- [ ] Vectorized env support.

### 5.3 Trajectory logger

- [ ] Recorded unit: `(state, action, next_state, done, puzzle_id, step)`.
- [ ] Trajectory sources are labeled separately: `optimal` (solver solution), `random` (legal random), `epsilon` (optimal + ε random deviation), `human` (from the web).
- [ ] Output: sharded `.npz` or Parquet.

**Acceptance criterion:** `gymnasium.utils.env_checker.check_env` passes; a rollout made from Python produces exactly the same result as `step` in Rust.

---

## Phase 6 — Web UI (WASM)

- [ ] `water_sort_core` + both generators via `wasm-bindgen`.
- [ ] Screen: tubes, move counter, undo, restart, new puzzle, generator selector, seed display and opening a puzzle from a seed.
- [ ] Completion screen: player moves, `opt_moves`, star count.
- [ ] Undo/restart rules as in Phase 1.5.
- [ ] Optional: export player trajectories in the Phase 5.3 format (`human` source).

**Acceptance criterion:** The same seed opens the same puzzle on the web and in Python.

---

## Phase 7 — JEPA preparation (Python, PyTorch)

This phase is outside the Rust side; only the requirements on the data interface and evaluation are listed here.

### 7.1 Model components

- [ ] Encoder `E(s) -> z`
- [ ] Action-conditioned predictor `P(z_t, a_t) -> ẑ_{t+1}`, target: EMA or stop-gradient target encoder.
- [ ] Inverse dynamics head `IDM(z_t, z_{t+1}) -> a_t` (a regularizer against latent collapse; cheap because the action space is small and discrete).
- [ ] State probe head: predicts tube contents from the latent. Used both for collapse monitoring and as an optional auxiliary loss.

### 7.2 Collapse monitoring

- [ ] Batch embedding variance and effective rank, logged every epoch.
- [ ] Probe accuracy, logged every epoch.

### 7.3 Planning

- [ ] Goal: the latent of the solved state. Water sort has multiple solved states (which tube holds which color), so the goal is defined either by the canonical solved state or by a "solved" classifier head.
- [ ] Beam search or MCTS over latents for the discrete action space; CEM as an alternative.

### 7.4 Baselines

- [ ] Random legal policy
- [ ] Greedy (the move that reduces color changes the most)
- [ ] DQN/DDQN with action mask + the Phase 5.2 reward
- [ ] Solver (upper bound)

### 7.5 Evaluation

- [ ] Solve rate
- [ ] Mean stars (the Phase 1.5 function, same as for human players)
- [ ] Mean `moves / opt_moves`
- [ ] All reported broken down by `opt_moves` range and by generator
- [ ] Cross-evaluation: train on uniform and test on turan, train on turan and test on uniform

---

## Open decisions

Full log with reasoning: [docs/decisions.md](docs/decisions.md).

1. **Definition of the Turan generator.** ✅ Decided (D1): time-seeded, strategy-based; default reverse scramble from a solved state.
2. **Uniform over which space?** ✅ Decided (D2): labeled configurations. Phase 2.3 measures the fraction of symmetric puzzles; canonical correction only if that fraction matters.
3. **Supported configuration range.** ⏳ Pending, from Phase 2.3 measurements. Proposed criterion: solver p99 < 1 s and timeout rate < 0.1 %.
4. **Move limit.** Proposed (D4): `k = 4`, revisit after the RL baseline.
5. **Star coefficients.** Proposed (D5): 0.10 / 0.25 / 0.50 as per-mille integers; to be updated as human player data comes in.
6. **CI.** ✅ Decided (D6): GitHub Actions, Ubuntu + Windows.

## References

- Ito et al., *Sorting Balls and Water: Equivalence and Computational Complexity*, FUN 2022 (arXiv:2202.09495). Shows water sort is NP-complete and that every solvable instance has a polynomial-length solution.
- Existing Rust solvers (for reference): `AmrSaber/color-sort-puzzle-solver`, `pkositsyn/water-sort-puzzle-solver`, `Matheritasiv/water_puzzle`.
- DQN/DDQN work on water sort (IEEE, 2024): optimal on small configurations, insufficient on large ones; useful for baseline comparison.
- Ng, Harada, Russell, *Policy Invariance Under Reward Transformations*, ICML 1999. Potential-based shaping.