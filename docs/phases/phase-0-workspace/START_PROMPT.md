# Phase 0 — start prompt

**Status: done (2026-10-02).** No prompt is needed; this file exists to keep the per-phase layout uniform.

What exists: a Cargo workspace with four crates (`water_sort_core`, `uniform_water_sort`, `turan_water_sort`, `water_sort_cli`), toolchain pinned to 1.99.0 in `rust-toolchain.toml`, workspace lints (`unsafe_code = "forbid"`, clippy pedantic), and GitHub Actions CI (`.github/workflows/ci.yml`) on Ubuntu + Windows plus an Ubuntu `heavy-tests` job. See [PLAN.md](PLAN.md).
