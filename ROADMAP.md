# Roadmap

What comes next, in order. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) has the design
behind each step.

**Performance only improves** (owner, 2026-10-18): a change to a pipeline or a hot path is
measured against what it replaces (A against B on the same input, byte-identical output,
wall time and callgrind instruction counts) and is merged only if it is no slower.

## Done

- 0.1.0: the table model, CSV as pandas writes it, Parquet with Impala's types.
- 0.3: sluice's CSV through tablekit (`CsvFormat`, a row at a time); `encoding` replaces
  `bom`.
- 0.4.0: text columns in one buffer; `Source`, `BatchWriter` and the serial driver `copy`;
  floats by zmij. The CSV writer is 4.9 times faster than 0.3.0's.

## 1. Concurrent export (`tablekit-run`)

After the first release. **Measure first:** time each stage per label (read, decode, build
the table, write) on a large capture, through FATMOPs-Binary's `perf/`. Then:

- **A job is a source and a writer.** The engine knows only jobs; each tool supplies its
  sources (`Source`).
- **One job per worker**, up to the core count, largest first. `std::thread::scope`, no new
  dependency.
- **Without an index, one pass for all labels**: a single scan routes each record to its
  label's job.
- **Stopping**: a shared flag, set by Ctrl-C or the first error, checked between batches.
  Finished files stay; unfinished ones are removed.
- **The same bytes either way**: a test writes with one worker and with many and compares.
- **Evaluated with and without threads** (owner, 2026-10-18): starting workers, handing
  batches between them and keeping the output in order cost something, and for a small
  table or a single label that can outweigh the gain. Time the serial and the concurrent
  engine side by side, from tiny tables to large ones, measure the fixed cost of starting the
  workers on its own, and use the concurrent one only where it wins (possibly above a size
  threshold). sluice's threaded dump is held to the same test.

## 2. Parquet row groups from small batches

`ParquetWriter` writes a row group per batch. A source that gives small batches therefore
makes small row groups, which Impala reads slowly. Gather batches up to a target size (rows
or bytes) before writing a group, so any source makes good files.

## 3. Fewer copies

FATMOPs-Binary decodes a message into its own columns and then copies them into a tablekit
batch. Decoding straight into tablekit columns (with `cmm-decode`'s planned
column-at-a-time decoding) would remove that copy. Measure before changing.

## 4. Impala's types in one place

`tablekit-parquet` and CMM-Code-Generator's Impala generator each map unsigned types to
Impala's (`u8` to `SMALLINT`, `u64` to `DECIMAL(20,0)`). A test in CMM-Code-Generator should
check the two agree, once it depends on tablekit.

## 5. More encodings, when asked

`encoding` takes Python's names, so another encoding (`utf-16`, `latin-1`) can be added
without changing the interface, when a user needs one.
