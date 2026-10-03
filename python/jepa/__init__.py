"""JEPA world model for Water Sort (Phase 7): data loading, models, training, collapse
monitoring, latent planning, baselines and evaluation.

Every game rule comes from :mod:`jepa_water_sort` (the Rust core); this package only learns
and plans. Entry points: ``python -m jepa.pipeline`` (the whole experiment),
``python -m jepa.train``, ``python -m jepa.dqn``.
"""
