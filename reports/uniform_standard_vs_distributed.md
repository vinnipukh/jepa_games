# `uniform` vs `uniform_distributed`

- A: `uniform` / `fisher_yates`
- B: `uniform` / `fisher_yates(layout=distributed)`

Produced by `water_sort_cli compare --a uniform:standard --b uniform:distributed --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

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
| opt mean / p50 / p99 | 11.20 / 11 / 14 | 10.61 / 11 / 13 |
| solve ms p50 / p99 | 0.13 / 0.5 | 0.13 / 0.9 |
| gen ms p50 / p99 | 0.6 / 2.4 | 0.6 / 1.6 |
| states/attempt p50 / p99 | 21 / 191 | 17 / 394 |
| symmetric % | 1.60 | 6.40 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.203 | 10.613 | 90.5 (7) | 9.9e-17 | 0.162 | 6.0e-12 |
| color changes | 9.483 | 8.362 | 290.0 (7) | 8.1e-59 | 0.288 | 7.5e-37 |
| segments | 13.483 | 13.898 | 49.0 (6) | 7.4e-9 | 0.135 | 2.0e-8 |
| random stuck rate | 0.003 | 0.001 |  |  | 0.053 | 0.117 |
| random capped rate | 0.010 | 0.013 |  |  | 0.026 | 0.884 |
| states_expanded | 30.048 | 42.892 |  |  | 0.180 | 1.2e-14 |

Canonical-hash overlap: 0 puzzles of A's 999 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 99.90 %, B 100.00 %.

- opt_moves histogram (value:A/B): `4:0/1 5:0/1 6:4/3 7:9/19 8:32/51 9:85/145 10:187/218 11:241/282 12:235/192 13:146/78 14:59/10 15:2/0`
- color changes histogram (value:A/B): `2:0/1 4:1/5 5:4/20 6:14/63 7:60/168 8:153/263 9:264/263 10:259/172 11:175/45 12:70/0`
- segments histogram (value:A/B): `8:1/2 9:4/0 10:14/7 11:60/29 12:153/108 13:264/215 14:259/282 15:175/258 16:70/99`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.078`, B `0 / 0 / 0 / 0.016`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.016 / 0.156`, B `0 / 0 / 0.031 / 0.234`
- states_expanded p10 / p50 / p90 / p99: A `11 / 21 / 48 / 191`, B `9 / 17 / 100 / 394`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.01 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.00 | 0.70 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.92 / 18 / 21 | 17.12 / 17 / 20 |
| solve ms p50 / p99 | 0.62 / 5.6 | 0.78 / 6.1 |
| gen ms p50 / p99 | 1.8 / 6.9 | 2.0 / 8.1 |
| states/attempt p50 / p99 | 95 / 2492 | 203 / 2985 |
| symmetric % | 0.80 | 3.50 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.923 | 17.124 | 141.9 (8) | 9.4e-27 | 0.204 | 1.1e-18 |
| color changes | 15.674 | 14.171 | 468.9 (7) | 3.9e-97 | 0.401 | 4.8e-71 |
| segments | 21.674 | 21.854 | 10.5 (6) | 0.104 | 0.056 | 0.084 |
| random stuck rate | 0.059 | 0.029 |  |  | 0.453 | 1.5e-90 |
| random capped rate | 0.186 | 0.197 |  |  | 0.092 | 3.8e-4 |
| states_expanded | 377.645 | 439.099 |  |  | 0.171 | 2.9e-13 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `13:8/14 14:14/33 15:31/103 16:102/171 17:221/259 18:247/234 19:242/139 20:111/44 21:23/3 22:1/0`
- color changes histogram (value:A/B): `9:0/1 10:0/8 11:7/27 12:15/91 13:46/170 14:133/268 15:218/255 16:272/155 17:227/24 18:82/1`
- segments histogram (value:A/B): `17:7/2 18:15/10 19:46/42 20:133/101 21:218/208 22:272/290 23:227/254 24:82/93`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.031 / 0.156 / 0.266`, B `0 / 0 / 0.062 / 0.578`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.141 / 0.438 / 0.656`, B `0 / 0.109 / 0.562 / 0.844`
- states_expanded p10 / p50 / p90 / p99: A `35 / 95 / 1060 / 2492`, B `23 / 204 / 1157 / 2985`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.03 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.00 | 2.82 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 24.59 / 25 / 28 | 23.70 / 24 / 27 |
| solve ms p50 / p99 | 5.20 / 41.3 | 4.29 / 46.0 |
| gen ms p50 / p99 | 7.5 / 43.8 | 6.7 / 48.7 |
| states/attempt p50 / p99 | 1798 / 15651 | 1406 / 19812 |
| symmetric % | 0.40 | 1.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 24.587 | 23.698 | 149.0 (9) | 1.4e-27 | 0.220 | 1.1e-21 |
| color changes | 21.749 | 20.072 | 535.3 (8) | 1.9e-110 | 0.428 | 7.1e-81 |
| segments | 29.749 | 29.855 | 7.5 (6) | 0.279 | 0.043 | 0.307 |
| random stuck rate | 0.191 | 0.084 |  |  | 0.538 | 1.5e-127 |
| random capped rate | 0.465 | 0.517 |  |  | 0.177 | 3.5e-14 |
| states_expanded | 2848.955 | 3001.856 |  |  | 0.100 | 8.1e-5 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `17:0/1 18:0/1 19:1/10 20:9/22 21:21/52 22:64/146 23:143/190 24:230/266 25:239/167 26:178/117 27:93/26 28:22/2`
- color changes histogram (value:A/B): `14:0/1 15:0/1 16:0/17 17:4/27 18:15/97 19:44/187 20:130/240 21:215/266 22:262/145 23:224/19 24:106/0`
- segments histogram (value:A/B): `24:0/2 25:4/3 26:15/20 27:44/32 28:130/112 29:215/203 30:262/255 31:224/261 32:106/112`
- random stuck rate p10 / p50 / p90 / p99: A `0.016 / 0.188 / 0.359 / 0.562`, B `0 / 0 / 0.297 / 0.797`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.750 / 0.906`, B `0.094 / 0.547 / 0.875 / 0.984`
- states_expanded p10 / p50 / p90 / p99: A `98 / 1798 / 6573 / 15651`, B `134 / 1495 / 8047 / 19812`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.05 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.10 | 5.03 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 31.26 / 31 / 35 | 30.31 / 30 / 34 |
| solve ms p50 / p99 | 24.81 / 229.3 | 18.17 / 221.3 |
| gen ms p50 / p99 | 28.5 / 234.5 | 23.8 / 225.2 |
| states/attempt p50 / p99 | 7979 / 71744 | 5321 / 70404 |
| symmetric % | 0.20 | 0.70 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 31.261 | 30.314 | 145.2 (9) | 8.8e-27 | 0.219 | 1.7e-21 |
| color changes | 27.722 | 25.990 | 576.2 (8) | 3.1e-119 | 0.460 | 2.4e-93 |
| segments | 37.722 | 37.806 | 3.9 (7) | 0.788 | 0.024 | 0.933 |
| random stuck rate | 0.336 | 0.195 |  |  | 0.438 | 1.1e-84 |
| random capped rate | 0.564 | 0.662 |  |  | 0.287 | 1.3e-36 |
| states_expanded | 13012.148 | 12104.885 |  |  | 0.161 | 8.3e-12 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `24:1/0 25:3/4 26:5/20 27:17/40 28:41/89 29:79/158 30:166/204 31:205/221 32:238/168 33:164/75 34:65/17 35:15/4 36:1/0`
- color changes histogram (value:A/B): `20:0/1 21:2/6 22:3/7 23:6/42 24:18/109 25:46/157 26:116/282 27:202/249 28:278/135 29:230/12 30:99/0`
- segments histogram (value:A/B): `31:2/0 32:3/1 33:6/6 34:18/12 35:46/45 36:116/120 37:202/185 38:278/280 39:230/252 40:99/99`
- random stuck rate p10 / p50 / p90 / p99: A `0.094 / 0.328 / 0.578 / 0.781`, B `0 / 0.078 / 0.594 / 0.938`
- random capped rate p10 / p50 / p90 / p99: A `0.281 / 0.578 / 0.828 / 0.953`, B `0.234 / 0.734 / 0.969 / 1`
- states_expanded p10 / p50 / p90 / p99: A `1521 / 7979 / 30303 / 71744`, B `752 / 5856 / 32522 / 70404`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 2.88 / 11 | 4.29 / 17 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 65.28 | 76.71 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 12.07 / 12 / 15 | 11.53 / 12 / 14 |
| solve ms p50 / p99 | 0.04 / 0.3 | 0.01 / 0.3 |
| gen ms p50 / p99 | 0.4 / 1.1 | 0.5 / 1.4 |
| states/attempt p50 / p99 | 28 / 89 | 12 / 66 |
| symmetric % | 1.70 | 2.20 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 12.070 | 11.531 | 81.3 (8) | 2.7e-14 | 0.156 | 4.1e-11 |
| color changes | 10.430 | 9.699 | 222.5 (6) | 3.0e-45 | 0.261 | 2.4e-30 |
| segments | 16.430 | 16.553 | 9.2 (5) | 0.102 | 0.048 | 0.194 |
| random stuck rate | 0.580 | 0.336 |  |  | 0.430 | 1.3e-81 |
| random capped rate | 0.211 | 0.312 |  |  | 0.185 | 1.9e-15 |
| states_expanded | 29.340 | 21.803 |  |  | 0.263 | 8.4e-31 |

Canonical-hash overlap: 3 puzzles of A's 994 distinct also occur in B's 1000 distinct (0.30 % of 1000). Distinct fraction within each sample: A 99.40 %, B 100.00 %.

- opt_moves histogram (value:A/B): `7:3/7 8:7/14 9:37/52 10:90/155 11:173/233 12:277/282 13:268/195 14:126/60 15:19/2`
- color changes histogram (value:A/B): `6:3/13 7:17/33 8:54/97 9:117/244 10:283/348 11:334/242 12:192/23`
- segments histogram (value:A/B): `12:3/1 13:17/14 14:54/35 15:117/124 16:283/252 17:334/355 18:192/219`
- random stuck rate p10 / p50 / p90 / p99: A `0.203 / 0.625 / 0.891 / 0.984`, B `0 / 0.188 / 0.875 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.062 / 0.625 / 0.891`, B `0 / 0.016 / 0.922 / 0.984`
- states_expanded p10 / p50 / p90 / p99: A `12 / 28 / 49 / 72`, B `11 / 19 / 37 / 54`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 12.49 / 57 | 24.48 / 106 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 92.00 | 95.92 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.64 / 19 / 21 | 18.09 / 18 / 21 |
| solve ms p50 / p99 | 0.07 / 1.9 | 0.01 / 1.5 |
| gen ms p50 / p99 | 2.6 / 7.8 | 2.6 / 6.7 |
| states/attempt p50 / p99 | 58 / 239 | 10 / 155 |
| symmetric % | 0.60 | 1.60 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.637 | 18.093 | 78.4 (8) | 1.0e-13 | 0.167 | 1.1e-12 |
| color changes | 16.406 | 15.632 | 230.3 (6) | 6.5e-47 | 0.269 | 3.3e-32 |
| segments | 25.406 | 25.549 | 10.5 (5) | 0.061 | 0.054 | 0.105 |
| random stuck rate | 0.766 | 0.539 |  |  | 0.333 | 4.0e-49 |
| random capped rate | 0.174 | 0.332 |  |  | 0.245 | 8.7e-27 |
| states_expanded | 85.635 | 61.791 |  |  | 0.330 | 3.0e-48 |

Canonical-hash overlap: 1 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.10 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `12:1/0 13:0/1 14:6/7 15:19/38 16:57/85 17:127/201 18:213/258 19:289/246 20:197/132 21:83/27 22:7/5 23:1/0`
- color changes histogram (value:A/B): `11:1/2 12:2/8 13:13/40 14:47/117 15:146/243 16:268/336 17:348/237 18:175/17`
- segments histogram (value:A/B): `20:1/0 21:2/2 22:13/8 23:47/43 24:146/129 25:268/251 26:348/338 27:175/229`
- random stuck rate p10 / p50 / p90 / p99: A `0.531 / 0.797 / 0.984 / 1`, B `0 / 0.641 / 0.984 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.438 / 0.906`, B `0 / 0.172 / 0.922 / 1`
- states_expanded p10 / p50 / p90 / p99: A `28 / 78 / 142 / 240`, B `23 / 53 / 107 / 200`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 7.45 / 33 | 8.84 / 39 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 86.58 | 88.69 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.54 / 18 / 21 | 16.84 / 17 / 20 |
| solve ms p50 / p99 | 0.04 / 0.3 | 0.02 / 0.4 |
| gen ms p50 / p99 | 0.8 / 2.2 | 1.2 / 3.7 |
| states/attempt p50 / p99 | 39 / 153 | 20 / 138 |
| symmetric % | 0.30 | 0.30 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.536 | 16.836 | 93.6 (9) | 3.1e-16 | 0.178 | 2.4e-14 |
| color changes | 15.625 | 14.811 | 174.4 (7) | 3.0e-34 | 0.235 | 1.1e-24 |
| segments | 21.625 | 21.779 | 10.4 (7) | 0.167 | 0.037 | 0.493 |
| random stuck rate | 0.590 | 0.239 |  |  | 0.581 | 1.1e-148 |
| random capped rate | 0.307 | 0.571 |  |  | 0.414 | 1.1e-75 |
| states_expanded | 50.071 | 45.660 |  |  | 0.125 | 2.7e-7 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `11:1/1 12:1/12 13:13/23 14:31/53 15:61/108 16:144/189 17:216/259 18:240/193 19:187/123 20:82/37 21:23/2 23:1/0`
- color changes histogram (value:A/B): `9:1/1 10:1/5 11:3/13 12:20/41 13:51/111 14:131/210 15:220/281 16:273/230 17:232/105 18:68/3`
- segments histogram (value:A/B): `15:1/0 16:1/1 17:3/5 18:20/14 19:51/41 20:131/114 21:220/215 22:273/280 23:232/228 24:68/102`
- random stuck rate p10 / p50 / p90 / p99: A `0.219 / 0.625 / 0.891 / 1`, B `0 / 0.078 / 0.781 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.281 / 0.672 / 0.984`, B `0 / 0.688 / 0.969 / 1`
- states_expanded p10 / p50 / p90 / p99: A `19 / 48 / 82 / 131`, B `17 / 41 / 81 / 135`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 41.17 / 176 | 46.73 / 223 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 97.57 | 97.86 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.86 / 24 / 28 | 23.27 / 23 / 27 |
| solve ms p50 / p99 | 0.07 / 1.0 | 0.03 / 1.0 |
| gen ms p50 / p99 | 4.3 / 18.7 | 4.2 / 15.9 |
| states/attempt p50 / p99 | 59 / 284 | 22 / 243 |
| symmetric % | 0.00 | 0.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.856 | 23.271 | 67.5 (10) | 1.3e-10 | 0.140 | 4.9e-9 |
| color changes | 21.539 | 20.804 | 163.8 (8) | 2.6e-31 | 0.187 | 8.8e-16 |
| segments | 29.539 | 29.785 | 19.6 (7) | 0.007 | 0.090 | 5.5e-4 |
| random stuck rate | 0.709 | 0.399 |  |  | 0.462 | 3.7e-94 |
| random capped rate | 0.252 | 0.527 |  |  | 0.408 | 1.6e-73 |
| states_expanded | 111.069 | 99.321 |  |  | 0.120 | 9.5e-7 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `15:1/0 16:1/0 17:1/2 18:2/4 19:10/14 20:31/45 21:53/81 22:114/167 23:191/217 24:211/225 25:208/157 26:108/71 27:58/14 28:9/3 29:2/0`
- color changes histogram (value:A/B): `13:1/0 15:2/1 16:3/6 17:6/17 18:22/56 19:58/97 20:133/193 21:240/269 22:246/259 23:200/102 24:89/0`
- segments histogram (value:A/B): `21:1/0 23:2/0 24:3/1 25:6/6 26:22/18 27:58/55 28:133/101 29:240/194 30:246/270 31:200/256 32:89/99`
- random stuck rate p10 / p50 / p90 / p99: A `0.438 / 0.734 / 0.984 / 1`, B `0 / 0.328 / 0.938 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.219 / 0.547 / 0.953`, B `0.016 / 0.547 / 0.969 / 1`
- states_expanded p10 / p50 / p90 / p99: A `36 / 100 / 191 / 347`, B `33 / 87 / 179 / 298`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.01 / 2 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 0.00 | 1.19 |
| timeout % | 0.00 | 0.00 |
| solved or below min_opt % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 27.33 / 27 / 31 | 26.50 / 27 / 30 |
| solve ms p50 / p99 | 4.52 / 47.7 | 6.87 / 97.7 |
| gen ms p50 / p99 | 7.0 / 54.1 | 9.6 / 100.2 |
| states/attempt p50 / p99 | 1769 / 19281 | 2446 / 37064 |
| symmetric % | 0.00 | 1.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 27.330 | 26.498 | 116.0 (9) | 8.8e-21 | 0.175 | 7.1e-14 |
| color changes | 24.703 | 23.145 | 390.7 (9) | 1.3e-78 | 0.375 | 3.5e-62 |
| segments | 31.703 | 31.970 | 27.3 (8) | 6.3e-4 | 0.099 | 9.9e-5 |
| random stuck rate | 0.119 | 0.041 |  |  | 0.572 | 4.1e-144 |
| random capped rate | 0.496 | 0.496 |  |  | 0.091 | 4.6e-4 |
| states_expanded | 2762.470 | 5602.835 |  |  | 0.187 | 8.8e-16 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `20:1/0 21:1/6 22:3/13 23:17/34 24:39/79 25:92/130 26:156/214 27:217/225 28:203/182 29:162/91 30:81/22 31:27/4 32:1/0`
- color changes histogram (value:A/B): `17:0/1 18:1/5 19:2/14 20:6/50 21:26/87 22:62/154 23:127/228 24:211/271 25:239/137 26:177/47 27:117/6 28:32/0`
- segments histogram (value:A/B): `25:1/0 26:2/1 27:6/6 28:26/19 29:62/55 30:127/94 31:211/161 32:239/256 33:177/252 34:117/124 35:32/32`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.266 / 0.375`, B `0 / 0 / 0.109 / 0.641`
- random capped rate p10 / p50 / p90 / p99: A `0.188 / 0.516 / 0.766 / 0.922`, B `0.141 / 0.484 / 0.844 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `96 / 1769 / 6337 / 19281`, B `132 / 2504 / 15031 / 37064`

