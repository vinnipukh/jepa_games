"""Sudoku (4x4 / 6x6 / 9x9). Puzzles are made from a random solved grid by removing cells while the
solution stays unique (checked with a counting backtracking solver). Deterministic per seed."""

from __future__ import annotations

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv

_BOX = {4: (2, 2), 6: (2, 3), 9: (3, 3)}  # (box rows, box cols)


def _ok(g: np.ndarray, r: int, c: int, v: int, n: int) -> bool:
    br, bc = _BOX[n]
    if v in g[r] or v in g[:, c]:
        return False
    r0, c0 = r // br * br, c // bc * bc
    return v not in g[r0 : r0 + br, c0 : c0 + bc]


def _solve(g: np.ndarray, n: int, rng: np.random.Generator | None, limit: int) -> int:
    """Count solutions up to ``limit``; fills ``g`` with the first one found when it returns >=1."""
    empties = np.argwhere(g == 0)
    if len(empties) == 0:
        return 1
    best, best_opts = None, None
    for r, c in empties:  # most constrained cell
        opts = [v for v in range(1, n + 1) if _ok(g, r, c, v, n)]
        if best_opts is None or len(opts) < len(best_opts):
            best, best_opts = (r, c), opts
            if len(opts) <= 1:
                break
    if not best_opts:
        return 0
    if rng is not None:
        best_opts = list(rng.permutation(best_opts))
    r, c = best
    count = 0
    for v in best_opts:
        g[r, c] = v
        count += _solve(g, n, rng, limit - count)
        if count >= limit:
            return count
        if count == 1 and limit == 1:
            return 1
    g[r, c] = 0
    return count


def make_puzzle(n: int, holes: int, rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    full = np.zeros((n, n), dtype=np.int8)
    _solve(full, n, rng, 1)
    puzzle = full.copy()
    cells = rng.permutation(n * n)
    removed = 0
    for idx in cells:
        if removed >= holes:
            break
        r, c = divmod(int(idx), n)
        keep = puzzle[r, c]
        puzzle[r, c] = 0
        trial = puzzle.copy()
        if _solve(trial, n, None, 2) == 1:
            removed += 1
        else:
            puzzle[r, c] = keep
    return puzzle, full


class SudokuEnv(GameEnv):
    """Action = ``(cell, value)`` encoded as ``cell * n + (value - 1)``.

    Reward: +1/(holes) for a correct fill, -1/(holes) for a wrong one (the cell is still filled so
    the agent can see consequences, but the episode only ends when the grid is full and correct or
    after ``max_steps``). Placing on a given cell is illegal."""

    name = "sudoku"

    def __init__(self, size: int = 4, holes: int | None = None, max_steps: int | None = None):
        super().__init__()
        assert size in _BOX
        self.n = size
        self.holes = holes if holes is not None else {4: 8, 6: 18, 9: 40}[size]
        self.max_steps = max_steps or 4 * self.holes
        self.action_space = spaces.Discrete(size * size * size)
        self.observation_space = spaces.Dict(
            {
                "grid": spaces.Box(0, size, (size, size), np.int8),
                "given": spaces.Box(0, 1, (size, size), np.int8),
            }
        )

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self.puzzle, self.solution = make_puzzle(self.n, self.holes, self.np_random)
        self.grid = self.puzzle.copy()
        self.steps = 0
        return self._observation(), {"solution_hash": int(self.solution.sum())}

    def _observation(self):
        return {"grid": self.grid.copy(), "given": (self.puzzle > 0).astype(np.int8)}

    def legal_actions(self):
        n = self.n
        return [
            (r * n + c) * n + v - 1
            for r in range(n) for c in range(n) if self.puzzle[r, c] == 0 for v in range(1, n + 1)
        ]

    def action_name(self, action):
        cell, v = divmod(action, self.n)
        r, c = divmod(cell, self.n)
        return f"r{r + 1}c{c + 1}={v + 1}"

    def parse_action(self, text):
        import re

        m = list(re.finditer(r"r(\d+)\s*c(\d+)\s*=\s*(\d+)", text.lower()))
        if not m:
            return None
        r, c, v = (int(x) for x in m[-1].groups())
        n = self.n
        if not (1 <= r <= n and 1 <= c <= n and 1 <= v <= n) or self.puzzle[r - 1, c - 1] != 0:
            return None
        return ((r - 1) * n + (c - 1)) * n + v - 1

    def step(self, action):
        n = self.n
        cell, v = divmod(int(action), n)
        r, c = divmod(cell, n)
        if self.puzzle[r, c] != 0:
            return self._illegal()
        self.steps += 1
        correct = self.solution[r, c] == v + 1
        self.grid[r, c] = v + 1
        reward = (1.0 if correct else -1.0) / self.holes
        solved = bool((self.grid == self.solution).all())
        if solved:
            reward += 1.0
        truncated = not solved and self.steps >= self.max_steps
        return self._observation(), reward, solved, truncated, {"correct": bool(correct), "solved": solved}

    def text_observation(self):
        rows = []
        br, bc = _BOX[self.n]
        for r in range(self.n):
            cells = []
            for c in range(self.n):
                cells.append(str(self.grid[r, c]) if self.grid[r, c] else ".")
                if (c + 1) % bc == 0 and c + 1 < self.n:
                    cells.append("|")
            rows.append(" ".join(cells))
            if (r + 1) % br == 0 and r + 1 < self.n:
                rows.append("-" * len(rows[-1]))
        return "\n".join(rows)

    def action_prompt(self):
        n = self.n
        return (
            f"Sudoku {n}x{n}: fill every '.' so each row, column and box holds 1..{n} once. "
            f"Reply with one move as r<row>c<col>=<value>, e.g. r1c2=3 (rows/cols 1..{n}). "
            "You cannot change given cells."
        )
