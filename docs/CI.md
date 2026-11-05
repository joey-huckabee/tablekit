# Rust checks

The owner approved GitHub Actions for this private repository, replacing the earlier
local-only testing rule. Local checks still run before pushing. CI uses the checked-in
synthetic fixtures; it never fetches reference repositories or production data.

## Coverage

| Runner | Toolchain | Checks |
|---|---|---|
| `ubuntu-24.04` (x64) | stable | Formatting; Clippy on every workspace target with warnings denied; workspace docs with warnings denied; release unit, integration, expected-file and documentation tests |
| `windows-2025` (x64 MSVC) | stable | The same release tests, including documentation examples, CSV bytes and atomic file behavior |
| `ubuntu-24.04` (x64) | 1.85.0 | Build every workspace target; debug unit, integration, expected-file and documentation tests |

All dependency-resolving commands use `--locked`. Compiler and rustdoc warnings are denied
in every Rust job. Stable deliberately follows upstream; the job prints the exact compiler
and Cargo versions. Update the MSRV cell and the local commands together if the workspace's
`rust-version` changes. Formatting and Clippy run once, on stable Linux, to avoid duplicate
checks. `cargo test --workspace` includes doctests; `--all-targets` would omit them.

There are four workspace crates and no optional crate features. The Parquet dependency
disables its default/Arrow features and explicitly enables Snappy, Rust-backend gzip and
Zstandard. These are exercised by the existing compression round-trip tests. Zstandard's
native build uses the hosted runner's C toolchain (MSVC on Windows); no system compression
packages or extra feature combinations are needed. `.gitattributes` keeps sources LF and
expected files byte-exact even on Windows. Linux and Windows cover the current file-output
portability needs; this matrix does not claim macOS or 32-bit validation.

The Rust workflow runs on PRs targeting `main`, pushes to `main`, and manual dispatch.
It checks drafts as well. Branch pushes are not also tested, avoiding duplicate runs when
a PR is open. There are no path filters, so documentation examples and policy changes get
checked. Each matrix job has a 25-minute timeout and two Cargo build workers. Superseded
runs on the same PR/ref are cancelled. No cache or artifact upload is needed initially;
measure run times before adding either. Benchmarks, fixture regeneration and the independent
pyarrow release check stay local and are not duplicated in CI.

## Dependency policy

Parquet and its compression libraries bring transitive dependencies, making a RustSec audit
useful even though the table model has none. The separate dependency workflow runs once per
PR, once on `main` pushes, manually, and weekly on Monday at 07:23 UTC. The weekly audit
detects new advisories without rebuilding the Rust matrix; its timeout is ten minutes.

`cargo-deny` 0.20.2 checks only **advisories and sources** against `deny.toml`:

- RustSec vulnerability and unsound advisories fail the check. Unmaintained advisories
  and yanked versions are reported for review; they are not blanket crate bans.
- Only crates.io is approved for external dependencies. New Git or registry sources require
  a reviewed policy change. Workspace path dependencies are allowed.
- There are no advisory exceptions. Any future exception must name the advisory, explain
  why it is safe for tablekit and give a review/removal condition beside the entry.

License allowlists, duplicate-version bans and a second `cargo-audit` job add no justified
gate here and are deliberately omitted. A failure from an unavailable advisory database is
an infrastructure failure, not evidence that dependencies are safe; rerun once it recovers.
Reproduce locally with `cargo deny --locked --workspace check advisories sources` using
version 0.20.2. This development tool adds no project dependency.

### Zstandard security update and consumer lockfiles

The workspace lockfile selects `zstd 0.13.4` and `zstd-safe 7.4.0`, fixing
`RUSTSEC-2026-0391` and `RUSTSEC-2026-0395` through `RUSTSEC-2026-0406`. The
[zstd advisory](https://rustsec.org/advisories/RUSTSEC-2026-0391.html) and
[zstd-safe advisory](https://rustsec.org/advisories/RUSTSEC-2026-0395.html) identify these
compatible fixed versions. All thirteen entries were checked against RustSec database
commit `965bf9ad083b58e6340d65829d85217b18be9290`; no advisory is ignored.

Only those two packages and checksums change. Parquet stays exactly 59.3.0; `zstd-sys`
stays 2.1.2+zstd.1.5.7 and the enabled features stay the same. Both updated wrappers declare
Rust 1.64 support; the workspace still requires and tests Rust 1.85. `zstd 0.13.4` requires
`zstd-safe >=7.4.0, <8`, so updating the former also raises the latter's minimum.

**This lockfile does not constrain consumers.** Cargo resolves Git/path libraries using the
consumer workspace's lockfile, not the library repository's lockfile. Parquet 59.3.0 accepts
`zstd ^0.13`; Parquet 59.3.1 has the same requirement, so that upgrade would not enforce a
patched minimum. No unused direct zstd dependency or Parquet change is added solely to
constrain a transitive dependency. An existing consumer lockfile can retain the affected
versions even after adopting a new tablekit Git revision. New resolutions ordinarily choose
a compatible current patch, but that is not a security guarantee; inspect and audit them.
An isolated Rust 1.85 path-consumer resolution reproduced this: with the updated library
checkout and an old consumer lockfile, Cargo retained 0.13.3/7.3.0. The targeted update
below changed just those wrappers to 0.13.4/7.4.0 in that consumer as well.

Read-only consumer snapshots inspected on 2026-11-05:

| Repository main commit | Lockfiles containing zstd 0.13.3 / zstd-safe 7.3.0 |
|---|---|
| FATMOPs-Binary `9dde0cb691e6b36813407eece76ea1c01fec101d` | `rust/Cargo.lock`, `rust/bakeoff/Cargo.lock`, `rust/fuzz/Cargo.lock` |
| sluice `ddad0f29f77e77b6aa1a1eb0c051442f19869a1f` | `Cargo.lock`, `fuzz/Cargo.lock` |
| CMM-Code-Generator `6a449d52e3c86b5947e3007dee720e50e790b611` | `Cargo.lock` |

Each listed lockfile needs its own reviewed update, tests and audit from the corresponding
workspace directory. With the inspected dependency graph, the minimal command is:

```bash
cargo +1.85.0 update -p zstd --precise 0.13.4
cargo +1.85.0 tree --locked -i zstd-safe
cargo deny --locked --workspace check advisories sources
```

Confirm zstd-safe resolves to 7.4.0 or a compatible fixed patch. Independent fuzz/bakeoff
workspaces may require their own toolchain and audit policy; use that repository's rules.
The inspected CMM fuzz and synthetic-tool lockfiles do not contain these codecs. These are
snapshot findings, not claims that downstream main stays unchanged. This PR changes none of
those repositories. The codec fix can be applied to their existing tablekit pins; when
adopting this PR's tablekit revision, keep all tablekit crate pins consistent and refresh
the consumer locks explicitly. A pin update alone does not carry this lockfile fix.

### Zstandard output compatibility

Compared the pre-update lockfile at `45bd7a098cf84bcbf34ff61a99c8ebe9f5e97065` with the
two-package update on the same Linux ARM64 host and stable Rust 1.99.1. A scratch Rust
harness wrote 131,072 synthetic rows in six columns: nullable booleans, signed/unsigned
integer boundaries, float boundaries and negative zero, nullable repeated Unicode/quoted
text and a varied integer sequence. It used 4096-byte data pages and 127-row encoder batches,
testing none/Snappy/gzip/Zstandard with dictionaries both off and on. Both versions read
back every row and compared it with the uncompressed baseline. All eight complete Parquet
files and the same-table CSV were byte-identical before/after; no expected file was changed.

| Zstandard output | Bytes, before and after | SHA-256, before and after |
|---|---|---|
| Dictionary off | 1,365,581 | `c5333e4cd7cb0aa071d77343c976e77024a028e5b6097dd0687452db84275c6b` |
| Dictionary on | 1,268,494 | `5714048ada232d40d736d4c2f88bdf0a12c1f82961d11b54c98cd71409ab0bdc` |

Sixteen measured in-memory Zstandard writes per variant after one warm-up gave median
39.750 ms before / 39.532 ms after with dictionaries off, and 37.740 ms before / 37.740 ms
after with dictionaries on. Input construction, correctness checks and disk output were
outside the timed region. This is a limited no-measured-regression comparison, not a
speedup claim or a guarantee for every workload; instruction counts were not collected.
The existing expected-file and compression round-trip tests remain the contract checks.

## Trust and maintenance

Both workflows grant only `contents: read` to the automatic GitHub token, disable checkout
credential persistence, and use ordinary `pull_request`, never `pull_request_target` or
privileged follow-up execution. They use hosted runners, no configured secrets, no external
analysis service, and no publishing or release steps. Neither workflow changes repository
settings. The dependency check downloads public registry/advisory metadata; it does not
upload source or fixtures to a service.

Reviewed action pins:

| Action | Release | Full commit |
|---|---|---|
| `actions/checkout` | v7.0.1 (Node 24) | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| `EmbarkStudios/cargo-deny-action` | v2.1.1 | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` |

The checkout action's inputs and Node runtime were reviewed at the pinned commit. The deny
action's `action.yml`, Dockerfile and entrypoint were reviewed: its base image is digest
pinned and its downloaded cargo-deny release is fixed at 0.20.2. That download uses HTTPS
without an additional checksum verification in the action; the full action SHA does not
make registry metadata, package mirrors or hosted runner images immutable. Review action
source and release notes before updating either pin. Rust is installed using the hosted
runner's `rustup`, without an extra toolchain action.

Reference inspection: `joey-huckabee/aero1553` at
`0dcb3e83da9d547bbf5ec5df7676f7dfed34d8db`, read-only. Its Linux/Windows Rust checks,
separate MSRV, docs and supply-chain checks informed this setup. Its publishing, binary
packaging, coverage, Python, performance and SonarCloud configuration are not part of
tablekit's CI. No dependency on that repository or its moving checkout was added.

Official references used to review the configuration:

- [GitHub workflow syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax)
- [GitHub secure use](https://docs.github.com/en/actions/reference/security/secure-use)
- [Checkout v7.0.1](https://github.com/actions/checkout/tree/3d3c42e5aac5ba805825da76410c181273ba90b1)
- [rustup installation](https://rust-lang.github.io/rustup/installation/index.html)
- [cargo-deny action v2.1.1](https://github.com/EmbarkStudios/cargo-deny-action/tree/3c6349835b2b7b196a839186cb8b78e02f7b5f25)
- [cargo-deny advisories](https://embarkstudios.github.io/cargo-deny/checks/advisories/cfg.html)
- [cargo-deny sources](https://embarkstudios.github.io/cargo-deny/checks/sources/cfg.html)
