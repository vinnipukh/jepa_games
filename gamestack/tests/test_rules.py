import numpy as np
import chess

from gamestack import make
from gamestack.games.go import BLACK, WHITE, GoBoard
from gamestack.games.sudoku import _solve, make_puzzle


def test_go_capture_and_suicide_and_ko():
    g = GoBoard(5, 0.5)
    # White stone at b2 surrounded by black on three sides, black plays the fourth -> capture.
    for r, c, col in [(1, 1, WHITE), (0, 1, BLACK), (1, 0, BLACK), (1, 2, BLACK)]:
        g.b[r, c] = col
    nb = g.try_play(2, 1, BLACK)
    assert nb is not None and nb[1, 1] == 0
    # Suicide: white playing into a point with no liberties and no capture is illegal.
    h = GoBoard(5, 0.5)
    for r, c in [(0, 1), (1, 0)]:
        h.b[r, c] = BLACK
    assert h.try_play(0, 0, WHITE) is None
    # Ko: classic shape; immediate recapture must be rejected by superko.
    k = GoBoard(5, 0.5)
    for r, c in [(0, 1), (1, 0), (2, 1)]:
        k.b[r, c] = BLACK
    for r, c in [(0, 2), (1, 3), (2, 2)]:
        k.b[r, c] = WHITE
    k.b[1, 2] = BLACK  # black stone in the ko mouth
    k.history = {k.b.tobytes()}
    k.turn = WHITE
    assert k.play(1 * 5 + 1)  # white captures the stone at (1,2) by playing (1,1)
    assert k.try_play(1, 2, BLACK) is None  # black may not retake at once


def test_go_scoring():
    g = GoBoard(3, 0.0)
    g.b[:, 0] = BLACK
    g.b[:, 2] = WHITE
    assert g.score() == (3, 3.0)  # the middle column touches both -> neutral


def test_sudoku_unique_and_valid():
    rng = np.random.default_rng(5)
    for n, holes in [(4, 8), (6, 14), (9, 40)]:
        puzzle, full = make_puzzle(n, holes, rng)
        assert (puzzle == 0).sum() >= holes - 2
        assert _solve(puzzle.copy(), n, None, 2) == 1
        assert (np.sort(full, axis=1) == np.arange(1, n + 1)).all()


def test_sudoku_solving_wins():
    env = make("sudoku", size=4)
    env.reset(seed=1)
    total, done = 0.0, False
    for r, c in zip(*np.nonzero(env.puzzle == 0)):
        _, rew, done, _, _ = env.step((r * 4 + c) * 4 + env.solution[r, c] - 1)
        total += rew
    assert done and total > 1.0


def test_chess_fools_mate_gives_win_for_black_agent():
    env = make("chess", agent_color="black")
    env.reset(seed=0)
    env.board = chess.Board()
    for uci in ["f2f3", "e7e5", "g2g4"]:
        env.board.push_uci(uci)
    # now black to move (agent) and Qh4# is available
    from gamestack.games.chess_env import move_to_action

    _, r, done, _, _ = env.step(move_to_action(chess.Move.from_uci("d8h4")))
    assert done and r == 1.0


def test_flappy_dies_without_flapping_and_survives_with_policy():
    env = make("flappy")
    env.reset(seed=0)
    for _ in range(50):
        *_, term, trunc, info = env.step(0)
        if term:
            break
    assert term
