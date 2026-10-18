# Roadmap

What comes after the first release, in order. [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
has the design behind each step.

## 1. sluice writes through tablekit

`sluice dump --format csv` and `sluice summary --format csv` have their own CSV code. They
should build a table and write it with `tablekit-csv`, so their quoting, numbers and line
endings match every other tool's. Their output will change where it differs today; say so in
sluice's changelog.

## 2. Concurrent export (`tablekit-run`)

**Measure first.** Time each stage per label (read, decode, build the table, write) on a large
capture, through FATMOPs-Binary's `perf/`. Then:

- **A job is a source and a writer.** A source gives a table's rows in batches of a fixed size;
  the tool supplies it (FATMOPs-Binary: read and decode one label). The engine knows only
  jobs.
- **One job per worker**, up to the core count, largest first. `std::thread::scope`, no new
  dependency.
- **Without an index, one pass for all labels**: a single scan routes each record to its
  label's job.
- **Stopping**: a shared flag, set by Ctrl-C or the first error, checked between batches.
  Finished files stay; unfinished ones are removed (`write_atomically` already does this per
  file).
- **The same bytes either way**: a test writes with one worker and with many and compares.
- **Evaluated with and without threads** (owner, 2026-10-18): starting workers, handing
  batches between them and keeping the output in order cost something, and for a small
  table or a single label that can outweigh the gain. Time the serial and the concurrent
  engine side by side, from tiny tables to large ones, measure the fixed cost of starting
  the workers on its own, and use the concurrent one only where it wins (possibly above a
  size threshold). sluice's threaded dump (its roadmap) is held to the same test.
- **Its `perf/` case** in FATMOPs-Binary: serial against concurrent, with and without the
  index, Rust against Python.

## 3. Fewer copies

FATMOPs-Binary decodes a message into its own columns and then copies them into a tablekit
batch. Decoding straight into tablekit columns (in `cmm-decode`'s planned column-at-a-time
decoding) would remove that copy. Measure before changing.

## 4. Impala's types in one place

`tablekit-parquet` and CMM-Code-Generator's Impala generator each map unsigned types to
Impala's (`u8` to `SMALLINT`, `u64` to `DECIMAL(20,0)`). A test in CMM-Code-Generator should
check the two agree, once it depends on tablekit.
