"""Common environment API for every game in the stack.

Every game is a ``GameEnv``: a Gymnasium environment with a *discrete* action space (so RL agents
and JEPA world models treat all games alike), plus a text interface for LLM agents:

* ``text_observation()``  – the state as prompt text,
* ``action_prompt()``     – the rules and the accepted action format,
* ``legal_actions()``     – the legal action ids (the discrete action space may be larger),
* ``action_names``        – id -> short text such as ``"e2e4"``, ``"place 3 at r2c5"``,
* ``parse_action(text)``  – LLM reply -> action id, or ``None`` if it is not a legal action.

Single-player games return ``reward`` for the agent. Two-player games (chess, go, uno) are played
by the agent against a built-in opponent (``opponent`` argument), so they are single-agent
environments from the outside.
"""

from __future__ import annotations

import re
from abc import abstractmethod
from typing import Any

import gymnasium as gym
import numpy as np


class GameEnv(gym.Env):
    """Base class. Subclasses set ``name`` and implement the abstract methods."""

    name: str = "game"
    metadata = {"render_modes": ["ansi"]}
    #: Reward the agent gets for an action that is not legal (no state change).
    illegal_reward: float = -1.0

    def __init__(self) -> None:
        self.np_random_seeded = False
        self._illegal_count = 0

    # ---- text interface -------------------------------------------------------------------
    @abstractmethod
    def text_observation(self) -> str: ...

    @abstractmethod
    def action_prompt(self) -> str: ...

    @abstractmethod
    def legal_actions(self) -> list[int]: ...

    @abstractmethod
    def action_name(self, action: int) -> str: ...

    def action_names(self) -> dict[int, str]:
        return {a: self.action_name(a) for a in self.legal_actions()}

    def parse_action(self, text: str) -> int | None:
        """Default parser: the last legal action name found in ``text`` (case-insensitive).

        Matching is on whole tokens so ``"e2e4"`` does not match inside ``"e2e4q"``."""
        best, best_pos = None, -1
        for action, name in self.action_names().items():
            pattern = r"(?<![\w-])" + re.escape(name.lower()) + r"(?![\w-])"
            for m in re.finditer(pattern, text.lower()):
                if m.start() >= best_pos:
                    best, best_pos = action, m.start()
        return best

    def step_text(self, text: str):
        """Step with an LLM reply. Unparseable text counts as an illegal action."""
        action = self.parse_action(text)
        if action is None:
            self._illegal_count += 1
            obs = self._observation()
            return obs, self.illegal_reward, False, False, {"illegal": True, "illegal_count": self._illegal_count}
        return self.step(action)

    def render(self) -> str:
        return self.text_observation()

    @abstractmethod
    def _observation(self) -> Any: ...

    def _illegal(self, info: dict[str, Any] | None = None):
        """Result tuple for an illegal action: no state change, penalty, not terminal."""
        self._illegal_count += 1
        out = dict(info or {})
        out.update(illegal=True, illegal_count=self._illegal_count)
        return self._observation(), self.illegal_reward, False, False, out

    def _reset_rng(self, seed: int | None) -> None:
        super().reset(seed=seed)
        self._illegal_count = 0


def one_hot(index: np.ndarray, n: int) -> np.ndarray:
    return np.eye(n, dtype=np.int8)[index]
