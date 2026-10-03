//! JSON Lines: one [`Record`] per line, for debugging and small datasets (`--format jsonl`).

use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use super::record::Record;

/// Writes records as JSON lines.
pub struct JsonlWriter<W: Write> {
    out: W,
}

impl JsonlWriter<BufWriter<File>> {
    /// Creates (or truncates) `path`.
    ///
    /// # Errors
    ///
    /// The file cannot be created.
    pub fn create(path: &Path) -> io::Result<Self> {
        Ok(Self::new(BufWriter::new(File::create(path)?)))
    }
}

impl<W: Write> JsonlWriter<W> {
    pub const fn new(out: W) -> Self {
        Self { out }
    }

    /// Appends one record.
    ///
    /// # Errors
    ///
    /// The underlying writer fails.
    pub fn write(&mut self, record: &Record) -> io::Result<()> {
        serde_json::to_writer(&mut self.out, record)?;
        self.out.write_all(b"\n")
    }

    /// Flushes and returns the writer.
    ///
    /// # Errors
    ///
    /// The flush fails.
    pub fn finish(mut self) -> io::Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }
}

/// Reads every record of a JSON Lines file, in file order. Blank lines are skipped.
///
/// # Errors
///
/// I/O errors and malformed lines (with their line number).
pub fn read_jsonl(path: &Path) -> io::Result<Vec<Record>> {
    let mut records = Vec::new();
    for (i, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let record = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}:{}: {e}", path.display(), i + 1),
            )
        })?;
        records.push(record);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::record::tests::sample;

    #[test]
    fn writes_and_reads_back() {
        let records: Vec<Record> = (0..5).map(sample).collect();
        let dir = std::env::temp_dir().join(format!("ws_jsonl_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.jsonl");
        let mut w = JsonlWriter::create(&path).unwrap();
        for r in &records {
            w.write(r).unwrap();
        }
        w.finish().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 5);
        assert_eq!(read_jsonl(&path).unwrap(), records);
        std::fs::write(&path, format!("{text}{{\"record_id\": 1}}\n")).unwrap();
        let err = read_jsonl(&path).unwrap_err().to_string();
        assert!(err.contains(":6:"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
