"""Baseline policies (PLAN 7.4) with the batched interface the evaluation uses.

Every policy has ``name``, ``reset(n, rng)`` (start ``n`` episodes; all randomness comes from
``rng``) and ``act(cells, envs)``: ``cells`` are the ``(B, n_tubes, capacity)`` real states of
the environments ``envs``; it returns ``B`` action indices ``from * n_tubes + to`` (``-1`` when
a state has no legal move). Every rule comes from the core through ``jepa_water_sort``.

- :class:`RandomLegal`: uniform over the legal moves.
- :class:`Greedy`: the legal move minimizing ``color_changes`` after the move, ties broken by
  ``segments`` and then at random (PLAN 7.4; the logger's ``greedy`` source orders by the solver
  heuristic first).
- :class:`Solver`: follows an optimal solution (the upper bound: stars 5, ratio 1).
- DQN / DDQN: :mod:`jepa.dqn`.
"""

from __future__ import annotations

import numpy as np

import jepa_water_sort as w
from jepa_water_sort.policies import SOLVE_MAX_STATES


class RandomLegal:
    name = "random"

    def reset(self, n: int, rng: np.random.Generator) -> None:
        self.rngs = [np.random.default_rng(s) for s in rng.integers(2**63, size=n)]

    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray:
        masks = w.batch_action_mask(cells)
        out = np.full(len(cells), -1, dtype=np.int64)
        for i, (m, e) in enumerate(zip(masks, envs)):
            legal = np.flatnonzero(m)
            if len(legal):
                out[i] = legal[self.rngs[e].integers(len(legal))]
        return out


class Greedy(RandomLegal):
    name = "greedy"

    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray:
        masks = w.batch_action_mask(cells)
        out = np.full(len(cells), -1, dtype=np.int64)
        for i, (c, m, e) in enumerate(zip(cells, masks, envs)):
            legal = np.flatnonzero(m)
            if len(legal) == 0:
                continue
            nxt, _ = w.batch_step(np.repeat(c[None], len(legal), 0), legal)
            scores = []
            for n in nxt:
                s = w.State.from_numpy(n)
                scores.append((s.color_changes(), s.segments()))
            best = min(scores)
            ties = [int(a) for a, sc in zip(legal, scores) if sc == best]
            out[i] = ties[self.rngs[e].integers(len(ties))]
        return out


class Solver:
    """Optimal play: solves each new state (the stored solution is not needed)."""

    name = "solver"

    def __init__(self, max_states: int = SOLVE_MAX_STATES):
        self.max_states = max_states

    def reset(self, n: int, rng: np.random.Generator) -> None:
        self.plans: list[list[int]] = [[] for _ in range(n)]
        self.expected: list[bytes | None] = [None] * n

    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray:
        out = np.full(len(cells), -1, dtype=np.int64)
        for i, (c, e) in enumerate(zip(cells, envs)):
            if self.expected[e] != c.tobytes() or not self.plans[e]:
                s = w.State.from_numpy(c)
                r = w.solve(s, self.max_states)
                n = s.n_tubes
                self.plans[e] = [f * n + t for f, t in r.solution] if r.solvable else []
            if self.plans[e]:
                a = self.plans[e].pop(0)
                out[i] = a
                nxt, _ = w.batch_step(c[None], np.array([a]))
                self.expected[e] = nxt[0].tobytes()
        return out
