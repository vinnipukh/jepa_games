"""Collect rollouts from ``GameEnv`` batches with a ``LogitPolicy``.

Group sampling: ``collect(..., group_size=G)`` runs G environments on the *same seed* (games are
deterministic per seed), the layout GRPO needs: G answers to one prompt, compared with each other.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Callable

import torch

from gamestack.core import GameEnv
from gamestack.rl.policy import LogitPolicy, masked_logits, sample


@dataclass
class Step:
    prompt: str
    legal: list[int]
    action: int
    reward: float


@dataclass
class Episode:
    seed: int
    group: int
    steps: list[Step] = field(default_factory=list)

    @property
    def ret(self) -> float:
        return sum(s.reward for s in self.steps)


def prompt_of(env: GameEnv) -> str:
    return f"{env.action_prompt()}\n\n{env.text_observation()}"


@torch.no_grad()
def collect(env_fn: Callable[[], GameEnv], policy: LogitPolicy, seeds: list[int], group_size: int = 1,
            max_steps: int = 200, temperature: float = 1.0, greedy: bool = False) -> list[Episode]:
    envs, eps = [], []
    for g, seed in enumerate(seeds):
        for _ in range(group_size):
            env = env_fn()
            env.reset(seed=seed)
            envs.append(env)
            eps.append(Episode(seed, g))
    active = list(range(len(envs)))
    for _ in range(max_steps):
        if not active:
            break
        prompts = [prompt_of(envs[i]) for i in active]
        legal = [envs[i].legal_actions() for i in active]
        logits = masked_logits(policy(prompts).float(), legal)
        actions = sample(logits, temperature, greedy).tolist()
        still = []
        for i, p, lg, a in zip(active, prompts, legal, actions):
            _, r, term, trunc, _ = envs[i].step(a)
            eps[i].steps.append(Step(p, lg, a, r))
            if not (term or trunc):
                still.append(i)
        active = still
    return eps
