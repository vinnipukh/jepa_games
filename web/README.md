# Water Sort web game

The browser game: the Rust core and both generators compiled to WASM (`water_sort_web`,
wasm-bindgen) with a small Vite + TypeScript front end in `app/`. All rules, move counting,
stars and generation run in wasm; TypeScript only renders and forwards clicks. The same seed or
puzzle code gives the same puzzle as in Rust and Python (golden vectors checked on wasm32, D20).

## Build and run

```bash
scripts/setup-cloud.sh --web       # wasm32 target, the locked wasm-bindgen-cli, npm packages
web/build.sh                       # cargo build (wasm32, release) + wasm-bindgen -> web/pkg
cd web/app
npm run dev                        # http://localhost:5173
npm run build                      # typecheck + static site in web/app/dist
npm run preview                    # serve dist
```

Rebuild `web/pkg` (`web/build.sh`) after every Rust change.

## Tests

```bash
cargo test --release -p water_sort_web --target wasm32-unknown-unknown   # Node, wasm-bindgen-test
```

`web/tests/golden.rs` replays the core, generator and rollout golden files; `web/tests/api.rs`
covers undo/restart counting, stars, seeds, codes and the trajectory export. The manual UI
checklist is in `docs/phases/phase-6-web/PLAN.md`.

## URLs

- `?code=<puzzle code>`
- `?gen=uniform&seed=<hex>&c=6&k=4&e=2` plus `&layout=distributed`, `&tier=easy|medium|hard`
- `?gen=turan&seed=<hex>&c=6&k=4&e=2&strategy=scramble&steps=40` (`reverse_search`,
  `scramble`, `constrained`, `pour_walk` (distributed only))

## Hosting (not active yet)

`.github/workflows/pages.yml` builds the static site and can deploy it to GitHub Pages. It only
runs when started by hand (Actions → "Web game (Pages)" → Run workflow):

- without "deploy", it only builds; download the `web-game` artifact (14 days) and serve it
  from any static host;
- with "deploy", it also publishes to Pages. Enable Pages first (Settings → Pages → Source:
  GitHub Actions); on a private repo this may need a paid plan.

The site is plain static files with relative paths (`web/app/dist` after `npm run build`), so it
works from any sub-path such as `https://<user>.github.io/jepa_games/`. It needs an HTTP server:
browsers refuse module workers and wasm from `file://`, so opening `index.html` from disk does not
work (`npm run preview` or `python -m http.server` in `dist` does).

## Trajectory export

"Export trajectory" downloads the play session (every pour, illegal pour, undo and restart) as
JSON in the Phase 5.3 row format with `source = "human"`. Nothing is uploaded. Import into
Parquet with `python -m jepa_water_sort.logger import-human --out DIR export.json ...`.
