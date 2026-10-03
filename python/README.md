# jepa_water_sort

Python access to the Rust Water Sort core (PyO3, abi3 wheels for Python ≥ 3.10), a Gymnasium
environment, a native vector environment and the trajectory logger for JEPA training data.
Every game rule runs in Rust (`water_sort_core`); Python results are bit-identical to Rust ones,
which `tests/test_golden.py` checks against the committed golden files.

```bash
cd python
uv venv --python 3.12
uv pip install maturin -r pyproject.toml --extra test
uv run --no-project maturin develop --release --locked
uv run --no-project python -m pytest
```

## Binding

```python
import jepa_water_sort as w

p = w.generate("uniform", w.Params(6, 4, 2), seed=1)        # or "turan", strategy=..., layout=...
p.state, p.opt_moves, p.solution, p.tier, p.puzzle_code, p.canonical_hash
nxt, units = w.step(p.state, (0, 6))                         # ValueError if illegal
w.legal_moves(p.state); w.action_mask(p.state); w.solve(p.state)
w.stars(21, p.opt_moves)                                     # StarBelowOptimalError below opt
w.batch_generate("uniform", (6, 4, 2), seeds=range(1000))    # GIL released, rayon
w.batch_step(states, actions)                                # (B, n_tubes, capacity) uint8
```

Turan strategies are strings: `reverse_search` (default), `scramble`, `pour_walk`
(distributed only), `constrained`, optionally with arguments as in the recorded variant, e.g.
`"scramble(steps=40)"`.

## Environment

```python
from jepa_water_sort import WaterSortEnv, WaterSortVectorEnv

env = WaterSortEnv("turan", params=(6, 4, 2), layout="standard", move_limit_k=4, shaping=False)
obs, info = env.reset(seed=0)            # or options={"puzzle_seed": s} / {"puzzle_code": c}
obs, reward, terminated, truncated, info = env.step(action)   # info["action_mask"], ["illegal"]
envs = WaterSortVectorEnv(256, generator="uniform")           # same episodes as SyncVectorEnv
```

Observation `(n_tubes, capacity, n_colors + 1)` int8 one-hot (last channel = empty), action
`from * n_tubes + to`. Illegal action: state unchanged, reward −1, counts towards the move limit
`k · opt_moves`. Terminated on solved or dead end, truncated at the move limit.

## Trajectories

```bash
python -m jepa_water_sort.logger collect --dataset ../data/uniform_c6k4e2 --split train \
    --source epsilon --epsilon 0.1 --policy-seed 0 --out ../data/traj_eps01_train
python -m jepa_water_sort.logger export-npz ../data/traj_eps01_train --out traj.npz
```

Sources: `optimal`, `random`, `epsilon`, `greedy` (collected on one dataset split only) and
`human` (`logger.import_human`). Output: zstd Parquet shards of about 1M transitions plus a
`manifest.json`; `logger.read_transitions` / `logger.to_numpy` load them, and
`encode_observation` rebuilds the one-hot observation. Details: D19 in `docs/decisions.md`.
