//! Writes a [`Batch`] as CSV, in the one format every tablekit user shares:
//! `docs/CSV-FORMAT.md`. By default that is what pandas writes with
//! `DataFrame.to_csv(path, index=False)` on Windows.
//!
//! ```
//! use tablekit::{Batch, Column, Values};
//! use tablekit_csv::{CsvOptions, write_to};
//!
//! let batch = Batch::new(vec![
//!     Column::new("time", Values::F64(vec![0.5, 1e-5])),
//!     Column::with_missing("Speed", Values::U16(vec![120, 0]), vec![true, false])?,
//!     Column::new("Valid", Values::Bool(vec![true, false])),
//!     Column::new("Note", Values::Text(vec!["a, b".into(), String::new()])),
//! ])?;
//! let mut out = Vec::new();
//! write_to(&mut out, &batch, &CsvOptions::default())?;
//! assert_eq!(
//!     out,
//!     b"time,Speed,Valid,Note\r\n0.5,120,True,\"a, b\"\r\n1e-05,,False,\r\n"
//! );
//! # Ok::<(), tablekit::TableError>(())
//! ```
//!
//! [`write_file`] writes a file whole or not at all. [`CsvWriter`] takes a table in several
//! batches, for tables too large to hold at once.

#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::todo,
        clippy::unimplemented,
        clippy::unreachable
    )
)]

mod float;

use std::io::Write;
use std::path::Path;

use tablekit::{Batch, Column, Field, TableError, Values, write_atomically};

/// How each line ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    /// `\r\n`: what pandas writes on Windows, and what RFC 4180 specifies. The default.
    #[default]
    CrLf,
    /// `\n`: what pandas writes on Linux and macOS.
    Lf,
}

impl LineEnding {
    fn bytes(self) -> &'static [u8] {
        match self {
            LineEnding::CrLf => b"\r\n",
            LineEnding::Lf => b"\n",
        }
    }
}

/// The choices a user may make; [`CsvOptions::default`] is pandas' `to_csv` on Windows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CsvOptions {
    /// Written for a missing value, and for a floating-point NaN: pandas' `na_rep`. Default
    /// empty.
    pub na_rep: String,
    /// How each line ends. Default [`LineEnding::CrLf`].
    pub line_ending: LineEnding,
    /// Start the file with a UTF-8 byte-order mark, which Excel needs to show non-ASCII text
    /// correctly: pandas' `encoding="utf-8-sig"`. Default `false`, as pandas.
    pub bom: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            na_rep: String::new(),
            line_ending: LineEnding::CrLf,
            bom: false,
        }
    }
}

/// The UTF-8 byte-order mark.
const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Bytes gathered before each write to the output.
const FLUSH_AT: usize = 1 << 20;

/// Writes a table as CSV in one or more batches: the header first, then each batch's rows.
pub struct CsvWriter<W: Write> {
    out: W,
    schema: Vec<Field>,
    options: CsvOptions,
    buffer: Vec<u8>,
    scratch: Vec<u8>,
    rows: u64,
}

impl<W: Write> CsvWriter<W> {
    /// Starts a table with these columns: writes the byte-order mark if asked, and the header.
    ///
    /// # Errors
    ///
    /// [`TableError::NoColumns`] for no columns, or the output failed.
    pub fn new(mut out: W, schema: Vec<Field>, options: CsvOptions) -> Result<Self, TableError> {
        if schema.is_empty() {
            return Err(TableError::NoColumns);
        }
        let mut buffer = Vec::with_capacity(FLUSH_AT + 4096);
        if options.bom {
            buffer.extend_from_slice(BOM);
        }
        let single = schema.len() == 1;
        for (i, field) in schema.iter().enumerate() {
            if i > 0 {
                buffer.push(b',');
            }
            text(&mut buffer, &field.name);
        }
        end_row(
            &mut buffer,
            single,
            schema.iter().all(|f| f.name.is_empty()),
            &options,
        );
        out.write_all(&buffer)?;
        buffer.clear();
        Ok(CsvWriter {
            out,
            schema,
            options,
            buffer,
            scratch: Vec::with_capacity(64),
            rows: 0,
        })
    }

    /// Appends `batch`'s rows.
    ///
    /// # Errors
    ///
    /// [`TableError::SchemaMismatch`] if its columns differ from the header's, or the output
    /// failed.
    pub fn write(&mut self, batch: &Batch) -> Result<(), TableError> {
        let found = batch.schema();
        if found != self.schema {
            return Err(TableError::SchemaMismatch {
                expected: self.schema.clone(),
                found,
            });
        }
        let columns = batch.columns();
        let single = columns.len() == 1;
        // HOT PATH: begin. One line per row, one field per value, into the reused buffer.
        for row in 0..batch.rows() {
            let line_start = self.buffer.len();
            for (i, column) in columns.iter().enumerate() {
                if i > 0 {
                    self.buffer.push(b',');
                }
                self.value(column, row);
            }
            let empty = self.buffer.len() == line_start;
            end_row(&mut self.buffer, single, empty, &self.options);
            if self.buffer.len() >= FLUSH_AT {
                self.out.write_all(&self.buffer)?;
                self.buffer.clear();
            }
        }
        // HOT PATH: end.
        self.rows += batch.rows() as u64;
        Ok(())
    }

    /// Writes what is buffered and returns the output, and how many rows were written.
    ///
    /// # Errors
    ///
    /// The output failed.
    pub fn finish(mut self) -> Result<(W, u64), TableError> {
        self.out.write_all(&self.buffer)?;
        self.out.flush()?;
        Ok((self.out, self.rows))
    }

    /// Appends one value, or `na_rep` for a missing one or a NaN.
    // HOT PATH: per value.
    #[inline]
    fn value(&mut self, column: &Column, row: usize) {
        let out = &mut self.buffer;
        if !column.is_present(row) {
            text(out, &self.options.na_rep);
            return;
        }
        match column.values() {
            Values::Bool(v) => {
                out.extend_from_slice(if v[row] { b"True" } else { b"False" });
            }
            Values::I8(v) => integer(out, v[row]),
            Values::I16(v) => integer(out, v[row]),
            Values::I32(v) => integer(out, v[row]),
            Values::I64(v) => integer(out, v[row]),
            Values::U8(v) => integer(out, v[row]),
            Values::U16(v) => integer(out, v[row]),
            Values::U32(v) => integer(out, v[row]),
            Values::U64(v) => integer(out, v[row]),
            Values::F32(v) => {
                let x = v[row];
                if x.is_nan() {
                    text(out, &self.options.na_rep);
                } else if x.is_infinite() {
                    out.extend_from_slice(if x > 0.0 { b"inf" } else { b"-inf" });
                } else {
                    float::write_f32(out, &mut self.scratch, x);
                }
            }
            Values::F64(v) => {
                let x = v[row];
                if x.is_nan() {
                    text(out, &self.options.na_rep);
                } else if x.is_infinite() {
                    out.extend_from_slice(if x > 0.0 { b"inf" } else { b"-inf" });
                } else {
                    float::write_f64(out, &mut self.scratch, x);
                }
            }
            Values::Text(v) => text(out, &v[row]),
        }
    }
}

/// Ends a row. A row of one empty field is written `""`, as Python's `csv` module (and so
/// pandas) writes it, so the line is not mistaken for a blank one.
#[inline]
fn end_row(out: &mut Vec<u8>, single: bool, empty: bool, options: &CsvOptions) {
    if single && empty {
        out.extend_from_slice(b"\"\"");
    }
    out.extend_from_slice(options.line_ending.bytes());
}

// HOT PATH: per value.
#[inline]
fn integer(out: &mut Vec<u8>, v: impl std::fmt::Display) {
    let _ = write!(out, "{v}");
}

/// Appends `s`, in double quotes (each inner quote doubled) only if it holds a comma, a
/// double quote, a carriage return or a line feed: Python's `csv.QUOTE_MINIMAL`, pandas'
/// default.
// HOT PATH: per value.
#[inline]
fn text(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    if !bytes
        .iter()
        .any(|&b| matches!(b, b',' | b'"' | b'\r' | b'\n'))
    {
        out.extend_from_slice(bytes);
        return;
    }
    out.push(b'"');
    for &b in bytes {
        if b == b'"' {
            out.push(b'"');
        }
        out.push(b);
    }
    out.push(b'"');
}

/// Writes `batch`, header and rows, to `out`. Returns the rows written.
///
/// # Errors
///
/// The output failed.
pub fn write_to(out: impl Write, batch: &Batch, options: &CsvOptions) -> Result<u64, TableError> {
    let mut w = CsvWriter::new(out, batch.schema(), options.clone())?;
    w.write(batch)?;
    Ok(w.finish()?.1)
}

/// Writes `batch` to the file at `path`, whole or not at all (see
/// [`tablekit::write_atomically`]); an existing file is replaced. Returns the rows written.
///
/// # Errors
///
/// The file could not be written.
pub fn write_file(path: &Path, batch: &Batch, options: &CsvOptions) -> Result<u64, TableError> {
    write_atomically(path, |out| write_to(out, batch, options))
}
