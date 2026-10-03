"""Tests for the Phase 7 ``jepa`` package (skipped when torch is not installed)."""

from __future__ import annotations

import json
import math

import numpy as np
import pytest

torch = pytest.importorskip("torch")

import jepa_water_sort as w  # noqa: E402
from jepa import baselines, data, eval as jeval, report  # noqa: E402
from jepa.config import (  # noqa: E402
    ModelConfig, PlanConfig, TrainConfig, load_config, save_config,
)
from jepa.models import WorldModel, ema_momentum  # noqa: E402
from jepa.monitor import effective_rank  # noqa: E402
from jepa.plan import LatentPlanner  # noqa: E402
from jepa_water_sort import logger  # noqa: E402

FIXTURE = "water_sort_cli/tests/fixtures/datasets/uniform_c5k4e2"


@pytest.fixture(scope="module")
def fixture_dataset(repo):
    return repo / FIXTURE


@pytest.fixture(scope="module")
def trajectories(fixture_dataset, tmp_path_factory):
    root = tmp_path_factory.mktemp("traj")
    out = {}
    for split, source in (("train", "optimal"), ("train", "random"), ("val", "epsilon")):
        d = root / f"{split}_{source}"
        logger.collect(fixture_dataset, split, source, d, epsilon=0.2, policy_seed=3, limit=40)
        out[(split, source)] = d
    return out


# -- data ---------------------------------------------------------------------------------------


def test_load_shapes_and_labels(trajectories):
    t = data.load([trajectories[("train", "optimal")], trajectories[("train", "random")]])
    n = len(t)
    assert t.state.shape == (n, 7, 4) and t.next_state.shape == (n, 7, 4)
    assert t.mask.shape == (n, 49) and t.mask[np.arange(n), t.action].all()
    assert t.sources == ("optimal", "random") and t.split == "train"
    # chain: the next row of the same episode, whose state is this row's next state
    linked = np.flatnonzero(t.chain >= 0)
    assert len(linked) > 0
    assert (t.state[t.chain[linked]] == t.next_state[linked]).all()
    assert (t.chain[t.done] == -1).all()
    # distance labels: 0 exactly on solved next states; optimal rows count down by one
    assert ((t.next_togo == 0) == t.solved).all()
    opt = t.source == 0
    assert (t.togo[opt] == t.next_togo[opt] + 1).all()
    # labels are cached and reloaded identically
    again = data.load([trajectories[("train", "optimal")], trajectories[("train", "random")]])
    assert (again.togo == t.togo).all() and (again.next_togo == t.next_togo).all()


def test_mixed_splits_refused(trajectories):
    with pytest.raises(ValueError, match="different splits"):
        data.load([trajectories[("train", "optimal")], trajectories[("val", "epsilon")]])


def test_one_hot_matches_env_observation():
    p = w.generate("uniform", (5, 4, 2), seed=4)
    cells = p.state.to_numpy()
    ours = data.one_hot(torch.as_tensor(cells), 5).numpy().astype(np.int8)
    assert (ours == w.encode_observation(cells, 5)).all()


def test_device_batch_multistep(trajectories):
    t = data.load([trajectories[("train", "optimal")]])
    d = data.DeviceData(t, torch.device("cpu"), 3)
    b = d.batch(torch.arange(len(t)))
    assert b["actions"].shape == (len(t), 3) and b["next_states"].shape == (len(t), 3, 7, 4)
    v = b["valid"].numpy()
    # step k exists iff the episode continues k more rows
    i = int(np.flatnonzero(v[:, 2])[0])
    assert (b["next_states"][i, 2].numpy() == t.next_state[t.chain[t.chain[i]]]).all()
    assert not v[t.done, 1].any()


# -- models -------------------------------------------------------------------------------------


def _model(dim=32):
    torch.manual_seed(0)
    return WorldModel(5, 4, ModelConfig(dim=dim, encoder_layers=1, predictor_layers=1)).eval()


def test_forward_shapes():
    m = _model()
    x = data.one_hot(torch.as_tensor(np.stack([w.generate("uniform", (5, 4, 2), seed=s).state.to_numpy()
                                               for s in range(3)])), 5)
    h = m.encode(x)
    assert h.shape == (3, 7, 32)
    a = torch.tensor([0 * 7 + 5, 1 * 7 + 6, 2 * 7 + 5])
    assert m.predict(h, a).shape == (3, 7, 32)
    idm = m.idm_logits(h, h)
    assert idm.shape == (3, 49) and torch.isinf(idm.view(3, 7, 7).diagonal(dim1=1, dim2=2)).all()
    assert m.legal_logits(h).shape == (3, 49)
    assert m.probe(h).shape == (3, 7, 4, 6)
    assert m.solved_logit(h).shape == (3,) and (m.distance(h) >= 0).all()


def test_encoder_and_predictor_are_tube_equivariant():
    m = _model()
    cells = torch.as_tensor(w.generate("uniform", (5, 4, 2), seed=9).state.to_numpy())[None]
    perm = torch.tensor([3, 0, 6, 1, 5, 2, 4])
    inv = torch.argsort(perm)
    with torch.no_grad():
        h = m.encode(data.one_hot(cells, 5))
        hp = m.encode(data.one_hot(cells[:, perm], 5))
        assert torch.allclose(hp, h[:, perm], atol=1e-5)
        a_from, a_to = 1, 5
        out = m.predict(h, torch.tensor([a_from * 7 + a_to]))
        outp = m.predict(hp, torch.tensor([int(inv[a_from]) * 7 + int(inv[a_to])]))
        assert torch.allclose(outp, out[:, perm], atol=1e-5)


def test_ema_schedule_and_update():
    assert ema_momentum(0, 100, 0.996, 1.0) == pytest.approx(0.996)
    assert ema_momentum(99, 100, 0.996, 1.0) == pytest.approx(1.0)
    m = _model()
    with torch.no_grad():
        for p in m.encoder.parameters():
            p.add_(1.0)
    before = [p.clone() for p in m.target_encoder.parameters()]
    m.update_target(0.9)
    for b, t, s in zip(before, m.target_encoder.parameters(), m.encoder.parameters()):
        assert torch.allclose(t, 0.9 * b + 0.1 * s)


def test_effective_rank():
    torch.manual_seed(0)
    assert effective_rank(torch.randn(4096, 16)) == pytest.approx(16, rel=0.05)
    collapsed = torch.randn(512, 1) * torch.ones(1, 16)
    assert effective_rank(collapsed) == pytest.approx(1.0, abs=0.01)


def test_config_roundtrip(tmp_path):
    cfg = TrainConfig(seed=4)
    cfg.model.dim = 48
    cfg.data.train = ["a", "b"]
    save_config(cfg, tmp_path / "c.json")
    back = load_config(tmp_path / "c.json")
    assert back == cfg and isinstance(back.model, ModelConfig)


def test_train_few_steps(trajectories, tmp_path):
    from jepa.train import load_model, train

    cfg = TrainConfig(max_steps=4, batch_size=16, warmup_steps=1, log_every=2, tensorboard=False,
                      device="cpu")
    cfg.model = ModelConfig(dim=32, encoder_layers=1, predictor_layers=1)
    cfg.data.train = [str(trajectories[("train", "optimal")]), str(trajectories[("train", "random")])]
    cfg.data.val = [str(trajectories[("val", "epsilon")])]
    cfg.data.monitor_size = 32
    final = train(cfg, tmp_path / "run", verbose=False)
    for key in ("effective_rank_frac", "probe_exact", "idm_acc", "pred1_exact"):
        assert key in final and math.isfinite(final[key])
    model, back = load_model(tmp_path / "run" / "model.pt")
    assert back == cfg and model.cfg.dim == 32
    lines = (tmp_path / "run" / "metrics.jsonl").read_text().splitlines()
    assert {json.loads(x)["kind"] for x in lines} == {"train", "monitor"}
    # validation trajectories from the training split are refused
    cfg.data.val = cfg.data.train
    with pytest.raises(ValueError, match="training split"):
        train(cfg, tmp_path / "run2", verbose=False)


# -- planner on an oracle world model -----------------------------------------------------------


class OracleModel(torch.nn.Module):
    """A 'world model' whose latent is the one-hot state itself, with exact dynamics, legality,
    solved flag and distance from the core. The planner must then play optimally."""

    def __init__(self, n_colors: int, capacity: int):
        super().__init__()
        self.n_colors, self.capacity = n_colors, capacity

    def _cells(self, h):
        idx = h.unflatten(-1, (self.capacity, self.n_colors + 1)).argmax(-1)
        idx = torch.where(idx == self.n_colors, torch.full_like(idx, w.EMPTY), idx)
        return idx.to(torch.uint8).numpy()

    def encode(self, x):
        return x.flatten(-2)

    def predict(self, h, action):
        nxt, _ = w.batch_step(self._cells(h), action.numpy())
        return self.encode(data.one_hot(torch.as_tensor(nxt), self.n_colors))

    def probe(self, h):
        return h.unflatten(-1, (self.capacity, self.n_colors + 1)) * 10

    def legal_logits(self, h):
        return torch.as_tensor(w.batch_action_mask(self._cells(h))).float() * 20 - 10

    def solved_logit(self, h):
        return torch.tensor([10.0 if w.State.from_numpy(c).is_solved() else -10.0
                             for c in self._cells(h)])

    def distance(self, h):
        return torch.tensor([float(w.solve(w.State.from_numpy(c), 100_000).opt_moves or 40)
                             for c in self._cells(h)])


@pytest.mark.parametrize("method", ["beam", "mcts", "cem"])
def test_planner_with_oracle_model(fixture_dataset, method):
    puzzles = jeval.test_puzzles(fixture_dataset, 4)
    cfg = PlanConfig(method=method, depth=2, width=4, mcts_simulations=16, cem_samples=32,
                     cem_elites=4, cem_iters=2)
    planner = LatentPlanner(OracleModel(5, 4), cfg)
    results = jeval.run_episodes(planner, puzzles, batch=2)
    assert all(r["solved"] for r in results)
    if method == "beam":
        # exact distance + depth: every step stays on an optimal path
        assert all(r["moves"] == r["opt_moves"] and r["stars"] == 5 for r in results)


def test_planner_legality_modes(fixture_dataset):
    puzzles = jeval.test_puzzles(fixture_dataset, 2)
    for legality in ("head", "probe", "none"):
        planner = LatentPlanner(OracleModel(5, 4), PlanConfig(depth=2, width=3, legality=legality))
        assert all(r["solved"] for r in jeval.run_episodes(planner, puzzles))


# -- baselines and metrics ----------------------------------------------------------------------


def test_baselines(fixture_dataset):
    puzzles = jeval.test_puzzles(fixture_dataset, 6)
    solver = jeval.run_episodes(baselines.Solver(), puzzles)
    assert all(r["solved"] and r["moves"] == r["opt_moves"] and r["stars"] == 5 for r in solver)
    for policy in (baselines.RandomLegal(), baselines.Greedy()):
        a = jeval.run_episodes(policy, puzzles, seed=5)
        b = jeval.run_episodes(policy, puzzles, seed=5)
        assert a == b and all(r["illegal"] == 0 for r in a)


def test_greedy_minimizes_color_changes():
    state = w.State.from_tubes([[0, 1], [2, 1], [0, 2], [], []], 2)
    pol = baselines.Greedy()
    pol.reset(1, np.random.default_rng(0))
    a = int(pol.act(state.to_numpy()[None], np.array([0]))[0])
    legal = np.flatnonzero(w.action_mask(state))
    best = min(w.step(state, int(x))[0].color_changes() for x in legal)
    assert w.step(state, a)[0].color_changes() == best


def _result(opt, moves, solved, limit_k=4, gen="uniform", rid=0):
    return {"record_id": rid, "opt_moves": opt, "moves": moves, "solved": solved,
            "move_limit": limit_k * opt, "stars": w.stars(moves, opt) if solved else 0,
            "generator": gen}


def test_metrics_hand_computed():
    rs = [_result(10, 10, True), _result(10, 15, True), _result(20, 80, False), _result(20, 25, True)]
    s = jeval.summarize(rs)
    assert s["n"] == 4 and s["solve_rate"] == 0.75
    assert s["ratio_solved"] == pytest.approx((1.0 + 1.5 + 1.25) / 3)
    assert s["ratio_all"] == pytest.approx((1.0 + 1.5 + 4.0 + 1.25) / 4)
    assert s["mean_stars"] == pytest.approx((5 + w.stars(15, 10) + 0 + w.stars(25, 20)) / 4)
    edges = jeval.bucket_edges([10, 10, 20, 20])
    assert edges == [10.0, 10.0, 20.0]
    assert [jeval.bucket_of(o, edges) for o in (9, 10, 11, 20, 21)] == [0, 0, 2, 2, 3]
    assert jeval.bucket_labels(edges) == ["≤10", "(empty)", "11–20", ">20"]
    b = jeval.breakdown(rs, edges, weights=[0.5, 0.0, 0.5, 0.0])
    assert b["buckets"][0]["solve_rate"] == 1.0 and b["buckets"][2]["solve_rate"] == 0.5
    assert b["matched_solve_rate"] == pytest.approx(0.75)
    # re-weighting removes a difficulty difference: same per-bucket rates, other mix
    b2 = jeval.breakdown([rs[0], rs[2], rs[2], rs[3]], edges, weights=[0.5, 0.0, 0.5, 0.0])
    assert b2["overall"]["solve_rate"] == 0.5 and b2["matched_solve_rate"] == pytest.approx(
        0.5 * 1.0 + 0.5 * (1 / 3))
    assert jeval.mean_std([1.0, 3.0]) == (2.0, pytest.approx(math.sqrt(2)))


def test_report_renders(tmp_path):
    recs = [
        {"policy": "greedy", "train": None, "seed": None, "test": "uniform_standard",
         "results": [_result(10, 12, True, rid=1), _result(12, 48, False, rid=2)]},
        {"policy": "jepa-beam", "train": "uniform_standard", "seed": 0, "test": "uniform_standard",
         "results": [_result(10, 10, True, rid=1), _result(12, 12, True, rid=2)], "leaked": 0},
    ]
    runs = {"uniform_standard": [{"effective_rank_frac": 0.7, "probe_exact": 0.99}]}
    info = {"preset": {"name": "t", "params": [5, 4, 2], "generators": ["uniform"],
                       "layouts": ["standard"], "seeds": [0], "default": ["uniform", "standard"],
                       "eval": {"puzzles": 2, "move_limit_k": 4, "policy_seed": 0},
                       "plan": {"method": "beam", "depth": 4, "width": 16, "score": "value",
                                "legality": "head"}},
            "command": "test", "torch": torch.__version__, "device": "cpu"}
    report.write(recs, runs, info, tmp_path / "r.md", tmp_path / "r.json")
    md = (tmp_path / "r.md").read_text(encoding="utf-8")
    assert "| planner solve rate > greedy (uniform_standard test) | 100.0 % vs 50.0 % | yes |" in md
    assert json.loads((tmp_path / "r.json").read_text())["bucket_edges"] == [10.0, 10.0, 10.0]


# -- the whole pipeline -------------------------------------------------------------------------


def test_pipeline_smoke(tmp_path):
    from jepa.pipeline import PRESETS, run

    out = tmp_path / "smoke"
    path = run(PRESETS["smoke"], out, workers=2)
    md = path.read_text(encoding="utf-8")
    assert "## Acceptance" in md and "## Cross-evaluation matrix" in md
    for policy in ("random", "greedy", "solver", "ddqn", "jepa-beam", "jepa-mcts", "jepa-cem"):
        assert f"| {policy}" in md
    js = json.loads((out / "report.json").read_text())
    solver = [g for g in js["groups"] if g["policy"] == "solver"][0]["seeds"][0]["overall"]
    assert solver["solve_rate"] == 1.0 and solver["ratio_solved"] == 1.0
    # a second run reuses every stage's output
    assert run(PRESETS["smoke"], out, workers=2) == path
