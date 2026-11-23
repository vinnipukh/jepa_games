# gamestack

One API over many games, for RL agents, JEPA world models and LLMs.

```python
from gamestack import make, MixEnv, list_games
env = make("chess", opponent="greedy")
obs, info = env.reset(seed=0)             # Gymnasium: discrete actions, numpy observations
print(env.text_observation(), env.action_prompt())
obs, r, term, trunc, info = env.step_text("e2e4")   # LLM reply -> action

mix = MixEnv({"sudoku": 1, "uno": 1, "go": 1})      # a different game every episode, text only
prompt, info = mix.reset(seed=0)
```

| Game | Source | Status |
|---|---|---|
| `water_sort` | repo Rust core (`jepa_water_sort`) | done |
| `sudoku` (4/6/9) | own | done |
| `flappy` | own | done |
| `pointer_chase`, `pointer_gui` | own | done |
| `chess` | python-chess | done |
| `go` (5/7/9) | own | done |
| `uno` | own | done |
| `crafter` (Minecraft-like) | crafter | done |
| `openfront` clone | own | planned |
| `openfront` real engine | OpenFrontIO (AGPL, external process) | planned |
| `minecraft` real / `horizon` | MineDojo / Mineflayer / horizon-rust | planned |

## Setup and tests

```bash
cd gamestack && uv venv --python 3.12 && uv pip install -e '.[all,test]'
python -m pytest                                  # water_sort is skipped without the native module
# with Water Sort: from python/ run `maturin develop --release`, then PYTHONPATH=gamestack pytest
```

Add a game: subclass `GameEnv` (`gamestack/core.py`), register it in `gamestack/registry.py`, and the contract tests pick it up.
See `docs/decisions.md` D23.
