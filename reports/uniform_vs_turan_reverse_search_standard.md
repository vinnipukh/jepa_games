# `uniform` vs `turan_reverse_search_standard`

- A: `uniform` / `fisher_yates`
- B: `turan` / `reverse_search(max_depth=300,max_states=10000,layout=standard)`

Produced by `water_sort_cli compare --a uniform:standard --b turan:search:10000:depth=300:standard --configs 4x4x2,6x4x2,8x4x2,10x4x2,6x3x1,9x3x1,6x4x1,8x4x1,7x5x2 --samples 1000 --max-states 5000000 --max-attempts 10000 --min-opt 1 --rollouts 64 --base-seed 20261002` (release build, 3 threads). Both sides use the seeds `splitmix64(base_seed ^ i)`; every number except the `ms` columns is deterministic.

Per configuration: the stats measurements side by side (rates per attempt; construction rejections are Turan attempts rejected before solving, D15), then per-puzzle metric means with two-sample tests over the accepted puzzles. chi² is Pearson's two-sample test on the histogram, with adjacent values merged until every bin holds at least 10 puzzles; KS is the two-sample Kolmogorov-Smirnov test (asymptotic p-value; conservative for discrete metrics). The random rates are per puzzle over its random legal-move rollouts (stuck: no legal move left; capped: hit `4 × opt_moves` moves). Overlap counts canonical hashes (puzzles equal up to tube order and color names) present in both samples.

## 4 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.30 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 11.20 / 11 / 14 | 11.36 / 11 / 14 |
| solve ms p50 / p99 | 0.14 / 0.5 | 0.14 / 0.5 |
| gen ms p50 / p99 | 0.6 / 1.2 | 15.5 / 30.3 |
| states/attempt p50 / p99 | 21 / 191 | 24 / 185 |
| symmetric % | 1.60 | 5.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 11.203 | 11.359 | 147.0 (7) | 1.8e-28 | 0.121 | 7.4e-7 |
| color changes | 9.483 | 9.758 | 239.8 (6) | 6.1e-49 | 0.179 | 1.7e-14 |
| segments | 13.483 | 13.758 | 239.8 (6) | 6.1e-49 | 0.179 | 1.7e-14 |
| random stuck rate | 0.003 | 0.004 |  |  | 0.026 | 0.884 |
| random capped rate | 0.010 | 0.003 |  |  | 0.078 | 0.004 |
| states_expanded | 30.048 | 30.345 |  |  | 0.152 | 1.4e-10 |

Canonical-hash overlap: 7 puzzles of A's 999 distinct also occur in B's 805 distinct (0.70 % of 1000). Distinct fraction within each sample: A 99.90 %, B 80.50 %.

- opt_moves histogram (value:A/B): `6:4/0 7:9/0 8:32/0 9:85/33 10:187/163 11:241/344 12:235/346 13:146/100 14:59/14 15:2/0`
- color changes histogram (value:A/B): `4:1/0 5:4/0 6:14/0 7:60/1 8:153/52 9:264/304 10:259/488 11:175/141 12:70/14`
- segments histogram (value:A/B): `8:1/0 9:4/0 10:14/0 11:60/1 12:153/52 13:264/304 14:259/488 15:175/141 16:70/14`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0 / 0 / 0.078`, B `0 / 0 / 0 / 0.078`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0 / 0.016 / 0.156`, B `0 / 0 / 0.016 / 0.062`
- states_expanded p10 / p50 / p90 / p99: A `11 / 21 / 48 / 191`, B `11 / 24 / 48 / 185`

## 6 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.01 / 2 |
| construction rejections % | 0.00 | 1.38 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.92 / 18 / 21 | 15.33 / 15 / 18 |
| solve ms p50 / p99 | 0.66 / 4.8 | 0.58 / 4.6 |
| gen ms p50 / p99 | 1.9 / 6.3 | 18.6 / 37.6 |
| states/attempt p50 / p99 | 95 / 2492 | 67 / 1785 |
| symmetric % | 0.80 | 2.90 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.923 | 15.331 | 957.4 (9) | 2.6e-200 | 0.635 | 1.7e-177 |
| color changes | 15.674 | 13.336 | 934.9 (7) | 1.4e-197 | 0.624 | 2.0e-171 |
| segments | 21.674 | 19.336 | 934.9 (7) | 1.4e-197 | 0.624 | 2.0e-171 |
| random stuck rate | 0.059 | 0.071 |  |  | 0.109 | 1.2e-5 |
| random capped rate | 0.186 | 0.061 |  |  | 0.381 | 3.6e-64 |
| states_expanded | 377.645 | 208.701 |  |  | 0.254 | 9.3e-29 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 998 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.80 %.

- opt_moves histogram (value:A/B): `11:0/3 12:0/10 13:8/78 14:14/187 15:31/293 16:102/219 17:221/143 18:247/57 19:242/10 20:111/0 21:23/0 22:1/0`
- color changes histogram (value:A/B): `9:0/1 10:0/5 11:7/43 12:15/201 13:46/327 14:133/248 15:218/134 16:272/38 17:227/3 18:82/0`
- segments histogram (value:A/B): `15:0/1 16:0/5 17:7/43 18:15/201 19:46/327 20:133/248 21:218/134 22:272/38 23:227/3 24:82/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.031 / 0.156 / 0.266`, B `0 / 0.047 / 0.172 / 0.266`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.141 / 0.438 / 0.656`, B `0 / 0.016 / 0.188 / 0.391`
- states_expanded p10 / p50 / p90 / p99: A `35 / 95 / 1060 / 2492`, B `27 / 68 / 677 / 1785`

## 8 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.03 / 2 |
| construction rejections % | 0.00 | 2.72 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 24.59 / 25 / 28 | 19.06 / 19 / 23 |
| solve ms p50 / p99 | 5.50 / 43.1 | 2.05 / 23.3 |
| gen ms p50 / p99 | 7.7 / 45.1 | 22.8 / 62.3 |
| states/attempt p50 / p99 | 1798 / 15651 | 151 / 8360 |
| symmetric % | 0.40 | 2.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 24.587 | 19.063 | 1702.4 (13) | < 1e-300 | 0.897 | < 1e-300 |
| color changes | 21.749 | 16.818 | 1685.7 (10) | < 1e-300 | 0.900 | < 1e-300 |
| segments | 29.749 | 24.818 | 1685.7 (10) | < 1e-300 | 0.900 | < 1e-300 |
| random stuck rate | 0.191 | 0.216 |  |  | 0.100 | 8.1e-5 |
| random capped rate | 0.465 | 0.147 |  |  | 0.590 | 2.7e-153 |
| states_expanded | 2848.955 | 999.153 |  |  | 0.459 | 6.0e-93 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `15:0/12 16:0/42 17:0/109 18:0/206 19:1/259 20:9/184 21:21/116 22:64/53 23:143/11 24:230/6 25:239/2 26:178/0 27:93/0 28:22/0`
- color changes histogram (value:A/B): `12:0/1 13:0/8 14:0/42 15:0/128 16:0/244 17:4/267 18:15/186 19:44/87 20:130/29 21:215/6 22:262/2 23:224/0 24:106/0`
- segments histogram (value:A/B): `20:0/1 21:0/8 22:0/42 23:0/128 24:0/244 25:4/267 26:15/186 27:44/87 28:130/29 29:215/6 30:262/2 31:224/0 32:106/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.016 / 0.188 / 0.359 / 0.562`, B `0.047 / 0.203 / 0.391 / 0.500`
- random capped rate p10 / p50 / p90 / p99: A `0.141 / 0.484 / 0.750 / 0.906`, B `0 / 0.094 / 0.406 / 0.656`
- states_expanded p10 / p50 / p90 / p99: A `98 / 1798 / 6573 / 15651`, B `61 / 155 / 2959 / 8360`

## 10 colors, capacity 4, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.04 / 2 |
| construction rejections % | 0.00 | 4.31 |
| unsolvable % | 0.10 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 31.26 / 31 / 35 | 22.57 / 23 / 27 |
| solve ms p50 / p99 | 24.31 / 213.1 | 7.44 / 83.1 |
| gen ms p50 / p99 | 27.6 / 216.9 | 33.0 / 105.9 |
| states/attempt p50 / p99 | 7979 / 71744 | 392 / 23223 |
| symmetric % | 0.20 | 2.00 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 31.261 | 22.570 | 1925.0 (16) | < 1e-300 | 0.976 | < 1e-300 |
| color changes | 27.722 | 20.116 | 1908.6 (13) | < 1e-300 | 0.968 | < 1e-300 |
| segments | 37.722 | 30.116 | 1908.6 (13) | < 1e-300 | 0.968 | < 1e-300 |
| random stuck rate | 0.336 | 0.388 |  |  | 0.149 | 3.6e-10 |
| random capped rate | 0.564 | 0.185 |  |  | 0.664 | 4.8e-194 |
| states_expanded | 13012.148 | 3669.137 |  |  | 0.469 | 5.0e-97 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `17:0/1 18:0/2 19:0/32 20:0/89 21:0/170 22:0/196 23:0/215 24:1/156 25:3/75 26:5/49 27:17/11 28:41/4 29:79/0 30:166/0 31:205/0 32:238/0 33:164/0 34:65/0 35:15/0 36:1/0`
- color changes histogram (value:A/B): `16:0/7 17:0/44 18:0/116 19:0/205 20:0/233 21:2/193 22:3/111 23:6/70 24:18/17 25:46/4 26:116/0 27:202/0 28:278/0 29:230/0 30:99/0`
- segments histogram (value:A/B): `26:0/7 27:0/44 28:0/116 29:0/205 30:0/233 31:2/193 32:3/111 33:6/70 34:18/17 35:46/4 36:116/0 37:202/0 38:278/0 39:230/0 40:99/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.094 / 0.328 / 0.578 / 0.781`, B `0.188 / 0.375 / 0.609 / 0.750`
- random capped rate p10 / p50 / p90 / p99: A `0.281 / 0.578 / 0.828 / 0.953`, B `0 / 0.141 / 0.453 / 0.688`
- states_expanded p10 / p50 / p90 / p99: A `1521 / 7979 / 30303 / 71744`, B `114 / 486 / 10600 / 23223`

## 6 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 2.88 / 11 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 65.28 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 12.07 / 12 / 15 | 12.36 / 12 / 14 |
| solve ms p50 / p99 | 0.04 / 0.4 | 0.25 / 0.5 |
| gen ms p50 / p99 | 0.5 / 1.5 | 10.7 / 19.7 |
| states/attempt p50 / p99 | 28 / 89 | 39 / 116 |
| symmetric % | 1.70 | 25.90 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 12.070 | 12.364 | 180.8 (7) | 1.3e-35 | 0.148 | 4.8e-10 |
| color changes | 10.430 | 10.517 | 292.5 (5) | 4.2e-61 | 0.175 | 7.1e-14 |
| segments | 16.430 | 16.517 | 292.5 (5) | 4.2e-61 | 0.175 | 7.1e-14 |
| random stuck rate | 0.580 | 0.472 |  |  | 0.257 | 2.0e-29 |
| random capped rate | 0.211 | 0.220 |  |  | 0.068 | 0.019 |
| states_expanded | 29.340 | 39.950 |  |  | 0.339 | 6.8e-51 |

Canonical-hash overlap: 4 puzzles of A's 994 distinct also occur in B's 303 distinct (0.40 % of 1000). Distinct fraction within each sample: A 99.40 %, B 30.30 %.

- opt_moves histogram (value:A/B): `7:3/0 8:7/0 9:37/0 10:90/3 11:173/159 12:277/404 13:268/341 14:126/91 15:19/2`
- color changes histogram (value:A/B): `6:3/0 7:17/0 8:54/0 9:117/16 10:283/512 11:334/411 12:192/61`
- segments histogram (value:A/B): `12:3/0 13:17/0 14:54/0 15:117/16 16:283/512 17:334/411 18:192/61`
- random stuck rate p10 / p50 / p90 / p99: A `0.203 / 0.625 / 0.891 / 0.984`, B `0.125 / 0.516 / 0.703 / 0.906`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.062 / 0.625 / 0.891`, B `0 / 0.062 / 0.719 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `12 / 28 / 49 / 72`, B `13 / 39 / 63 / 116`

## 9 colors, capacity 3, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 12.49 / 57 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 92.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 18.64 / 19 / 21 | 16.29 / 16 / 19 |
| solve ms p50 / p99 | 0.07 / 1.8 | 1.60 / 3.9 |
| gen ms p50 / p99 | 2.6 / 7.3 | 14.0 / 27.4 |
| states/attempt p50 / p99 | 58 / 239 | 74 / 266 |
| symmetric % | 0.60 | 26.40 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 18.637 | 16.293 | 930.9 (7) | 1.0e-196 | 0.631 | 2.8e-175 |
| color changes | 16.406 | 14.369 | 1004.5 (5) | 6.4e-215 | 0.672 | 9.6e-199 |
| segments | 25.406 | 23.369 | 1004.5 (5) | 6.4e-215 | 0.672 | 9.6e-199 |
| random stuck rate | 0.766 | 0.708 |  |  | 0.234 | 1.8e-24 |
| random capped rate | 0.174 | 0.118 |  |  | 0.174 | 1.0e-13 |
| states_expanded | 85.635 | 83.760 |  |  | 0.091 | 4.6e-4 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 858 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 85.80 %.

- opt_moves histogram (value:A/B): `12:1/0 14:6/54 15:19/217 16:57/313 17:127/257 18:213/125 19:289/25 20:197/9 21:83/0 22:7/0 23:1/0`
- color changes histogram (value:A/B): `11:1/0 12:2/0 13:13/198 14:47/384 15:146/299 16:268/91 17:348/26 18:175/2`
- segments histogram (value:A/B): `20:1/0 21:2/0 22:13/198 23:47/384 24:146/299 25:268/91 26:348/26 27:175/2`
- random stuck rate p10 / p50 / p90 / p99: A `0.531 / 0.797 / 0.984 / 1`, B `0.500 / 0.734 / 0.891 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.438 / 0.906`, B `0 / 0 / 0.359 / 0.766`
- states_expanded p10 / p50 / p90 / p99: A `28 / 78 / 142 / 240`, B `21 / 74 / 157 / 266`

## 6 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 7.45 / 33 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.10 |
| unsolvable % | 86.58 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 17.54 / 18 / 21 | 15.18 / 15 / 18 |
| solve ms p50 / p99 | 0.04 / 0.3 | 0.29 / 0.6 |
| gen ms p50 / p99 | 0.9 / 2.4 | 11.9 / 21.4 |
| states/attempt p50 / p99 | 39 / 153 | 52 / 128 |
| symmetric % | 0.30 | 7.90 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 17.536 | 15.176 | 1004.3 (8) | 1.8e-211 | 0.657 | 5.5e-190 |
| color changes | 15.625 | 13.565 | 978.8 (6) | 3.5e-208 | 0.658 | 1.4e-190 |
| segments | 21.625 | 19.565 | 978.8 (6) | 3.5e-208 | 0.658 | 1.4e-190 |
| random stuck rate | 0.590 | 0.536 |  |  | 0.135 | 2.0e-8 |
| random capped rate | 0.307 | 0.324 |  |  | 0.088 | 7.9e-4 |
| states_expanded | 50.071 | 52.682 |  |  | 0.080 | 0.003 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 914 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 91.40 %.

- opt_moves histogram (value:A/B): `11:1/0 12:1/2 13:13/14 14:31/235 15:61/408 16:144/249 17:216/81 18:240/10 19:187/1 20:82/0 21:23/0 23:1/0`
- color changes histogram (value:A/B): `9:1/0 10:1/2 11:3/0 12:20/83 13:51/417 14:131/363 15:220/115 16:273/19 17:232/1 18:68/0`
- segments histogram (value:A/B): `15:1/0 16:1/2 17:3/0 18:20/83 19:51/417 20:131/363 21:220/115 22:273/19 23:232/1 24:68/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.219 / 0.625 / 0.891 / 1`, B `0.188 / 0.562 / 0.844 / 0.984`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.281 / 0.672 / 0.984`, B `0 / 0.266 / 0.750 / 0.953`
- states_expanded p10 / p50 / p90 / p99: A `19 / 48 / 82 / 131`, B `17 / 52 / 89 / 128`

## 8 colors, capacity 4, 1 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 41.17 / 176 | 1.00 / 1 |
| construction rejections % | 0.00 | 0.00 |
| unsolvable % | 97.57 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 23.86 / 24 / 28 | 18.23 / 18 / 21 |
| solve ms p50 / p99 | 0.07 / 1.0 | 0.85 / 1.8 |
| gen ms p50 / p99 | 4.4 / 20.4 | 13.2 / 23.2 |
| states/attempt p50 / p99 | 59 / 284 | 84 / 255 |
| symmetric % | 0.00 | 8.30 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 23.856 | 18.233 | 1720.4 (12) | < 1e-300 | 0.905 | < 1e-300 |
| color changes | 21.539 | 16.558 | 1730.7 (10) | < 1e-300 | 0.909 | < 1e-300 |
| segments | 29.539 | 24.558 | 1730.7 (10) | < 1e-300 | 0.909 | < 1e-300 |
| random stuck rate | 0.709 | 0.684 |  |  | 0.094 | 2.6e-4 |
| random capped rate | 0.252 | 0.221 |  |  | 0.102 | 5.4e-5 |
| states_expanded | 111.069 | 88.315 |  |  | 0.159 | 1.6e-11 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 996 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 99.60 %.

- opt_moves histogram (value:A/B): `15:1/5 16:1/61 17:1/230 18:2/321 19:10/225 20:31/109 21:53/40 22:114/8 23:191/1 24:211/0 25:208/0 26:108/0 27:58/0 28:9/0 29:2/0`
- color changes histogram (value:A/B): `13:1/0 14:0/20 15:2/168 16:3/315 17:6/298 18:22/142 19:58/45 20:133/11 21:240/1 22:246/0 23:200/0 24:89/0`
- segments histogram (value:A/B): `21:1/0 22:0/20 23:2/168 24:3/315 25:6/298 26:22/142 27:58/45 28:133/11 29:240/1 30:246/0 31:200/0 32:89/0`
- random stuck rate p10 / p50 / p90 / p99: A `0.438 / 0.734 / 0.984 / 1`, B `0.391 / 0.719 / 0.922 / 1`
- random capped rate p10 / p50 / p90 / p99: A `0 / 0.219 / 0.547 / 0.953`, B `0 / 0.172 / 0.547 / 0.828`
- states_expanded p10 / p50 / p90 / p99: A `36 / 100 / 191 / 347`, B `24 / 84 / 150 / 255`

## 7 colors, capacity 5, 2 empty

| | A | B |
|---|---:|---:|
| samples | 1000 | 1000 |
| D3 criterion met | yes | yes |
| attempts mean / p99 | 1.00 / 1 | 1.16 / 3 |
| construction rejections % | 0.00 | 13.72 |
| unsolvable % | 0.00 | 0.00 |
| timeout % | 0.00 | 0.00 |
| solved or outside opt band % | 0.00 | 0.00 |
| opt mean / p50 / p99 | 27.33 / 27 / 31 | 19.52 / 19 / 24 |
| solve ms p50 / p99 | 4.84 / 47.9 | 1.37 / 28.3 |
| gen ms p50 / p99 | 7.1 / 50.9 | 22.4 / 66.8 |
| states/attempt p50 / p99 | 1769 / 19281 | 156 / 8743 |
| symmetric % | 0.00 | 1.10 |

| metric | A mean | B mean | chi² (df) | chi² p | KS D | KS p |
|---|---:|---:|---:|---:|---:|---:|
| opt_moves | 27.330 | 19.520 | 1892.9 (16) | < 1e-300 | 0.964 | < 1e-300 |
| color changes | 24.703 | 17.380 | 1888.1 (15) | < 1e-300 | 0.960 | < 1e-300 |
| segments | 31.703 | 24.380 | 1888.1 (15) | < 1e-300 | 0.960 | < 1e-300 |
| random stuck rate | 0.119 | 0.143 |  |  | 0.150 | 2.6e-10 |
| random capped rate | 0.496 | 0.119 |  |  | 0.689 | 6.6e-209 |
| states_expanded | 2762.470 | 975.474 |  |  | 0.505 | 2.0e-112 |

Canonical-hash overlap: 0 puzzles of A's 1000 distinct also occur in B's 1000 distinct (0.00 % of 1000). Distinct fraction within each sample: A 100.00 %, B 100.00 %.

- opt_moves histogram (value:A/B): `14:0/2 15:0/11 16:0/40 17:0/67 18:0/147 19:0/238 20:1/204 21:1/162 22:3/86 23:17/29 24:39/12 25:92/1 26:156/1 27:217/0 28:203/0 29:162/0 30:81/0 31:27/0 32:1/0`
- color changes histogram (value:A/B): `12:0/2 13:0/8 14:0/36 15:0/69 16:0/177 17:0/243 18:1/224 19:2/143 20:6/67 21:26/23 22:62/7 23:127/0 24:211/1 25:239/0 26:177/0 27:117/0 28:32/0`
- segments histogram (value:A/B): `19:0/2 20:0/8 21:0/36 22:0/69 23:0/177 24:0/243 25:1/224 26:2/143 27:6/67 28:26/23 29:62/7 30:127/0 31:211/1 32:239/0 33:177/0 34:117/0 35:32/0`
- random stuck rate p10 / p50 / p90 / p99: A `0 / 0.109 / 0.266 / 0.375`, B `0.031 / 0.141 / 0.266 / 0.391`
- random capped rate p10 / p50 / p90 / p99: A `0.188 / 0.516 / 0.766 / 0.922`, B `0 / 0.062 / 0.328 / 0.578`
- states_expanded p10 / p50 / p90 / p99: A `96 / 1769 / 6337 / 19281`, B `66 / 187 / 3476 / 8762`

