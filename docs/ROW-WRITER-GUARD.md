# Guarded and unguarded rows

`tablekit-csv` writes a row a cell at a time two ways. They write the same bytes and share
every cell method; they differ only in what happens to a row that is not finished.

| | `CsvFormat::begin_row` -> `RowWriter` | `CsvFormat::begin_row_unguarded` -> `UnguardedRowWriter` |
|---|---|---|
| Finished row (`finish()` returns `Ok`) | committed | committed |
| Refused row (wrong cell count; a cell of the wrong type in a debug build) | taken back | taken back |
| Row dropped before `finish()` | **taken back** | **left in the output**, with its separators and no line ending |
| Row abandoned by a panic | **taken back** during unwinding | **left in the output** |
| `std::mem::forget` | not taken back | not taken back |
| Cost on a hot path | a clean-up path on every call that may unwind | none |

`RowWriter` is the default and the right choice unless measurement says otherwise. It is
`UnguardedRowWriter` plus a guard: every cell method is `UnguardedRowWriter`'s, reached
through `Deref`. Both expose the same cell methods; a helper taking
`&mut UnguardedRowWriter` can serve both through dereference coercion. The types are not
interchangeable in every function signature, and their drop behavior remains different.

## Why the unguarded writer exists

A guard that runs on drop makes the compiler keep a clean-up path for every call in the row
that may unwind -- a growing buffer, a bounds check, a formatter -- so the row can be taken
back if it panics. In a row of dozens of inlined cells that path is part of the hot loop, and
it costs time even when nothing panics: it constrains how the cells are compiled.

Historical application-integration measurement: a 46-column protocol-record CSV exporter
performed whole-input decode and merge on 256 MiB recordings, on a GitHub-hosted x86 runner
with 2 CPUs and one rendering thread. Ratios compare the same application's pipeline with
its custom CSV writer; they are not isolated tablekit timings or a promise for other inputs.
The application-specific harness and recordings are not part of this repository:

| tablekit-csv | decode | merge |
|---|---|---|
| 0.7.0 (`RowWriter`) | 1.065, 1.067 | 1.017, 1.018 |
| 0.7.1 cell changes, `RowWriter` | 1.023, 1.024, 1.026 | 1.051, 1.054, 1.052 |
| 0.7.1 cell changes, guard removed | 0.999, 1.000, 0.997 | 0.999, 0.999 |
| 0.7.1, `UnguardedRowWriter` (application integration) | 1.014, 1.015, 1.016 | 1.006, 1.005 |

The first three rows ran on AMD EPYC runners (9V74 and 7763), the last on an Intel Xeon
6975P-C: GitHub assigns the CPU, so compare rows within a run rather than across them. The
output was byte-identical in every run, and the custom writer against itself read 1.000
(range 0.990-1.033). On aarch64 the guard's cost was within noise; the 2-CPU
x86 runner is where it shows.

## When the unguarded writer is safe

Only where an incomplete line can never reach a reader:

- the output is a temporary file renamed into place only when the whole export succeeds, so
  any error or panic leaves the destination untouched; or
- rows are rendered into a buffer that is written out only after every row in it has
  finished, and thrown away on any error or panic.

One integration pattern uses both safeguards: render a block into a buffer, write only
completed blocks, and commit the temporary file only after the entire export succeeds.
Validate fallible source conversions before starting a row where practical. This does not
replace the obligation to discard or explicitly truncate output after an abandoned row.

A caller that can recover from an abandoned row may also undo it by hand: note the buffer's
length before `begin_row_unguarded` and truncate to it.

Do not use the unguarded writer where a row's failure is handled and writing continues into
the same output -- that is what the guard is for.

## What is tested

`crates/tablekit-csv/tests/cell_formats.rs`:

- `unguarded_rows_write_what_python_formats` -- the pandas fixture `table-formats.csv`.
- `unguarded_guarded_and_row_write_the_same_bytes` -- 20,000 random rows of every type and
  format, both `na_rep`s, through `CsvFormat::row`, `RowWriter` and `UnguardedRowWriter`.
- `unguarded_unfinished_rows_leave_their_cells` -- a dropped row and a row abandoned by a
  panic leave exactly their cells; truncating undoes them; refused rows are taken back.
- `guarded_rows_still_roll_back_what_unguarded_rows_leave` -- the same sequence through
  `RowWriter` leaves nothing.
- `debug_builds_check_unguarded_cells` -- a debug build refuses a cell that does not suit its
  column, as `RowWriter` does.

The guarded writer's own tests (`unfinished_rows_roll_back_including_unwinding`,
`writer_reuses_buffer_after_errors_and_cancellation`) are unchanged, and the existing APIs
(`CsvFormat::row`, the batch writer, `CsvWriter`) run the same instructions as 0.7.0
(`examples/perf_existing.rs`, callgrind, 200,000 rows: row 348,324,215 against 348,324,205;
batch 346,473,892 against 346,473,869).
