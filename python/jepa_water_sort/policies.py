"""Policies for data collection and baselines (Phase 5.3).

A policy maps a :class:`~jepa_water_sort.State` to an action index ``from * n_tubes + to``, or
``None`` when there is no legal move. Every rule comes from the Rust core: legality from
``action_mask``, optimal moves from ``solve``. Randomness comes only from the
``numpy.random.Generator`` passed to :meth:`Policy.reset`, so an episode is reproducible from
the policy seed.
"""

from __future__ import annotations

import numpy as np

from jepa_water_sort import _native
from jepa_water_sort._native import State

#: Solver budget for (re-)solving inside a policy.
SOLVE_MAX_STATES = 5_000_000


def _legal(state: State) -> np.ndarray:
    return np.flatnonzero(_native.action_mask(state))


class Policy:
    """Base class. ``reset`` starts an episode, ``act`` picks the next action."""

    #: Trajectory source label (``optimal``, ``random``, ``epsilon``, ``greedy``).
    source = "policy"

    def reset(self, state: State, rng: np.random.Generator, solution: list[int] | None = None):
        """Starts an episode at ``state``. ``solution`` (action indices) is an optimal solution
        if the caller already has one (dataset records do); otherwise policies that need one
        solve ``state`` themselves."""
        self.rng = rng

    def act(self, state: State) -> int | None:
        raise NotImplementedError


class RandomPolicy(Policy):
    """Uniform over the legal moves."""

    source = "random"

    def act(self, state: State) -> int | None:
        legal = _legal(state)
        if len(legal) == 0:
            return None
        return int(legal[self.rng.integers(len(legal))])


class OptimalPolicy(Policy):
    """Follows an optimal solution, re-solving whenever the state leaves the planned path."""

    source = "optimal"

    def __init__(self, max_states: int = SOLVE_MAX_STATES):
        self.max_states = max_states

    def reset(self, state, rng, solution=None):
        super().reset(state, rng)
        self._plan: list[int] = list(solution) if solution is not None else []
        self._expected = state if solution is not None else None

    def _replan(self, state: State) -> None:
        result = _native.solve(state, self.max_states)
        n = state.n_tubes
        self._plan = [f * n + t for f, t in result.solution] if result.solvable else []
        self._expected = state

    def act(self, state: State) -> int | None:
        if self._expected != state:
            self._replan(state)
        if not self._plan:
            return None
        action = self._plan.pop(0)
        self._expected, _ = _native.step(state, action)
        return action


class EpsilonPolicy(OptimalPolicy):
    """The optimal move with probability ``1 - epsilon``, otherwise a uniform legal move. After a
    deviation the optimal policy re-solves from the new state (dead ends fall back to random
    legal moves)."""

    source = "epsilon"

    def __init__(self, epsilon: float, max_states: int = SOLVE_MAX_STATES):
        if not 0.0 <= epsilon <= 1.0:
            raise ValueError("epsilon must be in [0, 1]")
        super().__init__(max_states)
        self.epsilon = float(epsilon)

    def act(self, state: State) -> int | None:
        if self.rng.random() < self.epsilon:
            legal = _legal(state)
            if len(legal) == 0:
                return None
            return int(legal[self.rng.integers(len(legal))])
        action = super().act(state)
        if action is None:
            legal = _legal(state)
            return int(legal[self.rng.integers(len(legal))]) if len(legal) else None
        return action


class GreedyPolicy(Policy):
    """One-step lookahead: the legal move whose result has the lowest solver heuristic
    (``segments - n_colors``), then the fewest color changes; ties broken at random."""

    source = "greedy"

    def act(self, state: State) -> int | None:
        legal = _legal(state)
        if len(legal) == 0:
            return None
        scores = []
        for a in legal:
            nxt, _ = _native.step(state, int(a))
            scores.append((nxt.heuristic(), nxt.color_changes()))
        best = min(scores)
        ties = [int(a) for a, s in zip(legal, scores) if s == best]
        return ties[self.rng.integers(len(ties))]


def make_policy(source: str, epsilon: float | None = None) -> Policy:
    """The policy for a trajectory source name."""
    if source == "optimal":
        return OptimalPolicy()
    if source == "random":
        return RandomPolicy()
    if source == "greedy":
        return GreedyPolicy()
    if source == "epsilon":
        if epsilon is None:
            raise ValueError("the epsilon source needs epsilon")
        return EpsilonPolicy(epsilon)
    raise ValueError(f"unknown source {source!r} (expected optimal, random, epsilon or greedy)")
