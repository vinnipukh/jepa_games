import hashlib
import json

import numpy as np
import pyarrow.parquet as pq
import pytest

import jepa_water_sort as w
from jepa_water_sort import logger, policies
from jepa_water_sort.dataset import Dataset


@pytest.fixture(scope="module")
def uniform_ds(repo):
    return Dataset(repo / "water_sort_cli/tests/fixtures/datasets/uniform_c5k4e2")


@pytest.fixture(scope="module")
def turan_ds(repo):
    return Dataset(repo / "water_sort_cli/tests/fixtures/datasets/turan_pour_walk_distributed_c4k3e1")


def check_transitions(table, params):
    """Every row is a real core transition and episodes are contiguous."""
    arrays = logger.to_numpy(table, params)
    for i in range(len(table)):
        s = w.State.from_numpy(arrays["state"][i])
        nxt = w.State.from_numpy(arrays["next_state"][i])
        try:
            expected, _ = w.step(s, int(arrays["action"][i]))
            assert not arrays["illegal"][i]
        except ValueError:
            expected = s
            assert arrays["illegal"][i]
        assert nxt == expected
        assert arrays["solved"][i] == nxt.is_solved()
        if i + 1 < len(table) and arrays["episode"][i + 1] == arrays["episode"][i]:
            assert not arrays["done"][i]
            assert (arrays["state"][i + 1] == arrays["next_state"][i]).all()
            assert arrays["step"][i + 1] == arrays["step"][i] + 1
    return arrays


def test_dataset_reader(uniform_ds, turan_ds):
    val = uniform_ds.records("val")
    assert len(val) == uniform_ds.manifest["split"]["counts"]["val"]
    assert all(r.split == "val" for r in val)
    assert [r.record_id for r in val] == sorted(r.record_id for r in val)
    r = val[0]
    assert w.canonical_hash(r.state) == r.canonical_hash
    assert r.puzzle_id == f"uniform:{r.canonical_hash:016x}"
    assert len(uniform_ds.records()) == uniform_ds.manifest["records"]
    jsonl = turan_ds.records("train")
    assert jsonl and all(w.canonical_hash(x.state) == x.canonical_hash for x in jsonl)
    assert all(x.layout == "distributed" for x in jsonl)
    with pytest.raises(ValueError):
        uniform_ds.records("nope")


def test_collect_optimal(uniform_ds, tmp_path):
    m = logger.collect(uniform_ds, "val", "optimal", tmp_path / "opt")
    records = uniform_ds.records("val")
    assert m["episodes"] == len(records) == m["solved_episodes"]
    assert m["transitions"] == sum(r.opt_moves for r in records)
    assert m["dataset"]["manifest_sha256"] == uniform_ds.manifest_sha256
    assert m["split"] == "val" and m["source"] == "optimal" and m["epsilon"] is None
    table = logger.read_transitions(tmp_path / "opt")
    check_transitions(table, uniform_ds.params)
    assert set(table.column("split").to_pylist()) == {"val"}
    assert set(table.column("puzzle_id").to_pylist()) == {r.puzzle_id for r in records}
    # The optimal source replays the stored solutions.
    first = [a for a, e in zip(table.column("action").to_pylist(),
                               table.column("episode").to_pylist()) if e == 0]
    assert tuple(first) == records[0].solution
    shard = m["shards"][0]
    assert hashlib.sha256((tmp_path / "opt" / shard["path"]).read_bytes()).hexdigest() == shard["sha256"]
    with pytest.raises(FileExistsError):
        logger.collect(uniform_ds, "val", "optimal", tmp_path / "opt")


def test_split_isolation(uniform_ds, tmp_path):
    logger.collect(uniform_ds, "test", "random", tmp_path / "t", policy_seed=1)
    hashes = set(logger.read_transitions(tmp_path / "t").column("canonical_hash").to_pylist())
    train = {r.canonical_hash for r in uniform_ds.records("train")}
    assert hashes and not hashes & train
    assert all(uniform_ds.split_of(h) == "test" for h in hashes)


@pytest.mark.parametrize("source,epsilon", [("random", None), ("epsilon", 0.3), ("greedy", None)])
def test_collect_policies_are_reproducible(uniform_ds, tmp_path, source, epsilon):
    a = logger.collect(uniform_ds, "val", source, tmp_path / "a", epsilon=epsilon, policy_seed=7,
                       episodes_per_puzzle=2)
    b = logger.collect(uniform_ds, "val", source, tmp_path / "b", epsilon=epsilon, policy_seed=7,
                       episodes_per_puzzle=2)
    assert [s["sha256"] for s in a["shards"]] == [s["sha256"] for s in b["shards"]]
    c = logger.collect(uniform_ds, "val", source, tmp_path / "c", epsilon=epsilon, policy_seed=8,
                       episodes_per_puzzle=2)
    assert a["episodes"] == c["episodes"] == 2 * len(uniform_ds.records("val"))
    table = logger.read_transitions(tmp_path / "a")
    arrays = check_transitions(table, uniform_ds.params)
    assert not arrays["illegal"].any()
    # Episodes end in a solved state, a dead end, or at the move limit.
    ends = np.flatnonzero(arrays["done"])
    assert len(ends) == a["episodes"]
    if source == "epsilon":
        assert a["epsilon"] == 0.3


def test_epsilon_extremes_match_optimal_and_random():
    p = w.generate("uniform", (5, 4, 2), seed=3)
    rng = lambda: np.random.default_rng(0)  # noqa: E731
    opt = logger.run_episode(p.state, policies.OptimalPolicy(), rng(), p.opt_moves)
    eps0 = logger.run_episode(p.state, policies.EpsilonPolicy(0.0), rng(), p.opt_moves)
    assert [t.action for t in opt] == [t.action for t in eps0]
    assert len(opt) == p.opt_moves and opt[-1].solved
    # `opt` solved on its own; with the stored solution the policy replays it.
    assert len(logger.run_episode(p.state, policies.OptimalPolicy(), rng(), p.opt_moves,
                                  solution=p.solution_actions)) == p.opt_moves
    eps1 = logger.run_episode(p.state, policies.EpsilonPolicy(1.0), rng(), p.opt_moves)
    assert all(not t.illegal for t in eps1)
    with pytest.raises(ValueError):
        policies.EpsilonPolicy(1.5)
    with pytest.raises(ValueError):
        policies.make_policy("epsilon")


def test_shards_never_split_episodes(uniform_ds, tmp_path):
    m = logger.collect(uniform_ds, "train", "optimal", tmp_path / "s", shard_size=200, limit=40)
    assert len(m["shards"]) > 2
    assert sum(s["transitions"] for s in m["shards"]) == m["transitions"]
    seen = set()
    for s in m["shards"]:
        eps = set(pq.read_table(tmp_path / "s" / s["path"]).column("episode").to_pylist())
        assert not eps & seen and len(eps) == s["episodes"]
        seen |= eps
    assert len(seen) == m["episodes"] == 40


def test_jsonl_dataset(turan_ds, tmp_path):
    m = logger.collect(turan_ds, "train", "epsilon", tmp_path / "j", epsilon=0.2, limit=20)
    assert m["episodes"] == 20
    table = logger.read_transitions(tmp_path / "j")
    check_transitions(table, turan_ds.params)
    assert set(table.column("generator_id").to_pylist()) == {"turan"}


def test_import_human(tmp_path):
    p = w.generate("uniform", (4, 4, 2), seed=1)
    illegal = int(np.flatnonzero(~w.action_mask(p.state))[0])
    games = [{"puzzle_code": p.puzzle_code, "actions": [illegal] + p.solution_actions,
              "generator_id": "uniform", "opt_moves": p.opt_moves}]
    m = logger.import_human(games, tmp_path / "h", p.params)
    assert m["source"] == "human" and m["episodes"] == 1
    assert m["transitions"] == p.opt_moves + 1 and m["illegal_transitions"] == 1
    assert m["solved_episodes"] == 1
    table = logger.read_transitions(tmp_path / "h")
    check_transitions(table, p.params)
    assert table.column("puzzle_id")[0].as_py() == f"uniform:{p.canonical_hash:016x}"


WEB_EXPORT = "python/tests/fixtures/human_export_uniform_4x4_2.json"


@pytest.fixture
def web_export(repo):
    """Exported from the web game (Phase 6): an illegal pour, a pour, undo, a pour, restart,
    then the optimal solution."""
    return json.loads((repo / WEB_EXPORT).read_text(encoding="utf-8"))


def test_web_export_opens_the_same_puzzle_as_python(web_export):
    pz = web_export["puzzle"]
    p = w.generate(pz["generator_id"], tuple(pz["params"].values()), seed=int(pz["seed"], 16),
                   config=w.GenConfig.from_json(json.dumps(pz["config"])))
    assert p.puzzle_code == pz["puzzle_code"]
    assert f"{p.canonical_hash:016x}" == pz["canonical_hash"]
    assert p.opt_moves == pz["opt_moves"] and p.generator_variant == pz["variant"]
    assert w.stars(web_export["moves_counted"], p.opt_moves) == web_export["stars"]


def test_import_web_exports(web_export, tmp_path):
    segments = logger.human_segments(web_export)
    # illegal + pour | undo | pour | restart | solution
    assert [len(a) for _, a in segments] == [2, 1, web_export["puzzle"]["opt_moves"]]
    m = logger.import_web_exports([web_export, web_export], tmp_path / "h")
    assert m["source"] == "human" and m["episodes"] == 6
    assert m["solved_episodes"] == 2 and m["truncated_episodes"] == 0
    assert m["illegal_transitions"] == 2
    assert [s["first_episode"] for s in m["sessions"]] == [0, 3]
    assert m["sessions"][0]["stars"] == web_export["stars"]
    table = logger.read_transitions(tmp_path / "h")
    check_transitions(table, w.Params(4, 4, 2))
    assert table.num_rows == 2 * (3 + web_export["puzzle"]["opt_moves"])
    assert set(table.column("puzzle_id").to_pylist()) == {web_export["puzzle"]["puzzle_id"]}
    # The rows of every pour equal the export's rows.
    pours = [r for r in web_export["rows"] if r["event"] in ("pour", "illegal")]
    first = table.slice(0, len(pours)).to_pylist()
    for got, row in zip(first, pours):
        assert got["state"] == bytes(row["state"]) and got["next_state"] == bytes(row["next_state"])
        assert got["action"] == row["action"] and got["illegal"] == row["illegal"]


def test_import_human_cli(repo, tmp_path):
    logger.main(["import-human", str(repo / WEB_EXPORT), "--out", str(tmp_path / "cli")])
    assert logger.read_manifest(tmp_path / "cli")["episodes"] == 3


@pytest.mark.parametrize("tamper", ["state", "next_state", "units", "moves", "illegal", "hash",
                                    "format", "undo"])
def test_web_export_tampering_is_rejected(web_export, tmp_path, tamper):
    rows = web_export["rows"]
    pour = next(i for i, r in enumerate(rows) if r["event"] == "pour")
    if tamper == "state":
        rows[pour]["state"][0] = (rows[pour]["state"][0] + 1) % 4
    elif tamper == "next_state":
        rows[pour]["next_state"] = rows[pour]["state"]
    elif tamper == "units":
        rows[pour]["units_moved"] += 1
    elif tamper == "moves":
        rows[pour]["moves_counted"] += 1
    elif tamper == "illegal":
        rows[0]["illegal"] = False
    elif tamper == "hash":
        web_export["puzzle"]["canonical_hash"] = "0" * 16
    elif tamper == "format":
        web_export["format_version"] = 99
    elif tamper == "undo":
        rows.insert(0, {**rows[0], "event": "undo", "action": None, "illegal": False})
    with pytest.raises(ValueError):
        logger.import_web_exports([web_export], tmp_path / "t")
    assert not (tmp_path / "t" / "manifest.json").exists()


def test_web_exports_with_different_params_are_rejected(web_export, tmp_path):
    other = json.loads(json.dumps(web_export))
    other["puzzle"]["params"]["n_empty"] = 1
    with pytest.raises(ValueError, match="different params"):
        logger.import_web_exports([web_export, other], tmp_path / "p")


def test_export_npz_and_cli(uniform_ds, tmp_path):
    logger.main(["collect", "--dataset", str(uniform_ds.path), "--split", "val", "--source",
                 "epsilon", "--epsilon", "0.1", "--policy-seed", "0x2a", "--out",
                 str(tmp_path / "cli")])
    m = json.loads((tmp_path / "cli" / "manifest.json").read_text())
    assert m["policy_seed"] == f"{0x2a:016x}"
    logger.main(["export-npz", str(tmp_path / "cli"), "--out", str(tmp_path / "t.npz")])
    data = np.load(tmp_path / "t.npz")
    assert data["state"].shape == (m["transitions"], 7, 4)
    obs = w.encode_observation(data["state"], 5)
    assert obs.shape == (m["transitions"], 7, 4, 6)
    assert (w.decode_observation(obs) == data["state"]).all()
    table = logger.read_transitions(tmp_path / "cli")
    assert (logger.to_numpy(table, uniform_ds.params)["next_state"] == data["next_state"]).all()


def test_logger_validation(tmp_path):
    with pytest.raises(ValueError):
        logger.TrajectoryLogger(tmp_path / "x", w.Params(4), "nope")
    log = logger.TrajectoryLogger(tmp_path / "y", w.Params(4), "random")
    p = w.generate("uniform", (5, 4, 2), seed=1)
    t = logger.run_episode(p.state, policies.RandomPolicy(), np.random.default_rng(0), p.opt_moves)
    with pytest.raises(ValueError):
        log.log_episode(t, "uniform")
    m = log.close()
    assert m["transitions"] == 0 and m["shards"] == []
    assert len(logger.read_transitions(tmp_path / "y")) == 0
