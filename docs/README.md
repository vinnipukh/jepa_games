# jepa_games — Phase plans

This folder holds the detailed implementation plan for each phase in [`../roadmap.md`](../roadmap.md). The roadmap says *what* gets built; these documents say *how*, including design choices, task order, tests, and risks.

Each phase has its own folder under `phases/` with two files:

- `PLAN.md`: the detailed development plan.
- `START_PROMPT.md`: a self-contained prompt to paste into a new coding-agent session (local or cloud) to execute that phase. For finished phases it is kept as a record.

| Phase | Plan | Start prompt | Depends on | Status |
|---|---|---|---|---|
| 0 | [Workspace setup](phases/phase-0-workspace/PLAN.md) | [prompt](phases/phase-0-workspace/START_PROMPT.md) | — | done (2026-10-02) |
| 1 | [`water_sort_core`](phases/phase-1-core/PLAN.md) | [prompt](phases/phase-1-core/START_PROMPT.md) | 0 | done (2026-10-02) |
| 2 | [`uniform_water_sort`](phases/phase-2-uniform/PLAN.md) | [prompt](phases/phase-2-uniform/START_PROMPT.md) | 1 | done (2026-10-02; D3 proposed, awaiting decision) |
| 3 | [`turan_water_sort`](phases/phase-3-turan/PLAN.md) | [prompt](phases/phase-3-turan/START_PROMPT.md) | 1, 2 | not started — **next** |
| 4 | [Dataset and storage](phases/phase-4-dataset/PLAN.md) | [prompt](phases/phase-4-dataset/START_PROMPT.md) | 2, 3 | not started |
| 5 | [Python binding and Gymnasium env](phases/phase-5-python/PLAN.md) | [prompt](phases/phase-5-python/START_PROMPT.md) | 1–4 | not started |
| 6 | [Web UI (WASM)](phases/phase-6-web/PLAN.md) | [prompt](phases/phase-6-web/START_PROMPT.md) | 1–3, 5 | not started |
| 7 | [JEPA preparation](phases/phase-7-jepa/PLAN.md) | [prompt](phases/phase-7-jepa/START_PROMPT.md) | 4, 5 | not started |

Run phases in order, one PR per phase. A start prompt assumes all phases before it are merged into `main`. If an earlier phase recorded new decisions, read those in `decisions.md` too.

Decisions, including the places where these plans change the roadmap, are recorded in [`decisions.md`](decisions.md).

## Conventions

Each phase plan has the same sections:

- **Goal**: one paragraph.
- **Design**: types, modules, algorithms, and the reasons behind them.
- **Tasks**: ordered checklist. Each item should be a reviewable unit of work, roughly one commit or PR.
- **Tests**: what proves the phase works.
- **Acceptance**: the gate from the roadmap, plus anything added here.
- **Risks / open points**.

A phase is closed only when its acceptance gate passes in CI, not only on a local machine.

## Cross-cutting rules

1. **One source of game rules.** Everything game-related (moves, solving, stars, hashing) lives in `water_sort_core`. Generators, the CLI, Python, and the web only call into it.
2. **Bit-exact reproducibility across targets.** The same puzzle record must produce the same state on x86_64 native, wasm32, and through Python. This is enforced with golden test vectors (`water_sort_core/tests/golden/*.json`) checked on every target in CI.
3. **Determinism inside generation.** Nothing that decides whether a puzzle is accepted may depend on wall-clock time, thread scheduling, or hash-map iteration order.
4. **Stable hashes.** Never use `std::collections::hash_map::DefaultHasher` for persisted values. Its algorithm is not guaranteed to stay the same between Rust releases.
