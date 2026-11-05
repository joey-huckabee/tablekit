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
