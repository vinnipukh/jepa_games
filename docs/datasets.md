# Generating datasets

How to make puzzle datasets with `water_sort_cli` (Phase 4, D17). Build once with `cargo build --release -p water_sort_cli`; the binary is `target/release/water_sort_cli`. Datasets go under `data/` (gitignored).

## The default dataset

```
water_sort_cli generate --generator uniform --count 1e6 \
    --colors 6 --capacity 4 --empty 2 --split-files --out data/uniform_c6k4e2
water_sort_cli validate data/uniform_c6k4e2
```

## Harder datasets

Difficulty is set by two independent choices.

**1. Puzzle size.** More colors, taller tubes or fewer empty tubes make longer puzzles. Any cell of the supported range works (`water_sort_core::SUPPORTED`, D3); anything else is refused unless you pass `--allow-unsupported`.

| `--colors --capacity --empty` | optimal moves (mean / p99) | rough time for 1M, 4 cores |
|---|---|---|
| 6 4 2 (default) | 18 / 21 | 11 min (measured) |
| 8 4 2 | 25 / 28 | ~50 min |
| 9 4 2 | 28 / 31 | ~2 h |
| 10 4 2 | 31 / 35 | ~3.5 h |
| 11 4 2 (largest supported) | 35 / 38 | ~6 h |
| 8 4 1 (one empty tube) | 24 / 28 | ~25 min |
| 7 5 1 (taller tubes) | 27 / 31 | ~1 h |
| 12 3 2 (short tubes, many colors) | 27 / 30 | ~5 h |

Optimal-move columns are from [`reports/uniform_stats.md`](../reports/uniform_stats.md). Times are estimates from its generation p50 (measured on another machine), except the first row. Start with `--count 1e4` to measure the real speed on your machine.

**2. Difficulty tier.** `--tier hard` (or `medium`, `easy`) keeps only puzzles in that tier of the chosen size (D16). It costs about 3 attempts per kept puzzle, so it is roughly 3× slower than the plain size.

```
water_sort_cli generate --generator uniform --count 1e6 \
    --colors 9 --capacity 4 --empty 2 --tier hard --split-files --out data/uniform_c9k4e2_hard
```

You do not need `--tier` to sample by difficulty later: every record stores its `tier`, so a normal dataset can be filtered at load time.

Other options: `--layout distributed` (half-empty tubes, D14; supported range `SUPPORTED_DISTRIBUTED`), `--generator turan` (default reverse search: different puzzles, but mostly easy, D16), `--min-opt` / `--max-opt` for a custom band, `--format jsonl` for readable output, `--master-seed` to reproduce a run, `--threads` to limit cores.

## Turan datasets

Default (decided, D18): reverse search, the I2A / Boxoban-style generator (D16).

```
water_sort_cli generate --generator turan --count 1e6 \
    --colors 6 --capacity 4 --empty 2 --split-files --out data/turan_c6k4e2
```

About 85 puzzles/s on 2 threads at this size (~1.6 h per 1M on 4 cores), mostly easy-tier puzzles, duplicates rising with count (1.9 % at 20k). The other strategies stay available:

| `--strategy` | Builds | Notes |
|---|---|---|
| `reverse-search` (default) | best-scoring state of a search over reverse pours from solved | `--search-states` (10 000), `--search-depth` (300) |
| `pour-walk` | random walk over pours and reverse pours | `--layout distributed` only; `--steps` (160); closest to uniform's difficulty |
| `scramble` | random reverse pours from solved | `--steps` (40); short puzzles |
| `constrained` | uniform fill, no two touching units of one color | |

## Checking and combining

- `water_sort_cli validate <dir>`: re-checks every record and regenerates a 1 % sample.
- `water_sort_cli dedup-report <dir>`: duplicate, split and tier counts.
- `water_sort_cli leakage --a <dir> --a-split test --b <dir> --b-split all --exclude-from b --out <new dir>`: removes from B the puzzles that are in A's test split. Needed only when training on a whole dataset; train and test splits never overlap (D17).

## Trajectories (Phase 5)

Trajectory sets are collected on one split of a dataset with the Python logger (see
[`python/README.md`](../python/README.md), D19):

```bash
cd python
uv run --no-project python -m jepa_water_sort.logger collect --dataset ../data/uniform_c6k4e2 \
    --split train --source epsilon --epsilon 0.1 --policy-seed 0 --out ../data/traj_eps01_train
```

The split is re-checked per record, so a trajectory set never contains a puzzle of another
split. Same dataset, source, ε and policy seed give byte-identical shards.
