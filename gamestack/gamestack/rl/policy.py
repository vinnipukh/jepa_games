"""The policy interface: text (or any prompt) in, logits over the env's discrete actions out.

A ``LogitPolicy`` is any callable ``(prompts: list[str]) -> Tensor[B, n_actions]``. That covers

* a BERT-style encoder + head (``HFClassifierPolicy``): logits are the classifier outputs,
* a causal LLM (``CausalLMPolicy``): logits are the log-probability of each action's text given the
  prompt, so the model's own token logits drive the decision,
* anything you write (a JEPA latent policy can ignore the text and use observations instead).

Illegal actions are removed with ``masked_logits`` before sampling or ranking.
"""

from __future__ import annotations

from typing import Callable, Sequence

import torch
from torch import Tensor

LogitPolicy = Callable[[Sequence[str]], Tensor]


def masked_logits(logits: Tensor, legal: Sequence[Sequence[int]]) -> Tensor:
    """Set the logits of non-legal actions to -inf. ``legal[i]`` lists the legal ids of row i."""
    mask = torch.zeros_like(logits, dtype=torch.bool)
    for i, ids in enumerate(legal):
        mask[i, list(ids)] = True
    return logits.masked_fill(~mask, float("-inf"))


def sample(logits: Tensor, temperature: float = 1.0, greedy: bool = False) -> Tensor:
    if greedy:
        return logits.argmax(-1)
    return torch.distributions.Categorical(logits=logits / temperature).sample()


class HFClassifierPolicy:
    """Wrap a Hugging Face ``AutoModelForSequenceClassification`` (BERT + head etc.).

    ``num_labels`` must equal the env's number of actions. The model stays trainable; call it inside
    ``torch.no_grad()`` for rollouts (``collect`` does) and with grad in the loss."""

    def __init__(self, model, tokenizer, device: str | torch.device = "cpu", max_length: int = 256):
        self.model, self.tok, self.device, self.max_length = model.to(device), tokenizer, device, max_length

    def __call__(self, prompts: Sequence[str]) -> Tensor:
        enc = self.tok(list(prompts), return_tensors="pt", padding=True, truncation=True, max_length=self.max_length)
        return self.model(**{k: v.to(self.device) for k, v in enc.items()}).logits

    def parameters(self):
        return self.model.parameters()


class CausalLMPolicy:
    """Score each action's text with a causal LM: ``logit[a] = log p(action_text | prompt)``.

    ``action_names`` is the env's id -> text map (``env.action_names()``) and ``n_actions`` the size of
    its action space; ids without a name get -inf. Exact for multi-token names (sums token
    log-probs); cost is one forward pass per (prompt, legal action)."""

    def __init__(self, model, tokenizer, action_names: dict[int, str], n_actions: int,
                 device: str | torch.device = "cpu", template: str = "{prompt}\nAnswer:"):
        self.model, self.tok, self.device = model.to(device), tokenizer, device
        self.names, self.n_actions, self.template = action_names, n_actions, template
        if self.tok.pad_token is None:
            self.tok.pad_token = self.tok.eos_token

    def __call__(self, prompts: Sequence[str]) -> Tensor:
        out = torch.full((len(prompts), self.n_actions), float("-inf"), device=self.device)
        for i, p in enumerate(prompts):
            ctx = self.template.format(prompt=p)
            ctx_ids = self.tok(ctx, return_tensors="pt").input_ids[0]
            seqs, spans = [], []
            for a, name in self.names.items():
                ans = self.tok(" " + name, add_special_tokens=False, return_tensors="pt").input_ids[0]
                seqs.append(torch.cat([ctx_ids, ans]))
                spans.append((a, len(ctx_ids), len(ans)))
            batch = torch.nn.utils.rnn.pad_sequence(seqs, batch_first=True, padding_value=self.tok.pad_token_id).to(self.device)
            attn = torch.nn.utils.rnn.pad_sequence([torch.ones_like(s) for s in seqs], batch_first=True).to(self.device)
            logp = torch.log_softmax(self.model(input_ids=batch, attention_mask=attn).logits.float(), -1)
            for row, (a, start, n) in enumerate(spans):
                tgt = batch[row, start:start + n]
                out[i, a] = logp[row, start - 1:start - 1 + n].gather(-1, tgt[:, None]).sum()
        return out

    def parameters(self):
        return self.model.parameters()
