# Phase 5 — Python binding and Gymnasium environment

## Goal

Python access to the exact Rust game: a PyO3 package, a Gymnasium environment for RL baselines and data collection, and a trajectory logger producing the JEPA training data.

## 5.1 Binding — `jepa_water_sort`

Layout:

```
python/
├── Cargo.toml              # cdylib, pyo3 (abi3-py310), numpy crate
├── pyproject.toml          # maturin build backend
├── src/lib.rs              # #[pymodule]
├── jepa_water_sort/
│   ├── __init__.py         # re-exports native module
│   ├── env.py              # WaterSortEnv
│   ├── vector.py           # native batched vector env
│   ├── logger.py           # TrajectoryLogger
│   └── policies.py         # random, epsilon-optimal, greedy (for data + baselines)
└── tests/
```

API (thin wrappers; no logic in Python):

| Python | Rust |
|---|---|
| `generate(generator: str, params: Params, seed: int \| None = None, config=None, strategy=None, layout="standard") -> Puzzle` | `Generator::generate` (`fresh_seed` when `seed` is `None`) |
| `solve(state, max_states=..., max_time=None) -> SolveResult` | `solver::solve` |
| `step(state, move) -> (State, units_moved)` | `moves::apply` (raises `ValueError` on an illegal move) |
| `legal_moves(state) -> list[tuple[int, int]]` | |
| `action_mask(state) -> np.ndarray[bool]` | |
| `stars(player_moves, opt, config=None) -> int` | raises `StarBelowOptimalError` |
| `canonical_hash(state) -> int` | |
| `State.to_numpy()`, `State.from_numpy()`, `State.puzzle_code`, `State.from_code()` | |
| `batch_step(states: np.ndarray, actions: np.ndarray) -> (np.ndarray, np.ndarray)` | releases the GIL; rayon |
| `batch_generate(generator, params, seeds \| count, config) -> list[Puzzle]` | releases the GIL; rayon |

- `State` and `Puzzle` are `#[pyclass(frozen)]` wrapping the Rust structs, picklable via the puzzle code.
- abi3 wheels (`abi3-py310`) mean one wheel per OS covers Python 3.10+.
- Build with `uv venv --python 3.12` + `maturin develop --release`.

## 5.2 `WaterSortEnv`

```python
WaterSortEnv(generator="uniform" | "turan", params=Params(6, 4, 2), min_opt=..., move_limit_k=4,
             dead_end_check=False, compute_solvability=False, shaping=False, gamma=0.99)
```

- `reset(seed=None, options=None)`: the puzzle seed is drawn from `self.np_random` (Gymnasium convention), so `reset(seed=s)` gives a reproducible sequence of puzzles for both generators. `options={"puzzle_seed": ...}` loads a specific generator seed, and `options={"puzzle_code": ...}` loads a fixed puzzle.
- Constructor takes `layout="standard"|"distributed"` for both generators (D14) and `strategy=` for Turan (default `scramble`, with the Phase 3 `steps` default for the configuration).
- **Observation:** `Box(0, 1, (n_tubes, capacity, n_colors + 1), int8)`, one-hot with channel `n_colors` = empty.
- **Action:** `Discrete(n_tubes²)`, index `from * n_tubes + to`. `info["action_mask"]` is provided per step, plus an `action_masks()` method for sb3-contrib `MaskablePPO`.
- **Illegal action:** state unchanged, reward −1, `info["illegal"] = True`, counts toward the move limit. No exception, since `check_env` and unmasked agents must not crash.
- **Termination:** solved → `terminated=True`. Dead end (no legal moves, or with `dead_end_check` the solver proves unsolvable) → `terminated=True`, `info["dead_end"]=True`. Move limit `k * opt_moves` (D4) → `truncated=True`.
- **Reward:** −1 per move. Optional potential-based shaping `γΦ(s') − Φ(s)` with `Φ = −color_changes(s)`. Nothing else, so the optimal policy is unchanged (Ng et al. 1999).
- **`info`:** `opt_moves`, `moves_so_far`, `seed`, `generator_variant`, `layout`, `puzzle_code`, `canonical_hash`, `action_mask`, optional `solvable`.
- **Vectorized:** works with `gymnasium.vector.SyncVectorEnv` / `AsyncVectorEnv`. Additionally `WaterSortVectorEnv(num_envs)` is backed by `batch_step` with autoreset, for throughput.

## 5.3 Trajectory logger

- Unit row: `(puzzle_id, source, step, state, action, next_state, done, illegal)`, where `puzzle_id` = canonical hash + generator id.
- Sources: `optimal` (solver solution), `random` (uniform over legal moves), `epsilon` (the optimal move from the current state with probability 1−ε, otherwise random legal, re-solving after a deviation), `human` (imported from the web export, Phase 6).
- Collection runs only on the puzzles of a given split, so trajectory splits inherit the puzzle split (no leakage).
- Output: Parquet shards (`pyarrow`), about 1M transitions per shard, with `state` / `next_state` stored as fixed-size uint8 arrays (compact, and one-hot is rebuilt at load). Optional `.npz` export for quick experiments.
- A shard manifest records source, ε, policy seed, dataset manifest hash, and tool version.

## Tasks

Done (2026-10-03). Deviations and details: D19 in [`decisions.md`](../../decisions.md) (episode rules moved to `water_sort_core::episode`, Turan env default `reverse_search` per D16, extra logger columns and a `greedy` source).


1. [x] PyO3 crate + maturin build, `State`/`Puzzle` classes, scalar functions
2. [x] Batch functions with `allow_threads`
3. [x] `WaterSortEnv` + `check_env`
4. [x] Native vector env
5. [x] Policies + logger + shard writer
6. [x] Rust↔Python parity test
7. [x] CI job: `ubuntu-latest` + `windows-latest`, `uv` → `maturin develop` → `pytest`

## Tests

- `gymnasium.utils.env_checker.check_env(WaterSortEnv(...))` passes.
- **Parity:** Rust writes golden trajectories (`water_sort_core/tests/golden/rollouts.json`: puzzle code, action sequence, expected states/rewards/flags); pytest replays them through `step` and through the env and asserts exact equality. The Phase 1 golden seed vectors are also checked from Python.
- `batch_step` matches looped `step` on 10k random transitions.
- Pickle round-trip of `State` / `Puzzle`.

## Acceptance

Roadmap gate: `check_env` passes; a Python rollout gives exactly the same result as Rust `step`. Plus: the Python CI job is green on both OSes.
