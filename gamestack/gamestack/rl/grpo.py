"""A small, dependency-light GRPO for logit policies (classifiers, LLM action scorers).

For each prompt (seed) sample ``group_size`` episodes; advantage = (return - group mean) / group std;
loss = -mean(clipped-ratio * advantage) + beta * KL(policy || reference) over the sampled actions.
The ratio uses the log-probs from the rollout policy (one policy update per rollout batch by
default, so ratio starts at 1; ``epochs > 1`` reuses the batch and the clip then matters).

This is for the stack's own experiments and tests. For large LLM runs use the same environments
through TRL / verl / slime / PRIME-RL (adapters: planned) and keep this as the reference.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable

import torch

from gamestack.core import GameEnv
from gamestack.rl.policy import LogitPolicy, masked_logits
from gamestack.rl.rollout import Episode, collect


@dataclass
class GRPOConfig:
    group_size: int = 8
    prompts_per_step: int = 16
    epochs: int = 1
    clip: float = 0.2
    beta: float = 0.0          # KL weight to the frozen reference (needs ``ref_policy``)
    temperature: float = 1.0
    max_steps: int = 200
    lr: float = 1e-3
    grad_clip: float = 1.0


def advantages(eps: list[Episode], group_size: int) -> list[float]:
    out = []
    for g in range(0, len(eps), group_size):
        grp = eps[g:g + group_size]
        r = torch.tensor([e.ret for e in grp])
        a = (r - r.mean()) / (r.std(unbiased=False) + 1e-4)
        out += a.tolist()
    return out


def _flatten(eps: list[Episode], adv: list[float]):
    prompts, legal, actions, advs = [], [], [], []
    for e, a in zip(eps, adv):
        for s in e.steps:
            prompts.append(s.prompt); legal.append(s.legal); actions.append(s.action); advs.append(a)
    return prompts, legal, torch.tensor(actions), torch.tensor(advs)


def grpo_step(policy: LogitPolicy, optimizer: torch.optim.Optimizer, env_fn: Callable[[], GameEnv], seeds: list[int],
              cfg: GRPOConfig, ref_policy: LogitPolicy | None = None) -> dict[str, float]:
    eps = collect(env_fn, policy, seeds, cfg.group_size, cfg.max_steps, cfg.temperature)
    adv = advantages(eps, cfg.group_size)
    prompts, legal, actions, advs = _flatten(eps, adv)
    with torch.no_grad():
        old = torch.log_softmax(masked_logits(policy(prompts).float(), legal) / cfg.temperature, -1).gather(1, actions[:, None])[:, 0]
        ref = (torch.log_softmax(masked_logits(ref_policy(prompts).float(), legal), -1) if ref_policy and cfg.beta > 0 else None)
    loss_value = 0.0
    for _ in range(cfg.epochs):
        logp_all = torch.log_softmax(masked_logits(policy(prompts).float(), legal) / cfg.temperature, -1)
        logp = logp_all.gather(1, actions[:, None])[:, 0]
        ratio = (logp - old).exp()
        loss = -torch.min(ratio * advs, ratio.clamp(1 - cfg.clip, 1 + cfg.clip) * advs).mean()
        if ref is not None:  # k3 estimator of KL on the sampled action
            d = ref.gather(1, actions[:, None])[:, 0] - logp
            loss = loss + cfg.beta * (d.exp() - d - 1).mean()
        optimizer.zero_grad()
        loss.backward()
        params = [p for g in optimizer.param_groups for p in g["params"]]
        torch.nn.utils.clip_grad_norm_(params, cfg.grad_clip)
        optimizer.step()
        loss_value = loss.item()
    returns = torch.tensor([e.ret for e in eps])
    return {"loss": loss_value, "mean_return": float(returns.mean()), "n_steps": len(prompts)}
