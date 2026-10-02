# `uniform` vs `turan_scramble_standard`

- A: `uniform` / `fisher_yates`
- B: `turan` / `scramble(steps=40,max_extra_steps=100,layout=standard)`

Produced by `water_sort_cli compare --a uniform:standard --b turan:scramble:40:extra=100:standard --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

Per configuration: the stats measurements side by side (rates per attempt; construction rejections are Turan attempts rejected before solving, D15), then per-puzzle metric means with two-sample tests over the accepted puzzles. chi² is Pearson's two-sample test on the histogram, with adjacent values merged until every bin holds at least 10 puzzles; KS is the two-sample Kolmogorov-Smirnov test (asymptotic p-value; conservative for discrete metrics). The random rates are per puzzle over its random legal-move rollouts (stuck: no legal move left; capped: hit `4 × opt_moves` moves). Overlap counts canonical hashes (puzzles equal up to tube order and color names) present in both samples.

## 4 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 7.12 / 31 |
| construction rejections % | 0.00 | 85.96 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 11.20 / 11 / 14 | 9.46 / 9 / 13 |
| solve ms p50 / p99 | 0.12 / 0.5 | 0.00 / 0.2 |
| gen ms p50 / p99 | 0.6 / 1.1 | 0.6 / 1.3 |
| states/attempt p50 / p99 | 21 / 191 | 0 / 47 |
| symmetric % | 1.60 | 11.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.203 | 9.459 | 540.8 (8) | 1.2e-111 | 0.472 | 2.9e-98 |
| color changes | 9.483 | 8.064 | 481.7 (7) | 6.9e-100 | 0.411 | 1.3e-74 |
| segments | 13.483 | 12.064 | 481.7 (7) | 6.9e-100 | 0.411 | 1.3e-74 |
| random stuck rate | 0.003 | 0.005 |  |  | 0.068 | 0.019 |
| random capped rate | 0.010 | 0.005 |  |  | 0.055 | 0.094 |
| states_expanded | 30.048 | 21.028 |  |  | 0.267 | 9.9e-32 |

Canonical-hash overlap: 6 puzzles of A's 999 distinct also occur in B's 688 distinct (0.60 % of 1000). Distinct fraction within each sample: A 99.90 %, B 68.80 %.

- opt_moves histogram (value:A/B): `5:0/2 6:4/18 7:9/74 8:32/136 9:85/272 10:187/287 11:241/132 12:235/63 13:146/15 14:59/1 15:2/0`
- color changes histogram (value:A/B): `4:1/3 5:4/21 6:14/81 7:60/188 8:153/350 9:264/256 10:259/71 11:175/29 12:70/1`
- segments histogram (value:A/B): `8:1/3 9:4/21 10:14/81 11:60/188 12:153/350 13:264/256 14:259/71 15:175/29 16:70/1`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.078`, B `0 / 0 / 0.016 / 0.078`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.016 / 0.156`, B `0 / 0 / 0.016 / 0.062`
- states_expanded p10 / p50 / p90 / p99: A `11 / 21 / 48 / 191`, B `8 / 16 / 44 / 64`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 7.04 / 30 |
| construction rejections % | 0.00 | 85.80 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.92 / 18 / 21 | 13.57 / 14 / 17 |
| solve ms p50 / p99 | 0.73 / 6.2 | 0.00 / 1.2 |
| gen ms p50 / p99 | 2.2 / 7.9 | 1.5 / 5.5 |
| states/attempt p50 / p99 | 95 / 2492 | 0 / 250 |
| symmetric % | 0.80 | 4.40 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.923 | 13.575 | 1477.6 (11) | < 1e-300 | 0.817 | 1.5e-293 |
| color changes | 15.674 | 11.772 | 1430.1 (10) | < 1e-300 | 0.807 | 2.0e-286 |
| segments | 21.674 | 17.772 | 1430.1 (10) | < 1e-300 | 0.807 | 2.0e-286 |
| random stuck rate | 0.059 | 0.080 |  |  | 0.173 | 1.4e-13 |
| random capped rate | 0.186 | 0.025 |  |  | 0.574 | 4.1e-145 |
| states_expanded | 377.645 | 126.639 |  |  | 0.347 | 2.6e-53 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 995 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.50 %.

- opt_moves histogram (value:A/B): `8:0/2 9:0/6 10:0/27 11:0/87 12:0/143 13:8/211 14:14/222 15:31/172 16:102/91 17:221/29 18:247/8 19:242/2 20:111/0 21:23/0 22:1/0`
- color changes histogram (value:A/B): `7:0/2 8:0/11 9:0/47 10:0/130 11:7/251 12:15/246 13:46/188 14:133/91 15:218/29 16:272/4 17:227/1 18:82/0`
- segments histogram (value:A/B): `13:0/2 14:0/11 15:0/47 16:0/130 17:7/251 18:15/246 19:46/188 20:133/91 21:218/29 22:272/4 23:227/1 24:82/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.031 / 0.156 / 0.266`, B `0 / 0.062 / 0.172 / 0.297`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.141 / 0.438 / 0.656`, B `0 / 0 / 0.078 / 0.297`
- states_expanded p10 / p50 / p90 / p99: A `35 / 95 / 1060 / 2492`, B `13 / 61 / 140 / 1673`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 6.76 / 29 |
| construction rejections % | 0.00 | 85.20 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 24.59 / 25 / 28 | 17.36 / 17 / 22 |
| solve ms p50 / p99 | 5.13 / 38.7 | 0.00 / 7.7 |
| gen ms p50 / p99 | 7.4 / 40.9 | 3.6 / 21.1 |
| states/attempt p50 / p99 | 1798 / 15651 | 0 / 2610 |
| symmetric % | 0.40 | 2.50 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 24.587 | 17.361 | 1862.7 (15) | < 1e-300 | 0.953 | < 1e-300 |
| color changes | 21.749 | 15.336 | 1842.3 (12) | < 1e-300 | 0.953 | < 1e-300 |
| segments | 29.749 | 23.336 | 1842.3 (12) | < 1e-300 | 0.953 | < 1e-300 |
| random stuck rate | 0.191 | 0.222 |  |  | 0.162 | 6.0e-12 |
| random capped rate | 0.465 | 0.064 |  |  | 0.743 | 7.3e-243 |
| states_expanded | 2848.955 | 537.389 |  |  | 0.639 | 9.6e-180 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `10:0/1 11:0/1 12:0/4 13:0/18 14:0/37 15:0/91 16:0/167 17:0/225 18:0/175 19:1/152 20:9/91 21:21/22 22:64/10 23:143/6 24:230/0 25:239/0 26:178/0 27:93/0 28:22/0`
- color changes histogram (value:A/B): `9:0/1 10:0/1 11:0/7 12:0/37 13:0/90 14:0/170 15:0/240 16:0/209 17:4/151 18:15/66 19:44/15 20:130/11 21:215/2 22:262/0 23:224/0 24:106/0`
- segments histogram (value:A/B): `17:0/1 18:0/1 19:0/7 20:0/37 21:0/90 22:0/170 23:0/240 24:0/209 25:4/151 26:15/66 27:44/15 28:130/11 29:215/2 30:262/0 31:224/0 32:106/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.016 / 0.188 / 0.359 / 0.562`, B `0.078 / 0.219 / 0.375 / 0.500`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.750 / 0.906`, B `0 / 0.016 / 0.219 / 0.484`
- states_expanded p10 / p50 / p90 / p99: A `98 / 1798 / 6573 / 15651`, B `39 / 131 / 1636 / 6938`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 6.15 / 27 |
| construction rejections % | 0.00 | 83.75 |
| unsolvable % | 0.10 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 31.26 / 31 / 35 | 20.94 / 21 / 26 |
| solve ms p50 / p99 | 24.80 / 220.5 | 0.00 / 34.8 |
| gen ms p50 / p99 | 28.5 / 225.4 | 8.8 / 78.4 |
| states/attempt p50 / p99 | 7979 / 71744 | 0 / 10094 |
| symmetric % | 0.20 | 1.80 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 31.261 | 20.942 | 1952.4 (19) | < 1e-300 | 0.986 | < 1e-300 |
| color changes | 27.722 | 18.708 | 1935.5 (16) | < 1e-300 | 0.980 | < 1e-300 |
| segments | 37.722 | 28.708 | 1935.5 (16) | < 1e-300 | 0.980 | < 1e-300 |
| random stuck rate | 0.336 | 0.371 |  |  | 0.139 | 6.5e-9 |
| random capped rate | 0.564 | 0.104 |  |  | 0.777 | 1.5e-265 |
| states_expanded | 13012.148 | 2383.361 |  |  | 0.661 | 2.6e-192 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `14:0/2 15:0/7 16:0/8 17:0/36 18:0/67 19:0/124 20:0/196 21:0/164 22:0/162 23:0/120 24:1/57 25:3/38 26:5/14 27:17/3 28:41/2 29:79/0 30:166/0 31:205/0 32:238/0 33:164/0 34:65/0 35:15/0 36:1/0`
- color changes histogram (value:A/B): `13:0/6 14:0/10 15:0/34 16:0/78 17:0/140 18:0/214 19:0/175 20:0/160 21:2/94 22:3/56 23:6/24 24:18/8 25:46/1 26:116/0 27:202/0 28:278/0 29:230/0 30:99/0`
- segments histogram (value:A/B): `23:0/6 24:0/10 25:0/34 26:0/78 27:0/140 28:0/214 29:0/175 30:0/160 31:2/94 32:3/56 33:6/24 34:18/8 35:46/1 36:116/0 37:202/0 38:278/0 39:230/0 40:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.094 / 0.328 / 0.578 / 0.781`, B `0.188 / 0.375 / 0.562 / 0.703`
- random capped rate p10 / p50 / p90 / p99: A `0.281 / 0.578 / 0.828 / 0.953`, B `0 / 0.031 / 0.312 / 0.578`
- states_expanded p10 / p50 / p90 / p99: A `1521 / 7979 / 30303 / 71744`, B `99 / 265 / 8018 / 21901`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 2.88 / 11 | 2.52 / 9 |
| construction rejections % | 0.00 | 60.29 |
| unsolvable % | 65.28 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 12.07 / 12 / 15 | 10.12 / 10 / 13 |
| solve ms p50 / p99 | 0.04 / 0.3 | 0.00 / 0.3 |
| gen ms p50 / p99 | 0.4 / 1.6 | 0.4 / 0.8 |
| states/attempt p50 / p99 | 28 / 89 | 0 / 57 |
| symmetric % | 1.70 | 5.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 12.070 | 10.125 | 743.5 (7) | 2.9e-156 | 0.551 | 9.4e-134 |
| color changes | 10.430 | 8.804 | 744.9 (5) | 9.4e-159 | 0.567 | 1.3e-141 |
| segments | 16.430 | 14.804 | 744.9 (5) | 9.4e-159 | 0.567 | 1.3e-141 |
| random stuck rate | 0.580 | 0.567 |  |  | 0.198 | 1.2e-17 |
| random capped rate | 0.211 | 0.049 |  |  | 0.367 | 1.4e-59 |
| states_expanded | 29.340 | 20.961 |  |  | 0.336 | 5.3e-50 |

Canonical-hash overlap: 4 puzzles of A's 994 distinct also occur in B's 614 distinct (0.40 % of 1000). Distinct fraction within each sample: A 99.40 %, B 61.40 %.

- opt_moves histogram (value:A/B): `7:3/1 8:7/84 9:37/245 10:90/300 11:173/231 12:277/112 13:268/22 14:126/5 15:19/0`
- color changes histogram (value:A/B): `6:3/2 7:17/100 8:54/300 9:117/356 10:283/183 11:334/50 12:192/9`
- segments histogram (value:A/B): `12:3/2 13:17/100 14:54/300 15:117/356 16:283/183 17:334/50 18:192/9`
- random stuck rate p10 / p50 / p90 / p99: A `0.203 / 0.625 / 0.891 / 0.984`, B `0.391 / 0.562 / 0.781 / 0.953`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.062 / 0.625 / 0.891`, B `0 / 0 / 0.266 / 0.516`
- states_expanded p10 / p50 / p90 / p99: A `12 / 28 / 49 / 72`, B `9 / 13 / 44 / 60`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 12.49 / 57 | 2.46 / 9 |
| construction rejections % | 0.00 | 59.35 |
| unsolvable % | 92.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.64 / 19 / 21 | 14.12 / 14 / 18 |
| solve ms p50 / p99 | 0.07 / 2.0 | 0.00 / 2.3 |
| gen ms p50 / p99 | 2.7 / 7.3 | 1.6 / 3.4 |
| states/attempt p50 / p99 | 58 / 239 | 0 / 152 |
| symmetric % | 0.60 | 3.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.637 | 14.120 | 1534.6 (10) | < 1e-300 | 0.851 | < 1e-300 |
| color changes | 16.406 | 12.734 | 1512.8 (8) | < 1e-300 | 0.845 | < 1e-300 |
| segments | 25.406 | 21.734 | 1512.8 (8) | < 1e-300 | 0.845 | < 1e-300 |
| random stuck rate | 0.766 | 0.728 |  |  | 0.227 | 4.7e-23 |
| random capped rate | 0.174 | 0.036 |  |  | 0.472 | 2.9e-98 |
| states_expanded | 85.635 | 43.644 |  |  | 0.499 | 8.9e-110 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 944 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 94.40 %.

- opt_moves histogram (value:A/B): `11:0/37 12:1/114 13:0/212 14:6/247 15:19/200 16:57/124 17:127/44 18:213/19 19:289/3 20:197/0 21:83/0 22:7/0 23:1/0`
- color changes histogram (value:A/B): `10:0/40 11:1/139 12:2/275 13:13/255 14:47/199 15:146/68 16:268/24 17:348/0 18:175/0`
- segments histogram (value:A/B): `19:0/40 20:1/139 21:2/275 22:13/255 23:47/199 24:146/68 25:268/24 26:348/0 27:175/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.531 / 0.797 / 0.984 / 1`, B `0.594 / 0.719 / 0.891 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.438 / 0.906`, B `0 / 0 / 0.172 / 0.438`
- states_expanded p10 / p50 / p90 / p99: A `28 / 78 / 142 / 240`, B `13 / 24 / 97 / 187`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 7.45 / 33 | 4.86 / 20 |
| construction rejections % | 0.00 | 79.41 |
| unsolvable % | 86.58 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.54 / 18 / 21 | 11.79 / 12 / 15 |
| solve ms p50 / p99 | 0.04 / 0.4 | 0.00 / 0.4 |
| gen ms p50 / p99 | 1.0 / 3.2 | 0.5 / 1.4 |
| states/attempt p50 / p99 | 39 / 153 | 0 / 63 |
| symmetric % | 0.30 | 3.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.536 | 11.793 | 1748.7 (12) | < 1e-300 | 0.921 | < 1e-300 |
| color changes | 15.625 | 10.665 | 1708.5 (10) | < 1e-300 | 0.902 | < 1e-300 |
| segments | 21.625 | 16.665 | 1708.5 (10) | < 1e-300 | 0.902 | < 1e-300 |
| random stuck rate | 0.590 | 0.519 |  |  | 0.260 | 4.1e-30 |
| random capped rate | 0.307 | 0.074 |  |  | 0.586 | 3.1e-151 |
| states_expanded | 50.071 | 22.344 |  |  | 0.582 | 3.5e-149 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 923 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 92.30 %.

- opt_moves histogram (value:A/B): `8:0/4 9:0/47 10:0/153 11:1/217 12:1/277 13:13/177 14:31/92 15:61/25 16:144/5 17:216/2 18:240/0 19:187/1 20:82/0 21:23/0 23:1/0`
- color changes histogram (value:A/B): `7:0/5 8:0/51 9:1/159 10:1/241 11:3/272 12:20/176 13:51/74 14:131/18 15:220/3 16:273/0 17:232/0 18:68/1`
- segments histogram (value:A/B): `13:0/5 14:0/51 15:1/159 16:1/241 17:3/272 18:20/176 19:51/74 20:131/18 21:220/3 22:273/0 23:232/0 24:68/1`
- random stuck rate p10 / p50 / p90 / p99: A `0.219 / 0.625 / 0.891 / 1`, B `0.297 / 0.500 / 0.812 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.281 / 0.672 / 0.984`, B `0 / 0 / 0.359 / 0.562`
- states_expanded p10 / p50 / p90 / p99: A `19 / 48 / 82 / 131`, B `10 / 14 / 44 / 111`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 41.17 / 176 | 4.29 / 17 |
| construction rejections % | 0.00 | 76.67 |
| unsolvable % | 97.57 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.86 / 24 / 28 | 14.88 / 15 / 19 |
| solve ms p50 / p99 | 0.07 / 1.0 | 0.00 / 1.1 |
| gen ms p50 / p99 | 4.4 / 20.1 | 1.1 / 2.4 |
| states/attempt p50 / p99 | 59 / 284 | 0 / 144 |
| symmetric % | 0.00 | 1.60 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.856 | 14.884 | 1934.5 (17) | < 1e-300 | 0.979 | < 1e-300 |
| color changes | 21.539 | 13.731 | 1913.0 (14) | < 1e-300 | 0.974 | < 1e-300 |
| segments | 29.539 | 21.731 | 1913.0 (14) | < 1e-300 | 0.974 | < 1e-300 |
| random stuck rate | 0.709 | 0.638 |  |  | 0.257 | 2.0e-29 |
| random capped rate | 0.252 | 0.068 |  |  | 0.614 | 5.5e-166 |
| states_expanded | 111.069 | 36.990 |  |  | 0.686 | 4.3e-207 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 998 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.80 %.

- opt_moves histogram (value:A/B): `10:0/1 11:0/13 12:0/68 13:0/131 14:0/220 15:1/234 16:1/152 17:1/98 18:2/59 19:10/18 20:31/6 21:53/0 22:114/0 23:191/0 24:211/0 25:208/0 26:108/0 27:58/0 28:9/0 29:2/0`
- color changes histogram (value:A/B): `9:0/1 10:0/14 11:0/69 12:0/144 13:1/235 14:0/238 15:2/151 16:3/91 17:6/43 18:22/10 19:58/4 20:133/0 21:240/0 22:246/0 23:200/0 24:89/0`
- segments histogram (value:A/B): `17:0/1 18:0/14 19:0/69 20:0/144 21:1/235 22:0/238 23:2/151 24:3/91 25:6/43 26:22/10 27:58/4 28:133/0 29:240/0 30:246/0 31:200/0 32:89/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.438 / 0.734 / 0.984 / 1`, B `0.453 / 0.609 / 0.859 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.219 / 0.547 / 0.953`, B `0 / 0 / 0.312 / 0.531`
- states_expanded p10 / p50 / p90 / p99: A `36 / 100 / 191 / 347`, B `14 / 19 / 86 / 233`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 18.84 / 88 |
| construction rejections % | 0.00 | 94.69 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 27.33 / 27 / 31 | 17.66 / 18 / 23 |
| solve ms p50 / p99 | 4.56 / 44.5 | 0.00 / 2.3 |
| gen ms p50 / p99 | 6.8 / 46.9 | 3.6 / 24.2 |
| states/attempt p50 / p99 | 1769 / 19281 | 0 / 355 |
| symmetric % | 0.00 | 0.60 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 27.330 | 17.664 | 1938.0 (18) | < 1e-300 | 0.981 | < 1e-300 |
| color changes | 24.703 | 15.741 | 1932.5 (17) | < 1e-300 | 0.980 | < 1e-300 |
| segments | 31.703 | 22.741 | 1932.5 (17) | < 1e-300 | 0.980 | < 1e-300 |
| random stuck rate | 0.119 | 0.134 |  |  | 0.136 | 1.5e-8 |
| random capped rate | 0.496 | 0.066 |  |  | 0.783 | 1.1e-269 |
| states_expanded | 2762.470 | 485.932 |  |  | 0.611 | 2.3e-164 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:0/2 12:0/6 13:0/22 14:0/41 15:0/79 16:0/153 17:0/176 18:0/159 19:0/165 20:1/105 21:1/57 22:3/21 23:17/7 24:39/7 25:92/0 26:156/0 27:217/0 28:203/0 29:162/0 30:81/0 31:27/0 32:1/0`
- color changes histogram (value:A/B): `10:0/2 11:0/13 12:0/30 13:0/70 14:0/170 15:0/184 16:0/172 17:0/168 18:1/112 19:2/53 20:6/15 21:26/5 22:62/6 23:127/0 24:211/0 25:239/0 26:177/0 27:117/0 28:32/0`
- segments histogram (value:A/B): `17:0/2 18:0/13 19:0/30 20:0/70 21:0/170 22:0/184 23:0/172 24:0/168 25:1/112 26:2/53 27:6/15 28:26/5 29:62/6 30:127/0 31:211/0 32:239/0 33:177/0 34:117/0 35:32/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.266 / 0.375`, B `0.031 / 0.125 / 0.250 / 0.359`
- random capped rate p10 / p50 / p90 / p99: A `0.188 / 0.516 / 0.766 / 0.922`, B `0 / 0.016 / 0.219 / 0.453`
- states_expanded p10 / p50 / p90 / p99: A `96 / 1769 / 6337 / 19281`, B `28 / 165 / 593 / 6624`

