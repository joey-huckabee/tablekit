# CSV 0.7.0 candidate validation

This is release preparation, not evidence that v0.7.0 has been tagged. The coordinator owns
merge, final platform review and release. Only tablekit-csv advances to 0.7.0; tablekit and
Parquet stay 0.6.0, profile stays 0.6.1, and Parquet writer metadata stays `tablekit 0.6.0`.

## Reviewed behavior

- Duplicate named formats are refused even if the first entry is `Default` (reproduced
  before the fix). No expected file was regenerated or changed to conceal a difference.
- Quoted `text_with` counts quotes while building and expands backwards within its output
  allocation. With sufficient warmed capacity it makes no per-cell scratch allocation.
- An unfinished `RowWriter` rolls back on drop, including unwind. Successful `finish()`
  commits; count/type errors preserve previous rows. Deliberate `mem::forget` bypasses Drop.
- The public fast API retains the accepted **debug-only** method/type/format validation.
  Release builds write the method requested; they do not enforce the schema's format.
  Cell count and quoting remain checked in all builds. Dynamic callers use `CsvFormat::row`,
  which validates types in every build. No optional checking mode or new API was added.
- Tests cover all precisions 0–17, all hex widths 1–16, overflow beyond minimum hex width,
  numeric boundaries, UTF-8/ASCII replacement, empty and padded digits, quoting, cancellation,
  release-mode misuse, buffer reuse and batch/row/file/source writer agreement with the
  byte-exact synthetic pandas fixture. The precision cap is policy, not float significant
  digit precision. Both upper/lower hex formats are retained.
- Git hooks have executable mode 100755.

## ARM A/B measurements

Baseline: main `77cb72f8cc4b07e6f96fdf00188eda2d0fb53069`. Source and manifests of the CSV
and model crates match the earlier `c26e090` baseline; merged CI/security work changes no
CSV loop. Same `perf_existing.rs` source was compiled on both sides with stable Rust
1.99.1, release fat LTO, codegen-units=1 and release debug info enabled. Use **separate target
directories** for different snapshots: a shared target reused an incompatible 0.6.0 artifact
in an initial probe; that result was discarded and the baselines rebuilt independently.

Final variant: the five-byte boolean copy (`True\0`/`False`, trimmed before returning), plus
`inline(always)` on the existing unformatted `row` and `value` functions. The five-byte copy
alone was slower on ARM. These two hints were retained only after measuring the combination;
no other experimental inline changes remain.

Input: 200,000 invented rows, seven columns (Bool, I64, U64, F64, F32, U16, Text), including
quoted text and Unicode. The same input, defaults, row order and schema are used in every
mode. All six baseline/candidate outputs are identical: 12,105,974 bytes, FNV-1a
`b80f96ec7966d858`, SHA-256
`2011c6d46ba25b78af522a865e7f9f323a5abba69826e0af34b3e4d5b4f28a42`.

Linux ARM64, CPU affinity pinned to one available CPU. Twenty-one randomized-order paired
trials, one unmeasured warm-up and one measured render per process. Medians:

| Existing API | Main wall ms | Candidate wall ms | Wall ratio | Main Ir | Candidate Ir |
|---|---:|---:|---:|---:|---:|
| CsvFormat::row | 18.420542 | 18.333778 | 0.9953 | 344,477,827 | 348,326,895 |
| write_to batch | 19.721945 | 18.439129 | 0.9350 | 366,047,760 | 346,476,592 |
| CsvWriter batch | 19.766827 | 18.475552 | 0.9347 | 366,047,767 | 346,476,598 |

Row wall time is roughly neutral/slightly better; **row instructions increase 1.12%**.
Batch/writer wall times improve about 6.5%, and instructions fall about 5.35%. This is a
trade-off for review, not a claim that every metric improves or that arbitrary inputs cannot
regress. It supersedes the earlier author-head ARM/x86 instruction figures for this probe.
Valgrind 3.19 Callgrind Ir here is **whole process**, including deterministic input setup,
one render, checksum and output write; native wall timing excludes setup/checksum/I/O.
Do not compare these absolute counts with the author's render-only counts. Function-toggle
collection on ARM produced unreliable partial totals and was excluded; whole-process
collection avoids that call/return heuristic. x86 has different layout and CPU costs.

## Quoted-cell allocation evidence

Same `perf_quoted.rs`, author head `2b2be18b87c8cc5c9ca5563f2cf33b0f239e53b9` versus the
fixed implementation, separate target directories, 4096-byte reusable output, Unicode,
quotes, CR/LF and a padded u64. Each process first warms one row. Memcheck totals:

| Version | 0 measured rows | 10,000 measured rows | Additional allocations |
|---|---:|---:|---:|
| Author head (split_off) | 18 | 10,018 | 10,000 |
| Candidate | 18 | 18 | 0 |

Memcheck reports zero errors. The counts include process/setup allocations; the claim is
zero **additional** allocations for warmed rows, not an allocation-free process. Output
capacity/pointer checks and byte tests independently cover in-place expansion and prefixes.

## Windows evidence and remaining release gate

Coordinator-provided Windows x86 / Rust 1.99.0 measurements used seven randomized-order
trials with five warmed rounds each at 200,000 rows, comparing the scratch five-byte body on
ac355ae to baseline. Median ms, main/candidate: batch 40.6591/38.7886, writer
40.6600/39.2757, row 39.4814/37.7606. All bytes and the hash above matched. Those results
support the fixed-size copy, but **do not validate the final two inline hints**. The exact
pushed final source must be remeasured on Windows before release. The coordinator owns that
check and decides whether the disclosed instruction trade-off is acceptable; no gate is
silently waived. The author's earlier vendor-writer integration figures in CHANGELOG are
historical and were not reproduced using production/reference inputs here.

## Reproduction commands

Fetch main and the candidate; create separate worktrees BASE and CANDIDATE at the exact
commits being compared. Copy `crates/tablekit-csv/examples/perf_existing.rs` from candidate
into the same examples directory in BASE (the only baseline working-tree addition).
Build each separately, one at a time, with the same compiler and environment:

```bash
CARGO_BUILD_JOBS=2 CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_TARGET_DIR=/tmp/tk-base-target cargo +stable build --locked --release --manifest-path BASE/Cargo.toml -p tablekit-csv --example perf_existing
CARGO_BUILD_JOBS=2 CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_TARGET_DIR=/tmp/tk-candidate-target cargo +stable build --locked --release --manifest-path CANDIDATE/Cargo.toml -p tablekit-csv --example perf_existing
/tmp/tk-base-target/release/examples/perf_existing row 200000 5 /tmp/base.csv
/tmp/tk-candidate-target/release/examples/perf_existing row 200000 5 /tmp/candidate.csv
cmp /tmp/base.csv /tmp/candidate.csv
valgrind --tool=callgrind --callgrind-out-file=/tmp/base.cg /tmp/tk-base-target/release/examples/perf_existing row 200000 0 /tmp/base.csv
valgrind --tool=callgrind --callgrind-out-file=/tmp/candidate.cg /tmp/tk-candidate-target/release/examples/perf_existing row 200000 0 /tmp/candidate.csv
```

Repeat for `batch` and `writer`; randomize process order across at least seven pairs, five
warmed timed rounds per process. Keep compiler versions, profile and CPU affinity identical.
For allocation checks, build `perf_quoted` on the author head and candidate with isolated
targets; run `valgrind --tool=memcheck BINARY 0` and `BINARY 10000` under Memcheck and compare
`total heap usage`, not only the output Vec's capacity.

PowerShell equivalent for each build (set BASE/CANDIDATE to actual paths; use a different
CARGO_TARGET_DIR for each):

```powershell
$env:CARGO_BUILD_JOBS = '2'
$env:CARGO_PROFILE_RELEASE_DEBUG = '1'
$env:CARGO_TARGET_DIR = 'C:\Temp\tk-base-target'
cargo +1.99.0 build --locked --release --manifest-path BASE/Cargo.toml -p tablekit-csv --example perf_existing
$env:CARGO_TARGET_DIR = 'C:\Temp\tk-candidate-target'
cargo +1.99.0 build --locked --release --manifest-path CANDIDATE/Cargo.toml -p tablekit-csv --example perf_existing
& C:\Temp\tk-base-target\release\examples\perf_existing.exe row 200000 5 C:\Temp\base.csv
& C:\Temp\tk-candidate-target\release\examples\perf_existing.exe row 200000 5 C:\Temp\candidate.csv
Get-FileHash C:\Temp\base.csv -Algorithm SHA256
Get-FileHash C:\Temp\candidate.csv -Algorithm SHA256
```

Use the same compiler channel/version on both sides; 1.99.0 reproduces the coordinator's
Windows baseline. Run all three modes and interleave order; do not judge a single best run.

## Independent Parquet readback

The existing `tablekit-parquet` sample example wrote its invented all-types three-row
fixture. A separate cloud-only PyArrow 26.0.1 environment read it and checked all 13 column
names and physical/logical Arrow types (including signed widening for unsigned columns and
DECIMAL(20,0) for u64), every value, nulls, signed zero and Unicode/quoted text. All assertions
passed, including `created_by == "tablekit 0.6.0"`. No Python runtime/project dependency was
added. Reproduce by running `cargo run --locked --release -p tablekit-parquet --example sample -- DIR`
and reading DIR/sample.parquet with `pyarrow.parquet.read_table`; compare with the sample's
invented values and docs/PARQUET.md. Existing Rust codec round trips cover every enabled
compression option in the final suite.

## Downstream migration

Update consistent tablekit Git revisions in each consumer workspace and its separate native,
fuzz, bakeoff and tool workspaces. Add `cell_formats: Vec::new()` or the default update syntax
to exhaustive CsvOptions literals. Known sites at review time: FATMOPs-Binary
`rust/crates/fatmops-py/src/lib.rs`; sluice `python/native/src/lib.rs` and
`sluice/tests/summary.rs`. CMM's inspected handwritten sources had no CsvOptions literal.
Regenerate each consumer lockfile, verify the CSV crate alone is 0.7.0, audit zstd >=0.13.4 /
zstd-safe >=7.4.0, and run that repository's debug/release tests, docs and MSRV/native build
checks using synthetic inputs. A new tablekit pin does not import this repository's lockfile.
No downstream repository was changed by this worker.
