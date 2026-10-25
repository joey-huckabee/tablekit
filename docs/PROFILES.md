# Target profiles

`profiles/cloudera-impala-hive-v1.toml` is the single source of the baseline SQL and Parquet
type mappings. It also supplies SQL column comments, header and table-comment templates,
and the existing record and partition definitions. It contains no message definitions.
The CMM XML still supplies field names, paths, types, array lengths and enum definitions.
No XML additions are needed to reproduce the existing SQL comments.

The profile preserves the existing Impala SQL and Parquet schemas. Its name identifies a
baseline, not a certification of a particular cluster release. Validate it on the intended
Impala and Hive versions before deployment. Datetime interpretation remains deferred:
`SystemTime_T` resolves to `uint64`, which is stored exactly as `DECIMAL(20,0)`.

## Loading

The new `tablekit-profile` crate parses and validates the profile. Both CMM-Code-Generator
and tablekit-parquet use it. `DEFAULT_PARQUET_PROFILE` exposes the original bundled TOML text. Existing writer functions
use the bundled profile, embedded at
build time and parsed once. To edit a profile without rebuilding, load it explicitly:

```rust
use tablekit::{Batch, Column, Values};
use tablekit_parquet::{ParquetOptions, TargetProfile, write_file_with_profile};

# fn example() -> Result<(), Box<dyn std::error::Error>> {
let profile = TargetProfile::load("profiles/cloudera-impala-hive-v1.toml".as_ref())?;
let batch = Batch::new(vec![Column::new("ticks", Values::U64(vec![u64::MAX]))])?;
write_file_with_profile("out.parquet".as_ref(), &batch, &ParquetOptions::default(), &profile)?;
# Ok(())
# }
```

`ParquetWriter::new_with_profile` supports incremental writing. The matching
`write_to_with_profile`, `write_source_with_profile` and `write_source_file_with_profile`
functions cover buffers and streaming sources. Load one profile and pass that same object
to `cmm_codegen::impala::Impala::generate_with_profile` for the SQL.

In CMM-Code-Generator:

```powershell
cargo run --release -p cmm-gen -- generate --lang impala `
    ..\CMM_US_Description.xml --out C:\CMM-SQL `
    --profile ..\tablekit\profiles\cloudera-impala-hive-v1.toml
```

The regular `--database`, `--location` and `--force` options still apply. Invalid profiles
are refused before output directories are replaced. Omitting `--profile` uses the embedded
baseline. Edits to the bundled file require rebuilding unless it is passed explicitly.

## Rules and limits

- `schema_version = 1` describes the configuration format. Version the `id` whenever target
  behavior changes, and use the same profile version for SQL and Parquet generation.
- All twelve canonical source types must have explicit SQL, physical and logical mappings.
  Unknown keys, missing mappings, unsupported encodings of a type and SQL/Parquet mismatches
  are errors. Types are never chosen from the data, including empty or all-null columns.
- Conversions must preserve the full source range. `uint64` to `BIGINT` or `DOUBLE` is
  refused. The current decimal encoder supports exactly nine bytes, precision 20, scale 0.
- The current encoders allow `int8` to `TINYINT`, `SMALLINT` or `INT`; `int16` and `uint8`
  to `SMALLINT` or `INT`. Other types retain the representations in PARQUET.md. A conversion
  requiring a different value encoder must be implemented and tested before a profile may
  select it. This is capability validation, not an implicit conversion default.
- The first version preserves flattening, enum name/code columns, padding fields, lowercase
  identifiers, optional Parquet fields and the exporter record/partition layout. Incompatible
  layout changes are refused; configuring them alone cannot change the exporter.
- Compression and dictionary encoding remain writer options, separate from type mappings.
- Comment placeholders are validated, inserted once, and SQL-escaped by the generator.
  Field templates support `{source_path}`, `{xml_type}`, `{count}`. Header templates support
  `{package}`, `{version}`. Table comments support `{label}`, `{message_name}`,
  `{qualified_type}`, `{size}`. Unknown placeholders and unmatched braces are errors.

There are no StateVector-specific rules. The same profile applies to any checked CMM model.
SQL syntax and encoder implementations remain Rust code; configurable target policy and
comment wording live in TOML. Public compatibility constants used by existing consumers
are generated from that TOML during compilation.

## Integration and release

CMM-Code-Generator, FATMOPs-Binary and sluice consume tablekit-profile and the writers
through Git dependencies pinned to the same tablekit commit. No sibling source override
is needed. Update dependency pins in order: tablekit, CMM-Code-Generator, FATMOPs-Binary,
then sluice. The SQL generator and consumers must use the same profile version.

Cross-output tests generate SQL, write Parquet and inspect its footer for every primitive
type, empty and all-null columns. A custom profile changes uint8 to INT in both schemas
and changes SQL comment wording. Boundary round trips cover widening and the full uint64
range. Expected SQL files are unchanged.

Python Parquet exports accept profile=PATH; the FATMOPs export script and sluice load/dump
accept --profile PATH. Older installed packages must be upgraded for these options.

Validation on 2026-10-24 passed release workspace tests, Clippy and rustdoc in both
repositories, seven sluice load integration tests with local dependency overrides, and a
byte comparison of actual StateVector SQL against the approved local output (both default
and explicit profile selection). Invalid-profile rejection was checked with `--force`:
existing output remained intact. Parquet A/B output bytes also matched the previous writer;
timings are in CHANGELOG.md. Release validation also builds every tablekit target with
Rust 1.85.0. Callgrind instruction counts were not collected for this change.
