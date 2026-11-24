"""RLCD-style training: reinforcement learning for *calibrated* decisions.

Recipe (as described on the Laya model card, convaiinnovations/laya; implemented here from that
description, not from its code):

* the policy *reports a distribution* over the options (softmax of its logits),
* exploration adds zero-mean Gaussian noise to the logits,
* the reward is a strictly proper scoring rule of the reported distribution against the true
  outcome (log + spherical; ranked probability score for ordinal questions), so expected reward is
  maximised only by reporting honest probabilities,
* update = REINFORCE with a group-mean baseline (GRPO-style).

For a Gaussian exploration policy ``a = z + eps``, ``eps ~ N(0, sigma^2)``, the score function is
``grad log pi(a | z) = eps / sigma^2 * dz/dtheta``, which is what ``rlcd_step`` differentiates.
Environments must expose the true class: ``reset`` returns ``info["label"]`` (``classify`` does) and
ordinal tasks set ``env.ordinal = True``.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Sequence

import torch
from torch import Tensor

from gamestack.core import GameEnv
from gamestack.rl.policy import LogitPolicy, masked_logits
from gamestack.rl.rollout import prompt_of

EPS = 1e-6


# ---- strictly proper scoring rules (higher is better) --------------------------------------
def log_score(p: Tensor, y: Tensor) -> Tensor:
    return p.gather(-1, y[..., None])[..., 0].clamp_min(EPS).log()


def spherical_score(p: Tensor, y: Tensor) -> Tensor:
    return p.gather(-1, y[..., None])[..., 0] / p.norm(dim=-1).clamp_min(EPS)


def rps_score(p: Tensor, y: Tensor) -> Tensor:
    """1 - ranked probability score, for ordered options: wrong-by-one beats wrong-by-three."""
    k = p.shape[-1]
    onehot = torch.nn.functional.one_hot(y, k).to(p.dtype)
    diff = p.cumsum(-1) - onehot.cumsum(-1)
    return 1.0 - (diff[..., :-1] ** 2).sum(-1) / max(k - 1, 1)


def score(p: Tensor, y: Tensor, ordinal: bool = False, w_log: float = 0.5, w_sph: float = 0.5, w_rps: float = 1.0) -> Tensor:
    """Combined reward. ``log`` is rescaled to roughly [-1, 0] with ``/ log(k)`` so the terms are comparable."""
    k = p.shape[-1]
    r = w_log * log_score(p, y) / max(torch.log(torch.tensor(float(k))), 1.0) + w_sph * spherical_score(p, y)
    if ordinal:
        r = r + w_rps * rps_score(p, y)
    return r


# ---- training ------------------------------------------------------------------------------
@dataclass
class RLCDConfig:
    group_size: int = 8
    sigma: float = 0.5            # std of the exploration noise added to the logits
    lr: float = 1e-3
    grad_clip: float = 1.0
    w_log: float = 0.5
    w_sph: float = 0.5
    w_rps: float = 1.0


def _batch(env_fn: Callable[[], GameEnv], seeds: Sequence[int], group_size: int):
    prompts, legal, labels, ordinal = [], [], [], False
    for seed in seeds:
        env = env_fn()
        _, info = env.reset(seed=seed)
        ordinal = bool(getattr(env, "ordinal", False))
        p, lg = prompt_of(env), env.legal_actions()
        prompts += [p] * group_size
        legal += [lg] * group_size
        labels += [info["label"]] * group_size
    return prompts, legal, torch.tensor(labels), ordinal


def rlcd_step(policy: LogitPolicy, optimizer: torch.optim.Optimizer, env_fn: Callable[[], GameEnv], seeds: Sequence[int],
              cfg: RLCDConfig = RLCDConfig()) -> dict[str, float]:
    prompts, legal, labels, ordinal = _batch(env_fn, seeds, cfg.group_size)
    logits = masked_logits(policy(prompts).float(), legal)
    finite = torch.isfinite(logits)
    noise = torch.randn_like(logits) * cfg.sigma * finite
    with torch.no_grad():
        reported = torch.softmax(logits.detach() + noise, -1)
        reward = score(reported, labels, ordinal, cfg.w_log, cfg.w_sph, cfg.w_rps)
        g = reward.view(-1, cfg.group_size)
        adv = ((g - g.mean(1, keepdim=True)) / (g.std(1, unbiased=False, keepdim=True) + 1e-4)).view(-1)
    # grad log pi = eps / sigma^2 * dz/dtheta  (Gaussian exploration policy on the logits)
    surrogate = (noise.detach() / cfg.sigma**2 * logits.masked_fill(~finite, 0.0)).sum(-1)
    loss = -(adv * surrogate).mean()
    optimizer.zero_grad()
    loss.backward()
    torch.nn.utils.clip_grad_norm_([p for gr in optimizer.param_groups for p in gr["params"]], cfg.grad_clip)
    optimizer.step()
    with torch.no_grad():
        clean = torch.softmax(logits.detach(), -1)
        acc = (clean.argmax(-1) == labels).float().mean().item()
    return {"loss": loss.item(), "mean_reward": reward.mean().item(), "accuracy": acc}
