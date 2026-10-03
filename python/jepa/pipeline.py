"""The whole Phase 7 experiment in one command.

    python -m jepa.pipeline --preset full            # datasets → trajectories → train → eval → report
    python -m jepa.pipeline --preset smoke --out DIR # minutes on a CPU (CI)

Stages (each skipped when its output already exists, so an interrupted run resumes):

1. **datasets**: ``water_sort_cli generate`` (through ``cargo run --release``) for every
   (generator, layout) of the preset, with a fixed master seed and ``--created-at``, so the
   datasets are reproducible.
2. **trajectories**: ``jepa_water_sort.logger.collect`` on the train split (sources ``optimal``,
   ``random``, ``epsilon`` with ε = 0.2) and on the val split (``random``, ``epsilon``, for
   the monitor). Trajectory splits inherit the puzzle split.
3. **train**: one JEPA world model per (generator, layout) and training seed (:mod:`jepa.train`).
   **ablations**: loss / target variants on the default configuration (collapse study).
4. **dqn**: the DDQN baseline (:mod:`jepa.dqn`) on the preset's DQN training sets.
5. **eval**: every model (planner ``beam``) on every test set (the full cross-evaluation matrix:
   generator × layout on both sides), the other planners (``mcts``, ``cem``) and the baselines
   (random, greedy, solver, DDQN) on the default test set, baselines on every test set.
6. **report**: ``<out>/report.md`` and ``<out>/report.json`` (:mod:`jepa.report`).

Everything lands under ``--out`` (default ``data/jepa/<preset>``; ``data/`` is gitignored).
"""

from __future__ import annotations

import argparse
import copy
import json
import multiprocessing
import shutil
import subprocess
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

import numpy as np
import torch

from jepa import baselines, report
from jepa.config import EvalConfig, PlanConfig, TrainConfig, train_config_from_dict
from jepa.dqn import DQNConfig, DQNPolicy, train_dqn
from jepa.eval import run_episodes, test_puzzles, trained_hashes
from jepa.plan import LatentPlanner
from jepa.train import load_model, pick_device, train

REPO = Path(__file__).resolve().parents[2]
CREATED_AT = "2026-10-03T00:00:00Z"
STAGES = ("datasets", "trajectories", "train", "ablations", "dqn", "eval", "report")
TRAIN_SOURCES = ("optimal", "random", "epsilon")
VAL_SOURCES = ("random", "epsilon")


@dataclass
class Preset:
    name: str
    params: tuple[int, int, int] = (6, 4, 2)
    generators: tuple[str, ...] = ("uniform", "turan")
    layouts: tuple[str, ...] = ("standard", "distributed")
    #: Indices generated per dataset (duplicates are dropped; Turan has more of them).
    count: dict[str, int] = field(default_factory=lambda: {"uniform": 100_000, "turan": 60_000})
    master_seed: int = 7
    #: Puzzles per trajectory source (lowest record ids of the split).
    train_puzzles: int = 40_000
    val_puzzles: int = 4_000
    epsilon: float = 0.2
    seeds: tuple[int, ...] = (0, 1, 2)
    #: TrainConfig fields (nested dicts allowed) applied on top of the defaults.
    train: dict[str, Any] = field(default_factory=dict)
    #: (generator, layout) the DDQN baseline trains on; empty disables it.
    dqn_sets: tuple[tuple[str, str], ...] = (("uniform", "standard"),)
    dqn: dict[str, Any] = field(default_factory=dict)
    eval: EvalConfig = field(default_factory=EvalConfig)
    plan: PlanConfig = field(default_factory=PlanConfig)
    #: Extra planners run on the default test set only, with this many puzzles.
    extra_planners: tuple[str, ...] = ("mcts", "cem")
    extra_puzzles: int = 200
    #: The default configuration: (generator, layout).
    default: tuple[str, str] = ("uniform", "standard")
    #: Existing dataset directories by "<generator>_<layout>" (skips generation).
    datasets: dict[str, str] = field(default_factory=dict)
    #: Collapse ablations (PLAN 7, task 3): TrainConfig overrides, each trained once (first
    #: seed) on the default configuration; the planner is evaluated on the default test set
    #: unless the variant trains no planning heads.
    ablations: dict[str, dict[str, Any]] = field(default_factory=dict)


#: The ablations of the full preset.
ABLATIONS: dict[str, dict[str, Any]] = {
    "no_idm": {"loss": {"idm": 0.0}},
    "probe_aux": {"loss": {"probe_aux": 1.0}},
    "no_ema": {"ema_start": 0.0, "ema_end": 0.0},
    "single_step": {"data": {"max_horizon": 1}},
    "cosine_loss": {"loss": {"latent_loss": "cosine"}},
    "latent_only": {"loss": {"idm": 0.0, "legal": 0.0, "solved": 0.0, "value": 0.0}},
}
#: Ablations without planning heads are not evaluated with the planner.
NO_PLANNER = {"latent_only"}


PRESETS: dict[str, Preset] = {
    "full": Preset(
        name="full",
        train={"epochs": 10, "batch_size": 1024, "lr": 5e-4, "warmup_steps": 1000},
        dqn={"total_steps": 3_000_000},
        ablations=ABLATIONS,
    ),
    # One dataset, one seed: the default configuration only (a quick check of the acceptance
    # numbers before the full run).
    "default": Preset(
        name="default", generators=("uniform",), layouts=("standard",), seeds=(0,),
        train={"epochs": 10, "batch_size": 1024, "lr": 5e-4, "warmup_steps": 1000},
        dqn={"total_steps": 3_000_000},
    ),
    # The committed CI fixture (uniform 5 × 4 × 2, 200 puzzles): a few training steps and a
    # handful of episodes per policy, CPU only.
    "smoke": Preset(
        name="smoke", params=(5, 4, 2), generators=("uniform",), layouts=("standard",),
        train_puzzles=200, val_puzzles=50, seeds=(0,),
        train={"max_steps": 6, "batch_size": 32, "warmup_steps": 2, "log_every": 2,
               "tensorboard": False, "device": "cpu",
               "model": {"dim": 32, "encoder_layers": 1, "predictor_layers": 1},
               "data": {"monitor_size": 64}},
        dqn={"total_steps": 512, "envs": 16, "learning_starts": 64, "batch_size": 32,
             "buffer": 2048, "target_every": 8, "log_every": 256, "device": "cpu",
             "model": {"dim": 32, "encoder_layers": 1, "predictor_layers": 1}},
        eval=EvalConfig(puzzles=6, batch=4),
        plan=PlanConfig(depth=2, width=4, mcts_simulations=4, cem_samples=8, cem_elites=2,
                        cem_iters=1),
        extra_puzzles=3,
        ablations={"no_idm": ABLATIONS["no_idm"], "latent_only": ABLATIONS["latent_only"]},
        datasets={"uniform_standard": str(REPO / "water_sort_cli/tests/fixtures/datasets/uniform_c5k4e2")},
    ),
}


def _merge(base: dict, over: dict) -> dict:
    out = copy.deepcopy(base)
    for k, v in over.items():
        out[k] = _merge(out[k], v) if isinstance(v, dict) and isinstance(out.get(k), dict) else v
    return out


def _log(msg: str) -> None:
    print(f"[{time.strftime('%H:%M:%S')}] {msg}", flush=True)


# -- stages ----------------------------------------------------------------------------------


def dataset_dir(preset: Preset, out: Path, key: str) -> Path:
    if key in preset.datasets:
        return Path(preset.datasets[key])
    return out / "datasets" / key


def stage_datasets(preset: Preset, out: Path) -> None:
    c, k, e = preset.params
    for gen in preset.generators:
        for layout in preset.layouts:
            key = f"{gen}_{layout}"
            d = dataset_dir(preset, out, key)
            if (d / "manifest.json").exists():
                continue
            if key in preset.datasets:
                raise FileNotFoundError(f"dataset {d} has no manifest.json")
            _log(f"generating dataset {key} ({preset.count[gen]} indices)")
            cmd = [
                "cargo", "run", "--release", "--locked", "-q", "-p", "water_sort_cli", "--",
                "generate", "--generator", gen, "--layout", layout, "--count", str(preset.count[gen]),
                "--colors", str(c), "--capacity", str(k), "--empty", str(e), "--rollouts", "0",
                "--master-seed", str(preset.master_seed), "--created-at", CREATED_AT,
                "--split-files", "--out", str(d),
            ]
            subprocess.run(cmd, cwd=REPO, check=True)


def traj_dir(out: Path, key: str, split: str, source: str) -> Path:
    return out / "trajectories" / key / f"{split}_{source}"


def _collect(job: tuple[str, str, str, str, float, int, int]) -> str:
    from jepa_water_sort import logger

    dataset, split, source, out_dir, eps, seed, limit = job
    tmp = Path(out_dir + ".tmp")
    shutil.rmtree(tmp, ignore_errors=True)
    logger.collect(dataset, split, source, tmp, epsilon=eps if source == "epsilon" else None,
                   policy_seed=seed, limit=limit)
    tmp.rename(out_dir)
    return out_dir


def stage_trajectories(preset: Preset, out: Path, workers: int) -> None:
    jobs = []
    for gen in preset.generators:
        for layout in preset.layouts:
            key = f"{gen}_{layout}"
            ds = str(dataset_dir(preset, out, key))
            for split, sources, limit, seed in (("train", TRAIN_SOURCES, preset.train_puzzles, 0),
                                                ("val", VAL_SOURCES, preset.val_puzzles, 1)):
                for s in sources:
                    d = traj_dir(out, key, split, s)
                    if not (d / "manifest.json").exists():
                        d.parent.mkdir(parents=True, exist_ok=True)
                        jobs.append((ds, split, s, str(d), preset.epsilon, seed, limit))
    if not jobs:
        return
    _log(f"collecting {len(jobs)} trajectory sets on {workers} processes")
    ctx = multiprocessing.get_context("spawn")
    with ProcessPoolExecutor(min(workers, len(jobs)), mp_context=ctx) as pool:
        for d in pool.map(_collect, jobs):
            _log(f"  {d}")


def train_config(preset: Preset, out: Path, key: str, seed: int, device: str | None) -> TrainConfig:
    base = asdict(TrainConfig())
    cfg = train_config_from_dict(_merge(base, preset.train))
    cfg.name = f"{key}-seed{seed}"
    cfg.seed = seed
    cfg.data.train = [str(traj_dir(out, key, "train", s)) for s in TRAIN_SOURCES]
    cfg.data.val = [str(traj_dir(out, key, "val", s)) for s in VAL_SOURCES]
    if device:
        cfg.device = device
    return cfg


def run_dir(out: Path, key: str, seed: int) -> Path:
    return out / "runs" / key / f"seed{seed}"


def stage_train(preset: Preset, out: Path, device: str | None) -> None:
    for gen in preset.generators:
        for layout in preset.layouts:
            key = f"{gen}_{layout}"
            for seed in preset.seeds:
                d = run_dir(out, key, seed)
                if (d / "final.json").exists():
                    continue
                _log(f"training {key} seed {seed} → {d}")
                final = train(train_config(preset, out, key, seed, device), d)
                _log(f"  effective rank {final['effective_rank_frac']:.2f}, probe exact "
                     f"{final['probe_exact']:.3f}, pred1 exact {final.get('pred1_exact', float('nan')):.3f}")


def ablation_dir(out: Path, name: str) -> Path:
    return out / "runs" / f"ablation-{name}"


def stage_ablations(preset: Preset, out: Path, device: str | None) -> None:
    key = "_".join(preset.default)
    for name, over in preset.ablations.items():
        d = ablation_dir(out, name)
        if (d / "final.json").exists():
            continue
        cfg = train_config(preset, out, key, preset.seeds[0], device)
        cfg = train_config_from_dict(_merge(asdict(cfg), over))
        cfg.name = f"{key}-ablation-{name}"
        _log(f"training ablation {name} → {d}")
        train(cfg, d)


def stage_dqn(preset: Preset, out: Path, device: str | None) -> None:
    for gen, layout in preset.dqn_sets:
        key = f"{gen}_{layout}"
        for seed in preset.seeds:
            d = out / "runs" / f"dqn_{key}" / f"seed{seed}"
            if (d / "final.json").exists():
                continue
            base = asdict(DQNConfig())
            raw = _merge(base, preset.dqn)
            from jepa.config import ModelConfig

            raw["model"] = ModelConfig(**raw["model"])
            cfg = DQNConfig(**raw)
            cfg.dataset = str(dataset_dir(preset, out, key))
            cfg.seed = seed
            if device:
                cfg.device = device
            _log(f"training DDQN {key} seed {seed} → {d}")
            train_dqn(cfg, d)


def _eval_cached(path: Path, policy_factory, puzzles, excluded: set[int], ecfg: EvalConfig,
                 meta: dict[str, Any]) -> dict[str, Any]:
    if path.exists():
        return json.loads(path.read_text(encoding="utf-8"))
    keep = [p for p in puzzles if p.canonical_hash not in excluded]
    t0 = time.time()
    results = run_episodes(policy_factory(), keep, ecfg.move_limit_k, ecfg.policy_seed, ecfg.batch)
    rec = {**meta, "leaked": len(puzzles) - len(keep), "secs": time.time() - t0, "results": results}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(rec) + "\n", encoding="utf-8")
    return rec


def stage_eval(preset: Preset, out: Path, device: str | None) -> list[dict[str, Any]]:
    dev = pick_device(device or "auto")
    ecfg = preset.eval
    keys = [f"{g}_{l}" for g in preset.generators for l in preset.layouts]
    default_key = "_".join(preset.default)
    tests = {k: test_puzzles(dataset_dir(preset, out, k), ecfg.puzzles) for k in keys}
    edir = out / "eval"
    records: list[dict[str, Any]] = []

    def meta(policy: str, train_key: str | None, seed: int | None, test_key: str) -> dict:
        return {"policy": policy, "train": train_key, "seed": seed, "test": test_key}

    # Baselines on every test set.
    for tk, puzzles in tests.items():
        for name, factory in (("random", baselines.RandomLegal), ("greedy", baselines.Greedy),
                              ("solver", baselines.Solver)):
            _log(f"eval {name} on {tk}")
            records.append(_eval_cached(edir / f"{name}__{tk}.json", factory, puzzles, set(), ecfg,
                                        meta(name, None, None, tk)))
    # DDQN on every test set.
    for gen, layout in preset.dqn_sets:
        key = f"{gen}_{layout}"
        excluded = trained_hashes([])  # DDQN trains on the train split only (no overlap, D17)
        for seed in preset.seeds:
            ckpt = out / "runs" / f"dqn_{key}" / f"seed{seed}" / "dqn.pt"
            for tk, puzzles in tests.items():
                _log(f"eval ddqn[{key} seed {seed}] on {tk}")
                records.append(_eval_cached(
                    edir / f"ddqn-{key}-seed{seed}__{tk}.json",
                    lambda: DQNPolicy(ckpt, dev), puzzles, excluded, ecfg,
                    meta("ddqn", key, seed, tk)))
    # JEPA planners: beam on the full matrix, the others on the default test set.
    for key in keys:
        for seed in preset.seeds:
            rd = run_dir(out, key, seed)
            model, cfg = load_model(rd / "model.pt", dev)
            excluded = trained_hashes(cfg.data.train)
            plans = [("beam", tk, tests[tk]) for tk in keys]
            if key == default_key:
                plans += [(m, default_key, tests[default_key][:preset.extra_puzzles])
                          for m in preset.extra_planners]
            for method, tk, puzzles in plans:
                pcfg = copy.deepcopy(preset.plan)
                pcfg.method = method
                name = f"jepa-{method}"
                _log(f"eval {name}[{key} seed {seed}] on {tk} ({len(puzzles)} puzzles)")
                records.append(_eval_cached(
                    edir / f"{name}-{key}-seed{seed}__{tk}.json",
                    lambda: LatentPlanner(model, pcfg, dev, cfg.loss.value_cap),
                    puzzles, excluded, ecfg, {**meta(name, key, seed, tk), "plan": asdict(pcfg)}))
    for name in preset.ablations:
        if name in NO_PLANNER:
            continue
        model, cfg = load_model(ablation_dir(out, name) / "model.pt", dev)
        pcfg = copy.deepcopy(preset.plan)
        policy = f"jepa-{pcfg.method}"
        _log(f"eval {policy}[ablation {name}] on {default_key}")
        records.append(_eval_cached(
            edir / f"ablation-{name}__{default_key}.json",
            lambda: LatentPlanner(model, pcfg, dev, cfg.loss.value_cap),
            tests[default_key], trained_hashes(cfg.data.train), ecfg,
            {**meta(policy, f"ablation:{name}", 0, default_key), "plan": asdict(pcfg)}))
    return records


def run(preset: Preset, out: Path, *, device: str | None = None, stages: list[str] | None = None,
        workers: int | None = None, report_path: Path | None = None) -> Path:
    stages = stages or list(STAGES)
    out.mkdir(parents=True, exist_ok=True)
    (out / "preset.json").write_text(json.dumps(asdict(preset), indent=2) + "\n", encoding="utf-8")
    workers = workers or max(1, multiprocessing.cpu_count())
    t0 = time.time()
    if "datasets" in stages:
        stage_datasets(preset, out)
    if "trajectories" in stages:
        stage_trajectories(preset, out, workers)
    if "train" in stages:
        stage_train(preset, out, device)
    if "ablations" in stages:
        stage_ablations(preset, out, device)
    if "dqn" in stages:
        stage_dqn(preset, out, device)
    records = stage_eval(preset, out, device) if "eval" in stages or "report" in stages else []
    path = out / "report.md"
    if "report" in stages:
        runs = {}
        for gen in preset.generators:
            for layout in preset.layouts:
                key = f"{gen}_{layout}"
                runs[key] = [json.loads((run_dir(out, key, s) / "final.json").read_text())
                             for s in preset.seeds]
        for name in preset.ablations:
            runs[f"ablation:{name}"] = [json.loads((ablation_dir(out, name) / "final.json").read_text())]
        info = {
            "preset": asdict(preset),
            "command": " ".join([Path(sys.executable).name, "-m", "jepa.pipeline", *sys.argv[1:]]),
            "torch": torch.__version__,
            "device": str(pick_device(device or "auto")),
            "cuda_device": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
            "wall_secs_this_invocation": time.time() - t0,
        }
        report.write(records, runs, info, path, out / "report.json")
        if report_path is not None:
            report_path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, report_path)
            shutil.copyfile(out / "report.json", report_path.with_suffix(".json"))
        _log(f"report: {path}")
    return path


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(prog="python -m jepa.pipeline", description=__doc__.split("\n")[0])
    ap.add_argument("--preset", default="full", choices=sorted(PRESETS))
    ap.add_argument("--out", help="output directory (default data/jepa/<preset>)")
    ap.add_argument("--device", help="cuda, cpu, mps (default: auto)")
    ap.add_argument("--stages", nargs="+", choices=STAGES)
    ap.add_argument("--seeds", type=int, nargs="+", help="override the preset's training seeds")
    ap.add_argument("--workers", type=int, help="processes for trajectory collection")
    ap.add_argument("--report", help="also copy report.md / report.json here (e.g. reports/jepa_eval.md)")
    a = ap.parse_args(argv)
    preset = copy.deepcopy(PRESETS[a.preset])
    if a.seeds:
        preset.seeds = tuple(a.seeds)
    out = Path(a.out) if a.out else REPO / "data" / "jepa" / preset.name
    run(preset, out, device=a.device, stages=a.stages, workers=a.workers,
        report_path=Path(a.report) if a.report else None)


if __name__ == "__main__":
    np.set_printoptions(precision=4)
    main()
