import pickle

import numpy as np
import pytest

import jepa_water_sort as w

P = w.Params(6, 4, 2)


def test_params():
    assert P.n_tubes == 8 and P.n_units == 24
    assert w.Params(6) == P
    assert P.as_tuple() == (6, 4, 2)
    assert pickle.loads(pickle.dumps(P)) == P
    assert P.is_supported() and P.is_supported("distributed")
    with pytest.raises(ValueError):
        w.Params(0, 4, 2)
    with pytest.raises(ValueError):
        w.Params(15, 4, 2)


def test_gen_config_json_round_trip():
    cfg = w.GenConfig(min_opt=5, max_opt=20, random_rollouts=0)
    assert w.GenConfig.from_json(cfg.to_json()) == cfg
    assert pickle.loads(pickle.dumps(cfg)) == cfg
    # The default serializes exactly like the dataset manifests' `gen_config_json`.
    assert w.GenConfig().to_json() == (
        '{"min_opt":1,"max_attempts":10000,"max_states":5000000,'
        '"metrics":{"random_rollouts":64,"rollout_cap_factor":4,"dead_end_ratios":false}}'
    )


def test_generate_is_deterministic():
    a = w.generate("uniform", P, seed=7)
    b = w.generate("uniform", (6, 4, 2), seed=7)
    assert a == b
    assert a.seed == 7 and a.generator_id == "uniform" and a.generator_variant == "fisher_yates"
    assert a.layout == "standard" and a.params == P
    assert a.opt_moves == len(a.solution) == len(a.solution_actions)
    assert a.tier in ("easy", "medium", "hard")
    assert a.canonical_hash == w.canonical_hash(a.state)
    assert w.State.from_code(a.puzzle_code) == a.state
    assert a.metrics["opt_moves"] == a.opt_moves


def test_generate_fresh_seed_is_recorded():
    a = w.generate("uniform", P)
    assert w.generate("uniform", P, seed=a.seed) == a
    t = w.generate("turan", P, config=w.GenConfig(random_rollouts=0))
    assert w.generate("turan", P, seed=t.seed, config=w.GenConfig(random_rollouts=0)) == t


def test_generator_and_strategy_names():
    assert w.variant("uniform") == "fisher_yates"
    assert w.variant("uniform", layout="distributed") == "fisher_yates(layout=distributed)"
    assert w.variant("turan") == "reverse_search(max_depth=300,max_states=10000,layout=standard)"
    assert w.variant("turan", "scramble") == (
        "scramble(steps=40,max_extra_steps=100,layout=standard)"
    )
    assert w.variant("turan", "scramble(steps=12)", "distributed") == (
        "scramble(steps=12,layout=distributed)"
    )
    assert w.variant("turan", "pour-walk", "distributed") == (
        "pour_walk(steps=160,layout=distributed)"
    )
    assert w.variant("turan", "constrained") == "constrained(layout=standard)"
    # A variant string reads back as its own strategy.
    v = "reverse_search(max_depth=50,max_states=2000,layout=distributed)"
    assert w.variant("turan", v, "distributed") == v
    for bad in ["nope", "scramble(steps=x)", "scramble(foo=1)", "scramble(steps=1"]:
        with pytest.raises(ValueError):
            w.variant("turan", bad)
    with pytest.raises(ValueError):
        w.variant("turan", "scramble(layout=distributed)", "standard")
    with pytest.raises(ValueError):
        w.variant("uniform", "scramble")
    with pytest.raises(ValueError):
        w.variant("other")
    with pytest.raises(ValueError):
        w.generate("turan", P, seed=1, strategy="pour_walk")  # distributed only (D16)


def test_generation_error():
    cfg = w.GenConfig(min_opt=1000, max_attempts=3, random_rollouts=0)
    with pytest.raises(w.GenerationError):
        w.generate("uniform", P, seed=1, config=cfg)


def test_step_and_errors():
    s = w.State.from_tubes([[0, 1], [1, 0], []], capacity=2)
    assert s.params == w.Params(2, 2, 1)
    nxt, k = w.step(s, (0, 2))
    assert k == 1 and nxt.tubes == [[0], [1, 0], [1]]
    assert w.step(s, 0 * 3 + 2) == (nxt, 1)
    for bad in [(0, 0), (2, 0), (0, 1), (5, 0)]:
        with pytest.raises(ValueError):
            w.step(s, bad)
    with pytest.raises(ValueError):
        w.step(s, 9)
    assert w.legal_moves(s) == [(0, 2), (1, 2)]
    mask = w.action_mask(s)
    assert mask.dtype == np.bool_ and mask.shape == (9,)
    assert np.flatnonzero(mask).tolist() == [2, 5]


def test_state_numpy_round_trip():
    p = w.generate("uniform", P, seed=3, layout="distributed")
    a = p.state.to_numpy()
    assert a.dtype == np.uint8 and a.shape == (8, 4)
    assert w.State.from_numpy(a) == p.state
    assert w.State.from_numpy(a.astype(np.int64)) == p.state
    assert p.state.to_bytes() == a.tobytes()
    assert p.state.layout_matches("distributed")
    bad = a.copy()
    bad[0] = [0, w.EMPTY, 1, w.EMPTY]
    with pytest.raises(ValueError):
        w.State.from_numpy(bad)
    with pytest.raises(ValueError):
        w.State.from_tubes([[0, 0], [1]], capacity=2)


def test_pickle():
    p = w.generate("turan", P, seed=11, strategy="scramble", config=w.GenConfig(random_rollouts=0))
    q = pickle.loads(pickle.dumps(p))
    assert q == p and q.metrics == p.metrics and q.solution == p.solution
    s = pickle.loads(pickle.dumps(p.state))
    assert s == p.state and hash(s) == hash(p.state)
    assert len({p.state, s}) == 1


def test_solve():
    p = w.generate("uniform", P, seed=5)
    r = w.solve(p.state)
    assert r.solvable and r.status == "solvable" and r.opt_moves == p.opt_moves
    assert r.solution == p.solution
    s = p.state
    for m in r.solution:
        s, _ = w.step(s, m)
    assert s.is_solved()
    assert w.solve(p.state, max_states=1).status == "timeout"
    stuck = w.State.from_tubes([[0, 1], [1, 0], []], capacity=2)
    assert w.solve(stuck).solvable
    dead = w.State.from_tubes([[0, 1], [1, 0]], capacity=2)  # no empty tube, no move
    r = w.solve(dead)
    assert r.status == "unsolvable" and r.opt_moves is None and r.solution is None


def test_stars():
    assert w.stars(10, 10) == 5
    assert w.stars(11, 10) == 4
    assert w.stars(30, 10) == 1
    assert w.stars(31, 30, (100, 250, 500)) == 4
    with pytest.raises(w.StarBelowOptimalError):
        w.stars(9, 10)
    assert issubclass(w.StarBelowOptimalError, ValueError)


def test_canonical_and_tier():
    s = w.State.from_tubes([[1, 0], [0, 1], []], capacity=2)
    t = w.State.from_tubes([[], [0, 1], [1, 0]], capacity=2)
    assert w.canonical(s) == w.canonical(t)
    assert w.canonical_hash(s) == w.canonical_hash(t)
    assert w.tier(P, 1) == "easy"
    assert w.tier(P, 100) == "hard"
    assert w.tier(w.Params(14, 8, 2), 10) is None


def test_splitmix64():
    assert w.splitmix64(0) == 0xE220A8397B1DCDAF
