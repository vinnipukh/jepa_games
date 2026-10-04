"""One-off: evaluate the uniform-trained model (default preset) on a Turan test set.

python python/turan_cross_eval.py            # from the repo root, GPU torch venv
"""
from __future__ import annotations

import copy
from pathlib import Path

from jepa import baselines
from jepa.dqn import DQNPolicy
from jepa.eval import test_puzzles, trained_hashes
from jepa.pipeline import PRESETS, REPO, _eval_cached, dataset_dir, run_dir, stage_datasets
from jepa.plan import LatentPlanner
from jepa.train import load_model, pick_device

out = REPO / "data" / "jepa" / "default"
preset = copy.deepcopy(PRESETS["default"])
preset.generators = ("turan",)
preset.count = {**preset.count, "turan": 10_000}  # ~1000 test puzzles; the CLI's turan search is slower than uniform
stage_datasets(preset, out)  # skipped if datasets/turan_standard/manifest.json exists

dev = pick_device("cuda")
ecfg = preset.eval
tk = "turan_standard"
puzzles = test_puzzles(dataset_dir(preset, out, tk), ecfg.puzzles)
edir = out / "eval"
rows = []

for name, factory in (("random", baselines.RandomLegal), ("greedy", baselines.Greedy), ("solver", baselines.Solver)):
    rows.append(_eval_cached(edir / f"{name}__{tk}.json", factory, puzzles, set(), ecfg,
                             {"policy": name, "train": None, "seed": None, "test": tk}))

ckpt = out / "runs" / "dqn_uniform_standard" / "seed0" / "dqn.pt"
rows.append(_eval_cached(edir / f"ddqn-uniform_standard-seed0__{tk}.json", lambda: DQNPolicy(ckpt, dev), puzzles,
                         trained_hashes([]), ecfg, {"policy": "ddqn", "train": "uniform_standard", "seed": 0, "test": tk}))

model, cfg = load_model(run_dir(out, "uniform_standard", 0) / "model.pt", dev)
pcfg = preset.plan
rows.append(_eval_cached(edir / f"jepa-{pcfg.method}-uniform_standard-seed0__{tk}.json",
                         lambda: LatentPlanner(model, pcfg, dev, cfg.loss.value_cap), puzzles,
                         trained_hashes(cfg.data.train), ecfg,
                         {"policy": f"jepa-{pcfg.method}", "train": "uniform_standard", "seed": 0, "test": tk}))

print(f"\nTest set {tk}: {len(puzzles)} puzzles")
print(f"{'policy':<30}{'n':>5}{'solved':>9}{'stars':>8}")
for r in rows:
    res = r["results"]
    n = len(res)
    label = r["policy"] + (f" (train {r['train']})" if r["train"] else "")
    print(f"{label:<30}{n:>5}{100 * sum(x['solved'] for x in res) / n:>8.1f}%{sum(x['stars'] for x in res) / n:>8.2f}")
