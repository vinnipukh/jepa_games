//! `manifest.json`: everything needed to reproduce, check and describe a dataset directory.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use water_sort_core::{GenConfig, Layout, Params};

use super::record::hex_u64;
use super::split::{Split, SplitRanges};

/// Manifest name inside a dataset directory.
pub const MANIFEST: &str = "manifest.json";
/// Bumped when the record schema or the directory layout changes.
pub const FORMAT_VERSION: u32 = 1;

/// The tool version recorded with every dataset: crate version + git commit.
pub fn tool_version() -> String {
    format!(
        "water_sort_cli {}+{}",
        env!("CARGO_PKG_VERSION"),
        env!("WATER_SORT_GIT_COMMIT")
    )
}

/// Storage format of the record files.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    Parquet,
    Jsonl,
}

impl Format {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Parquet => "parquet",
            Self::Jsonl => "jsonl",
        }
    }
}

/// The generator that produced a dataset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratorInfo {
    pub id: String,
    pub version: u32,
    pub variant: String,
    pub layout: Layout,
    /// Generator spec string that rebuilds the generator (`GenSpec`'s `FromStr`).
    pub spec: String,
}

/// One record file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Relative to the dataset directory.
    pub path: String,
    /// The split all its records belong to, or `None` for a file holding every split.
    pub split: Option<Split>,
    pub records: u64,
    pub bytes: u64,
    pub sha256: String,
}

/// An index whose generation failed (e.g. `TooManyAttempts`); it has no record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Failure {
    pub record_id: u64,
    #[serde(with = "hex_u64")]
    pub seed: u64,
    pub error: String,
}

/// Dedup outcome of the generation run (see `dedup`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DedupSummary {
    /// Records dropped because an earlier record had the same canonical form.
    pub duplicates: u64,
    /// `duplicates / generated`.
    pub duplicate_rate: f64,
    /// Distinct canonical forms that share a 64-bit hash with an earlier record; both are kept.
    pub hash_collisions: u64,
    /// File listing every dropped record id with the id it duplicates.
    pub dropped_file: String,
    /// `(generated so far, duplicates so far)` at every power of ten and at the end.
    pub saturation: Vec<(u64, u64)>,
}

/// Split rule and per-split counts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SplitInfo {
    pub rule: String,
    pub ranges: SplitRanges,
    /// `[start, end)` bucket range per split.
    pub buckets: BTreeMap<Split, (u8, u8)>,
    pub counts: BTreeMap<Split, u64>,
}

/// `manifest.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub generator: GeneratorInfo,
    pub params: Params,
    pub gen_config: GenConfig,
    /// The exact JSON string stored in every record's `gen_config` column.
    pub gen_config_json: String,
    /// Whether `params` is in the supported range of the layout (`--allow-unsupported`).
    pub supported: bool,
    #[serde(with = "hex_u64")]
    pub master_seed: u64,
    /// `given` (`--master-seed`) or `fresh` (the generator's `fresh_seed`).
    pub master_seed_source: String,
    pub seed_rule: String,
    /// Indices generated: `0..count`.
    pub count: u64,
    /// Records stored, after dedup and failures.
    pub records: u64,
    pub failures: Vec<Failure>,
    pub dedup: DedupSummary,
    pub split: SplitInfo,
    /// Records per tier (`none` outside the supported range).
    pub tiers: BTreeMap<String, u64>,
    /// Records per split and tier.
    pub split_tiers: BTreeMap<Split, BTreeMap<String, u64>>,
    pub format: Format,
    pub files: Vec<FileEntry>,
    pub tool_version: String,
    /// The `created_at` value of every record (RFC 3339).
    pub created_at: String,
    /// Wall-clock start and end of the run (RFC 3339); not part of the data.
    pub started_at: String,
    pub finished_at: String,
    pub wall_time_secs: f64,
    pub threads: usize,
    /// Set when this dataset was derived from another by `leakage --exclude-from`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclusion: Option<Exclusion>,
}

/// Records removed from a dataset because they also occur in another one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    /// Dataset the records were removed from.
    pub source: String,
    pub source_manifest_sha256: String,
    /// Dataset whose puzzles were excluded.
    pub other: String,
    pub other_manifest_sha256: String,
    /// `None`: all splits.
    pub other_split: Option<Split>,
    pub split: Option<Split>,
    pub removed: u64,
}

impl Manifest {
    pub fn read(dir: &Path) -> io::Result<Self> {
        let path = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| io::Error::new(e.kind(), format!("{}: {e}", path.display())))?;
        serde_json::from_str(&text).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        })
    }

    pub fn write(&self, dir: &Path) -> io::Result<()> {
        let mut text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        text.push('\n');
        std::fs::write(dir.join(MANIFEST), text)
    }
}

/// Lower-case hex SHA-256 of a file, and its size.
pub fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    let mut buf = vec![0; 1 << 16];
    let mut bytes = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    let digest = hasher.finalize();
    let hex = digest.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    });
    Ok((hex, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_reference() {
        let dir = std::env::temp_dir().join(format!("ws_sha_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("abc");
        std::fs::write(&path, b"abc").unwrap();
        assert_eq!(
            sha256_file(&path).unwrap(),
            (
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                3
            )
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
