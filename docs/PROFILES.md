# Target profiles

`tablekit-profile` supplies explicit SQL/Parquet type mappings, comment templates and
record/partition declarations. `tablekit-parquet` uses the validated mappings to choose
physical and logical encodings. A caller's SQL generator can use the same profile to keep
its declarations consistent with the file schema. Tablekit does not generate SQL statements,
parse application schemas or infer types from sample values.

The bundled [cloudera-impala-hive-v1.toml](../profiles/cloudera-impala-hive-v1.toml) is a
compatibility baseline, not certification for a particular cluster release. Validate the
intended Impala/Hive versions before deployment. Applications own timestamp interpretation
and unit conversion: a profile cannot establish UTC or reinterpret integer clock values.

## Loading

`TargetProfile::load` parses and validates an explicit file. Default writer entry points use
the bundled profile embedded at build time and parsed once; `DEFAULT_PARQUET_PROFILE`
exposes the bundled TOML text. To load a profile without rebuilding:

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
functions cover buffers and streaming sources. A SQL generator can consume the same
`TargetProfile` object and the `tablekit-profile` APIs. Edits to the embedded TOML require a
rebuild unless the edited file is loaded explicitly.

## Rules and compatibility limits

- `schema_version = 1` is the configuration format. Use a distinct profile `id` when target
  policy changes, and use matching profile versions for SQL declarations and Parquet output.
- All twelve canonical source types require explicit SQL, physical and logical mappings.
  Unknown keys, missing mappings, unsupported encodings and SQL/Parquet mismatches are
  refused. Empty or all-null columns do not change the selected types.
- Full source ranges must be preserved. `uint64` to `BIGINT` or `DOUBLE` is refused; its
  decimal encoder requires nine bytes, precision 20 and scale 0.
- The encoders support `int8` to `TINYINT`, `SMALLINT` or `INT`, and `int16`/`uint8` to
  `SMALLINT` or `INT`. Other types retain the representations in [PARQUET.md](PARQUET.md).
  A different encoding needs an implementation and tests before a profile can select it.
- This version requires `path_separator = "_"`, the database prefix already present in the
  bundled profile, and `max_identifier` in 1–128. The prefix is a compatibility constraint,
  not a general database-naming option. Inspect the bundled TOML before integrating it.
- The configured record layout remains the bundled `record_time` column of type `float64`.
  Partition declarations can select/reorder only the known text columns listed below.
  These declarations are metadata for callers: the Parquet writer does not add record or
  partition columns to a batch, create directories or register a database table.
- Compression and dictionary settings are writer options, separate from type mappings.
- Placeholder names and braces are validated. Field templates accept `{source_path}`,
  `{xml_type}` and `{count}`; header templates accept `{package}` and `{version}`; table
  comments accept `{label}`, `{message_name}`, `{qualified_type}` and `{size}`. These are
  compatibility keys supplied by the caller, not built-in application schema parsing.
  SQL escaping when emitting statements remains the generator's responsibility.

The profile's identifiers and template defaults are retained for output compatibility.
They are not evidence that arbitrary database prefixes, record layouts, partition names or
SQL dialects are supported. Source-type validation and SQL syntax handling remain Rust code;
changing TOML cannot enable arbitrary lossy conversions. Public compatibility constants are
generated from the bundled profile during compilation.

## Partition selection and order

The bundled profile declares six text partitions: `venue`, `sdl_name`, `mission_date`,
`mission_desc`, `tail_name`, `test_num`. Custom profiles may keep an ordered subset of their
`[[sql.partition_columns]]` entries. For none, remove those entries and set
`partition_columns = []` under `[sql]`. Names must remain known and unique, with text types.
`TargetProfile::select_partitions` performs the same validated projection. It does not select
filesystem paths or change the fixed record declarations. A caller chooses whether and how
to use the declarations in directory names and SQL.

## Rendering into reusable buffers

`tablekit_profile::render_into(template, values, &mut output)` appends the same
single-pass substitutions as `render`, using caller-owned capacity. It does not clear
existing output. Clear a scratch string before replacing a comment and retain it while
processing a table. Inserted values are literal, including braces, Unicode, quotes and line
breaks. Duplicate supplied keys use the first match. Unknown placeholders and unmatched
braces return errors; a failed append may leave a partial prefix, which must be discarded.
The owned `render` API delegates to the same parser.

Tests compare both renderers with an independent frozen parser, including malformed input,
repeated keys and large/small buffer reuse. Retained capacity reflects the largest comment;
cumulative allocation savings alone do not imply lower peak memory. Count formatting is a
separate caller concern. Historical validation and measurements are in [CHANGELOG.md](../CHANGELOG.md).

## Integration guidance

Pin multiple tablekit crates to one repository revision for compatible shared Rust types.
A SQL generator and file writer should use the same profile version and explicit mapping.
Test schemas, boundary values, empty/all-null columns and unsupported-profile rejection.
Keep source-data decoding, clock conversion and filesystem/database operations in the
application. See the file helpers for atomic replacement guarantees.

The profile crate is versioned independently; changing only its implementation need not
change the model or Parquet writer version and `created_by` metadata. A library repository's
Cargo.lock does not control an application's lockfile: update and audit each independent
workspace. See [CI.md](CI.md#zstandard-security-update-and-consumer-lockfiles).
