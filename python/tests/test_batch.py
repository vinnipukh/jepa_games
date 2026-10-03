import threading
import time

import numpy as np
import pytest

import jepa_water_sort as w

P = w.Params(6, 4, 2)
NO_ROLLOUTS = w.GenConfig(random_rollouts=0)


def random_transitions(n, seed, layout="standard"):
    """`n` (state, action) pairs from random walks with a mix of legal and illegal actions."""
    rng = np.random.default_rng(seed)
    puzzles = w.batch_generate("uniform", P, seeds=list(range(64)), config=NO_ROLLOUTS, layout=layout)
    states, actions = [], []
    s = puzzles[0].state
    for i in range(n):
        if i % 50 == 0:
            s = puzzles[(i // 50) % len(puzzles)].state
        legal = np.flatnonzero(w.action_mask(s))
        if len(legal) == 0 or rng.random() < 0.2:
            a = int(rng.integers(P.n_tubes**2))
        else:
            a = int(rng.choice(legal))
        states.append(s.to_numpy())
        actions.append(a)
        try:
            s, _ = w.step(s, a)
        except ValueError:
            pass
    return np.stack(states), np.array(actions)


@pytest.mark.parametrize("layout", ["standard", "distributed"])
def test_batch_step_matches_looped_step(layout):
    states, actions = random_transitions(10_000, seed=1, layout=layout)
    nxt, units = w.batch_step(states, actions)
    assert nxt.shape == states.shape and nxt.dtype == np.uint8
    assert units.shape == (10_000,) and units.dtype == np.uint8
    illegal = 0
    for i in range(len(actions)):
        s = w.State.from_numpy(states[i])
        try:
            expected, k = w.step(s, int(actions[i]))
        except ValueError:
            expected, k = s, 0
            illegal += 1
        assert units[i] == k
        assert w.State.from_numpy(nxt[i]) == expected
    assert 1000 < illegal < 5000  # both branches are exercised


def test_batch_step_small_and_empty():
    s = w.State.from_tubes([[0, 1], [1, 0], []], capacity=2)
    nxt, units = w.batch_step(s.to_numpy()[None], [2])
    assert units.tolist() == [1]
    assert w.State.from_numpy(nxt[0]) == w.step(s, 2)[0]
    nxt, units = w.batch_step(np.zeros((0, 3, 2), np.uint8), np.zeros(0, np.int64))
    assert nxt.shape == (0, 3, 2) and units.shape == (0,)


def test_batch_step_errors():
    s = w.State.from_tubes([[0, 1], [1, 0], []], capacity=2).to_numpy()[None]
    with pytest.raises(ValueError, match="actions"):
        w.batch_step(s, [9])
    with pytest.raises(ValueError, match="actions"):
        w.batch_step(s, [-1])
    with pytest.raises(ValueError):
        w.batch_step(s, [1, 2])
    bad = np.repeat(s, 300, axis=0)
    bad[123, 0, 0] = 7
    with pytest.raises(ValueError, match=r"states\[123\]"):
        w.batch_step(bad, np.zeros(300, np.int64))


@pytest.mark.parametrize(
    "generator,strategy,layout",
    [
        ("uniform", None, "standard"),
        ("uniform", None, "distributed"),
        ("turan", None, "standard"),
        ("turan", "pour_walk", "distributed"),
    ],
)
def test_batch_generate_matches_generate(generator, strategy, layout):
    seeds = [0, 1, 2**63, 2**64 - 1, 12345]
    batch = w.batch_generate(generator, P, seeds=seeds, strategy=strategy, layout=layout)
    single = [w.generate(generator, P, seed=s, strategy=strategy, layout=layout) for s in seeds]
    assert batch == single
    assert [p.seed for p in batch] == seeds


def test_batch_generate_count_and_errors():
    ps = w.batch_generate("turan", P, count=8, config=NO_ROLLOUTS)
    assert len(ps) == 8 and len({p.seed for p in ps}) == 8
    with pytest.raises(ValueError):
        w.batch_generate("uniform", P)
    with pytest.raises(ValueError):
        w.batch_generate("uniform", P, seeds=[1], count=1)
    with pytest.raises(w.GenerationError):
        w.batch_generate("uniform", P, seeds=[1, 2], config=w.GenConfig(min_opt=999, max_attempts=2))


def test_batch_generate_releases_the_gil():
    """A Python thread keeps running while a long batch generates."""
    ticks = []
    stop = threading.Event()

    def ticker():
        while not stop.is_set():
            ticks.append(time.perf_counter())
            time.sleep(0.001)

    t = threading.Thread(target=ticker)
    t.start()
    start = time.perf_counter()
    w.batch_generate("uniform", w.Params(8, 4, 2), seeds=list(range(400)), config=NO_ROLLOUTS)
    end = time.perf_counter()
    stop.set()
    t.join()
    during = [x for x in ticks if start < x < end]
    assert end - start > 0.05
    assert len(during) >= 5
