# `uniform_distributed` vs `turan_pour_walk_distributed`

- A: `uniform` / `fisher_yates(layout=distributed)`
- B: `turan` / `pour_walk(steps=160,layout=distributed)`

Produced by `water_sort_cli compare --a uniform:distributed --b turan:walk:160:distributed --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

Per configuration: the stats measurements side by side (rates per attempt; construction rejections are Turan attempts rejected before solving, D15), then per-puzzle metric means with two-sample tests over the accepted puzzles. chi² is Pearson's two-sample test on the histogram, with adjacent values merged until every bin holds at least 10 puzzles; KS is the two-sample Kolmogorov-Smirnov test (asymptotic p-value; conservative for discrete metrics). The random rates are per puzzle over its random legal-move rollouts (stuck: no legal move left; capped: hit `4 × opt_moves` moves). Overlap counts canonical hashes (puzzles equal up to tube order and color names) present in both samples.

## 4 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.00 | 0.10 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 10.61 / 11 / 13 | 10.32 / 10 / 13 |
| solve ms p50 / p99 | 0.15 / 0.9 | 0.14 / 0.6 |
| gen ms p50 / p99 | 0.6 / 1.8 | 0.8 / 2.0 |
| states/attempt p50 / p99 | 17 / 394 | 18 / 252 |
| symmetric % | 6.40 | 7.40 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 10.613 | 10.316 | 34.6 (8) | 3.2e-5 | 0.096 | 1.8e-4 |
| color changes | 8.362 | 8.024 | 37.7 (7) | 3.5e-6 | 0.089 | 6.6e-4 |
| segments | 13.898 | 13.592 | 36.6 (6) | 2.1e-6 | 0.092 | 3.8e-4 |
| random stuck rate | 0.001 | 0.001 |  |  | 0.003 | 1.000 |
| random capped rate | 0.013 | 0.013 |  |  | 0.026 | 0.884 |
| states_expanded | 42.892 | 37.547 |  |  | 0.019 | 0.993 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 999 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.90 %.

- opt_moves histogram (value:A/B): `4:1/0 5:1/2 6:3/4 7:19/29 8:51/75 9:145/149 10:218/275 11:282/247 12:192/178 13:78/39 14:10/2`
- color changes histogram (value:A/B): `2:1/0 3:0/2 4:5/4 5:20/37 6:63/102 7:168/181 8:263/283 9:263/252 10:172/123 11:45/16`
- segments histogram (value:A/B): `8:2/2 10:7/12 11:29/50 12:108/132 13:215/241 14:282/298 15:258/223 16:99/42`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.016`, B `0 / 0 / 0 / 0.016`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.031 / 0.234`, B `0 / 0 / 0.031 / 0.234`
- states_expanded p10 / p50 / p90 / p99: A `9 / 17 / 100 / 394`, B `9 / 18 / 92 / 252`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.70 | 0.50 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.12 / 17 / 20 | 16.39 / 16 / 20 |
| solve ms p50 / p99 | 0.84 / 7.4 | 0.72 / 5.6 |
| gen ms p50 / p99 | 2.2 / 9.2 | 2.1 / 9.4 |
| states/attempt p50 / p99 | 203 / 2985 | 154 / 2600 |
| symmetric % | 3.50 | 3.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.124 | 16.387 | 105.4 (8) | 3.3e-19 | 0.186 | 1.3e-15 |
| color changes | 14.171 | 13.486 | 102.9 (8) | 1.1e-18 | 0.187 | 8.8e-16 |
| segments | 21.854 | 21.138 | 115.9 (7) | 5.5e-22 | 0.196 | 2.7e-17 |
| random stuck rate | 0.029 | 0.024 |  |  | 0.018 | 0.997 |
| random capped rate | 0.197 | 0.192 |  |  | 0.029 | 0.789 |
| states_expanded | 439.099 | 364.705 |  |  | 0.078 | 0.004 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:0/6 12:0/12 13:14/27 14:33/85 15:103/162 16:171/215 17:259/239 18:234/149 19:139/79 20:44/26 21:3/0`
- color changes histogram (value:A/B): `8:0/1 9:1/9 10:8/29 11:27/68 12:91/146 13:170/231 14:268/244 15:255/181 16:155/74 17:24/17 18:1/0`
- segments histogram (value:A/B): `16:0/2 17:2/15 18:10/38 19:42/90 20:101/176 21:208/238 22:290/246 23:254/153 24:93/42`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.062 / 0.578`, B `0 / 0 / 0.047 / 0.516`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.562 / 0.844`, B `0 / 0.094 / 0.547 / 0.859`
- states_expanded p10 / p50 / p90 / p99: A `23 / 204 / 1157 / 2985`, B `20 / 156 / 974 / 2600`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.03 / 2 | 1.02 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 2.82 | 1.96 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.70 / 24 / 27 | 22.13 / 22 / 26 |
| solve ms p50 / p99 | 4.93 / 50.4 | 3.53 / 43.5 |
| gen ms p50 / p99 | 7.6 / 53.4 | 6.0 / 46.0 |
| states/attempt p50 / p99 | 1406 / 19812 | 961 / 19082 |
| symmetric % | 1.70 | 1.80 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.698 | 22.129 | 332.8 (10) | 1.8e-65 | 0.337 | 2.7e-50 |
| color changes | 20.072 | 18.599 | 361.9 (9) | 1.8e-72 | 0.373 | 1.6e-61 |
| segments | 29.855 | 28.282 | 426.0 (8) | 5.0e-87 | 0.391 | 1.5e-67 |
| random stuck rate | 0.084 | 0.088 |  |  | 0.024 | 0.933 |
| random capped rate | 0.517 | 0.482 |  |  | 0.073 | 0.009 |
| states_expanded | 3001.856 | 2329.892 |  |  | 0.108 | 1.5e-5 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `15:0/1 16:0/2 17:1/10 18:1/24 19:10/49 20:22/106 21:52/165 22:146/210 23:190/192 24:266/140 25:167/66 26:117/28 27:26/6 28:2/1`
- color changes histogram (value:A/B): `13:0/2 14:1/16 15:1/31 16:17/64 17:27/127 18:97/210 19:187/253 20:240/163 21:266/99 22:145/30 23:19/5`
- segments histogram (value:A/B): `23:0/4 24:2/18 25:3/44 26:20/68 27:32/151 28:112/259 29:203/219 30:255/154 31:261/66 32:112/17`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.297 / 0.797`, B `0 / 0 / 0.312 / 0.719`
- random capped rate p10 / p50 / p90 / p99: A `0.094 / 0.547 / 0.875 / 0.984`, B `0.062 / 0.484 / 0.875 / 0.984`
- states_expanded p10 / p50 / p90 / p99: A `134 / 1495 / 8047 / 19812`, B `75 / 1011 / 5550 / 19082`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.05 / 2 | 1.05 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 5.03 | 4.76 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 30.31 / 30 / 34 | 27.62 / 28 / 33 |
| solve ms p50 / p99 | 19.58 / 222.7 | 13.88 / 166.3 |
| gen ms p50 / p99 | 24.7 / 227.5 | 18.4 / 171.2 |
| states/attempt p50 / p99 | 5321 / 70404 | 3356 / 55063 |
| symmetric % | 0.70 | 1.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 30.314 | 27.616 | 670.8 (11) | 9.8e-137 | 0.503 | 1.5e-111 |
| color changes | 25.990 | 23.513 | 726.6 (10) | 1.2e-149 | 0.528 | 7.4e-123 |
| segments | 37.806 | 35.231 | 793.8 (9) | 4.5e-165 | 0.566 | 4.1e-141 |
| random stuck rate | 0.195 | 0.184 |  |  | 0.041 | 0.363 |
| random capped rate | 0.662 | 0.630 |  |  | 0.069 | 0.016 |
| states_expanded | 12104.885 | 7595.842 |  |  | 0.161 | 8.3e-12 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `20:0/1 21:0/2 22:0/5 23:0/19 24:0/49 25:4/90 26:20/129 27:40/184 28:89/177 29:158/146 30:204/117 31:221/47 32:168/23 33:75/8 34:17/3 35:4/0`
- color changes histogram (value:A/B): `17:0/3 18:0/3 19:0/18 20:1/41 21:6/83 22:7/140 23:42/204 24:109/201 25:157/155 26:282/89 27:249/47 28:135/15 29:12/1`
- segments histogram (value:A/B): `29:0/3 30:0/5 31:0/22 32:1/52 33:6/90 34:12/161 35:45/211 36:120/206 37:185/137 38:280/81 39:252/24 40:99/8`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.594 / 0.938`, B `0 / 0.078 / 0.516 / 0.859`
- random capped rate p10 / p50 / p90 / p99: A `0.234 / 0.734 / 0.969 / 1`, B `0.188 / 0.688 / 0.953 / 1`
- states_expanded p10 / p50 / p90 / p99: A `752 / 5856 / 32522 / 70404`, B `511 / 3650 / 18379 / 55063`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 4.29 / 17 | 4.16 / 19 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 76.71 | 75.98 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 11.53 / 12 / 14 | 11.22 / 11 / 14 |
| solve ms p50 / p99 | 0.01 / 0.3 | 0.02 / 0.3 |
| gen ms p50 / p99 | 0.5 / 1.5 | 1.1 / 5.0 |
| states/attempt p50 / p99 | 12 / 66 | 13 / 68 |
| symmetric % | 2.20 | 3.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.531 | 11.218 | 28.1 (7) | 2.1e-4 | 0.074 | 0.008 |
| color changes | 9.699 | 9.423 | 28.4 (6) | 8.0e-5 | 0.086 | 0.001 |
| segments | 16.553 | 16.221 | 38.8 (5) | 2.5e-7 | 0.113 | 4.9e-6 |
| random stuck rate | 0.336 | 0.355 |  |  | 0.034 | 0.603 |
| random capped rate | 0.312 | 0.284 |  |  | 0.049 | 0.176 |
| states_expanded | 21.803 | 21.111 |  |  | 0.043 | 0.307 |

Canonical-hash overlap: 1 puzzles of A's 1000 distinct also occur in B's 999 distinct (0.10 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.90 %.

- opt_moves histogram (value:A/B): `5:0/2 6:0/1 7:7/12 8:14/25 9:52/78 10:155/172 11:233/245 12:282/277 13:195/159 14:60/25 15:2/4`
- color changes histogram (value:A/B): `4:0/2 5:0/5 6:13/15 7:33/54 8:97/153 9:244/242 10:348/321 11:242/186 12:23/22`
- segments histogram (value:A/B): `11:0/2 12:1/6 13:14/16 14:35/63 15:124/170 16:252/282 17:355/323 18:219/138`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.188 / 0.875 / 0.984`, B `0 / 0.219 / 0.891 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.016 / 0.922 / 0.984`, B `0 / 0 / 0.875 / 0.969`
- states_expanded p10 / p50 / p90 / p99: A `11 / 19 / 37 / 54`, B `10 / 19 / 36 / 58`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 24.48 / 106 | 16.53 / 70 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 95.92 | 93.95 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.09 / 18 / 21 | 17.47 / 18 / 21 |
| solve ms p50 / p99 | 0.01 / 1.6 | 0.02 / 1.6 |
| gen ms p50 / p99 | 2.7 / 6.6 | 5.6 / 24.2 |
| states/attempt p50 / p99 | 10 / 155 | 16 / 166 |
| symmetric % | 1.60 | 1.60 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.093 | 17.468 | 89.3 (8) | 6.4e-16 | 0.154 | 7.7e-11 |
| color changes | 15.632 | 15.049 | 112.1 (7) | 3.4e-21 | 0.187 | 8.8e-16 |
| segments | 25.549 | 24.854 | 138.5 (6) | 2.0e-27 | 0.220 | 1.1e-21 |
| random stuck rate | 0.539 | 0.577 |  |  | 0.059 | 0.059 |
| random capped rate | 0.332 | 0.276 |  |  | 0.077 | 0.005 |
| states_expanded | 61.791 | 60.721 |  |  | 0.040 | 0.394 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `12:0/3 13:1/13 14:7/33 15:38/83 16:85/153 17:201/199 18:258/225 19:246/175 20:132/89 21:27/24 22:5/3`
- color changes histogram (value:A/B): `10:0/2 11:2/7 12:8/40 13:40/101 14:117/181 15:243/266 16:336/242 17:237/135 18:17/26`
- segments histogram (value:A/B): `20:0/2 21:2/11 22:8/43 23:43/111 24:129/199 25:251/287 26:338/236 27:229/111`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.641 / 0.984 / 1`, B `0 / 0.688 / 0.984 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.172 / 0.922 / 1`, B `0 / 0.109 / 0.875 / 0.984`
- states_expanded p10 / p50 / p90 / p99: A `23 / 53 / 107 / 200`, B `21 / 54 / 101 / 186`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 8.84 / 39 | 7.20 / 31 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 88.69 | 86.12 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 16.84 / 17 / 20 | 14.85 / 15 / 19 |
| solve ms p50 / p99 | 0.02 / 0.4 | 0.02 / 0.3 |
| gen ms p50 / p99 | 1.1 / 4.0 | 1.7 / 7.4 |
| states/attempt p50 / p99 | 20 / 138 | 20 / 131 |
| symmetric % | 0.30 | 1.30 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 16.836 | 14.852 | 449.3 (10) | 2.9e-90 | 0.400 | 1.1e-70 |
| color changes | 14.811 | 12.940 | 503.1 (9) | 1.2e-102 | 0.416 | 2.0e-76 |
| segments | 21.779 | 19.865 | 525.8 (9) | 1.7e-107 | 0.428 | 7.1e-81 |
| random stuck rate | 0.239 | 0.300 |  |  | 0.098 | 1.2e-4 |
| random capped rate | 0.571 | 0.447 |  |  | 0.144 | 1.6e-9 |
| states_expanded | 45.660 | 37.760 |  |  | 0.156 | 4.1e-11 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `7:0/1 8:0/1 9:0/5 10:0/16 11:1/32 12:12/80 13:23/118 14:53/162 15:108/182 16:189/189 17:259/125 18:193/59 19:123/23 20:37/7 21:2/0`
- color changes histogram (value:A/B): `6:0/1 7:0/2 8:0/13 9:1/21 10:5/61 11:13/115 12:41/182 13:111/192 14:210/204 15:281/138 16:230/62 17:105/8 18:3/1`
- segments histogram (value:A/B): `13:0/1 14:0/2 15:0/14 16:1/26 17:5/58 18:14/119 19:41/191 20:114/192 21:215/202 22:280/133 23:228/55 24:102/7`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.781 / 0.984`, B `0 / 0.156 / 0.859 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.688 / 0.969 / 1`, B `0 / 0.438 / 0.938 / 1`
- states_expanded p10 / p50 / p90 / p99: A `17 / 41 / 81 / 135`, B `13 / 32 / 69 / 123`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 46.73 / 223 | 21.96 / 91 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 97.86 | 95.45 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.27 / 23 / 27 | 19.21 / 19 / 24 |
| solve ms p50 / p99 | 0.03 / 1.0 | 0.04 / 1.2 |
| gen ms p50 / p99 | 4.1 / 16.6 | 7.2 / 38.9 |
| states/attempt p50 / p99 | 22 / 243 | 26 / 240 |
| symmetric % | 0.00 | 1.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.271 | 19.207 | 1105.8 (13) | 3.2e-228 | 0.693 | 2.5e-211 |
| color changes | 20.804 | 17.003 | 1174.8 (11) | 4.4e-245 | 0.713 | 1.1e-223 |
| segments | 29.785 | 25.917 | 1207.2 (11) | 4.6e-252 | 0.724 | 1.3e-230 |
| random stuck rate | 0.399 | 0.430 |  |  | 0.066 | 0.024 |
| random capped rate | 0.527 | 0.397 |  |  | 0.158 | 2.2e-11 |
| states_expanded | 99.321 | 76.218 |  |  | 0.213 | 2.4e-20 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:0/1 12:0/1 13:0/2 14:0/22 15:0/32 16:0/50 17:2/123 18:4/142 19:14/167 20:45/181 21:81/118 22:167/90 23:217/41 24:225/22 25:157/7 26:71/1 27:14/0 28:3/0`
- color changes histogram (value:A/B): `10:0/1 11:0/3 12:0/10 13:0/34 14:0/60 15:1/115 16:6/174 17:17/190 18:56/187 19:97/116 20:193/69 21:269/33 22:259/8 23:102/0`
- segments histogram (value:A/B): `19:0/1 20:0/4 21:0/10 22:0/34 23:0/68 24:1/115 25:6/180 26:18/190 27:55/184 28:101/119 29:194/58 30:270/33 31:256/4 32:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.328 / 0.938 / 1`, B `0 / 0.422 / 0.922 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0.016 / 0.547 / 0.969 / 1`, B `0 / 0.344 / 0.922 / 1`
- states_expanded p10 / p50 / p90 / p99: A `33 / 87 / 179 / 298`, B `20 / 63 / 141 / 304`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 2 | 1.01 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 1.19 | 1.38 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 26.50 / 27 / 30 | 23.13 / 23 / 28 |
| solve ms p50 / p99 | 7.86 / 114.2 | 3.44 / 53.0 |
| gen ms p50 / p99 | 10.5 / 118.1 | 5.9 / 56.6 |
| states/attempt p50 / p99 | 2446 / 37064 | 1092 / 22239 |
| symmetric % | 1.00 | 1.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 26.498 | 23.128 | 900.2 (12) | 5.1e-185 | 0.605 | 3.6e-161 |
| color changes | 23.145 | 19.946 | 918.6 (11) | 6.2e-190 | 0.615 | 1.6e-166 |
| segments | 31.970 | 28.705 | 961.3 (11) | 4.1e-199 | 0.637 | 1.3e-178 |
| random stuck rate | 0.041 | 0.041 |  |  | 0.030 | 0.753 |
| random capped rate | 0.496 | 0.421 |  |  | 0.138 | 8.7e-9 |
| states_expanded | 5602.835 | 2761.280 |  |  | 0.214 | 1.5e-20 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `16:0/1 17:0/4 18:0/11 19:0/41 20:0/65 21:6/108 22:13/149 23:34/168 24:79/190 25:130/124 26:214/81 27:225/39 28:182/13 29:91/6 30:22/0 31:4/0`
- color changes histogram (value:A/B): `14:0/4 15:0/12 16:0/40 17:1/70 18:5/102 19:14/176 20:50/198 21:87/170 22:154/126 23:228/64 24:271/26 25:137/11 26:47/0 27:6/1`
- segments histogram (value:A/B): `22:0/1 23:0/3 24:0/19 25:0/41 26:1/82 27:6/114 28:19/184 29:55/207 30:94/161 31:161/109 32:256/54 33:252/20 34:124/4 35:32/1`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.109 / 0.641`, B `0 / 0 / 0.109 / 0.594`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.844 / 0.953`, B `0.047 / 0.422 / 0.797 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `132 / 2504 / 15031 / 37064`, B `73 / 1127 / 6663 / 22239`

