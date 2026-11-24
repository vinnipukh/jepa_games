"""Difficulty calibration: how often does a policy succeed on each task setting?

RL needs tasks a model solves *sometimes*: a pass rate near 0 or 1 gives no learning signal. Run a
policy (random, your model's logits, or a scripted baseline) over a grid of env settings and keep
the settings whose pass rate falls in a target band.

    python -m gamestack.rl.calibrate --game classify --grid '[{"noise":0.2},{"noise":0.6}]' --episodes 200
"""

from __future__ import annotations

import argparse
import json
from typing import Callable, Sequence

import torch

from gamestack import make
from gamestack.rl.policy import LogitPolicy
from gamestack.rl.rollout import collect


def random_policy(n_actions: int) -> LogitPolicy:
    return lambda prompts: torch.zeros(len(prompts), n_actions)


def pass_rate(game: str, kwargs: dict, policy: LogitPolicy, episodes: int = 200, greedy: bool = False,
              seed0: int = 10_000, success: Callable[[float], bool] = lambda r: r > 0) -> float:
    eps = collect(lambda: make(game, **kwargs), policy, list(range(seed0, seed0 + episodes)), greedy=greedy)
    return sum(success(e.ret) for e in eps) / len(eps)


def calibrate(game: str, grid: Sequence[dict], policy_fn: Callable[[dict], LogitPolicy], episodes: int = 200,
              band: tuple[float, float] = (0.1, 0.9), greedy: bool = False) -> list[dict]:
    rows = []
    for kw in grid:
        rate = pass_rate(game, kw, policy_fn(kw), episodes, greedy)
        rows.append({"game": game, "kwargs": kw, "pass_rate": rate, "in_band": band[0] <= rate <= band[1]})
    return rows


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--game", required=True)
    ap.add_argument("--grid", default="[{}]", help="JSON list of env kwargs dicts")
    ap.add_argument("--episodes", type=int, default=200)
    ap.add_argument("--band", type=float, nargs=2, default=(0.1, 0.9))
    a = ap.parse_args()
    rows = calibrate(a.game, json.loads(a.grid),
                     lambda kw: random_policy(make(a.game, **kw).action_space.n), a.episodes, tuple(a.band))
    for r in rows:
        print(f"{'*' if r['in_band'] else ' '} pass={r['pass_rate']:.2f}  {r['game']} {r['kwargs']}")
    print("(* = inside the target band; policy = uniform random)")


if __name__ == "__main__":
    main()
