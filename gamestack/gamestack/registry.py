"""Name -> environment factory. Game modules are imported lazily so a missing optional dependency
(python-chess, the Rust water-sort extension, crafter) only breaks that one game."""

from __future__ import annotations

import importlib
from typing import Any

from gamestack.core import GameEnv

# name -> (module, class, extra pip/build requirement shown in errors)
_GAMES: dict[str, tuple[str, str, str]] = {
    "water_sort": ("gamestack.games.water_sort", "WaterSortGame", "cd python && maturin develop --release"),
    "sudoku": ("gamestack.games.sudoku", "SudokuEnv", ""),
    "flappy": ("gamestack.games.flappy", "FlappyEnv", ""),
    "pointer_chase": ("gamestack.games.pointer", "PointerChaseEnv", ""),
    "pointer_gui": ("gamestack.games.pointer", "PointerGuiEnv", ""),
    "chess": ("gamestack.games.chess_env", "ChessEnv", "pip install chess"),
    "go": ("gamestack.games.go", "GoEnv", ""),
    "crafter": ("gamestack.games.crafter_env", "CrafterGame", "pip install crafter"),
    "classify": ("gamestack.games.classify", "ClassificationEnv", ""),
    "uno": ("gamestack.games.uno", "UnoEnv", ""),
}


def register(name: str, module: str, cls: str, requirement: str = "") -> None:
    _GAMES[name] = (module, cls, requirement)


def list_games() -> list[str]:
    return sorted(_GAMES)


def make(name: str, **kwargs: Any) -> GameEnv:
    if name not in _GAMES:
        raise KeyError(f"unknown game {name!r}; available: {list_games()}")
    module, cls, requirement = _GAMES[name]
    try:
        return getattr(importlib.import_module(module), cls)(**kwargs)
    except ImportError as e:
        hint = f" (setup: {requirement})" if requirement else ""
        raise ImportError(f"game {name!r} is unavailable: {e}{hint}") from e
