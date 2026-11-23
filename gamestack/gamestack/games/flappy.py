"""Flappy Bird on a small integer grid. Fully deterministic given the seed (pipe gaps are drawn
from the env RNG). Actions: 0 = glide, 1 = flap."""

from __future__ import annotations

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv


class FlappyEnv(GameEnv):
    name = "flappy"

    def __init__(self, height: int = 12, width: int = 16, gap: int = 4, spacing: int = 6, max_steps: int = 500):
        super().__init__()
        self.h, self.w, self.gap, self.spacing, self.max_steps = height, width, gap, spacing, max_steps
        self.bird_x = 3
        self.action_space = spaces.Discrete(2)
        self.observation_space = spaces.Dict(
            {
                "grid": spaces.Box(0, 2, (height, width), np.int8),  # 0 empty, 1 pipe, 2 bird
                "vy": spaces.Box(-5, 5, (1,), np.int8),
            }
        )

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self.y, self.vy, self.t, self.score = self.h // 2, 0, 0, 0
        self.pipes: list[list[int]] = []  # [x, gap_top]
        x = self.w
        while x < 2 * self.w:
            self.pipes.append([x, self._gap_top()])
            x += self.spacing
        return self._observation(), {}

    def _gap_top(self) -> int:
        return int(self.np_random.integers(1, self.h - self.gap))

    def _observation(self):
        g = np.zeros((self.h, self.w), dtype=np.int8)
        for x, top in self.pipes:
            if 0 <= x < self.w:
                g[:, x] = 1
                g[top : top + self.gap, x] = 0
        g[int(np.clip(self.y, 0, self.h - 1)), self.bird_x] = 2
        return {"grid": g, "vy": np.array([self.vy], dtype=np.int8)}

    def step(self, action):
        if action not in (0, 1):
            return self._illegal()
        self.vy = -2 if action == 1 else min(self.vy + 1, 3)
        self.y += self.vy
        self.t += 1
        reward = 0.1
        for p in self.pipes:
            p[0] -= 1
            if p[0] == self.bird_x - 1:  # passed a pipe
                reward += 1.0
                self.score += 1
        if self.pipes and self.pipes[0][0] < 0:
            self.pipes.pop(0)
        if self.pipes[-1][0] < self.w - self.spacing:
            self.pipes.append([self.pipes[-1][0] + self.spacing, self._gap_top()])
        dead = self.y < 0 or self.y >= self.h
        for x, top in self.pipes:
            if x == self.bird_x and not (top <= self.y < top + self.gap):
                dead = True
        if dead:
            return self._observation(), -1.0, True, False, {"score": self.score}
        return self._observation(), reward, False, self.t >= self.max_steps, {"score": self.score}

    def legal_actions(self):
        return [0, 1]

    def action_name(self, action):
        return ("glide", "flap")[action]

    def text_observation(self):
        chars = {0: ".", 1: "#", 2: "B"}
        grid = self._observation()["grid"]
        return "\n".join("".join(chars[v] for v in row) for row in grid) + f"\nvertical speed: {self.vy} (negative = up)"

    def action_prompt(self):
        return (
            "Flappy Bird. B is the bird, # are pipes, gaps are '.'. Each turn the bird falls "
            "unless you flap. Reply 'flap' or 'glide'. Fly through the gaps; hitting a pipe or the "
            "floor/ceiling ends the game."
        )
