"""Reading Phase 4 puzzle datasets (``water_sort_cli generate`` output, D17).

A dataset directory holds ``manifest.json`` and the record files it lists (Parquet or JSONL).
The Parquet schema is the contract; this module only reads it. Seeds and hashes are 16-digit
hex strings in JSONL and ``uint64`` in Parquet; both read back as Python ints.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path

import pyarrow as pa
import pyarrow.parquet as pq

from jepa_water_sort._native import Params, State

SPLITS = ("train", "val", "test")


@dataclass(frozen=True)
class PuzzleRecord:
    """The fields of a dataset record the trajectory tools use."""

    record_id: int
    generator_id: str
    generator_variant: str
    layout: str
    seed: int
    puzzle_code: str
    opt_moves: int
    #: Optimal solution as action indices ``from * n_tubes + to``.
    solution: tuple[int, ...]
    canonical_hash: int
    split: str
    tier: str | None

    @property
    def state(self) -> State:
        return State.from_code(self.puzzle_code)

    @property
    def puzzle_id(self) -> str:
        """``<generator_id>:<canonical hash as 16 hex digits>``."""
        return puzzle_id(self.generator_id, self.canonical_hash)


def puzzle_id(generator_id: str, canonical_hash: int) -> str:
    return f"{generator_id}:{canonical_hash:016x}"


class Dataset:
    """A dataset directory: its manifest and its records."""

    def __init__(self, path: str | Path):
        self.path = Path(path)
        manifest_path = self.path / "manifest.json"
        raw = manifest_path.read_bytes()
        self.manifest = json.loads(raw)
        #: sha256 of ``manifest.json``, recorded in trajectory manifests.
        self.manifest_sha256 = hashlib.sha256(raw).hexdigest()
        p = self.manifest["params"]
        self.params = Params(p["n_colors"], p["capacity"], p["n_empty"])
        self.generator = self.manifest["generator"]
        buckets = self.manifest["split"]["buckets"]
        self._buckets = {name: tuple(buckets[name]) for name in SPLITS}

    def split_of(self, canonical_hash: int) -> str:
        """The split the D17 rule (``canonical_hash % 100``) assigns."""
        bucket = canonical_hash % 100
        for name, (lo, hi) in self._buckets.items():
            if lo <= bucket < hi:
                return name
        raise ValueError(f"bucket {bucket} is in no split")

    def records(self, split: str | None = None) -> list[PuzzleRecord]:
        """Records in ``record_id`` order, only those of ``split`` if given. Each record's split
        is checked against the canonical-hash rule, so a trajectory set built from one split can
        never contain a puzzle of another (no leakage)."""
        if split is not None and split not in SPLITS:
            raise ValueError(f"unknown split {split!r} (expected one of {SPLITS})")
        out: list[PuzzleRecord] = []
        for f in self.manifest["files"]:
            if split is not None and f.get("split") not in (None, split):
                continue
            path = self.path / f["path"]
            rows = _read_parquet(path) if self.manifest["format"] == "parquet" else _read_jsonl(path)
            for r in rows:
                if r.split != self.split_of(r.canonical_hash):
                    raise ValueError(
                        f"record {r.record_id}: split {r.split!r} does not match its canonical hash"
                    )
                if split is None or r.split == split:
                    out.append(r)
        out.sort(key=lambda r: r.record_id)
        return out


def _record(row: dict, seed: int, canonical_hash: int) -> PuzzleRecord:
    return PuzzleRecord(
        record_id=int(row["record_id"]),
        generator_id=row["generator_id"],
        generator_variant=row["generator_variant"],
        layout=row["layout"],
        seed=seed,
        puzzle_code=row["puzzle_code"],
        opt_moves=int(row["opt_moves"]),
        solution=tuple(int(a) for a in row["solution"]),
        canonical_hash=canonical_hash,
        split=row["split"],
        tier=row.get("tier"),
    )


_COLUMNS = [
    "record_id", "generator_id", "generator_variant", "layout", "seed", "puzzle_code",
    "opt_moves", "solution", "canonical_hash", "split", "tier",
]


def _read_parquet(path: Path) -> list[PuzzleRecord]:
    table: pa.Table = pq.read_table(path, columns=_COLUMNS)
    return [_record(row, int(row["seed"]), int(row["canonical_hash"])) for row in table.to_pylist()]


def _read_jsonl(path: Path) -> list[PuzzleRecord]:
    out = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            if line.strip():
                row = json.loads(line)
                out.append(_record(row, int(row["seed"], 16), int(row["canonical_hash"], 16)))
    return out
