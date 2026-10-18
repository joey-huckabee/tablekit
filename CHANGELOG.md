# Changelog

All notable changes to this repository are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases are tagged following
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). A change to what is written is
called out under **Format**.

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
