import pytest

torch = pytest.importorskip("torch")

from gamestack import make
from gamestack.rl.calibration import brier, ece, fit_temperature, predict_probs, selective_accuracy
from gamestack.rl.rlcd import RLCDConfig, log_score, rlcd_step, rps_score, score, spherical_score
from tests.test_rl import BagPolicy


@pytest.mark.parametrize("rule", [log_score, spherical_score, rps_score])
def test_scoring_rules_are_proper(rule):
    """Expected score under a true distribution q is maximised by reporting p = q."""
    q = torch.tensor([0.6, 0.3, 0.1])
    ys = torch.arange(3)

    def expected(p):
        return sum(q[y] * rule(p, ys[y]) for y in range(3))

    truthful = expected(q)
    for p in (torch.tensor([0.9, 0.05, 0.05]), torch.tensor([0.34, 0.33, 0.33]), torch.tensor([0.4, 0.4, 0.2]),
              torch.tensor([0.8, 0.1, 0.1])):
        assert truthful >= expected(p) - 1e-6, (rule.__name__, p)


def test_rps_prefers_near_miss():
    near = rps_score(torch.tensor([0.0, 1.0, 0.0, 0.0]), torch.tensor(2))
    far = rps_score(torch.tensor([1.0, 0.0, 0.0, 0.0]), torch.tensor(2))
    assert near > far


def test_rlcd_learns_and_is_calibrated():
    torch.manual_seed(0)
    kw = dict(task="topic", n_classes=4, n_words=10, noise=0.35)
    model = BagPolicy(4)
    opt = torch.optim.Adam(model.parameters(), lr=0.05)
    cfg = RLCDConfig(group_size=8, sigma=0.5)
    for step in range(120):
        rlcd_step(model, opt, lambda: make("classify", **kw), list(range(step * 16, step * 16 + 16)), cfg)
    logits, labels = predict_probs(model, lambda: make("classify", **kw), range(50_000, 50_400))
    probs = torch.softmax(logits, -1)
    acc = (probs.argmax(-1) == labels).float().mean().item()
    assert acc > 0.9, acc
    assert ece(probs, labels) < 0.15  # proper scoring rewards honest confidence


def test_rlcd_ordinal_runs_and_improves():
    torch.manual_seed(0)
    kw = dict(task="ordinal", n_classes=5, n_words=12, noise=0.3)
    model = BagPolicy(5)
    opt = torch.optim.Adam(model.parameters(), lr=0.05)
    first = rlcd_step(model, opt, lambda: make("classify", **kw), list(range(16)), RLCDConfig())["mean_reward"]
    for step in range(1, 100):
        stats = rlcd_step(model, opt, lambda: make("classify", **kw), list(range(step * 16, step * 16 + 16)), RLCDConfig())
    assert stats["mean_reward"] > first + 0.3 and stats["accuracy"] > 0.6


def test_temperature_scaling_fixes_overconfidence():
    torch.manual_seed(0)
    labels = torch.randint(0, 4, (2000,))
    logits = torch.randn(2000, 4)
    logits[torch.arange(2000), labels] += 1.0       # modest signal...
    logits = logits * 5                              # ...made very overconfident
    t = fit_temperature(logits, labels)
    assert t > 2.0
    before = ece(torch.softmax(logits, -1), labels)
    after = ece(torch.softmax(logits / t, -1), labels)
    assert after < before / 3
    sel = selective_accuracy(torch.softmax(logits / t, -1), labels, risk=0.2)
    assert 0 <= sel["coverage"] <= 1
    assert brier(torch.softmax(logits / t, -1), labels) < brier(torch.softmax(logits, -1), labels)
