# ADR-0025: One package — `delvec` is the engine, `delvewright-dsl` is the format, and nothing else is published

- **Status**: Accepted
- **Date**: 2026-09-07
- **Source**: the ruling that the workspace has two members and a feature
  never adds a crate; measured against the tree at the revision this ADR was
  written from.
- **Supersedes, in part**: ADR-0023 §6 (both channels publish the same eight
  crates). Every other section of ADR-0023 stands.

## Context

ADR-0023 §6 put eight crates on crates.io: the format crate on its own version
line, and seven engine crates on one line, `=`-pinned to each other, uploaded
in dependency order. What that bought was a crate boundary per capability. What
it cost was carried on every push and every release:

- seven `=` requirements held in step by a gate, and a publish that could
  half-succeed across eight uploads, retried by a script that compares the
  registry's tarball against ours file by file;
- eight front pages a stranger reads, each bound by two gates to the build's
  constants;
- a packaging gate that assembled a private registry of seven sibling tarballs
  to prove the eighth built standing alone;
- a coverage gate that resolved a diagnostic constant by which crate a test
  imported it from, because two crates reused one constant name;
- a map of names — which crate holds the compiler, which the admission, which
  the renderer — that ADR-0023 §3 had already refused to make a creator carry
  at the command line, still carried by every `use` path, every reference page
  that named a crate, and every reader.

None of the boundaries encoded a capability boundary. A subcommand is named
after its object, never its crate (ADR-0023 §3); the crates depended on each
other in one direction with the binary at the top; and nothing but the binary
consumed any of them. The boundary also invited the one thing the constitution
forbids here: a feature arriving as a crate.

## Decision

1. **Two crates are published, and exactly two: `delvewright-dsl` and
   `delvec`.** `delvewright-dsl` defines the campaign format and numbers its own
   line (ADR-0024). `delvec` is the engine — the compiler, the grammar back end,
   schematic conversion, prefab admission, playtest harvesting and the GPU
   render arms — as one library with one binary, on the engine line. Why two
   rather than one: the format's number is the `dsl_version` and moves on its
   own clock; the engine's number is the release. Why two rather than more:
   `cargo install delvec` resolves its dependencies on crates.io, so every crate
   the binary is assembled from must be public — a private crate cannot be one
   — and every public crate is a version to hold in step, a page to bind, a
   tarball to prove and a name to carry. The count of published crates equals
   the count of crates, so the count of crates is the smallest that keeps the
   two version lines apart.
2. **A feature never adds a crate.** It adds a module under
   `crates/delvec/src/`, or a subcommand of the one binary. `versions.toml
   [engine].crates` is the closed publish set, in publish order;
   `validation/check-versions.sh` holds it equal to `[dsl_crate, crate]` and to
   the workspace's members both ways, and refuses a third member by name.
3. **The former crates are modules of `delvec`, named as they were**:
   `delvec::compiler`, `delvec::grammar`, `delvec::schem`, `delvec::admit`,
   `delvec::orchestrator`, `delvec::render`. A path that read
   `delvewright_grammar::…` reads `delvec::grammar::…`. Their integration tests
   are `crates/delvec/tests/<module>_*.rs`; their unit tests moved with their
   modules; the compiler's `build.rs` and its vendored `data/` sit at the
   package root; the ported grammar core's `LICENSE-GDMC25` sits beside the
   package manifest, in the one package that carries the port.
4. **The crates.io gap at v1.2.0.** The v1.2.0 release filled the archive
   shelf and published nothing to crates.io: the registry's newest `delvec` is
   an earlier version assembled from the seven-crate shape, and `cargo install
   delvec` installs that until the next engine version. Both channels re-align
   at that version, published from one tree in this shape — `delvewright-dsl`
   at its current number where the registry lacks it, then `delvec`. The seven
   library crates already on the registry stay as they are (a published
   version is permanent) and nothing is ever published under their names again.
5. **The measured build cost.** On the development machine (aarch64 macOS,
   rustc 1.97.1, dev profile, `cargo build -p delvec`, three runs each, wall
   seconds; CPU user seconds in parentheses):

   | | before (eight crates) | after (two crates) |
   |---|---|---|
   | cold build (target dir removed) | 109.8 · 99.9 · 95.6 (708.6 · 688.5 · 673.4) | 108.9 · 104.4 · 103.9 (666.4 · 650.6 · 660.3) |
   | rebuild after `touch` of the compiler's `emit.rs` | 4.0 · 2.8 · 2.4 (2.44 · 2.40 · 2.43) | 3.3 · 2.9 · 2.8 (2.65 · 2.66 · 2.69) |
   | rebuild after one comment line appended to `emit.rs` | 3.9 · 2.4 · 2.4 (2.33 · 2.38 · 2.47) | 2.8 · 2.6 · 2.6 (2.54 · 2.53 · 2.53) |

   The median cold wall is 4.5% slower (99.9 → 104.4 s) at 4% less CPU (688.5 → 660.3 user seconds): one large crate parallelises less than eight but does less total work. A rebuild after a touch or a one-line edit rebuilds one crate where it rebuilt four (compiler, admit, render, delvec), and rustc's incremental cache makes both cheap — 2.4–4.0 s before, 2.6–3.3 s after, with about 0.3 s more user time per run in the one-crate shape. Neither direction moves the edit-compile-run loop by a second.

## Consequences

- The six library READMEs go with their crates; the two gates that derive the
  crates.io page set from the manifests find two pages.
- `tools/check-publishable.sh` packages two crates and rebuilds `delvec` from
  the two tarballs alone; `tools/crates-io-publish.sh` uploads
  `delvewright-dsl`, then `delvec`; `.github/workflows/engine-release.yml`
  publishes in that order. `[workspace.dependencies]` holds one entry.
- `tools/check-dw-codes.py` resolves a diagnostic constant per module, through
  `pub use` re-exports, because one crate now holds every diagnostic module and
  a name-keyed table would let one constant shadow another.
- The prefab generator workspaces still source-include the engine's
  `schem::fluid` and the format's `blockshape` by path; the paths moved, the
  arrangement did not.
- ADR-0023 §6 is superseded; its §1–§5 and §7–§10 stand. ADR-0017's publishing
  machinery stands, applied to two crates.

## Revisit triggers

- A consumer other than the binary needs the engine as a library on a version
  line of its own. That is a crate, and it pays every cost this ADR names,
  knowingly.
- The format's number and the engine's number stop moving on separate clocks:
  then the second crate is a candidate to fold too.
