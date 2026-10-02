# `uniform_distributed` vs `turan_scramble_distributed`

- A: `uniform` / `fisher_yates(layout=distributed)`
- B: `turan` / `scramble(steps=40,layout=distributed)`

Produced by `water_sort_cli compare --a uniform:distributed --b turan:scramble:40:extra=100:distributed --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

Per configuration: the stats measurements side by side (rates per attempt; construction rejections are Turan attempts rejected before solving, D15), then per-puzzle metric means with two-sample tests over the accepted puzzles. chi² is Pearson's two-sample test on the histogram, with adjacent values merged until every bin holds at least 10 puzzles; KS is the two-sample Kolmogorov-Smirnov test (asymptotic p-value; conservative for discrete metrics). The random rates are per puzzle over its random legal-move rollouts (stuck: no legal move left; capped: hit `4 × opt_moves` moves). Overlap counts canonical hashes (puzzles equal up to tube order and color names) present in both samples.

## 4 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 10.61 / 11 / 13 | 9.06 / 9 / 12 |
| solve ms p50 / p99 | 0.15 / 0.9 | 0.14 / 0.6 |
| gen ms p50 / p99 | 0.7 / 2.8 | 0.6 / 3.6 |
| states/attempt p50 / p99 | 17 / 394 | 16 / 87 |
| symmetric % | 6.40 | 18.90 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 10.613 | 9.064 | 476.4 (8) | 8.0e-98 | 0.414 | 1.1e-75 |
| color changes | 8.362 | 7.419 | 237.1 (7) | 1.5e-47 | 0.277 | 4.0e-34 |
| segments | 13.898 | 12.439 | 529.9 (6) | 3.0e-111 | 0.449 | 5.8e-89 |
| random stuck rate | 0.001 | 0.003 |  |  | 0.051 | 0.144 |
| random capped rate | 0.013 | 0.006 |  |  | 0.063 | 0.036 |
| states_expanded | 42.892 | 22.129 |  |  | 0.201 | 3.6e-18 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 883 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 88.30 %.

- opt_moves histogram (value:A/B): `4:1/0 5:1/0 6:3/25 7:19/114 8:51/214 9:145/256 10:218/243 11:282/111 12:192/32 13:78/5 14:10/0`
- color changes histogram (value:A/B): `2:1/0 4:5/8 5:20/57 6:63/182 7:168/272 8:263/278 9:263/155 10:172/41 11:45/7`
- segments histogram (value:A/B): `8:2/0 9:0/2 10:7/43 11:29/176 12:108/305 13:215/284 14:282/151 15:258/35 16:99/4`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.016`, B `0 / 0 / 0 / 0.062`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.031 / 0.234`, B `0 / 0 / 0.016 / 0.047`
- states_expanded p10 / p50 / p90 / p99: A `9 / 17 / 100 / 394`, B `8 / 16 / 42 / 87`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.70 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.12 / 17 / 20 | 13.15 / 13 / 17 |
| solve ms p50 / p99 | 0.93 / 8.7 | 0.61 / 4.4 |
| gen ms p50 / p99 | 2.4 / 10.5 | 1.7 / 6.2 |
| states/attempt p50 / p99 | 203 / 2985 | 54 / 1690 |
| symmetric % | 3.50 | 10.10 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.124 | 13.154 | 1358.8 (11) | 9.5e-285 | 0.775 | 3.4e-264 |
| color changes | 14.171 | 11.164 | 1108.1 (9) | 8.5e-233 | 0.690 | 1.6e-209 |
| segments | 21.854 | 18.142 | 1401.9 (9) | 3.0e-296 | 0.790 | 1.7e-274 |
| random stuck rate | 0.029 | 0.031 |  |  | 0.231 | 7.4e-24 |
| random capped rate | 0.197 | 0.038 |  |  | 0.402 | 2.1e-71 |
| states_expanded | 439.099 | 143.536 |  |  | 0.482 | 1.9e-102 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 999 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.90 %.

- opt_moves histogram (value:A/B): `8:0/1 9:0/10 10:0/49 11:0/115 12:0/157 13:14/242 14:33/220 15:103/131 16:171/55 17:259/15 18:234/4 19:139/1 20:44/0 21:3/0`
- color changes histogram (value:A/B): `7:0/5 8:0/26 9:1/118 10:8/169 11:27/276 12:91/223 13:170/120 14:268/49 15:255/13 16:155/1 17:24/0 18:1/0`
- segments histogram (value:A/B): `13:0/1 14:0/2 15:0/27 16:0/116 17:2/179 18:10/274 19:42/219 20:101/127 21:208/46 22:290/9 23:254/0 24:93/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.062 / 0.578`, B `0 / 0 / 0.094 / 0.312`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.562 / 0.844`, B `0 / 0.016 / 0.125 / 0.297`
- states_expanded p10 / p50 / p90 / p99: A `23 / 204 / 1157 / 2985`, B `13 / 54 / 353 / 1690`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.03 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 2.82 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.70 / 24 / 27 | 16.91 / 17 / 21 |
| solve ms p50 / p99 | 4.90 / 48.4 | 1.98 / 20.0 |
| gen ms p50 / p99 | 7.8 / 51.4 | 3.8 / 22.0 |
| states/attempt p50 / p99 | 1406 / 19812 | 118 / 6017 |
| symmetric % | 1.70 | 7.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.698 | 16.909 | 1824.3 (14) | < 1e-300 | 0.944 | < 1e-300 |
| color changes | 20.072 | 14.704 | 1680.0 (12) | < 1e-300 | 0.900 | < 1e-300 |
| segments | 29.855 | 23.675 | 1803.1 (12) | < 1e-300 | 0.931 | < 1e-300 |
| random stuck rate | 0.084 | 0.103 |  |  | 0.285 | 4.3e-36 |
| random capped rate | 0.517 | 0.099 |  |  | 0.651 | 1.5e-186 |
| states_expanded | 3001.856 | 647.970 |  |  | 0.574 | 4.1e-145 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:0/2 12:0/4 13:0/27 14:0/65 15:0/124 16:0/176 17:1/229 18:1/183 19:10/111 20:22/57 21:52/19 22:146/2 23:190/1 24:266/0 25:167/0 26:117/0 27:26/0 28:2/0`
- color changes histogram (value:A/B): `10:0/4 11:0/22 12:0/72 13:0/136 14:1/209 15:1/262 16:17/153 17:27/88 18:97/39 19:187/12 20:240/3 21:266/0 22:145/0 23:19/0`
- segments histogram (value:A/B): `19:0/6 20:0/18 21:0/72 22:0/138 23:0/216 24:2/262 25:3/153 26:20/88 27:32/35 28:112/9 29:203/3 30:255/0 31:261/0 32:112/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.297 / 0.797`, B `0 / 0.062 / 0.281 / 0.500`
- random capped rate p10 / p50 / p90 / p99: A `0.094 / 0.547 / 0.875 / 0.984`, B `0 / 0.031 / 0.297 / 0.531`
- states_expanded p10 / p50 / p90 / p99: A `134 / 1495 / 8047 / 19812`, B `30 / 118 / 2258 / 6017`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.05 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 5.03 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 30.31 / 30 / 34 | 20.41 / 20 / 25 |
| solve ms p50 / p99 | 20.28 / 224.4 | 7.98 / 87.7 |
| gen ms p50 / p99 | 26.1 / 230.9 | 10.9 / 91.7 |
| states/attempt p50 / p99 | 5321 / 70404 | 244 / 22282 |
| symmetric % | 0.70 | 7.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 30.314 | 20.410 | 1965.9 (18) | < 1e-300 | 0.989 | < 1e-300 |
| color changes | 25.990 | 18.025 | 1909.9 (15) | < 1e-300 | 0.974 | < 1e-300 |
| segments | 37.806 | 28.988 | 1954.3 (15) | < 1e-300 | 0.986 | < 1e-300 |
| random stuck rate | 0.195 | 0.201 |  |  | 0.195 | 4.0e-17 |
| random capped rate | 0.662 | 0.167 |  |  | 0.675 | 1.6e-200 |
| states_expanded | 12104.885 | 2411.489 |  |  | 0.572 | 4.1e-144 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `14:0/1 15:0/5 16:0/22 17:0/46 18:0/108 19:0/157 20:0/191 21:0/179 22:0/123 23:0/91 24:0/50 25:4/20 26:20/7 27:40/0 28:89/0 29:158/0 30:204/0 31:221/0 32:168/0 33:75/0 34:17/0 35:4/0`
- color changes histogram (value:A/B): `12:0/1 13:0/3 14:0/23 15:0/58 16:0/134 17:0/191 18:0/207 19:0/163 20:1/109 21:6/69 22:7/30 23:42/9 24:109/3 25:157/0 26:282/0 27:249/0 28:135/0 29:12/0`
- segments histogram (value:A/B): `24:0/6 25:0/22 26:0/57 27:0/146 28:0/188 29:0/199 30:0/169 31:0/97 32:1/79 33:6/30 34:12/5 35:45/2 36:120/0 37:185/0 38:280/0 39:252/0 40:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.594 / 0.938`, B `0.016 / 0.156 / 0.453 / 0.688`
- random capped rate p10 / p50 / p90 / p99: A `0.234 / 0.734 / 0.969 / 1`, B `0 / 0.109 / 0.453 / 0.703`
- states_expanded p10 / p50 / p90 / p99: A `752 / 5856 / 32522 / 70404`, B `56 / 244 / 7150 / 22282`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 4.29 / 17 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 76.71 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 11.53 / 12 / 14 | 9.66 / 10 / 13 |
| solve ms p50 / p99 | 0.02 / 0.4 | 0.23 / 0.4 |
| gen ms p50 / p99 | 0.6 / 2.5 | 0.4 / 1.0 |
| states/attempt p50 / p99 | 12 / 66 | 11 / 58 |
| symmetric % | 2.20 | 5.60 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.531 | 9.660 | 726.5 (7) | 1.3e-152 | 0.540 | 1.7e-128 |
| color changes | 9.699 | 8.286 | 611.9 (6) | 6.4e-129 | 0.482 | 1.9e-102 |
| segments | 16.553 | 14.882 | 790.1 (5) | 1.6e-168 | 0.571 | 1.3e-143 |
| random stuck rate | 0.336 | 0.404 |  |  | 0.387 | 3.4e-66 |
| random capped rate | 0.312 | 0.054 |  |  | 0.339 | 6.8e-51 |
| states_expanded | 21.803 | 16.343 |  |  | 0.376 | 1.6e-62 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 713 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 71.30 %.

- opt_moves histogram (value:A/B): `7:7/5 8:14/165 9:52/324 10:155/274 11:233/152 12:282/66 13:195/11 14:60/3 15:2/0`
- color changes histogram (value:A/B): `5:0/2 6:13/32 7:33/207 8:97/365 9:244/263 10:348/97 11:242/30 12:23/4`
- segments histogram (value:A/B): `12:1/3 13:14/72 14:35/295 15:124/375 16:252/190 17:355/55 18:219/10`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.188 / 0.875 / 0.984`, B `0.109 / 0.406 / 0.750 / 0.953`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.016 / 0.922 / 0.984`, B `0 / 0 / 0.250 / 0.688`
- states_expanded p10 / p50 / p90 / p99: A `11 / 19 / 37 / 54`, B `8 / 11 / 33 / 58`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 24.48 / 106 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 95.92 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.09 / 18 / 21 | 13.70 / 14 / 18 |
| solve ms p50 / p99 | 0.01 / 1.9 | 1.39 / 3.8 |
| gen ms p50 / p99 | 3.2 / 7.9 | 1.8 / 4.8 |
| states/attempt p50 / p99 | 10 / 155 | 18 / 162 |
| symmetric % | 1.60 | 3.80 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.093 | 13.696 | 1528.4 (10) | < 1e-300 | 0.835 | < 1e-300 |
| color changes | 15.632 | 12.252 | 1376.9 (8) | 5.6e-292 | 0.775 | 3.4e-264 |
| segments | 25.549 | 21.842 | 1512.5 (8) | < 1e-300 | 0.829 | < 1e-300 |
| random stuck rate | 0.539 | 0.532 |  |  | 0.215 | 1.0e-20 |
| random capped rate | 0.332 | 0.052 |  |  | 0.476 | 6.3e-100 |
| states_expanded | 61.791 | 33.915 |  |  | 0.526 | 6.2e-122 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 960 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 96.00 %.

- opt_moves histogram (value:A/B): `11:0/71 12:0/162 13:1/241 14:7/230 15:38/177 16:85/76 17:201/30 18:258/10 19:246/3 20:132/0 21:27/0 22:5/0`
- color changes histogram (value:A/B): `9:0/6 10:0/89 11:2/209 12:8/300 13:40/206 14:117/132 15:243/45 16:336/13 17:237/0 18:17/0`
- segments histogram (value:A/B): `19:0/21 20:0/128 21:2/281 22:8/273 23:43/179 24:129/92 25:251/25 26:338/1 27:229/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.641 / 0.984 / 1`, B `0.172 / 0.594 / 0.859 / 0.969`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.172 / 0.922 / 1`, B `0 / 0 / 0.188 / 0.766`
- states_expanded p10 / p50 / p90 / p99: A `23 / 53 / 107 / 200`, B `12 / 18 / 79 / 162`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 8.84 / 39 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 88.69 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 16.84 / 17 / 20 | 11.14 / 11 / 15 |
| solve ms p50 / p99 | 0.02 / 0.4 | 0.20 / 0.5 |
| gen ms p50 / p99 | 1.1 / 4.1 | 0.4 / 1.1 |
| states/attempt p50 / p99 | 20 / 138 | 12 / 95 |
| symmetric % | 0.30 | 2.80 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 16.836 | 11.141 | 1723.0 (12) | < 1e-300 | 0.905 | < 1e-300 |
| color changes | 14.811 | 9.961 | 1693.7 (10) | < 1e-300 | 0.905 | < 1e-300 |
| segments | 21.779 | 16.749 | 1748.7 (10) | < 1e-300 | 0.921 | < 1e-300 |
| random stuck rate | 0.239 | 0.289 |  |  | 0.369 | 3.2e-60 |
| random capped rate | 0.571 | 0.096 |  |  | 0.618 | 3.8e-168 |
| states_expanded | 45.660 | 19.580 |  |  | 0.652 | 4.1e-187 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 971 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 97.10 %.

- opt_moves histogram (value:A/B): `7:0/1 8:0/22 9:0/113 10:0/215 11:1/259 12:12/203 13:23/128 14:53/44 15:108/14 16:189/1 17:259/0 18:193/0 19:123/0 20:37/0 21:2/0`
- color changes histogram (value:A/B): `6:0/2 7:0/24 8:0/125 9:1/236 10:5/269 11:13/202 12:41/107 13:111/29 14:210/6 15:281/0 16:230/0 17:105/0 18:3/0`
- segments histogram (value:A/B): `13:0/3 14:0/32 15:0/146 16:1/254 17:5/288 18:14/171 19:41/88 20:114/16 21:215/2 22:280/0 23:228/0 24:102/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.781 / 0.984`, B `0.047 / 0.188 / 0.656 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.688 / 0.969 / 1`, B `0 / 0 / 0.406 / 0.812`
- states_expanded p10 / p50 / p90 / p99: A `17 / 41 / 81 / 135`, B `10 / 12 / 42 / 95`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 46.73 / 223 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 97.86 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.27 / 23 / 27 | 14.04 / 14 / 18 |
| solve ms p50 / p99 | 0.03 / 0.9 | 0.70 / 1.4 |
| gen ms p50 / p99 | 4.0 / 15.6 | 1.1 / 2.7 |
| states/attempt p50 / p99 | 22 / 243 | 15 / 185 |
| symmetric % | 0.00 | 2.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.271 | 14.035 | 1950.3 (16) | < 1e-300 | 0.985 | < 1e-300 |
| color changes | 20.804 | 12.867 | 1926.6 (13) | < 1e-300 | 0.975 | < 1e-300 |
| segments | 29.785 | 21.652 | 1944.2 (13) | < 1e-300 | 0.981 | < 1e-300 |
| random stuck rate | 0.399 | 0.350 |  |  | 0.206 | 4.6e-19 |
| random capped rate | 0.527 | 0.105 |  |  | 0.654 | 2.9e-188 |
| states_expanded | 99.321 | 30.239 |  |  | 0.701 | 3.1e-216 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 999 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.90 %.

- opt_moves histogram (value:A/B): `10:0/1 11:0/59 12:0/128 13:0/222 14:0/221 15:0/177 16:0/103 17:2/58 18:4/22 19:14/6 20:45/2 21:81/1 22:167/0 23:217/0 24:225/0 25:157/0 26:71/0 27:14/0 28:3/0`
- color changes histogram (value:A/B): `8:0/1 9:0/1 10:0/69 11:0/125 12:0/235 13:0/241 14:0/173 15:1/94 16:6/43 17:17/13 18:56/4 19:97/1 20:193/0 21:269/0 22:259/0 23:102/0`
- segments histogram (value:A/B): `17:0/1 18:0/6 19:0/75 20:0/155 21:0/248 22:0/240 23:0/147 24:1/80 25:6/36 26:18/11 27:55/1 28:101/0 29:194/0 30:270/0 31:256/0 32:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.328 / 0.938 / 1`, B `0.078 / 0.266 / 0.719 / 0.969`
- random capped rate p10 / p50 / p90 / p99: A `0.016 / 0.547 / 0.969 / 1`, B `0 / 0 / 0.453 / 0.859`
- states_expanded p10 / p50 / p90 / p99: A `33 / 87 / 179 / 298`, B `12 / 15 / 71 / 185`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 1.19 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 26.50 / 27 / 30 | 16.99 / 17 / 22 |
| solve ms p50 / p99 | 7.02 / 102.7 | 1.60 / 24.5 |
| gen ms p50 / p99 | 9.6 / 104.8 | 3.3 / 26.5 |
| states/attempt p50 / p99 | 2446 / 37064 | 167 / 7419 |
| symmetric % | 1.00 | 7.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 26.498 | 16.993 | 1947.7 (18) | < 1e-300 | 0.982 | < 1e-300 |
| color changes | 23.145 | 14.873 | 1917.4 (16) | < 1e-300 | 0.973 | < 1e-300 |
| segments | 31.970 | 23.059 | 1948.9 (16) | < 1e-300 | 0.985 | < 1e-300 |
| random stuck rate | 0.041 | 0.048 |  |  | 0.311 | 6.7e-43 |
| random capped rate | 0.496 | 0.095 |  |  | 0.678 | 2.7e-202 |
| states_expanded | 5602.835 | 628.849 |  |  | 0.637 | 1.3e-178 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:0/4 12:0/9 13:0/27 14:0/64 15:0/125 16:0/174 17:0/200 18:0/172 19:0/118 20:0/65 21:6/30 22:13/9 23:34/3 24:79/0 25:130/0 26:214/0 27:225/0 28:182/0 29:91/0 30:22/0 31:4/0`
- color changes histogram (value:A/B): `9:0/1 10:0/11 11:0/27 12:0/69 13:0/122 14:0/195 15:0/203 16:0/182 17:1/102 18:5/59 19:14/22 20:50/5 21:87/2 22:154/0 23:228/0 24:271/0 25:137/0 26:47/0 27:6/0`
- segments histogram (value:A/B): `18:0/4 19:0/23 20:0/54 21:0/120 22:0/190 23:0/200 24:0/197 25:0/119 26:1/55 27:6/30 28:19/5 29:55/3 30:94/0 31:161/0 32:256/0 33:252/0 34:124/0 35:32/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.109 / 0.641`, B `0 / 0.016 / 0.125 / 0.297`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.844 / 0.953`, B `0 / 0.047 / 0.281 / 0.547`
- states_expanded p10 / p50 / p90 / p99: A `132 / 2504 / 15031 / 37064`, B `21 / 167 / 1703 / 7419`

