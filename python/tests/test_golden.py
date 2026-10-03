"""Rust <-> Python parity: the committed golden files, replayed from Python with exact equality.

``rollouts.json`` is written by ``water_sort_core/tests/golden.rs`` from the core episode rules;
the other files are the Phase 1-3 vectors (rule 2 in CLAUDE.md).
"""

import json

import numpy as np
import pytest

import jepa_water_sort as w
from jepa_water_sort import WaterSortEnv


def load(repo, rel):
    return json.loads((repo / rel).read_text(encoding="utf-8"))


def state_of(d):
    p = d["params"]
    s = w.State.from_tubes(d["tubes"], p["capacity"])
    assert s.params == w.Params(p["n_colors"], p["capacity"], p["n_empty"])
    return s


@pytest.fixture(scope="module")
def rollouts(repo):
    return load(repo, "water_sort_core/tests/golden/rollouts.json")["cases"]


def test_rollouts_cover_every_outcome(rollouts):
    steps = [s for c in rollouts for s in c["steps"]]
    for flag in ["illegal", "dead_end", "solved", "truncated"]:
        assert any(s[flag] for s in steps), flag
    assert len(steps) > 1000


def test_rollouts_through_step(rollouts):
    for case in rollouts:
        s = w.State.from_code(case["puzzle_code"])
        for st in case["steps"]:
            try:
                nxt, units = w.step(s, st["action"])
                assert not st["illegal"]
            except ValueError:
                assert st["illegal"]
                nxt, units = s, 0
            assert nxt.puzzle_code == st["state"], case["name"]
            assert units == st["units_moved"]
            assert nxt.is_solved() == st["solved"]
            s = nxt


def test_rollouts_through_env_step(rollouts):
    for case in rollouts:
        s = w.State.from_code(case["puzzle_code"])
        for i, st in enumerate(case["steps"]):
            nxt, units, reward, term, trunc, illegal, dead_end, solved = w.env_step(
                s, st["action"], i, case["move_limit"], case["shaping_gamma"],
                case["dead_end_max_states"],
            )
            assert nxt.puzzle_code == st["state"]
            assert reward == st["reward"], case["name"]  # exact, not approximate
            assert (units, term, trunc, illegal, dead_end, solved) == (
                st["units_moved"], st["terminated"], st["truncated"], st["illegal"],
                st["dead_end"], st["solved"],
            )
            assert st["moves_so_far"] == i + 1
            s = nxt


def test_rollouts_through_batch_env_step(rollouts):
    """All cases with the same rules advance together, one batch per step index."""
    groups = {}
    for case in rollouts:
        key = (w.State.from_code(case["puzzle_code"]).params.as_tuple(), case["shaping_gamma"],
               case["dead_end_max_states"])
        groups.setdefault(key, []).append(case)
    for (_, gamma, dead), cases in groups.items():
        states = np.stack([w.State.from_code(c["puzzle_code"]).to_numpy() for c in cases])
        for i in range(max(len(c["steps"]) for c in cases)):
            live = [j for j, c in enumerate(cases) if i < len(c["steps"])]
            out = w.batch_env_step(
                states[live],
                [cases[j]["steps"][i]["action"] for j in live],
                np.full(len(live), i, dtype=np.uint32),
                [cases[j]["move_limit"] for j in live],
                gamma,
                dead,
            )
            for row, j in enumerate(live):
                st = cases[j]["steps"][i]
                assert w.State.from_numpy(out[0][row]).puzzle_code == st["state"]
                assert out[1][row] == st["units_moved"] and out[2][row] == st["reward"]
                assert (out[3][row], out[4][row], out[5][row], out[6][row], out[7][row]) == (
                    st["terminated"], st["truncated"], st["illegal"], st["dead_end"], st["solved"]
                )
            states[live] = out[0]


def test_rollouts_through_env(rollouts):
    replayed = 0
    for case in rollouts:
        if case["opt_moves"] is None:
            continue  # unsolvable start: the env only loads solvable puzzles
        start = w.State.from_code(case["puzzle_code"])
        gamma = case["shaping_gamma"]
        env = WaterSortEnv(
            params=start.params,
            move_limit_k=case["move_limit_k"],
            shaping=gamma is not None,
            gamma=0.99 if gamma is None else gamma,
            dead_end_check=case["dead_end_max_states"] is not None,
            dead_end_max_states=case["dead_end_max_states"] or 1,
        )
        obs, info = env.reset(options={"puzzle_code": case["puzzle_code"]})
        assert info["opt_moves"] == case["opt_moves"]
        assert info["move_limit"] == case["move_limit"]
        assert (w.decode_observation(obs) == start.to_numpy()).all()
        for st in case["steps"]:
            obs, reward, term, trunc, info = env.step(st["action"])
            assert w.State.from_numpy(w.decode_observation(obs)).puzzle_code == st["state"]
            assert reward == st["reward"]
            assert (term, trunc) == (st["terminated"], st["truncated"])
            assert (info["illegal"], info["dead_end"], info["solved"]) == (
                st["illegal"], st["dead_end"], st["solved"])
            assert info["moves_so_far"] == st["moves_so_far"]
            assert info["units_moved"] == st["units_moved"]
        replayed += 1
    assert replayed > 50


def test_canonical_vectors(repo):
    for case in load(repo, "water_sort_core/tests/golden/canonical.json"):
        s = state_of(case["state"])
        assert w.canonical(s) == state_of(case["canonical"])
        assert f"{w.canonical_hash(s):016x}" == case["hash"]


def test_puzzle_code_vectors(repo):
    for case in load(repo, "water_sort_core/tests/golden/puzzle_codes.json"):
        s = state_of(case["state"])
        assert s.puzzle_code == case["code"]
        assert w.State.from_code(case["code"]) == s


def test_solution_vectors(repo):
    for case in load(repo, "water_sort_core/tests/golden/solutions.json"):
        s = state_of(case["state"])
        r = w.solve(s, case["max_states"])
        (status, expected), = case["result"].items()
        assert r.status == status.lower()
        assert r.states_expanded == expected["states_expanded"]
        if status == "Solvable":
            assert r.opt_moves == expected["opt_moves"]
            assert r.solution == [(m["from"], m["to"]) for m in expected["solution"]]


def test_seed_vectors(repo):
    d = load(repo, "water_sort_core/tests/golden/seeds.json")
    for case in d["splitmix64"]:
        assert w.splitmix64(int(case["input"], 16)) == int(case["output"], 16)
    for case in d["time_seed"]:
        assert w.time_seed(int(case["now_nanos"], 16), case["counter"]) == int(case["seed"], 16)


def generator_files(repo):
    return sorted((repo / "uniform_water_sort/tests/golden").glob("*.json")) + sorted(
        (repo / "turan_water_sort/tests/golden").glob("*.json"))


def test_generator_vectors(repo):
    files = generator_files(repo)
    assert len(files) == 27
    for path in files:
        d = json.loads(path.read_text(encoding="utf-8"))
        variant = d["variant"]
        layout = "distributed" if "layout=distributed" in variant else "standard"
        strategy = variant if d["generator"] == "turan" else None
        assert w.variant(d["generator"], strategy, layout) == variant
        p = d["params"]
        params = w.Params(p["n_colors"], p["capacity"], p["n_empty"])
        cfg = w.GenConfig.from_json(json.dumps(d["config"]))
        seeds = [int(c["seed"], 16) for c in d["cases"]]
        puzzles = w.batch_generate(d["generator"], params, seeds=seeds, config=cfg,
                                   strategy=strategy, layout=layout)
        for case, puzzle in zip(d["cases"], puzzles):
            assert puzzle.generator_version == d["version"]
            assert puzzle.attempts == case["attempts"], path.name
            assert puzzle.opt_moves == case["opt_moves"]
            assert puzzle.puzzle_code == case["puzzle_code"]
            assert f"{puzzle.canonical_hash:016x}" == case["canonical_hash"]
            assert puzzle.solution == [(m["from"], m["to"]) for m in case["solution"]]
