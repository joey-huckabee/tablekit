//! tablekit-csv writes, byte for byte, what pandas wrote for the same tables
//! (`testdata/expected/csv/make.py`).

use std::path::{Path, PathBuf};

use tablekit::{Batch, Column, TableError, Values};
use tablekit_csv::{CsvOptions, CsvWriter, LineEnding, write_file, write_to};

fn expected(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/expected/csv")
        .join(name)
}

/// The table `make.py` gives pandas.
fn table() -> Batch {
    Batch::new(vec![
        Column::new("time", Values::F64(vec![0.5, 1e-05, 1e16, 123.0])),
        Column::new("flag", Values::Bool(vec![true, false, true, false])),
        Column::new("small", Values::I8(vec![-128, 0, 127, 5])),
        Column::new("count", Values::U64(vec![u64::MAX, 0, 1, 2])),
        Column::new("offset", Values::I64(vec![i64::MIN, i64::MAX, -1, 0])),
        Column::with_missing(
            "level",
            Values::F32(vec![0.1, 1e-4, 3.4e38, 0.0]),
            vec![true, true, true, false],
        )
        .unwrap(),
        Column::new(
            "edge",
            Values::F64(vec![f64::INFINITY, f64::NEG_INFINITY, 0.0, -0.0]),
        ),
        Column::new(
            "note",
            Values::Text(vec![
                "plain".into(),
                "a, b".into(),
                "say \"hi\"".into(),
                "two\nlines".into(),
            ]),
        ),
        Column::with_missing(
            "Mode",
            Values::Text(vec![
                "On".into(),
                String::new(),
                "Off".into(),
                String::new(),
            ]),
            vec![true, false, true, true],
        )
        .unwrap(),
        Column::new("Pos.X", Values::I16(vec![1, -2, 3, -4])),
    ])
    .unwrap()
}

fn check(batch: &Batch, options: &CsvOptions, file: &str) {
    let mut out = Vec::new();
    write_to(&mut out, batch, options).unwrap();
    let want = std::fs::read(expected(file)).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&want),
        "{file}"
    );
    assert_eq!(out, want, "{file}");
}

#[test]
fn default_options_write_what_pandas_writes() {
    check(&table(), &CsvOptions::default(), "table.csv");
}

#[test]
fn a_byte_order_mark_as_utf_8_sig() {
    let options = CsvOptions {
        bom: true,
        ..CsvOptions::default()
    };
    check(&table(), &options, "table-bom.csv");
}

#[test]
fn na_rep_for_missing_values_and_nan() {
    let options = CsvOptions {
        na_rep: "NA".into(),
        ..CsvOptions::default()
    };
    check(&table(), &options, "table-na.csv");
}

#[test]
fn line_feeds_alone() {
    let options = CsvOptions {
        line_ending: LineEnding::Lf,
        ..CsvOptions::default()
    };
    check(&table(), &options, "table-lf.csv");
}

#[test]
fn one_empty_field_is_written_as_two_quotes() {
    let batch = Batch::new(vec![
        Column::with_missing(
            "x",
            Values::Text(vec!["a".into(), String::new(), String::new()]),
            vec![true, true, false],
        )
        .unwrap(),
    ])
    .unwrap();
    check(&batch, &CsvOptions::default(), "one-column.csv");
}

#[test]
fn integers_stay_integers_when_some_are_missing() {
    // pandas would write 1.0 and 3.0 here; see docs/CSV-FORMAT.md.
    let batch = Batch::new(vec![
        Column::with_missing("n", Values::U32(vec![1, 0, 3]), vec![true, false, true]).unwrap(),
        Column::with_missing(
            "b",
            Values::Bool(vec![true, false, false]),
            vec![true, true, false],
        )
        .unwrap(),
    ])
    .unwrap();
    let mut out = Vec::new();
    write_to(&mut out, &batch, &CsvOptions::default()).unwrap();
    assert_eq!(out, b"n,b\r\n1,True\r\n,False\r\n3,\r\n");
}

#[test]
fn several_batches_make_one_table() {
    let whole = table();
    let mut w = CsvWriter::new(Vec::new(), whole.schema(), CsvOptions::default()).unwrap();
    w.write(&whole).unwrap();
    w.write(&whole).unwrap();
    let (out, rows) = w.finish().unwrap();
    assert_eq!(rows, 8);
    let one = std::fs::read(expected("table.csv")).unwrap();
    let header = one.iter().position(|&b| b == b'\n').unwrap() + 1;
    assert_eq!(out, [&one[..], &one[header..]].concat());

    let other = Batch::new(vec![Column::new("time", Values::F64(vec![1.0]))]).unwrap();
    let mut w = CsvWriter::new(Vec::new(), whole.schema(), CsvOptions::default()).unwrap();
    assert!(matches!(
        w.write(&other),
        Err(TableError::SchemaMismatch { .. })
    ));
}

#[test]
fn files_are_written_whole_or_not_at_all() {
    let dir = std::env::temp_dir().join(format!("tablekit-csv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.csv");
    assert_eq!(
        write_file(&path, &table(), &CsvOptions::default()).unwrap(),
        4
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(expected("table.csv")).unwrap()
    );
    assert!(!tablekit::temporary_path(&path).exists());

    // A failed write leaves the earlier file as it was, and no temporary file.
    let failed = tablekit::write_atomically(&path, |out| {
        std::io::Write::write_all(out, b"partial")?;
        Err::<(), _>(TableError::NoColumns)
    });
    assert!(failed.is_err());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        std::fs::read(expected("table.csv")).unwrap()
    );
    assert!(!tablekit::temporary_path(&path).exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
