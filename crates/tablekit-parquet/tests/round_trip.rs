//! Parquet files read back with the types of docs/PARQUET.md and every value, nulls included.

use std::fs::File;
use std::path::Path;

use parquet::file::reader::{FileReader, SerializedFileReader};
use parquet::schema::printer::print_schema;
use tablekit::{Batch, Column, TableError, Values};
use tablekit_parquet::{ParquetCompression, ParquetOptions, ParquetWriter, write_file};

fn table() -> Batch {
    let some = |n: usize| (0..n).map(|i| i != 1).collect::<Vec<_>>();
    Batch::new(vec![
        Column::new("record_time", Values::F64(vec![0.5, 1e-5, 1e16])),
        Column::with_missing("flag", Values::Bool(vec![true, false, false]), some(3)).unwrap(),
        Column::new("i8", Values::I8(vec![-128, 0, 127])),
        Column::new("i16", Values::I16(vec![i16::MIN, 0, i16::MAX])),
        Column::new("i32", Values::I32(vec![i32::MIN, 0, i32::MAX])),
        Column::new("i64", Values::I64(vec![i64::MIN, 0, i64::MAX])),
        Column::new("u8", Values::U8(vec![0, 1, u8::MAX])),
        Column::new("u16", Values::U16(vec![0, 1, u16::MAX])),
        Column::with_missing("u32", Values::U32(vec![0, 1, u32::MAX]), some(3)).unwrap(),
        Column::with_missing("u64", Values::U64(vec![0, 1, u64::MAX]), some(3)).unwrap(),
        Column::new("f32", Values::F32(vec![0.1, -1.5, f32::MAX])),
        Column::new("f64", Values::F64(vec![0.1, -0.0, f64::MIN_POSITIVE])),
        Column::with_missing(
            "mode",
            Values::Text(vec!["On".into(), String::new(), "a, \"b\"".into()]),
            some(3),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn read(path: &Path) -> (String, Vec<String>) {
    let reader = SerializedFileReader::new(File::open(path).unwrap()).unwrap();
    let mut schema = Vec::new();
    print_schema(&mut schema, reader.metadata().file_metadata().schema());
    let rows = reader
        .get_row_iter(None)
        .unwrap()
        .map(|row| row.unwrap().to_string())
        .collect();
    (String::from_utf8(schema).unwrap(), rows)
}

fn dir() -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("tablekit-parquet-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn types_and_values_read_back() {
    let path = dir().join("round_trip.parquet");
    assert_eq!(
        write_file(&path, &table(), &ParquetOptions::default()).unwrap(),
        3
    );
    let (schema, rows) = read(&path);
    let want_schema = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/expected/parquet/schema.txt"),
    )
    .unwrap();
    assert_eq!(schema, want_schema);
    // As the parquet crate's reader prints them: a decimal ends in `.`, floats it cannot
    // print briefly in `E` notation.
    assert_eq!(
        rows,
        [
            "{record_time: 0.5, flag: true, i8: -128, i16: -32768, i32: -2147483648, \
             i64: -9223372036854775808, u8: 0, u16: 0, u32: 0, u64: 0., f32: 0.1, f64: 0.1, \
             mode: \"On\"}",
            "{record_time: 0.00001, flag: null, i8: 0, i16: 0, i32: 0, i64: 0, u8: 1, u16: 1, \
             u32: null, u64: null, f32: -1.5E0, f64: -0E0, mode: null}",
            "{record_time: 10000000000000000.0, flag: false, i8: 127, i16: 32767, \
             i32: 2147483647, i64: 9223372036854775807, u8: 255, u16: 65535, u32: 4294967295, \
             u64: 18446744073709551615., f32: 3.4028235E38, f64: 2.2250738585072014E-308, \
             mode: \"a, \"b\"\"}",
        ]
    );
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn uncompressed_and_in_several_row_groups() {
    let path = dir().join("groups.parquet");
    let whole = table();
    let options = ParquetOptions {
        compression: ParquetCompression::None,
    };
    tablekit::write_atomically(&path, |out| {
        let mut w = ParquetWriter::new(out, whole.schema(), &options)?;
        w.write(&whole)?;
        w.write(&whole)?;
        assert_eq!(w.finish()?, 6);
        Ok(())
    })
    .unwrap();
    let reader = SerializedFileReader::new(File::open(&path).unwrap()).unwrap();
    assert_eq!(reader.metadata().num_row_groups(), 2);
    assert_eq!(reader.metadata().file_metadata().num_rows(), 6);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn names_must_be_unique() {
    let batch = Batch::new(vec![
        Column::new("a", Values::I32(vec![1])),
        Column::new("a", Values::I32(vec![2])),
    ])
    .unwrap();
    let path = dir().join("dupes.parquet");
    let e = write_file(&path, &batch, &ParquetOptions::default()).unwrap_err();
    assert!(
        matches!(e, TableError::DuplicateName(ref n) if n == "a"),
        "{e}"
    );
    assert!(!path.exists());
    assert!(!tablekit::temporary_path(&path).exists());
}
