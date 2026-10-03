"""Water Sort puzzles backed by the Rust core (``water_sort_core``).

Every game rule (moves, solving, stars, hashing, generation) runs in Rust through the native
module, so results are bit-identical to the Rust crates. This package adds the Gymnasium
environment, the native vector environment, policies and the trajectory logger on top.
"""

import gymnasium

from jepa_water_sort._native import (
    EMPTY,
    GenConfig,
    GenerationError,
    Params,
    Puzzle,
    SolveResult,
    StarBelowOptimalError,
    State,
    __version__,
    action_mask,
    batch_action_mask,
    batch_env_step,
    batch_generate,
    batch_step,
    canonical,
    canonical_hash,
    env_step,
    fresh_seed,
    generate,
    is_dead_end,
    legal_moves,
    move_limit,
    solve,
    splitmix64,
    stars,
    step,
    tier,
    time_seed,
    variant,
)
from jepa_water_sort.env import WaterSortEnv, decode_observation, encode_observation
from jepa_water_sort.vector import WaterSortVectorEnv

ENV_ID = "jepa_water_sort/WaterSort-v0"
if ENV_ID not in gymnasium.envs.registry:
    gymnasium.register(ENV_ID, entry_point="jepa_water_sort.env:WaterSortEnv")

__all__ = [
    "ENV_ID",
    "WaterSortEnv",
    "WaterSortVectorEnv",
    "decode_observation",
    "encode_observation",
    "env_step",
    "is_dead_end",
    "move_limit",
    "EMPTY",
    "GenConfig",
    "GenerationError",
    "Params",
    "Puzzle",
    "SolveResult",
    "StarBelowOptimalError",
    "State",
    "__version__",
    "action_mask",
    "batch_action_mask",
    "batch_env_step",
    "batch_generate",
    "batch_step",
    "canonical",
    "canonical_hash",
    "fresh_seed",
    "generate",
    "legal_moves",
    "solve",
    "splitmix64",
    "stars",
    "step",
    "tier",
    "time_seed",
    "variant",
]
