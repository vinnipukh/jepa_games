import pickle

import gymnasium as gym
import numpy as np
import pytest
from gymnasium.utils.env_checker import check_env

import jepa_water_sort as w
from jepa_water_sort import WaterSortEnv

P = w.Params(6, 4, 2)


@pytest.mark.parametrize(
    "kwargs",
    [
        {},
        {"generator": "turan"},
        {"generator": "uniform", "layout": "distributed", "shaping": True},
        {"generator": "turan", "layout": "distributed", "strategy": "pour_walk"},
        {"generator": "turan", "strategy": "scramble", "params": (4, 3, 1), "dead_end_check": True},
        {"compute_solvability": True, "params": w.Params(3, 3, 1)},
    ],
)
def test_check_env(kwargs):
    env = WaterSortEnv(**kwargs)
    check_env(env, skip_render_check=True)
    check_env(env.unwrapped)


def test_gym_make():
    env = gym.make(w.ENV_ID, params=(4, 4, 2))
    obs, info = env.reset(seed=0)
    assert obs.shape == (6, 4, 5)
    check_env(env.unwrapped)


def test_spaces_and_observation():
    env = WaterSortEnv(params=P)
    assert env.observation_space.shape == (8, 4, 7)
    assert env.observation_space.dtype == np.int8
    assert env.action_space.n == 64
    obs, info = env.reset(seed=1)
    assert obs.dtype == np.int8 and obs.sum() == 8 * 4
    assert (w.decode_observation(obs) == env.state.to_numpy()).all()
    assert (obs[..., 6] == (env.state.to_numpy() == w.EMPTY)).all()
    for key in ["opt_moves", "moves_so_far", "seed", "generator_variant", "layout", "puzzle_code",
                "canonical_hash", "action_mask", "move_limit"]:
        assert key in info
    assert info["moves_so_far"] == 0
    assert info["move_limit"] == 4 * info["opt_moves"]
    assert info["generator_variant"] == "fisher_yates"
    assert (info["action_mask"] == env.action_masks()).all()
    assert w.generate("uniform", P, seed=info["seed"]).puzzle_code == info["puzzle_code"]


def test_reset_is_reproducible():
    for generator in ["uniform", "turan"]:
        a, b = WaterSortEnv(generator), WaterSortEnv(generator)
        codes_a = [a.reset(seed=5)[1]["puzzle_code"]] + [a.reset()[1]["puzzle_code"] for _ in range(3)]
        codes_b = [b.reset(seed=5)[1]["puzzle_code"]] + [b.reset()[1]["puzzle_code"] for _ in range(3)]
        assert codes_a == codes_b
        assert len(set(codes_a)) == 4
        assert a.reset(seed=6)[1]["puzzle_code"] != codes_a[0]


def test_reset_options():
    env = WaterSortEnv()
    _, info = env.reset(options={"puzzle_seed": 42})
    puzzle = w.generate("uniform", P, seed=42)
    assert info["puzzle_code"] == puzzle.puzzle_code and info["seed"] == 42
    assert info["opt_moves"] == puzzle.opt_moves
    _, info = env.reset(options={"puzzle_code": puzzle.puzzle_code})
    assert info["puzzle_code"] == puzzle.puzzle_code and info["seed"] is None
    assert info["opt_moves"] == puzzle.opt_moves
    _, info = env.reset(options={"puzzle": puzzle})
    assert info["seed"] == 42
    with pytest.raises(ValueError):
        env.reset(options={"puzzle_code": w.generate("uniform", (4, 4, 2), seed=1).puzzle_code})
    with pytest.raises(ValueError):
        env.reset(options={"puzzle_seed": 1, "puzzle_code": puzzle.puzzle_code})


def test_optimal_play_solves_with_minus_one_rewards():
    env = WaterSortEnv()
    _, info = env.reset(seed=3)
    puzzle = w.generate("uniform", P, seed=info["seed"])
    total = 0.0
    for i, (f, t) in enumerate(puzzle.solution):
        obs, r, terminated, truncated, info = env.step(f * P.n_tubes + t)
        total += r
        assert not info["illegal"] and info["moves_so_far"] == i + 1
        assert terminated == (i == len(puzzle.solution) - 1)
        assert not truncated
    assert info["solved"] and not info["dead_end"]
    assert total == -puzzle.opt_moves


def test_illegal_action_and_truncation():
    env = WaterSortEnv(move_limit_k=1)
    _, info = env.reset(seed=0)
    illegal = int(np.flatnonzero(~info["action_mask"])[0])
    before = env.state
    for i in range(info["opt_moves"]):
        obs, r, terminated, truncated, info = env.step(illegal)
        assert env.state == before and info["illegal"] and r == -1.0
        assert not terminated
        assert truncated == (i == info["opt_moves"] - 1)


def test_dead_end_terminates():
    env = WaterSortEnv(params=(3, 2, 1))
    # One pour (2 -> 3) from a dead end: no legal move after it.
    env.reset(options={"puzzle_code": w.State.from_tubes([[0, 2], [1, 2], [0, 1], []], 2).puzzle_code})
    _, r, terminated, truncated, info = env.step(2 * 4 + 3)
    assert terminated and info["dead_end"] and not info["solved"] and not truncated


def test_solver_dead_end_check():
    start = w.State.from_tubes([[], [0, 1, 1], [0, 2, 2], [0, 1, 2]], 3)
    assert w.legal_moves(start) and not w.is_dead_end(start) and w.is_dead_end(start, 10_000)
    # A move after which legal moves remain but the position is still unsolvable.
    action = next(
        f * 4 + t
        for f, t in w.legal_moves(start)
        if not w.is_dead_end(w.step(start, (f, t))[0])
    )
    for check, expected in [(True, True), (False, False)]:
        env = WaterSortEnv(params=(3, 3, 1), dead_end_check=check)
        env._start(start, 10, None)
        _, _, terminated, _, info = env.step(action)
        assert terminated == info["dead_end"] == expected


def test_shaping_reward():
    env = WaterSortEnv(shaping=True, gamma=0.9)
    _, info = env.reset(seed=2)
    s = env.state
    a = int(np.flatnonzero(info["action_mask"])[0])
    _, r, *_ = env.step(a)
    nxt, _ = w.step(s, a)
    assert r == -1.0 + (0.9 * -nxt.color_changes() - -s.color_changes())


def test_min_opt_and_render():
    env = WaterSortEnv(min_opt=19, render_mode="ansi")
    for i in range(3):
        _, info = env.reset(seed=i)
        assert info["opt_moves"] >= 19
    assert env.render() == str(env.state)
    with pytest.raises(ValueError):
        WaterSortEnv(render_mode="human")
    with pytest.raises(ValueError):
        WaterSortEnv(strategy="scramble")


def test_env_pickles():
    env = WaterSortEnv(generator="turan")
    env.reset(seed=0)
    clone = pickle.loads(pickle.dumps(env))
    assert clone.state == env.state
