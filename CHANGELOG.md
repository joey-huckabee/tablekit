# Changelog

All notable changes to this repository are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases are tagged following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). A change to what is written is
called out under **Format**.

## [0.3.2] - 2026-10-18

### Changed

- Floats are formatted by `zmij` (Victor Zverovich's Żmij algorithm, ported by David
  Tolnay) instead of `ryu`: about twice as fast for doubles (21.6 ms against 44.5 ms for a
  million record times) and 1.35 times for floats. It too takes the even digit on a tie.
  `ryu` is no longer a dependency. zmij is pinned exactly, since a new version could change
  digits; the NumPy corpus test must pass before moving it.
- A row is checked and written in one pass (a refused row is still taken back whole), its
  room is reserved once, values are formatted inline, and a float zmij already writes in
  NumPy's layout (the common case: `0.000921`, `123.0`) is copied as it is. With sluice
  writing rows a megabyte at a time, `sluice dump --format csv --hex 8` of a 738,433-record
  capture takes 0.473 s (best of nine), against 0.594 s with zmij alone and 0.633 s for
  sluice's hand-written CSV before tablekit.

### Format

- Unchanged: the NumPy corpus, the pandas expected files, and a 738,433-record sluice dump
  are byte-identical to 0.3.1's.

## [0.3.1] - 2026-10-18

### Changed

- CSV numbers are formatted by `ryu` (floats) and `itoa` (integers) on the stack, instead of
  through Rust's formatting machinery: a sluice dump row is formatted about as fast as
  sluice's own hand-written CSV was (0.640 s against 0.648 s for a 738,433-record capture,
  best of seven), where 0.3.0 took about half as long again. Ryu also takes the even digit
  when two shortest forms are equally close, as NumPy does, so 0.3.0's second formatting
  pass for that case is gone.

### Format

- Unchanged: the same bytes for every value, checked against the NumPy corpus and against
  0.3.0's output for the capture above.

## [0.3.0] - 2026-10-18

### Changed (breaking)

- `CsvOptions::bom` is replaced by `CsvOptions::encoding`, an `Encoding`: `Utf8` (the
  default) or `Utf8Sig`, parsed from Python's names (`"utf-8"`, `"utf-8-sig"`) as pandas'
  `encoding` takes them. Other encodings can be added without another interface change.

### Added

- `tablekit::Value`, one borrowed value of a row, and `Column::value(row)`.
- `CsvFormat`: formats the header and each row (a slice of `Value`s, checked against the
  columns) into the caller's buffer, for a tool that writes each record as it reads it with
  no allocation per row. `CsvWriter` is built on it, and gains `write_row`.
- `TableError::RowLength` and `TableError::ValueType`.

### Format

- Unchanged. The expected file `table-bom.csv` is now `table-utf-8-sig.csv`.

## [0.2.0] - 2026-10-18

### Added

- `ParquetOptions::dictionary` (default `true`, as before): `false` writes every value
  `PLAIN`, with no dictionary pages, for a reader that refuses the `RLE_DICTIONARY` label the
  parquet crate gives dictionary pages even in a version 1.0 file (an old Impala).

### Format

- Unchanged by default. `created_by` is now `tablekit 0.2.0`.

## [0.1.0] - 2026-10-18

The first release: single-threaded.

### Added

- `tablekit`: the table model. `Column` (a name, `Values` of one `DataType`, which rows are
  missing), `Batch` (equally long columns), `TableError`, and `write_atomically` (a file
  written whole or not at all).
- `tablekit-csv`: `write_file`, `write_to` and `CsvWriter` (a table in several batches).
  `CsvOptions`: `na_rep`, `line_ending`, `bom`.
- `tablekit-parquet`: `write_file`, `write_to` and `ParquetWriter` (a row group per batch).
  `ParquetOptions`: `compression`.
- `docs/ARCHITECTURE.md`, `docs/CSV-FORMAT.md`, `docs/PARQUET.md`.

### Format

- CSV is what pandas 3 writes with `to_csv(path, index=False)` on Windows: CRLF, no
  byte-order mark, `True`/`False`, empty for a missing value or NaN, minimal quoting, and
  floats as NumPy writes them. Checked byte for byte against files pandas wrote, and against
  about 16,000 floats NumPy formatted. Whole numbers stay whole when some are missing, where
  pandas would write `5.0`.
- Parquet has Impala's types: unsigned integers widened to the next signed type, `u64` as
  `DECIMAL(20,0)`, every column optional, Snappy compression.
