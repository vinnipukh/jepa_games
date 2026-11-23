"""Uno (two players: the agent vs a built-in bot). Standard 108-card deck, imperfect information:
the agent sees its own hand, the top card, the current colour and the bot's hand size.

Action ids (62):
  0..51    play colour c = id // 13, face f = id % 13 (0-9, 10 skip, 11 reverse, 12 draw-two)
  52..55   play Wild, choosing colour id - 52
  56..59   play Wild Draw Four, choosing colour id - 56
  60       draw a card
  61       pass (only after drawing a card this turn)
With two players Reverse acts as Skip. Draw-two / Wild Draw Four make the opponent draw and lose the
turn. Playing a card you could not play (not in hand / not matching) is illegal. Reward: +1 win,
-1 loss, ``0.01`` shaping is not used. Drawn cards may be played right away or kept (then pass)."""

from __future__ import annotations

import numpy as np
from gymnasium import spaces

from gamestack.core import GameEnv

COLORS = ["red", "yellow", "green", "blue"]
FACES = [str(i) for i in range(10)] + ["skip", "reverse", "draw2"]
DRAW, PASS = 60, 61
N_TYPES = 52 + 4 + 4  # card types a hand can contain: 52 coloured, wild, wild4 (ids 0..51, 52, 53)


def card_name(card: int) -> str:
    """Card ids: 0..51 coloured, 52 = wild, 53 = wild-draw-4."""
    if card == 52:
        return "wild"
    if card == 53:
        return "wild4"
    return f"{COLORS[card // 13]} {FACES[card % 13]}"


def make_deck() -> list[int]:
    deck = []
    for c in range(4):
        deck.append(c * 13)  # one 0
        for f in range(1, 13):
            deck += [c * 13 + f] * 2
    deck += [52] * 4 + [53] * 4
    return deck  # 108


class UnoEnv(GameEnv):
    name = "uno"

    def __init__(self, hand_size: int = 7, max_turns: int = 400):
        super().__init__()
        self.hand_size, self.max_turns = hand_size, max_turns
        self.action_space = spaces.Discrete(62)
        self.observation_space = spaces.Dict(
            {
                "hand": spaces.Box(0, 20, (54,), np.int8),
                "top": spaces.Box(0, 53, (1,), np.int8),
                "color": spaces.Box(0, 3, (1,), np.int8),
                "opp_cards": spaces.Box(0, 108, (1,), np.int16),
            }
        )

    # ---- game logic -----------------------------------------------------------------------
    def reset(self, *, seed=None, options=None):
        self._reset_rng(seed)
        deck = [int(x) for x in self.np_random.permutation(make_deck())]
        self.deck = deck
        self.hands = [[self._draw_one() for _ in range(self.hand_size)] for _ in range(2)]
        while True:  # starting card must be a plain number card
            top = self._draw_one()
            if top < 52 and top % 13 < 10:
                break
            self.deck.insert(0, top)
        self.discard = [top]
        self.color = top // 13
        self.turn = 0  # 0 = agent
        self.drew = False
        self.t = 0
        self._bot_turns()
        return self._observation(), {}

    def _draw_one(self) -> int:
        if not self.deck:
            keep = self.discard[-1:]
            self.deck = [int(x) for x in self.np_random.permutation(self.discard[:-1])] if len(self.discard) > 1 else make_deck()
            self.discard = keep
        return self.deck.pop()

    def _playable(self, card: int) -> bool:
        if card >= 52:
            return True
        top = self.discard[-1]
        return card // 13 == self.color or (top < 52 and card % 13 == top % 13)

    def _apply_play(self, player: int, action: int) -> bool:
        """Play ``action`` (0..59) for ``player``. Returns True if the turn passes to the opponent."""
        if action < 52:
            card, new_color = action, action // 13
        elif action < 56:
            card, new_color = 52, action - 52
        else:
            card, new_color = 53, action - 56
        self.hands[player].remove(card)
        self.discard.append(card)
        self.color = new_color
        other = 1 - player
        if card == 53:
            self.hands[other] += [self._draw_one() for _ in range(4)]
            return False
        if card < 52 and card % 13 == 12:
            self.hands[other] += [self._draw_one() for _ in range(2)]
            return False
        if card < 52 and card % 13 in (10, 11):
            return False
        return True

    def _legal_for(self, player: int, drew: bool) -> list[int]:
        out = []
        for card in sorted(set(self.hands[player])):
            if not self._playable(card):
                continue
            if card < 52:
                out.append(card)
            elif card == 52:
                out += [52 + c for c in range(4)]
            else:
                out += [56 + c for c in range(4)]
        out.append(PASS if drew else DRAW)
        if drew:
            pass  # after drawing: play the drawn card (or any playable) or pass
        return out

    def _bot_turns(self):
        """Run the bot until it is the agent's turn or the game ends."""
        while self.turn == 1 and not self._over():
            legal = [a for a in self._legal_for(1, False) if a != DRAW]
            if legal:
                action = legal[int(self.np_random.integers(len(legal)))]
                passes = self._apply_play(1, action)
                if not self._over():
                    self.turn = 0 if passes else 1
                self.t += 1
            else:
                self.hands[1].append(self._draw_one())
                keep_turn = False
                if self._playable(self.hands[1][-1]) and self.hands[1][-1] < 52:
                    keep_turn = not self._apply_play(1, self.hands[1][-1])
                self.turn = 1 if keep_turn and not self._over() else 0
                self.t += 1
            if self.t > self.max_turns:
                break
        self.drew = False

    def _over(self):
        return not self.hands[0] or not self.hands[1]

    # ---- gym API --------------------------------------------------------------------------
    def _observation(self):
        hand = np.zeros(54, dtype=np.int8)
        for c in self.hands[0]:
            hand[c] += 1
        return {
            "hand": hand,
            "top": np.array([self.discard[-1]], dtype=np.int8),
            "color": np.array([self.color], dtype=np.int8),
            "opp_cards": np.array([len(self.hands[1])], dtype=np.int16),
        }

    def legal_actions(self):
        return self._legal_for(0, self.drew)

    def step(self, action):
        action = int(action)
        if action not in self.legal_actions():
            return self._illegal()
        self.t += 1
        if action == DRAW:
            self.hands[0].append(self._draw_one())
            self.drew = True
        elif action == PASS:
            self.turn, self.drew = 1, False
        else:
            passes = self._apply_play(0, action)
            self.drew = False
            if not self._over():
                self.turn = 1 if passes else 0
        if self.turn == 1:
            self._bot_turns()
        won = not self.hands[0]
        lost = not self.hands[1]
        done = won or lost
        reward = 1.0 if won else -1.0 if lost else 0.0
        return self._observation(), reward, done, not done and self.t >= self.max_turns, {}

    def action_name(self, action):
        if action == DRAW:
            return "draw"
        if action == PASS:
            return "pass"
        if action < 52:
            return card_name(action)
        if action < 56:
            return f"wild {COLORS[action - 52]}"
        return f"wild4 {COLORS[action - 56]}"

    def text_observation(self):
        hand = ", ".join(card_name(c) for c in sorted(self.hands[0]))
        return (
            f"Top card: {card_name(self.discard[-1])}. Current colour: {COLORS[self.color]}.\n"
            f"Your hand: {hand}\nOpponent holds {len(self.hands[1])} cards."
            + ("\nYou already drew this turn: play a card or 'pass'." if self.drew else "")
        )

    def action_prompt(self):
        return (
            "Uno, 2 players. Match the top card by colour or face. Reply with the card to play, e.g. 'red 7', "
            "'blue skip', 'green draw2', or for wilds 'wild red' / 'wild4 blue' (the colour you choose). "
            "Reply 'draw' if you cannot or do not want to play; after drawing you may play or reply 'pass'. "
            "Skip, reverse and draw2 / wild4 give you another turn (opponent skipped). First to empty their hand wins."
        )
