"""The JEPA world model (PLAN 7.1).

Everything works on *tube tokens* ``(B, n_tubes, dim)``:

- :class:`Encoder` ``E``: a shared per-tube MLP over the tube's one-hot slots, then transformer
  layers across tubes **without tube position embeddings**, so ``E`` is permutation-equivariant
  over tubes (reordering the tubes reorders the tokens the same way).
- :class:`Predictor` ``P(h_t, a_t)``: the action ``(from, to)`` enters as learned *source* and
  *target* flags added to those two tube tokens, so ``P`` is equivariant too and works for any
  ``n_tubes``. It predicts the target encoder's tokens of ``s_{t+1}``; feeding its output back
  in gives multi-step latent rollouts.
- Heads: :class:`PairHead` for the inverse-dynamics model ``IDM(h_t, h_{t+1}) → a_t`` and for the
  legality head ``h_t → legal (from, to)``; :class:`Probe` (per-cell colors from tube tokens);
  :class:`GraphHead` for the solved classifier (the planning goal) and the distance-to-go value.

The pooled latent ``z`` (mean over tubes) is what the collapse monitor measures.
"""

from __future__ import annotations

import copy
import math

import torch
from torch import nn
from torch.nn import functional as F

from jepa.config import ModelConfig


def _layers(cfg: ModelConfig, n: int) -> nn.TransformerEncoder:
    layer = nn.TransformerEncoderLayer(
        cfg.dim, cfg.heads, cfg.dim * cfg.mlp_ratio, cfg.dropout,
        activation="gelu", batch_first=True, norm_first=True,
    )
    return nn.TransformerEncoder(layer, n, enable_nested_tensor=False)


def _mlp(d_in: int, d_hidden: int, d_out: int) -> nn.Sequential:
    return nn.Sequential(nn.Linear(d_in, d_hidden), nn.GELU(), nn.Linear(d_hidden, d_out))


class Encoder(nn.Module):
    def __init__(self, n_colors: int, capacity: int, cfg: ModelConfig):
        super().__init__()
        self.tube = _mlp(capacity * (n_colors + 1), cfg.dim * 2, cfg.dim)
        self.layers = _layers(cfg, cfg.encoder_layers)
        self.norm = nn.LayerNorm(cfg.dim)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        """``x``: one-hot ``(B, n_tubes, capacity, n_colors + 1)`` → tokens ``(B, n_tubes, dim)``."""
        h = self.tube(x.flatten(-2))
        return self.norm(self.layers(h))


class Predictor(nn.Module):
    def __init__(self, cfg: ModelConfig):
        super().__init__()
        self.source = nn.Parameter(torch.randn(cfg.dim) * 0.02)
        self.target = nn.Parameter(torch.randn(cfg.dim) * 0.02)
        self.inp = nn.Linear(cfg.dim, cfg.dim)
        self.layers = _layers(cfg, cfg.predictor_layers)
        self.out = nn.Sequential(nn.LayerNorm(cfg.dim), nn.Linear(cfg.dim, cfg.dim))
        self.norm = nn.LayerNorm(cfg.dim)

    def forward(self, h: torch.Tensor, action: torch.Tensor) -> torch.Tensor:
        """``h``: tokens ``(B, T, dim)``; ``action``: ``(B,)`` indices ``from * T + to``."""
        n_tubes = h.shape[1]
        src = F.one_hot(action // n_tubes, n_tubes).to(h.dtype).unsqueeze(-1)
        tgt = F.one_hot(action % n_tubes, n_tubes).to(h.dtype).unsqueeze(-1)
        x = self.inp(h) + src * self.source + tgt * self.target
        return self.norm(h + self.out(self.layers(x)))


class PairHead(nn.Module):
    """Logits over ordered tube pairs ``(B, T, T)`` (row = from, column = to) from per-tube
    features: ``s_i + t_j + <q_i, k_j> / sqrt(d)``; the diagonal is ``-inf``."""

    def __init__(self, d_in: int, dim: int):
        super().__init__()
        self.feat = _mlp(d_in, dim, dim)
        self.ctx = _layers(ModelConfig(dim=dim, heads=4), 1)
        self.src = nn.Linear(dim, 1)
        self.tgt = nn.Linear(dim, 1)
        self.q = nn.Linear(dim, dim)
        self.k = nn.Linear(dim, dim)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        u = self.ctx(self.feat(x))
        logits = (
            self.src(u) + self.tgt(u).transpose(1, 2)
            + self.q(u) @ self.k(u).transpose(1, 2) / math.sqrt(u.shape[-1])
        )
        eye = torch.eye(x.shape[1], dtype=torch.bool, device=x.device)
        return logits.masked_fill(eye, float("-inf"))


class Probe(nn.Module):
    """Per-cell color logits ``(B, T, capacity, n_colors + 1)`` from tube tokens."""

    def __init__(self, dim: int, n_colors: int, capacity: int):
        super().__init__()
        self.capacity, self.classes = capacity, n_colors + 1
        self.net = _mlp(dim, dim, capacity * (n_colors + 1))

    def forward(self, h: torch.Tensor) -> torch.Tensor:
        return self.net(h).unflatten(-1, (self.capacity, self.classes))


class GraphHead(nn.Module):
    """A scalar per state from tube tokens, through mean and max pooling."""

    def __init__(self, dim: int):
        super().__init__()
        self.tok = _mlp(dim, dim, dim)
        self.net = _mlp(2 * dim, dim, 1)

    def forward(self, h: torch.Tensor) -> torch.Tensor:
        u = self.tok(h)
        return self.net(torch.cat([u.mean(1), u.amax(1)], -1)).squeeze(-1)


class WorldModel(nn.Module):
    def __init__(self, n_colors: int, capacity: int, cfg: ModelConfig):
        super().__init__()
        self.n_colors, self.capacity, self.cfg = n_colors, capacity, cfg
        d = cfg.dim
        self.encoder = Encoder(n_colors, capacity, cfg)
        self.target_encoder = copy.deepcopy(self.encoder).requires_grad_(False)
        self.predictor = Predictor(cfg)
        self.idm = PairHead(3 * d, d)
        self.legal = PairHead(d, d)
        #: Detached monitoring probe (never sends gradients into the encoder).
        self.probe = Probe(d, n_colors, capacity)
        #: Auxiliary probe with gradients (used only when ``LossConfig.probe_aux > 0``).
        self.probe_aux = Probe(d, n_colors, capacity)
        self.solved = GraphHead(d)
        self.value = GraphHead(d)

    # -- latents -----------------------------------------------------------------------------

    def encode(self, x: torch.Tensor) -> torch.Tensor:
        return self.encoder(x)

    @torch.no_grad()
    def encode_target(self, x: torch.Tensor) -> torch.Tensor:
        return self.target_encoder(x)

    def predict(self, h: torch.Tensor, action: torch.Tensor) -> torch.Tensor:
        return self.predictor(h, action)

    @staticmethod
    def pool(h: torch.Tensor) -> torch.Tensor:
        """The pooled latent ``z`` (mean over tubes)."""
        return h.mean(1)

    # -- heads -------------------------------------------------------------------------------

    def idm_logits(self, h: torch.Tensor, h_next: torch.Tensor) -> torch.Tensor:
        """``(B, T*T)`` action logits."""
        return self.idm(torch.cat([h, h_next, h_next - h], -1)).flatten(1)

    def legal_logits(self, h: torch.Tensor) -> torch.Tensor:
        """``(B, T*T)`` legality logits (diagonal ``-inf``)."""
        return self.legal(h).flatten(1)

    def solved_logit(self, h: torch.Tensor) -> torch.Tensor:
        return self.solved(h)

    def distance(self, h: torch.Tensor) -> torch.Tensor:
        """Predicted distance to solved (moves, ≥ 0)."""
        return F.softplus(self.value(h))

    # -- EMA ---------------------------------------------------------------------------------

    @torch.no_grad()
    def update_target(self, momentum: float) -> None:
        for t, s in zip(self.target_encoder.parameters(), self.encoder.parameters()):
            t.lerp_(s, 1.0 - momentum)
        for t, s in zip(self.target_encoder.buffers(), self.encoder.buffers()):
            t.copy_(s)


def ema_momentum(step: int, total: int, start: float, end: float) -> float:
    """Cosine schedule from ``start`` to ``end`` over ``total`` steps."""
    if total <= 1:
        return end
    t = min(step / (total - 1), 1.0)
    return end - (end - start) * (math.cos(math.pi * t) + 1) / 2
