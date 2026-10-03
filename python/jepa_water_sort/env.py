"""Gymnasium environment for Water Sort (Phase 5.2).

The episode rules (reward, illegal actions, termination, truncation) are
``water_sort_core::episode`` in Rust, reached through ``_native.env_step``; this module only
builds observations and ``info``.
"""

from __future__ import annotations

from typing import Any

import gymnasium as gym
import numpy as np
from gymnasium import spaces

from jepa_water_sort import _native
from jepa_water_sort._native import EMPTY, GenConfig, Params, Puzzle, State

#: Default puzzle shape: the Phase 4 dataset configuration (D18).
DEFAULT_PARAMS = Params(6, 4, 2)
#: Default generation settings: the core defaults without the 64 random rollouts. Rollouts only
#: fill the difficulty metrics, so every seed gives the same puzzle as with ``GenConfig()``.
DEFAULT_CONFIG = GenConfig(random_rollouts=0)
#: Default solver budget of the optional dead-end check.
DEAD_END_MAX_STATES = 100_000


def encode_observation(cells: np.ndarray, n_colors: int) -> np.ndarray:
    """One-hot encoding of ``(..., n_tubes, capacity)`` uint8 cells (``State.to_numpy()``):
    shape ``(..., n_tubes, capacity, n_colors + 1)`` int8, channel ``n_colors`` = empty."""
    index = np.where(cells == EMPTY, n_colors, cells)
    return np.eye(n_colors + 1, dtype=np.int8)[index]


def decode_observation(obs: np.ndarray) -> np.ndarray:
    """Inverse of :func:`encode_observation`: back to uint8 cells with ``EMPTY`` = 255."""
    n_colors = obs.shape[-1] - 1
    index = np.argmax(obs, axis=-1).astype(np.uint8)
    index[index == n_colors] = EMPTY
    return index


def with_min_opt(config: GenConfig, min_opt: int) -> GenConfig:
    """``config`` with ``min_opt`` replaced."""
    return GenConfig(
        min_opt=min_opt,
        max_opt=config.max_opt,
        max_attempts=config.max_attempts,
        max_states=config.max_states,
        random_rollouts=config.random_rollouts,
        rollout_cap_factor=config.rollout_cap_factor,
        dead_end_ratios=config.dead_end_ratios,
    )


def _as_params(params: Params | tuple[int, int, int]) -> Params:
    return params if isinstance(params, Params) else Params(*params)


class WaterSortEnv(gym.Env):
    """Water Sort as a Gymnasium environment.

    - Observation: ``Box(0, 1, (n_tubes, capacity, n_colors + 1), int8)``, one-hot per cell,
      channel ``n_colors`` meaning empty.
    - Action: ``Discrete(n_tubes**2)``, index ``from * n_tubes + to``. ``info["action_mask"]``
      and :meth:`action_masks` give the legal actions.
    - An illegal action leaves the state unchanged, costs −1, sets ``info["illegal"]`` and counts
      towards the move limit.
    - Reward −1 per action, plus ``gamma * Phi(s') - Phi(s)`` with ``Phi = -color_changes`` when
      ``shaping`` is on.
    - Terminated when solved or at a dead end (no legal move; with ``dead_end_check`` also a
      state the solver proves unsolvable). Truncated after ``move_limit_k * opt_moves`` actions
      (D4).

    ``reset(seed=s)`` draws puzzle seeds from ``self.np_random``, so the sequence of puzzles is
    reproducible for both generators. ``options={"puzzle_seed": int}`` loads the generator's
    puzzle for that seed, ``options={"puzzle_code": str}`` a fixed position (solved once to get
    ``opt_moves``), and ``options={"puzzle": Puzzle}`` a generated puzzle.
    """

    metadata = {"render_modes": ["ansi"], "render_fps": 4}

    def __init__(
        self,
        generator: str = "uniform",
        params: Params | tuple[int, int, int] = DEFAULT_PARAMS,
        layout: str = "standard",
        strategy: str | None = None,
        config: GenConfig | None = None,
        min_opt: int | None = None,
        move_limit_k: int = 4,
        dead_end_check: bool = False,
        dead_end_max_states: int = DEAD_END_MAX_STATES,
        compute_solvability: bool = False,
        solvability_max_states: int = DEAD_END_MAX_STATES,
        shaping: bool = False,
        gamma: float = 0.99,
        render_mode: str | None = None,
    ):
        self.generator = generator
        self.params = _as_params(params)
        self.layout = layout
        self.strategy = strategy
        config = DEFAULT_CONFIG if config is None else config
        if min_opt is not None:
            config = with_min_opt(config, int(min_opt))
        self.config = config
        self.move_limit_k = int(move_limit_k)
        self.dead_end_check = bool(dead_end_check)
        self.dead_end_max_states = int(dead_end_max_states)
        self.compute_solvability = bool(compute_solvability)
        self.solvability_max_states = int(solvability_max_states)
        self.shaping = bool(shaping)
        self.gamma = float(gamma)
        if render_mode is not None and render_mode not in self.metadata["render_modes"]:
            raise ValueError(f"unsupported render_mode {render_mode!r}")
        self.render_mode = render_mode
        # Fails early on a bad generator, strategy or layout.
        self.generator_variant = _native.variant(generator, strategy, layout)

        p = self.params
        self.n_tubes = p.n_tubes
        self.observation_space = spaces.Box(
            0, 1, (p.n_tubes, p.capacity, p.n_colors + 1), dtype=np.int8
        )
        self.action_space = spaces.Discrete(p.n_tubes**2)

        self.state: State | None = None
        self.puzzle_state: State | None = None
        self.puzzle_seed: int | None = None
        self.opt_moves = 0
        self.move_limit = 0
        self.moves_so_far = 0

    # -- Gymnasium API ---------------------------------------------------------------------

    def reset(self, *, seed: int | None = None, options: dict[str, Any] | None = None):
        super().reset(seed=seed)
        options = options or {}
        given = [k for k in ("puzzle", "puzzle_code", "puzzle_seed") if options.get(k) is not None]
        if len(given) > 1:
            raise ValueError(f"pass at most one of {given}")
        if "puzzle" in given:
            self._load_puzzle(options["puzzle"])
        elif "puzzle_code" in given:
            self._load_code(options["puzzle_code"])
        else:
            if "puzzle_seed" in given:
                puzzle_seed = int(options["puzzle_seed"])
            else:
                puzzle_seed = int(self.np_random.integers(0, 2**64, dtype=np.uint64))
            self._load_puzzle(self._generate(puzzle_seed))
        return self._obs(), self._info()

    def step(self, action):
        if self.state is None:
            raise RuntimeError("call reset() before step()")
        state, units, reward, terminated, truncated, illegal, dead_end, solved = _native.env_step(
            self.state,
            int(action),
            self.moves_so_far,
            self.move_limit,
            self.gamma if self.shaping else None,
            self.dead_end_max_states if self.dead_end_check else None,
        )
        self.state = state
        self.moves_so_far += 1
        info = self._info()
        info.update(illegal=illegal, dead_end=dead_end, solved=solved, units_moved=units)
        return self._obs(), reward, terminated, truncated, info

    def render(self):
        if self.render_mode == "ansi" and self.state is not None:
            return str(self.state)
        return None

    def action_masks(self) -> np.ndarray:
        """Legal actions as a bool array (the sb3-contrib ``MaskablePPO`` convention)."""
        return _native.action_mask(self.state)

    # -- helpers ----------------------------------------------------------------------------

    def _generate(self, puzzle_seed: int) -> Puzzle:
        return _native.generate(
            self.generator, self.params, puzzle_seed, self.config, self.strategy, self.layout
        )

    def _check_params(self, state: State) -> None:
        if state.params != self.params:
            raise ValueError(f"puzzle has {state.params}, the environment uses {self.params}")

    def _load_puzzle(self, puzzle: Puzzle) -> None:
        self._check_params(puzzle.state)
        self._start(puzzle.state, puzzle.opt_moves, puzzle.seed)

    def _load_code(self, code: str) -> None:
        state = State.from_code(code)
        self._check_params(state)
        result = _native.solve(state, self.config.max_states)
        if not result.solvable:
            raise ValueError(f"puzzle {code} is not solvable ({result.status})")
        self._start(state, result.opt_moves, None)

    def _start(self, state: State, opt_moves: int, puzzle_seed: int | None) -> None:
        self.state = state
        self.puzzle_state = state
        self.puzzle_seed = puzzle_seed
        self.opt_moves = opt_moves
        self.move_limit = _native.move_limit(self.move_limit_k, opt_moves)
        self.moves_so_far = 0

    def _obs(self) -> np.ndarray:
        return encode_observation(self.state.to_numpy(), self.params.n_colors)

    def _info(self) -> dict[str, Any]:
        info = {
            "opt_moves": self.opt_moves,
            "moves_so_far": self.moves_so_far,
            "move_limit": self.move_limit,
            # numpy uint64 rather than int, so Gymnasium's vector envs batch values >= 2**63.
            "seed": None if self.puzzle_seed is None else np.uint64(self.puzzle_seed),
            "generator_variant": self.generator_variant,
            "layout": self.layout,
            "puzzle_code": self.puzzle_state.puzzle_code,
            "canonical_hash": np.uint64(_native.canonical_hash(self.puzzle_state)),
            "action_mask": _native.action_mask(self.state),
        }
        if self.compute_solvability:
            info["solvable"] = _native.solve(self.state, self.solvability_max_states).status
        return info
