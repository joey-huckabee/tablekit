# Roadmap

[ARCHITECTURE.md](docs/ARCHITECTURE.md) explains the design and
[PROJECT-MAP.md](docs/PROJECT-MAP.md) identifies implemented APIs and extension boundaries.
The items below are plans, not delivery commitments.

Performance changes need A/B measurements on the same input: byte-identical output, wall
time and instruction counts. Record regressions and trade-offs explicitly; startup and
buffering costs matter as well as steady-state throughput. A measured slowdown blocks merge
until it is resolved; report metrics that trade off rather than hiding them.

## Implemented milestones

- 0.1.0: typed tables, pandas-compatible CSV and Parquet with lossless SQL-target mappings.
- 0.3.0: row-at-a-time `CsvFormat`; `encoding` replaces `bom`.
- 0.4.0: compact text columns, `Source`, `BatchWriter`, serial `copy` and zmij floats.
  The historical CSV benchmark was 4.9 times faster than 0.3.0; see [CHANGELOG.md](CHANGELOG.md).
- 0.5.0: validated target profiles with SQL/Parquet type mappings and comment templates.
- 0.6.0: shared Parquet compression, dictionary, page-size and encoder-batch controls;
  ordered selection of the profile's known partition declarations.
- Profile 0.6.1: append-only template rendering into reusable buffers.
- CSV 0.7.0: cell formats and a guarded fixed-schema row writer.
- CSV 0.7.1: an explicitly unguarded row writer and faster cell implementations.

## Concurrent export

The current driver is serial. A proposed `tablekit-run` engine would accept jobs consisting
of an application-supplied `Source` and a writer. Measure reading, transformation, table
construction and writing before deciding which stage benefits from concurrency.

Candidate requirements:

- Bound the number of active jobs and memory per batch.
- Evaluate largest-first scheduling when job sizes are known.
- Check cancellation between batches, keeping completed files and removing unfinished ones.
- Require identical bytes for serial and parallel execution.
- Compare tiny and large workloads, including thread startup and queue costs. Keep the serial
  path where concurrency does not pay for itself.

Applications sharing a single input scan may need routing by output table, rather than a
separate scan for each job. Input parsing, indexing and record classification remain outside
this library. No concurrent engine or scheduling API is implemented yet.

## Parquet row groups from small batches

`ParquetWriter` currently writes one row group per batch. Sources with small batches can
produce small row groups. Investigate gathering batches up to a target row or byte count,
with a defined memory limit, and measure the resulting read/write trade-offs.

## Fewer copies

Applications may already own column buffers before constructing a table. `Text::from_parts`
can transfer a packed text buffer without copying; other ownership-transfer patterns need
measurement and API design. Measure retained capacity as well as cumulative allocations.

## Target profiles

Shared SQL/Parquet mappings and template rendering are implemented. Current profile
validation intentionally restricts database-prefix, record-layout and partition choices;
see [PROFILES.md](docs/PROFILES.md). Broader target support requires encoder and validation
work, compatibility tests and explicit format decisions, not just a new TOML file.

## Storage controls and encodings

Snappy, none, gzip and Zstandard, dictionaries, data-page bytes and encoder-batch rows are
implemented Parquet options. Compression levels, dictionary-page sizing and row-group
coalescing are separate possible improvements. No claim of acceptance by every database
release follows from these controls.

CSV currently supports UTF-8 and UTF-8 with a byte-order mark. Additional encodings can be
considered when there is a concrete use case and expected-file coverage.

## Reusable template rendering

`render_into` preserves substitution and error behavior while appending into caller-owned
capacity. Further count formatting or compiled-template work needs separate measurements
and exact-output tests. Buffer retention and peak memory should be evaluated alongside
allocation savings.
