"""Trajectory shards (Phase 5 logger output) as training tensors.

A :class:`Transitions` holds every row of one or more trajectory directories as numpy arrays:
``(s, a, s', done)`` plus what the heads need, all computed by the Rust core through
``jepa_water_sort``:

- ``mask`` / ``next_mask``: legal actions of ``s`` / ``s'`` (``batch_action_mask``).
- ``togo`` / ``next_togo``: exact distance to solved (optimal moves, ``solve``), ``-1`` when the
  solver budget ran out and ``-2`` for an unsolvable state. Labels are cached per directory in
  ``jepa-labels-<max_states>.npz``.
- ``chain``: index of the episode's next row, ``-1`` at its end, for multi-step losses.

States stay ``uint8`` cells ``(N, n_tubes, capacity)`` (255 = empty); :func:`one_hot` builds the
``(…, n_tubes, capacity, n_colors + 1)`` encoding on the device, the same encoding as
``WaterSortEnv`` observations. Trajectory splits inherit the puzzle split (the logger only
collects one dataset split per directory), and :func:`load` refuses to mix splits.
"""

from __future__ import annotations

import hashlib
import json
import multiprocessing
import os
from concurrent.futures import ProcessPoolExecutor
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import torch

import jepa_water_sort as w
from jepa_water_sort import logger

UNKNOWN = -1
UNSOLVABLE = -2


@dataclass
class Transitions:
    params: w.Params
    state: np.ndarray
    action: np.ndarray
    next_state: np.ndarray
    done: np.ndarray
    solved: np.ndarray
    mask: np.ndarray
    next_mask: np.ndarray
    togo: np.ndarray
    next_togo: np.ndarray
    chain: np.ndarray
    source: np.ndarray
    sources: tuple[str, ...]
    split: str | None

    def __len__(self) -> int:
        return len(self.action)

    def subset(self, index: np.ndarray) -> Transitions:
        """Rows ``index``; ``chain`` is remapped and links leaving the subset become ``-1``."""
        index = np.asarray(index, dtype=np.int64)
        remap = np.full(len(self) + 1, -1, dtype=np.int64)
        remap[index] = np.arange(len(index))
        chain = self.chain[index]
        chain = np.where(chain >= 0, remap[chain], -1)
        return Transitions(
            self.params, self.state[index], self.action[index], self.next_state[index],
            self.done[index], self.solved[index], self.mask[index], self.next_mask[index],
            self.togo[index], self.next_togo[index], chain, self.source[index], self.sources,
            self.split,
        )


def params_of(trajectory_dir: str | Path) -> w.Params:
    p = logger.read_manifest(trajectory_dir)["params"]
    return w.Params(p["n_colors"], p["capacity"], p["n_empty"])


def _solve_chunk(args: tuple[bytes, int, int, int]) -> np.ndarray:
    raw, n_tubes, capacity, max_states = args
    cells = np.frombuffer(raw, dtype=np.uint8).reshape(-1, n_tubes, capacity)
    out = np.empty(len(cells), dtype=np.int16)
    for i, c in enumerate(cells):
        r = w.solve(w.State.from_numpy(c), max_states)
        out[i] = r.opt_moves if r.solvable else (UNSOLVABLE if r.status == "unsolvable" else UNKNOWN)
    return out


def distance_labels(cells: np.ndarray, max_states: int, workers: int | None = None) -> np.ndarray:
    """Exact distance to solved for ``(N, n_tubes, capacity)`` cells (int16; ``UNKNOWN`` on a
    solver timeout, ``UNSOLVABLE``). Each distinct state is solved once."""
    n, n_tubes, capacity = cells.shape
    if n == 0:
        return np.empty(0, dtype=np.int16)
    flat = np.ascontiguousarray(cells.reshape(n, -1))
    keys = flat.view(np.dtype((np.void, flat.shape[1]))).ravel()
    uniq, inverse = np.unique(keys, return_inverse=True)
    uniq_cells = np.frombuffer(uniq.tobytes(), dtype=np.uint8).reshape(len(uniq), -1)
    workers = workers or max(1, min(os.cpu_count() or 1, 16))
    if len(uniq) < 20_000:  # starting worker processes costs more than solving
        workers = 1
    chunk = max(1, -(-len(uniq) // (workers * 8)))
    jobs = [
        (uniq_cells[i:i + chunk].tobytes(), n_tubes, capacity, max_states)
        for i in range(0, len(uniq), chunk)
    ]
    if workers == 1 or len(jobs) == 1:
        parts = [_solve_chunk(j) for j in jobs]
    else:
        # spawn, not fork: forking after the core's rayon pool started can deadlock (D19).
        with ProcessPoolExecutor(workers, mp_context=multiprocessing.get_context("spawn")) as pool:
            parts = list(pool.map(_solve_chunk, jobs))
    return np.concatenate(parts)[inverse.ravel()]


def _labels(trajectory_dir: Path, arrays: dict[str, np.ndarray], max_states: int):
    n = len(arrays["action"])
    if max_states <= 0:
        unknown = np.full(n, UNKNOWN, dtype=np.int16)
        return unknown, unknown.copy()
    manifest_sha = hashlib.sha256((trajectory_dir / "manifest.json").read_bytes()).hexdigest()
    path = trajectory_dir / f"jepa-labels-{max_states}.npz"
    if path.exists():
        cached = np.load(path)
        if str(cached["manifest_sha256"]) == manifest_sha and len(cached["togo"]) == n:
            return cached["togo"], cached["next_togo"]
    both = distance_labels(np.concatenate([arrays["state"], arrays["next_state"]]), max_states)
    togo, next_togo = both[:n], both[n:]
    np.savez(path, togo=togo, next_togo=next_togo, manifest_sha256=np.array(manifest_sha))
    return togo, next_togo


def load_dir(trajectory_dir: str | Path, label_max_states: int = 100_000) -> Transitions:
    """One trajectory directory."""
    d = Path(trajectory_dir)
    manifest = logger.read_manifest(d)
    params = params_of(d)
    arrays = logger.to_numpy(logger.read_transitions(d), params)
    n = len(arrays["action"])
    episode, step = arrays["episode"], arrays["step"]
    chain = np.full(n, -1, dtype=np.int64)
    if n > 1:
        nxt = (episode[1:] == episode[:-1]) & (step[1:] == step[:-1] + 1)
        chain[:-1][nxt] = np.arange(1, n)[nxt]
    togo, next_togo = _labels(d, arrays, label_max_states)
    source = manifest["source"]
    return Transitions(
        params=params,
        state=arrays["state"],
        action=arrays["action"].astype(np.int64),
        next_state=arrays["next_state"],
        done=arrays["done"],
        solved=arrays["solved"],
        mask=w.batch_action_mask(arrays["state"]),
        next_mask=w.batch_action_mask(arrays["next_state"]),
        togo=togo,
        next_togo=next_togo,
        chain=chain,
        source=np.zeros(n, dtype=np.int8),
        sources=(source,),
        split=manifest.get("split"),
    )


def concat(parts: list[Transitions]) -> Transitions:
    if not parts:
        raise ValueError("no trajectories")
    params = parts[0].params
    if any(p.params != params for p in parts):
        raise ValueError("trajectory directories have different params")
    splits = {p.split for p in parts}
    if len(splits) > 1:
        raise ValueError(f"trajectory directories come from different splits: {sorted(map(str, splits))}")
    sources: list[str] = []
    source_ids, chains, offset = [], [], 0
    for p in parts:
        ids = []
        for s in p.sources:
            if s not in sources:
                sources.append(s)
            ids.append(sources.index(s))
        source_ids.append(np.asarray(ids, dtype=np.int8)[p.source])
        chains.append(np.where(p.chain >= 0, p.chain + offset, -1))
        offset += len(p)

    def cat(name: str) -> np.ndarray:
        return np.concatenate([getattr(p, name) for p in parts])

    return Transitions(
        params, cat("state"), cat("action"), cat("next_state"), cat("done"), cat("solved"),
        cat("mask"), cat("next_mask"), cat("togo"), cat("next_togo"), np.concatenate(chains),
        np.concatenate(source_ids), tuple(sources), splits.pop(),
    )


def load(dirs: list[str | Path], label_max_states: int = 100_000) -> Transitions:
    """Several trajectory directories of one params and one split, concatenated."""
    return concat([load_dir(d, label_max_states) for d in dirs])


# -- tensors -------------------------------------------------------------------------------


def one_hot(cells: torch.Tensor, n_colors: int) -> torch.Tensor:
    """``(…, n_tubes, capacity)`` uint8 cells → float one-hot ``(…, n_tubes, capacity,
    n_colors + 1)``, channel ``n_colors`` = empty (``WaterSortEnv``'s observation)."""
    index = cells.long()
    index = torch.where(index == w.EMPTY, torch.full_like(index, n_colors), index)
    return torch.nn.functional.one_hot(index, n_colors + 1).float()


def cell_targets(cells: torch.Tensor, n_colors: int) -> torch.Tensor:
    """Cell class indices (``n_colors`` = empty) for the probe."""
    index = cells.long()
    return torch.where(index == w.EMPTY, torch.full_like(index, n_colors), index)


class DeviceData:
    """A :class:`Transitions` copied to a device once; :meth:`batch` gathers rows by index,
    including the rows of the next ``horizon - 1`` steps for multi-step losses."""

    def __init__(self, t: Transitions, device: torch.device, horizon: int):
        self.n = len(t)
        self.params = t.params
        self.horizon = horizon
        dev = device

        def tensor(a: np.ndarray, dtype=None) -> torch.Tensor:
            return torch.as_tensor(np.ascontiguousarray(a), dtype=dtype).to(dev)

        self.state = tensor(t.state)
        self.next_state = tensor(t.next_state)
        self.action = tensor(t.action, torch.long)
        self.solved = tensor(t.solved, torch.bool)
        self.mask = tensor(t.mask, torch.bool)
        self.next_mask = tensor(t.next_mask, torch.bool)
        self.togo = tensor(t.togo.astype(np.float32))
        self.next_togo = tensor(t.next_togo.astype(np.float32))
        self.chain = tensor(t.chain, torch.long)

    def batch(self, index: torch.Tensor) -> dict[str, torch.Tensor]:
        """``rows[k]`` is the row of step k (k = 0..horizon-1) after ``index``; ``valid[:, k]``
        says whether it exists in the same episode."""
        rows = [index]
        valid = [torch.ones_like(index, dtype=torch.bool)]
        cur = index
        for _ in range(1, self.horizon):
            nxt = self.chain[cur.clamp(min=0)]
            ok = valid[-1] & (nxt >= 0)
            cur = torch.where(ok, nxt, torch.zeros_like(nxt))
            rows.append(cur)
            valid.append(ok)
        r = torch.stack(rows, 1)
        return {
            "state": self.state[index],
            "mask": self.mask[index],
            "togo": self.togo[index],
            "actions": self.action[r],
            "next_states": self.next_state[r],
            "next_masks": self.next_mask[r],
            "next_solved": self.solved[r],
            "next_togo": self.next_togo[r],
            "valid": torch.stack(valid, 1),
        }


def manifest_summary(dirs: list[str | Path]) -> list[dict]:
    """What a run trained on: path, source, ε, split, transitions, manifest sha256."""
    out = []
    for d in dirs:
        raw = (Path(d) / "manifest.json").read_bytes()
        m = json.loads(raw)
        out.append({
            "path": str(d), "source": m.get("source"), "epsilon": m.get("epsilon"),
            "split": m.get("split"), "transitions": m.get("transitions"),
            "manifest_sha256": hashlib.sha256(raw).hexdigest(),
        })
    return out
