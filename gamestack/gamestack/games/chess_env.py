"""Chess against a built-in opponent, rules from python-chess.

Action = ``(from * 64 + to) * 5 + promo`` with promo 0 = none, 1..4 = N, B, R, Q. Reward is
terminal: +1 win, -1 loss, 0 draw. The agent plays White unless ``agent_color="black"``."""

from __future__ import annotations

import chess
import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv

_PROMO = [None, chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN]
_VALUE = {chess.PAWN: 1, chess.KNIGHT: 3, chess.BISHOP: 3, chess.ROOK: 5, chess.QUEEN: 9, chess.KING: 0}


def move_to_action(m: chess.Move) -> int:
    return (m.from_square * 64 + m.to_square) * 5 + (_PROMO.index(m.promotion) if m.promotion else 0)


def action_to_move(a: int) -> chess.Move:
    sq, promo = divmod(a, 5)
    f, t = divmod(sq, 64)
    return chess.Move(f, t, _PROMO[promo])


class ChessEnv(GameEnv):
    name = "chess"

    def __init__(self, opponent: str = "random", agent_color: str = "white", max_plies: int = 300):
        super().__init__()
        assert opponent in ("random", "greedy")
        self.opponent, self.agent_white, self.max_plies = opponent, agent_color == "white", max_plies
        self.action_space = spaces.Discrete(64 * 64 * 5)
        self.observation_space = spaces.Box(-6, 6, (8, 8), np.int8)

    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        self.board = chess.Board()
        if not self.agent_white:
            self._opponent_move()
        return self._observation(), {}

    def _observation(self):
        out = np.zeros((8, 8), dtype=np.int8)  # +piece for White, -piece for Black; rank 8 on row 0
        for sq, p in self.board.piece_map().items():
            out[7 - sq // 8, sq % 8] = p.piece_type * (1 if p.color else -1)
        return out

    def _opponent_move(self):
        moves = list(self.board.legal_moves)
        if self.opponent == "greedy":
            def gain(m):
                cap = self.board.piece_at(m.to_square)
                return (_VALUE[cap.piece_type] if cap else 0) + (8 if m.promotion else 0) + self.np_random.random() * 0.1
            move = max(moves, key=gain)
        else:
            move = moves[int(self.np_random.integers(len(moves)))]
        self.board.push(move)

    def _result(self):
        """(done, reward from the agent's view)."""
        if self.board.is_game_over(claim_draw=True):
            r = self.board.result(claim_draw=True)
            if r == "1/2-1/2":
                return True, 0.0
            white_won = r == "1-0"
            return True, 1.0 if white_won == self.agent_white else -1.0
        return False, 0.0

    def legal_actions(self):
        return [move_to_action(m) for m in self.board.legal_moves] if self.board.turn == self.agent_white else []

    def step(self, action):
        move = action_to_move(int(action))
        if move not in self.board.legal_moves:
            return self._illegal()
        self.board.push(move)
        done, reward = self._result()
        if not done:
            self._opponent_move()
            done, reward = self._result()
        truncated = not done and self.board.ply() >= self.max_plies
        return self._observation(), reward, done, truncated, {"fen": self.board.fen()}

    def action_name(self, action):
        return action_to_move(action).uci()

    def parse_action(self, text):
        import re

        legal = {m.uci(): m for m in self.board.legal_moves}
        for tok in reversed(re.findall(r"[a-h][1-8][a-h][1-8][qrbn]?", text.lower())):
            if tok in legal:
                return move_to_action(legal[tok])
        for tok in reversed(re.findall(r"[A-Za-z][A-Za-z0-9+#=\-]*", text)):  # SAN fallback
            try:
                return move_to_action(self.board.parse_san(tok))
            except ValueError:
                continue
        return None

    def text_observation(self):
        side = "White" if self.board.turn else "Black"
        return f"{self.board.unicode(borders=False)}\nFEN: {self.board.fen()}\n{side} to move."

    def action_prompt(self):
        return "Chess. You choose the moves for your side. Reply with one legal move in UCI (e.g. e2e4, e7e8q) or SAN (e.g. Nf3)."
