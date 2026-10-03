# Turan difficulty follow-up (D16): opt band, PourWalk, ReverseSearch

Follow-up to D15 before Phase 4. The planned non-absorbing scramble did not work (see
[`turan_mixing_probe.md`](turan_mixing_probe.md): a reverse walk makes at most
`n_colors × (capacity − 1)` progressing steps, whatever rule picks them). What was built instead,
after looking at how other puzzle generators handle difficulty:

- **A. `opt_moves` band.** `GenConfig::max_opt`, together with `min_opt`, rejects puzzles
  outside a target range, for every generator. Commercial water sort and ball sort games do the
  same: generate, solve, reject.
- **B. `TuranStrategy::PourWalk { steps }`** (distributed only): a random walk over pours *and*
  reverse pours. It is the water sort version of a random-move Rubik's cube scramble: it mixes
  towards uniform, so `steps` really controls difficulty.
- **C. `TuranStrategy::ReverseSearch { max_depth, max_states }`**: the Sokoban generator of the
  I2A paper (Weber, Racanière et al. 2017, after Taylor & Parberry 2011), which produced the
  Boxoban levels (Guez et al. 2019). It runs a depth-first search over reverse moves from the
  solved state, scores every visited state, and keeps the best one.
  - Score: color switches along the path × `segments − n_colors`. I2A uses box switches × box
    displacement.
  - A state scores 0 if it has a sorted tube (I2A: a box on its target) or, in the standard
    layout, if its heights are not standard.
  - Depth limit 300, I2A's value.

Boxoban then built its medium and hard sets by filtering the generated levels on an agent's
success. A is the solver-based version of that filter. Filtering on an agent belongs to Phase 7.

Reproduce (release build, `--threads 3` on 4 cores):

| report | command | wall |
|---|---|---:|
| [`turan_pour_walk_steps_sweep.md`](turan_pour_walk_steps_sweep.md) | `sweep --strategy pour-walk --layouts distributed --steps 10,20,40,80,160,320,640 --samples 300` | 278 s |
| [`turan_reverse_search_budget_sweep.md`](turan_reverse_search_budget_sweep.md) | `sweep --strategy reverse-search --steps 1000,3000,10000,30000,100000 --samples 300` | 2998 s |
| [`uniform_vs_turan_pour_walk_distributed.md`](uniform_vs_turan_pour_walk_distributed.md) | `compare --a uniform:distributed --b turan:walk:160:distributed` (9 configs, 1000 samples, `--rollouts 64 --base-seed 20261002`) | 59 s |
| [`uniform_vs_turan_reverse_search_standard.md`](uniform_vs_turan_reverse_search_standard.md) | `compare --a uniform:standard --b turan:search:10000:standard` (same settings) | 95 s |
| [`uniform_vs_turan_reverse_search_distributed.md`](uniform_vs_turan_reverse_search_distributed.md) | `compare --a uniform:distributed --b turan:search:10000:distributed` (same settings) | 97 s |

The compares use the same 9 configurations, seeds and settings as the Phase 3 reports
([`uniform_vs_turan_standard.md`](uniform_vs_turan_standard.md),
[`uniform_vs_turan_distributed.md`](uniform_vs_turan_distributed.md)). The Scramble columns
below come from those reports.

## 1. Mean `opt_moves` against uniform, per strategy

Each strategy column holds: mean `opt_moves` / KS distance of its `opt_moves` distribution from
uniform's (0 = same, 1 = disjoint) / distinct puzzles out of 1000 (canonical hash) / generation
p99 in ms.

**Standard**

| config | uniform | Scramble 40 | ReverseSearch 10k |
|---|---:|---:|---:|
| 4x4x2 | 11.20 | 9.46 / 0.472 / 69 % / 1.3 | 11.36 / 0.121 / 80 % / 30.3 |
| 6x4x2 | 17.92 | 13.57 / 0.817 / 100 % / 5.5 | 15.33 / 0.635 / 100 % / 37.6 |
| 8x4x2 | 24.59 | 17.36 / 0.953 / 100 % / 21.1 | 19.06 / 0.897 / 100 % / 62.3 |
| 10x4x2 | 31.26 | 20.94 / 0.986 / 100 % / 78.4 | 22.57 / 0.976 / 100 % / 105.9 |
| 6x3x1 | 12.07 | 10.12 / 0.551 / 61 % / 0.8 | 12.36 / 0.148 / 30 % / 19.7 |
| 9x3x1 | 18.64 | 14.12 / 0.851 / 94 % / 3.4 | 16.29 / 0.631 / 86 % / 27.4 |
| 6x4x1 | 17.54 | 11.79 / 0.921 / 92 % / 1.4 | 15.18 / 0.657 / 91 % / 21.4 |
| 8x4x1 | 23.86 | 14.88 / 0.979 / 100 % / 2.4 | 18.23 / 0.905 / 100 % / 23.2 |
| 7x5x2 | 27.33 | 17.66 / 0.981 / 100 % / 24.2 | 19.52 / 0.964 / 100 % / 66.8 |

**Distributed**

| config | uniform | Scramble 40 | ReverseSearch 10k | PourWalk 160 |
|---|---:|---:|---:|---:|
| 4x4x2 | 10.61 | 9.06 / 0.414 / 88 % / 3.6 | 10.68 / 0.107 / 93 % / 34.2 | 10.32 / 0.096 / 100 % / 2.0 |
| 6x4x2 | 17.12 | 13.15 / 0.775 / 100 % / 6.2 | 14.66 / 0.582 / 100 % / 37.4 | 16.39 / 0.186 / 100 % / 9.4 |
| 8x4x2 | 23.70 | 16.91 / 0.944 / 100 % / 22.0 | 18.42 / 0.884 / 100 % / 49.1 | 22.13 / 0.337 / 100 % / 46.0 |
| 10x4x2 | 30.31 | 20.41 / 0.989 / 100 % / 91.7 | 21.93 / 0.973 / 100 % / 103.1 | 27.62 / 0.503 / 100 % / 171.2 |
| 6x3x1 | 11.53 | 9.66 / 0.540 / 71 % / 1.0 | 12.23 / 0.233 / 35 % / 20.9 | 11.22 / 0.074 / 100 % / 5.0 |
| 9x3x1 | 18.09 | 13.70 / 0.835 / 96 % / 4.8 | 16.20 / 0.521 / 88 % / 28.8 | 17.47 / 0.154 / 100 % / 24.2 |
| 6x4x1 | 16.84 | 11.14 / 0.905 / 97 % / 1.1 | 15.03 / 0.535 / 93 % / 21.5 | 14.85 / 0.400 / 100 % / 7.4 |
| 8x4x1 | 23.27 | 14.04 / 0.985 / 100 % / 2.7 | 18.08 / 0.895 / 100 % / 31.0 | 19.21 / 0.693 / 100 % / 38.9 |
| 7x5x2 | 26.50 | 16.99 / 0.982 / 100 % / 26.5 | 18.92 / 0.961 / 100 % / 62.3 | 23.13 / 0.605 / 100 % / 56.6 |

- **Scramble** stays the easiest strategy and the furthest from uniform (KS D 0.4–0.99).
- **ReverseSearch 10k**
  - Difficulty: 1.5–4 moves harder than Scramble everywhere. At small configurations it matches
    uniform's mean (4x4x2, 6x3x1). At large ones it stays well below (10x4x2: 22.6 vs 31.3), so
    its distribution stays distinct from uniform's.
  - Cost: solvable by construction, about 1 attempt per puzzle. Construction rejections (no
    state scored above 0) reach up to 33 % of attempts, only for large standard configurations
    with two empty tubes. The search itself costs 20–160 ms p99 per puzzle at 10k states and
    about 10× that at 100k.
  - **Repeats:** it keeps the best-scoring state, so small configurations repeat puzzles. 6x3x1
    gives only 30–35 % distinct puzzles in 1000 (Scramble: 61–71 %). Phase 4 deduplicates by
    canonical hash anyway, but a dataset of small configurations would then hold far fewer
    puzzles than were generated.
- **PourWalk 160**
  - Closest to uniform (KS D 0.07–0.7) and never repeats a puzzle.
  - `steps` sets how close: in the sweep, mean `opt_moves` levels off at 92–100 % of uniform's
    between 80 and 640 steps, later for larger configurations.
  - With one empty tube most endpoints are unsolvable, as for uniform (attempts mean up to
    about 60 at 12x3x1).

### Budget and steps

- **ReverseSearch:** raising the budget from 1k to 100k states adds 1–3 moves, at 100× the cost.
  The gain is largest with one empty tube (7x5x1 standard: 17.1 → 19.8). Configurations with
  capacity 3 and two empty tubes do not improve. 10k is where extra budget stops paying off.
- **PourWalk:** `steps` from 10 to 640 covers about 60 % to 92–100 % of uniform's mean.

## 2. Difficulty tiers through the `opt_moves` band (A)

The tiers are uniform's `opt_moves` tertiles for each configuration (easy / medium / hard). The
table gives the share of each generator's puzzles that fall in each tier. Hitting a tier with
the band costs about `1 / share` attempts, each a full generation and solve. A 0 % share means
that generator cannot produce the tier at any practical cost.

| layout | config | tiers (uniform opt) | generator | P(easy) | P(medium) | P(hard) |
|---|---|---|---|---:|---:|---:|
| standard | 6x3x1 | easy ≤ 12, medium 13–13, hard ≥ 14 | uniform | 58.7 % | 26.8 % | 14.5 % |
| standard | 6x3x1 | | Scramble 40 | 97.3 % | 2.2 % | 0.5 % |
| standard | 6x3x1 | | ReverseSearch 10k | 56.6 % | 34.1 % | 9.3 % |
| standard | 9x3x1 | easy ≤ 18, medium 19–19, hard ≥ 20 | uniform | 42.3 % | 28.9 % | 28.8 % |
| standard | 9x3x1 | | Scramble 40 | 99.7 % | 0.3 % | 0.0 % |
| standard | 9x3x1 | | ReverseSearch 10k | 96.6 % | 2.5 % | 0.9 % |
| standard | 6x4x1 | easy ≤ 17, medium 18–18, hard ≥ 19 | uniform | 46.7 % | 24.0 % | 29.3 % |
| standard | 6x4x1 | | Scramble 40 | 99.9 % | 0.0 % | 0.1 % |
| standard | 6x4x1 | | ReverseSearch 10k | 98.9 % | 1.0 % | 0.1 % |
| standard | 8x4x1 | easy ≤ 23, medium 24–25, hard ≥ 26 | uniform | 40.4 % | 41.9 % | 17.7 % |
| standard | 8x4x1 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| standard | 8x4x1 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| standard | 4x4x2 | easy ≤ 11, medium 12–12, hard ≥ 13 | uniform | 55.8 % | 23.5 % | 20.7 % |
| standard | 4x4x2 | | Scramble 40 | 92.1 % | 6.3 % | 1.6 % |
| standard | 4x4x2 | | ReverseSearch 10k | 54.0 % | 34.6 % | 11.4 % |
| standard | 6x4x2 | easy ≤ 17, medium 18–19, hard ≥ 20 | uniform | 37.6 % | 48.9 % | 13.5 % |
| standard | 6x4x2 | | Scramble 40 | 99.0 % | 1.0 % | 0.0 % |
| standard | 6x4x2 | | ReverseSearch 10k | 93.3 % | 6.7 % | 0.0 % |
| standard | 8x4x2 | easy ≤ 24, medium 25–25, hard ≥ 26 | uniform | 46.8 % | 23.9 % | 29.3 % |
| standard | 8x4x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| standard | 8x4x2 | | ReverseSearch 10k | 99.8 % | 0.2 % | 0.0 % |
| standard | 10x4x2 | easy ≤ 31, medium 32–32, hard ≥ 33 | uniform | 51.7 % | 23.8 % | 24.5 % |
| standard | 10x4x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| standard | 10x4x2 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| standard | 7x5x2 | easy ≤ 27, medium 28–28, hard ≥ 29 | uniform | 52.6 % | 20.3 % | 27.1 % |
| standard | 7x5x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| standard | 7x5x2 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| distributed | 6x3x1 | easy ≤ 11, medium 12–12, hard ≥ 13 | uniform | 46.1 % | 28.2 % | 25.7 % |
| distributed | 6x3x1 | | Scramble 40 | 92.0 % | 6.6 % | 1.4 % |
| distributed | 6x3x1 | | ReverseSearch 10k | 22.8 % | 38.5 % | 38.7 % |
| distributed | 6x3x1 | | PourWalk 160 | 53.5 % | 27.7 % | 18.8 % |
| distributed | 9x3x1 | easy ≤ 18, medium 19–19, hard ≥ 20 | uniform | 59.0 % | 24.6 % | 16.4 % |
| distributed | 9x3x1 | | Scramble 40 | 99.7 % | 0.3 % | 0.0 % |
| distributed | 9x3x1 | | ReverseSearch 10k | 96.9 % | 2.2 % | 0.9 % |
| distributed | 9x3x1 | | PourWalk 160 | 70.9 % | 17.5 % | 11.6 % |
| distributed | 6x4x1 | easy ≤ 16, medium 17–18, hard ≥ 19 | uniform | 38.6 % | 45.2 % | 16.2 % |
| distributed | 6x4x1 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| distributed | 6x4x1 | | ReverseSearch 10k | 92.1 % | 7.8 % | 0.1 % |
| distributed | 6x4x1 | | PourWalk 160 | 78.6 % | 18.4 % | 3.0 % |
| distributed | 8x4x1 | easy ≤ 23, medium 24–24, hard ≥ 25 | uniform | 53.0 % | 22.5 % | 24.5 % |
| distributed | 8x4x1 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| distributed | 8x4x1 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| distributed | 8x4x1 | | PourWalk 160 | 97.0 % | 2.2 % | 0.8 % |
| distributed | 4x4x2 | easy ≤ 10, medium 11–11, hard ≥ 12 | uniform | 43.8 % | 28.2 % | 28.0 % |
| distributed | 4x4x2 | | Scramble 40 | 85.2 % | 11.1 % | 3.7 % |
| distributed | 4x4x2 | | ReverseSearch 10k | 42.3 % | 38.1 % | 19.6 % |
| distributed | 4x4x2 | | PourWalk 160 | 53.4 % | 24.7 % | 21.9 % |
| distributed | 6x4x2 | easy ≤ 17, medium 18–18, hard ≥ 19 | uniform | 58.0 % | 23.4 % | 18.6 % |
| distributed | 6x4x2 | | Scramble 40 | 99.5 % | 0.4 % | 0.1 % |
| distributed | 6x4x2 | | ReverseSearch 10k | 97.4 % | 2.3 % | 0.3 % |
| distributed | 6x4x2 | | PourWalk 160 | 74.6 % | 14.9 % | 10.5 % |
| distributed | 8x4x2 | easy ≤ 23, medium 24–24, hard ≥ 25 | uniform | 42.2 % | 26.6 % | 31.2 % |
| distributed | 8x4x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| distributed | 8x4x2 | | ReverseSearch 10k | 99.6 % | 0.4 % | 0.0 % |
| distributed | 8x4x2 | | PourWalk 160 | 75.9 % | 14.0 % | 10.1 % |
| distributed | 10x4x2 | easy ≤ 30, medium 31–31, hard ≥ 32 | uniform | 51.5 % | 22.1 % | 26.4 % |
| distributed | 10x4x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| distributed | 10x4x2 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| distributed | 10x4x2 | | PourWalk 160 | 91.9 % | 4.7 % | 3.4 % |
| distributed | 7x5x2 | easy ≤ 26, medium 27–27, hard ≥ 28 | uniform | 47.6 % | 22.5 % | 29.9 % |
| distributed | 7x5x2 | | Scramble 40 | 100.0 % | 0.0 % | 0.0 % |
| distributed | 7x5x2 | | ReverseSearch 10k | 100.0 % | 0.0 % | 0.0 % |
| distributed | 7x5x2 | | PourWalk 160 | 94.2 % | 3.9 % | 1.9 % |

- Uniform reaches every tier in about 3–8 attempts.
- Scramble and ReverseSearch reach uniform's medium and hard tiers only at the smallest
  configurations. A band on them can only select easier subsets, so their difficulty range is
  their own. That is the distribution difference D1 asks for, but "hard" then has to be defined
  per generator.
- PourWalk (160 steps) lands 1–22 % of its puzzles in uniform's hard tier, and more with more
  steps.

## 3. Structural gap (from the probe)

13–49 % of uniform puzzles have no predecessor: no pour leads to them, so no reverse walk or
search can end there. Scramble and ReverseSearch never produce such puzzles. PourWalk can,
because it also pours forward. See [`turan_mixing_probe.md`](turan_mixing_probe.md) §4.

## Sources

- I2A level generator: Weber, Racanière et al., *Imagination-Augmented Agents for Deep
  Reinforcement Learning* (NeurIPS 2017), appendix "Level Generation for Sokoban",
  <https://arxiv.org/abs/1707.06203>. Taylor & Parberry, *Procedural Generation of Sokoban
  Levels* (GAMEON-NA 2011), <https://ianparberry.com/research/sokoban/>.
- Boxoban: Guez et al., *An Investigation of Model-Free Planning* (ICML 2019),
  <https://arxiv.org/abs/1901.03559>, and <https://github.com/google-deepmind/boxoban-levels>.
- Random-move vs random-state scrambles: <https://arxiv.org/abs/2410.20630>,
  <https://www.cuberoot.me/scramble/555-about>.
- Water sort and ball sort complexity: Kawahara et al., *Sorting Balls and Water: Equivalence
  and Computational Complexity* (2022), <https://arxiv.org/abs/2202.09495>.
