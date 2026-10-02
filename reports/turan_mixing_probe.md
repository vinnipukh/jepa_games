# Turan non-absorbing scramble: probe (D15 follow-up (a))

Status: **the follow-up as planned does not work.** Steering the reverse walk away from states
without a predecessor does not make `steps` a difficulty knob. Work stopped here for a decision
(CLAUDE.md: stop and report with evidence before deviating).

Everything below is from throwaway probe programs (release build, 200–300 walks per cell, seeds
`splitmix64(i)`), not from the `sweep` command. The probe walk is the Phase 3 `Scramble` walk
(same candidate order, `bounded_u32`, inverse exclusion) plus a filter that keeps only reverse moves
into a state that is still "alive" after `depth` more reverse moves (depth 1 = the planned rule,
`has_reverse_move` of the successor), falling back to all moves when none qualifies.

## 1. `opt_moves` still saturates, at the same value

Distributed layout (no return step involved). `opt mean` is over all walks, `dead` the walks that
still ended in a state without a predecessor, `inverse/walk` the pours that undid the previous one
(the back-and-forth pattern).

| config | depth | steps 10 | steps 40 | steps 160 | steps 640 | dead (≥ 40 steps) | dies at step (mean) | inverse/walk at 640 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 4x4x2 | 0 (Scramble) | 7.88 | 9.05 | 9.05 | 9.05 | 151/200 | 11.6 | 153.8 |
| 4x4x2 | 1 (planned) | 7.86 | 8.95 | 8.95 | 8.95 | 97/200 | 12.1 | 323.3 |
| 4x4x2 | 2 | 7.85 | 8.97 | 8.97 | 8.97 | 69/200 | 12.1 | 411.1 |
| 6x3x1 | 0 | 8.79 | 9.61 | 9.61 | 9.61 | 200/200 | 10.5 | 0.0 |
| 6x3x1 | 1 | 8.81 | 9.69 | 9.69 | 9.69 | 190/200 | 11.0 | 31.5 |
| 6x3x1 | 2 | 8.82 | 9.76 | 9.76 | 9.76 | 190/200 | 10.9 | 31.5 |
| 8x4x1 | 0 | 9.51 | 14.22 | 14.22 | 14.22 | 200/200 | 15.1 | 0.0 |
| 8x4x1 | 1 | 9.51 | 14.51 | 14.51 | 14.51 | 188/200 | 15.6 | 37.5 |
| 8x4x1 | 2 | 9.51 | 14.64 | 14.64 | 14.64 | 184/200 | 15.7 | 50.0 |
| 7x5x2 | 0 | 9.04 | 17.16 | 17.16 | 17.16 | 183/200 | 19.2 | 52.6 |
| 7x5x2 | 1 | 9.04 | 17.23 | 17.23 | 17.23 | 154/200 | 19.6 | 142.3 |
| 7x5x2 | 2 | 9.04 | 17.32 | 17.32 | 17.32 | 131/200 | 19.8 | 195.0 |

Uniform means for reference (standard layout, `reports/uniform_vs_turan_standard.md`): 4x4x2
11.20, 6x3x1 12.07, 8x4x1 23.86, 7x5x2 27.33.

- Mean `opt_moves` at 40, 160 and 640 steps is identical to two decimals at every depth. The
  filter gains at most +0.2 moves.
- Most walks still end in a dead end at about the same step. The planned fallback ("no qualifying
  move: draw from all moves") is reached in a state where *every* successor is a dead end, and a
  deeper lookahead only moves that funnel back one step.
- The walks that survive do so only by moving one unit back and forth: up to 2 of every 3 pours
  undo the previous one. A guard against that has nothing else to pick (the inverse is excluded
  already unless it is the only allowed move), so the guard would only turn those walks into dead
  ends.

## 2. Standard layout: the planned arm never returns with 2 empty tubes

Same walks with the standard-layout return step, `max_extra_steps = 100`, 300 walks:

| config | Scramble returned | depth-1 arm returned |
|---|---:|---:|
| 3x3x1 | 116 | 121 |
| 6x3x1 | 124 | 140 |
| 8x4x1 | 64 | 85 |
| 9x3x1 | 110 | 143 |
| 4x4x2 | 39 | **0** |
| 6x4x2 | 34 | **0** |
| 10x4x2 | 49 | **0** |
| 7x5x2 | 13 | **0** |

With two empty tubes, the surviving walks back-and-forth on non-standard heights forever, so the
generator hits `TooManyAttempts` (10 000) at 4x4x2. The arm as specified cannot ship for the
standard layout.

## 3. Why: reverse pours never increase the top-run excess

Let `E(s) = Σ (top_run − 1)` over the non-empty tubes. A reverse move takes `k` units of the top
color `c` from tube `A` (keeping a `c` on top of `A`, or emptying it) and puts them on `B`, onto a
different color unless `A` is full. Case by case, `E` changes by:

- `k < run(A)`, `B` non-empty with top run `r_B` and top ≠ `c`: `−r_B` (≤ −1);
- `k < run(A)`, `B` empty: `−1`;
- `A` emptied (it held only the run), onto `B`: `−(r_B − 1)` (0 if `B` is empty or `r_B = 1`);
- `A` full, onto a `B` whose top is `c`: `0`.

So `E` never increases along a reverse walk (checked on 1.2 million reverse moves from random
walks over the 9 compare configurations). The solved state has `E₀ = n_colors × (capacity − 1)`;
every step that is not one of the two `E`-preserving kinds uses up at least 1. At `E = 0` (all top
runs have length 1) the only reverse moves left move a single-unit tube somewhere: the
back-and-forth pattern. A reverse walk therefore makes at most `E₀` progressing steps whatever rule
picks them, and `steps` beyond about `E₀` cannot add difficulty. This is a property of the reverse
move rule (D15: a pour moves the whole run that fits), not of the step selection. Any reverse-walk
arm will saturate.

## 4. Structural gap to uniform (requested diagnostic)

Share of uniform fills (Fisher-Yates over the layout, 100 000 per cell, before solving) with no
predecessor (`has_reverse_move` false), and their mean `E`:

| config | standard: no predecessor | standard: mean E | distributed: no predecessor | distributed: mean E | E₀ |
|---|---:|---:|---:|---:|---:|
| 4x4x2 | 41.7 % | 0.92 | 12.9 % | 1.07 | 12 |
| 6x4x2 | 43.7 % | 0.86 | 15.9 % | 0.99 | 18 |
| 8x4x2 | 44.6 % | 0.83 | 18.8 % | 0.94 | 24 |
| 10x4x2 | 45.1 % | 0.82 | 21.7 % | 0.91 | 30 |
| 6x3x1 | 48.0 % | 0.75 | 24.6 % | 0.79 | 12 |
| 9x3x1 | 49.1 % | 0.72 | 28.5 % | 0.76 | 18 |
| 6x4x1 | 43.7 % | 0.86 | 32.0 % | 0.95 | 18 |
| 8x4x1 | 44.6 % | 0.83 | 35.5 % | 0.91 | 24 |
| 7x5x2 | 41.6 % | 0.91 | 21.7 % | 1.08 | 28 |

About 45 % of uniform standard puzzles (13–36 % distributed) are states that no pour can lead to.
A rule that avoids such states excludes them by construction, so it would move Turan *further*
from uniform on that axis, not closer.
