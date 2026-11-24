"""Calibration metrics and post-hoc temperature scaling for decision models.

The Laya model card reports that its checkpoints ship over-confident until one temperature per
(question type, option count) is refitted on the user's own data. ``fit_temperature`` does that.
"""

from __future__ import annotations

from typing import Callable, Sequence

import torch
from torch import Tensor

from gamestack.core import GameEnv
from gamestack.rl.policy import LogitPolicy, masked_logits
from gamestack.rl.rollout import prompt_of


@torch.no_grad()
def predict_probs(policy: LogitPolicy, env_fn: Callable[[], GameEnv], seeds: Sequence[int]) -> tuple[Tensor, Tensor]:
    """Logits (legal-masked) and true labels on ``seeds``. Needs ``info['label']`` from ``reset``."""
    prompts, legal, labels = [], [], []
    for s in seeds:
        env = env_fn()
        _, info = env.reset(seed=s)
        prompts.append(prompt_of(env)); legal.append(env.legal_actions()); labels.append(info["label"])
    return masked_logits(policy(prompts).float(), legal), torch.tensor(labels)


def ece(probs: Tensor, labels: Tensor, bins: int = 15) -> float:
    conf, pred = probs.max(-1)
    hit = (pred == labels).float()
    edges = torch.linspace(0, 1, bins + 1)
    total = 0.0
    for lo, hi in zip(edges[:-1], edges[1:]):
        m = (conf > lo) & (conf <= hi)
        if m.any():
            total += m.float().mean().item() * abs(hit[m].mean().item() - conf[m].mean().item())
    return total


def brier(probs: Tensor, labels: Tensor) -> float:
    onehot = torch.nn.functional.one_hot(labels, probs.shape[-1]).float()
    return ((probs - onehot) ** 2).sum(-1).mean().item()


def nll(probs: Tensor, labels: Tensor) -> float:
    return -probs.gather(-1, labels[:, None]).clamp_min(1e-9).log().mean().item()


def fit_temperature(logits: Tensor, labels: Tensor, iters: int = 200) -> float:
    """Temperature T minimising NLL of softmax(logits / T) (finite logits only)."""
    finite = torch.isfinite(logits)
    z = logits.masked_fill(~finite, -1e9)
    log_t = torch.zeros((), requires_grad=True)
    opt = torch.optim.LBFGS([log_t], lr=0.5, max_iter=iters)

    def closure():
        opt.zero_grad()
        loss = torch.nn.functional.cross_entropy(z / log_t.exp(), labels)
        loss.backward()
        return loss

    opt.step(closure)
    return float(log_t.detach().exp())


def selective_accuracy(probs: Tensor, labels: Tensor, risk: float = 0.1) -> dict[str, float]:
    """Abstain when confidence is below the lowest threshold whose answered-set error is <= ``risk``."""
    conf, pred = probs.max(-1)
    hit = (pred == labels).float()
    order = conf.argsort(descending=True)
    cum_err = 1 - hit[order].cumsum(0) / torch.arange(1, len(hit) + 1)
    ok = (cum_err <= risk).nonzero()
    if len(ok) == 0:
        return {"coverage": 0.0, "accuracy": float("nan"), "threshold": 1.0}
    k = int(ok.max()) + 1
    return {"coverage": k / len(hit), "accuracy": float(hit[order][:k].mean()), "threshold": float(conf[order][k - 1])}
