# ADR-0021: Creator toolchain re-derived — one distributed binary, registry Nucleation, an off-the-shelf viewer core

- **Status**: Proposed
- **Date**: 2026-08-13
- **Source**: owner request, 2026-08-13 — re-derive the toolchain shape under the
  dependencies as they are today, not as they were when ADR-0017/0018 were
  written. Owner rulings it rests on: the client jar is a creator prerequisite
  and visual validation is indispensable (2026-08-13); a download is never
  re-split to save the creator disk space (2026-08-13); binary size under
  100 MB and build time are not concerns (2026-08-13); client-jar download is
  the default and any disk scan is explicit opt-in (2026-08-13); every
  validation the pipeline needs must be runnable on the creator's own machine,
  with a source build as the guarantee and binary distribution only an
  optimisation of it (2026-08-14)
- **Refines**: ADR-0017 (its §3 revisit trigger has fired), ADR-0018
- **Constrained by**: ADR-0006 (determinism), ADR-0013 (licence allowlist),
  ADR-0010 (EULA: never redistribute Mojang jars)

## Context — what was re-measured, 2026-08-13

ADR-0017 §3 kept `delve-render` off the shelf on three grounds. Each was
re-checked, and each has moved:

1. **"Its `nucleation` git pin makes it unpublishable by construction."**
   Stale since **2026-08-12**: Nucleation **0.10.8** is on crates.io (MIT) and
   its published feature list carries `rendering = ["meshing", "dep:wgpu",
   "dep:image", "dep:pollster"]`
   (`curl https://crates.io/api/v1/crates/nucleation/0.10.8` → created
   `2026-08-12T09:48Z`). The published source exports the exact API this repo
   uses — `rendering::gpu::GpuRenderer` (headless `new`),
   `meshing::ResourcePackSource`, `camera::project_point` (grepped in the
   downloaded `.crate`). Demonstrated, not assumed: `crates/render` with only
   its dependency line changed to `nucleation = { version = "=0.10.8",
   features = ["rendering"] }` **compiles clean** (`cargo check`, 12 s warm)
   **and passes its whole test suite** — `cargo test`: 104 passed, 0 failed,
   plus all **5 `#[ignore]`d GPU tests** (`cargo test --test gpu --
   --ignored`: 5 passed in 1.25 s, real Metal adapter, real 1.21.11 client
   jar) — and its `Cargo.lock` then carries **zero** `git+` sources.
2. **"It needs the EULA-gated client jar."** The creator must have the client
   jar — visual validation is indispensable (owner ruling, 2026-08-13).
   Possession is now a premise, not a blocker; only the *acquisition
   mechanism* was unsettled (§5).
3. **"It needs a GPU/driver stack."** True of **three subcommands** — `piece`,
   `batch`, `fidelity-gate` — the only arms reaching
   `nucleation`/`wgpu` (`grep -ln nucleation crates/render/src/*.rs` →
   `main.rs`, `nbt.rs`, `render.rs`; `render.rs` is used by exactly those
   arms). `scene`, `panorama`, `contact-sheet`, `index` — and the interactive
   `viewer` of PR #392 — are CPU-only (JSON emission, PNG compositing, a
   self-contained HTML page). ADR-0017 §3 attributed a property of three
   subcommands to the whole binary.

Two more premises checked:

- **The authoring loop's required visual channel is already CPU and already
  inside `delvec`** (`snapshot` / `blocking-chart`;
  `docs/reference/distribution-size.md` §2). The skill's `delve-render` steps
  say "skip with a note if unavailable" (`SKILL.md`, visual-review step) —
  a degraded branch an indispensable visual channel no longer permits, the same
  shape ADR-0018 §3 refused for skins.
- **The size record replicates.** Same commands as `distribution-size.md`, this
  tree, 2026-08-13: stripped release `delvec` 8,756,464 B in 32.3 s (doc:
  8,756,512 B at `main` `374bbfb`); stable toolchain 1,477,376 KB and pinned
  1.97.1 + five shelf targets 2,108,396 KB (doc: identical).

## Decision (Proposed)

### 1. The distributed creator surface is ONE binary: `delvec`

The CPU render surface — `viewer`, `scene`, `panorama`, `contact-sheet`,
`index` — moves into `delvec` as subcommands. A creator obtains one binary by
the three existing paths (ADR-0017); nothing render-shaped is a second
download.

**This move drags no `nucleation`, and that fact is load-bearing** —
`nucleation` is not pure Rust (§3 blocker 1), so pulling it would red the
cross-build shelf gate on both musl targets. Verified at module level, not by
grep (`docs/notes/musl-static-dlopen-gpu-verification.md`): `render.rs` is
reached only from the `piece`/`batch`/`fidelity-gate` arms,
`nbt::build_schematic` — the one nucleation-typed function outside it — only
from `render.rs`, and the CPU arms reach `scene`/`panorama`/`sheet`/`index`
only. So `delvec`'s dependency graph stays nucleation-free under this section.

**No cargo feature may gate any subcommand.** A feature ships a
same-name-different-capability binary — the defect where the artifact's name
promises a surface its bytes do not carry. The surface is unconditional code,
and the proof is artifact-side, not conventional:
`tools/build-release-binaries.sh` runs each built binary and asserts its
`--help` subcommand list equals the clap surface parsed from source (the
parser `tools/check-skill-version.py` already has), per target, at build time.
`tools/check-publishable.sh`'s packaged-tarball build already proves the
crates.io bytes build the same binary.

### 2. `crates/render` re-enters the root workspace, on registry Nucleation

Dependency becomes `nucleation = { version = "=0.10.8", features =
["rendering"] }` and `versions.toml [render]` pins the registry version
instead of a git rev. The separate-workspace quarantine existed for exactly
one reason — a git dependency is cloned during *resolution*, so it taxed every
cargo command in the repo (`tools/check-workspace-git-deps.py`, PR #388) — and
a registry dependency does none of that (content-addressed, cached, no
per-command reach). The checker's own expiry arm then **forces** the
`ALLOWED` entry to be dropped: an allowlisted lock with no git dependency is a
red.

CI: the render workspace's duplicate `fmt`/`clippy`/`test` steps, its second
target-dir cache, and the named git-fetch step all collapse into the workspace
jobs. Workspace builds that touch the render crate then compile the wgpu stack;
build time is not a concern (owner ruling, 2026-08-13), so this is a note, not
an argument. The root `Cargo.lock` gains the nucleation/wgpu entries, but the
cross-build shelf gate compiles only `delvec`'s own graph
(`cargo check -p delvec --bin delvec`, `tools/build-release-binaries.sh`), so
§3's blocker 1 stays out of its reach.

### 3. The GPU arms are built from a checkout, not shipped on the shelf

`piece`/`batch`/`fidelity-gate` remain in `crates/render` (binary
`delve-render`) and are not part of the distributed archive. This is a
statement about *distribution*, not about capability: every validation the
pipeline needs is runnable on the creator's own machine, and the source build
is what guarantees that — a prebuilt binary is only an optimisation of it
(owner ruling, 2026-08-14). The skill's Init section is what makes the
guarantee real: it establishes a complete toolchain before work begins, and
where the shelf cannot carry an arm, Init builds it from the checkout at the
step that needs it rather than leaving the step to degrade.

Two grounds for keeping that split:

- **Nothing on the creator's required path needs a GPU.** `delvec
  snapshot`/`blocking-chart` (layout) and the `viewer` page (interiors, player
  POV, real models per §4) are CPU, and Chunky via `scene`/`panorama` is
  already out-of-process. What genuinely needs the GPU arms is the massing
  channel — `contact-sheet` composites what `batch` renders — and the fidelity
  gate; those are steps the creator reaches from a checkout, which the previous
  paragraph makes unconditional. So the CPU surface is not the whole visual
  channel — it is the part that ships prebuilt.
- The shelf's Linux targets are **musl-static** on purpose (no glibc floor),
  and a fully static musl binary cannot `dlopen` a Vulkan loader, which is how
  wgpu reaches a Linux GPU. **Verified on a real Linux host, end to end with
  the real crate** (`docs/notes/musl-static-dlopen-gpu-verification.md`): a
  crate-free `dlopen` probe under the shelf's exact `RUSTFLAGS` answers
  `Dynamic loading not supported` on both static-musl targets while the gnu
  control succeeds; the real render crate on `=0.10.8` reports `DW0723 gpu
  init: NoGpuAdapter` (exit 5) where the glibc build of the same source, same
  container, same lavapipe passes the fidelity gate; `strace` shows the static
  binary opens **0** Vulkan/EGL/GL objects against the control's 164. The
  cause is `crt-static`, isolated by a native-Alpine reproduction.

Two further blockers, both hit **before** `dlopen` is ever reached and both
stronger than the claim above (same note, "Beyond the claim"):

1. **`nucleation` is not pure Rust.** `blake3 1.8.6` is a non-optional
   dependency — a bare `nucleation = "=0.10.8"` pulls it, `rendering` or not —
   and its build script compiles C: `cargo check --target
   aarch64-unknown-linux-musl` fails with `failed to find tool
   "aarch64-linux-musl-gcc"`. This contradicts the "whole dependency set is
   pure Rust" premise recorded beside `versions.toml [engine].targets` (that
   comment stays true of `delvec` only because §1 keeps nucleation out of its
   graph; the implementing PR re-words it to say so). Build scripts run under
   `cargo check`, so the `engine binaries (cross-build shelf)` gate reds on
   both musl targets the moment `delvec` acquires **any** nucleation
   dependency.
2. **The shelf's linker cannot resolve `-ldl`.** `libloading` emits `-ldl`,
   and rustc's self-contained musl sysroot carries no `libdl.a` (musl folds
   `dl` into libc); the verification's static build linked only because a
   Debian `musl-dev` empty `libdl.a` was injected by hand. Nothing in the
   release recipe supplies that.

The falsifying branch was measured and is **decided against**: a *dynamically
linked* musl build passes the fidelity gate with a PNG byte-identical to the
glibc one across two distros, so dropping `crt-static` does buy Linux GPU
rendering — at the price of a binary that no longer starts on a machine without
musl's loader, which is the exact property the shelf exists for. **`crt-static`
stays.** The trade is refused because it pays with the shelf's only reason to
exist and buys something the creator already has by another route: source build
is the guarantee of completeness (owner ruling, 2026-08-14), so a Linux creator
who needs the GPU arms builds them, and the archive keeps its
no-loader-floor property for everyone who does not.

### 4. The viewer's rendering core is `deepslate` (MIT), not hand-written WebGL

PR #392 carries 1,069 lines of bespoke WebGL (`crates/render/src/viewer/page.js`)
drawing each blockstate as a mean-colour box. An off-the-shelf core exists:

| | **deepslate** (misode) |
|---|---|
| what | TypeScript library rendering Minecraft structures in-browser (WebGL) from real blockstate/model/texture resources; reads structure NBT |
| licence | MIT (npm metadata + repo) — ADR-0013 allowlist |
| maintained | npm 0.26.0 published 2026-05-20; repo push same day; 250 stars |
| deps | `gl-matrix`, `md5`, `pako` |
| needs client jar | resources are caller-supplied — built from the creator's own jar, which is a declared prerequisite, never fetched, never redistributed |

Adopting it replaces the bespoke mesher/renderer with maintained code and
**raises** fidelity (real block models instead of coloured boxes). The page
stays a single self-contained file: the pinned deepslate bundle and the
jar-derived resources are embedded, so byte-determinism is unchanged (same
pinned bytes in → same page out). Costs, named: a jar→resources extraction
step (the crate already reads the jar as a zip for `blockcolor`); the
`docs/ACKNOWLEDGEMENTS.md` entry (MIT, verified above) in the adopting PR; and
a **gate before adoption** — a spike rendering the committed prefab fixtures at
1.21.11 from jar-derived resources, judged against the current viewer's output.
The rest of this ADR does not depend on §4 either way.

**The spike has run and passed** (2026-08-13), across eleven prefabs plus a
49-block torture fixture, and the owner has judged the pages and ruled for
adoption (2026-08-14). Three things it established that change this section
rather than merely confirming it:

- **The suspected failure category was the wrong one.** deepslate is not a plain
  model renderer — `SpecialRenderer.js` covers chests, signs, banners, bells,
  beds, skulls, shulker boxes, conduits, decorated pots and fluids. Of those,
  only **banners** fail — and the cause is upstream's, not the pinned game's:
  it requests `entity/banner/banner_base`, a path no Minecraft version has ever
  shipped. 1.21.11 has `entity/banner_base.png` (cloth plus the pole strip the
  renderer's own UVs address) at the top level, while `entity/banner/` holds
  only the 43 pattern textures. Shields carry the identical edit. Fixed by a
  local patch of the two texture ids, reported upstream, **not forked** (owner
  ruling, 2026-08-14): we look after our own build and do not commit to
  maintaining theirs.
- **Two data sources the client jar cannot supply**, and adoption depends on
  both: a `BlockPropertiesProvider` for **multipart** blocks — obtainable from
  the pinned *server* jar's `--reports` output, generated locally and never
  redistributed — and deepslate's private special-texture id table, where a
  wrong id fails **silently** as magenta. Three ids were wrong on the first pass
  and only an in-page mesh probe caught them, so that probe is bound to every
  version bump rather than to a doc line.
- **Two capabilities the swap deletes**, named so they are decided rather than
  discovered: biome tint (deepslate's colour table is fixed, so `--biome` and
  the colormap sampling stop affecting the picture) and `--palette`, the
  jar-free page. Both are accepted (owner ruling, 2026-08-14): biome tint does
  not carry any judgement this page exists to support, and the jar-free page is
  redundant once the jar is a declared prerequisite.

Measured cost: vendored bundle 287,920 B (82 KB gzipped, byte-identical across
repeated builds); pages 320–518 KB against 48–85 KB today; browser load 31 ms at
315 cells to 1,482 ms at 42,336; three consecutive builds byte-identical, so
ADR-0006 holds. `page.js` falls from 1,069 lines to 341 even with an atlas
packer deepslate does not ship; `blockcolor.rs` stays, because `palette`,
`snapshot`/`blocking-chart` and the fidelity gate all still use it.

Everything else surveyed, with the disqualifier: **WebSchematics**
(Apache-2.0, but archived June 2024 and `.schem`-only);
**Jopgood/minecraft-schematic-viewer** (no licence — not a licence);
**shulkr / Bloxelizer** (hosted services, no licence, nothing to vendor);
**Nucleation's own WASM bindings** (MIT, parsing/meshing — its rendering
half is native wgpu, not a browser core; it stays our native renderer);
**prismarine-viewer** (MIT, but renders a live world through a server + bot —
the wrong shape for reviewing a prefab file).

### 5. Client jar acquisition mirrors the server jar's settled mechanism

`versions.toml [minecraft]` gains `client_jar_url` / `client_jar_sha1` /
`client_jar_sha256` / `client_jar_size`, from piston-meta for 1.21.11
(fetched 2026-08-13: url
`https://piston-data.mojang.com/v1/objects/ba2df812…/client.jar`, sha1
`ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`, size 31,152,600 B — equal to the
jar already resolved on the dev machine; sha256 computed at pin time). A
fetch-once, hash-refusing cache in the exact shape of
`validation/server-bootstrap-cache.sh` fills the existing resolution ladder
(`--textures` / `$DELVEWRIGHT_CLIENT_JAR` / `~/.chunky/resources`). Fetching
from Mojang's CDN is what every launcher does; the jar is never committed,
baked, or redistributed (ADR-0010). **Download is the default and scanning the
creator's disk for launcher-installed jars happens only behind an explicit
opt-in flag** — this is an owner ruling (2026-08-13), not a proposal of this
ADR.

### 6. What stays non-Rust, and the criterion that decides

A piece stays non-Rust exactly when one of two things is true:

- **(a) it is an upstream wheel** — reimplementing it is the reinvention the
  owner forbids: Chunky (GPL-3.0 JAR, invoked out-of-process, never linked),
  mineflayer (TypeScript harness, validation tier only), `delve_skin`'s
  skinpy/skinview3d lineage (Python, ADR-0018 §3 owner ruling), deepslate
  (§4);
- **(b) its runtime is not ours to choose** — browser pages run JS; CI checks
  run where `python3` is the runner's guarantee and cross-platform is
  irrelevant (ADR-0018 §2's classification, unchanged).

Everything the creator invokes by hand remains one Rust binary; everything
else is either a wheel kept in its upstream language or CI-only Python.

## Alternatives rejected

- **Status quo** (render undistributed, own workspace, bespoke WebGL): every
  ground it rests on is stale or narrowed — see Context.
- **Full one-binary including the GPU arms**: blocked three ways on the shelf
  as it stands — verified dlopen failure under `crt-static`, nucleation's C
  build script with no musl cross-compiler in the recipe, and the missing
  `libdl.a` (§3) — and no creator need requires it, because the source build
  already guarantees completeness on the creator's machine. The dynamic-musl
  variant that would unblock it trades away the no-loader-floor property and is
  refused (§3).
- **`delve-render` as a second shelf item**: a second distributed binary and a
  second version line (ADR-0016 arms grow) for a tool whose creator-facing
  need §1 just absorbed.
- **Keep the hand-written WebGL**: 1,069 lines of bespoke renderer, lower
  fidelity than the maintained MIT wheel — the reinvention rule decides,
  subject to §4's spike gate.

## Consequences

- ADR-0017 §3's revisit trigger has fired; on acceptance this ADR supersedes
  that section's exclusion (the rest of ADR-0017 stands).
- The skill's visual-review step loses its skip-with-a-note branch: with the
  surface inside `delvec` and the jar a declared prerequisite, visual review
  becomes an unconditional step. What replaces the branch is the skill's Init
  section, which establishes the toolchain — including a source build of any
  arm the shelf does not carry — before authoring begins. Same-PR skill/docs
  sync per the tooling-sync rule;
  `docs/reference/{tools,distribution-size,compiler}.md` re-measure and
  re-describe in the implementing PRs.
- `delvec` grows by the CPU render surface — unmeasured until implemented.
  Binary size under 100 MB and build time are not concerns (owner ruling,
  2026-08-13), so the number is a record, not a gate: the implementing PR
  re-measures into `distribution-size.md` per that file's own convention.
- The shelf archives stay whole per target: a download is never re-split to
  save the creator disk space (owner ruling, 2026-08-13).
- PR #392 (viewer) and PR #422 (aimable camera) rebase onto whichever half of
  this lands first. **§4 changes the viewer's emitted-page contract and the
  `SCHEMA` id must bump** — an earlier draft of this ADR asserted the opposite,
  and the adoption spike disproved it: the payload goes from
  `{palette, RLE voxels, box}` to `{nbt, blockstates, models, textures, flags,
  defaults}`. Nothing about the two page formats is compatible.

## Revisit triggers

- A Linux shelf target stops being static-musl for a reason unrelated to
  rendering: that removes one of §3's three blockers and reopens the placement
  of the GPU arms — the other two (nucleation's C build script, the missing
  `libdl.a`) would still need their own answers. §3 refuses to make that change
  *for* the GPU arms; it does not prejudge a change made for another reason.
- What the verification left open: `x86_64` static musl was exercised with the
  `dlopen` probe only (mechanism is in libc, not the architecture, but the
  full render binary was not built for it), and no real GPU was involved
  anywhere (lavapipe is a software ICD) — driver-specific behaviour is
  unspoken for.
- Upstream deepslate takes the banner/shield texture ids back to paths the jar
  supplies → the local patch is dropped rather than carried.
- Nucleation's registry cadence stops carrying the API this repo needs → the
  separate-workspace quarantine plus `check-workspace-git-deps.py` allowlist
  is the recorded fallback, already proven to work.
