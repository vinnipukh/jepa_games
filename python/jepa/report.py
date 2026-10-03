"""Markdown + JSON report of a pipeline run (PLAN 7.5).

Input: the evaluation records (one per policy × training set × seed × test set, each with its
per-puzzle results) and the final monitor metrics of every training run. Seeds are aggregated
as mean ± std (sample std). Buckets are the ``opt_moves`` quartiles of all test puzzles pooled,
so every cell of every table uses the same edges, and ``matched`` solve rates re-weight each
cell's per-bucket rates to the pooled bucket distribution.
"""

from __future__ import annotations

import json
import math
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

from jepa.eval import bucket_edges, bucket_labels, bucket_weights, breakdown, mean_std

#: Acceptance thresholds (PLAN 7, proposed).
MIN_RANK_FRAC = 0.5
MIN_PROBE_EXACT = 0.95


def _fmt(m: float, s: float | None = None, pct: bool = False, digits: int = 2) -> str:
    if m is None or (isinstance(m, float) and math.isnan(m)):
        return "–"
    if pct:
        return f"{100 * m:.1f} %" if s is None or s == 0 else f"{100 * m:.1f} ± {100 * s:.1f} %"
    return f"{m:.{digits}f}" if s is None or s == 0 else f"{m:.{digits}f} ± {s:.{digits}f}"


def _table(header: list[str], rows: list[list[str]]) -> str:
    out = ["| " + " | ".join(header) + " |", "|" + "|".join("---" for _ in header) + "|"]
    out += ["| " + " | ".join(r) + " |" for r in rows]
    return "\n".join(out)


def aggregate(records: list[dict[str, Any]]) -> tuple[dict, list[float], list[float]]:
    """``groups[(policy, train, test)]`` → per-seed breakdowns, plus the pooled edges/weights."""
    pooled: dict[tuple[str, int], int] = {}
    for r in records:
        for x in r["results"]:
            pooled[(r["test"], x["record_id"])] = x["opt_moves"]
    edges = bucket_edges(pooled.values())
    weights = bucket_weights(pooled.values(), edges)
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for r in records:
        b = breakdown(r["results"], edges, weights)
        b["leaked"] = r.get("leaked", 0)
        b["seed"] = r.get("seed")
        groups[(r["policy"], r.get("train"), r["test"])].append(b)
    return groups, edges, weights


def _stat(group: list[dict], *path) -> tuple[float, float]:
    vals = []
    for b in group:
        v: Any = b
        for p in path:
            v = v[p]
        vals.append(v)
    return mean_std(vals)


def write(records: list[dict[str, Any]], runs: dict[str, list[dict]], info: dict[str, Any],
          md_path: Path, json_path: Path) -> None:
    groups, edges, weights = aggregate(records)
    labels = bucket_labels(edges)
    preset = info["preset"]
    default_key = "_".join(preset["default"])
    keys = [f"{g}_{l}" for g in preset["generators"] for l in preset["layouts"]]
    p = preset["params"]
    lines: list[str] = []
    add = lines.append
    add("# JEPA world model: evaluation report\n")
    add(f"Generated {time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime())} by `{info['command']}` "
        f"(preset `{preset['name']}`; torch {info['torch']}, device {info['device']}"
        + (f", {info['cuda_device']}" if info.get("cuda_device") else "") + ").\n")
    add(f"Puzzles: {p[0]} colors × capacity {p[1]} × {p[2]} empty; generators "
        f"{', '.join(preset['generators'])}; layouts {', '.join(preset['layouts'])}. Training seeds "
        f"{list(preset['seeds'])} (mean ± std over seeds). Test: the {preset['eval']['puzzles']} lowest "
        f"record ids of each test split, move limit {preset['eval']['move_limit_k']} · opt_moves, "
        f"policy seed {preset['eval']['policy_seed']}. Planner: {preset['plan']['method']} "
        f"(depth {preset['plan']['depth']}, width {preset['plan']['width']}, score "
        f"`{preset['plan']['score']}`, legality `{preset['plan']['legality']}`, revisit check "
        f"`{preset['plan'].get('revisits')}`).\n")
    add(f"`opt_moves` buckets (quartiles of all test puzzles pooled): {', '.join(labels)}; pooled "
        f"weights {', '.join(f'{100 * w:.0f} %' for w in weights)}.\n")

    # Acceptance
    acc_rows = []
    d_runs = runs.get(default_key, [])
    rank = mean_std([r["effective_rank_frac"] for r in d_runs])
    probe = mean_std([r["probe_exact"] for r in d_runs])
    acc_rows.append(["effective rank > 50 % of the latent dim", _fmt(*rank, pct=True),
                     "yes" if rank[0] > MIN_RANK_FRAC else "**no**"])
    acc_rows.append(["detached probe exact match > 95 % (held-out states)", _fmt(*probe, pct=True),
                     "yes" if probe[0] > MIN_PROBE_EXACT else "**no**"])
    planner = groups.get((f"jepa-{preset['plan']['method']}", default_key, default_key), [])
    greedy = groups.get(("greedy", None, default_key), [])
    if planner and greedy:
        ps = _stat(planner, "overall", "solve_rate")
        gs = _stat(greedy, "overall", "solve_rate")
        acc_rows.append([f"planner solve rate > greedy ({default_key} test)",
                         f"{_fmt(*ps, pct=True)} vs {_fmt(*gs, pct=True)}",
                         "yes" if ps[0] > gs[0] else "**no**"])
    add("## Acceptance (default configuration)\n")
    add(_table(["criterion", "value", "met"], acc_rows) + "\n")

    # Model health
    add("## World model health (collapse monitor, final epoch, held-out val states)\n")
    cols = [("effective_rank_frac", "eff. rank / dim", True), ("z_std_mean", "z std", False),
            ("probe_cell_acc", "probe cell", True), ("probe_exact", "probe exact", True),
            ("idm_acc", "IDM acc", True), ("legal_exact", "legality exact", True),
            ("pred1_exact", "1-step exact", True), ("pred2_exact", "2-step exact", True),
            ("pred3_exact", "3-step exact", True), ("solved_recall", "solved recall", True),
            ("value_mae", "value MAE", False)]
    rows = []
    for key, rs in runs.items():
        if key.startswith("ablation:"):
            continue
        row = [f"`{key}`"]
        for name, _, pct in cols:
            row.append(_fmt(*mean_std([r.get(name, math.nan) for r in rs]), pct=pct))
        rows.append(row)
    add(_table(["train set"] + [c[1] for c in cols], rows) + "\n")
    add("*k-step exact*: the probe decoding of the k-step latent rollout equals the real state. "
        "*value MAE*: distance-to-go error in moves (unsolvable states count as the cap).\n")

    method = f"jepa-{preset['plan']['method']}"
    # Default configuration: every policy
    add(f"## Policies on the default test set (`{default_key}`)\n")
    add(f"`{method}` is the planner of the matrix; the other `jepa-*` rows are planner variants "
        f"(see `PLANNER_VARIANTS` in `jepa/pipeline.py`) on the first {preset.get('extra_puzzles')} "
        "test puzzles.\n")
    seen = sorted({k[0] for k in groups if k[2] == default_key})
    first = ["random", "greedy", "ddqn", method]
    pol_order = [x for x in first if x in seen] + [x for x in seen if x not in first + ["solver"]]
    pol_order += ["solver"] if "solver" in seen else []
    rows, brows = [], []
    for pol in pol_order:
        for (gp, gt, gtest), g in sorted(groups.items(), key=lambda x: str(x[0])):
            if gp != pol or gtest != default_key or (gt not in (None, default_key)):
                continue
            n = int(g[0]["overall"]["n"])
            rows.append([pol + (f" (train `{gt}`)" if gt else ""), str(n),
                         _fmt(*_stat(g, "overall", "solve_rate"), pct=True),
                         _fmt(*_stat(g, "overall", "mean_stars")),
                         _fmt(*_stat(g, "overall", "ratio_solved")),
                         _fmt(*_stat(g, "overall", "ratio_all"))])
            brows.append([pol] + [_fmt(*_stat(g, "buckets", i, "solve_rate"), pct=True)
                                  for i in range(4)])
    add(_table(["policy", "puzzles", "solve rate", "mean stars", "moves/opt (solved)",
                "moves/opt (all)"], rows) + "\n")
    add("Solve rate by `opt_moves` bucket:\n")
    add(_table(["policy"] + labels, brows) + "\n")

    # Cross-evaluation matrix
    add(f"## Cross-evaluation matrix ({method})\n")
    add("Rows: training set; columns: test set. Each cell: solve rate, then the bucket-matched "
        "solve rate in brackets, mean ± std over seeds. Leaked test puzzles (in the model's "
        "training puzzles) are removed per cell; their count is listed below the table.\n")
    rows, leaks = [], []
    for tr in keys:
        row = [f"`{tr}`"]
        for te in keys:
            g = groups.get((method, tr, te))
            if not g:
                row.append("–")
                continue
            row.append(f"{_fmt(*_stat(g, 'overall', 'solve_rate'), pct=True)} "
                       f"({_fmt(*_stat(g, 'matched_solve_rate'), pct=True)})")
            if g[0]["leaked"]:
                leaks.append(f"`{tr}` → `{te}`: {g[0]['leaked']}")
        rows.append(row)
    for base in ("greedy", "random"):
        row = [f"*{base}*"]
        for te in keys:
            g = groups.get((base, None, te))
            row.append(f"{_fmt(*_stat(g, 'overall', 'solve_rate'), pct=True)} "
                       f"({_fmt(*_stat(g, 'matched_solve_rate'), pct=True)})" if g else "–")
        rows.append(row)
    add(_table(["train \\ test"] + [f"`{k}`" for k in keys], rows) + "\n")
    add("Leaked puzzles removed: " + ("; ".join(leaks) if leaks else "none") + ".\n")
    rows = []
    for tr in keys:
        row = [f"`{tr}`"]
        for te in keys:
            g = groups.get((method, tr, te))
            row.append(_fmt(*_stat(g, "overall", "mean_stars")) if g else "–")
        rows.append(row)
    add("Mean stars:\n")
    add(_table(["train \\ test"] + [f"`{k}`" for k in keys], rows) + "\n")
    if len(preset["layouts"]) > 1:
        add("Generator shift: the 2 × 2 blocks with equal layouts. Layout transfer (D14): train "
            "`standard` → test `distributed` and the reverse, for each generator.\n")

    # Ablations
    abl = [k for k in runs if k.startswith("ablation:")]
    if abl:
        add(f"## Collapse ablations (`{default_key}`, seed {preset['seeds'][0]})\n")
        add("Each variant changes one thing in the default training configuration. *latent only* "
            "trains no IDM, legality, solved or value head (pure JEPA loss; the probe stays "
            "detached), so it is not run with the planner.\n")
        rows = []
        variants = [("default", runs.get(default_key, [])[:1], (method, default_key))]
        variants += [(k.split(":", 1)[1], runs[k], (method, k)) for k in abl]
        for name, rs, (pol, tr) in variants:
            r = rs[0] if rs else {}
            g = [b for b in groups.get((pol, tr, default_key), []) if b.get("seed") in (None, 0,
                 preset["seeds"][0])][:1]
            rows.append([name] + [_fmt(r.get(c, math.nan), pct=pct) for c, _, pct in
                                  (cols[0], cols[1], cols[3], cols[4], cols[6], cols[8])]
                        + [_fmt(*_stat(g, "overall", "solve_rate"), pct=True) if g else "–"])
        add(_table(["variant", "eff. rank / dim", "z std", "probe exact", "IDM acc",
                    "1-step exact", "3-step exact", f"{method} solve rate"], rows) + "\n")

    # Per test set, by bucket
    add("## By `opt_moves` bucket, per test set\n")
    for te in keys:
        rows = []
        for (gp, gt, gtest), g in sorted(groups.items(), key=lambda x: str(x[0])):
            if gtest != te or gp not in (method, "greedy", "random", "solver", "ddqn") or (
                    gt or "").startswith("ablation:"):
                continue
            rows.append([gp + (f" (train `{gt}`)" if gt else "")]
                        + [_fmt(*_stat(g, "buckets", i, "solve_rate"), pct=True) for i in range(4)]
                        + [str(int(g[0]["overall"]["n"]))])
        add(f"Test `{te}`:\n")
        add(_table(["policy"] + labels + ["puzzles"], rows) + "\n")

    md_path.write_text("\n".join(lines), encoding="utf-8")
    payload = {
        "info": info,
        "bucket_edges": edges,
        "bucket_weights": weights,
        "runs": runs,
        "groups": [{"policy": k[0], "train": k[1], "test": k[2], "seeds": v}
                   for k, v in groups.items()],
    }
    json_path.write_text(json.dumps(payload, indent=1, default=str) + "\n", encoding="utf-8")
