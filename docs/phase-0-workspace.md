# Phase 0 — Workspace setup

## Goal

An empty but fully wired workspace: all crates exist, dependencies are pinned, CI runs format / lint / test on Linux and Windows and is green.

## Prerequisites (local machine)

The current machine has no Rust toolchain installed. Needed:

- `rustup` with the stable toolchain, plus components `rustfmt` and `clippy`.
- `rustup target add wasm32-unknown-unknown` (used from Phase 6, cheap to add now).
- Python ≥ 3.10 via `uv` (already present) for Phase 5. Use a 3.12 venv; 3.14 support depends on the PyO3 version.

## Design

```
jepa_games/
├── Cargo.toml              # [workspace], [workspace.dependencies], [workspace.lints]
├── Cargo.lock              # committed: reproducibility depends on exact crate versions
├── rust-toolchain.toml     # pinned stable version + components + wasm32 target
├── .github/workflows/ci.yml
├── water_sort_core/        # lib
├── uniform_water_sort/     # lib
├── turan_water_sort/       # lib
├── water_sort_cli/         # bin
├── python/                 # added to the workspace in Phase 5 (cdylib, maturin)
└── web/                    # added to the workspace in Phase 6 (cdylib, wasm-bindgen)
```

- `[workspace.dependencies]`: `rand_core`, `rand_chacha`, `getrandom`, `rayon`, `serde` (derive), `serde_json`, `xxhash-rust` (xxh3), `thiserror`, `clap` (CLI), `proptest` (dev). `rand` is used only where convenient outside generation (see D10).
- `[workspace.lints.rust] unsafe_code = "forbid"` workspace-wide, which is stricter than the roadmap. The only planned exception is the optional RDSEED source in `turan_water_sort` (D1), which would override it to `deny` locally with a justified `#[allow]`. `water_sort_core` also gets `#![forbid(unsafe_code)]` explicitly, as the roadmap asks.
- `[workspace.lints.clippy]`: `pedantic` at warn level, with a short allow-list for noisy lints (`module_name_repetitions`, `must_use_candidate`).
- Release profile for the solver: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`.

## CI (`.github/workflows/ci.yml`)

```yaml
on: [push, pull_request]
jobs:
  rust:
    strategy:
      matrix: { os: [ubuntu-latest, windows-latest] }
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable   # reads rust-toolchain.toml
        with: { components: "rustfmt, clippy" }
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all --check
      - run: cargo clippy --workspace --all-targets -- -D warnings
      - run: cargo test --workspace
  # added later:
  # heavy-tests (Phase 1/2): cargo test --release --workspace -- --ignored   (ubuntu only)
  # wasm (Phase 6):   cargo build --target wasm32-unknown-unknown -p web; wasm-bindgen-test
  # python (Phase 5): uv + maturin develop + pytest
```

`fmt` and `clippy` run once per OS: cheap, and Windows catches path and line-ending issues.

## Tasks

- [x] `git init`, `.gitignore`
- [x] Install toolchain; add `rust-toolchain.toml` (pinned 1.99.0)
- [x] Workspace `Cargo.toml` with members, shared dependencies, lints, profiles
- [x] Four crates with `cargo new --lib` / `--bin`, each with one trivial test
- [x] `#![forbid(unsafe_code)]` in `water_sort_core/src/lib.rs`
- [x] `.github/workflows/ci.yml`
- [x] `.gitattributes` (`* text=auto eol=lf`) so `cargo fmt --check` behaves the same on Windows
- [ ] Push to GitHub, confirm CI is green on both OSes

## Acceptance

`cargo test --workspace` passes locally and CI is green on `ubuntu-latest` and `windows-latest`.

## Risks / open points

- Repository: `jepa_games` on GitHub.
