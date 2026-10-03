# Continuing development (cloud sessions)

Development continues phase by phase in cloud coding-agent sessions. Everything a session needs is in the repo; nothing depends on the original local machine.

## One-time setup (user)

1. Give the cloud environment access to the private repo `vinnipukh/jepa_games` (connect GitHub and allow the Claude GitHub app on this repo).
2. Allow network access in the cloud environment (rustup, crates.io; later PyPI / npm).
3. Optional: set `scripts/setup-cloud.sh` as the environment's setup script, so the toolchain is ready when a session starts.

## Per phase

1. Open a new session on `main`.
2. Paste the session prompt below. It sends the agent to the next phase's `START_PROMPT.md`. Alternatively, paste that phase's `START_PROMPT.md` block directly.
3. The agent works on branch `phase-N-<name>` and opens a PR when CI is green.
4. Review the PR (another session can do it: "review PR #N against docs/phases/phase-N-<name>/PLAN.md and docs/decisions.md"). Decide anything marked **proposed** in `docs/decisions.md`. Merge.
5. Repeat with the next phase.

Phase order and status: [`README.md`](README.md). Phase 6 is next.

## Session prompt (paste this)

```text
You are continuing development of the jepa_games project (github.com/vinnipukh/jepa_games,
private) in a fresh cloud session. All context is in the repo.

1. Run `scripts/setup-cloud.sh` (add --python for phases 5 and 7, --web for phase 6) if `cargo`
   or the pinned toolchain is missing.
2. Read CLAUDE.md, then docs/README.md. The status table shows the next phase that is not done.
   Phase order matters: every phase assumes all earlier phases are merged into main. If the
   previous phase's PR is still open, stop and tell me.
3. Open docs/phases/<that phase>/START_PROMPT.md and execute the prompt inside its code block
   exactly as written: read the files it lists, follow PLAN.md's task order, respect
   docs/decisions.md.
4. Work on the branch the prompt names, one focused commit per task, fmt + clippy + tests green
   after each. Record deviations as new D-entries. Ask me before decisions marked as mine in
   CLAUDE.md (supported ranges, defaults from measurements, external services, costly runs).
5. When the phase's "DONE WHEN" list is met and CI is green, push, open a PR to main with a
   summary (what was built, measurements, decisions recorded, anything left for me to decide)
   and report back. Do not merge it yourself.
```
