"""Classification as a one-step decision environment.

The observation is a text, the action is a label, the reward is verifiable: +1 correct, ``-1``
(``wrong_reward``) wrong. An unparseable LLM reply costs ``illegal_reward`` and the agent may retry
up to ``max_attempts`` times (then the episode ends). This is the shape RL-for-classifier work
uses (rule-based accuracy reward plus a format requirement), so a BERT-style classifier (logits
over labels), an LLM (logits over label tokens) and a text-only RL trainer all see the same task.

Tasks (procedural, deterministic per seed, with difficulty knobs):

* ``topic``: each class owns a keyword pool; a sentence mixes ``n_words`` words, a fraction
  ``noise`` of which come from other classes. Harder = more noise, more classes, fewer words.
* ``majority``: a bag of words from ``n_classes`` symbols; the label is the most frequent one.
* ``parity``: a binary string; the label is its parity (hard for bag-of-words models; a useful
  probe of whether a model really reads the input).

Real data: ``ClassificationEnv.from_arrays(texts, labels, label_names)`` or ``from_hf``.
"""

from __future__ import annotations

import re
import string
from typing import Sequence

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv

_SYLL = ["ka", "to", "mi", "ra", "ne", "lu", "so", "vi", "pe", "da", "zu", "fo", "gi", "ba", "xe", "ho"]


def _word(rng: np.random.Generator) -> str:
    return "".join(_SYLL[int(i)] for i in rng.integers(0, len(_SYLL), size=2)) + "n"


class ClassificationEnv(GameEnv):
    name = "classify"

    def __init__(
        self,
        task: str = "topic",
        n_classes: int = 4,
        n_words: int = 12,
        noise: float = 0.3,
        pool_size: int = 12,
        wrong_reward: float = -1.0,
        max_attempts: int = 3,
        data: tuple[Sequence[str], Sequence[int], Sequence[str]] | None = None,
        max_chars: int = 4096,
    ):
        super().__init__()
        assert task in ("topic", "majority", "parity", "data")
        self.task = "data" if data is not None else task
        self.n_words, self.noise, self.pool_size = n_words, noise, pool_size
        self.wrong_reward, self.max_attempts = wrong_reward, max_attempts
        if data is not None:
            self.texts, self.labels, self.label_names = list(data[0]), list(data[1]), list(data[2])
            self.n_classes = len(self.label_names)
        else:
            self.n_classes = 2 if task == "parity" else n_classes
            self.label_names = [f"class{i}" for i in range(self.n_classes)]
        self.action_space = spaces.Discrete(self.n_classes)
        self.observation_space = spaces.Text(max_length=max_chars, charset=string.printable)
        self.text = ""
        self.label = 0
        self._attempts = 0

    # ---- constructors for real data ---------------------------------------------------------
    @classmethod
    def from_arrays(cls, texts, labels, label_names, **kw):
        return cls(data=(texts, labels, label_names), **kw)

    @classmethod
    def from_hf(cls, name: str, split: str = "train", text_col: str = "text", label_col: str = "label",
                limit: int | None = None, config: str | None = None, **kw):
        """Load a Hugging Face classification dataset (needs ``datasets`` and network access)."""
        from datasets import load_dataset

        ds = load_dataset(name, config, split=split)
        if limit:
            ds = ds.select(range(min(limit, len(ds))))
        names = ds.features[label_col].names
        return cls.from_arrays(list(ds[text_col]), list(ds[label_col]), names, **kw)

    # ---- episode ------------------------------------------------------------------------------
    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self._attempts = 0
        rng = self.np_random
        if self.task == "data":
            i = int(rng.integers(len(self.texts)))
            self.text, self.label = self.texts[i], int(self.labels[i])
        elif self.task == "topic":
            # Keyword pools depend only on the pool seed, not on the episode seed, so the task is
            # learnable across episodes. (Derived from fixed numbers, not from ``seed``.)
            pools = self._pools()
            self.label = int(rng.integers(self.n_classes))
            words = []
            for _ in range(self.n_words):
                if rng.random() < self.noise:
                    c = int(rng.integers(self.n_classes))
                else:
                    c = self.label
                words.append(pools[c][int(rng.integers(self.pool_size))])
            self.text = " ".join(words)
        elif self.task == "majority":
            symbols = [f"s{i}" for i in range(self.n_classes)]
            counts = rng.multinomial(self.n_words, np.ones(self.n_classes) / self.n_classes)
            while (counts == counts.max()).sum() > 1:  # unique winner
                counts = rng.multinomial(self.n_words, np.ones(self.n_classes) / self.n_classes)
            self.label = int(counts.argmax())
            seq = [symbols[i] for i, c in enumerate(counts) for _ in range(c)]
            self.text = " ".join(seq[j] for j in rng.permutation(len(seq)))
        else:  # parity
            bits = rng.integers(0, 2, size=self.n_words)
            self.label = int(bits.sum() % 2)
            self.text = " ".join(map(str, bits))
        return self.text, {"label": self.label}

    def _pools(self):
        if not hasattr(self, "_pools_cache"):
            rng = np.random.default_rng(12345)
            seen: set[str] = set()
            pools = []
            for _ in range(self.n_classes):
                pool = []
                while len(pool) < self.pool_size:
                    w = _word(rng)
                    if w not in seen:
                        seen.add(w)
                        pool.append(w)
                pools.append(pool)
            self._pools_cache = pools
        return self._pools_cache

    def _observation(self):
        return self.text

    def step(self, action):
        a = int(action)
        if not 0 <= a < self.n_classes:
            return self._fail()
        correct = a == self.label
        return self.text, 1.0 if correct else self.wrong_reward, True, False, {"correct": correct, "label": self.label}

    def _fail(self):
        self._attempts += 1
        done = self._attempts >= self.max_attempts
        return self.text, self.illegal_reward, done, False, {"illegal": True, "illegal_count": self._attempts}

    def step_text(self, text: str):
        a = self.parse_action(text)
        return self._fail() if a is None else self.step(a)

    # ---- text interface -----------------------------------------------------------------------
    def legal_actions(self):
        return list(range(self.n_classes))

    def action_name(self, action):
        return self.label_names[action]

    def parse_action(self, text):
        # Prefer "answer: <label>" (what the prompt asks for), else the last label mentioned.
        m = list(re.finditer(r"answer\s*[:=]\s*", text.lower()))
        if m:
            text = text[m[-1].end():]
            first = None
            for a, n in self.action_names().items():
                hit = re.search(r"(?<![\w-])" + re.escape(n.lower()) + r"(?![\w-])", text.lower())
                if hit and (first is None or hit.start() < first[0]):
                    first = (hit.start(), a)
            return first[1] if first else None
        return super().parse_action(text)

    def text_observation(self):
        return f"Text: {self.text}"

    def action_prompt(self):
        return ("Classify the text. Choose exactly one label from: " + ", ".join(self.label_names)
                + ". Reply in the form 'Answer: <label>'.")
