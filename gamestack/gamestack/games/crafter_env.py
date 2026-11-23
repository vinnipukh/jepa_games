"""Crafter (https://github.com/danijar/crafter): a 2D Minecraft-like survival game (collect wood,
craft a pickaxe, mine, fight zombies, eat, sleep). 22 achievements; reward = achievement unlocked
(+1) plus Crafter's health delta. Observation is the 64x64 RGB image; the text view is the local
semantic map plus inventory, so LLMs can play without vision."""

from __future__ import annotations

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv


class CrafterGame(GameEnv):
    name = "crafter"

    def __init__(self, area=(64, 64), view=(9, 9), size=(64, 64), length: int = 10000, text_radius: int = 4):
        super().__init__()
        import crafter  # optional dependency

        self._crafter = crafter
        self._kw = dict(area=area, view=view, size=size, length=length)
        self.env = crafter.Env(**self._kw, seed=0)
        # noop, move_left, ..., place_table, ... "do" is renamed: it is an ordinary English word and
        # would match inside LLM prose.
        self.names = ["interact" if n == "do" else n for n in self.env.action_names]
        self.action_space = spaces.Discrete(len(self.names))
        self.observation_space = spaces.Box(0, 255, (*size, 3), np.uint8)
        self.text_radius = text_radius
        self._info: dict = {}
        self._last = None

    def reset(self, *, seed=None, options=None):
        self._illegal_count = 0
        self.env = self._crafter.Env(**self._kw, seed=0 if seed is None else int(seed))
        self._last = self.env.reset()
        self._info = {}
        return self._last, {}

    def _observation(self):
        return self._last

    def step(self, action):
        self._last, reward, done, info = self.env.step(int(action))
        self._info = info
        achieved = sum(v > 0 for v in info["achievements"].values())
        # Crafter marks time-limit ends with discount != 0 only via length; treat done as terminated
        # when the player died, truncated at the step limit.
        died = info["inventory"]["health"] <= 0
        trunc = done and not died
        return self._last, float(reward), bool(done and died), bool(trunc), {"achievements": achieved, **info}

    def legal_actions(self):
        return list(range(len(self.names)))

    def action_name(self, action):
        return self.names[action]

    def text_observation(self):
        world = self.env._world
        px, py = self.env._player.pos
        r = self.text_radius
        mats = world._mat_ids if hasattr(world, "_mat_ids") else {}
        inv = {k: v for k, v in (self._info.get("inventory") or self.env._player.inventory).items() if v}
        rows = []
        for y in range(py - r, py + r + 1):
            row = []
            for x in range(px - r, px + r + 1):
                if (x, y) == (px, py):
                    row.append("@")
                elif not (0 <= x < world.area[0] and 0 <= y < world.area[1]):
                    row.append("#")
                else:
                    obj = world[(x, y)][1]
                    mat = world[(x, y)][0]
                    row.append("Z" if obj and type(obj).__name__ == "Zombie" else
                               "S" if obj and type(obj).__name__ == "Skeleton" else
                               "C" if obj and type(obj).__name__ == "Cow" else mat[:1])
            rows.append(" ".join(row))
        return "\n".join(rows) + f"\nInventory: {inv}\nFacing: {self.env._player.facing}"

    def action_prompt(self):
        return (
            "Crafter survival. '@' is you, Z zombie, S skeleton, C cow; terrain letters: g grass, t tree, "
            "s stone/sand, w water, c coal, i iron, d diamond, l lava, p path, T table. Unlock achievements "
            "(collect wood, place table, make pickaxe, mine, eat, drink, sleep...). Reply with one action: "
            + ", ".join(self.names) + "."
        )
