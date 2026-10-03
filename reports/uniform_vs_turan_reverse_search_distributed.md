# `uniform_distributed` vs `turan_reverse_search_distributed`

- A: `uniform` / `fisher_yates(layout=distributed)`
- B: `turan` / `reverse_search(max_depth=300,max_states=10000,layout=distributed)`

Produced by `water_sort_cli compare --a uniform:distributed --b turan:search:10000:depth=300:distributed --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

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
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 10.61 / 11 / 13 | 10.68 / 11 / 13 |
| solve ms p50 / p99 | 0.12 / 0.9 | 0.15 / 0.7 |
| gen ms p50 / p99 | 0.5 / 2.1 | 17.3 / 34.2 |
| states/attempt p50 / p99 | 17 / 394 | 16 / 261 |
| symmetric % | 6.40 | 8.10 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 10.613 | 10.682 | 106.6 (7) | 4.7e-20 | 0.107 | 1.9e-5 |
| color changes | 8.362 | 8.791 | 157.7 (6) | 1.8e-31 | 0.181 | 8.2e-15 |
| segments | 13.898 | 13.884 | 205.9 (5) | 1.5e-42 | 0.162 | 6.0e-12 |
| random stuck rate | 0.001 | 0.004 |  |  | 0.021 | 0.979 |
| random capped rate | 0.013 | 0.004 |  |  | 0.083 | 0.002 |
| states_expanded | 42.892 | 31.498 |  |  | 0.089 | 6.6e-4 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 931 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 93.10 %.

- opt_moves histogram (value:A/B): `4:1/0 5:1/0 6:3/0 7:19/0 8:51/17 9:145/96 10:218/310 11:282/381 12:192/159 13:78/35 14:10/2`
- color changes histogram (value:A/B): `2:1/0 4:5/0 5:20/0 6:63/7 7:168/69 8:263/286 9:263/433 10:172/175 11:45/29 12:0/1`
- segments histogram (value:A/B): `8:2/0 10:7/0 11:29/1 12:108/38 13:215/253 14:282/513 15:258/174 16:99/21`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.016`, B `0 / 0 / 0 / 0.156`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.031 / 0.234`, B `0 / 0 / 0.016 / 0.062`
- states_expanded p10 / p50 / p90 / p99: A `9 / 17 / 100 / 394`, B `10 / 16 / 55 / 261`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.70 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.12 / 17 / 20 | 14.66 / 15 / 18 |
| solve ms p50 / p99 | 0.82 / 6.6 | 0.63 / 5.5 |
| gen ms p50 / p99 | 2.0 / 8.1 | 19.8 / 37.4 |
| states/attempt p50 / p99 | 203 / 2985 | 58 / 2038 |
| symmetric % | 3.50 | 5.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.124 | 14.662 | 870.2 (9) | 1.6e-181 | 0.582 | 3.5e-149 |
| color changes | 14.171 | 12.397 | 633.0 (8) | 1.9e-131 | 0.513 | 5.3e-116 |
| segments | 21.854 | 19.488 | 994.4 (7) | 2.0e-210 | 0.655 | 7.8e-189 |
| random stuck rate | 0.029 | 0.045 |  |  | 0.212 | 3.7e-20 |
| random capped rate | 0.197 | 0.061 |  |  | 0.298 | 2.0e-39 |
| states_expanded | 439.099 | 246.283 |  |  | 0.304 | 5.2e-41 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `10:0/2 11:0/8 12:0/43 13:14/159 14:33/246 15:103/271 16:171/174 17:259/71 18:234/23 19:139/3 20:44/0 21:3/0`
- color changes histogram (value:A/B): `9:1/11 10:8/53 11:27/168 12:91/316 13:170/262 14:268/135 15:255/48 16:155/7 17:24/0 18:1/0`
- segments histogram (value:A/B): `16:0/2 17:2/29 18:10/171 19:42/323 20:101/285 21:208/139 22:290/44 23:254/7 24:93/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.062 / 0.578`, B `0 / 0 / 0.156 / 0.438`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.562 / 0.844`, B `0 / 0.016 / 0.203 / 0.375`
- states_expanded p10 / p50 / p90 / p99: A `23 / 204 / 1157 / 2985`, B `16 / 58 / 741 / 2038`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.03 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 2.82 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.70 / 24 / 27 | 18.42 / 18 / 23 |
| solve ms p50 / p99 | 4.88 / 52.7 | 2.37 / 21.4 |
| gen ms p50 / p99 | 7.5 / 55.8 | 22.2 / 49.1 |
| states/attempt p50 / p99 | 1406 / 19812 | 188 / 8029 |
| symmetric % | 1.70 | 4.30 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.698 | 18.424 | 1646.5 (12) | < 1e-300 | 0.884 | < 1e-300 |
| color changes | 20.072 | 15.907 | 1477.1 (11) | < 1e-300 | 0.811 | 2.9e-289 |
| segments | 29.855 | 24.996 | 1658.5 (10) | < 1e-300 | 0.896 | < 1e-300 |
| random stuck rate | 0.084 | 0.146 |  |  | 0.326 | 4.3e-47 |
| random capped rate | 0.517 | 0.159 |  |  | 0.560 | 3.8e-138 |
| states_expanded | 3001.856 | 952.726 |  |  | 0.371 | 7.2e-61 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `14:0/9 15:0/29 16:0/76 17:1/175 18:1/231 19:10/233 20:22/145 21:52/72 22:146/19 23:190/7 24:266/4 25:167/0 26:117/0 27:26/0 28:2/0`
- color changes histogram (value:A/B): `11:0/1 12:0/10 13:0/43 14:1/125 15:1/225 16:17/244 17:27/209 18:97/96 19:187/37 20:240/6 21:266/4 22:145/0 23:19/0`
- segments histogram (value:A/B): `20:0/1 21:0/2 22:0/33 23:0/115 24:2/224 25:3/272 26:20/204 27:32/102 28:112/36 29:203/9 30:255/2 31:261/0 32:112/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.297 / 0.797`, B `0 / 0.078 / 0.406 / 0.594`
- random capped rate p10 / p50 / p90 / p99: A `0.094 / 0.547 / 0.875 / 0.984`, B `0 / 0.109 / 0.406 / 0.656`
- states_expanded p10 / p50 / p90 / p99: A `134 / 1495 / 8047 / 19812`, B `29 / 188 / 2795 / 8029`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.05 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 5.03 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 30.31 / 30 / 34 | 21.93 / 22 / 26 |
| solve ms p50 / p99 | 18.69 / 210.8 | 9.74 / 72.7 |
| gen ms p50 / p99 | 23.8 / 215.5 | 34.0 / 103.1 |
| states/attempt p50 / p99 | 5321 / 70404 | 1419 / 22104 |
| symmetric % | 0.70 | 3.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 30.314 | 21.930 | 1928.8 (16) | < 1e-300 | 0.973 | < 1e-300 |
| color changes | 25.990 | 19.198 | 1872.5 (14) | < 1e-300 | 0.960 | < 1e-300 |
| segments | 37.806 | 30.283 | 1918.6 (13) | < 1e-300 | 0.974 | < 1e-300 |
| random stuck rate | 0.195 | 0.260 |  |  | 0.246 | 5.3e-27 |
| random capped rate | 0.662 | 0.212 |  |  | 0.630 | 1.0e-174 |
| states_expanded | 12104.885 | 3414.610 |  |  | 0.343 | 4.3e-52 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `17:0/5 18:0/24 19:0/58 20:0/128 21:0/218 22:0/192 23:0/181 24:0/96 25:4/70 26:20/25 27:40/2 28:89/1 29:158/0 30:204/0 31:221/0 32:168/0 33:75/0 34:17/0 35:4/0`
- color changes histogram (value:A/B): `15:0/13 16:0/51 17:0/93 18:0/195 19:0/215 20:1/213 21:6/120 22:7/74 23:42/21 24:109/5 25:157/0 26:282/0 27:249/0 28:135/0 29:12/0`
- segments histogram (value:A/B): `26:0/5 27:0/41 28:0/96 29:0/188 30:0/238 31:0/198 32:1/129 33:6/78 34:12/20 35:45/6 36:120/1 37:185/0 38:280/0 39:252/0 40:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.594 / 0.938`, B `0.016 / 0.234 / 0.562 / 0.750`
- random capped rate p10 / p50 / p90 / p99: A `0.234 / 0.734 / 0.969 / 1`, B `0 / 0.172 / 0.500 / 0.750`
- states_expanded p10 / p50 / p90 / p99: A `752 / 5856 / 32522 / 70404`, B `63 / 1419 / 9622 / 22104`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 4.29 / 17 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 76.71 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 11.53 / 12 / 14 | 12.23 / 12 / 14 |
| solve ms p50 / p99 | 0.01 / 0.3 | 0.26 / 0.7 |
| gen ms p50 / p99 | 0.5 / 1.7 | 10.8 / 20.9 |
| states/attempt p50 / p99 | 12 / 66 | 37 / 116 |
| symmetric % | 2.20 | 27.10 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.531 | 12.233 | 243.8 (6) | 8.6e-50 | 0.233 | 2.9e-24 |
| color changes | 9.699 | 10.369 | 256.2 (6) | 1.9e-52 | 0.287 | 1.3e-36 |
| segments | 16.553 | 16.519 | 314.2 (5) | 8.8e-66 | 0.158 | 2.2e-11 |
| random stuck rate | 0.336 | 0.462 |  |  | 0.359 | 5.0e-57 |
| random capped rate | 0.312 | 0.214 |  |  | 0.177 | 3.5e-14 |
| states_expanded | 21.803 | 36.654 |  |  | 0.462 | 3.7e-94 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 351 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 35.10 %.

- opt_moves histogram (value:A/B): `7:7/0 8:14/0 9:52/0 10:155/10 11:233/218 12:282/385 13:195/305 14:60/80 15:2/2`
- color changes histogram (value:A/B): `6:13/0 7:33/0 8:97/3 9:244/97 10:348/481 11:242/366 12:23/53`
- segments histogram (value:A/B): `12:1/0 13:14/0 14:35/0 15:124/16 16:252/510 17:355/413 18:219/61`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.188 / 0.875 / 0.984`, B `0.109 / 0.516 / 0.703 / 0.922`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.016 / 0.922 / 0.984`, B `0 / 0 / 0.719 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `11 / 19 / 37 / 54`, B `13 / 37 / 56 / 116`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 24.48 / 106 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 95.92 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.09 / 18 / 21 | 16.20 / 16 / 19 |
| solve ms p50 / p99 | 0.01 / 1.6 | 1.69 / 4.2 |
| gen ms p50 / p99 | 2.8 / 7.7 | 14.7 / 28.8 |
| states/attempt p50 / p99 | 10 / 155 | 71 / 266 |
| symmetric % | 1.60 | 26.80 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.093 | 16.196 | 698.7 (7) | 1.3e-146 | 0.521 | 1.2e-119 |
| color changes | 15.632 | 14.255 | 600.6 (6) | 1.7e-126 | 0.480 | 1.3e-101 |
| segments | 25.549 | 23.369 | 1075.4 (5) | 2.8e-230 | 0.699 | 5.3e-215 |
| random stuck rate | 0.539 | 0.697 |  |  | 0.304 | 5.2e-41 |
| random capped rate | 0.332 | 0.120 |  |  | 0.278 | 2.3e-34 |
| states_expanded | 61.791 | 78.302 |  |  | 0.233 | 2.9e-24 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 878 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 87.80 %.

- opt_moves histogram (value:A/B): `13:1/2 14:7/71 15:38/236 16:85/298 17:201/246 18:258/116 19:246/22 20:132/9 21:27/0 22:5/0`
- color changes histogram (value:A/B): `11:2/0 12:8/26 13:40/216 14:117/374 15:243/274 16:336/83 17:237/25 18:17/2`
- segments histogram (value:A/B): `21:2/0 22:8/197 23:43/386 24:129/298 25:251/91 26:338/26 27:229/2`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.641 / 0.984 / 1`, B `0.469 / 0.734 / 0.875 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.172 / 0.922 / 1`, B `0 / 0 / 0.375 / 0.766`
- states_expanded p10 / p50 / p90 / p99: A `23 / 53 / 107 / 200`, B `19 / 71 / 149 / 266`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 8.84 / 39 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 88.69 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 16.84 / 17 / 20 | 15.03 / 15 / 18 |
| solve ms p50 / p99 | 0.02 / 0.4 | 0.28 / 0.6 |
| gen ms p50 / p99 | 1.2 / 2.9 | 11.4 / 21.5 |
| states/attempt p50 / p99 | 20 / 138 | 49 / 118 |
| symmetric % | 0.30 | 8.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 16.836 | 15.026 | 732.0 (8) | 9.3e-153 | 0.535 | 4.0e-126 |
| color changes | 14.811 | 13.363 | 633.5 (6) | 1.4e-133 | 0.503 | 1.5e-111 |
| segments | 21.779 | 19.570 | 1059.9 (6) | 9.7e-226 | 0.689 | 6.6e-209 |
| random stuck rate | 0.239 | 0.502 |  |  | 0.469 | 5.0e-97 |
| random capped rate | 0.571 | 0.332 |  |  | 0.343 | 4.3e-52 |
| states_expanded | 45.660 | 49.439 |  |  | 0.136 | 1.5e-8 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 930 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 93.00 %.

- opt_moves histogram (value:A/B): `11:1/0 12:12/6 13:23/59 14:53/241 15:108/382 16:189/233 17:259/68 18:193/10 19:123/1 20:37/0 21:2/0`
- color changes histogram (value:A/B): `9:1/0 10:5/2 11:13/13 12:41/162 13:111/399 14:210/308 15:281/100 16:230/15 17:105/1 18:3/0`
- segments histogram (value:A/B): `16:1/2 17:5/0 18:14/83 19:41/415 20:114/364 21:215/115 22:280/19 23:228/2 24:102/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.078 / 0.781 / 0.984`, B `0.109 / 0.531 / 0.844 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.688 / 0.969 / 1`, B `0 / 0.266 / 0.766 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `17 / 41 / 81 / 135`, B `16 / 49 / 86 / 118`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 46.73 / 223 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 97.86 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.27 / 23 / 27 | 18.08 / 18 / 21 |
| solve ms p50 / p99 | 0.03 / 0.9 | 0.91 / 2.4 |
| gen ms p50 / p99 | 3.6 / 15.8 | 14.3 / 31.0 |
| states/attempt p50 / p99 | 22 / 243 | 81 / 228 |
| symmetric % | 0.00 | 8.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.271 | 18.079 | 1677.5 (12) | < 1e-300 | 0.895 | < 1e-300 |
| color changes | 20.804 | 16.360 | 1620.7 (9) | < 1e-300 | 0.877 | < 1e-300 |
| segments | 29.785 | 24.560 | 1765.8 (10) | < 1e-300 | 0.918 | < 1e-300 |
| random stuck rate | 0.399 | 0.631 |  |  | 0.371 | 7.2e-61 |
| random capped rate | 0.527 | 0.243 |  |  | 0.370 | 1.5e-60 |
| states_expanded | 99.321 | 84.280 |  |  | 0.106 | 2.3e-5 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 998 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.80 %.

- opt_moves histogram (value:A/B): `14:0/2 15:0/15 16:0/75 17:2/257 18:4/300 19:14/213 20:45/98 21:81/32 22:167/6 23:217/2 24:225/0 25:157/0 26:71/0 27:14/0 28:3/0`
- color changes histogram (value:A/B): `13:0/6 14:0/32 15:1/206 16:6/323 17:17/271 18:56/119 19:97/34 20:193/8 21:269/1 22:259/0 23:102/0`
- segments histogram (value:A/B): `22:0/21 23:0/167 24:1/314 25:6/297 26:18/144 27:55/45 28:101/11 29:194/1 30:270/0 31:256/0 32:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.328 / 0.938 / 1`, B `0.266 / 0.688 / 0.906 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0.016 / 0.547 / 0.969 / 1`, B `0 / 0.172 / 0.609 / 0.891`
- states_expanded p10 / p50 / p90 / p99: A `33 / 87 / 179 / 298`, B `21 / 81 / 147 / 228`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.01 / 2 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 1.19 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 26.50 / 27 / 30 | 18.92 / 19 / 23 |
| solve ms p50 / p99 | 7.04 / 96.1 | 1.73 / 35.3 |
| gen ms p50 / p99 | 9.6 / 98.4 | 23.4 / 62.3 |
| states/attempt p50 / p99 | 2446 / 37064 | 201 / 11503 |
| symmetric % | 1.00 | 3.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 26.498 | 18.921 | 1876.9 (15) | < 1e-300 | 0.961 | < 1e-300 |
| color changes | 23.145 | 16.592 | 1825.2 (14) | < 1e-300 | 0.945 | < 1e-300 |
| segments | 31.970 | 24.804 | 1890.3 (14) | < 1e-300 | 0.964 | < 1e-300 |
| random stuck rate | 0.041 | 0.069 |  |  | 0.328 | 1.1e-47 |
| random capped rate | 0.496 | 0.122 |  |  | 0.632 | 7.8e-176 |
| states_expanded | 5602.835 | 1159.754 |  |  | 0.461 | 9.3e-94 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `12:0/1 14:0/4 15:0/21 16:0/66 17:0/123 18:0/196 19:0/197 20:0/208 21:6/115 22:13/49 23:34/16 24:79/1 25:130/3 26:214/0 27:225/0 28:182/0 29:91/0 30:22/0 31:4/0`
- color changes histogram (value:A/B): `10:0/1 12:0/10 13:0/20 14:0/74 15:0/159 16:0/208 17:1/222 18:5/185 19:14/86 20:50/26 21:87/5 22:154/2 23:228/2 24:271/0 25:137/0 26:47/0 27:6/0`
- segments histogram (value:A/B): `19:0/1 20:0/2 21:0/17 22:0/64 23:0/129 24:0/208 25:0/242 26:1/183 27:6/113 28:19/31 29:55/8 30:94/0 31:161/2 32:256/0 33:252/0 34:124/0 35:32/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0.109 / 0.641`, B `0 / 0.031 / 0.203 / 0.484`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.844 / 0.953`, B `0 / 0.062 / 0.328 / 0.594`
- states_expanded p10 / p50 / p90 / p99: A `132 / 2504 / 15031 / 37064`, B `26 / 201 / 3365 / 11503`

