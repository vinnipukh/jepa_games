import numpy as np
import pytest
from gymnasium.vector import AsyncVectorEnv, SyncVectorEnv

import jepa_water_sort as w
from jepa_water_sort import WaterSortEnv, WaterSortVectorEnv

KWARGS = [
    {"params": (4, 3, 1), "move_limit_k": 1},
    {"params": (5, 4, 2), "generator": "turan", "shaping": True, "gamma": 0.9},
    {"params": (4, 4, 2), "layout": "distributed", "move_limit_k": 2},
]


def random_actions(rng, masks, p_illegal=0.2):
    """A legal action per row, or with probability ``p_illegal`` any action."""
    out = []
    for mask in masks:
        legal = np.flatnonzero(mask)
        if len(legal) == 0 or rng.random() < p_illegal:
            out.append(int(rng.integers(len(mask))))
        else:
            out.append(int(rng.choice(legal)))
    return np.array(out)


@pytest.mark.parametrize("kwargs", KWARGS)
def test_matches_sync_vector_env(kwargs):
    n = 4
    native = WaterSortVectorEnv(n, **kwargs)
    sync = SyncVectorEnv([lambda: WaterSortEnv(**kwargs) for _ in range(n)])
    assert native.single_observation_space == sync.single_observation_space
    assert native.single_action_space == sync.single_action_space
    assert native.observation_space == sync.observation_space
    obs_a, info_a = native.reset(seed=10)
    obs_b, info_b = sync.reset(seed=10)
    assert (obs_a == obs_b).all()
    rng = np.random.default_rng(0)
    episodes = 0
    for t in range(300):
        for key in ["action_mask", "opt_moves", "moves_so_far", "seed", "canonical_hash",
                    "puzzle_code"]:
            assert (np.asarray(info_a[key]) == np.asarray(info_b[key])).all(), (t, key)
        assert (native.action_masks() == info_b["action_mask"]).all()
        actions = random_actions(rng, info_b["action_mask"])
        obs_a, r_a, term_a, trunc_a, info_a = native.step(actions)
        obs_b, r_b, term_b, trunc_b, info_b = sync.step(actions)
        assert (obs_a == obs_b).all(), t
        assert (r_a == r_b).all(), t
        assert (term_a == term_b).all() and (trunc_a == trunc_b).all(), t
        for key in ["illegal", "dead_end", "solved", "units_moved"]:
            if key in info_b:
                m = info_b[f"_{key}"]
                assert (info_a[f"_{key}"] == m).all()
                assert (info_a[key][m] == info_b[key][m]).all(), (t, key)
        episodes += int((term_a | trunc_a).sum())
    assert episodes > 10
    # Reseeding restarts the same sequence.
    assert (native.reset(seed=10)[0] == sync.reset(seed=10)[0]).all()


def test_async_vector_env():
    kwargs = {"params": (4, 3, 1)}
    # "spawn": forking after the rayon pool has started can deadlock the children.
    envs = AsyncVectorEnv([lambda: WaterSortEnv(**kwargs) for _ in range(2)], context="spawn")
    native = WaterSortVectorEnv(2, **kwargs)
    obs_a, info = envs.reset(seed=3)
    obs_b, _ = native.reset(seed=3)
    assert (obs_a == obs_b).all()
    rng = np.random.default_rng(1)
    for _ in range(20):
        actions = random_actions(rng, info["action_mask"], 0.0)
        obs_a, r_a, *_, info = envs.step(actions)
        obs_b, r_b, *_ = native.step(actions)
        assert (obs_a == obs_b).all() and (r_a == r_b).all()
    envs.close()


def test_batch_env_step_matches_env_step():
    rng = np.random.default_rng(2)
    puzzles = w.batch_generate("uniform", (5, 4, 2), seeds=list(range(300)),
                               config=w.GenConfig(random_rollouts=0))
    states = [p.state for p in puzzles]
    moves = rng.integers(0, 30, size=len(states)).astype(np.uint32)
    limits = rng.integers(1, 30, size=len(states)).astype(np.uint32)
    masks = w.batch_action_mask(np.stack([s.to_numpy() for s in states]))
    actions = random_actions(rng, masks, 0.3)
    for gamma in [None, 0.97]:
        out = w.batch_env_step(np.stack([s.to_numpy() for s in states]), actions, moves, limits, gamma)
        for i, s in enumerate(states):
            nxt, units, reward, term, trunc, illegal, dead, solved = w.env_step(
                s, int(actions[i]), int(moves[i]), int(limits[i]), gamma)
            assert w.State.from_numpy(out[0][i]) == nxt
            assert (out[1][i], out[2][i], out[3][i], out[4][i], out[5][i], out[6][i],
                    out[7][i]) == (units, reward, term, trunc, illegal, dead, solved)
    with pytest.raises(ValueError):
        w.batch_env_step(np.stack([states[0].to_numpy()]), [10_000], [0], [1])
    with pytest.raises(ValueError):
        w.batch_env_step(np.stack([states[0].to_numpy()]), [0, 1], [0], [1])


def test_vector_env_basics():
    env = WaterSortVectorEnv(3)
    obs, info = env.reset(seed=0)
    assert obs.shape == (3, 8, 4, 7) and obs.dtype == np.int8
    assert env.action_space.shape == (3,)
    assert info["action_mask"].shape == (3, 64)
    with pytest.raises(ValueError):
        WaterSortVectorEnv(0)
    with pytest.raises(ValueError):
        env.reset(seed=[1, 2])
