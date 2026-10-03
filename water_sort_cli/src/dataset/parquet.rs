//! Parquet storage (the default format): the Arrow schema of [`Record`], zstd compression, row
//! groups of [`ROW_GROUP`] records.
//!
//! The writer's output depends only on the records written, so a dataset generated with any
//! thread count is byte-identical.

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use arrow_array::builder::{
    FixedSizeBinaryBuilder, Float32Builder, ListBuilder, UInt8Builder, UInt16Builder,
    UInt32Builder, UInt64Builder,
};
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Float32Type, TimestampMicrosecondType, UInt8Type, UInt16Type, UInt32Type, UInt64Type,
};
use arrow_array::{
    Array, ArrayRef, RecordBatch, StringArray, StructArray, TimestampMicrosecondArray,
};
use arrow_schema::{DataType, Field, Fields, Schema, SchemaRef, TimeUnit};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, ZstdLevel};
use parquet::file::properties::WriterProperties;
use water_sort_core::{Layout, Params, Tier};

use super::record::{Record, RecordMetrics};
use super::split::Split;

/// Records per row group.
pub const ROW_GROUP: usize = 64 * 1024;
/// zstd level.
const ZSTD_LEVEL: i32 = 3;

fn params_fields() -> Fields {
    Fields::from(vec![
        Field::new("n_colors", DataType::UInt8, false),
        Field::new("capacity", DataType::UInt8, false),
        Field::new("n_empty", DataType::UInt8, false),
    ])
}

fn solution_item() -> Arc<Field> {
    Arc::new(Field::new("item", DataType::UInt16, false))
}

/// The Arrow schema of a dataset with `params` (the state column has a fixed width).
pub fn schema(params: Params) -> SchemaRef {
    let cells = i32::try_from(params.n_tubes() * usize::from(params.capacity))
        .expect("at most MAX_TUBES × MAX_CAP cells");
    let utf8 = |name: &str| Field::new(name, DataType::Utf8, false);
    Arc::new(Schema::new(vec![
        Field::new("record_id", DataType::UInt64, false),
        utf8("generator_id"),
        Field::new("generator_version", DataType::UInt32, false),
        utf8("generator_variant"),
        utf8("layout"),
        Field::new("params", DataType::Struct(params_fields()), false),
        utf8("gen_config"),
        Field::new("seed", DataType::UInt64, false),
        utf8("puzzle_code"),
        Field::new("state", DataType::FixedSizeBinary(cells), false),
        Field::new("opt_moves", DataType::UInt32, false),
        Field::new("tier", DataType::Utf8, true),
        Field::new("solution", DataType::List(solution_item()), false),
        Field::new("canonical_hash", DataType::UInt64, false),
        Field::new("attempts", DataType::UInt32, false),
        Field::new("metrics_states_expanded", DataType::UInt64, false),
        Field::new("metrics_color_changes", DataType::UInt32, false),
        Field::new("metrics_segments", DataType::UInt32, false),
        Field::new("metrics_random_rollouts", DataType::UInt32, false),
        Field::new("metrics_random_stuck_rate", DataType::Float32, false),
        Field::new("metrics_random_capped_rate", DataType::Float32, false),
        Field::new("metrics_dead_end_ratio_d1", DataType::Float32, true),
        Field::new("metrics_dead_end_ratio_d2", DataType::Float32, true),
        utf8("split"),
        Field::new(
            "created_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        utf8("tool_version"),
    ]))
}

fn strings<'a>(values: impl Iterator<Item = &'a str>) -> ArrayRef {
    Arc::new(values.map(Some).collect::<StringArray>())
}

fn u32s(values: impl Iterator<Item = u32>) -> ArrayRef {
    let mut b = UInt32Builder::new();
    b.extend(values.map(Some));
    Arc::new(b.finish())
}

fn u64s(values: impl Iterator<Item = u64>) -> ArrayRef {
    let mut b = UInt64Builder::new();
    b.extend(values.map(Some));
    Arc::new(b.finish())
}

fn f32s(values: impl Iterator<Item = Option<f32>>) -> ArrayRef {
    let mut b = Float32Builder::new();
    b.extend(values);
    Arc::new(b.finish())
}

/// Builds one record batch. All records must have `params`.
pub fn to_batch(
    params: Params,
    records: &[Record],
) -> Result<RecordBatch, arrow_schema::ArrowError> {
    let schema = schema(params);
    let cells = params.n_tubes() * usize::from(params.capacity);
    let r = records;
    let params_array = {
        let col = |f: fn(&Params) -> u8| -> ArrayRef {
            let mut b = UInt8Builder::new();
            b.extend(r.iter().map(|x| Some(f(&x.params))));
            Arc::new(b.finish())
        };
        StructArray::try_new(
            params_fields(),
            vec![col(|p| p.n_colors), col(|p| p.capacity), col(|p| p.n_empty)],
            None,
        )?
    };
    let mut state =
        FixedSizeBinaryBuilder::with_capacity(r.len(), i32::try_from(cells).unwrap_or(0));
    for x in r {
        state.append_value(&x.state)?;
    }
    let mut solution = ListBuilder::new(UInt16Builder::new()).with_field(solution_item());
    for x in r {
        solution.values().append_slice(&x.solution);
        solution.append(true);
    }
    let tiers: StringArray = r.iter().map(|x| x.tier.map(|t| t.to_string())).collect();
    let m = |x: &Record| x.metrics;
    // Microseconds: Python's `datetime` cannot hold nanoseconds (the run's clock reading is
    // truncated to whole microseconds, see `GenerateOptions::created_at`).
    let created_at = TimestampMicrosecondArray::from(
        r.iter()
            .map(|x| x.created_at.div_euclid(1000))
            .collect::<Vec<_>>(),
    )
    .with_timezone("UTC");
    let columns: Vec<ArrayRef> = vec![
        u64s(r.iter().map(|x| x.record_id)),
        strings(r.iter().map(|x| x.generator_id.as_str())),
        u32s(r.iter().map(|x| x.generator_version)),
        strings(r.iter().map(|x| x.generator_variant.as_str())),
        strings(r.iter().map(|x| x.layout.name())),
        Arc::new(params_array),
        strings(r.iter().map(|x| x.gen_config.as_str())),
        u64s(r.iter().map(|x| x.seed)),
        strings(r.iter().map(|x| x.puzzle_code.as_str())),
        Arc::new(state.finish()),
        u32s(r.iter().map(|x| x.opt_moves)),
        Arc::new(tiers),
        Arc::new(solution.finish()),
        u64s(r.iter().map(|x| x.canonical_hash)),
        u32s(r.iter().map(|x| x.attempts)),
        u64s(r.iter().map(|x| m(x).states_expanded)),
        u32s(r.iter().map(|x| m(x).color_changes)),
        u32s(r.iter().map(|x| m(x).segments)),
        u32s(r.iter().map(|x| m(x).random_rollouts)),
        f32s(r.iter().map(|x| Some(m(x).random_stuck_rate))),
        f32s(r.iter().map(|x| Some(m(x).random_capped_rate))),
        f32s(r.iter().map(|x| m(x).dead_end_ratio_d1)),
        f32s(r.iter().map(|x| m(x).dead_end_ratio_d2)),
        strings(r.iter().map(|x| x.split.name())),
        Arc::new(created_at),
        strings(r.iter().map(|x| x.tool_version.as_str())),
    ];
    RecordBatch::try_new(schema, columns)
}

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// Reads the records of one batch written by [`to_batch`].
pub fn from_batch(batch: &RecordBatch) -> io::Result<Vec<Record>> {
    let col = |name: &str| {
        batch
            .column_by_name(name)
            .ok_or_else(|| invalid(format!("missing column {name}")))
    };
    let u64_col = |name: &str| col(name).map(|c| c.as_primitive::<UInt64Type>().clone());
    let u32_col = |name: &str| col(name).map(|c| c.as_primitive::<UInt32Type>().clone());
    let f32_col = |name: &str| col(name).map(|c| c.as_primitive::<Float32Type>().clone());
    let str_col = |name: &str| col(name).map(|c| c.as_string::<i32>().clone());

    let record_id = u64_col("record_id")?;
    let generator_id = str_col("generator_id")?;
    let generator_version = u32_col("generator_version")?;
    let generator_variant = str_col("generator_variant")?;
    let layout = str_col("layout")?;
    let params = col("params")?.as_struct().clone();
    let p_col = |name: &str| {
        params
            .column_by_name(name)
            .map(|c| c.as_primitive::<UInt8Type>().clone())
            .ok_or_else(|| invalid(format!("missing column params.{name}")))
    };
    let (n_colors, capacity, n_empty) = (p_col("n_colors")?, p_col("capacity")?, p_col("n_empty")?);
    let gen_config = str_col("gen_config")?;
    let seed = u64_col("seed")?;
    let puzzle_code = str_col("puzzle_code")?;
    let state = col("state")?.as_fixed_size_binary().clone();
    let opt_moves = u32_col("opt_moves")?;
    let tier = str_col("tier")?;
    let solution = col("solution")?.as_list::<i32>().clone();
    let canonical_hash = u64_col("canonical_hash")?;
    let attempts = u32_col("attempts")?;
    let states_expanded = u64_col("metrics_states_expanded")?;
    let color_changes = u32_col("metrics_color_changes")?;
    let segments = u32_col("metrics_segments")?;
    let random_rollouts = u32_col("metrics_random_rollouts")?;
    let stuck = f32_col("metrics_random_stuck_rate")?;
    let capped = f32_col("metrics_random_capped_rate")?;
    let d1 = f32_col("metrics_dead_end_ratio_d1")?;
    let d2 = f32_col("metrics_dead_end_ratio_d2")?;
    let split = str_col("split")?;
    let created_at = col("created_at")?
        .as_primitive::<TimestampMicrosecondType>()
        .clone();
    let tool_version = str_col("tool_version")?;

    let opt = |a: &arrow_array::PrimitiveArray<Float32Type>, i: usize| {
        (!a.is_null(i)).then(|| a.value(i))
    };
    (0..batch.num_rows())
        .map(|i| {
            Ok(Record {
                record_id: record_id.value(i),
                generator_id: generator_id.value(i).to_owned(),
                generator_version: generator_version.value(i),
                generator_variant: generator_variant.value(i).to_owned(),
                layout: layout
                    .value(i)
                    .parse::<Layout>()
                    .map_err(|e| invalid(e.to_string()))?,
                params: Params {
                    n_colors: n_colors.value(i),
                    capacity: capacity.value(i),
                    n_empty: n_empty.value(i),
                },
                gen_config: gen_config.value(i).to_owned(),
                seed: seed.value(i),
                puzzle_code: puzzle_code.value(i).to_owned(),
                state: state.value(i).to_vec(),
                opt_moves: opt_moves.value(i),
                tier: if tier.is_null(i) {
                    None
                } else {
                    Some(
                        tier.value(i)
                            .parse::<Tier>()
                            .map_err(|e| invalid(e.to_string()))?,
                    )
                },
                solution: solution
                    .value(i)
                    .as_primitive::<UInt16Type>()
                    .values()
                    .to_vec(),
                canonical_hash: canonical_hash.value(i),
                attempts: attempts.value(i),
                metrics: RecordMetrics {
                    states_expanded: states_expanded.value(i),
                    color_changes: color_changes.value(i),
                    segments: segments.value(i),
                    random_rollouts: random_rollouts.value(i),
                    random_stuck_rate: stuck.value(i),
                    random_capped_rate: capped.value(i),
                    dead_end_ratio_d1: opt(&d1, i),
                    dead_end_ratio_d2: opt(&d2, i),
                },
                split: split.value(i).parse::<Split>().map_err(invalid)?,
                created_at: created_at.value(i) * 1000,
                tool_version: tool_version.value(i).to_owned(),
            })
        })
        .collect()
}

/// Writes records to a Parquet file, one row group per [`ROW_GROUP`] records.
pub struct ParquetWriter<W: Write + Send> {
    params: Params,
    writer: ArrowWriter<W>,
    buffer: Vec<Record>,
}

impl ParquetWriter<File> {
    /// Creates (or truncates) `path`.
    pub fn create(path: &Path, params: Params) -> io::Result<Self> {
        Self::new(File::create(path)?, params)
    }
}

impl<W: Write + Send> ParquetWriter<W> {
    pub fn new(out: W, params: Params) -> io::Result<Self> {
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(
                ZstdLevel::try_new(ZSTD_LEVEL).map_err(io::Error::other)?,
            ))
            .set_max_row_group_row_count(Some(ROW_GROUP))
            .build();
        let writer =
            ArrowWriter::try_new(out, schema(params), Some(props)).map_err(io::Error::other)?;
        Ok(Self {
            params,
            writer,
            buffer: Vec::with_capacity(ROW_GROUP),
        })
    }

    /// Appends one record.
    pub fn write(&mut self, record: Record) -> io::Result<()> {
        if record.params != self.params {
            return Err(invalid(format!(
                "record {} has params {:?}, the file has {:?}",
                record.record_id, record.params, self.params
            )));
        }
        self.buffer.push(record);
        if self.buffer.len() == ROW_GROUP {
            self.flush_group()?;
        }
        Ok(())
    }

    fn flush_group(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let batch = to_batch(self.params, &self.buffer).map_err(io::Error::other)?;
        self.writer.write(&batch).map_err(io::Error::other)?;
        self.writer.flush().map_err(io::Error::other)?;
        self.buffer.clear();
        Ok(())
    }

    /// Writes the remaining records and the file footer.
    pub fn finish(mut self) -> io::Result<W> {
        self.flush_group()?;
        self.writer.into_inner().map_err(io::Error::other)
    }
}

/// Reads every record of a Parquet file, in file order.
pub fn read_parquet(path: &Path) -> io::Result<Vec<Record>> {
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)
        .and_then(ParquetRecordBatchReaderBuilder::build)
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let mut records = Vec::new();
    for batch in reader {
        let batch = batch.map_err(|e| invalid(format!("{}: {e}", path.display())))?;
        records.extend(from_batch(&batch)?);
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataset::record::tests::{P, sample};

    #[test]
    fn writes_and_reads_back() {
        let mut records: Vec<Record> = (0..20).map(sample).collect();
        records[3].tier = None;
        records[4].metrics.dead_end_ratio_d1 = Some(0.25);
        records[4].metrics.dead_end_ratio_d2 = Some(0.5);
        let write = |records: &[Record]| {
            let mut w = ParquetWriter::new(Vec::new(), P).unwrap();
            for r in records {
                w.write(r.clone()).unwrap();
            }
            w.finish().unwrap()
        };
        let bytes = write(&records);
        assert_eq!(bytes, write(&records), "output is deterministic");
        let dir = std::env::temp_dir().join(format!("ws_parquet_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.parquet");
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(read_parquet(&path).unwrap(), records);
        std::fs::remove_dir_all(&dir).unwrap();

        let mut w = ParquetWriter::new(Vec::new(), Params { n_empty: 1, ..P }).unwrap();
        assert!(w.write(records[0].clone()).is_err());
    }

    #[test]
    fn schema_has_the_planned_columns() {
        let s = schema(P);
        assert_eq!(
            s.field_with_name("state").unwrap().data_type(),
            &DataType::FixedSizeBinary(24)
        );
        assert!(s.field_with_name("tier").unwrap().is_nullable());
        assert_eq!(s.fields().len(), 26);
    }
}
