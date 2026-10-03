"""Trajectory logger (Phase 5.3): JEPA training transitions as Parquet shards.

One row per transition::

    puzzle_id, generator_id, canonical_hash, record_id, split, source, episode, step,
    state, action, next_state, done, truncated, illegal, solved

``state`` / ``next_state`` are ``fixed_size_binary(n_tubes * capacity)``, the dataset ``state``
encoding (row-major, bottom to top, 255 = empty); the one-hot observation is rebuilt at load
(:func:`~jepa_water_sort.env.encode_observation`). ``puzzle_id`` is
``<generator_id>:<canonical hash as 16 hex digits>``. ``done`` is ``terminated or truncated``.

Sources: ``optimal`` (solver solution), ``random`` (uniform over legal moves), ``epsilon``
(optimal with probability 1 − ε, else random legal, re-solving after a deviation), ``greedy``
(one-step heuristic lookahead) and ``human`` (imported play, Phase 6). Collection from a
dataset uses only the puzzles of one split, so trajectory splits inherit the puzzle split.

A directory holds ``shard-00000.parquet``, ... (about ``shard_size`` transitions each, never
splitting an episode) and ``manifest.json`` (source, ε, policy seed, dataset manifest hash,
move limit, tool version, shard list with sha256). Every episode's randomness comes from
``SeedSequence([policy_seed, record_id, repeat])``, so a collection is reproducible and
independent of the order puzzles are processed in.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections.abc import Iterable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

from jepa_water_sort import _native
from jepa_water_sort._native import Params, State, __version__
from jepa_water_sort.dataset import Dataset, puzzle_id
from jepa_water_sort.policies import Policy, make_policy

FORMAT_VERSION = 1
DEFAULT_SHARD_SIZE = 1_000_000
ROW_GROUP_SIZE = 65_536
SOURCES = ("optimal", "random", "epsilon", "greedy", "human")
TOOL_VERSION = f"jepa_water_sort {__version__}"


def transition_schema(params: Params) -> pa.Schema:
    cells = params.n_tubes * params.capacity
    return pa.schema([
        pa.field("puzzle_id", pa.string(), nullable=False),
        pa.field("generator_id", pa.string(), nullable=False),
        pa.field("canonical_hash", pa.uint64(), nullable=False),
        pa.field("record_id", pa.uint64()),
        pa.field("split", pa.string()),
        pa.field("source", pa.string(), nullable=False),
        pa.field("episode", pa.uint64(), nullable=False),
        pa.field("step", pa.uint32(), nullable=False),
        pa.field("state", pa.binary(cells), nullable=False),
        pa.field("action", pa.uint16(), nullable=False),
        pa.field("next_state", pa.binary(cells), nullable=False),
        pa.field("done", pa.bool_(), nullable=False),
        pa.field("truncated", pa.bool_(), nullable=False),
        pa.field("illegal", pa.bool_(), nullable=False),
        pa.field("solved", pa.bool_(), nullable=False),
    ])


@dataclass(frozen=True)
class Transition:
    state: State
    action: int
    next_state: State
    done: bool
    truncated: bool
    illegal: bool
    solved: bool


def run_episode(
    state: State,
    policy: Policy,
    rng: np.random.Generator,
    opt_moves: int,
    move_limit_k: int = 4,
    solution: Iterable[int] | None = None,
    dead_end_max_states: int | None = None,
) -> list[Transition]:
    """Plays one episode under the environment's rules (``water_sort_core::episode``) until it
    terminates, truncates at ``move_limit_k * opt_moves``, or the policy has no move."""
    policy.reset(state, rng, None if solution is None else list(solution))
    limit = _native.move_limit(move_limit_k, opt_moves)
    out: list[Transition] = []
    moves = 0
    while True:
        action = policy.act(state)
        if action is None:
            break
        nxt, _, _, terminated, truncated, illegal, _, solved = _native.env_step(
            state, action, moves, limit, None, dead_end_max_states
        )
        moves += 1
        out.append(Transition(state, action, nxt, terminated or truncated, truncated, illegal,
                              solved))
        state = nxt
        if terminated or truncated:
            break
    return out


def replay_actions(
    state: State, actions: Iterable[int], opt_moves: int | None = None, move_limit_k: int = 4
) -> list[Transition]:
    """Replays recorded actions (e.g. human play) under the environment's rules. Illegal
    actions are kept and flagged. Without ``opt_moves`` there is no move limit."""
    limit = 2**32 - 1 if opt_moves is None else _native.move_limit(move_limit_k, opt_moves)
    out: list[Transition] = []
    for moves, action in enumerate(actions):
        nxt, _, _, terminated, truncated, illegal, _, solved = _native.env_step(
            state, int(action), moves, limit
        )
        out.append(Transition(state, int(action), nxt, terminated or truncated, truncated,
                              illegal, solved))
        state = nxt
        if terminated or truncated:
            break
    return out


def episode_rng(policy_seed: int, record_id: int, repeat: int) -> np.random.Generator:
    return np.random.default_rng(np.random.SeedSequence([policy_seed, record_id, repeat]))


class TrajectoryLogger:
    """Writes transitions to Parquet shards and a manifest. Use as a context manager or call
    :meth:`close`."""

    def __init__(
        self,
        out_dir: str | Path,
        params: Params,
        source: str,
        *,
        epsilon: float | None = None,
        policy_seed: int | None = None,
        move_limit_k: int = 4,
        dataset: Dataset | None = None,
        split: str | None = None,
        shard_size: int = DEFAULT_SHARD_SIZE,
        extra: dict[str, Any] | None = None,
    ):
        if source not in SOURCES:
            raise ValueError(f"unknown source {source!r} (expected one of {SOURCES})")
        if shard_size < 1:
            raise ValueError("shard_size must be positive")
        self.out_dir = Path(out_dir)
        self.out_dir.mkdir(parents=True, exist_ok=True)
        if any(self.out_dir.glob("shard-*.parquet")) or (self.out_dir / "manifest.json").exists():
            raise FileExistsError(f"{self.out_dir} already holds trajectories")
        self.params = params
        self.source = source
        self.epsilon = epsilon
        self.policy_seed = policy_seed
        self.move_limit_k = move_limit_k
        self.dataset = dataset
        self.split = split
        self.shard_size = shard_size
        self.extra = extra or {}
        self.schema = transition_schema(params)
        self._rows: dict[str, list] = {name: [] for name in self.schema.names}
        self._shards: list[dict[str, Any]] = []
        self._episodes = 0
        self._transitions = 0
        self._solved = 0
        self._truncated = 0
        self._illegal = 0
        self._closed = False

    def __enter__(self) -> TrajectoryLogger:
        return self

    def __exit__(self, *exc) -> None:
        if exc[0] is None:
            self.close()

    @property
    def episodes(self) -> int:
        return self._episodes

    def log_episode(
        self,
        transitions: list[Transition],
        generator_id: str,
        canonical_hash: int | None = None,
        record_id: int | None = None,
        split: str | None = None,
    ) -> None:
        """Adds one episode. ``canonical_hash`` defaults to the first state's."""
        if self._closed:
            raise RuntimeError("logger is closed")
        if not transitions:
            return
        if transitions[0].state.params != self.params:
            raise ValueError(f"episode has {transitions[0].state.params}, logger {self.params}")
        if canonical_hash is None:
            canonical_hash = _native.canonical_hash(transitions[0].state)
        pid = puzzle_id(generator_id, canonical_hash)
        rows = self._rows
        episode = self._episodes
        for i, t in enumerate(transitions):
            rows["puzzle_id"].append(pid)
            rows["generator_id"].append(generator_id)
            rows["canonical_hash"].append(canonical_hash)
            rows["record_id"].append(record_id)
            rows["split"].append(split)
            rows["source"].append(self.source)
            rows["episode"].append(episode)
            rows["step"].append(i)
            rows["state"].append(t.state.to_bytes())
            rows["action"].append(t.action)
            rows["next_state"].append(t.next_state.to_bytes())
            rows["done"].append(t.done)
            rows["truncated"].append(t.truncated)
            rows["illegal"].append(t.illegal)
            rows["solved"].append(t.solved)
        last = transitions[-1]
        self._episodes += 1
        self._transitions += len(transitions)
        self._solved += int(last.solved)
        self._truncated += int(last.truncated)
        self._illegal += sum(t.illegal for t in transitions)
        if len(rows["step"]) >= self.shard_size:
            self._flush()

    def _flush(self) -> None:
        n = len(self._rows["step"])
        if n == 0:
            return
        table = pa.Table.from_pydict(self._rows, schema=self.schema)
        name = f"shard-{len(self._shards):05d}.parquet"
        path = self.out_dir / name
        pq.write_table(table, path, compression="zstd", compression_level=3,
                       row_group_size=ROW_GROUP_SIZE)
        episodes = len(set(self._rows["episode"]))
        self._shards.append({
            "path": name,
            "transitions": n,
            "episodes": episodes,
            "bytes": path.stat().st_size,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        })
        self._rows = {name: [] for name in self.schema.names}

    def close(self) -> dict[str, Any]:
        """Writes the last shard and ``manifest.json``; returns the manifest."""
        if self._closed:
            raise RuntimeError("logger is closed")
        self._flush()
        self._closed = True
        p = self.params
        dataset = None
        if self.dataset is not None:
            dataset = {
                "path": str(self.dataset.path),
                "manifest_sha256": self.dataset.manifest_sha256,
                "generator": self.dataset.generator,
                "split": self.split,
            }
        manifest = {
            "format_version": FORMAT_VERSION,
            "kind": "trajectories",
            "source": self.source,
            "epsilon": self.epsilon,
            "policy_seed": None if self.policy_seed is None else f"{self.policy_seed:016x}",
            "episode_seed_rule": "SeedSequence([policy_seed, record_id, repeat])",
            "params": {"n_colors": p.n_colors, "capacity": p.capacity, "n_empty": p.n_empty},
            "move_limit_k": self.move_limit_k,
            "dataset": dataset,
            "split": self.split,
            "state_encoding": "row-major n_tubes x capacity, bottom to top, 255 = empty",
            "columns": self.schema.names,
            "episodes": self._episodes,
            "transitions": self._transitions,
            "solved_episodes": self._solved,
            "truncated_episodes": self._truncated,
            "illegal_transitions": self._illegal,
            "shard_size": self.shard_size,
            "shards": self._shards,
            "tool_version": TOOL_VERSION,
            **self.extra,
        }
        text = json.dumps(manifest, indent=1) + "\n"
        (self.out_dir / "manifest.json").write_text(text, encoding="utf-8")
        return manifest


def collect(
    dataset: str | Path | Dataset,
    split: str,
    source: str,
    out_dir: str | Path,
    *,
    epsilon: float | None = None,
    policy_seed: int = 0,
    episodes_per_puzzle: int = 1,
    limit: int | None = None,
    move_limit_k: int = 4,
    dead_end_check: bool = False,
    shard_size: int = DEFAULT_SHARD_SIZE,
) -> dict[str, Any]:
    """Collects trajectories on the puzzles of one dataset split and returns the manifest.

    ``limit`` caps the number of puzzles (lowest ``record_id`` first). ``optimal`` replays each
    record's stored solution, the other sources play with their policy.
    """
    if source == "human":
        raise ValueError("human trajectories are imported with import_human, not collected")
    if source == "epsilon" and epsilon is None:
        raise ValueError("the epsilon source needs epsilon")
    ds = dataset if isinstance(dataset, Dataset) else Dataset(dataset)
    records = ds.records(split)
    if limit is not None:
        records = records[:limit]
    policy = make_policy(source, epsilon)
    dead_end = 100_000 if dead_end_check else None
    with TrajectoryLogger(
        out_dir, ds.params, source, epsilon=epsilon if source == "epsilon" else None,
        policy_seed=policy_seed, move_limit_k=move_limit_k, dataset=ds, split=split,
        shard_size=shard_size, extra={"episodes_per_puzzle": episodes_per_puzzle,
                                      "dead_end_check": dead_end_check},
    ) as log:
        for r in records:
            state = r.state
            for repeat in range(episodes_per_puzzle):
                rng = episode_rng(policy_seed, r.record_id, repeat)
                transitions = run_episode(state, policy, rng, r.opt_moves, move_limit_k,
                                          r.solution, dead_end)
                log.log_episode(transitions, r.generator_id, r.canonical_hash, r.record_id,
                                r.split)
    return json.loads((Path(out_dir) / "manifest.json").read_text(encoding="utf-8"))


def import_human(
    games: Iterable[dict[str, Any]],
    out_dir: str | Path,
    params: Params,
    *,
    move_limit_k: int = 4,
    shard_size: int = DEFAULT_SHARD_SIZE,
) -> dict[str, Any]:
    """Imports played games as ``human`` trajectories. Each game is a dict with
    ``puzzle_code`` and ``actions`` (action indices), and optionally ``generator_id``
    (default ``"unknown"``), ``opt_moves`` (enables the move limit), ``record_id`` and
    ``split``. Actions are replayed through the core rules; illegal ones are flagged."""
    with TrajectoryLogger(out_dir, params, "human", move_limit_k=move_limit_k,
                          shard_size=shard_size) as log:
        for g in games:
            state = State.from_code(g["puzzle_code"])
            transitions = replay_actions(state, g["actions"], g.get("opt_moves"), move_limit_k)
            log.log_episode(transitions, g.get("generator_id", "unknown"), None,
                            g.get("record_id"), g.get("split"))
    return json.loads((Path(out_dir) / "manifest.json").read_text(encoding="utf-8"))


def read_manifest(trajectory_dir: str | Path) -> dict[str, Any]:
    return json.loads((Path(trajectory_dir) / "manifest.json").read_text(encoding="utf-8"))


def read_transitions(trajectory_dir: str | Path) -> pa.Table:
    """All shards of a trajectory directory as one table, in shard order."""
    d = Path(trajectory_dir)
    manifest = read_manifest(d)
    tables = [pq.read_table(d / s["path"]) for s in manifest["shards"]]
    if not tables:
        p = manifest["params"]
        return transition_schema(Params(p["n_colors"], p["capacity"], p["n_empty"])).empty_table()
    return pa.concat_tables(tables)


def to_numpy(table: pa.Table, params: Params) -> dict[str, np.ndarray]:
    """Transition columns as numpy arrays; ``state`` / ``next_state`` as
    ``(N, n_tubes, capacity)`` uint8."""
    shape = (-1, params.n_tubes, params.capacity)

    def cells(name: str) -> np.ndarray:
        col = table.column(name).combine_chunks()
        buf = col.buffers()[1]
        data = np.frombuffer(buf, dtype=np.uint8)
        start = col.offset * params.n_tubes * params.capacity
        return data[start:start + len(col) * params.n_tubes * params.capacity].reshape(shape).copy()

    out = {"state": cells("state"), "next_state": cells("next_state")}
    for name, dtype in [("action", np.uint16), ("done", np.bool_), ("truncated", np.bool_),
                        ("illegal", np.bool_), ("solved", np.bool_), ("step", np.uint32),
                        ("episode", np.uint64), ("canonical_hash", np.uint64)]:
        out[name] = table.column(name).to_numpy(zero_copy_only=False).astype(dtype)
    return out


def export_npz(trajectory_dir: str | Path, out_path: str | Path) -> Path:
    """Writes every transition of a trajectory directory to one compressed ``.npz``."""
    manifest = read_manifest(trajectory_dir)
    p = manifest["params"]
    arrays = to_numpy(read_transitions(trajectory_dir),
                      Params(p["n_colors"], p["capacity"], p["n_empty"]))
    out_path = Path(out_path)
    np.savez_compressed(out_path, **arrays)
    return out_path


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(prog="python -m jepa_water_sort.logger")
    sub = parser.add_subparsers(dest="command", required=True)
    c = sub.add_parser("collect", help="collect trajectories on one split of a dataset")
    c.add_argument("--dataset", required=True)
    c.add_argument("--split", required=True, choices=["train", "val", "test"])
    c.add_argument("--source", required=True, choices=["optimal", "random", "epsilon", "greedy"])
    c.add_argument("--epsilon", type=float)
    c.add_argument("--policy-seed", type=lambda s: int(s, 0), default=0)
    c.add_argument("--episodes-per-puzzle", type=int, default=1)
    c.add_argument("--limit", type=int)
    c.add_argument("--move-limit-k", type=int, default=4)
    c.add_argument("--dead-end-check", action="store_true")
    c.add_argument("--shard-size", type=int, default=DEFAULT_SHARD_SIZE)
    c.add_argument("--out", required=True)
    e = sub.add_parser("export-npz", help="write a trajectory directory to one .npz")
    e.add_argument("trajectories")
    e.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    if args.command == "collect":
        m = collect(args.dataset, args.split, args.source, args.out, epsilon=args.epsilon,
                    policy_seed=args.policy_seed, episodes_per_puzzle=args.episodes_per_puzzle,
                    limit=args.limit, move_limit_k=args.move_limit_k,
                    dead_end_check=args.dead_end_check, shard_size=args.shard_size)
        print(f"{m['episodes']} episodes, {m['transitions']} transitions, "
              f"{len(m['shards'])} shards in {args.out}")
    else:
        print(export_npz(args.trajectories, args.out))


if __name__ == "__main__":
    main()
