"""Collapse monitoring (PLAN 7.2), run on a fixed held-out batch every epoch.

- ``z_std_mean`` / ``z_std_min``: per-dimension std of the pooled latent ``z`` (collapse alert
  when the mean drops below 0.01).
- ``effective_rank``: ``exp(entropy(normalized singular values))`` of the centered ``z`` batch
  matrix (Roy & Vetterli 2007); ``effective_rank_frac`` divides by the latent dim.
- ``probe_cell_acc`` / ``probe_exact``: the detached probe's per-cell accuracy and full-state
  exact match on held-out states.
- ``idm_acc``: inverse-dynamics accuracy on held-out transitions (argmax over legal actions).
- ``legal_acc`` / ``legal_exact``: legality head per action and per state.
- ``solved_acc`` / ``solved_recall``, ``value_mae``: the planning heads.
- ``pred{k}_exact``: the probe decoding of the k-step latent rollout equals the real state after
  k steps (the world model's accuracy in state space).
"""

from __future__ import annotations

import math

import torch

from jepa.data import cell_targets, one_hot
from jepa.models import WorldModel

COLLAPSE_STD = 0.01


def effective_rank(z: torch.Tensor) -> float:
    z = z.float()
    z = z - z.mean(0, keepdim=True)
    s = torch.linalg.svdvals(z)
    p = s / s.sum().clamp(min=1e-12)
    entropy = -(p * torch.log(p.clamp(min=1e-12))).sum()
    return float(torch.exp(entropy))


@torch.no_grad()
def metrics(model: WorldModel, batch: dict[str, torch.Tensor], value_cap: float) -> dict[str, float]:
    model.eval()
    nc = model.n_colors
    out: dict[str, float] = {}
    h = model.encode(one_hot(batch["state"], nc))
    z = model.pool(h)
    std = z.float().std(0)
    out["z_std_mean"] = float(std.mean())
    out["z_std_min"] = float(std.min())
    out["effective_rank"] = effective_rank(z)
    out["effective_rank_frac"] = out["effective_rank"] / z.shape[1]
    out["token_effective_rank"] = effective_rank(h.flatten(0, 1))
    out["collapse_alert"] = float(out["z_std_mean"] < COLLAPSE_STD)

    cells = cell_targets(batch["state"], nc)
    pred = model.probe(h).argmax(-1)
    out["probe_cell_acc"] = float((pred == cells).float().mean())
    out["probe_exact"] = float((pred == cells).flatten(1).all(1).float().mean())

    nxt = batch["next_states"]
    h1 = model.encode(one_hot(nxt[:, 0], nc))
    idm = model.idm_logits(h, h1).masked_fill(~batch["mask"], float("-inf"))
    out["idm_acc"] = float((idm.argmax(1) == batch["actions"][:, 0]).float().mean())

    legal = model.legal_logits(h) > 0
    out["legal_acc"] = float((legal == batch["mask"]).float().mean())
    out["legal_exact"] = float((legal == batch["mask"]).all(1).float().mean())

    # Planning heads on the real next states (held-out batches are mostly unsolved, so the
    # recall on the solved ones is reported separately).
    solved = batch["next_solved"][:, 0]
    sp = model.solved_logit(h1) > 0
    out["solved_acc"] = float((sp == solved).float().mean())
    out["solved_recall"] = float(sp[solved].float().mean()) if solved.any() else math.nan
    togo = batch["next_togo"][:, 0]
    known = togo != -1
    if known.any():
        target = torch.where(togo < 0, torch.full_like(togo, value_cap), togo.clamp(max=value_cap))
        out["value_mae"] = float((model.distance(h1)[known] - target[known]).abs().mean())

    # Multi-step rollouts decoded by the probe.
    hk = h
    valid = batch["valid"]
    for k in range(batch["actions"].shape[1]):
        hk = model.predict(hk, batch["actions"][:, k])
        ok = valid[:, k]
        if ok.any():
            real = cell_targets(nxt[:, k], nc)
            dec = model.probe(hk).argmax(-1)
            out[f"pred{k + 1}_exact"] = float((dec == real).flatten(1).all(1)[ok].float().mean())
            if k == 0:
                sk = model.solved_logit(hk) > 0
                out["pred1_solved_acc"] = float((sk == solved).float().mean())
    model.train()
    return out
