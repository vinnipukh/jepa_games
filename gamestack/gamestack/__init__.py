"""gamestack: a stack of RL environments with one API for RL / JEPA agents and LLMs."""

from gamestack.core import GameEnv
from gamestack.mix import MixEnv
from gamestack.registry import list_games, make, register

__version__ = "0.1.0"

__all__ = ["GameEnv", "MixEnv", "list_games", "make", "register"]
