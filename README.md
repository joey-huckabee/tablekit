# tablekit

Tablekit is a Rust library for typed tables and consistent CSV and Parquet output.
Build named columns with explicit types and missing values, then write a batch, stream
batches, or format CSV rows into a reusable buffer. CSV defaults follow pandas on Windows;
Parquet uses explicit, lossless type mappings compatible with the bundled Impala/Hive target.

Use it for reporting and spreadsheet exports, converting records into columnar files, or
sharing one output contract across multiple applications. Input parsing and interpretation
stay in your application; the table model has no external dependencies.

- **Batch exports:** construct a `Batch` and use the file or buffer helpers.
- **Streaming tables:** implement `Source` to supply batches without holding a whole table.
- **Fixed-schema CSV:** write cells directly with guarded or explicitly unguarded row writers.
  Fast cell methods validate type/format agreement in debug builds only; use `CsvFormat::row`
  for fully checked dynamic rows.

See the [crate and integration map](docs/PROJECT-MAP.md) for boundaries and planned work.
[Target profiles](docs/PROFILES.md) document the bundled SQL/Parquet mappings and their
current compatibility restrictions.

| Crate | What it does |
|---|---|
| `tablekit` | The table model: named, typed columns, missing values, batches; writing a file whole or not at all |
| `tablekit-csv` | CSV, as pandas writes it by default ([docs/CSV-FORMAT.md](docs/CSV-FORMAT.md)) |
| `tablekit-parquet` | Parquet, with Impala's types ([docs/PARQUET.md](docs/PARQUET.md)) |
| `tablekit-profile` | Validated TOML shared by SQL generation and Parquet writing: types, SQL comments and schema policy ([docs/PROFILES.md](docs/PROFILES.md)) |

| Document | What it covers |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Why one library, where it sits, how an export runs, where concurrency will go, the decisions |
| [docs/CSV-FORMAT.md](docs/CSV-FORMAT.md) | Every rule of the CSV output, its options, and where it differs from pandas |
| [docs/PARQUET.md](docs/PARQUET.md) | The Parquet types and file layout, and column names for Impala |
| [ROADMAP.md](ROADMAP.md) | What comes next |
| [AGENTS.md](AGENTS.md) | The rules for working in this repository |
| `cargo doc --open` | The API, with runnable examples |

## Crate versions and compatibility

The current workspace uses `tablekit-csv` 0.7.1, model/tablekit and Parquet 0.6.0, and
`tablekit-profile` 0.6.1. Independent crate versioning preserves the Parquet writer's metadata
when only CSV changes. Since CSV 0.7.0, exhaustive `CsvOptions` literals need `cell_formats`
or `..CsvOptions::default()`. See [CHANGELOG.md](CHANGELOG.md),
[CSV 0.7.0 validation](docs/CSV-070-VALIDATION.md) and
[guarded/unguarded rows](docs/ROW-WRITER-GUARD.md).

## Use

Add the crates you need, pinned to a commit:

```toml
[dependencies]
tablekit = { git = "https://github.com/joey-huckabee/tablekit", rev = "COMMIT" }
tablekit-csv = { git = "https://github.com/joey-huckabee/tablekit", rev = "COMMIT" }
tablekit-parquet = { git = "https://github.com/joey-huckabee/tablekit", rev = "COMMIT" }
```

Replace `COMMIT` with a reviewed repository revision. Use the same revision for multiple
tablekit crates so their shared Rust types are compatible. An application must maintain and
audit its own lockfile; a dependency repository's Cargo.lock does not constrain it.

```rust
use tablekit::{Batch, Column, Values};

let batch = Batch::new(vec![
    Column::new("time", Values::F64(vec![0.5, 1.0])),
    Column::with_missing("Speed", Values::U16(vec![120, 0]), vec![true, false])?,
    Column::new("Valid", Values::Bool(vec![true, false])),
])?;
tablekit_csv::write_file("out.csv".as_ref(), &batch, &tablekit_csv::CsvOptions::default())?;
tablekit_parquet::write_file("out.parquet".as_ref(), &batch, &tablekit_parquet::ParquetOptions::default())?;
```

Every call returns `Result<_, tablekit::TableError>`.

For a table too large to hold at once, implement `tablekit::Source` (a batch at a time) and
write it with `tablekit_csv::write_source_file` or `tablekit_parquet::write_source_file`; or
drive `CsvWriter` and `ParquetWriter` yourself. A tool that writes CSV as it reads each
record uses `tablekit_csv::CsvFormat`, a row at a time into its own buffer; one whose columns
are fixed in its code can write each row a cell at a time with `CsvFormat::begin_row`
(`RowWriter`), avoiding an intermediate `Value` array; where the whole output is discarded
on any error, `CsvFormat::begin_row_unguarded` drops the rollback of an
unfinished row and its cost ([docs/ROW-WRITER-GUARD.md](docs/ROW-WRITER-GUARD.md)). A column
can be written in hexadecimal or with fixed decimals (`CsvOptions::cell_formats`,
[docs/CSV-FORMAT.md](docs/CSV-FORMAT.md#cell-formats)).

Every change that touches the writing path is measured against what it replaces, A against
B on the same input, byte-identical output, wall time and callgrind instruction counts; a
change that is slower is not merged. CHANGELOG.md records the figures.

## Build and test

Rust 1.85 or later, on 64-bit targets. GitHub Actions checks stable on Linux and Windows,
and Rust 1.85.0 on Linux. See [docs/CI.md](docs/CI.md) for the matrix, dependency audit policy
and action pins. Run these before pushing:

```bash
git config core.hooksPath .githooks       # once per clone
cargo +stable fmt --all --check
cargo +stable clippy --locked --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo +stable doc --locked --workspace --no-deps
cargo +stable test --locked --workspace --release
cargo +1.85.0 build --locked --workspace --all-targets
cargo +1.85.0 test --locked --workspace
```

The expected files come from pandas and NumPy themselves. To regenerate them (pandas 3,
NumPy 2), run `python make.py` in `testdata/expected/csv/` and in `testdata/floats/`.

Before a release, read a Parquet file tablekit wrote with pyarrow
(`pandas.read_parquet`), as an independent check of the format.
