"""Evaluation (PLAN 7.5): episodes on fixed test puzzles, metrics, breakdowns, the matrix.

- Test puzzles are the lowest record ids of a dataset's test split (fixed puzzle codes); the
  episode rules are the environment's (``batch_env_step``: move limit ``k · opt_moves`` (D4),
  dead ends terminate). Policy randomness comes from ``EvalConfig.policy_seed``.
- Per puzzle: solved, moves, stars (``jepa_water_sort.stars``, 0 when unsolved).
- Metrics: solve rate, mean stars, mean ``moves / opt_moves`` over solved episodes
  (``ratio_solved``) and with failures counted at the move limit (``ratio_all``).
- Breakdowns by ``opt_moves`` bucket and by generator. Buckets are the quartiles of the pooled
  test puzzles of a report, so every matrix cell uses the same edges; ``matched_solve_rate``
  re-weights a cell's per-bucket solve rates to the pooled bucket distribution, so a gap between
  cells is not a difficulty difference.
- Leakage: a test puzzle whose canonical hash is among the puzzles a model trained on is
  dropped from that model's cells and counted (``leaked``).
"""

from __future__ import annotations

import math
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Protocol

import numpy as np
import pyarrow.parquet as pq
import torch

import jepa_water_sort as w
from jepa_water_sort.dataset import Dataset


class BatchPolicy(Protocol):
    name: str

    def reset(self, n: int, rng: np.random.Generator) -> None: ...

    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray: ...


@dataclass(frozen=True)
class TestPuzzle:
    record_id: int
    puzzle_code: str
    opt_moves: int
    generator_id: str
    layout: str
    canonical_hash: int
    tier: str | None


def test_puzzles(dataset_dir: str | Path, n: int) -> list[TestPuzzle]:
    """The ``n`` lowest record ids of the dataset's test split."""
    ds = Dataset(dataset_dir)
    return [
        TestPuzzle(r.record_id, r.puzzle_code, r.opt_moves, r.generator_id, r.layout,
                   r.canonical_hash, r.tier)
        for r in ds.records("test")[:n]
    ]


def trained_hashes(trajectory_dirs: Iterable[str | Path]) -> set[int]:
    """Canonical hashes of the puzzles in trajectory directories."""
    out: set[int] = set()
    for d in trajectory_dirs:
        for f in sorted(Path(d).glob("shard-*.parquet")):
            col = pq.read_table(f, columns=["canonical_hash"]).column(0)
            out.update(int(x) for x in np.unique(col.to_numpy()))
    return out


def run_episodes(policy: BatchPolicy, puzzles: list[TestPuzzle], move_limit_k: int = 4,
                 seed: int = 0, batch: int = 256) -> list[dict[str, Any]]:
    """Plays every puzzle once; returns one result dict per puzzle (in order)."""
    out: list[dict[str, Any]] = []
    for c0 in range(0, len(puzzles), batch):
        chunk = puzzles[c0:c0 + batch]
        n = len(chunk)
        rng = np.random.default_rng(np.random.SeedSequence([seed, c0]))
        torch.manual_seed(int(rng.integers(2**63)))
        policy.reset(n, rng)
        cells = np.stack([w.State.from_code(p.puzzle_code).to_numpy() for p in chunk])
        moves = np.zeros(n, dtype=np.uint32)
        limits = np.array([w.move_limit(move_limit_k, p.opt_moves) for p in chunk], dtype=np.uint32)
        active = np.ones(n, dtype=bool)
        solved = np.zeros(n, dtype=bool)
        illegal = np.zeros(n, dtype=np.uint32)
        reason = np.array(["running"] * n, dtype=object)
        while active.any():
            idx = np.flatnonzero(active)
            acts = np.asarray(policy.act(cells[idx], idx), dtype=np.int64)
            gave_up = acts < 0
            for j in idx[gave_up]:
                active[j] = False
                reason[j] = "no_action"
            go = idx[~gave_up]
            if len(go) == 0:
                continue
            nxt, _, _, term, trunc, ill, dead, sol = w.batch_env_step(
                cells[go], acts[~gave_up], moves[go], limits[go]
            )
            cells[go] = nxt
            moves[go] += 1
            illegal[go] += ill.astype(np.uint32)
            solved[go] = sol
            for j, t, tr, de, so in zip(go, term, trunc, dead, sol):
                if t or tr:
                    active[j] = False
                    reason[j] = "solved" if so else ("dead_end" if de else "truncated")
        for i, p in enumerate(chunk):
            m = int(moves[i])
            out.append({
                "record_id": p.record_id,
                "puzzle_code": p.puzzle_code,
                "generator": p.generator_id,
                "layout": p.layout,
                "opt_moves": p.opt_moves,
                "tier": p.tier,
                "solved": bool(solved[i]),
                "moves": m,
                "move_limit": int(limits[i]),
                "illegal": int(illegal[i]),
                "end": str(reason[i]),
                "stars": int(w.stars(m, p.opt_moves)) if solved[i] else 0,
            })
    return out


# -- metrics -------------------------------------------------------------------------------------


def bucket_edges(opt_moves: Iterable[int]) -> list[float]:
    """Quartile edges ``[q25, q50, q75]`` of ``opt_moves`` (bucket i: edges[i-1] < x <= edges[i])."""
    a = np.asarray(list(opt_moves), dtype=float)
    if len(a) == 0:
        return [math.inf] * 3
    return [float(np.quantile(a, q, method="lower")) for q in (0.25, 0.5, 0.75)]


def bucket_of(opt: int, edges: list[float]) -> int:
    return int(np.searchsorted(np.asarray(edges), opt, side="left"))


def bucket_labels(edges: list[float]) -> list[str]:
    """``opt_moves`` ranges of the four buckets; coinciding quartiles give an empty bucket."""
    if not all(math.isfinite(x) for x in edges):
        return ["q1", "q2", "q3", "q4"]
    e = [int(x) for x in edges]
    out = [f"≤{e[0]}"]
    for lo, hi in ((e[0] + 1, e[1]), (e[1] + 1, e[2])):
        out.append("(empty)" if lo > hi else (f"{lo}" if lo == hi else f"{lo}–{hi}"))
    out.append(f">{e[2]}")
    return out


def summarize(results: list[dict[str, Any]]) -> dict[str, float]:
    """Solve rate, mean stars, ``moves / opt_moves`` (solved only, and with failures at the
    move limit)."""
    if not results:
        return {"n": 0, "solve_rate": math.nan, "mean_stars": math.nan,
                "ratio_solved": math.nan, "ratio_all": math.nan}
    solved = np.array([r["solved"] for r in results])
    ratio = np.array([r["moves"] / r["opt_moves"] for r in results])
    limit_ratio = np.array([r["move_limit"] / r["opt_moves"] for r in results])
    return {
        "n": len(results),
        "solve_rate": float(solved.mean()),
        "mean_stars": float(np.mean([r["stars"] for r in results])),
        "ratio_solved": float(ratio[solved].mean()) if solved.any() else math.nan,
        "ratio_all": float(np.where(solved, ratio, limit_ratio).mean()),
    }


def breakdown(results: list[dict[str, Any]], edges: list[float],
              weights: list[float] | None = None) -> dict[str, Any]:
    """Overall metrics, per bucket, per generator, and the bucket-matched solve rate."""
    by_bucket = [[] for _ in range(4)]
    for r in results:
        by_bucket[bucket_of(r["opt_moves"], edges)].append(r)
    gens = sorted({r["generator"] for r in results})
    out = {
        "overall": summarize(results),
        "buckets": [summarize(b) for b in by_bucket],
        "generators": {g: summarize([r for r in results if r["generator"] == g]) for g in gens},
    }
    if weights is not None:
        rates = [b["solve_rate"] for b in out["buckets"]]
        ok = [(wt, r) for wt, r in zip(weights, rates) if not math.isnan(r) and wt > 0]
        tot = sum(wt for wt, _ in ok)
        out["matched_solve_rate"] = sum(wt * r for wt, r in ok) / tot if tot else math.nan
    return out


def bucket_weights(opt_moves: Iterable[int], edges: list[float]) -> list[float]:
    counts = np.zeros(4)
    for o in opt_moves:
        counts[bucket_of(o, edges)] += 1
    return list(counts / max(1, counts.sum()))


def mean_std(values: Iterable[float]) -> tuple[float, float]:
    v = [x for x in values if x is not None and not math.isnan(x)]
    if not v:
        return math.nan, math.nan
    return float(np.mean(v)), float(np.std(v, ddof=1)) if len(v) > 1 else 0.0
