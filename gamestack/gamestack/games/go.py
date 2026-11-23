"""Go on a small board with Tromp-Taylor area scoring, positional superko and komi.

Action ``i`` in ``[0, size²)`` plays that point, ``size²`` is pass. The agent is Black and moves
first; the built-in opponent plays random legal moves that do not fill its own eye. Reward is
terminal: +1 win, -1 loss (komi means no draws)."""

from __future__ import annotations

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv

EMPTY, BLACK, WHITE = 0, 1, 2


class GoBoard:
    def __init__(self, size: int, komi: float):
        self.n, self.komi = size, komi
        self.b = np.zeros((size, size), dtype=np.int8)
        self.history = {self.b.tobytes()}
        self.passes = 0
        self.turn = BLACK
        self.moves = 0

    def neighbours(self, r, c):
        for dr, dc in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            rr, cc = r + dr, c + dc
            if 0 <= rr < self.n and 0 <= cc < self.n:
                yield rr, cc

    def group(self, b, r, c):
        colour, stack, seen, libs = b[r, c], [(r, c)], {(r, c)}, set()
        while stack:
            x, y = stack.pop()
            for nr, nc in self.neighbours(x, y):
                if b[nr, nc] == EMPTY:
                    libs.add((nr, nc))
                elif b[nr, nc] == colour and (nr, nc) not in seen:
                    seen.add((nr, nc))
                    stack.append((nr, nc))
        return seen, libs

    def try_play(self, r, c, colour):
        """Board after playing, or None if illegal (occupied, suicide, superko)."""
        if self.b[r, c] != EMPTY:
            return None
        b = self.b.copy()
        b[r, c] = colour
        other = 3 - colour
        for nr, nc in self.neighbours(r, c):
            if b[nr, nc] == other:
                stones, libs = self.group(b, nr, nc)
                if not libs:
                    for sr, sc in stones:
                        b[sr, sc] = EMPTY
        _, libs = self.group(b, r, c)
        if not libs or b.tobytes() in self.history:
            return None
        return b

    def play(self, action: int) -> bool:
        if action == self.n * self.n:
            self.passes += 1
        else:
            nb = self.try_play(*divmod(action, self.n), self.turn)
            if nb is None:
                return False
            self.b, self.passes = nb, 0
            self.history.add(nb.tobytes())
        self.turn = 3 - self.turn
        self.moves += 1
        return True

    def legal(self, colour=None):
        colour = colour or self.turn
        out = [i for i in range(self.n * self.n) if self.try_play(*divmod(i, self.n), colour) is not None]
        return out + [self.n * self.n]

    def is_own_eye(self, i, colour):
        r, c = divmod(i, self.n)
        return all(self.b[nr, nc] == colour for nr, nc in self.neighbours(r, c))

    def score(self):
        """(black, white) area score; empty regions count for the single colour bordering them."""
        black = int((self.b == BLACK).sum())
        white = int((self.b == WHITE).sum())
        seen = set()
        for r in range(self.n):
            for c in range(self.n):
                if self.b[r, c] != EMPTY or (r, c) in seen:
                    continue
                stack, region, border = [(r, c)], {(r, c)}, set()
                while stack:
                    x, y = stack.pop()
                    for nr, nc in self.neighbours(x, y):
                        v = self.b[nr, nc]
                        if v == EMPTY and (nr, nc) not in region:
                            region.add((nr, nc))
                            stack.append((nr, nc))
                        elif v != EMPTY:
                            border.add(int(v))
                seen |= region
                if border == {BLACK}:
                    black += len(region)
                elif border == {WHITE}:
                    white += len(region)
        return black, white + self.komi


class GoEnv(GameEnv):
    name = "go"

    def __init__(self, size: int = 5, komi: float = 0.5, max_moves: int | None = None):
        super().__init__()
        self.size, self.komi = size, komi
        self.max_moves = max_moves or 4 * size * size
        self.action_space = spaces.Discrete(size * size + 1)
        self.observation_space = spaces.Box(0, 2, (size, size), np.int8)

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self.game = GoBoard(self.size, self.komi)
        return self._observation(), {}

    def _observation(self):
        return self.game.b.copy()

    def legal_actions(self):
        return self.game.legal(BLACK)

    def _finished(self):
        return self.game.passes >= 2 or self.game.moves >= self.max_moves

    def _reward(self):
        b, w = self.game.score()
        return 1.0 if b > w else -1.0

    def step(self, action):
        action = int(action)
        if not 0 <= action <= self.size**2 or not self.game.play(action):
            return self._illegal()
        if not self._finished():
            # Random move that does not fill its own eye; pass only when nothing else is left
            # or the agent just passed while White is ahead.
            cands = [i for i in self.game.legal(WHITE) if i != self.size**2 and not self.game.is_own_eye(i, WHITE)]
            b, w = self.game.score()
            if not cands or (self.game.passes >= 1 and w > b):
                pick = self.size**2
            else:
                pick = cands[int(self.np_random.integers(len(cands)))]
            self.game.play(pick)
        done = self._finished()
        reward = self._reward() if done else 0.0
        return self._observation(), reward, done, False, {"score": self.game.score()}

    def action_name(self, action):
        if action == self.size**2:
            return "pass"
        r, c = divmod(action, self.size)
        return f"{chr(ord('a') + c)}{r + 1}"

    def text_observation(self):
        sym = {EMPTY: ".", BLACK: "X", WHITE: "O"}
        header = "  " + " ".join(chr(ord("a") + c) for c in range(self.size))
        rows = [f"{r + 1} " + " ".join(sym[int(v)] for v in self.game.b[r]) for r in range(self.size)]
        b, w = self.game.score()
        return "\n".join([header, *rows]) + f"\nYou are X (Black). Area score now: X {b}, O {w} (komi included)."

    def action_prompt(self):
        return (
            f"Go {self.size}x{self.size}, you are X (Black) and move first. Reply with a point such as 'c3' "
            "(column letter, row number) or 'pass'. Area scoring; two consecutive passes end the game."
        )
