"""Water Sort: adapter over the repo's Rust-backed ``jepa_water_sort.WaterSortEnv`` (D19), adding the
common text interface. Needs the native module: ``cd python && maturin develop --release``."""

from __future__ import annotations

import numpy as np

from gamestack.core import GameEnv

_LETTERS = "abcdefghijklmnopqrstuvwxyz"


class WaterSortGame(GameEnv):
    name = "water_sort"

    def __init__(self, params=(6, 4, 2), **kwargs):
        super().__init__()
        from jepa_water_sort import WaterSortEnv  # lazy: needs the compiled extension

        self.env = WaterSortEnv(params=params, **kwargs)
        self.action_space = self.env.action_space
        self.observation_space = self.env.observation_space
        self.n_tubes = params[0] + params[2]

    def reset(self, *, seed=None, options=None):
        self._illegal_count = 0
        self._last, info = self.env.reset(seed=seed, options=options)
        self._mask = info.get("action_mask")
        return self._last, info

    def _observation(self):
        return self._last

    def step(self, action):
        self._last, r, term, trunc, info = self.env.step(action)
        self._mask = info.get("action_mask", self._mask)
        return self._last, r, term, trunc, info

    def legal_actions(self):
        return [int(i) for i in np.flatnonzero(self.env.action_masks())]

    def action_name(self, action):
        a, b = divmod(action, self.n_tubes)
        return f"{a + 1}->{b + 1}"

    def parse_action(self, text):
        import re

        m = list(re.finditer(r"(\d+)\s*(?:->|to|→)\s*(\d+)", text.lower()))
        if not m:
            return None
        a, b = int(m[-1].group(1)) - 1, int(m[-1].group(2)) - 1
        if not (0 <= a < self.n_tubes and 0 <= b < self.n_tubes):
            return None
        action = a * self.n_tubes + b
        return action if action in self.legal_actions() else None

    def text_observation(self):
        from jepa_water_sort.env import decode_observation

        cells = decode_observation(self._last)
        lines = []
        for i, tube in enumerate(cells, start=1):
            colours = [_LETTERS[c] for c in tube if c != 255]  # bottom to top
            lines.append(f"tube {i}: {' '.join(colours) if colours else '(empty)'}")
        return "\n".join(lines) + "\n(listed bottom to top)"

    def action_prompt(self):
        return (
            "Water Sort. Each tube lists its liquids bottom to top. Pour the top liquid group of one tube "
            "onto another tube whose top matches (or is empty) and has room. Make every tube a single colour. "
            "Reply 'from->to' with tube numbers, e.g. '2->5'."
        )
