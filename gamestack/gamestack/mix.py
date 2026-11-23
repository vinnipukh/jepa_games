"""Mixed environment: every episode is a different game, with one text interface.

Meant for LLM agents and for training generalists. Observations are prompts
(``"[game] rules ... state ..."``), actions are text replies. Episodes are drawn by weight; the
inner game of an episode is seeded from the mix seed so runs are reproducible.

``MixEnv.reset`` -> ``(text, info)`` with ``info["game"]``; ``MixEnv.step(text)`` ->
``(text, reward, terminated, truncated, info)``. ``info["illegal"]`` is set for unparseable or
illegal replies, which cost ``illegal_reward`` and do not change the state."""

from __future__ import annotations

from typing import Any

import numpy as np

from gamestack.core import GameEnv
from gamestack.registry import make


class MixEnv:
    def __init__(self, games: dict[str, float] | list[str], game_kwargs: dict[str, dict] | None = None):
        if isinstance(games, list):
            games = {g: 1.0 for g in games}
        self.names = list(games)
        w = np.array([games[n] for n in self.names], dtype=float)
        self.weights = w / w.sum()
        self.game_kwargs = game_kwargs or {}
        self.envs: dict[str, GameEnv] = {}
        self.rng = np.random.default_rng()
        self.current: GameEnv | None = None

    def _env(self, name: str) -> GameEnv:
        if name not in self.envs:
            self.envs[name] = make(name, **self.game_kwargs.get(name, {}))
        return self.envs[name]

    def prompt(self) -> str:
        g = self.current
        return f"[{g.name}] {g.action_prompt()}\n\n{g.text_observation()}"

    def reset(self, *, seed: int | None = None, game: str | None = None) -> tuple[str, dict[str, Any]]:
        if seed is not None:
            self.rng = np.random.default_rng(seed)
        name = game or self.names[int(self.rng.choice(len(self.names), p=self.weights))]
        self.current = self._env(name)
        _, info = self.current.reset(seed=int(self.rng.integers(2**31)))
        info = dict(info)
        info["game"] = name
        info["legal_actions"] = list(self.current.action_names().values())
        return self.prompt(), info

    def step(self, text: str):
        assert self.current is not None, "call reset() first"
        _, reward, term, trunc, info = self.current.step_text(text)
        info = dict(info)
        info["game"] = self.current.name
        info["legal_actions"] = list(self.current.action_names().values())
        return self.prompt(), reward, term, trunc, info
