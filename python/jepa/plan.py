"""Planning in latent space (PLAN 7.3), executed MPC style: replan from the real state every step.

All planners are batched over environments: :meth:`LatentPlanner.act` takes the current cells of
``B`` real environments and returns one action each. The model sees the real state only through
the encoder; everything below the root is a latent rollout of the predictor.

- **Root**: legal actions come from the real rules (``batch_action_mask``).
- **Below the root** (``PlanConfig.legality``): the learned legality head (default), the core
  rules applied to the probe-decoded predicted state (``probe``), or no filter (``none``).
- **Score** (``PlanConfig.score``): ``value`` ranks a node by ``f = depth + h(ẑ)`` with ``h`` the
  learned distance-to-go (0 once the solved head fires), an A*-style cost; ``solved`` ranks by
  the solved head's probability (the plan's primary goal), ties broken by depth.
- **Revisits** (``PlanConfig.revisits``): root actions leading back to a state visited in the real
  episode are ranked last. ``real`` finds them with the real rules at the root (the same
  information as the root's legal mask), ``probe`` from the probe-decoded predicted child (model
  only), ``off`` not at all. Without it, MPC with an imperfect model cycles.

Methods: ``beam`` (width × depth beam search), ``mcts`` (UCT with value = solved probability or
negative distance, children expanded together), ``cem`` (cross-entropy method over action
sequences).
"""

from __future__ import annotations

import math

import numpy as np
import torch

import jepa_water_sort as w
from jepa.config import PlanConfig
from jepa.data import one_hot
from jepa.models import WorldModel

#: Cost added to a root action whose predicted next state was visited before.
REVISIT_PENALTY = 1000.0


class LatentPlanner:
    """A batched MPC policy. Call :meth:`reset` at the start of every batch of episodes."""

    def __init__(self, model: WorldModel, cfg: PlanConfig, device: torch.device | str = "cpu",
                 value_cap: float = 40.0):
        self.model = model.eval()
        self.cfg = cfg
        self.device = torch.device(device)
        self.value_cap = value_cap
        self.name = f"jepa-{cfg.method}"
        self.visited: list[set[bytes]] = []

    def reset(self, n: int, rng: np.random.Generator) -> None:
        self.visited = [set() for _ in range(n)]
        self.rng = rng

    # -- helpers ---------------------------------------------------------------------------------

    def _encode(self, cells: np.ndarray) -> torch.Tensor:
        x = one_hot(torch.as_tensor(cells, device=self.device), self.model.n_colors)
        return self.model.encode(x)

    def _decode(self, h: torch.Tensor) -> np.ndarray:
        """Probe-decoded cells (uint8, 255 = empty)."""
        idx = self.model.probe(h).argmax(-1)
        idx = torch.where(idx == self.model.n_colors, torch.full_like(idx, w.EMPTY), idx)
        return idx.to(torch.uint8).cpu().numpy()

    def _legal(self, h: torch.Tensor) -> torch.Tensor:
        """Legality below the root ``(N, T*T)`` bool."""
        n = h.shape[1]
        off = ~torch.eye(n, dtype=torch.bool, device=h.device).flatten()
        if self.cfg.legality == "none":
            return off.expand(h.shape[0], -1).clone()
        head = self.model.legal_logits(h) > 0
        if self.cfg.legality == "head":
            return head
        if self.cfg.legality == "probe":
            cells = self._decode(h)
            try:
                return torch.as_tensor(w.batch_action_mask(cells), device=h.device)
            except ValueError:
                rows = []
                for i, c in enumerate(cells):
                    try:
                        rows.append(torch.as_tensor(w.action_mask(w.State.from_numpy(c))))
                    except ValueError:  # not a valid state: fall back to the head
                        rows.append(head[i].cpu())
                return torch.stack(rows).to(h.device)
        raise ValueError(f"unknown legality {self.cfg.legality!r}")

    def _cost(self, h: torch.Tensor, depth: torch.Tensor | float) -> torch.Tensor:
        """Lower is better. ``value``: depth + distance (distance 0 when solved); ``solved``:
        −P(solved) + 1e-3 · depth."""
        p = torch.sigmoid(self.model.solved_logit(h).float())
        if self.cfg.score == "value":
            dist = self.model.distance(h).float().clamp(max=self.value_cap)
            dist = torch.where(p > self.cfg.solved_threshold, torch.zeros_like(dist), dist)
            return depth + dist
        if self.cfg.score == "solved":
            return -p + 1e-3 * depth
        raise ValueError(f"unknown score {self.cfg.score!r}")

    def _is_goal(self, h: torch.Tensor) -> torch.Tensor:
        return torch.sigmoid(self.model.solved_logit(h).float()) > self.cfg.solved_threshold

    def _revisit_penalty(self, env: np.ndarray, h_child: torch.Tensor,
                         actions: torch.Tensor) -> torch.Tensor:
        """``env``: environment per root child, ``actions``: the root action leading to it."""
        pen = torch.zeros(len(env), device=h_child.device)
        mode = self.cfg.revisits
        if mode == "off" or len(env) == 0:
            return pen
        if mode == "real":
            cells, _ = w.batch_step(self._root_cells[self._row_of[env]], actions.cpu().numpy())
        elif mode == "probe":
            cells = self._decode(h_child)
        else:
            raise ValueError(f"unknown revisits mode {mode!r}")
        hits = [cells[i].tobytes() in self.visited[e] for i, e in enumerate(env)]
        return pen + torch.as_tensor(hits, device=h_child.device, dtype=torch.float32) * REVISIT_PENALTY

    # -- entry point -----------------------------------------------------------------------------

    @torch.no_grad()
    def act(self, cells: np.ndarray, envs: np.ndarray) -> np.ndarray:
        """``cells``: ``(B, n_tubes, capacity)`` current real states of environments ``envs``.
        Returns ``B`` actions (``-1`` when a state has no legal move)."""
        for c, e in zip(cells, envs):
            self.visited[e].add(c.tobytes())
        self._root_cells = cells
        self._row_of = np.zeros(len(self.visited), dtype=np.int64)
        self._row_of[np.asarray(envs)] = np.arange(len(envs))
        root_mask = torch.as_tensor(w.batch_action_mask(cells), device=self.device)
        method = {"beam": self._beam, "mcts": self._mcts, "cem": self._cem}.get(self.cfg.method)
        if method is None:
            raise ValueError(f"unknown planning method {self.cfg.method!r}")
        actions = method(self._encode(cells), root_mask, np.asarray(envs))
        actions[~root_mask.any(1).cpu().numpy()] = -1
        return actions

    # -- beam search -----------------------------------------------------------------------------

    def _beam(self, h0: torch.Tensor, root_mask: torch.Tensor, envs: np.ndarray) -> np.ndarray:
        B, T, d = h0.shape
        A = T * T
        W = self.cfg.width
        dev = h0.device
        frontier = h0.unsqueeze(1)  # (B, 1, T, d)
        alive = torch.ones(B, 1, dtype=torch.bool, device=dev)
        first = torch.full((B, 1), -1, dtype=torch.long, device=dev)
        path_cost = torch.full((B, 1), -math.inf, device=dev)
        best_cost = torch.full((B,), math.inf, device=dev)
        best_depth = torch.zeros(B, dtype=torch.long, device=dev)
        best_action = torch.full((B,), -1, dtype=torch.long, device=dev)
        done = torch.zeros(B, dtype=torch.bool, device=dev)
        for depth in range(1, self.cfg.depth + 1):
            nb = frontier.shape[1]
            if depth == 1:
                legal = root_mask.unsqueeze(1)
            else:
                live = (alive & ~done.view(B, 1)).flatten()
                legal = torch.zeros(B * nb, A, dtype=torch.bool, device=dev)
                if live.any():
                    legal[live] = self._legal(frontier.reshape(B * nb, T, d)[live])
                legal = legal.view(B, nb, A)
            legal = legal & alive.unsqueeze(-1) & ~done.view(B, 1, 1)
            b_idx, w_idx, a_idx = legal.nonzero(as_tuple=True)
            if len(b_idx) == 0:
                break
            child = self.model.predict(frontier[b_idx, w_idx], a_idx)
            cost = self._cost(child, float(depth))
            if self.cfg.score == "value" and self.cfg.consistent:
                # A path costs at least as much as any of its prefixes: a deep latent whose
                # distance head is optimistic cannot undercut its own parent.
                # A node the solved head marks as solved keeps its exact cost (its depth).
                cost = torch.where(self._is_goal(child), cost,
                                   torch.maximum(cost, path_cost[b_idx, w_idx]))
            goal = self._is_goal(child)
            fa = a_idx if depth == 1 else first[b_idx, w_idx]
            if depth == 1:
                cost = cost + self._revisit_penalty(envs[b_idx.cpu().numpy()], child, a_idx)
            # Best node per environment so far (ties go to the deeper node).
            order = torch.argsort(cost)
            ob = b_idx[order]
            first_pos = torch.full((B,), len(order), dtype=torch.long, device=dev)
            first_pos.scatter_reduce_(0, ob, torch.arange(len(order), device=dev), "amin")
            has = first_pos < len(order)
            pick = order[first_pos[has]]
            cand = cost[pick]
            better = cand <= best_cost[has]
            sel = has.nonzero(as_tuple=True)[0][better]
            best_cost[sel] = cand[better]
            best_action[sel] = fa[pick[better]]
            best_depth[sel] = depth
            # Stop expanding environments that reached a goal.
            reached = torch.zeros(B, dtype=torch.bool, device=dev)
            reached[b_idx[goal]] = True
            # Next frontier: the W cheapest children per environment.
            keep_order = torch.argsort(cost, stable=True)
            kb = b_idx[keep_order]
            env_order = torch.argsort(kb, stable=True)
            order2 = keep_order[env_order]
            ob2 = b_idx[order2]
            counts = torch.bincount(ob2, minlength=B)
            starts = torch.cumsum(counts, 0) - counts
            rank = torch.arange(len(order2), device=dev) - starts[ob2]
            k = rank < W
            sel_rows = order2[k]
            nw = min(W, int(counts.max()))
            new_frontier = torch.zeros(B, nw, T, d, dtype=child.dtype, device=dev)
            new_alive = torch.zeros(B, nw, dtype=torch.bool, device=dev)
            new_first = torch.full((B, nw), -1, dtype=torch.long, device=dev)
            new_cost = torch.full((B, nw), -math.inf, device=dev)
            rb, rr = b_idx[sel_rows], rank[k]
            new_frontier[rb, rr] = child[sel_rows]
            new_alive[rb, rr] = ~goal[sel_rows]
            new_first[rb, rr] = fa[sel_rows]
            new_cost[rb, rr] = cost[sel_rows]
            frontier, alive, first, path_cost = new_frontier, new_alive, new_first, new_cost
            done = done | reached
        out = best_action.cpu().numpy()
        # Environments without any candidate (no legal move) keep -1.
        return out

    # -- MCTS ------------------------------------------------------------------------------------

    def _node_value(self, h: torch.Tensor) -> torch.Tensor:
        """Value of a state in [−1, 1]-ish units: −distance / cap (``value``) or P(solved)."""
        p = torch.sigmoid(self.model.solved_logit(h).float())
        if self.cfg.score == "value":
            dist = self.model.distance(h).float().clamp(max=self.value_cap)
            dist = torch.where(p > self.cfg.solved_threshold, torch.zeros_like(dist), dist)
            return -dist / self.value_cap
        return p

    def _mcts(self, h0: torch.Tensor, root_mask: torch.Tensor, envs: np.ndarray) -> np.ndarray:
        """UCT run in lockstep over environments; every expansion predicts all children of the
        selected leaves in one batch."""
        B, T, d = h0.shape
        step_cost = 1.0 / self.value_cap if self.cfg.score == "value" else 0.0
        gamma = 1.0 if self.cfg.score == "value" else 0.97
        # Per tree: lists of nodes; node = dict(h, children{a: idx}, N, W, Q per child...).
        trees = []
        for b in range(B):
            trees.append({"h": [h0[b]], "kids": [None], "N": [0], "Wsum": [0.0], "terminal": [False],
                          "value": [0.0]})
        pending_root = [(b, 0) for b in range(B)]
        self._mcts_expand(trees, pending_root, root_mask, envs, step_cost, gamma, root=True)
        for _ in range(self.cfg.mcts_simulations):
            leaves, paths = [], []
            for b, t in enumerate(trees):
                node, path = 0, [0]
                while t["kids"][node] and not t["terminal"][node]:
                    parent_n = max(1, t["N"][node])
                    best, best_u = None, -math.inf
                    for a, c in t["kids"][node].items():
                        q = t["Wsum"][c] / t["N"][c] if t["N"][c] else t["value"][c]
                        prior = 1.0 / len(t["kids"][node])
                        u = -step_cost + gamma * q + self.cfg.mcts_c * prior * math.sqrt(parent_n) / (1 + t["N"][c])
                        if u > best_u:
                            best, best_u = c, u
                    node = best
                    path.append(node)
                paths.append(path)
                if t["kids"][node] is None and not t["terminal"][node]:
                    leaves.append((b, node))
            if leaves:
                self._mcts_expand(trees, leaves, None, envs, step_cost, gamma, root=False)
            for b, path in enumerate(paths):
                t = trees[b]
                leaf = path[-1]
                kids = t["kids"][leaf]
                if kids:
                    v = max(-step_cost + gamma * t["value"][c] for c in kids.values())
                else:
                    v = t["value"][leaf]
                for node in reversed(path):
                    t["N"][node] += 1
                    t["Wsum"][node] += v
                    v = -step_cost + gamma * v
        out = np.full(B, -1, dtype=np.int64)
        for b, t in enumerate(trees):
            kids = t["kids"][0]
            if kids:
                out[b] = max(kids, key=lambda a: (t["N"][kids[a]], t["value"][kids[a]]))
        return out

    def _mcts_expand(self, trees, leaves, root_mask, envs, step_cost, gamma, root):
        hs = torch.stack([trees[b]["h"][n] for b, n in leaves])
        legal = root_mask if root else self._legal(hs)
        b_rows, a_idx = legal.nonzero(as_tuple=True)
        if len(b_rows) == 0:
            for b, n in leaves:
                trees[b]["kids"][n] = {}
            return
        child = self.model.predict(hs[b_rows], a_idx)
        value = self._node_value(child)
        goal = self._is_goal(child)
        if root:
            env_of = np.array([envs[leaves[i][0]] for i in b_rows.cpu().numpy()])
            value = value - self._revisit_penalty(env_of, child, a_idx) / REVISIT_PENALTY
        for (b, n) in leaves:
            trees[b]["kids"][n] = {}
        rows = b_rows.cpu().numpy()
        acts = a_idx.cpu().numpy()
        vals = value.cpu().numpy()
        goals = goal.cpu().numpy()
        for j, (r, a) in enumerate(zip(rows, acts)):
            b, n = leaves[r]
            t = trees[b]
            t["h"].append(child[j])
            t["kids"].append(None)
            t["N"].append(0)
            t["Wsum"].append(0.0)
            t["terminal"].append(bool(goals[j]))
            t["value"].append(float(vals[j]))
            t["kids"][n][int(a)] = len(t["h"]) - 1

    # -- CEM -------------------------------------------------------------------------------------

    def _cem(self, h0: torch.Tensor, root_mask: torch.Tensor, envs: np.ndarray) -> np.ndarray:
        B, T, d = h0.shape
        A = T * T
        H, S, E = self.cfg.depth, self.cfg.cem_samples, self.cfg.cem_elites
        dev = h0.device
        logits = torch.zeros(B, H, A, device=dev)
        best_cost = torch.full((B,), math.inf, device=dev)
        best_action = torch.full((B,), -1, dtype=torch.long, device=dev)
        for _ in range(self.cfg.cem_iters):
            h = h0.repeat_interleave(S, 0)
            seq = torch.zeros(B * S, H, dtype=torch.long, device=dev)
            cost = torch.full((B * S,), math.inf, device=dev)
            finished = torch.zeros(B * S, dtype=torch.bool, device=dev)
            for t in range(H):
                legal = root_mask.repeat_interleave(S, 0) if t == 0 else self._legal(h)
                lg = logits[:, t].repeat_interleave(S, 0).masked_fill(~legal, -math.inf)
                none = ~legal.any(1)
                lg[none] = 0.0
                a = torch.multinomial(torch.softmax(lg, 1), 1, generator=None).squeeze(1)
                seq[:, t] = a
                h = self.model.predict(h, a)
                c = self._cost(h, float(t + 1))
                if t == 0:
                    c = c + self._revisit_penalty(np.repeat(envs, S), h, a)
                c = torch.where(finished | none, torch.full_like(c, math.inf), c)
                cost = torch.minimum(cost, c)
                finished = finished | self._is_goal(h) | none
            cost = cost.view(B, S)
            elite = torch.topk(-cost, E, dim=1).indices  # (B, E)
            seqs = seq.view(B, S, H)
            elite_seq = torch.gather(seqs, 1, elite.unsqueeze(-1).expand(-1, -1, H))
            counts = torch.zeros(B, H, A, device=dev)
            counts.scatter_add_(2, elite_seq.transpose(1, 2), torch.ones(B, H, E, device=dev))
            logits = torch.log(counts / E + 1e-3)
            top = cost.min(1)
            better = top.values < best_cost
            best_cost = torch.where(better, top.values, best_cost)
            best_first = seqs[torch.arange(B, device=dev), top.indices, 0]
            best_action = torch.where(better, best_first, best_action)
        return best_action.cpu().numpy()
