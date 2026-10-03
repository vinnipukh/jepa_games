//! Small committed fixture datasets (`tests/fixtures/datasets/*`).
//!
//! - They regenerate byte for byte from their options on every CI target (Linux and Windows), so
//!   the record files, including Parquet with zstd, do not depend on the platform.
//! - `validate` passes on the committed files, with every record regenerated from its seed.
//!
//! Regenerate with `WATER_SORT_BLESS=1 cargo test -p water_sort_cli --test dataset_fixture` and
//! review the diff: a change in the record files means the schema or a generator changed.

use std::path::{Path, PathBuf};

use turan_water_sort::{Turan, TuranStrategy};
use uniform_water_sort::Uniform;
use water_sort_cli::args::GenSpec;
use water_sort_cli::dataset::generate::{CHUNK, GenerateOptions, WINDOW_CHUNKS, generate};
use water_sort_cli::dataset::validate::validate;
use water_sort_cli::dataset::{Format, Manifest, SplitRanges};
use water_sort_core::{GenConfig, Layout, Params};

fn fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("datasets")
        .join(name)
}

fn fixtures() -> Vec<(&'static str, GenerateOptions)> {
    let base = GenerateOptions {
        spec: GenSpec::Uniform(Uniform::new(Layout::Standard)),
        params: Params {
            n_colors: 5,
            capacity: 4,
            n_empty: 2,
        },
        gen_config: GenConfig::default(),
        count: 200,
        master_seed: 0x5EED_2026_1003,
        master_seed_source: "given".into(),
        split: SplitRanges::DEFAULT,
        split_files: true,
        format: Format::Parquet,
        // 2026-10-03T00:00:00Z
        created_at: 1_790_985_600_000_000_000,
        tool_version: "fixture".into(),
        supported: true,
        threads: Some(2),
        progress: false,
        chunk: CHUNK,
        window_chunks: WINDOW_CHUNKS,
    };
    vec![
        ("uniform_c5k4e2", base.clone()),
        (
            "turan_pour_walk_distributed_c4k3e1",
            GenerateOptions {
                spec: GenSpec::Turan(Turan::new(
                    TuranStrategy::DEFAULT_POUR_WALK,
                    Layout::Distributed,
                )),
                params: Params {
                    n_colors: 4,
                    capacity: 3,
                    n_empty: 1,
                },
                count: 120,
                split_files: false,
                format: Format::Jsonl,
                ..base
            },
        ),
    ]
}

/// Files whose bytes must reproduce exactly (the manifest and report also hold wall-clock
/// timings).
fn data_files(dir: &Path, m: &Manifest) -> Vec<String> {
    let mut names: Vec<String> = m.files.iter().map(|f| f.path.clone()).collect();
    if dir.join("duplicates.csv").exists() {
        names.push("duplicates.csv".into());
    }
    names
}

#[test]
fn fixtures_regenerate_byte_for_byte() {
    let bless = std::env::var_os("WATER_SORT_BLESS").is_some();
    for (name, opts) in fixtures() {
        let tmp = std::env::temp_dir().join(format!("ws_fixture_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let m = generate(&tmp, &opts).unwrap();
        let dir = fixture_dir(name);
        if bless {
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            for entry in std::fs::read_dir(&tmp).unwrap() {
                let entry = entry.unwrap();
                std::fs::copy(entry.path(), dir.join(entry.file_name())).unwrap();
            }
        }
        let committed = Manifest::read(&dir).unwrap();
        assert_eq!(committed.files, m.files, "{name}: file hashes");
        for f in data_files(&tmp, &m) {
            let a = std::fs::read(tmp.join(&f)).unwrap();
            let b = std::fs::read(dir.join(&f)).unwrap();
            assert!(a == b, "{name}/{f} differs from the committed fixture");
        }
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}

#[test]
fn fixtures_validate() {
    for (name, _) in fixtures() {
        let report = validate(&fixture_dir(name), 1.0).unwrap();
        assert!(report.ok(), "{name}: {}", report.summary());
        assert_eq!(report.regenerated, report.records);
        assert!(report.records > 0);
    }
}

/// The acceptance property at a larger scale (release, CI job `heavy-tests`): the same master
/// seed with 1 and 4 threads gives byte-identical record files, and the result validates.
#[test]
#[ignore = "heavy: 2 x 10k puzzles; run with --release -- --ignored"]
fn thread_count_does_not_change_the_data() {
    let opts = GenerateOptions {
        params: Params {
            n_colors: 6,
            capacity: 4,
            n_empty: 2,
        },
        count: 10_000,
        ..fixtures().remove(0).1
    };
    let mut files = Vec::new();
    for threads in [1, 4] {
        let dir = std::env::temp_dir().join(format!("ws_heavy_{threads}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let m = generate(
            &dir,
            &GenerateOptions {
                threads: Some(threads),
                ..opts.clone()
            },
        )
        .unwrap();
        let report = validate(&dir, 0.05).unwrap();
        assert!(report.ok(), "{}", report.summary());
        files.push(m.files);
        std::fs::remove_dir_all(&dir).unwrap();
    }
    assert_eq!(files[0], files[1]);
}
