"""Native vector environment: ``num_envs`` Water Sort games stepped in one Rust call.

Behaves like ``gymnasium.vector.SyncVectorEnv`` over :class:`~jepa_water_sort.env.WaterSortEnv`
with the default next-step autoreset: the step after an episode ends resets that sub-env and
returns its first observation with reward 0. Sub-env ``i`` draws its puzzle seeds from its own
generator, seeded with ``seed + i`` on ``reset(seed=seed)``, exactly as ``SyncVectorEnv`` seeds
its copies, so both produce the same episodes for the same seed.
"""

from __future__ import annotations

from typing import Any

import numpy as np
from gymnasium import spaces
from gymnasium.utils import seeding
from gymnasium.vector import AutoresetMode, VectorEnv
from gymnasium.vector.utils import batch_space

from jepa_water_sort import _native
from jepa_water_sort._native import GenConfig, Params
from jepa_water_sort.env import (
    DEAD_END_MAX_STATES,
    DEFAULT_CONFIG,
    DEFAULT_PARAMS,
    _as_params,
    encode_observation,
    with_min_opt,
)


class WaterSortVectorEnv(VectorEnv):
    """``num_envs`` copies of ``WaterSortEnv(**kwargs)``, stepped by ``batch_env_step``.

    Takes the same keyword arguments as :class:`~jepa_water_sort.env.WaterSortEnv` except
    ``compute_solvability`` and ``render_mode``. ``infos`` holds numpy arrays (``action_mask``,
    ``opt_moves``, ``moves_so_far``, ``move_limit``, ``seed``, ``canonical_hash``,
    ``puzzle_code``; after a step also ``illegal``, ``dead_end``, ``solved``, ``units_moved``),
    each with a ``_key`` mask as in Gymnasium's vector envs.
    """

    metadata = {"autoreset_mode": AutoresetMode.NEXT_STEP}

    def __init__(
        self,
        num_envs: int,
        generator: str = "uniform",
        params: Params | tuple[int, int, int] = DEFAULT_PARAMS,
        layout: str = "standard",
        strategy: str | None = None,
        config: GenConfig | None = None,
        min_opt: int | None = None,
        move_limit_k: int = 4,
        dead_end_check: bool = False,
        dead_end_max_states: int = DEAD_END_MAX_STATES,
        shaping: bool = False,
        gamma: float = 0.99,
    ):
        if num_envs < 1:
            raise ValueError("num_envs must be at least 1")
        self.num_envs = int(num_envs)
        self.generator = generator
        self.params = _as_params(params)
        self.layout = layout
        self.strategy = strategy
        config = DEFAULT_CONFIG if config is None else config
        self.config = config if min_opt is None else with_min_opt(config, int(min_opt))
        self.move_limit_k = int(move_limit_k)
        self.dead_end_max_states = int(dead_end_max_states) if dead_end_check else None
        self.shaping_gamma = float(gamma) if shaping else None
        self.generator_variant = _native.variant(generator, strategy, layout)

        p = self.params
        self.single_observation_space = spaces.Box(
            0, 1, (p.n_tubes, p.capacity, p.n_colors + 1), dtype=np.int8
        )
        self.single_action_space = spaces.Discrete(p.n_tubes**2)
        self.observation_space = batch_space(self.single_observation_space, self.num_envs)
        self.action_space = batch_space(self.single_action_space, self.num_envs)

        n = self.num_envs
        self.states = np.full((n, p.n_tubes, p.capacity), _native.EMPTY, dtype=np.uint8)
        self.opt_moves = np.zeros(n, dtype=np.uint32)
        self.move_limits = np.zeros(n, dtype=np.uint32)
        self.moves_so_far = np.zeros(n, dtype=np.uint32)
        self.seeds = np.zeros(n, dtype=np.uint64)
        self.canonical_hashes = np.zeros(n, dtype=np.uint64)
        self.puzzle_codes = np.full(n, "", dtype=object)
        self._autoreset = np.zeros(n, dtype=np.bool_)
        self._rngs: list[np.random.Generator | None] = [None] * n

    def reset(self, *, seed: int | list[int | None] | None = None, options: dict | None = None):
        if options:
            raise ValueError("WaterSortVectorEnv.reset takes no options")
        if seed is None:
            seeds = [None] * self.num_envs
        elif isinstance(seed, (int, np.integer)):
            seeds = [int(seed) + i for i in range(self.num_envs)]
        else:
            seeds = list(seed)
            if len(seeds) != self.num_envs:
                raise ValueError(f"{len(seeds)} seeds for {self.num_envs} envs")
        for i, s in enumerate(seeds):
            if s is not None or self._rngs[i] is None:
                self._rngs[i], _ = seeding.np_random(s)
        self._load(np.arange(self.num_envs))
        self._autoreset[:] = False
        return self._obs(), self._infos()

    def step(self, actions):
        actions = np.asarray(actions, dtype=np.int64).reshape(self.num_envs)
        resetting = self._autoreset.copy()
        active = ~resetting
        n = self.num_envs
        rewards = np.zeros(n, dtype=np.float64)
        terminated = np.zeros(n, dtype=np.bool_)
        truncated = np.zeros(n, dtype=np.bool_)
        illegal = np.zeros(n, dtype=np.bool_)
        dead_end = np.zeros(n, dtype=np.bool_)
        solved = np.zeros(n, dtype=np.bool_)
        units = np.zeros(n, dtype=np.uint8)
        if active.any():
            idx = np.flatnonzero(active)
            out = _native.batch_env_step(
                self.states[idx],
                actions[idx],
                self.moves_so_far[idx],
                self.move_limits[idx],
                self.shaping_gamma,
                self.dead_end_max_states,
            )
            (self.states[idx], units[idx], rewards[idx], terminated[idx], truncated[idx],
             illegal[idx], dead_end[idx], solved[idx]) = out
            self.moves_so_far[idx] += 1
        if resetting.any():
            self._load(np.flatnonzero(resetting))
        self._autoreset = terminated | truncated
        infos = self._infos()
        for key, value in [("illegal", illegal), ("dead_end", dead_end), ("solved", solved),
                           ("units_moved", units)]:
            infos[key] = value
            infos[f"_{key}"] = active.copy()
        return self._obs(), rewards, terminated, truncated, infos

    def action_masks(self) -> np.ndarray:
        """``(num_envs, n_tubes**2)`` legal actions (sb3-contrib ``MaskablePPO`` convention)."""
        return _native.batch_action_mask(self.states)

    # -- helpers ----------------------------------------------------------------------------

    def _load(self, idx: np.ndarray) -> None:
        """Draws a puzzle seed for each sub-env in ``idx`` and generates the puzzles."""
        seeds = [int(self._rngs[i].integers(0, 2**64, dtype=np.uint64)) for i in idx]
        puzzles = _native.batch_generate(
            self.generator, self.params, seeds, None, self.config, self.strategy, self.layout
        )
        for i, puzzle in zip(idx, puzzles):
            self.states[i] = puzzle.state.to_numpy()
            self.opt_moves[i] = puzzle.opt_moves
            self.move_limits[i] = _native.move_limit(self.move_limit_k, puzzle.opt_moves)
            self.moves_so_far[i] = 0
            self.seeds[i] = puzzle.seed
            self.canonical_hashes[i] = puzzle.canonical_hash
            self.puzzle_codes[i] = puzzle.puzzle_code

    def _obs(self) -> np.ndarray:
        return encode_observation(self.states, self.params.n_colors)

    def _infos(self) -> dict[str, Any]:
        everywhere = np.ones(self.num_envs, dtype=np.bool_)
        infos = {
            "action_mask": self.action_masks(),
            "opt_moves": self.opt_moves.astype(np.int64),
            "moves_so_far": self.moves_so_far.astype(np.int64),
            "move_limit": self.move_limits.astype(np.int64),
            "seed": self.seeds.copy(),
            "canonical_hash": self.canonical_hashes.copy(),
            "puzzle_code": self.puzzle_codes.copy(),
        }
        for key in list(infos):
            infos[f"_{key}"] = everywhere.copy()
        return infos
