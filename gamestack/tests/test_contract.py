"""Contract every game must satisfy (the same API for RL/JEPA agents and LLMs)."""

import numpy as np
import pytest

from gamestack import MixEnv, list_games, make

GAMES = list_games()
KW = {"crafter": {"length": 150}}


def _env(name):
    try:
        return make(name, **KW.get(name, {}))
    except ImportError as e:
        pytest.skip(str(e))


def _flat(obs):
    return np.concatenate([np.ravel(v) for v in obs.values()]) if isinstance(obs, dict) else np.ravel(obs)


@pytest.mark.parametrize("name", GAMES)
def test_seed_determinism(name):
    a, b = _env(name), _env(name)
    oa, _ = a.reset(seed=7)
    ob, _ = b.reset(seed=7)
    assert (_flat(oa) == _flat(ob)).all()
    assert a.text_observation() == b.text_observation()
    rng = np.random.default_rng(0)
    for _ in range(30):
        legal = a.legal_actions()
        if not legal:
            break
        act = legal[int(rng.integers(len(legal)))]
        ra, rb = a.step(act), b.step(act)
        assert (_flat(ra[0]) == _flat(rb[0])).all() and ra[1] == rb[1] and ra[2] == rb[2]
        if ra[2] or ra[3]:
            break


@pytest.mark.parametrize("name", GAMES)
def test_random_legal_play_terminates_and_stays_in_space(name):
    env = _env(name)
    rng = np.random.default_rng(1)
    for ep in range(3):
        obs, _ = env.reset(seed=ep)
        for _ in range(3000):
            assert env.observation_space.contains(obs), (name, obs)
            legal = env.legal_actions()
            assert legal and all(env.action_space.contains(a) for a in legal)
            obs, r, term, trunc, info = env.step(legal[int(rng.integers(len(legal)))])
            assert not info.get("illegal"), "a legal action was rejected"
            if term or trunc:
                break
        else:
            pytest.fail(f"{name} episode did not end in 3000 steps")


@pytest.mark.parametrize("name", GAMES)
def test_text_roundtrip(name):
    env = _env(name)
    env.reset(seed=3)
    assert env.text_observation() and env.action_prompt()
    names = env.action_names()
    assert names
    for action, text in list(names.items())[:20]:
        parsed = env.parse_action(f"I think the best move is {text}.")
        assert parsed == action, (name, text, parsed, action)
    assert env.parse_action("no idea what to do") is None


@pytest.mark.parametrize("name", GAMES)
def test_illegal_text_penalised_without_state_change(name):
    env = _env(name)
    env.reset(seed=4)
    before = env.text_observation()
    obs, r, term, trunc, info = env.step_text("gibberish ###")
    assert info["illegal"] and r == env.illegal_reward and not term
    assert env.text_observation() == before


def test_mix_env():
    mix = MixEnv([g for g in GAMES if g != "water_sort"])
    seen = set()
    for ep in range(12):
        text, info = mix.reset(seed=ep)
        seen.add(info["game"])
        assert text.startswith(f"[{info['game']}]")
        for _ in range(5):
            names = info["legal_actions"]
            if not names:
                break
            text, r, term, trunc, info = mix.step(names[0])
            assert not info.get("illegal")
            if term or trunc:
                break
    assert len(seen) >= 4
