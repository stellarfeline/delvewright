# ADR-0017: Toolchain distribution — `cargo install delvec`, a checksum-verified release shelf, and CI as the only publisher

- **Status**: Accepted
- **Date**: 2026-08-06
- **Source**: owner decision in conversation, 2026-08-06
- **Refines**: ADR-0014 (creator distribution), ADR-0016 (three-layer versioning)

## Context

ADR-0016 made `delvec` a versioned engine (`v<semver>`, tags + GitHub Releases,
from v1.0.0) and ADR-0014 said a future plugin bootstrap fetches "pinned,
checksum-verified multi-platform binaries from GitHub Releases". Neither shelf
existed: release `v1.0.0` carried **zero assets**, the compiler package was
`delvewright-compiler` with `publish = false`, and nothing had ever been
published to crates.io. So the engine pin a campaign release records, and the
`requires.delvec` window the `/new-delve` skill declares, both bound to nothing
a creator could obtain.

`cargo publish` is a one-way door: a version can never be reused and a name can
never be freed (`cargo yank` only stops new dependents selecting it; the bytes
stay downloadable forever). That asymmetry — tags and release assets are cheap
and mutable, the registry is permanent — drives every choice below.

## Decision

### 1. crates.io identity is `delvec`

`cargo install` resolves by CRATE name, never by binary name, so the package is
renamed `delvewright-compiler` → **`delvec`**. The LIBRARY target keeps the name
`delvewright_compiler`, so the 366 in-tree `use delvewright_compiler::` paths do
not churn for zero gain; an external dependent writes `delvec = "1"` and
`use delvewright_compiler::…`.

### 2. `delvewright-dsl` is published, on its own version line

`delvec` depends on it, so it must exist on crates.io first. It is **not** on the
engine version line and starts at `0.1.0`, with `delvec` declaring an exact
`=0.1.0` requirement so one `delvec` version resolves one dsl build.

Lockstep was rejected for a concrete reason: publication can half-succeed (dsl
lands, `delvec` fails), which burns that dsl version permanently, and under
lockstep the retry would have to move `delvec` too — putting the engine out of
step with the git tag ADR-0016 pins it to. With an independent line a burned dsl
version costs a dsl bump and nothing else. `0.x` is also the honest semver: it is
the format the engine speaks, not a library anyone should depend on directly.

Every other workspace member keeps `publish = false`, asserted in both
directions so a new crate cannot drift onto the registry.

### 3. The release shelf is `delvec` on five targets — `delve-render` is not on it

`x86_64`/`aarch64-unknown-linux-musl`, `x86_64`/`aarch64-apple-darwin`,
`x86_64-pc-windows-msvc`; one `.tar.gz` per target (binary + LICENSE) plus a
`SHA256SUMS` file, on the `v<semver>` release. Linux is **musl-static** so a
download has no glibc floor; the list lives in `versions.toml [engine].targets`
and nothing else carries a copy of it.

ADR-0014 named `delvec`/`delve-render`. This **narrows that to `delvec`**:
`delve-render` needs a GPU/driver stack and the EULA-gated Minecraft client jar
for textures, neither of which a downloaded binary can carry, so it would be a
shelf item that fails for most downloaders. Its own dependency (`nucleation`,
pinned by git rev) also makes it unpublishable to crates.io by construction.
Renders are validation artifacts that never ship in a delve, and the skill's
visual-review step already tolerates its absence.
**Revisit trigger**: ADR-0014's M4 bootstrap, which can arrange a host's
textures and driver, is the point at which shipping `delve-render` becomes
honest.

### 4. CI is the only publisher — including the first publish

No human runs `cargo publish` or uploads an asset, ever, and there is no
hand-published v1: a *failed* publish costs nothing and is retryable, only a
*successful wrong* publish is irreversible, and that risk is lower from a clean
tagged checkout than from a working tree. A publish path whose first real
exercise is some later release with nobody watching is not a proven path.

`.github/workflows/engine-release.yml` fires on a `v<semver>` tag. An accidental
publish is prevented **by construction**, not by convention:

- `CARGO_REGISTRY_TOKEN` is a **GitHub Environment secret** on an environment
  (`crates-io`) with **required reviewers**. Exactly one job declares that
  environment, so it is the only job in the repository that can obtain the
  token, and the run physically pauses for approval before it starts.
- The tag name must equal `[engine].version` in the tagged tree, and the tagged
  commit must be an ancestor of `main` — the approval prompt shows a ref, not a
  diff, so this is what makes the published thing reviewed, CI-green history.
- The registry step runs only after the whole shelf has built.
- The upload is **idempotent by checksum**: a version already on the index with
  byte-identical contents is skipped, and one with *different* contents is a hard
  failure by name. That is what makes a half-succeeded publish safely retryable.

### 5. `v1.0.0` is re-tagged onto the commit that carries this machinery

**Owner ruling, 2026-08-06.** The `v1.0.0` tag and its GitHub Release already
existed, with zero assets, on a commit that predates any of the above — so the
shelf could not be filled at that tag without inventing an engine version the
engine had not changed to justify. The tag is therefore **force-moved** onto the
commit that merges this ADR (`git tag -f v1.0.0 && git push -f origin v1.0.0`).
Bumping to `1.0.1` instead was considered and **declined**: it would leave
`v1.0.0` — the version ADR-0016 names as the engine's starting point, and the
version campaign pins and the skill's `verified_with` already refer to — an
empty shelf forever.

Moving a published tag in a public repository is a deliberate act and is only
safe because the engine at `v1.0.0` is unchanged. That is not a claim, it is
these four observations, recorded so a future reader can re-check them rather
than take the ruling on trust:

1. **No compiler source moved.** `git diff origin/main...<merge> -- 'crates/*/src/'`
   is empty. The whole `crates/` diff is two `Cargo.toml` files, one README, and
   one doc-comment line in `crates/compiler/examples/gen_hello_room.rs` — an
   example, which `[package] exclude` keeps out of the published crate anyway.
2. **The version the binary reports is untouched.** `DELVEC_VERSION` is
   `env!("CARGO_PKG_VERSION")` and `[package] version` stayed `1.0.0`, so
   `manifest.json`'s `delvec_version` and every storybook marker still resolve to
   the same string.
3. **Byte-identity is asserted, not assumed.** The ADR-0006 double-build gates in
   `crates/compiler/tests/cli.rs` — `build_is_byte_identical_across_runs`,
   `keep_crawl_builds_and_double_build_is_byte_identical`,
   `v04_showcase_double_build_is_byte_identical` — are green, alongside the full
   `cargo test --workspace` (153 test binaries).
4. **All eleven required checks were green on the merging PR** (#318, Actions run
   `31082398909`), including `tier 2` (datapack load + the generated PackTest
   suite), which boots the emitted delve.

Only the release identity moves; a delve built at the old `v1.0.0` and one built
at the new one are byte-identical, which is the property that makes the move a
bookkeeping change rather than a silent re-release.

**This is a one-time act, not a precedent.** It is available only because
`v1.0.0` had no assets and therefore no downloader could hold bytes that
disagree with the tag. Once the shelf is filled, a released tag is as immutable
in practice as a crates.io version: re-tagging would leave published archives
and checksums describing a commit the tag no longer names. Future engine
releases move forward by version.

### 6. Agreement is a red, not an intention

`versions.toml [engine]` is the single source for the engine version, the crate
names, the dsl requirement, the toolchain, and the shelf.
`validation/check-versions.sh` binds all of them to the manifests, the workspace
and the release matrix; `tools/check-skill-version.py` already binds the skill's
window; the release workflow binds the git tag. Every gate states its binding
count.

## Consequences

- `cargo install delvec` becomes the creator-facing install, and `cargo build`
  in a pipeline checkout becomes one of several true paths — the skill and
  `docs/reference/tools.md` say so.
- Adding a workspace crate now requires `publish = false` (or an `[engine]`
  entry), and adding a dependency that does not cross-compile fails on the PR
  that adds it, not at release time.
- ADR-0014's M4 bootstrap has the shelf it was written against, in the shape it
  described.

### What happened in practice — 2026-08-08, first release (v1.1.0)

The Decision above is unchanged and stands; this records where its **realisation**
diverged, because §4's phrase "prevented **by construction**, not by convention"
turned out to describe two halves with very different footing.

The containment half held exactly as written: `CARGO_REGISTRY_TOKEN` existed only
as an environment secret, the repository had no repository-level secrets, and
exactly one job in the whole repository declared an environment — so no other job
could ever obtain the token.

The approval half did not exist. The `crates-io` environment had been created to
hold the secret, but its required-reviewer rule was never saved
(`protection_rules: []`). Nothing paused. A `v1.1.0` tag push published
`delvewright-dsl 0.1.0` and `delvec 1.1.0` to crates.io with no review, through a
job named "publish to crates.io (owner approval)". The artifacts were verified
correct afterwards — published tree byte-identical to the tag, both distribution
channels compiling the island campaign identically — so the payload was right and
only the control failed. crates.io versions cannot be deleted, so v1.1.0 stands
and the tag must not be re-cut.

The generalisable part is not "someone forgot a checkbox". It is that **every
element that made this gate look real lived in the repository, and the single
element that made it bind did not** — the same out-of-band shape that
`tools/check-required-contexts.py` exists for one door further out. A gate whose
binding no artifact can observe is indistinguishable from a vacuous one, and this
project's own doctrine already says a green gate that binds to nothing is not a
pass.

So §4 now has a repository-side realisation, and the obligation is keyed to
`environment:` — the object class — rather than to the job that was burned:

- `tools/assert-run-approved.sh` is the first step of any environment-gated job.
  It reads the run's own approval history and refuses when no approval names the
  environment. A run that was never held records none.
- `tools/check-approval-gates.py` (CI, required) fails any environment-gated job
  that lacks that assertion, asserts a *different* environment than it declares,
  runs anything before it, or omits `actions: read`.

## Revisit triggers

- `delve-render` gains a self-contained runtime story (see §3).
- crates.io publishes a scoped-token model that would let the release token be
  narrowed further than `publish-update` on two crates.
