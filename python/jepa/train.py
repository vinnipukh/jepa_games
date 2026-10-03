"""Training loop (PLAN 7.1–7.2).

``python -m jepa.train --config cfg.json --out runs/x`` trains one model. The run directory gets
``config.json`` (the full :class:`~jepa.config.TrainConfig`), ``data.json`` (trajectory
manifests used, with sha256), ``metrics.jsonl`` (training losses and the per-epoch monitor),
TensorBoard event files under ``tb/``, ``last.pt`` after every epoch and ``model.pt`` plus
``final.json`` at the end. A run whose ``model.pt`` exists is not retrained.

Loss per batch (weights in :class:`~jepa.config.LossConfig`):

- latent: ``P`` rolled out k = 1..max_horizon steps from ``E(s_t)`` against
  ``sg(E_target(s_{t+k}))`` (smooth-L1 or cosine), horizon k weighted ``horizon_decay^(k-1)``;
- idm: masked cross-entropy of ``IDM(E(s_t), E(s_{t+1}))`` over the legal actions of ``s_t``;
- legal: BCE of the legality head against the real legal mask;
- probe: the detached probe's cross-entropy (always; it cannot reach the encoder) plus the
  auxiliary probe with gradients when ``probe_aux > 0``;
- solved / value: the solved classifier and the distance-to-go head on the encoded next state
  and on every predicted latent, so the planner's heads are trained on predictor outputs too.
"""

from __future__ import annotations

import argparse
import json
import math
import random
import time
from pathlib import Path
from typing import Any

import numpy as np
import torch
from torch.nn import functional as F

from jepa import data as jdata
from jepa.config import TrainConfig, load_config, save_config, to_dict
from jepa.models import WorldModel, ema_momentum
from jepa.monitor import metrics as monitor_metrics

SOLVED_POS_WEIGHT = 5.0


def pick_device(name: str) -> torch.device:
    if name != "auto":
        return torch.device(name)
    if torch.cuda.is_available():
        return torch.device("cuda")
    if getattr(torch.backends, "mps", None) is not None and torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def seed_everything(seed: int) -> None:
    random.seed(seed)
    np.random.seed(seed)
    torch.manual_seed(seed)


def _latent_loss(pred: torch.Tensor, target: torch.Tensor, kind: str) -> torch.Tensor:
    """Per-sample loss ``(B,)``."""
    if kind == "smooth_l1":
        return F.smooth_l1_loss(pred, target, reduction="none").mean((1, 2))
    if kind == "cosine":
        return (1 - F.cosine_similarity(pred, target, dim=-1)).mean(1)
    raise ValueError(f"unknown latent loss {kind!r}")


def _masked_mean(x: torch.Tensor, m: torch.Tensor) -> torch.Tensor:
    m = m.to(x.dtype)
    return (x * m).sum() / m.sum().clamp(min=1.0)


def _value_target(togo: torch.Tensor, cap: float) -> tuple[torch.Tensor, torch.Tensor]:
    known = togo != jdata.UNKNOWN
    target = torch.where(togo == jdata.UNSOLVABLE, torch.full_like(togo, cap), togo.clamp(max=cap))
    return target, known


def compute_losses(
    model: WorldModel, b: dict[str, torch.Tensor], cfg: TrainConfig
) -> tuple[torch.Tensor, dict[str, torch.Tensor]]:
    """The total loss and its detached terms (tensors, so logging costs no sync)."""
    lc = cfg.loss
    nc = model.n_colors
    B, K = b["actions"].shape
    valid = b["valid"]
    h0 = model.encode(jdata.one_hot(b["state"], nc))
    with torch.no_grad():
        targets = model.encode_target(jdata.one_hot(b["next_states"].flatten(0, 1), nc))
        targets = targets.unflatten(0, (B, K))

    logs: dict[str, torch.Tensor] = {}
    total = torch.zeros((), device=h0.device)
    latent = torch.zeros((), device=h0.device)
    rollouts = []
    hk = h0
    for k in range(K):
        hk = model.predict(hk, b["actions"][:, k])
        lk = _masked_mean(_latent_loss(hk.float(), targets[:, k].float(), lc.latent_loss), valid[:, k])
        latent = latent + lc.horizon_decay**k * lk
        logs[f"latent_k{k + 1}"] = lk.detach()
        rollouts.append((hk, k))
    total = total + lc.latent * latent
    logs["latent"] = latent.detach()

    h1 = model.encode(jdata.one_hot(b["next_states"][:, 0], nc))
    if lc.idm > 0:
        logits = model.idm_logits(h0, h1).float().masked_fill(~b["mask"], float("-inf"))
        idm = F.cross_entropy(logits, b["actions"][:, 0])
        total = total + lc.idm * idm
        logs["idm"] = idm.detach()

    if lc.legal > 0:
        logits = model.legal_logits(h0).float()
        n = h0.shape[1]
        off = ~torch.eye(n, dtype=torch.bool, device=h0.device).flatten()
        legal = F.binary_cross_entropy_with_logits(logits[:, off], b["mask"][:, off].float())
        total = total + lc.legal * legal
        logs["legal"] = legal.detach()

    cells = jdata.cell_targets(b["state"], nc)
    probe = F.cross_entropy(model.probe(h0.detach()).float().flatten(0, -2), cells.flatten())
    total = total + probe
    logs["probe"] = probe.detach()
    if lc.probe_aux > 0:
        aux = F.cross_entropy(model.probe_aux(h0).float().flatten(0, -2), cells.flatten())
        total = total + lc.probe_aux * aux
        logs["probe_aux"] = aux.detach()

    pos_weight = torch.tensor(SOLVED_POS_WEIGHT, device=h0.device)
    # Encoded states: s_t (togo) and s_{t+1}; predicted latents: s_{t+k} for valid k.
    heads_in = [(h0, None, b["togo"], torch.ones_like(valid[:, 0])),
                (h1, b["next_solved"][:, 0], b["next_togo"][:, 0], valid[:, 0])]
    heads_in += [(hk, b["next_solved"][:, k], b["next_togo"][:, k], valid[:, k])
                 for hk, k in rollouts]
    if lc.solved > 0:
        labeled = [(h, label, m) for h, label, _, m in heads_in if label is not None]
        terms = []
        for h, label, m in labeled:
            bce = F.binary_cross_entropy_with_logits(
                model.solved_logit(h).float(), label.float(), pos_weight=pos_weight, reduction="none"
            )
            term = _masked_mean(bce, m)
            total = total + lc.solved * term / len(labeled)
            terms.append(term.detach())
        logs["solved"] = torch.stack(terms).mean()
    if lc.value > 0:
        vals = []
        for h, _, togo, m in heads_in:
            target, known = _value_target(togo, lc.value_cap)
            err = F.smooth_l1_loss(model.distance(h).float(), target, reduction="none")
            v = _masked_mean(err, m & known)
            total = total + lc.value * v / len(heads_in)
            vals.append(v.detach())
        logs["value"] = torch.stack(vals).mean()
    logs["total"] = total.detach()
    return total, logs


def _lr(step: int, total: int, cfg: TrainConfig) -> float:
    if step < cfg.warmup_steps:
        return cfg.lr * (step + 1) / cfg.warmup_steps
    t = (step - cfg.warmup_steps) / max(1, total - cfg.warmup_steps)
    return cfg.lr * 0.5 * (1 + math.cos(math.pi * min(t, 1.0)))


def build_model(cfg: TrainConfig, params) -> WorldModel:
    return WorldModel(params.n_colors, params.capacity, cfg.model)


def save_checkpoint(path: Path, model: WorldModel, cfg: TrainConfig, params, extra=None) -> None:
    torch.save({
        "model": model.state_dict(),
        "config": to_dict(cfg),
        "params": [params.n_colors, params.capacity, params.n_empty],
        **(extra or {}),
    }, path)


def load_model(path: str | Path, device: str | torch.device = "cpu") -> tuple[WorldModel, TrainConfig]:
    from jepa.config import train_config_from_dict

    ckpt = torch.load(path, map_location=device, weights_only=False)
    cfg = train_config_from_dict(ckpt["config"])
    n_colors, capacity, _ = ckpt["params"]
    model = WorldModel(n_colors, capacity, cfg.model)
    model.load_state_dict(ckpt["model"])
    model.to(device).eval()
    return model, cfg


def _monitor_batch(val: jdata.DeviceData, size: int, seed: int) -> dict[str, torch.Tensor]:
    rng = np.random.default_rng(seed)
    index = np.sort(rng.choice(val.n, size=min(size, val.n), replace=False))
    return val.batch(torch.as_tensor(index, device=val.state.device))


class _Writer:
    def __init__(self, out: Path, tensorboard: bool):
        self.jsonl = (out / "metrics.jsonl").open("a", encoding="utf-8")
        self.tb = None
        if tensorboard:
            try:
                from torch.utils.tensorboard import SummaryWriter

                self.tb = SummaryWriter(str(out / "tb"))
            except ImportError:
                print("tensorboard is not installed; logging to metrics.jsonl only")

    def log(self, kind: str, step: int, values: dict[str, float]) -> None:
        self.jsonl.write(json.dumps({"kind": kind, "step": step, **values}) + "\n")
        self.jsonl.flush()
        if self.tb is not None:
            for k, v in values.items():
                if isinstance(v, (int, float)) and math.isfinite(v):
                    self.tb.add_scalar(f"{kind}/{k}", v, step)

    def close(self) -> None:
        self.jsonl.close()
        if self.tb is not None:
            self.tb.close()


def train(cfg: TrainConfig, out_dir: str | Path, *, verbose: bool = True) -> dict[str, Any]:
    """Trains one model into ``out_dir`` and returns the final monitor metrics."""
    out = Path(out_dir)
    if (out / "model.pt").exists() and (out / "final.json").exists():
        return json.loads((out / "final.json").read_text(encoding="utf-8"))
    out.mkdir(parents=True, exist_ok=True)
    seed_everything(cfg.seed)
    if cfg.num_threads:
        torch.set_num_threads(cfg.num_threads)
    device = pick_device(cfg.device)
    save_config(cfg, out / "config.json")
    t0 = time.time()
    train_t = jdata.load(cfg.data.train, cfg.data.label_max_states)
    val_t = jdata.load(cfg.data.val, cfg.data.label_max_states) if cfg.data.val else None
    if val_t is not None and val_t.split == train_t.split:
        raise ValueError(f"validation trajectories come from the training split ({train_t.split})")
    (out / "data.json").write_text(json.dumps({
        "train": jdata.manifest_summary(cfg.data.train),
        "val": jdata.manifest_summary(cfg.data.val),
        "train_transitions": len(train_t),
        "val_transitions": 0 if val_t is None else len(val_t),
        "device": str(device),
        "torch": torch.__version__,
    }, indent=2) + "\n", encoding="utf-8")
    params = train_t.params
    K = cfg.data.max_horizon
    train_d = jdata.DeviceData(train_t, device, K)
    if val_t is None:
        # No validation directories: hold out the last 5 % of training rows for monitoring only.
        cut = int(len(train_t) * 0.95)
        val_d = jdata.DeviceData(train_t.subset(np.arange(cut, len(train_t))), device, K)
    else:
        val_d = jdata.DeviceData(val_t, device, K)
    if verbose:
        print(f"loaded {train_d.n} train / {val_d.n} val transitions in {time.time() - t0:.1f} s "
              f"on {device}")
    mon = _monitor_batch(val_d, cfg.data.monitor_size, cfg.seed)

    model = build_model(cfg, params).to(device)
    trainable = [p for n, p in model.named_parameters() if not n.startswith("target_encoder.")]
    decay = [p for p in trainable if p.ndim >= 2]
    no_decay = [p for p in trainable if p.ndim < 2]
    opt = torch.optim.AdamW(
        [{"params": decay, "weight_decay": cfg.weight_decay}, {"params": no_decay, "weight_decay": 0.0}],
        lr=cfg.lr, betas=(0.9, 0.95),
    )
    steps_per_epoch = max(1, train_d.n // cfg.batch_size)
    total = cfg.epochs * steps_per_epoch
    if cfg.max_steps is not None:
        total = min(total, cfg.max_steps)
    use_amp = cfg.amp and device.type == "cuda"
    writer = _Writer(out, cfg.tensorboard)
    gen = torch.Generator(device="cpu").manual_seed(cfg.seed)
    step, epoch = 0, 0
    final: dict[str, Any] = {}
    while step < total:
        perm = torch.randperm(train_d.n, generator=gen).to(device)
        t_epoch = time.time()
        for i in range(steps_per_epoch):
            if step >= total:
                break
            idx = perm[i * cfg.batch_size:(i + 1) * cfg.batch_size]
            b = train_d.batch(idx)
            for g in opt.param_groups:
                g["lr"] = _lr(step, total, cfg)
            with torch.autocast(device.type, dtype=torch.bfloat16, enabled=use_amp):
                loss, logs = compute_losses(model, b, cfg)
            opt.zero_grad(set_to_none=True)
            loss.backward()
            gn = torch.nn.utils.clip_grad_norm_(trainable, cfg.grad_clip)
            opt.step()
            m = ema_momentum(step, total, cfg.ema_start, cfg.ema_end)
            model.update_target(m)
            step += 1
            if step % cfg.log_every == 0 or step == total:
                logs = {k: float(v) for k, v in logs.items()}
                logs.update(lr=opt.param_groups[0]["lr"], ema=m, grad_norm=float(gn))
                writer.log("train", step, logs)
                if verbose:
                    print(f"step {step}/{total} " + " ".join(
                        f"{k}={v:.4g}" for k, v in logs.items() if k in
                        ("total", "latent", "idm", "legal", "probe", "solved", "value")))
            if cfg.monitor_every and step % cfg.monitor_every == 0 and step < total:
                mid = monitor_metrics(model, mon, cfg.loss.value_cap)
                writer.log("monitor", step, mid)
                if verbose:
                    print(f"monitor @ {step}: " + " ".join(f"{k}={v:.4g}" for k, v in mid.items()))
        epoch += 1
        final = monitor_metrics(model, mon, cfg.loss.value_cap)
        final.update(epoch=epoch, step=step, epoch_secs=time.time() - t_epoch)
        writer.log("monitor", step, final)
        if verbose:
            print(f"epoch {epoch}: " + " ".join(f"{k}={v:.4g}" for k, v in final.items()))
        save_checkpoint(out / "last.pt", model, cfg, params, {"step": step, "epoch": epoch})
    writer.close()
    final["train_secs"] = time.time() - t0
    save_checkpoint(out / "model.pt", model, cfg, params, {"step": step, "epoch": epoch})
    (out / "final.json").write_text(json.dumps(final, indent=2) + "\n", encoding="utf-8")
    (out / "last.pt").unlink(missing_ok=True)
    return final


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(prog="python -m jepa.train", description=__doc__.split("\n")[0])
    p.add_argument("--config", help="TrainConfig JSON (only the fields that differ)")
    p.add_argument("--train", nargs="+", help="training trajectory directories")
    p.add_argument("--val", nargs="*", help="validation trajectory directories")
    p.add_argument("--out", required=True)
    p.add_argument("--seed", type=int)
    p.add_argument("--epochs", type=int)
    p.add_argument("--max-steps", type=int)
    p.add_argument("--device")
    a = p.parse_args(argv)
    cfg = load_config(a.config) if a.config else TrainConfig()
    if a.train:
        cfg.data.train = a.train
    if a.val is not None:
        cfg.data.val = a.val
    for name in ("seed", "epochs", "max_steps", "device"):
        if getattr(a, name) is not None:
            setattr(cfg, name, getattr(a, name))
    if not cfg.data.train:
        p.error("no training trajectories (--train or data.train in --config)")
    train(cfg, a.out)


if __name__ == "__main__":
    main()
