"""Two "pointer" tasks.

``PointerChaseEnv``: a hidden table maps each index to another index. The agent must find where
a start index ends up after ``hops`` pointer follows. It can *read* an entry (small cost) or
*answer*. Tests multi-step memory/reasoning without a board.

``PointerGuiEnv``: a mouse-pointer task on a small grid of labelled buttons. Move the pointer and
click the button named in the instruction (a miniature computer-use environment)."""

from __future__ import annotations

import re

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv


class PointerChaseEnv(GameEnv):
    """Actions: ``i`` in ``[0, n)`` = read entry i, ``n + i`` = answer i."""

    name = "pointer_chase"

    def __init__(self, n: int = 8, hops: int = 3, read_cost: float = 0.02, max_steps: int | None = None):
        super().__init__()
        self.n, self.hops, self.read_cost = n, hops, read_cost
        self.max_steps = max_steps or 4 * n
        self.action_space = spaces.Discrete(2 * n)
        self.observation_space = spaces.Dict(
            {
                "known": spaces.Box(-1, n - 1, (n,), np.int16),  # -1 = not yet read
                "start": spaces.Box(0, n - 1, (1,), np.int16),
                "hops": spaces.Box(0, 64, (1,), np.int16),
            }
        )

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self.table = self.np_random.integers(0, self.n, size=self.n)
        self.start = int(self.np_random.integers(0, self.n))
        cur = self.start
        for _ in range(self.hops):
            cur = int(self.table[cur])
        self.answer = cur
        self.known = np.full(self.n, -1, dtype=np.int16)
        self.steps = 0
        return self._observation(), {}

    def _observation(self):
        return {
            "known": self.known.copy(),
            "start": np.array([self.start], dtype=np.int16),
            "hops": np.array([self.hops], dtype=np.int16),
        }

    def step(self, action):
        action = int(action)
        self.steps += 1
        if action < self.n:
            self.known[action] = self.table[action]
            return self._observation(), -self.read_cost, False, self.steps >= self.max_steps, {}
        correct = action - self.n == self.answer
        return self._observation(), 1.0 if correct else -1.0, True, False, {"correct": bool(correct)}

    def legal_actions(self):
        return list(range(2 * self.n))

    def action_name(self, action):
        return f"read {action}" if action < self.n else f"answer {action - self.n}"

    def parse_action(self, text):
        m = list(re.finditer(r"\b(read|answer)\s+(\d+)", text.lower()))
        if not m:
            return None
        kind, i = m[-1].group(1), int(m[-1].group(2))
        if not 0 <= i < self.n:
            return None
        return i if kind == "read" else self.n + i

    def text_observation(self):
        known = ", ".join(f"{i}->{v}" if v >= 0 else f"{i}->?" for i, v in enumerate(self.known))
        return f"Known entries: {known}\nStart at {self.start}, follow the pointer {self.hops} times."

    def action_prompt(self):
        return (
            f"A hidden table maps each index 0..{self.n - 1} to an index. Start at the given index and "
            f"follow the pointer {self.hops} times; the index you land on is the answer. "
            "Reply 'read <i>' to reveal entry i (small cost) or 'answer <i>' to finish."
        )


class PointerGuiEnv(GameEnv):
    """Actions: 0..3 move pointer (up, down, left, right), 4 = click."""

    name = "pointer_gui"
    MOVES = [(-1, 0), (1, 0), (0, -1), (0, 1)]

    def __init__(self, size: int = 5, n_buttons: int = 4, max_steps: int = 40):
        super().__init__()
        self.size, self.n_buttons, self.max_steps = size, n_buttons, max_steps
        self.action_space = spaces.Discrete(5)
        self.observation_space = spaces.Dict(
            {
                "buttons": spaces.Box(0, n_buttons, (size, size), np.int8),  # 0 none, else label id
                "pointer": spaces.Box(0, size - 1, (2,), np.int8),
                "target": spaces.Box(1, n_buttons, (1,), np.int8),
            }
        )

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        s = self.size
        cells = self.np_random.permutation(s * s)[: self.n_buttons + 1]
        self.buttons = np.zeros((s, s), dtype=np.int8)
        for label, c in enumerate(cells[: self.n_buttons], start=1):
            self.buttons[divmod(int(c), s)] = label
        self.pointer = np.array(divmod(int(cells[-1]), s), dtype=np.int8)
        self.target = int(self.np_random.integers(1, self.n_buttons + 1))
        self.t = 0
        return self._observation(), {}

    def _observation(self):
        return {"buttons": self.buttons.copy(), "pointer": self.pointer.copy(), "target": np.array([self.target], dtype=np.int8)}

    def step(self, action):
        if not 0 <= int(action) <= 4:
            return self._illegal()
        self.t += 1
        if action < 4:
            dr, dc = self.MOVES[action]
            self.pointer = np.clip(self.pointer + np.array([dr, dc]), 0, self.size - 1).astype(np.int8)
            return self._observation(), -0.01, False, self.t >= self.max_steps, {}
        hit = int(self.buttons[tuple(self.pointer)])
        if hit == self.target:
            return self._observation(), 1.0, True, False, {"correct": True}
        return self._observation(), -0.2, False, self.t >= self.max_steps, {"correct": False}

    def legal_actions(self):
        return list(range(5))

    def action_name(self, action):
        return ["up", "down", "left", "right", "click"][action]

    def text_observation(self):
        rows = []
        for r in range(self.size):
            row = []
            for c in range(self.size):
                ch = str(self.buttons[r, c]) if self.buttons[r, c] else "."
                row.append(f"[{ch}]" if (r, c) == tuple(self.pointer) else f" {ch} ")
            rows.append("".join(row))
        return "\n".join(rows) + f"\nClick button {self.target}. Pointer is the bracketed cell."

    def action_prompt(self):
        return "Move the pointer with 'up', 'down', 'left', 'right' and press 'click' while on the button named in the instruction."
