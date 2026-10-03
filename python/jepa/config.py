"""Run configuration: plain dataclasses, saved as ``config.json`` next to every run.

A run is fully described by its :class:`TrainConfig` (data, model, optimisation, seed); the
planner and evaluation settings are :class:`PlanConfig` and :class:`EvalConfig`. Every field
has a default, so ``TrainConfig()`` is the default configuration and a JSON file only needs the
fields it changes (:func:`load_config`).
"""

from __future__ import annotations

import dataclasses
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, TypeVar

T = TypeVar("T")


@dataclass
class DataConfig:
    #: Trajectory directories (``jepa_water_sort.logger collect`` output) used for training.
    train: list[str] = field(default_factory=list)
    #: Trajectory directories for held-out monitoring (collected on the dataset's val split).
    val: list[str] = field(default_factory=list)
    #: Longest multi-step prediction horizon k (the loss uses k = 1..max_horizon).
    max_horizon: int = 3
    #: Solver budget for the distance-to-go labels (``togo``); 0 disables the labels.
    label_max_states: int = 100_000
    #: Held-out states/transitions used by the monitor every epoch.
    monitor_size: int = 4096


@dataclass
class ModelConfig:
    #: Width of the tube tokens.
    dim: int = 128
    #: Transformer layers across tubes in the encoder and the predictor.
    encoder_layers: int = 3
    predictor_layers: int = 3
    heads: int = 4
    #: Feed-forward width as a multiple of ``dim``.
    mlp_ratio: int = 4
    dropout: float = 0.0


@dataclass
class LossConfig:
    #: Weights of the loss terms. ``probe_aux`` > 0 trains the probe with gradients into the
    #: encoder (the ablation flag of PLAN 7.1); the monitored probe is always a detached copy.
    latent: float = 1.0
    idm: float = 0.1
    probe_aux: float = 0.0
    solved: float = 0.1
    legal: float = 0.1
    value: float = 0.1
    #: Weight of horizon k in the multi-step loss is ``horizon_decay ** (k - 1)``.
    horizon_decay: float = 0.7
    #: ``smooth_l1`` or ``cosine`` between the predicted and the target tube tokens.
    latent_loss: str = "smooth_l1"
    #: Distance labels above this are clipped (unsolvable states get ``value_cap``).
    value_cap: float = 40.0


@dataclass
class TrainConfig:
    name: str = "jepa"
    seed: int = 0
    data: DataConfig = field(default_factory=DataConfig)
    model: ModelConfig = field(default_factory=ModelConfig)
    loss: LossConfig = field(default_factory=LossConfig)
    batch_size: int = 512
    epochs: int = 20
    #: If set, stops after this many optimizer steps (smoke tests).
    max_steps: int | None = None
    lr: float = 3e-4
    weight_decay: float = 0.05
    warmup_steps: int = 500
    grad_clip: float = 1.0
    #: EMA momentum of the target encoder, cosine schedule from ``ema_start`` to ``ema_end``.
    ema_start: float = 0.996
    ema_end: float = 1.0
    #: ``auto`` picks cuda, then mps, then cpu.
    device: str = "auto"
    #: Mixed precision (bf16 autocast) on CUDA.
    amp: bool = True
    num_threads: int | None = None
    log_every: int = 50
    #: Also run the monitor every this many steps (it always runs at the end of an epoch).
    monitor_every: int | None = None
    tensorboard: bool = True


@dataclass
class PlanConfig:
    #: ``beam`` (default), ``mcts`` or ``cem``.
    method: str = "beam"
    #: Leaf score: ``value`` (learned distance-to-go, f = depth + h) or ``solved`` (the solved
    #: head's probability, the plan's primary goal).
    score: str = "value"
    #: Legality below the root: ``head`` (learned legality head), ``probe`` (core rules on the
    #: probe-decoded state) or ``none`` (every from != to pair).
    legality: str = "head"
    depth: int = 4
    width: int = 16
    #: Never step back into a state already visited in the real episode (checked at the root
    #: with the real rules), unless no other move is left.
    avoid_revisits: bool = True
    mcts_simulations: int = 64
    mcts_c: float = 1.5
    cem_samples: int = 256
    cem_elites: int = 32
    cem_iters: int = 3
    #: A latent counts as solved when the solved head's probability exceeds this.
    solved_threshold: float = 0.5


@dataclass
class EvalConfig:
    #: Test puzzles per test dataset (lowest record ids of its test split).
    puzzles: int = 500
    move_limit_k: int = 4
    policy_seed: int = 0
    #: Puzzles planned together in one batch.
    batch: int = 256


def to_dict(cfg: Any) -> dict[str, Any]:
    return asdict(cfg)


def _from_dict(cls: type[T], data: dict[str, Any]) -> T:
    kwargs = {}
    names = {f.name: f for f in dataclasses.fields(cls)}
    for key, value in data.items():
        if key not in names:
            raise ValueError(f"{cls.__name__} has no field {key!r}")
        ftype = names[key].type
        sub = _SUBCONFIGS.get(ftype if isinstance(ftype, str) else getattr(ftype, "__name__", ""))
        kwargs[key] = _from_dict(sub, value) if sub is not None and isinstance(value, dict) else value
    return cls(**kwargs)


_SUBCONFIGS: dict[str, type] = {
    "DataConfig": DataConfig,
    "ModelConfig": ModelConfig,
    "LossConfig": LossConfig,
}


def train_config_from_dict(data: dict[str, Any]) -> TrainConfig:
    return _from_dict(TrainConfig, data)


def plan_config_from_dict(data: dict[str, Any]) -> PlanConfig:
    return _from_dict(PlanConfig, data)


def save_config(cfg: Any, path: str | Path) -> None:
    Path(path).write_text(json.dumps(to_dict(cfg), indent=2) + "\n", encoding="utf-8")


def load_config(path: str | Path) -> TrainConfig:
    return train_config_from_dict(json.loads(Path(path).read_text(encoding="utf-8")))
