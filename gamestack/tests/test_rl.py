import json
import subprocess
import sys

import pytest

torch = pytest.importorskip("torch")

from gamestack import make
from gamestack.rl.calibrate import pass_rate, random_policy
from gamestack.rl.grpo import GRPOConfig, advantages, grpo_step
from gamestack.rl.policy import CausalLMPolicy, HFClassifierPolicy, masked_logits
from gamestack.rl.rollout import Episode, Step, collect


class BagPolicy(torch.nn.Module):
    """Toy model: hashed bag of words -> linear logits."""

    def __init__(self, n_actions, buckets=2048):
        super().__init__()
        self.buckets = buckets
        self.emb = torch.nn.EmbeddingBag(buckets, n_actions, mode="sum")

    def forward(self, prompts):
        ids = [[hash_word(w) % self.buckets for w in p.split("Text:")[-1].split()] for p in prompts]
        flat = torch.tensor([i for row in ids for i in row])
        offs = torch.tensor([0] + [len(r) for r in ids[:-1]]).cumsum(0)
        return self.emb(flat, offs)


def hash_word(w):  # stable across runs, unlike hash()
    h = 0
    for c in w:
        h = (h * 131 + ord(c)) % 1_000_003
    return h


def test_advantages_group_normalised():
    eps = [Episode(0, 0, [Step("p", [0, 1], 0, r)]) for r in (1.0, -1.0, 1.0, -1.0)]
    adv = advantages(eps, 4)
    assert abs(sum(adv)) < 1e-5 and adv[0] > 0 > adv[1]


def test_masked_logits_blocks_illegal():
    out = masked_logits(torch.zeros(1, 4), [[1, 3]])
    assert out[0, 0] == float("-inf") and out[0, 1] == 0


def test_grpo_learns_topic_classification():
    torch.manual_seed(0)
    kw = dict(task="topic", n_classes=4, n_words=10, noise=0.3)
    model = BagPolicy(4)
    opt = torch.optim.Adam(model.parameters(), lr=0.05)
    cfg = GRPOConfig(group_size=8, prompts_per_step=16)
    before = pass_rate("classify", kw, model, 200, greedy=True)
    for step in range(60):
        seeds = list(range(step * 16, step * 16 + 16))
        grpo_step(model, opt, lambda: make("classify", **kw), seeds, cfg)
    after = pass_rate("classify", kw, model, 200, greedy=True)
    assert before < 0.6 and after > 0.9, (before, after)


def test_random_policy_pass_rate_near_chance():
    rate = pass_rate("classify", dict(task="topic", n_classes=4), random_policy(4), 400)
    assert 0.15 < rate < 0.35


def test_parity_is_not_solved_by_bag_of_words():
    torch.manual_seed(0)
    kw = dict(task="parity", n_words=8)
    model = BagPolicy(2)
    opt = torch.optim.Adam(model.parameters(), lr=0.05)
    cfg = GRPOConfig()
    for step in range(40):
        grpo_step(model, opt, lambda: make("classify", **kw), list(range(step * 16, step * 16 + 16)), cfg)
    assert pass_rate("classify", kw, model, 300, greedy=True) < 0.7  # order-blind model stays near chance


def test_text_reply_reward_and_retry_limit():
    env = make("classify", max_attempts=2)
    env.reset(seed=0)
    label = env.label_names[env.label]
    _, r, term, *_ = env.step_text(f"I think it is. Answer: {label}")
    assert r == 1.0 and term
    env.reset(seed=1)
    assert env.step_text("???")[2] is False and env.step_text("???")[2] is True


def test_hf_classifier_policy_with_tiny_bert():
    transformers = pytest.importorskip("transformers")
    cfg = transformers.BertConfig(vocab_size=500, hidden_size=32, num_hidden_layers=2, num_attention_heads=2,
                                  intermediate_size=64, num_labels=4)
    bert = transformers.BertForSequenceClassification(cfg)

    class Tok:
        def __call__(self, texts, return_tensors, padding, truncation, max_length):
            rows = [[hash_word(w) % 499 + 1 for w in t.split()][:max_length] for t in texts]
            n = max(map(len, rows))
            ids = torch.tensor([r + [0] * (n - len(r)) for r in rows])
            return {"input_ids": ids, "attention_mask": (ids > 0).long()}

    policy = HFClassifierPolicy(bert, Tok())
    kw = dict(task="topic", n_classes=4)
    eps = collect(lambda: make("classify", **kw), policy, [0, 1, 2], group_size=2)
    assert len(eps) == 6 and all(len(e.steps) == 1 for e in eps)
    opt = torch.optim.Adam(policy.parameters(), lr=1e-3)
    stats = grpo_step(policy, opt, lambda: make("classify", **kw), [0, 1], GRPOConfig(group_size=4))
    assert stats["n_steps"] == 8


def test_causal_lm_policy_scores_are_sequence_logprobs():
    transformers = pytest.importorskip("transformers")
    vocab = {}

    class Tok:
        pad_token, eos_token, pad_token_id = "<pad>", "<pad>", 0

        def _ids(self, text):
            return [vocab.setdefault(w, len(vocab) + 1) for w in text.split()]

        def __call__(self, text, return_tensors=None, add_special_tokens=True):
            return type("E", (), {"input_ids": torch.tensor([self._ids(text)])})()

    tok = Tok()
    for w in "a b c d e f g h".split():
        tok._ids(w)
    gpt = transformers.GPT2LMHeadModel(transformers.GPT2Config(vocab_size=64, n_embd=16, n_layer=1, n_head=2, n_positions=64))
    gpt.eval()
    names = {0: "a", 1: "b c"}
    pol = CausalLMPolicy(gpt, tok, names, 3, template="{prompt} d")
    with torch.no_grad():
        scores = pol(["e f"])[0]
        ids = torch.tensor([tok._ids("e f d") + tok._ids("b c")])
        lp = torch.log_softmax(gpt(ids).logits, -1)[0]
        n = ids.shape[1]
        want = lp[n - 3, ids[0, n - 2]] + lp[n - 2, ids[0, n - 1]]
    assert scores[2] == float("-inf") and torch.allclose(scores[1], want, atol=1e-5)


def test_calibrate_cli():
    out = subprocess.run([sys.executable, "-m", "gamestack.rl.calibrate", "--game", "classify", "--grid",
                          json.dumps([{"noise": 0.0}, {"noise": 0.9}]), "--episodes", "60"],
                         capture_output=True, text=True, check=True).stdout
    assert out.count("pass=") == 2
