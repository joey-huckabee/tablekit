//! Writes tables as CSV, in the one format every tablekit user shares:
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
//! Three ways in, one format:
//!
//! | | Takes | For |
//! |---|---|---|
//! | [`write_file`], [`write_to`] | a whole [`Batch`] | most uses; a file is written whole or not at all |
//! | [`CsvWriter`] | a table in several batches | tables too large to hold at once |
//! | [`CsvFormat`] | a row at a time, as borrowed [`Value`]s, into the caller's buffer | a tool that writes each record as it reads it, allocating nothing per row |

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
use std::str::FromStr;

use tablekit::{Batch, Field, TableError, Value, write_atomically};

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

/// How the text is encoded, named as Python names it (pandas' `encoding`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    /// `"utf-8"`: UTF-8, no byte-order mark. pandas' default, and the default.
    #[default]
    Utf8,
    /// `"utf-8-sig"`: UTF-8 starting with a byte-order mark, which Excel needs to show
    /// non-ASCII text correctly.
    Utf8Sig,
}

impl Encoding {
    /// Every encoding tablekit writes, by its Python name.
    pub const NAMES: [&'static str; 2] = ["utf-8", "utf-8-sig"];

    /// Its Python name: `"utf-8"` or `"utf-8-sig"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Utf8Sig => "utf-8-sig",
        }
    }

    /// What the file starts with.
    fn preamble(self) -> &'static [u8] {
        match self {
            Encoding::Utf8 => b"",
            Encoding::Utf8Sig => b"\xEF\xBB\xBF",
        }
    }
}

impl FromStr for Encoding {
    type Err = String;

    /// Reads a Python encoding name, as Python matches them: case and `-` or `_` aside
    /// (`"UTF-8"`, `"utf8"`, `"utf_8_sig"`).
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let key: String = name
            .chars()
            .filter(|c| !matches!(c, '-' | '_' | ' '))
            .flat_map(char::to_lowercase)
            .collect();
        match key.as_str() {
            "utf8" | "u8" => Ok(Encoding::Utf8),
            "utf8sig" => Ok(Encoding::Utf8Sig),
            _ => Err(format!(
                "tablekit writes the encodings {}, not {name:?}",
                Encoding::NAMES.map(|n| format!("{n:?}")).join(" and ")
            )),
        }
    }
}

/// The choices a user may make; [`CsvOptions::default`] is pandas' `to_csv` on Windows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CsvOptions {
    /// Written for a missing value, and for a floating-point NaN: pandas' `na_rep`. Default
    /// empty.
    pub na_rep: String,
    /// How each line ends. Default [`LineEnding::CrLf`].
    pub line_ending: LineEnding,
    /// How the text is encoded: pandas' `encoding`. Default [`Encoding::Utf8`].
    pub encoding: Encoding,
}

/// Bytes [`CsvWriter`] gathers before each write to its output.
const FLUSH_AT: usize = 1 << 20;

/// The CSV format for one table's columns: formats the header and each row into a buffer the
/// caller owns. [`CsvWriter`] is built on it; a tool that writes each record as it reads it
/// uses it directly.
///
/// ```
/// use tablekit::{DataType, Field, Value};
/// use tablekit_csv::{CsvFormat, CsvOptions};
///
/// let fields = vec![
///     Field { name: "label".into(), data_type: DataType::U32 },
///     Field { name: "time".into(), data_type: DataType::F64 },
/// ];
/// let mut format = CsvFormat::new(fields, CsvOptions::default())?;
/// let mut out = Vec::new();
/// format.start(&mut out);
/// format.row(&mut out, &[Value::U32(7), Value::F64(0.5)])?;
/// format.row(&mut out, &[Value::U32(8), Value::Missing])?;
/// assert_eq!(out, b"label,time\r\n7,0.5\r\n8,\r\n");
/// # Ok::<(), tablekit::TableError>(())
/// ```
#[derive(Clone, Debug)]
pub struct CsvFormat {
    schema: Vec<Field>,
    options: CsvOptions,
}

impl CsvFormat {
    /// The format for these columns.
    ///
    /// # Errors
    ///
    /// [`TableError::NoColumns`] for none.
    pub fn new(schema: Vec<Field>, options: CsvOptions) -> Result<Self, TableError> {
        if schema.is_empty() {
            return Err(TableError::NoColumns);
        }
        Ok(CsvFormat { schema, options })
    }

    /// The columns.
    #[must_use]
    pub fn schema(&self) -> &[Field] {
        &self.schema
    }

    /// Appends what the file starts with: the encoding's byte-order mark, if it has one, and
    /// the header row.
    pub fn start(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(self.options.encoding.preamble());
        let line_start = out.len();
        for (i, field) in self.schema.iter().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            text(out, &field.name);
        }
        self.end_row(out, line_start);
    }

    /// Appends one row: a value per column, each of the column's type or
    /// [`Value::Missing`].
    ///
    /// # Errors
    ///
    /// [`TableError::RowLength`] or [`TableError::ValueType`]; nothing is appended.
    // HOT PATH: per row.
    #[inline]
    pub fn row(&mut self, out: &mut Vec<u8>, row: &[Value<'_>]) -> Result<(), TableError> {
        if row.len() != self.schema.len() {
            return Err(row_length(self.schema.len(), row.len()));
        }
        // One pass: each value is checked as it is written, and a refused row is taken back.
        // Room for a typical row up front, so the pushes below do not grow the buffer.
        out.reserve(row.len() * 24 + 2);
        let line_start = out.len();
        for (i, (&value, field)) in row.iter().zip(&self.schema).enumerate() {
            if let Some(found) = value.data_type() {
                if found != field.data_type {
                    out.truncate(line_start);
                    return Err(value_type(field, found));
                }
            }
            if i > 0 {
                out.push(b',');
            }
            self.value(out, value);
        }
        self.end_row(out, line_start);
        Ok(())
    }

    /// Appends one row whose values are known to fit the columns.
    // HOT PATH: per row.
    #[inline]
    fn row_unchecked<'a>(&mut self, out: &mut Vec<u8>, row: impl Iterator<Item = Value<'a>>) {
        let line_start = out.len();
        for (i, value) in row.enumerate() {
            if i > 0 {
                out.push(b',');
            }
            self.value(out, value);
        }
        self.end_row(out, line_start);
    }

    /// Ends a row. A row of one empty field is written `""`, as Python's `csv` module (and so
    /// pandas) writes it, so the line is not mistaken for a blank one.
    #[inline]
    fn end_row(&self, out: &mut Vec<u8>, line_start: usize) {
        if self.schema.len() == 1 && out.len() == line_start {
            out.extend_from_slice(b"\"\"");
        }
        out.extend_from_slice(self.options.line_ending.bytes());
    }

    /// Appends NaN as `na_rep`, and infinity as `inf` or `-inf`.
    #[cold]
    fn not_finite(&self, out: &mut Vec<u8>, nan: bool, positive: bool) {
        if nan {
            text(out, &self.options.na_rep);
        } else {
            infinity(out, positive);
        }
    }

    /// Appends one value, or `na_rep` for a missing one or a NaN.
    // HOT PATH: per value. Inlined into `row`'s loop: as a call it cost about 5% of a dump.
    #[inline(always)]
    fn value(&self, out: &mut Vec<u8>, value: Value<'_>) {
        match value {
            Value::Missing => text(out, &self.options.na_rep),
            Value::Bool(b) => out.extend_from_slice(if b { b"True" } else { b"False" }),
            Value::I8(v) => integer(out, v),
            Value::I16(v) => integer(out, v),
            Value::I32(v) => integer(out, v),
            Value::I64(v) => integer(out, v),
            Value::U8(v) => integer(out, v),
            Value::U16(v) => integer(out, v),
            Value::U32(v) => integer(out, v),
            Value::U64(v) => integer(out, v),
            Value::F32(x) if x.is_finite() => float::write_f32(out, x),
            Value::F64(x) if x.is_finite() => float::write_f64(out, x),
            Value::F32(x) => self.not_finite(out, x.is_nan(), x > 0.0),
            Value::F64(x) => self.not_finite(out, x.is_nan(), x > 0.0),
            Value::Text(s) => text(out, s),
        }
    }
}

#[cold]
fn row_length(expected: usize, found: usize) -> TableError {
    TableError::RowLength { expected, found }
}

#[cold]
fn value_type(field: &Field, found: tablekit::DataType) -> TableError {
    TableError::ValueType {
        column: field.name.clone(),
        expected: field.data_type,
        found,
    }
}

/// Writes a table as CSV in one or more batches: the header first, then each batch's rows.
pub struct CsvWriter<W: Write> {
    out: W,
    format: CsvFormat,
    buffer: Vec<u8>,
    rows: u64,
}

impl<W: Write> CsvWriter<W> {
    /// Starts a table with these columns: writes the encoding's byte-order mark, if any, and
    /// the header.
    ///
    /// # Errors
    ///
    /// [`TableError::NoColumns`] for no columns, or the output failed.
    pub fn new(mut out: W, schema: Vec<Field>, options: CsvOptions) -> Result<Self, TableError> {
        let format = CsvFormat::new(schema, options)?;
        let mut buffer = Vec::with_capacity(FLUSH_AT + 4096);
        format.start(&mut buffer);
        out.write_all(&buffer)?;
        buffer.clear();
        Ok(CsvWriter {
            out,
            format,
            buffer,
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
        if found != self.format.schema {
            return Err(TableError::SchemaMismatch {
                expected: self.format.schema.clone(),
                found,
            });
        }
        let columns = batch.columns();
        // HOT PATH: begin. One line per row, one field per value, into the reused buffer.
        for row in 0..batch.rows() {
            self.format
                .row_unchecked(&mut self.buffer, columns.iter().map(|c| c.value(row)));
            if self.buffer.len() >= FLUSH_AT {
                self.out.write_all(&self.buffer)?;
                self.buffer.clear();
            }
        }
        // HOT PATH: end.
        self.rows += batch.rows() as u64;
        Ok(())
    }

    /// Appends one row of borrowed values (see [`CsvFormat::row`]).
    ///
    /// # Errors
    ///
    /// As [`CsvFormat::row`], or the output failed.
    // HOT PATH: per row.
    pub fn write_row(&mut self, row: &[Value<'_>]) -> Result<(), TableError> {
        self.format.row(&mut self.buffer, row)?;
        self.rows += 1;
        if self.buffer.len() >= FLUSH_AT {
            self.out.write_all(&self.buffer)?;
            self.buffer.clear();
        }
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
}

/// Appends an integer's decimal digits, formatted on the stack by `itoa`.
// HOT PATH: per value.
#[inline]
fn integer(out: &mut Vec<u8>, v: impl itoa::Integer) {
    out.extend_from_slice(itoa::Buffer::new().format(v).as_bytes());
}

#[inline]
fn infinity(out: &mut Vec<u8>, positive: bool) {
    out.extend_from_slice(if positive { b"inf" } else { b"-inf" });
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
