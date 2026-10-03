"""DQN / DDQN baseline with action masking (PLAN 7.4).

- Q-network: the same permutation-equivariant tube encoder as the world model, with a pair head
  giving ``Q(s, from → to)``; illegal actions are masked in the argmax and in the targets.
- Environment: the dataset's train puzzles, stepped with ``batch_env_step`` (the Phase 5.2
  rules: −1 per action, optional potential shaping, move limit ``k · opt_moves``, dead ends
  terminate). Truncation does not cut the bootstrap; termination does.
- ``double=True`` (default) is DDQN: the online network picks ``a*`` in ``s'``, the target
  network evaluates it.

``python -m jepa.dqn --dataset DIR --out DIR`` trains; :class:`DQNPolicy` plays.
"""

from __future__ import annotations

import argparse
import json
import math
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path

import numpy as np
import torch
from torch import nn
from torch.nn import functional as F

import jepa_water_sort as w
from jepa.config import ModelConfig
from jepa.data import one_hot
from jepa.models import Encoder, PairHead
from jepa.train import pick_device, seed_everything
from jepa_water_sort.dataset import Dataset


@dataclass
class DQNConfig:
    dataset: str = ""
    seed: int = 0
    model: ModelConfig = field(default_factory=lambda: ModelConfig(dim=128, encoder_layers=3))
    double: bool = True
    shaping: bool = True
    gamma: float = 0.99
    move_limit_k: int = 4
    envs: int = 256
    total_steps: int = 2_000_000
    buffer: int = 500_000
    batch_size: int = 512
    lr: float = 2.5e-4
    learning_starts: int = 20_000
    #: Sampled transitions per collected transition (updates per iteration = envs · ratio / batch).
    replay_ratio: float = 4.0
    target_every: int = 2_000
    eps_start: float = 1.0
    eps_end: float = 0.05
    eps_fraction: float = 0.3
    device: str = "auto"
    log_every: int = 10_000


class QNet(nn.Module):
    def __init__(self, n_colors: int, capacity: int, cfg: ModelConfig):
        super().__init__()
        self.n_colors = n_colors
        self.encoder = Encoder(n_colors, capacity, cfg)
        self.head = PairHead(cfg.dim, cfg.dim)

    def forward(self, cells: torch.Tensor) -> torch.Tensor:
        return self.head(self.encoder(one_hot(cells, self.n_colors))).flatten(1)


def masked_argmax(q: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
    return q.masked_fill(~mask, -math.inf).argmax(1)


class _Envs:
    """``n`` episodes on the dataset's train puzzles, reset in a seeded random order."""

    def __init__(self, ds: Dataset, n: int, k: int, rng: np.random.Generator, shaping_gamma):
        self.records = ds.records("train")
        self.k, self.rng, self.gamma = k, rng, shaping_gamma
        p = ds.params
        self.cells = np.zeros((n, p.n_tubes, p.capacity), dtype=np.uint8)
        self.moves = np.zeros(n, dtype=np.uint32)
        self.limits = np.zeros(n, dtype=np.uint32)
        self.returns = np.zeros(n)
        for i in range(n):
            self._reset(i)

    def _reset(self, i: int) -> None:
        r = self.records[int(self.rng.integers(len(self.records)))]
        self.cells[i] = r.state.to_numpy()
        self.moves[i] = 0
        self.limits[i] = w.move_limit(self.k, r.opt_moves)
        self.returns[i] = 0.0

    def step(self, actions: np.ndarray):
        nxt, _, rew, term, trunc, _, _, solved = w.batch_env_step(
            self.cells, actions, self.moves, self.limits, self.gamma
        )
        prev = self.cells.copy()
        self.cells = nxt.copy()
        self.moves += 1
        self.returns += rew
        finished = []
        for i in np.flatnonzero(term | trunc):
            finished.append((bool(solved[i]), self.returns[i]))
            self._reset(i)
        return prev, nxt, rew, term, trunc, finished


def train_dqn(cfg: DQNConfig, out_dir: str | Path, verbose: bool = True) -> dict:
    out = Path(out_dir)
    if (out / "dqn.pt").exists() and (out / "final.json").exists():
        return json.loads((out / "final.json").read_text(encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    (out / "config.json").write_text(json.dumps(asdict(cfg), indent=2) + "\n", encoding="utf-8")
    seed_everything(cfg.seed)
    device = pick_device(cfg.device)
    ds = Dataset(cfg.dataset)
    p = ds.params
    rng = np.random.default_rng(cfg.seed)
    envs = _Envs(ds, cfg.envs, cfg.move_limit_k, rng, cfg.gamma if cfg.shaping else None)
    q = QNet(p.n_colors, p.capacity, cfg.model).to(device)
    q_t = QNet(p.n_colors, p.capacity, cfg.model).to(device)
    q_t.load_state_dict(q.state_dict())
    opt = torch.optim.Adam(q.parameters(), lr=cfg.lr)
    N, T, C = cfg.buffer, p.n_tubes, p.capacity
    buf_s = torch.zeros(N, T, C, dtype=torch.uint8, device=device)
    buf_s2 = torch.zeros(N, T, C, dtype=torch.uint8, device=device)
    buf_a = torch.zeros(N, dtype=torch.long, device=device)
    buf_r = torch.zeros(N, device=device)
    buf_d = torch.zeros(N, device=device)
    buf_m2 = torch.zeros(N, T * T, dtype=torch.bool, device=device)
    pos, size = 0, 0
    log = (out / "metrics.jsonl").open("w", encoding="utf-8")
    recent: list[tuple[bool, float]] = []
    t0 = time.time()
    steps = 0
    next_log = cfg.log_every
    updates = 0
    while steps < cfg.total_steps:
        frac = min(1.0, steps / max(1, cfg.eps_fraction * cfg.total_steps))
        eps = cfg.eps_start + frac * (cfg.eps_end - cfg.eps_start)
        mask = w.batch_action_mask(envs.cells)
        with torch.no_grad():
            qa = q(torch.as_tensor(envs.cells, device=device))
            greedy = masked_argmax(qa, torch.as_tensor(mask, device=device)).cpu().numpy()
        explore = rng.random(cfg.envs) < eps
        actions = greedy.copy()
        for i in np.flatnonzero(explore):
            legal = np.flatnonzero(mask[i])
            actions[i] = legal[rng.integers(len(legal))] if len(legal) else 0
        prev, nxt, rew, term, trunc, finished = envs.step(actions)
        recent.extend(finished)
        n = len(actions)
        idx = (torch.arange(n, device=device) + pos) % N
        buf_s[idx] = torch.as_tensor(prev, device=device)
        buf_s2[idx] = torch.as_tensor(nxt, device=device)
        buf_a[idx] = torch.as_tensor(actions, device=device)
        buf_r[idx] = torch.as_tensor(rew, dtype=torch.float32, device=device)
        buf_d[idx] = torch.as_tensor(term, dtype=torch.float32, device=device)
        buf_m2[idx] = torch.as_tensor(w.batch_action_mask(nxt), device=device)
        pos = (pos + n) % N
        size = min(N, size + n)
        steps += n
        if steps >= cfg.learning_starts:
            for _ in range(max(1, round(n * cfg.replay_ratio / cfg.batch_size))):
                b = torch.randint(0, size, (cfg.batch_size,), device=device)
                with torch.no_grad():
                    m2 = buf_m2[b]
                    q_next_t = q_t(buf_s2[b])
                    if cfg.double:
                        a2 = masked_argmax(q(buf_s2[b]), m2)
                    else:
                        a2 = masked_argmax(q_next_t, m2)
                    v2 = q_next_t.gather(1, a2[:, None]).squeeze(1)
                    v2 = torch.where(m2.any(1), v2, torch.zeros_like(v2))
                    target = buf_r[b] + cfg.gamma * (1 - buf_d[b]) * v2
                pred = q(buf_s[b]).gather(1, buf_a[b][:, None]).squeeze(1)
                loss = F.smooth_l1_loss(pred, target)
                opt.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(q.parameters(), 10.0)
                opt.step()
                updates += 1
                if updates % cfg.target_every == 0:
                    q_t.load_state_dict(q.state_dict())
        if steps >= next_log:
            next_log += cfg.log_every
            solved = [s for s, _ in recent[-1000:]]
            rec = {"steps": steps, "eps": eps, "updates": updates,
                   "episodes": len(recent),
                   "solve_rate_recent": float(np.mean(solved)) if solved else None,
                   "secs": time.time() - t0}
            log.write(json.dumps(rec) + "\n")
            log.flush()
            if verbose:
                print(" ".join(f"{k}={v}" for k, v in rec.items()))
    log.close()
    torch.save({"model": q.state_dict(), "config": asdict(cfg),
                "params": [p.n_colors, p.capacity, p.n_empty]}, out / "dqn.pt")
    solved = [s for s, _ in recent[-1000:]]
    final = {"steps": steps, "updates": updates, "train_secs": time.time() - t0,
             "solve_rate_recent": float(np.mean(solved)) if solved else None}
    (out / "final.json").write_text(json.dumps(final, indent=2) + "\n", encoding="utf-8")
    return final


class DQNPolicy:
    """Masked greedy play with a trained Q-network."""

    def __init__(self, path: str | Path, device: str | torch.device = "cpu"):
        ckpt = torch.load(path, map_location=device, weights_only=False)
        n_colors, capacity, _ = ckpt["params"]
        mcfg = ModelConfig(**ckpt["config"]["model"])
        self.device = torch.device(device)
        self.q = QNet(n_colors, capacity, mcfg).to(device).eval()
        self.q.load_state_dict(ckpt["model"])
        self.name = "ddqn" if ckpt["config"].get("double", True) else "dqn"

    def reset(self, n: int, rng: np.random.Generator) -> None:
        pass

    @torch.no_grad()
    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray:
        mask = torch.as_tensor(w.batch_action_mask(cells), device=self.device)
        a = masked_argmax(self.q(torch.as_tensor(cells, device=self.device)), mask).cpu().numpy()
        a[~mask.any(1).cpu().numpy()] = -1
        return a


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(prog="python -m jepa.dqn", description=__doc__.split("\n")[0])
    ap.add_argument("--dataset", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--seed", type=int, default=0)
    ap.add_argument("--total-steps", type=int)
    ap.add_argument("--no-double", action="store_true")
    ap.add_argument("--device", default="auto")
    a = ap.parse_args(argv)
    cfg = DQNConfig(dataset=a.dataset, seed=a.seed, double=not a.no_double, device=a.device)
    if a.total_steps:
        cfg.total_steps = a.total_steps
    train_dqn(cfg, a.out)


if __name__ == "__main__":
    main()
