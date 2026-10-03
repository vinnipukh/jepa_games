"""Water Sort puzzles backed by the Rust core (``water_sort_core``).

Every game rule (moves, solving, stars, hashing, generation) runs in Rust through the native
module, so results are bit-identical to the Rust crates. This package adds the Gymnasium
environment, the native vector environment, policies and the trajectory logger on top.
"""

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
    batch_generate,
    batch_step,
    canonical,
    canonical_hash,
    fresh_seed,
    generate,
    legal_moves,
    solve,
    splitmix64,
    stars,
    step,
    tier,
    variant,
)

__all__ = [
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
    "variant",
]
