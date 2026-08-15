# Tool surface — every runnable tool in this repo

Live inventory of what an authoring, admission or validation session can actually
run today (CLAUDE.md *Tooling sync*). Nothing aspirational is listed: every
invocation below was executed. Semantics live in the per-tool references —
[`compiler.md`](compiler.md) for `delvec`, [`i18n.md`](i18n.md) for translation,
the crate READMEs for the rest; this page is the index and the flag surface.

Each entry carries a **class**, which decides how it enters a skill:

- **agent** — an LLM-facing workflow step. When the symptom appears, running it is
  not optional.
- **human** — human-in-the-loop. A skill mentions it in one line and moves on;
  never blocks.
- **CI** — a gate; a session runs it only to reproduce a red check locally.
- **spike** — one-off measurement rigs, not part of the shipped pipeline.

Rust binaries run from repo root as
`cargo run -q -p <package> --bin <bin> -- <args>` (packages below), or from a
`cargo build` target directory. The one exception is `delve-render`, whose crate
is its own workspace: it takes `--manifest-path crates/render/Cargo.toml` instead
of `-p` (§4).

**How you get `delvec`** (ADR-0017 — three true paths, pick by what you are
doing). Everything else in this file is pipeline-repo-only and has one path.

| Path | For | How |
|---|---|---|
| `cargo run`/`cargo build` in this repo | compiler/DSL development, and every CI job here | see below |
| `cargo install delvec` | authoring a campaign without a pipeline checkout | installs the crate `delvec` (the lib target inside it is still `delvewright_compiler`) |
| a release archive | pinned/offline installs, and ADR-0014's future plugin bootstrap | `delvec-v<version>-<target>.tar.gz` + `SHA256SUMS` on the `v<version>` GitHub Release, five targets (`versions.toml [engine].targets`) |

A release archive and `cargo install delvec@<version>` at the same version are
the same engine: both are built from the tag whose name equals
`versions.toml [engine].version`, and the release workflow refuses to run when
they disagree. They are not the same *bytes*: the archive is stripped and the
`cargo install` build is not (8,053,424 B vs 10,012,928 B at v1.1.0).

**How big is all this?** `delvec` is ~3 MB to download and ~8 MB installed; the
prefab library is another ~95 KB; nothing else in the authoring loop is large.
The measured inventory — per platform, what each install path costs, what the
validation tiers cost on top, and why the binary is 8 MB rather than 1 MB — is
[`distribution-size.md`](distribution-size.md). Read it before quoting a size.

**Profile.** Either form is fine: the workspace sets `[profile.dev] opt-level = 1`
so an ordinary `cargo build` / `cargo run` produces a `delvec` fast enough for a
real campaign (`nobodys-cave-island`: 46s). It is not optional decoration — at the
cargo default of `opt-level = 0` that same build takes 12m51s and reads as a hang.
Add `--release` only for a long unattended run (25s on the same campaign); it
costs ~20s per incremental rebuild, so it is the wrong choice while iterating.
All profiles emit byte-identical output (ADR-0006 is profile-independent, and
measured to be — `docs/notes/build-profile-measurements.md`).

---

## 1. `delvec` — the compiler (`crates/compiler`, package `delvec`) · agent

The only path from DSL to datapack (ADR-0001). Full behavior:
[`compiler.md`](compiler.md).

| Subcommand | Purpose | Key flags |
|---|---|---|
| `validate <dir>` | stage schema + referential validation | — |
| `analyze <dir>` | quest-graph reachability (implies `validate`) | — |
| `build <dir> -o <out>` | full deterministic build (implies `analyze`) | `-o/--out` (required) |
| `fmt <path>…` | canonical form for authored JSON — object keys sorted, **arrays never** | `--check` (report only; exit 1 if anything is off) |
| `schema --stage <n\|all>` | export a stage's JSON Schema | `--stage` (required) |
| `l10n-inventory <dir>` | l10n key inventory as JSON (translation input) | `--lang` |
| `snapshot <dir>` | one draft frame + scene manifest (spec-0015) | `--camera x,y,z,yaw,pitch[,fov]`, `--at <anchor>`, `--orbit <deg>`, `--dist <n>`, `--shot <id>`, `--labels`, `--width 960`, `--height 540`, `-o snapshot.png`, `--timing` |
| `blocking-chart <dir>` | per-elevation cutaway floor plans (spec-0015) | `-o blocking-chart`, `--timing` |
| `edit apply <dir>` | replay the stage-7 edit script, persist a green candidate | `--batch <file>`, `-o edit-shots` |
| `edit preview <dir>` | same replay + renders, never writes the campaign | `--batch <file>`, `-o edit-shots` |
| `calibrate <report>` | harvested shot proposals → `anchor + offset` DSL patch (spec-0019) | `--layout <creator-datapack/layout.json>` (required), `-o shot-patch.json` |

Global flags on every subcommand: `--json`, `--prefabs <dir>` (default
`campaigns/prefabs`), `--lang <code>` (default `en`), `--version`.
`build` with no `--lang` is the **release** build: it ships every declared
language inside the delve's resource pack and lets the client pick (i18n v2,
spec-0029). `--lang <code>` is the single-language bake for local dev — it swaps
the strings before emission and ships no lang files.
Exit codes and the `--json` diagnostic shape: [`compiler.md` §1](compiler.md).

**`delvec fmt` is a mandatory step, not a tidiness option.** Run it over the
campaign directory **after the last edit
of a stage document or an l10n sidecar and before committing** — a three-key
insertion into a non-canonical sidecar once produced a 103-insertion /
100-deletion diff, and that is what it exists to stop. It sorts object keys and
**never** touches array order (`quests[]`, `objectives[]`, `effects[]` are
ordered; sorting one changes the game), and it proves that on every file it
writes. Full canonical form, discovery rules and the `DW077x` codes:
[`compiler.md` §9](compiler.md).

```
delvec fmt campaigns/campaigns/<id>          # rewrite in place
delvec fmt --check campaigns/campaigns/<id>  # what CI asks
```

## 2. `delve-schem` — schematic import (`crates/schem`, package `delvewright-schem`) · agent

Converts a Sponge schematic (`.schem`, v2/v3) into a vanilla structure `.nbt`.
Step 1 of prefab admission. See [`../../crates/schem/README.md`](../../crates/schem/README.md).

```
delve-schem convert <input.schem> -o <out.nbt>
    [--split 48]          # max part size per axis (structure cap); oversize input
                          # is tiled into parts + a <base>.split.json manifest
    [--palette-report]    # print the full input block-state palette (audit feed)
    [--json]
```

Every palette entry it writes states **every** property the block has. A schematic
may spell out only the properties its author cared about; the template is stamped
at the pinned DataVersion, so the completion says out loud what vanilla would
otherwise fill in from the block's default state, and no reader of the file has to
carry a table of 1.21.11 defaults to know what a cell is. It happens at
`convert::build_region` — the workspace's one structure-template byte boundary,
which the grammar back end passes through too — rather than in each caller's
palette construction, so a new caller cannot be lossy by omission.

## 2a. `delve-grammar` — the box-split grammar back end (`crates/grammar`) · agent

**The entry point for making a prefab** (spec-0027 §3; the whole procedure is
[`prefab-procedure.md`](prefab-procedure.md)). Before it existed the crate's only
caller was `cargo test`, so a grammar prefab could not be produced without
writing Rust.

```
delve-grammar list                       # every library program, with its params and roles
delve-grammar show   --program <id>      # that program as the typed JSON IR (the corpus)
delve-grammar check  (--program <id> | --file <p.json>)
delve-grammar expand (--program <id> | --file <p.json>) --region XxYxZ -o <dir>
    [--seed N] [--param NAME=VALUE]... [--role ROLE=BLOCKSTATE]...   # a restyle keeps the role's axis frame
    [--id <prefab-id>] [--traversable [--allow-falls]] [--symmetric x|y|z]
    [--reachable-floor]
delve-grammar coverage [--json <path>]   # which IR constructs no example demonstrates
delve-grammar audit [--library] [--campaign-root <path>]... [--exclusions <path>]
```

**There is no maximum region.** A vanilla structure template holds 48 blocks per
axis; an expansion past that is written as a set of `≤48` tiles plus one
manifest, cut deterministically from the region alone. The cap is an internal
packaging detail and reaches no author and no flag (DEC-0069). `piece` and
`audit` below take that manifest and treat the zone as one thing; both refuse a
lone tile of a set and name the manifest instead.

**A program that declares a spatial contract is checked against its own blocks,
with no flag** (spec-0036). The nine obligations — coverage, closure, edge proof
per class with its declared `rise`, graph-confined per-cell reachability, the
computed out-of-walk kinds, anchors, exterior faces — run inside `expand`, each
with its binding count, and a red writes no `.nbt`. `--traversable` then reads
its claim off the declared `exterior` edges, so its binding count is doors.
Every opt-out the contract used is printed by name: which shelf is `posted` on
which anchors, which bar the walk had to open, which envelope claims sky.
`delve-admit audit` is the same checker's second door, for a piece nobody
generated.

`list` names an **`idiom-*` block**: eleven teaching programs, one per technique
of the IR plus one composition, each documented at a region and seed in
[`grammar.md`](grammar.md) §2c. `show --program idiom-shape` prints one, and it
is the fastest way to see how a taper, an erosion mix, a symmetric aperture or
one rule called with different content is actually written. Read that block
before starting a piece.

`delve-grammar audit` is the sweep: it expands and judges EVERY program of a
corpus at the expansion that corpus declares, prints a binding count per gate
plus the corpus-wide count of fills resolved out of the scope's own axis frame,
and writes nothing. `--library` walks the rule library, whose registry carries
each entry's region and seed; `--campaign-root <content repo>` walks every
`campaigns/<campaign>/design/programs/` there, driven by that campaign's own
`zones.json` manifest. It reds on a failed gate, on a gate that examined zero
objects, on a programs directory with no manifest, on a program file the manifest
does not name, and on a corpus that turned out empty. `--exclusions` takes the
record of programs that are known red with the exact codes each must fail with;
it inverts those assertions and never removes them, so a recorded program that
starts passing is a finding too. Ids are audit labels, so the record reaches both
corpora — `library/<program>` and `<campaign>/<zone>` — and it currently holds
one entry, `library/causeway` (`DW0800`), against a missing `nav` capability.
Both repos' CI run it — the pipeline repo's `campaign
builds` job against the pinned content SHA, and the content repo's `zone
programs` job against the branch under review, with no paths filter.

The two corpora are **counted apart** (`corpus: library N` / `corpus: campaign N
over R root(s)`). The rule library is the pipeline repo's own, so `--library` over
an empty one reds. A campaign root that carries no zone program is a fact about a
checkout — an in-progress campaign lives on a content development branch until the
owner has played it — so the run names that zero as a finding and stays green.
Which campaigns a pinned checkout is expected to carry, and how many zone programs
each declares, is enumerated in the pipeline repo's
`.github/content-zone-corpus.json` and checked against the tree by
`crates/grammar/tests/campaign_zones.rs` inside `cargo test`
([`grammar.md`](grammar.md) §4f). Counting the corpora together is what let a full
library carry an empty campaign root to a green board.

`--file` is the authoring form: a grammar program written as JSON, which is what
spec-0027 means by "the LLM authors rules". Every program declares its own
`version` (`grammar.md` §2); a version this build does not know, or a construct
newer than the version declared, is a refusal before any work is done. `check`
validates structure with no region and no seed — run it after every edit, and
budget it as a typo check rather than a review: every defect it can see is a
name, an arity or a version, and with no region it never sees geometry. A
mirrored rule called on both sides, a sconce course at the wrong height and a
parapet that blocks its own anchor all pass it. `expand` writes `<id>.nbt` (or
the tile set above), `<id>.json` (prefab metadata, with the program hash + seed
that regenerate the bytes) and `<id>.report.json` (the gate verdicts).

`<id>` is the prefab's identity — all three filenames and the datapack structure
path — so it is lowercase letters, digits and hyphens only. `--id` sets it;
otherwise it defaults to the library program id, or to the **input file's stem**
with `--file`. The program's `name` field is not the id: it identifies the
program in the metadata's provenance row. An unusable id is refused before the
expansion runs — nothing is written and no verdict is printed, because a `pass`
above a failure is the line a reader stops at.

Gates, each reporting its **binding count**: `blocks-exist` (every painted block
state exists in 1.21.11), `non-empty`, and three opt-in ones — `traversable` (a
body walks from the approach end to the exit end; `--allow-falls` for a piece
entered off a ledge), `symmetric` (the piece is its own mirror image across the
mid-plane of the named world axis, compared by presence rather than by block
state) and `reachable-floor` (every cell of floor **under a roof** can be walked
to from the grade entrance). A red gate writes **no** `.nbt`. Every gate judges
the whole expansion, tiled or not — a tile is a packaging unit and never a
semantic one, so binding counts stay zone-level. The verdict is printed only once
the prefab has been written, so every `pass` on the terminal is a `pass` about
files that exist.

`traversable` is a claim about the **route** and nothing more: both faces it
joins are at ground level, so a piece can pass it with every storey above the
floor stranded. The **reachability measurement** answers the other question and
runs on every expansion, flag or no flag — how much of the standable floor a body
reaches on foot from the grade entrance, how much of the rest sits under a roof
(a room with no way in) versus open to the sky (a roof, a parapet, a terrace: the
engine cannot tell which and never gates on them), how many disconnected pockets
there are, and the bounding box of the five worth walking to. `--reachable-floor`
turns the roofed half of that into a verdict, for a piece that claims a body can
get everywhere indoors.

**A one-way descent cannot make that claim and cannot be excused from it.** The
predicate that would say "a body gets down there and not back up"
(`nav::reachable_with_fall`) is library-internal and no CLI surface reaches it,
so a piece whose design is a one-way drop simply fails `--reachable-floor`
(`drop-shaft` 9×12×9 seed 1: 28 of 63 roofed cells unreached) and takes exit 4
with nothing written. Leave the flag off on such a piece and read the always-on
measurement, where the lower level is an `unreachable_sheltered` pocket.

Measurements — fill ratio, standable cells, footprint area/perimeter, silhouette
complexity, per-block shares, reachability — are reported with no threshold and
are deliberately not called gates: spec-0027 §4's craft gates are not built, and
`crates/grammar/src/gates.rs` says what blocks them.

`coverage` measures the **corpus**, not a program: `show --program` is where an
author starts, so an IR construct no library program writes does not exist in
practice however well the IR supports it. It counts every `Node` kind, every
`Cond` kind and every palette paint kind, prints each with its binding count,
names every zero as a finding, and exits `4` when any construct is
undemonstrated. `--json` writes the same report as a file. It measures
**demonstration, not expressiveness** — see [`grammar.md`](grammar.md) §8, and
the sentence the command itself prints on every run.

Exit: `0` ok · `2` input/usage · `3` output · `4` a gate went red.

## 3. `delve-admit` — prefab admission (`crates/admit`, package `delvewright-admit`) · agent + human

The gate every prefab passes before the library will place it: mechanical palette
audit (ADR-0013 licence discipline + code-injection forbid), socket carving,
anchors, lighting, catalog cards. See [`../../crates/admit/README.md`](../../crates/admit/README.md).

Admission order for an imported piece (**`resolve-jigsaw` runs before `socket`**):

```
delve-admit audit <nbt|manifest.json> [--allowlist <json>] [-o report.json]   # CI gate
    # also: the spatial contract's second door — a piece whose metadata declares
    # spaces is judged against its own bytes by the same checker `delve-grammar
    # expand` uses, and a disagreement is DW0782 (exit 1)
delve-admit resolve-jigsaw <nbt>                                # neutralize foreign worldgen markers
delve-admit socket <nbt> --pos x,y,z --facing north|south|east|west
                         [--opening 3,3] [--name keep:socket]
                         [--target keep:socket] [--pool keep:pool]
delve-admit anchor <nbt> --name anchor/<id>
                         [--pos x,y,z] [--facing <kw>]
                         [--region x1,y1,z1:x2,y2,z2] [--block <id>]
delve-admit lighting <nbt|manifest.json> [--write] [--dark-threshold 3]
delve-admit catalog validate <card.json ...>
```

`socket`, `anchor` and `lighting --write` each own **a named part** of the
prefab's metadata and rewrite the file with everything else — anchors, sockets,
licence, the `license.generated_by` row that says what regenerates the `.nbt`,
the declared `waterline_y`, and any key this version of the tool does not model —
as they found it. They can be run in any order and repeatedly. That holds because
these tools and the compiler share one definition of the document
([`prefab-procedure.md`](prefab-procedure.md) §9); a tool with its own copy of
the shape deletes whatever its copy omits, silently, on the way out.

The part is as deep as the edit really goes, and for `anchor` that is **four
fields of one anchor** rather than the anchor map: `pos`, `facing`, `region` and
`block` say where the anchor is, and re-annotating an anchor the piece already
carries leaves the rest of it alone — the `dispenser` cell and `trigger_block` a
trap's hardware lives on, the `resolves_to` the exporter resolved from the
piece's own contract, and any anchor key this version does not model. Naming a
`--pos` does supersede a `--region` and vice versa: where the anchor is, is one
property written two ways. `crates/admit/tests/metadata_preservation.rs` holds
every step to this, path by path, on a real export carrying every field at risk.

`lighting` measures the **minimum block light over the roofed floor a body can
walk to from outside**, and its report states the binding it took that minimum
over: `standable_cells` in the whole region box, of which `reachable_cells` on
foot from `entry_cells` at grade, of which `measured_cells` are roofed. A
free-standing building sits in a box with ground around it, so a minimum taken
over the box is the unlit outdoors every time — a verdict no lighting design can
change. A binding of **zero** is `DW0752` and fails the command: a sealed piece
has no player space to grade, and a pitch-dark crypt is exactly the piece that
would otherwise pass by having nothing to measure. `--write` refuses (`DW0753`)
when there is no metadata to write into, rather than manufacturing a skeleton
that claims `spdx: UNKNOWN` about an asset whose licence it has not established.

`audit` and `lighting` both take a **tile-set manifest** and treat the zone as
one thing: the tiles are reassembled, and light crosses a packaging plane like
any other cell. Handing any command **one tile** is `DW0739`, and the refusal
holds after the tile has been copied away from its manifest — a tile is
recognised by the name `<base>.x<i>y<j>z<k>.nbt` it carries, not by what happens
to sit in the directory beside it.

Gallery curation is the **human** half — the owner walks a browse world and leaves
notes; the agent only builds and harvests:

```
delve-admit gallery <dir-of-nbt> -o <out> [--id <gallery-id>] [--cols 4]
delve-admit curate <server.log> --layout <gallery-layout.json> [-o report.json]
delve-admit curate-merge <report.json> --catalog <catalog-dir>
```

`gallery` **refuses to write** a tree whose `.mcfunction` the pinned 1.21.11
server would not parse (`DW0760`, one diagnostic per offending line), checked
against the same vendored Brigadier tree `delvec` validates its own emission with.
One bad line costs the whole function: four legacy camelCase gamerules and a
`text_opacity:255b` are enough to make 1.21.11 drop `admit:load` and
`admit:finish` whole, leaving a gallery world with no objectives, nothing
forceloaded, no piece placed and no label summoned.

## 4. `delve-render` — render layer (`crates/render`, package `delvewright-render`) · agent

Textured prefab shot sets, the missing-texture fidelity gate, and Chunky scene
emission for whole-scene / player-POV review. Needs the 1.21.11 client jar via
`--textures` or `$DELVEWRIGHT_CLIENT_JAR`. See
[`../../crates/render/README.md`](../../crates/render/README.md).

**This one crate is its OWN cargo workspace**, so it is the one entry on this page
that `-p` does not reach: build and run it as
`cargo run -q --manifest-path crates/render/Cargo.toml --bin delve-render -- <args>`
(`cargo test --manifest-path crates/render/Cargo.toml` for its tests). It is the
only crate here with a git dependency — Nucleation, pinned by rev — and cargo
clones a git dependency while RESOLVING the workspace that declares it, so inside
the root workspace that clone was a precondition for every cargo command in the
repo, `cargo run -p delvec` included. Excluding it is what confines the reach to
the crate that needs it; `tools/check-workspace-git-deps.py` is what keeps it
confined.

```
delve-render piece <nbt|manifest.json> -o <dir> [--view SPEC]…
                                             # planned multi-angle set for one prefab, plus any
                                             #   camera you aim yourself
delve-render batch <prefab-dir> -o <dir> [--view SPEC]…
                                             # the same for a whole library
delve-render fidelity-gate [-o <dir>]        # FAIL if any missing-texture placeholder renders
delve-render scene <build-dir> -o <dir> [--world world]   # Chunky scene JSONs from render-plan.json
delve-render panorama <build-dir> -o <dir> [--world world] [--bearing se|sw|ne|nw] [--spp 300]
                                             # the whole-map 45° oblique release panorama
delve-render index <build-dir> -o <file>     # image <-> expect pairs for a reviewing agent
delve-render contact-sheet <dir> -o <sheet.png> [--scores scores.json] [--shot ext-se]
                                             # [--columns N] [--thumb 256] [--title T]
                                             # many candidates, ONE page, for the owner to curate
delve-render viewer <nbt|dir|manifest.json>... -o <page.html> [--title T]
                                             # ONE interactive page: a camera the reviewer drives,
                                             # every block drawn from the pinned version's own model
delve-render palette <nbt|dir>... -o <palette.json> [--biome minecraft:plains]
                                             # the derived per-blockstate colour/shape table
```

Global: `--json`, `--textures <path>`, `--size 1024`. Exit codes and the dark-shot
review policy: [`compiler.md` §5](compiler.md).

### `piece` / `batch` — the per-prefab set · agent runs it, human reads it

Three kinds of camera per prefab, and a `<stem>-shots.json` manifest naming every
one. Two are planned for you; the third you aim.

**The planned cameras cannot be aimed.** Yaw, pitch and field of view belong to
the shot kind; `--size` and `--textures` change the pixels and not the
viewpoint. `<stem>-shots.json` is the reference for what each one did — it
records kind, yaw, pitch and field of view per shot, and the eye height, on
every run. The consequence worth planning around: the four exteriors sit at yaw
45/135/225/315 and `top` looks straight down, so **the planned set contains no
square-on elevation of any face**, and its only level camera is the eye camera,
which stands inside the piece. A facade is otherwise only ever seen at a slant;
`--view` below is the one flag that photographs one flat-on.

**Orbit** (`ext-ne/-se/-sw/-nw`, `top`, `door-<i>`, `anchor-<name>`) fit
themselves to the model from outside: massing, silhouette, floor plan, where a
socket or an anchor sits. `top`, `door-*` and `anchor-*` strip the top Y layer so
an outside camera can see into a roofed piece.

**Eye** (`eye-<anchor>`) stand *inside* the piece — a body's eye at 1.62 above a
standing cell, at each declared anchor, looking along that anchor's own `facing`,
at Minecraft's first-person field of view. This is the only camera that shows
what a body in the piece sees, and it is what §5 of
[`prefab-procedure.md`](prefab-procedure.md) judges the scene against.

The eye cell is resolved, not assumed: the anchor's own cell when a body fits
there, else up to 3 blocks back along the facing (so the anchor's object stays in
frame), else the nearest open body cell that still has the anchor in front of it.
Anything but the anchor's own cell raises `DW0727` and is recorded in the
manifest with its offset. An anchor with no body cell in reach gets **no** eye
shot; that is `DW0727` too, and the run's summary line always states the binding
count (eye shots / eligible anchors / declared anchors). An eye shot that renders
as nothing but background is reported as an empty frame under the same code — the
anchor is aimed at nothing in the piece.

The manifest carries, per eye shot, the anchor and its declared cell, the
standing cell and offset, the camera point, the facing, whether the cell has a
floor, and how many open cells lie ahead before the view is stopped (and by
what). A camera that stepped back is invisible in its own frame, so it is written
down rather than implied.

**Views** (`--view`, repeatable) are cameras you aim, appended to the planned set
under a name you choose. A view is a **bearing** plus a **subject box**:

```sh
delve-render piece out/notre-dame.json -o shots/ \
    --view name=west-front,face=north \
    --view name=north-flank,face=west
```

| Key | Meaning |
|---|---|
| `face=` | `north\|south\|east\|west\|up\|down` — square-on at that face of the subject box |
| `yaw=` | any other bearing, in degrees (`face` and `yaw` are alternatives; one is required) |
| `of=` | the subject box: `model` (default) or a declared anchor's full name |
| `name=` | shot name and file stem; defaults to `view-<face>` / `view-yaw<deg>` |
| `pitch=` | degrees, default 0 — a face view is level |
| `fov=` | degrees, default 45 (the orbit lens, so a view is comparable with `ext-*`) |
| `zoom=` | 1 frames the whole framed box; >1 closer, <1 further back |
| `cutaway=` | `true` strips the top Y layer, as `top` and `anchor-*` do |

A `face=` view frames **that face**, not the whole box, which is what makes it a
usable elevation of a deep building: the west front of a 31×64×93 cathedral fills
the frame at `zoom=1` instead of retreating behind ninety blocks of nave. `of=`
narrows the framed box further, so `face=east,of=anchor/altar` is a close-up of
one anchor's east side.

This is the only camera in the set that is **square-on at a face**. A building
whose identity is one elevation — a west front, a gatehouse, a castle's approach
face — has no picture in the planned set, and the near workaround does not work:
a level eye camera with a 70° field reaches ≈ `0.7 × distance` above eye height,
so framing a 20-block front needs ≈26 blocks of standoff, and a forecourt long
enough to hold it shrinks the building in every orbit frame instead.

A view is refused, before a single frame is rendered, when its spec is malformed,
when it names a subject the piece does not declare (the error lists the anchors
that exist), or when its name is already a planned shot's — which would overwrite
that image (`DW0721`, exit 2). A view that renders as nothing but background is
reported as an empty frame under `DW0727`, the same code an anchor's blank eye
shot gets, and says which bearing and zoom produced it. Every run states its view
binding count, in the summary line and in the manifest, beside the anchor counts.

The manifest records a view's `spec` verbatim along with its face, subject, aim
point and zoom, so any frame in a review set can be re-asked for exactly.

**On a tiled zone the eye shots are the zone's.** Pass the manifest and the
tiles are reassembled before anything is planned, so a body stands at the
anchor's zone cell and looks across a cut as if it were not there: measured on a
2-tile 20×10×84 ward, an anchor 6 blocks past the cut reads 54 open cells ahead
and the image shows the corridor running the whole length of the zone. Nothing
about packaging reaches the camera, the placement, the clearance or the
filenames. `crates/render/tests/tileset.rs` holds that claim — from one tile the
same anchor is out of bounds and yields no eye shot at all.

### `contact-sheet` — the curation page (spec-0027 §3, spec-0028 §3) · agent builds it, owner chooses from it

Lays candidate renders out as one page the owner picks massing from. **Building
the page is agent work; choosing from it is hers** — the tool exists to put the
decision in front of her eye, never to make it. It needs **no GPU and no client
jar** — it composites images the renderer already made — so unlike the rest of
this section it runs anywhere, including CI.

Input is a directory in either shape: one subdirectory of shots per candidate
(`delve-render batch` output — the representative angle is `--shot`, default
`ext-se`, falling back to the first render by name), or a flat directory of
`.png` renders. `--shot` given **explicitly** and missing for some candidate is
an error, never a silent substitution of another angle: a page whose cells face
different directions is not a comparison.

A manifest is **always** written beside the PNG as `<stem>.json` — cell → rank,
id, image, score, plus the binding counts and the layout used. That is how "she
picked number 7" resolves back to a prefab id, and it is also the input
`tools/refscore.py` reads, which keeps `delve-render` the single discoverer of
what a candidate is and what it is called.

**The score RANKS the page; it NEVER gates it** (owner ruling, spec-0028 §3).
With `--scores` the page is ordered best-match first, ties and unscored last, by
id — and every candidate is on it. The low scorer is present, last. An unscored
candidate is present, last, and labelled unscored, because a missing measurement
is not a bad one. This is enforced, not documented: the ordering is a seam, and
whatever it returns must be a permutation of the candidate set or the command
refuses with `DW0725` before drawing a pixel. The binding count is printed and
recorded on every run; a score set that bound to **zero** candidates is an error
(`DW0726`, exit 2), not a page in id order that looks like a successful ranking.

```sh
delve-render batch prefabs/zone2 -o .sheets/renders          # (GPU + client jar)
delve-render contact-sheet .sheets/renders -o .sheets/zone2.png
python3 tools/refscore.py --sheet .sheets/zone2.json \
    --reference .refimg/zone2.png --backend open-clip -o .sheets/zone2-scores.json
delve-render contact-sheet .sheets/renders -o .sheets/zone2.png \
    --scores .sheets/zone2-scores.json                        # the ranked page
```

The **real** metric backends are not installed by anything in this repo and are
deliberately absent from CI (PyTorch plus multi-GB weights). Make the venv once,
by hand, and only if you want them — the loop works without them:

```sh
python3 -m venv .refscore-venv && .refscore-venv/bin/pip install open_clip_torch
.refscore-venv/bin/python tools/refscore.py --sheet ... --backend open-clip ...
# VQAScore instead (text-conditioned, needs --prompt): pip install t2v-metrics
```

CI runs the same loop with `--backend stub` — deterministic, offline, keyless,
**not a similarity measure**, and loudly labelled as such on the page and in the
score file. That is what makes the ranking verifiable without a model; it is not
a substitute for one, and an uninstalled real backend exits 4 rather than
quietly becoming the stub.

Sheets are generation-time working material like renders and reference images:
`.sheets/` is gitignored, nothing here ships, and nothing here can move a delve's
bytes. Two runs over the same inputs produce the same page byte for byte (CI
asserts it) so a cell number means one thing.

`panorama` computes its camera from `render-plan.json`'s `layout_aabb`: a 45°
oblique on a corner bearing (`se` default), solved back until every corner of the
layout is in frame with a 12% margin, sun at 50° altitude 40° off the camera
bearing, chunk list = the layout's own chunks, and — iff the plan states
`horizon: ocean` — Chunky's ambient water plane at the compiler's sea level. One
scene per bearing (`<campaign>_panorama_<bearing>.json`), so four bearings coexist
in one scene dir.

### `viewer` — the page the reviewer drives · agent builds it, owner decides from it

A still render answers *is the set pretty*. Only a camera the reviewer drives
answers *what is it like to be in here* — where the way in is, which face the
party walks on, where the interactables sit, how the interior reads at eye
height. `viewer` turns one prefab (or a directory of them, with a switcher) into
**one self-contained `.html`**: no CDN, no external stylesheet, no fetch, so it
opens from `file://` and survives the strict CSP a Claude Artifact is published
under. Every byte is inline.

```sh
delve-render viewer campaigns/prefabs/island-mountain.nbt -o .sheets/mountain.html
delve-render viewer campaigns/prefabs -o .sheets/library.html      # all 36, one page
```

**The blocks are real blocks.** The page carries the pinned client jar's own
resources for every blockstate it contains — `blockstates/<id>.json`, the whole
`parent` chain of every model they name, and every `.png` those models reference,
all inline — and [deepslate](https://github.com/misode/deepslate) (MIT, vendored
and embedded) draws them by walking that chain exactly as the game does. So a
wall is a wall, a stair is a stair, a chest is a chest, a torch has a flame and a
pane of glass is a pane. Typically a hundred textures and two hundred models for
a library page.

Two facts the client jar does not carry come from elsewhere. Per-block render
flags are derived from model geometry and texture alpha. Per-block **default
state** — what the game reads an unwritten property as — comes from the pinned
block registry, never from a guess: a bare `minecraft:cobblestone_wall` is a wall
POST (`up=true`, every side `none`), and "the first legal value" would give
`up=false` with `east=low`, which is a different block.

`--textures` accepts an unpacked resource directory as well as the jar, which is
how a page is built with **no client jar present** (CI does exactly this), and
the seam a creator uses to point the page at their own resource pack.

Rebuild the vendored renderer with `tools/build-deepslate-bundle.sh` (needs
`npm`; installs into a scratch directory, never into the repo). It pins the
versions, applies one local patch, and refuses if upstream has moved the ids it
patches. Two consecutive builds are byte-identical.

**The page is walked.** It opens on a pair of feet and it stays there: `W`/`A`/`S`/`D`
walk, the mouse looks, `Space` and `C` rise and sink, `Shift` moves faster, arrow
keys turn a little at a time, right- or middle-drag slides the camera without
turning, and the wheel moves along the view axis. Nothing is conditional — there
is no camera state in which a movement key does nothing, and no gesture that
changes meaning depending on which view was picked. `Orbit the whole piece` is a
labelled button that swings the camera around the outside for a look at the
massing; while it is on, dragging orbits, and any movement key puts the reviewer
back on their feet with the button visibly released.

The whole mapping lives in one file, `crates/render/src/viewer/controls.js`,
which knows nothing about how the page draws. `crates/render/tests/controls.test.mjs`
executes it — pressing keys and checking where the body ends up in each of the
four cardinal facings — and CI runs it as a step of `rust (fmt, clippy, test)`.

**Cameras.** `Ground level`, `Exterior ¾` and `Plan` always exist; every declared
anchor and jigsaw socket adds a **point of view** — eye at **1.62 blocks** above
the floor of that cell, the height a standing player actually sees from. A
socket's facing points *out* of the piece, so its point of view looks the other
way. The page opens on the first anchor whose name stem is a reserved way in
(`spawn`, `entry`, `entrance`, `threshold`), else the first socket, skipping any
whose eye would land inside a block; a prefab that declares none stands the
reviewer on the ground off the south face, still walkable from the first frame.
The **cutaway** slider hides everything above a Y level and re-meshes, which is
how a roofed interior gets read at all.

**Anchors come from `<basename>.json` beside the `.nbt`** — the same metadata
`piece` already reads, so hand-built prefabs carry them today and a grammar
snapshot's semantics sidecar loads through the same reader. Point anchors and
region anchors both draw. **Zero anchors is a stated finding** (`DW0726`), not a
quiet success: the page says so and offers exterior and plan only.

**A tiled zone is one building.** A zone past the 48-per-axis structure-template
cap ships as several `.nbt` files and one manifest. `viewer` reassembles it
before it draws anything, exactly as `piece` does, and a lone tile passed by name
is refused with the manifest named. Pointed at a directory holding such a set,
the page shows the zone and never its tiles — a review of a building sliced at a
packaging boundary passes and means nothing. `piece --view` frames a tiled zone
the same way and off the same anchors, so an elevation of a zone is asked for
exactly as an elevation of a single template is.

**`viewer` and `piece --view` answer the same question differently**, and the
difference is the artifact. Both put a camera where the planned set has none;
the page hands it to a person for as long as they want it, and a view hands back
**a PNG and a manifest line** — something a trial record, a review report or a
byte-comparison can cite. Drive the page to decide what the picture should be;
declare the view to keep it.

**Fidelity is reported, never assumed.** Three findings, each with its cell
count, on the page and on stderr:

- `DW0790` — **a blockstate the pinned version does not have**: the id, its
  model, or one of its textures is absent, so the page draws a placeholder. It is
  a finding about the picture; whether the same id is a defect in the world is
  `DW0734`'s question, decided by the template's own `DataVersion`. That is how
  `minecraft:chain` was found (1.21.11 renamed it `minecraft:iron_chain`): the
  page cannot draw it, and the pre-pin template carrying it is datafixed on load,
  so the game does.
- `DW0791` — **a palette entry that leaves unwritten a property its own
  blockstate definition selects a model with**. Legal, and a running server
  places the right block; but the page can only draw it from the version's
  default state, so what the file says and what the reviewer is looking at are
  different blocks. Worst on a `multipart` definition, where an unwritten
  property matches no case at all: a `cobblestone_wall` with nothing written drew
  a **solid cube** where a wall post stands, and every tool reported it resolved.
  Measured over the library at the pinned content SHA: **15 palette entries
  across 7 of the 36 prefabs**, which is 7 distinct blockstates — a barrel's
  `open`, a grass block's and a podzol's `snowy`, a button's `powered`, a fence
  gate's `in_wall`. No connection class is among them: an omitted connection is
  `DW0735`, and the generators write those states from the piece's own
  neighbours. Counting every unwritten property instead of only the selecting
  ones gives 84 entries across 20 prefabs; the difference is `waterlogged` and
  the non-selecting residue beside it (a trapdoor's `powered`, `signal_fire`,
  `cracked`, `facing`, `distance`) — real, and invisible.
- **drawn as nothing, or drawn as the missing-texture checker** — measured in the
  browser by meshing each blockstate alone, because neither is visible from the
  resources. The checker case is how a wrong block-entity texture id presents:
  those ids are asked for by code and never by a model file, so the block renders
  magenta and nothing is said.

Beside them the page states **what each check examined** — how many blockstates,
how many textures at what atlas size, how many block-entity texture ids resolved
against the source, and which Minecraft version. A page that examined nothing
must not read like a page that examined everything and found nothing.

`DW0792` is the toolchain's own failure rather than the prefab's, and refuses
(exit 10): the vendored renderer has lost its texture-id patch, or a
block-entity texture id the emitter asks for is absent from a source that
declares itself to be the pinned game. Which of those an absence means is
decided by the source — a jar that says it is 1.21.11 is complete by definition,
a resource pack is entitled to be partial and gets `DW0790` instead.

**Size.** Geometry is never JSON: the grid is run-length encoded as
`(palette index u16, run length u16)` and base64'd, so the payload tracks how
complicated the building is rather than how big its box is, and only exposed
faces become triangles — in the browser, from that grid, rebuilt through the
renderer's own `addBlock`. Measured: `keep-gate-room` **368 KiB**;
`island-mountain` (36×28×42, 42,336 cells) **464 KiB**; all 36 committed prefabs
on one page **656 KiB**, of which **281 KiB** is the vendored renderer, present
once however many prefabs a page holds. The ceiling is 16 MB.

Two runs over the same input produce the same page byte for byte (ADR-0006).
`#model=<id>&preset=<id>&cut=<y>` in the URL opens a specific view, so a link
points at the thing being discussed.

`palette` writes the derived per-blockstate colour and shape table on its own —
the input the snapshot chart, the palette-selection tooling and the fidelity gate
read.

## 4a. Chunky — the official renderer (external process) · agent + human

Chunky (GPL-3.0) is the renderer for every Delvewright frame that has to look
like Minecraft: whole-scene review shots, storybook scene illustrations, and the
per-release whole-map panorama. It is **never linked or vendored** — `delve-render`
writes scene JSON, `ChunkyLauncher.jar` renders it as a separate program.
Attribution: [`../ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md).

Install (once per machine):

```sh
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot     # self-installs the pinned core
```

The launcher self-installs cores into `~/.chunky/lib`. The pinned one is
`chunky-core-2.5.0-SNAPSHOT.474.g156e2bb` (`versions.toml [render]`,
`scene::CHUNKY_CORE`) — 1.21.x needs a snapshot core; the stable line stops at
1.20.4.

**Textures come from the creator's own client jar** and are never redistributed:
Chunky reads `~/.chunky/resources/minecraft.jar` (or `--textures <jar>`), the
same EULA-gated jar `delve-render` resolves.

Render + extract:

```sh
delve-render scene <build-dir> -o scenes --world ./world      # or: panorama
java -jar ChunkyLauncher.jar -scene-dir scenes -render <scene-name> -f
java -jar ChunkyLauncher.jar -scene-dir scenes -snapshot <scene-name> out.png
```

`<scene-name>` is the file stem, without `.json`. Every emitted file is named
after the scene's own Chunky `name`, campaign-qualified — `hello-world_spawn`,
`hello-world_pov_leg0_wp1`, `hello-world_panorama_se` — and that same stem names
its caches and its rendered `.png`.

Operational facts, paid for in a debugging session (2026-08-06):

1. **Chunky caches loaded chunks** in `<scene>.octree2` and `<scene>.dump` (plus
   `.dump.backup`, `.emittergrid`) beside the scene — keyed on the scene's `name`
   field, **not** its file name. Chunky treats `name` as the scene's identity: load
   `foo.json` whose `name` is `bar` and it writes `bar.json` and `bar.*` caches
   next to it, so a re-emitted `foo.json` never invalidates them and `-render bar`
   silently serves the old scene. `delve-render` therefore emits every file under
   its own scene name, which makes the two agree by construction. Re-rendering after a change
   to `chunkList`, camera, sun or water settings **silently reuses the stale
   cache** — no warning, wrong frame. This is automated away: any `delve-render`
   scene or panorama emission deletes exactly those siblings for the scenes it
   writes (`render::cache`). Hand-edit a scene JSON and you own the deletion:
   Chunky's own `-reload-chunks` re-reads the world but does **not** reset the
   accumulated `.dump`, so the new frame keeps averaging in the old samples.
2. **Ocean-horizon delves need the water-world plane, and only the layout's
   chunks.** The shipped world save holds only the chunks the layout occupies, so
   the sea must come from Chunky (`waterWorldEnabled: true`, `waterWorldHeight:
   62.875` = sea level 62 + the 0.875 block-water surface, with
   `waterWorldHeightOffsetEnabled: false` — the default `true` would silently drop
   the plane 0.125). `waterWorldClipEnabled` keeps the plane out of the loaded
   chunks, so widening `chunkList` to the surrounding pure-ocean chunks only adds
   more of the save's own block water beside it — and the two read at visibly
   different tones, a seam across the emptiest part of the frame. Trimming to the
   layout's chunks shrinks that seam to the layout's own chunk footprint (a small
   layout inside a 16x16 chunk still shows the ring; a layout that fills its
   chunks shows none). Emission handles all of it from the plan's `horizon` fact;
   nothing to set by hand.
3. **The progress counter `(N of <image height>)` counts scanlines, not
   samples** — a 1024px render reads `(512 of 1,024)` at half a *pass*. Watch
   `spp` / the target, not that number.

Speed doctrine: the core is **CPU-only** — the official OpenCL plugin is WIP and
effectively unavailable on Apple Silicon, so there is no GPU path; do not wait for
one. Go wide instead: one `java -jar ChunkyLauncher.jar … -render` **process per
scene**, run in parallel (give each `-threads <n>` so they do not all claim every
core), and tier the sample budget with `-target` — ~64 for a draft you only need
to judge framing on, ~300 for final art (`delve-render panorama --spp`'s default),
500 for the review scene set (`scene`'s `sppTarget`).

## 5. `delve-harvest` — playtest note harvester (`crates/orchestrator`, package `delvewright-orchestrator`) · human

Pairs in-game `[DelveNote]` stamps with the creator's chat notes into
`playtest-report.json` (spec-0006). The capture half is human — the owner plays and
runs `/trigger dw.note`; the agent runs the harvester afterwards.

The same pass harvests spec-0019 `[DelveShot]` stamps (`/trigger dw.done`) into
`rehearsal-report.json`, written **only** when the session actually stamped a
shot proposal — feed that report to `delvec calibrate`.

```
delve-harvest <server.log> <creator-datapack/layout.json> [-o playtest-report.json]
                                                          [--rehearsal-out rehearsal-report.json]
```

Full loop, including how the log is captured:
[`../../validation/README.md`](../../validation/README.md).

## 6. Python tooling (`tools/`)

Never shipped inside a delve.

| Tool | Class | Invocation |
|---|---|---|
| `tools/block-appearance.py` | agent | `python3 tools/block-appearance.py (--id <block>... \| --near '#rrggbb' \| --list \| --screen \| --mix <spec> \| --program <p.json>) [--where EXPR]... [-n N] [--full-cube-only] [--exclude-tinted] [--technical] [--sheet [PATH]] [--seed N] [--jar <client.jar>] [--json]` — **what a block actually looks like, and what a palette made of them reads as**, measured from the pinned client jar. The palette step of [`prefab-procedure.md`](prefab-procedure.md) §2, and it is three steps, not one. **Query**: `--id` measures named blocks, `--near` ranks by colour, `--list` dumps the shelf — a block's name is not its appearance (`packed_mud` is orange), so a palette is queried, never recalled. **Screen**: `--screen` plus repeatable `--where` narrows by measured axes in the order given — `full_cube`, `not tinted`, `not gravity`, `L>=0.75` (Oklab lightness), `C_mean<0.02` (how coloured), `texture_range<=0.30` (how loud the pattern), `form=slab`, `family=minecraft:sandstone`. Constraints **eliminate, never score**; the cascade prints its survivor count at every step, and the worked screen takes 1146 blocks to 14. An unknown or malformed facet is refused, never ignored. **Measure the mix**: `--mix 'a=3,b=3,c=4'` (repeatable) or `--program p.json` (every `palette` role AND every inline `fill` material) reports `chroma_mass`, `chromatic_area`, the **named** `loudest_member` with its area share, `dominant_hue`, and `void_area` — `minecraft:air` is a paint member like any other (it is the whole of decay in the grammar), so it is counted as area with no colour rather than dropped, and the mean is renormalised over the solid share. A mean colour is printed and is **never the verdict** — swapping half a sandstone mix for calcite and polished diorite moves the mean 13.5 RGB units while the chromatic area falls 60% → 30%. Every report states its **binding count** (paints examined, mixes with ≥ 2 members) and calls a zero binding a FINDING. **Look**: `--sheet` writes a PNG to `.sheets/palette/` (gitignored) — each survivor tiled and labelled, each mix as its seeded weighted tiling, i.e. the wall at distance zero. No GPU, no Chunky, no world; byte-identical at one seed; a path inside the repo but outside `.sheets/palette` is refused before any measurement runs (ADR-0006/ADR-0013). Per-block JSON carries Oklab `L`, `L_p05`, `L_p95`, `L_sd`, `texture_range`, `C_mean`, `C_p90`, `C_max`, `hue`, plus `family`/`form` from the vendored classification table and the `gravity`/`technical`/`tinted` flags. Ids are checked against `crates/compiler/data/blocks-1.21.11.json`, and **a missing registry is a named refusal naming the fallback** (take role names from the corpus via `delve-grammar show`) rather than a traceback, because this tool is run from checkouts that do not carry `crates/`; technical blocks are excluded unless `--technical`. Jar resolution is `delve-render`'s: `--jar`, `$DELVEWRIGHT_CLIENT_JAR`, `~/.chunky/resources/minecraft.jar` — **without it the tool refuses**, because an answer given without the textures is the recollection it exists to remove. Stdlib only — no packages to install. What it cannot decide is stated on the artifact: whether a palette reads as its cultural referent, and role fitness (light emission is in no vanilla data branch, so a screen for "pale neutral wall" returns `pearlescent_froglight`) |
| `tools/extract-block-defaults.py` | agent (rare) | `python3 tools/extract-block-defaults.py <blocks/data.min.json> crates/compiler/data/block-defaults-1.21.11.json` — regenerate the pinned 1.21.11 block **default-state** table from the same `misode/mcmeta` summary its sibling reads: that file keeps the source entry's legal values, this one keeps its default state. It is what lets a reader that is not a running server know what a palette entry leaving properties out actually means. Pins and checks the source SHA-256 and the block count, and refuses a default that is not one of its own property's legal values. Only ever run when ADR-0009's revisit triggers fire |
| `tools/extract-block-classification.py` | agent (rare) | `python3 tools/extract-block-classification.py <tag/block/data.min.json> <recipe/data.min.json> crates/compiler/data/block-classification-1.21.11.json [--report] [--family-tags] [--loose]` — derives every block's **form** and material **family** from the pinned `misode/mcmeta` 1.21.11 summary; no client jar, so the result is vendored and available in CI. Form comes from vanilla's shape tags (`#slabs`, `#stairs`, `#walls`, `#fences`, `#doors`, `#trapdoors`, `#buttons`, `#pressure_plates`, `#all_signs`), resolved transitively; `pane` has no vanilla tag and is read off the blockstate connection signature, which reaches iron and copper bars too. Family is the connected components of the derivation graph — stonecutting, cooking, and crafting recipes with **exactly one ingredient, and it a block** — which is what keeps a compound from reading as a derivation (`granite` is diorite + quartz). 1166 blocks → 788 families, largest 20 (deepslate). `--report` prints the largest families and the stats; `--family-tags` and `--loose` are measurement switches for the two rules **not** shipped (they give largest families of 87 and 41). Pins and checks both source SHA-256s and the block count; see `crates/compiler/data/PROVENANCE.md`. Only ever run when ADR-0009's revisit triggers fire |
| `tools/extract-block-registry.py` | agent (rare) | `python3 tools/extract-block-registry.py <blocks/data.min.json> crates/compiler/data/blocks-1.21.11.json` — regenerate the pinned 1.21.11 block-state registry from a `misode/mcmeta` summary. Pins and checks the source SHA-256 and the block count; see `crates/compiler/data/PROVENANCE.md`. Only ever run when ADR-0009's revisit triggers fire |
| `tools/extract-shape-properties.py` | agent (rare) | `python3 tools/extract-shape-properties.py <minecraft-1.21.11-client.jar> crates/compiler/data/blockstate-shape-props-1.21.11.json` — regenerate the shape-carrying (multipart) property table behind `DW0735` from the client jar's blockstate definitions. Pins the jar's `version.json` to 1.21.11 / DataVersion 4671 and cross-checks every derived property against the block registry; see `crates/compiler/data/PROVENANCE.md`. Only ever run when ADR-0009's revisit triggers fire |
| `tools/build-deepslate-bundle.sh` | agent (rare) | `tools/build-deepslate-bundle.sh` — rebuild the renderer the prefab review page embeds (`crates/render/src/viewer/deepslate.bundle.js`). Needs `npm` and network; installs into a scratch directory, never into the repo. Pins deepslate, gl-matrix and esbuild by exact version, applies the local banner/shield texture-id patch with an exact expected hit count, and prints the licence of everything in the bundle. Two consecutive builds are byte-identical, which is what keeps the page byte-identical (ADR-0006). Refuses if upstream has moved the ids it patches — which is the signal to drop the patch rather than widen it |
| `tools/i18n-translate.py` | agent | `python3 tools/i18n-translate.py <campaign-dir> --lang <code> [--config f] [--delvec cmd] [--batch-size n] [--dry-run] [--force] [--no-validate] [--reflect\|--no-reflect]` — external OpenAI-compatible API, generation-time only; `--reflect` runs the three-step translate → critique → revise pass; see [`i18n.md`](i18n.md) |
| `tools/refimg.py` | human (advisory, at the design-alignment gate) | `python3 tools/refimg.py (--prompt P | --prompt-file F) [--out stem] [--style-code HEX | --style-ref IMG ...] [--seed N] [--chain-from INTERACTION_ID] [--style-note TEXT] [--count N] [--rendering-speed TURBO|DEFAULT|QUALITY] [--resolution WxH] [--dry-run]` — draws a **reference image**: concept art produced BEFORE any prefab exists, so the owner confirms the design against a picture rather than prose. **Not a render** — a render is a candidate prefab imaged by `delve-render`, later, at contact-sheet curation; two stages, two producers. Config is `[refimg]` in the gitignored `delvewright.local.toml` (convention block in `delvewright.toml`); the key never enters a file — `api_key_env` names an env var read at call time, one var per provider. Two providers: `gemini-native` (the Interactions API — anchors on reference images, **no seed**) and `ideogram-v3` (style-CODE anchor **and** a seed, but its generate response was measured NOT to return a code, so the code must be read off the web UI). A flag the configured provider cannot honour is **refused**, never silently dropped: `--seed` on a seedless provider exits 1 saying what that costs. Absent config exits 2 saying what to add; **malformed config is a hard error** (an inline `api_key`, an unknown provider, a bad `rendering_speed`) so a typo can never silently downgrade the anchor. The provider name carries **capability**, not wire format: an OpenAI-compatible images endpoint without image input would accept a style-anchored request and *silently ignore* the anchor, shipping N unrelated pictures with no error — so only verified providers are listed (`ideogram-v3` and `gemini-native` today) and anything else is refused. `--style-code` and `--style-ref` are mutually exclusive (provider constraint, enforced locally rather than discovered as a 4xx). The **full provider response is always written beside the image** as `<stem>.json`: the anchor a series needs is only recoverable from what the provider actually returns, and the docs do not promise a style code comes back. Output goes to `.refimg/` (gitignored) — generation-time working material, never shipped, never in the content repo, so output licensing never touches a shipped asset (ADR-0013) and nothing here can move a delve's bytes |
| `tools/refscore.py` | human (advisory, at contact-sheet curation) | `python3 tools/refscore.py (--sheet MANIFEST.json \| --images P...) [--reference IMG] [--prompt TEXT] [--backend stub\|open-clip\|vqascore] [--model M] [--device cpu\|cuda\|mps] [-o scores.json] [--dry-run]` — scores candidate **renders** against a **reference image**, to ORDER a contact sheet. **The score RANKS; it never GATES** (owner ruling, spec-0028 §3): this tool emits one score per candidate and no verdict of any kind — no threshold, no keep/reject — and `delve-render contact-sheet` refuses (`DW0725`) any ordering that is not a permutation of the candidate set. Promoting the score to a gate needs its own owner-approved amendment backed by batch data. `--sheet` is the recommended input (the `.json` the sheet always writes beside its PNG), which makes `delve-render` the SINGLE discoverer of candidates so the ids cannot drift; a mismatch is not silent either — the sheet states its binding count and errors on zero (`DW0726`). Three backends: `stub` is a deterministic, offline, dependency-free hash of the file bytes — **not a similarity measure**, and it says so on every artifact it touches (the sheet paints "STUB SCORES" across its header); it exists to exercise the whole loop, and it is what CI runs. `open-clip` (MIT) is image↔image CLIP cosine and needs `--reference`; `vqascore` (Apache-2.0) is **text-conditioned** — it asks a VLM how well a render answers `--prompt` and never looks at the reference image, so it requires a prompt and **refuses** a `--reference` it would silently ignore. Both real backends pull PyTorch and multi-GB weights: they are **deliberately absent from CI**, nothing in this repo installs them, and they live in a creator's own virtualenv (`.refscore-venv/`, gitignored) — a missing dependency exits 4 naming the install line and **never falls back to the stub**, because a stub number that looks like a measurement is worse than no number. Config is `[refscore]` in the gitignored `delvewright.local.toml` (convention block in `delvewright.toml`); absent config with no `--backend` exits 2 saying what to add, malformed config is a hard error, and an inline `api_key` is refused outright (today's backends run locally and need no key at all). Output goes to `.sheets/` (gitignored) — generation-time working material, never shipped, never in the content repo (ADR-0013), unable to move a delve's bytes (ADR-0006) |
| `tools/derive-client-langs.py` | human | `python3 tools/derive-client-langs.py [--version V] [--rust]` — re-derives `dsl::mclang::CLIENT_LANGS` (the language files the **pinned** client loads) from Mojang's version manifest → version metadata → asset index, printing the sha1 of every document it read so the derivation is auditable. Run it when ADR-0009's Minecraft pin moves, diff the printed table into `crates/dsl/src/mclang.rs`, `cargo fmt`. Never run by CI or by a build — the compiler must not reach the network (ADR-0006) |
| `tools/skin/` (`delve_skin`) | agent | `python -m delve_skin all <cast.json> --skins-dir D --catalog-dir D --preview-dir D [--id ID] [--scale N]`, or the `build` / `preview` / `catalog` stages individually. Needs its own venv (`pip install -r tools/skin/requirements.txt`); see [`../../tools/skin/README.md`](../../tools/skin/README.md) |
| `tools/build-every-campaign.py` | CI + agent (run it before proposing any engine change that touches emission, layout or validation) | `python3 tools/build-every-campaign.py --delvec <binary> [--content <checkout>]` — builds **every campaign** the pinned content repo carries, in **every language its `world.json` declares**, and reds if one stops building. Closes the gap that let a change reach 10/10 green while stopping the flagship released campaign `nobodys-cave-island` from building at all (26 × `DW0364`): every other gate builds a FIXTURE, and a fixture exercises one verb, where a campaign is the only place the verbs meet a real prefab library, a real layout solve and a real translation sidecar. Campaigns are **discovered** (any dir under `<content>/campaigns/` with a `world.json`), never listed, so the next content re-pin gates a new campaign with nobody remembering. `--delvec` is required and never inferred — the gate's whole subject is *which engine* built the campaign. A campaign that cannot build today goes in `.github/campaign-build-exclusions.toml`, which **inverts** the assertion rather than removing it: still built, must still fail, and must fail with **exactly** the recorded `expect_codes` — an extra code is a new break that was hiding behind the exclusion, and a SUCCESS is an expired exclusion, both red. Currently one entry: `hollow-vigil`, `DW0331`. States its binding count every run (discovered / built green / known-red, each named); discovering zero campaigns, building zero campaigns, or an exclusion naming a campaign that no longer exists are each a red. Runs in CI as `campaign builds (every campaign in the content repo)`, on every push |
| `tools/staging-gate.py` | agent (**mandatory** before any build is handed to the owner) | `python3 tools/staging-gate.py --campaign <dir> --build <delvec-out> [--ledger docs/playtest-findings.json] [--report R.md] [--json R.json] [--strict]` — **the coverage gate on the findings ledger**, and the only tool that asks a question the ladder cannot: for every defect the owner has EVER reported, on any campaign, does a general-form check exist and does it BIND — non-zero — on the build about to be staged. It re-runs nothing; it reads `docs/playtest-findings.json` and reports one row per finding (finding → general form → the check carrying it → that check's binding count here → verdict). Six reds, each a way a green has really lied on this project: `NO-GENERAL-FORM` (the instance was fixed, the class never built), `MISSING-CHECK` (the ledger names a check this engine no longer has — not in source, undocumented, or asserted by no test), `UNBOUND` (matched zero objects), `INAPPLICABLE` (zero binding AND zero precondition — this campaign cannot exercise the class at all), `UNFENCED` (the campaign's `dsl_version` never reached the surface the check keys off), `NO-SOURCE` (the campaign has no stage JSON, so nothing can be measured — the bell remake's state today, and never a pass). The one non-red escape is playtest-methodology.md rule 2's: `DECLARED-UNCOVERABLE`, requiring a `disposition` AND a substantive `justification`, counted in the headline because rule 4 makes each one a risk item at that staging review. On a pass it mints an **admission token** (`<build>/staging-admission.json`) binding the sha256 of that tree's `manifest.json`; a refusal DELETES any existing token. `--stage-anyway "<reason>" --acknowledge-red <N>` is the deliberate override — N must equal the current red count exactly, so it cannot be typed from memory. **Invoked by the staging surface, not by a doc line**: `tools/playtest-server.sh` runs it between build and container, `validation/owner-play.yaml` requires its token via the `staging-admission` service, and `release.yml` runs it before the GHCR push. It shipped called by nothing but its own tests — the UNRUN shape — and `tools/tests/test_staging_gate.py` now carries a tripwire against that returning. **Not a CI status check, deliberately** — it is red today by design (18 rows on `nobodys-cave-island`), and wiring an honest red list as a blocking context would force the one thing CLAUDE.md forbids: weakening it to get green. Its own falsification suite (every verdict driven red then green, plus the token round-trip through the real verifier) IS in CI. |
| `tools/check-dw-codes.py` | CI | `python3 tools/check-dw-codes.py` — asserts the DW catalog in `compiler.md` matches `crates/**/*.rs` both ways, and that every code has a test |
| `tools/check-diagnostic-messages.py` | CI | `python3 tools/check-diagnostic-messages.py` — **a diagnostic's message is checked, not only its code.** The row above proves a code exists, is documented, is unique and is asserted by a test; all four are satisfied by a rule whose message reads "… so freezing it would put a  on disk …", which is what `delve-grammar expand` printed at the moment it refused an author's export. The dropped noun and its doubled space sat across a `\`-newline continuation, so no LINE of the source contained the gap, and every required status check was green on it — the message is the one part of a diagnostic nothing had ever read, and it is the entire product at the moment an author meets it. This walks `crates/*/src/**/*.rs`, collects every message the source can produce, **renders** each one (escapes decoded, continuations joined, every `{…}` filled with a non-empty sample) and refuses a gap in the result: a doubled space inside running prose, a gap after an article, a space before punctuation, a dangling `a`/`an`/`the`, an empty quoted span, empty brackets. Reading source rather than running code is what reaches `crates/render`, a workspace of its own that no `cargo test` at the repo root builds. Four site shapes, enumerated because a shape missing from the list is a hole in the gate rather than a message that is fine: the last argument of `Diagnostic::error`/`warning` (the compiler's 4-arg form and the schem/admit/render 2-arg form alike), a `message:` struct-literal field, `write!`/`writeln!`/`write_str` inside a `Display` impl for an error type — one the crate gives an `impl std::error::Error`, or one whose name ends in `Error`, since `dsl::fmt::ParseError` carries a `DW0770`/`DW0771` code and a message and implements nothing, and the trait alone would have left it unread; that shape is what reaches the motivating instance, an error enum and not a `Diagnostic` — and the bare print family, which is the ONLY way `delve-orchestrator` refuses anything, so keying the gate to the `Diagnostic` type alone would have left that binary at a binding count of zero while reporting a pass. It follows a message one hop through a named helper and through a local the message is assembled into, because the longest diagnostics in the compiler are built that way. **There is no allowlist and no marker**: report tables, whose rows deliberately line a label up against a value, are told apart by SHAPE — a run of spaces is alignment only when the message (or a sibling message of the same function) REPRODUCES the column, which a single dropped word cannot do, and the prose rules read a line with its backtick spans blanked, so an author shown a character set is not read as a sentence with a gap in it. An escape hatch a dropped word could satisfy would be the vacuous opt-out CLAUDE.md names. **The one thing it cannot judge is a substitution that is EMPTY at runtime** — `format!("put a {kind} on disk")` prints the same defect, and from the source every template with a `{}` looks alike; assuming instead that any substitution may be empty would red several hundred correct messages (`{id:?}` renders `""`, never nothing). Leading and trailing whitespace is deliberately not a rule either: measured over the whole message set it fires once, on a sentence fragment built to be concatenated, and zero times on a real defect. States its binding count per crate every run — **zero rendered messages is a red** — and prints, by name, every message expression it could not render and every one it found forwarded from a site checked where it was built, so a message out of reach is a line in the log rather than a silent pass. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-reference-versions.py` | CI | `python3 tools/check-reference-versions.py` — binds every version a READER ACTS ON to the build, by EQUALITY in **both directions**, over two document sets. **(1) `compiler.md`'s version header**: `delvec X` == `crates/compiler/Cargo.toml` `[package] version`, `dsl Y` == `crates/dsl/src/envelope.rs` `SUPPORTED_DSL_VERSION`, `mc Z` == `versions.toml` `[minecraft] version`, and the bold supported-`dsl_version` list == `SUPPORTED_DSL_VERSIONS` as an **ordered** sequence (a set comparison would pass on a shuffled list, and the list doubles as the reading order for the "additive superset" claim beside it). Also binds the `DW0102` catalog row's `{…}` set to the same constant, since `DW0102` fires on exactly `!is_supported_version(version)`. Motivating instance: the header read `delvec 0.1.0`, `dsl 0.8.0`, `{0.2.0 … 0.8.0}` while the build was at `delvec 1.1.0` / `dsl 0.9.0` and accepted `0.9.0` — with the body of that same file documenting the v0.9 surface correctly, and every gate green, because no gate related the two. It is the first thing an authoring session reads to pick a stage envelope's `dsl_version`. **(2) the crates.io FRONT PAGES**, over the derived publishable-README set (`tools/lib/publishable.py`, shared with `check-crates-io-readmes.py`) — those pages state the Minecraft version, the `dsl_version` window and the minimum Rust, the facts that decide whether a visitor can use the crate, and they were bound to nothing. Two rules per page: the three labelled claims (`**Minecraft**: Java Edition <mc>`, ``**Campaign format**: `dsl_version` `<first>` through `<last>` ``, `**Rust**: <rust-version> or newer`) must be **present and equal** — an absent claim is exit **2**, because a page that drops its compatibility section stops telling a stranger the one thing they need; and **no unbound version literal anywhere on the page**, i.e. every `X.Y.Z` must be one of the build's constants (pinned MC, a supported `dsl_version`, a publishable crate's `version`/`rust-version`). The second rule is what reaches prose the bullets never touch — the `delvec` page states the Minecraft version three times, one of them inside "the vendored 1.21.11 Brigadier command tree". `UNBOUND_VERSION_LITERALS` is the only exemption, is **empty on purpose**, needs a written justification per entry, and reports its own stale entries. Equality is the point: **the stale-OLDER direction is the one that actually happens** (docs are written once, the build moves), and a gate that only rejects "newer than the build" is exactly what let a storybook ship a `v1.0` marker through the whole `v1.1` release green. `check-dw-codes.py` is green on the `DW0102` row and always would be — it proves a code EXISTS in both source and doc and is asserted by a test, never that the BEHAVIOR the doc ascribes to it is the behavior the code has; this is the mechanically checkable slice of that gap. States its binding counts every run — supported-version count, pages examined, version literals scanned; **zero publishable READMEs, or a page on which zero literals were scanned, is a red**, and that is not hypothetical: the literal regex first shipped with a lookahead that forbade a trailing dot, so it matched nothing on either page (both write `Java Edition 1.21.11.`) and printed green. A source file that no longer matches the expected shape exits **2** with the regex to fix named — fix the regex, never loosen the check. Runs as a step of `docs (local link check)`, and again in `engine-release.yml`'s `crates-preflight` before the crates.io one-way door — a step, not a job, since every job name is a required status context |
| `tools/check-crates-io-readmes.py` | CI | `python3 tools/check-crates-io-readmes.py` — **no internal reference on a page a stranger lands on**. `crates/compiler/README.md` and `crates/dsl/README.md` are rendered VERBATIM as the crates.io front pages of `delvec` and `delvewright-dsl` and are the only documents here whose reader has never seen this repository; the `delvec` page opened with "The deterministic compiler (spec-0002, ADR-0001/0006/0011)" for months, a citation a visitor cannot resolve and gains nothing from. Without this gate CLAUDE.md's *Audience separation in docs* is held by memory alone. Six unambiguous patterns, chosen because none can appear in honest prose about a Minecraft compiler: `spec-NNNN`, `ADR-NNNN`, `DWNNNN`, `task #N`, `PR #N`, and any markdown link (inline, image or reference definition) whose target is repo-relative — crates.io serves the markdown with no repository around it, so such a link goes nowhere. The diagnostic pattern is **digit-anchored on purpose**: it must not fire on "a stable `DW####` code", which is exactly what these pages legitimately want to say. **What it deliberately does NOT check**: the present-tense half of the rule (CLAUDE.md *A reader-facing document is written in the present tense of the current version*). Its tells — "now", "still", "since", "originally" — are ordinary English, and a checker for them would red on honest content, which is worse than no checker: a gate that reds correct prose teaches its readers to ignore it. That half stays a review obligation, recorded as such in the local decision ledger. No allowlist, and fenced code blocks are scanned like any other line — a gate a page can step out of with three backticks is not a gate. The file set is **derived, never listed**: every crate under `crates/*/` whose `[package] publish` is not `false`, resolved through its `[package] readme` key (`tools/lib/publishable.py`), so a crate that becomes publishable inherits the gate with no edit. States its binding count; **examining zero pages is exit 2**, as is a crate manifest that no longer parses or a workspace member outside `crates/*/`. Runs as a step of `rust (fmt, clippy, test)` — the job that already owns what crates.io receives — and again in `engine-release.yml`'s `crates-preflight` before the one-way door; a step, not a job, since every job name is a required status context |
| `tools/check-capability-ownership.py` | CI | `python3 tools/check-capability-ownership.py` — a capability must belong to the **object class it acts on**, not to the verb that first needed it. Motivating instance: `close-gate.sealed_hint` emits its own interaction bodies, its own actionbar reply and its own baked English, privately re-implementing `EnvTrigger{on:use} + narrate`, which the DSL already exposes generally. Five ledgers, each an allowlist carrying a REASON per entry: **A** every `summon minecraft:interaction` in the compiler (exactly one is `EnvTrigger`); **B** every compiler-baked player-facing English string; **C** DSL structs declared separately with an identical field set (`TrapDisarm`/`TimedGateDisarm`); **D** a cross-cutting modifier absent from some variants of a tagged enum (`requires_flags` rides 16 of 26 effects); **E** every `Vec<QuestEffect>` bundle must be reachable by some enumeration — this is the one that catches a *sixth* effect root, which `check-effect-roots.py` cannot see because it greps for the five it knows. Most entries are **OPEN FINDINGS** with a named lift, catalogued in `docs/notes/capability-ownership-audit.md`; the gate's job is that none can be added or removed in silence. **Known non-proof, stated in its docstring**: A/B are text scans (a body built through a helper that hides the `summon`, or a default assembled from fragments, is invisible); C/D/E parse `stages.rs` structurally but see only what `pub` fields and variant blocks look like textually. States a binding count per check every run; **a check that examined zero objects or matched zero is a red**. The counts live in exactly one place a reader reaches — the `Binds today` table of `docs/notes/capability-ownership-audit.md` — and the tool **reads that table back and reds on a disagreement**, naming the row and both figures, so a hand-copied number cannot sit below the build. A row it cannot find for a check it ran is a failure, not a skip, and it prints how many rows it examined. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-effect-roots.py` | CI | `python3 tools/check-effect-roots.py` — no source file may enumerate the campaign's **effect roots** by hand. An effect root is a `Vec<QuestEffect>` emission can lower; there are five, four hang off the quests stage and the fifth off dialogue, and nothing about the DSL's shape makes them findable by inspection — so every walk that needed "every effect" was written by someone enumerating the roots they knew about. Six were found and fixed independently; a sweep found thirteen more; this gate, on the run that introduced it, found three the sweep had missed (`continuity::excluded_npcs`, `emit::first_damage_players`, `emit_v04_packtests`' despawn scan). **None was ever red** — a walk that visits four of five roots looks correct over any campaign that does not use the fifth. `delvewright_dsl::effects` is now the one enumeration and every walk inherits it, which closes the thirteen; this gate is what stops a fourteenth, since the root fields are ordinary public fields and no type can forbid the loop. Flags a window of 40 source lines naming **3+ distinct** roots, outside an allowlist that carries a REASON per entry (the enumeration itself; `validate::reserved_v06_world`, sound by construction; `plan::required_anchors_for_area`, an open finding printed on every run). **Known non-proof, stated in its docstring**: a proximity heuristic over text — roots spread across a hundred lines, or reached through a helper taking the list as an argument, are invisible. States its binding count every run (currently 128 files, 92 markers); **examining zero files or finding zero markers is a red**, because a renamed field would otherwise leave it quietly green forever. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-grammar-ir-compat.py` | CI | `python3 tools/check-grammar-ir-compat.py` — a grammar `Program` is a long-lived on-disk document, and its two ways of growing new surface are not equally safe. A new **variant** of a tagged enum is safe: an older engine meets an `"op"` it does not know and fails loud at serde, and every exhaustive `match` forces an arm. A `#[serde(default)]` **struct field** is not — it rides through every walk untouched in both directions, so an engine that predates the field deserialises the document with the field's default, expands, passes every gate, and writes different geometry with nothing to say about it. Measured on the motivating instance, one file and three engines: `origin/main` built the UNREFLECTED transept at exit 0 with `blocks-exist` and `non-empty` green, while the engine that knows `reorient.mirror` built the reflected one — two buildings, one document, no complaint. Two gates. **Closed schema**: every deserialisable IR object type carries `deny_unknown_fields`, or sits in an `EXEMPT` table that may only SHRINK and carries a reason (one entry: `ir::Mark`, whose `at` is `#[serde(flatten)]`, which serde cannot combine with the attribute — it compiles and then refuses every well-formed mark). **Version ledger**: every optional field has a row in `grammar.md` §2e, checked in BOTH directions, and a row above the `1.0.0` floor must name a `*_SINCE` constant that `version.rs` declares AND that some `ProgramError::FencedConstruct` in `ir.rs` refuses under a guard **reading that field** — counted per field, since `mirror` is a field of two types and one refusal cannot stand for both. **Known non-proof, stated in its docstring**: it reads Rust as text, so a field added through a macro is invisible, and it cannot see whether a fence's *semantics* are right — only that a refusal exists and looks at the right name. States both binding counts every run; either examining zero is a red |
| `tools/check-unsanctioned-identifiers.py` | CI | `python3 tools/check-unsanctioned-identifiers.py [--tighten]` — **no repository artifact carries an identifier a reader cannot resolve.** The sanctioned ones are ADR numbers, spec numbers and DW codes (CLAUDE.md *Privacy in repo artifacts*); this refuses the three that are not, measured and reported separately because they arrive by different routes — a **task id** (`task #N`, `tasks #N`, `task-#N`, any case), a **pull-request or issue number** (`(#N)`, `PR #N`, `PRs #N/#N`, `PR <n>`, `pull/<n>`; a bare `#N` of one digit is left alone, since single digits are row numbers and numbered sections inside the document that writes them), and a **dated attribution**, an ISO date within 120 characters of a person or a decision word inside the same paragraph. A plain date is deliberately NOT a finding — an ADR's own `Date:` header, an upstream release, the day a licence was verified — because ADRs are the one place history legitimately lives and a licence date is evidence a reader may need to re-check. It is a RATCHET, not a one-off sweep: the count refilled twice inside a week of the first tidy because merged branches carry their own citations in and nothing looked. `FLOOR` holds, per file, exactly how many occurrences that file may still contain, compared by **equality in both directions** — above it is a new citation, below it is a floor that must be lowered in the same change, a file with matches and no entry is a red, and an entry on a file with none is a red. `--tighten` rewrites the entries and **refuses to raise one or to add one**, which closes the convenient way to turn this red green; `FLOOR` lives in the checker rather than in a data file so raising a number is a diff to a gate. `ALLOWED` carries only the two test files whose SUBJECT is these strings — this gate's own and `check-crates-io-readmes.py`'s — and the property that opt-out demands is one the defect cannot supply: an assertion in the file must FAIL if the string stops being a citation, which a document that merely wants to keep one cannot produce. A finding names the file, line, kind and token, and says what to do with the sentence: keep the OBLIGATION as a plain present-tense fact or delete it, and never replace the citation with a note about what it used to assert — that trades an unresolvable reference for a changelog, which is the same defect one layer out. Reads `git ls-files`, so an untracked scratch file is not a repository artifact. Deterministic, offline, stdlib-only python3. States its binding counts — files examined, bytes examined, per-kind totals, floor and allowlist sizes — and **examining nothing exits 2**, since a check that matched no files must never look like a check that passed. Runs as a step of `docs (local link check)` and again in `engine-release.yml`'s `crates-preflight`, because the tagged tree a release publishes from is not necessarily the tree CI last saw |
| `tools/check-doc-dupes.py` | CI | `python3 tools/check-doc-dupes.py [path …]` — merge-artifact gate over `docs/**/*.md` + `README.md`: no two body rows in one markdown table share a first-cell key, no heading repeats within a file, no git conflict markers. Kills the class that put `shortcuts[]` in the stage-5 table twice. Same-key rows in *different* tables are fine; a genuine same-table collision means restructure the table, not allowlist it |
| `tools/check-stated-counts.py` | CI | `python3 tools/check-stated-counts.py` — **a number stated in a reference document is bound to the thing that decides it.** A page that names how many teaching programs the `idiom-*` block holds is asserting a fact about the build that nothing computed, and it goes stale the way every uncomputed number goes stale. The class arrives at a MERGE, not at an edit: one branch adds a teaching technique and moves the count in `tools.md`, `prefab-procedure.md`, the `/new-delve` skill and the §2c index table; another, which never touches the library, adds a paragraph three lines above that table which names the old number twice — once as a cardinal, once as an ordinal denying the new one exists. `git merge-tree --write-tree` resolves the pair with **no conflict**, since they are different regions of one file, and the merged page tells an authoring session that a technique it has does not exist. Measured on that tree, `check-doc-dupes`, `check-dw-codes`, `check-reference-versions` and `check-grammar-ir-compat` are all green — and so is this gate on either branch **alone**. Built as a registry rather than as an idiom counter: an ORACLE names a computation over the tree plus the PHRASINGS prose uses to state it (a phrasing may carry an offset, which is how an ordinal claim about the next index states a cardinal fact about the count), and a SITE is a page — optionally one section of it — that states that oracle. Binding a further count is a SITE row and at most one oracle function, never a second script, because the object class is *a count stated in prose* and not the idiom index (CLAUDE.md: a capability belongs to the object class it acts on). The oracles today are the `idiom-*` teaching programs and the whole `library::PROGRAMS` table (`crates/grammar/src/library/mod.rs`) and the numbered rows of [`grammar.md`](grammar.md) §2c; that table is additionally held to the library **by set**, so a program with no row — or a row naming no program — is a red no count comparison would catch. States its binding count per site on every run, and a **site that states no count at all is a red**: that is the page whose sentence was reworded, and where the gate would otherwise have gone quietly dark. Per-phrasing hit counts are printed but a phrasing matching nothing is not a failure — the phrasings are the vocabulary for stating a count, not an assertion that each page uses each one. There is deliberately no allowlist: an opt-out here would be satisfied by the very drift it exempts. It does not bind every number in prose — most counts in `docs/reference/` are narrative measurements of one run and name no enumerable set — and it does not yet bind a claim about a tool's OUTPUT to what the tool prints. Because a page's own numbers are what it checks, a row in this table quotes no count it does not mean. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-numbered-doc-uniqueness.py` | CI | `python3 tools/check-numbered-doc-uniqueness.py [--base <ref>]` (default `origin/main`) — `docs/specs/spec-NNNN-*.md` and `docs/adr/NNNN-*.md` are both picked by an agent LISTING the directory and writing the next integer, and two new files never produce a git conflict, so a check that only looks at the working tree is green on every branch that collides: one branch carrying `spec-0033-declared-body-traversal.md` and a later one carrying `spec-0033-grammar-corpus.md` (which listed `docs/specs/` on a `main` that did not yet have the first file) both claimed 0033 for three days, and every per-branch check stayed green because each branch's own directory was internally consistent — the collision existed only in the union of the two trees. This gate computes that union: for every number, it takes every filename claiming it in the checkout AND every filename claiming it at `--base` (fetched by a preceding CI step, never by the script itself — it does no network I/O), and fails if more than one distinct filename claims a number. One rule catches three shapes without distinguishing them up front — a cross-branch collision (the shape above), a self-collision within this branch alone, and a pre-existing self-collision already on `--base` regardless of whether this branch touches that series. **Known limitation, stated in its own docstring, not just here**: it is blind to any OTHER open branch that has not yet merged into `--base` — nothing short of the GitHub API would see that, and this repo's CI token deliberately stays `contents: read` (same stance as `check-required-contexts.py`). The gap closes only when the first colliding PR merges and the second one's CI is RE-RUN against the updated base (automatic only if branch protection requires branches to be up to date before merging); absent that, a stale-green PR can still merge a collision. `DEC-NNNN` (single-file, checked by the local decision-ledger checker) and `DW-NNNN` (own dedicated uniqueness section in `check-dw-codes.py`) are deliberately NOT covered by this script — both considered and excluded with reasons in its docstring. States its binding count (files examined per series, on both sides) every run; a series with zero files on BOTH sides is a red. When `--base` is missing it refuses and prints how to fetch it, computed from the repository it is running in by `tools/lib/gitbase.py` — a full clone is told to fetch plainly, an already-shallow one to fetch at depth 1. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-version-ledger-uniqueness.py` | CI | `python3 tools/check-version-ledger-uniqueness.py [--base <ref>]` (default `origin/main`) — the numbering defect above, one layer down, on a different kind of number. A **version ledger** is picked exactly the way a spec number is: list the versions that exist, write the next one. So two branches take the same number for DIFFERENT surfaces and both stay green — one writes `MIRROR_SINCE = "1.1.0"` and the other writes `CONTRACT_SINCE = "1.1.0"` into the same new `crates/grammar/src/version.rs`. Unlike two new spec files those two DO conflict textually; the hazard is the **resolution**, because unioning both constants under one `1.1.0` compiles, passes clippy, and ships an engine that accepts a `1.1.0` document written against the construct it does not implement and silently drops it — ADR-0018 §7's failure, reintroduced by the fence's own numbering. A version's surface is named by its **fence anchor** (code, not prose, so it does not drift with wording): the `*_SINCE` constants plus each `RESERVED_VERSIONS` row for the grammar ledger, the `is_vNN` predicates resolved through `ordinal()` for `dsl_version`. Five rules over the union of the checkout and `--base` — one number one surface; one surface one number (a fence may not move once it is on the base); every number past the founding one is claimed by something (so skipping a number does not free you from declaring it); the list is append-only; and a reservation is deleted by the change that lands its surface. **Reservation** is how a number is held for a sibling change: it claims the version under the NAME of the constant that change will define, so a forward declaration and the change that fulfils it agree, and a forward declaration naming the wrong constant reds. Covers every version ledger in the repo, one row per ledger in `LEDGERS`; the object class is *a version ledger*, not one crate. **Two limitations, both in its own docstring**: it is blind to another open branch that has not merged into `--base` (the CI token stays `contents: read`), so what it guarantees is that once the FIRST of two colliding PRs merges the second goes red; and `dsl_version`'s anchors are self-naming (`0.11.0` forces `is_v11`), so rule 1 cannot bind there — rules 2–5 do, and a test pins the blind spot so closing it reds rather than leaving the docstring lying. States its binding count (versions, anchors and reservations per ledger, on both sides) every run; zero anchors on BOTH sides is a red, and a ledger that parses to zero versions or to zero traceable anchors exits **2** with the pattern to fix named — fix the pattern, never loosen the check. When `--base` is missing it refuses and prints how to fetch it, computed from the repository it is running in by `tools/lib/gitbase.py` — a full clone is told to fetch plainly, an already-shallow one to fetch at depth 1. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-workspace-git-deps.py` | CI | `python3 tools/check-workspace-git-deps.py` — no cargo workspace in this repo resolves a **git dependency** it has not quarantined. A required status check answers for the uptime of every host it reaches, and a git dependency is the one reach cargo gives a job no way to decline: it is cloned while RESOLVING the workspace that declares it, a workspace resolves all of its members, and neither `-p <crate>` nor `--locked` nor marking the dependency `optional` narrows that — all three measured, and the `optional` result is the surprising one. So `delvewright-render`'s Nucleation pin made 227 MB of clone from two repositories a precondition for `cargo run -p delvec`, in five required jobs that never build the render crate, and one transient TLS failure on that reach (`the SSL certificate is invalid; class=Ssl (16)`) reddened `tier 2` on a **docs-only** PR. Reads every `Cargo.lock` in the repo, which IS the resolved graph of the workspace that owns it, and flags any package whose `source` is `git+…`. `ALLOWED` carries `crates/render/Cargo.lock` with its reason — that crate is its own workspace precisely so the reach belongs to it — and an allowlisted lock carrying **no** git dependency is a finding too, since an exemption that has outlived its reason is how the next one gets waved through. It deliberately does not check that `crates/render` is still excluded: if it re-enters the root workspace the root lock gains the git packages and this reds on the lock, which is the property that actually matters. Registry (crates.io) dependencies are out of scope — content-addressed, cached, and not on the table. States its binding count; examining zero locks is a red. Third site of the class this repo already refuses at the `docs` job's `lychee --offline` and at `server-bootstrap-cache.sh`'s single Mojang fetch. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/check-compose-isolation.py` | CI | `python3 tools/check-compose-isolation.py` — isolation-by-construction gate: no service in `validation/compose.yaml` may pin a `container_name` or publish `ports`, because those are the only two things `docker compose -p <project>` does NOT isolate. `validation/owner-play.yaml` is the ONLY file allowed a fixed host port, and only `127.0.0.1:25565:25565` (the owner's client address) plus the container names a human needs to find; every other override may publish only ephemeral ports (`127.0.0.1::<port>`). Replaces `check-worker-override.py`, which merely required a matching `!reset` — so the pin survived and every caller had to remember an extra `-f`; the omission cost a run twice (`server`, then `bot`) |
| `tools/check-harness-dsl-version.py` | CI | `python3 tools/check-harness-dsl-version.py` — sync gate: the compiler's `SUPPORTED_DSL_VERSION` (`crates/dsl/src/envelope.rs`) must be a member of the harness's `SUPPORTED_DSL_VERSIONS` allowlist (`harness/src/critical-path.ts`). Nothing else relates the two files; spec-0026 moved the compiler to `0.9.0` while the harness allowlist still ended at `0.8.0`, and the bot tier refused every campaign at the version gate after the server booted and the bot connected |
| `tools/check-storybook-version.py` | CI + agent (**mandatory** at the `/new-delve` storybook step) | `python3 tools/check-storybook-version.py [--campaigns <dir>]` (default `campaigns/campaigns`) — every campaign storybook (content repo `campaigns/<id>/README.md` + one `README.<code>.md` per declared language) opens with `> **Requires delve engine <X> or newer** — last verified with delvec <Y>.`, within the first 10 lines, exactly once, byte-identical across editions. `<X>` must equal the MAX `dsl_version` over the campaign's six stage documents — the drift this gate exists for (the marker is the ONE internal-machinery item allowed in a player-facing README, so it must be TRUE); `<Y>` may not exceed the engine's own `DELVEC_VERSION`. Missing, malformed, buried, duplicated, or mismatched = red; an empty campaigns root is red too (a vacuous pass is worse than a failure). **And the marker must be the only version literal in the file.** Checking the marker harder is not what the v1.1.0 island release needed: its marker was correct and the storybook still told a host to `docker run …/delve-nobodys-cave-island:v1.0.0` — the version it had just replaced. THREE literals sat in that README (the marker; a `**v1.0.0** (exact engine pin: …)` campaign stamp, a lie by construction between releases since `main` is not a released version; the host command's tag) and only the marker was bound to anything, with the localized edition carrying a fourth as a translated gloss that had drifted a whole minor behind the untranslated stamp one line above it. Since a binding per number would have to be invented per campaign, the rule is that a storybook may carry **no** version literal but the marker, and the numbers those lines wanted live where they are GENERATED (the release page, `versions.toml`). Two recognisers, one rule, so the message is actionable: a **pinned OCI tag** — `<registry>/<path>:<tag>` with `tag != latest`, the line a host copy-pastes — reports the file, line and tag and says to write `:latest` (which *is* the storybook's claim) and send an exact-version reader to the release page; a **bare `v?N.N.N`** anywhere else covers the stamp and the gloss (two-component numbers are deliberately not versions — `CC BY-SA 4.0` is a licence; `:vX.Y.Z` as a prose placeholder is not a literal). The marker line and any malformed attempt at one are exempt, so a broken marker stays ONE finding, and the literal clauses run even when the marker is absent (an unstamped storybook can still hand out a dead image tag). States the literal clauses' own binding count — storybook files read — and reading ZERO of them is red: allowlisting the only campaign that ships a storybook would otherwise leave them examining nothing while reporting green. Campaigns blocked by an in-flight content PR sit in the script's `ALLOWLIST` with the blocking PR and its removal condition, are PRINTED on every run, and go red the moment their marker becomes correct. Runs in CI as `campaign storybooks (engine-version marker)` — over the content repo at `versions.toml` `[content].sha`, which is bumped by hand, so a storybook defect is caught at the next pin bump rather than on the content PR that introduces it (the content repo runs no CI on `campaigns/**` today); the content repo's own campaign CI can run this same script against a pinned engine checkout, the way `prefab-audit.yml` there already builds `delve-admit` from one |
| `tools/planner-state.sh` | agent (**session start** — the planner opens on this page, never on recall) | `./tools/planner-state.sh` — a presence check on the other half of the constitution, then one page of coordination state **computed** from git, gh and the decision ledger. **The first section is neither computed nor optional**: `CLAUDE.md` is half a constitution, and the other half — dispatch, review and merge gates, staging, decision sessions — is deployment-specific, so it lives in `CLAUDE.local.md`, gitignored. That file is **loaded by the same memory loader as `CLAUDE.md`**, which is what gives the local half the same force as the checked-in half; it is instructions rather than text an agent is shown, and this page therefore does not print it — an earlier design did, which made the half that governs dispatch and merge into a tool result, the standing of a doc line. What this page adds is the one thing a loader cannot: a missing memory file loads **silently**, so its **absence is a refusal that names the file** and says the session is running on half a constitution, and its presence is one line stating its size. A gitignored file is missing on exactly the machine that never had it, so a silent no-op there would be the UNRUN vacuity mode wearing the fix's clothes. The decision ledger and its checker moved to the same gitignored directory for the same reason and are invoked from here, because CI can no longer see them. The computed sections: both checkouts' commit + dirtiness; **whether either repository is SHALLOW** — the one corruption on this page that reports numbers instead of errors, so it is the one a planner cannot notice unaided: a truncated history makes `merge-base` return nothing, `git merge origin/main` answer "refusing to merge unrelated histories", and ahead/behind come back as confident integers computed from the two commits that survived (measured: a branch 1 ahead and 5 behind read as 401 and 1). It is sticky and shared — the boundary lives in the object store, so one `--depth` fetch in any linked worktree shallows the main checkout too — so the finding names the condition, says not to reset or force-push on any number that checkout produced, and gives the repair (`git fetch --unshallow --no-tags`). The section states how many checkouts it examined and how many were shallow, and says so when it could examine none; every worktree beyond main with dirty/unpushed counts (each is owned by an open dispatch or it is garbage); every branch carrying commits on **no** remote — the one category of git state a machine failure actually destroys, and the section that on its own first run surfaced 13 unpushed spec-0016 commits and a complete unpushed demo campaign that a same-day manual sweep had missed; open PRs in both repos; and the local decision ledger's open/unenforced rows with ages. Exists because every expensive coordination failure in the record — an undelivered worker result, a forgotten inventory, 36 unreclaimed worktrees — was planner state living in a context window instead of an artifact; when this page and a planner's narrative disagree, the page wins. **Invoked by the events, not a doc line** (the UNRUN shape), and the events are chosen for a one-long-lived-session workflow: `.claude/settings.json` runs it unconditionally on `SessionStart` — which fires on startup, resume, **and after every context compaction**, exactly the moments a planner is a reconstruction of its former self — and on `UserPromptSubmit` with `--if-stale 12`, so inside one long session the page refreshes with the next message once its `.git/`-local stamp is older than 12 h and stays silent otherwise. The page also prints every `docs/ideas.md` row still in `captured`/`elaborating` with its age — the binding that makes an owner idea mechanically un-losable (an idea leaves that list only by graduating to a spec/task or by an explicit owner `declined`; capture protocol in `docs/ideas.md` itself). **One section of it is not read-only**: the worktree section runs `tools/worktree-reclaim.py --apply`, because listing stale worktrees is what this page did while the disk filled twice — reading a list is not draining it, and the obligation to drain it lived in a sentence. That is the only thing here that changes the machine, and it is safe unattended because of what it demands before deleting anything (see that tool's row); everything it refuses is reported instead. It never fails the session — a section that cannot be computed says so and the page continues, because an absent answer is itself state worth seeing |
| `tools/check-required-contexts.py` | CI | `python3 tools/check-required-contexts.py` — keeps `.github/required-status-checks.txt` and `ci.yml`'s job `name:` values in lockstep, **both directions**. All ten CI jobs are required status checks: an advisory job is a job that does not gate, and at three of ten required, `tier 2` (datapack load + the whole generated PackTest suite), the storybook engine-version marker and the prefab determinism gate did not block a merge — only `gh pr merge`'s own refusal on UNSTABLE did, and `--admin` went straight through. Requiring all ten creates the deadlock this checker guards: branch protection matches a required context by its NAME STRING, so a renamed job stops reporting forever and blocks every PR *including the one that would fix it*. Renaming a job is therefore three steps — add the new context to protection, merge the rename + manifest update, drop the old context. The reverse direction matters as much: a job with no manifest line is a gate nobody must obey, which is how the seven drifted. `ADVISORY_JOBS` in the checker is the only exemption and is empty on purpose; each entry needs a reason a future reader can weigh. Reads only the repo — CI's token has `contents: read` and cannot see branch protection, and a gate that needs a privileged token is a gate that quietly stops running. States its binding count; parsing zero jobs or zero contexts is a red |
| `tools/assert-run-approved.sh` | CI (release) | `bash tools/assert-run-approved.sh <environment>` — the run-time half of the above, and the first step of `publish-crates`. Reads this run's own approval history (`/actions/runs/<id>/approvals`) and refuses when no `approved` entry names the environment: a run that was never held records none, which is exactly the state the incident run is still in. Needs only `actions: read` on its own repo, so it never becomes a gate that quietly stops running for want of a privileged token. Does **not** prove the approver differs from whoever pushed the tag — that is `prevent_self_review`, configured in the same out-of-band settings; what this asserts is that a human passed through a review UI at all, which is the step that did not happen. Materialises the API response to a file before parsing, never `curl | jq` (a pipe hides the producer's exit status) |
| `tools/check-skill-version.py` | CI | `python3 tools/check-skill-version.py` — ADR-0016's **third version line**, made true. `.claude/skills/new-delve/SKILL.md`'s frontmatter declares the skill's own product version (`version:`), the engine window it drives (`requires: delvec: ">=X.0.0 <A.0.0"`) and the engine it was proven on (`verified_with:`); this gate is what stops those being a `requires:` nobody reads. **The last two are different claims and bind differently.** (1) `requires.delvec` is COMPATIBILITY — what a creator reads as "older engines will not work" — so it is ADR-0016's own **major window**, stable across a whole line, and binds by MEMBERSHIP: the ceiling is the floor's next major and this repo's engine sits inside the window (the direction that catches `delvec 2.0.0` shipping beside a skill that still says `<2.0.0`). (2) `verified_with` is EVIDENCE — the one engine this tree actually exercises the skill on — so it binds by EQUALITY to `crates/compiler/Cargo.toml`'s `[package] version`, the single source `DELVEC_VERSION` derives from, in **both** directions: above names a compiler that does not exist, below is stale evidence from a build no longer in the tree. Restamping it is one line in the engine's own release commit, and it never moves `version:` or the compatibility window. Pinning the window's floor to the engine instead — the first draft of this gate — would make the frontmatter assert after every release that older engines are unsupported, which nothing tested, and would make ADR-0016's own example un-writable at 1.1.0. (3) Every `delvec` subcommand the skill's code spans name, and every long flag named with it, must exist in the clap CLI parsed out of `crates/compiler/src/main.rs` (nested `edit apply`/`preview` actions fold into their parent, so `delvec apply` is correctly not a subcommand) — that is what makes the window a claim about a real command surface rather than a shrug. States its binding count on every run — currently 9 distinct subcommands, i.e. all of them; **extracting zero subcommand references is a red**, as is parsing zero subcommands out of `main.rs`, because a green that binds to nothing is vacuous (CLAUDE.md). **Known non-proof, stated in the script's docstring and in its OK line**: a window floor that has drifted too LOW — the skill adopting a subcommand added in 1.1.0 while the window still opens at 1.0.0 — is invisible here, because check 3 tests against the CURRENT CLI and this repo holds one engine. A green means the window is internally consistent and the engine in the tree is inside it, never that the whole line was tested. Runs as a step of the `docs (local link check)` job — a step, not a job, since every job name is a required status context |
| `tools/check-publishable.sh` | CI | `bash tools/check-publishable.sh [--allow-dirty]` — ADR-0017: proves `cargo install delvec` will work **without publishing anything**, on every push. `cargo publish` is a one-way door (a version can never be reused, a name never freed), so the packaging contract cannot wait for release day. Three checks: (1) both publishable crates `cargo package`, which is where a path-only dependency, a missing `description`/`license` or a stray `publish = false` fails by name; (2) the GENERATED manifest crates.io will serve carries no dependency `path`, carries the exact `=` requirement `versions.toml [engine].dsl_crate_req` declares, and has dropped **every** path-only dev-dependency entirely — which is *why* an unpublished sibling may be used by `delvec`'s tests, verified rather than assumed. That set is read out of `crates/compiler/Cargo.toml` and its binding count printed, never named in the script: a named dev-dep binds to the one somebody thought of, and the second such dependency arrived and was examined by nothing; (3) the packaged `delvec` tarball, extracted into a temp dir with **no workspace above it and no path dep anywhere**, builds its binary with `delvewright-dsl` supplied from the packaged DSL tarball, i.e. the bytes crates.io will hold. Check 3 is what stops the gate being vacuous: `cargo publish --dry-run` alone could satisfy the sibling dependency from `crates/dsl` on disk and prove nothing about a stranger's download. **Does not prove** that crates.io accepts the upload — nothing pre-publication can; the release workflow's post-publish index poll covers that. Runs as a step of `rust (fmt, clippy, test)` — a step, not a job, since every job name is a required status context. `--allow-dirty` is local-only: CI works from a clean checkout so the VCS-dirty refusal stays armed. Creates `target/` before redirecting a log into it and reports a missing or empty log as such — the v1.0.0 run had no build cache, so the redirect failed, `cargo package` never ran, and the script blamed it anyway (`tools/check-shell-redirect-dirs.py` now forbids the shape repo-wide) |
| `tools/build-release-binaries.sh` | CI | `bash tools/build-release-binaries.sh (--list-targets \| --check-only \| --target <triple>)` — the ONE definition of the release shelf, called by both the standing CI gate and `engine-release.yml`, so the two cannot drift. Holds no copy of any pin: version and targets come from `versions.toml [engine]` (a hardcoded triple is a `check-versions.sh` failure). `--check-only` is the CI job `engine binaries (cross-build shelf)`: `cargo check` every target on one ubuntu runner — rustup ships std for all five regardless of host, and build scripts still run, so a new dependency that will not compile for musl/msvc/darwin fails on the PR that adds it instead of at release time with the tag already pushed; **an empty target list is a red**, not a pass. `--target <triple>` builds, archives (`delvec-v<version>-<triple>.tar.gz`, binary + LICENSE for GPL-3.0 §4) and emits the checksum line. `.tar.gz` for every target including Windows on purpose: one archive format is one extraction path for ADR-0014's bootstrap, and a per-OS format branch is somewhere for the shelf to end up half-built. `*-linux-musl` links with rustup's own `rust-lld` rather than `musl-gcc`, so the same command works on the owner's macOS workstation and on the runners (measured: cross-linked macOS/arm64 → x86_64 musl, `static-pie`), and every musl artifact is then asserted to carry **no `PT_INTERP`** — read out of the ELF header, not pattern-matched on `file`'s prose — because a musl binary that quietly acquired a dynamic interpreter breaks on a stranger's machine rather than here. Every value it reads out of `versions.toml` is produced by a python that pins `newline="\n"`: without that, Windows' `\r\n` made the msvc target compare unequal to itself on the msvc runner alone (`tools/check-python-shell-newlines.py`) |
| `tools/crates-io-publish.sh` | CI | `bash tools/crates-io-publish.sh (--plan \| --publish)` — the only path to crates.io; no human ever runs `cargo publish` for this project (ADR-0017). **Idempotent by checksum**: for each crate it asks the sparse index what is already there — absent → publish; present with our exact sha256 → skip; present with *different* bytes → hard fail by name, because crates.io will never accept the new bytes. That is what makes the half-succeeded sequence (`delvewright-dsl` lands, `delvec` fails) safely retryable instead of burning a version. The index lookup is **bind-tested** against `serde 1.0.0` before it is trusted, because a broken lookup would report every crate absent and silently disable the skip branch — the unbound-gate class (the first draft of this script had exactly that bug: `python3 - <<'PY'` binds stdin to the heredoc, so a piped index body was discarded). One `cargo publish -p … -p …` invocation, so cargo owns dependency ordering and its own wait-for-index; this script adds the POST-condition instead — a poll on an observable (both crates visible with our checksums), 180 s timeout, 5 s interval, never a sleep chosen by feel. `--plan` touches nothing and needs no credential; `--publish` reads `CARGO_REGISTRY_TOKEN` straight out of the environment, never runs `cargo login`, never writes a credential to disk |
| `tools/check-shell-pipe-shortcircuit.py` | CI | `python3 tools/check-shell-pipe-shortcircuit.py` — forbids a consumer that stops reading before its producer stops writing on the right of a pipe (`grep -q`, `grep -m N`, `head -N`) in every repo `*.sh`. Under `set -o pipefail` such a consumer exits at the first match, the producer dies of SIGPIPE (141), and pipefail promotes 141 to the pipeline: **the pipeline reports failure precisely because the match succeeded**, at a rate set by how much the producer still had to write. Measured against a live, healthy server whose log contained `Done (` exactly once: 28 false negatives in 30 runs. This is what made `playtest-server.sh` print "server did not come up" for a server that was up, and the same shape sat under both 25565 guards and `dw_mutex_port_bound` — where a false negative frees the owner's sacred mutex while a human is playing. Prescribed idiom: capture, then test with bash's own `[[ $out == *pat* ]]` / `[[ $out =~ re ]]` / `${out%%$'\n'*}`, spawning no process at all. `docs/experiments/` is excluded (frozen record); `EXEMPT_LINES` carries exactly one justified line-level exemption, and a stale entry there is itself a red |
| `tools/check-python-shell-newlines.py` | CI | `python3 tools/check-python-shell-newlines.py` — every **inline** python a repo shell script or workflow `run:` block executes and that writes to stdout must declare `sys.stdout.reconfigure(newline="\n")`. Python's text-mode stdout translates `\n` to `\r\n` **on Windows**, and the trailing `\r` survives both command substitution and `IFS= read -r` — so on the first-ever release run (v1.0.0, 2026-08-06) `tools/build-release-binaries.sh` rejected `x86_64-pc-windows-msvc` as "not in versions.toml [engine].targets" on the msvc runner and only there, while the four unix targets went green. Invisible on every runner but one, and the eleven green checks on the PR that added the script never ran that one. The rule is deliberately "every printing program", not "every captured one": the site that broke was a heredoc inside a shell FUNCTION whose capture happens at three separate call sites, so a checker reasoning about the invocation would have passed the one bug it exists to catch — and pinning `\n` on a stream nobody reads costs one line and changes nothing. Out of scope by rule, not by allowlist: `python3 script.py` (no inline text — a committed `.py` is not a shell boundary), programs with no `print(`/`sys.stdout` (they answer by exit status), and python run inside `docker run`/`docker exec` (a pinned Linux image by construction). States its binding count; zero files or zero programs is a red |
| `tools/check-live-commands.py` | CI | `python3 tools/check-live-commands.py` — nothing in this repo may speak to a Minecraft server without being able to hear it. Three rules, the first two driven by pinned artifacts rather than a typed list. **(1)** A shell/Node file that invokes `rcon-cli` must reach it through the shared rejection rule (`tools/lib/rcon.sh` / `tools/lib/rcon.mjs`); six sites did not, and the rule already existed — correct — privately inside one spike, which is exactly why the next two callers wrote the unchecked version. **(2)** A `gamerule <name> <value>` line anywhere in `*.sh`/`*.mjs`/`*.js`/`*.ts`/`*.rs`/`*.mcfunction` must name a rule the pinned 1.21.11 server has, checked against the literal children of `gamerule` in `crates/compiler/data/commands-1.21.11.json`, so it cannot drift from ADR-0009. Motivating instances: the gallery's four legacy camelCase rules (which cost `admit:load` and `admit:finish` in their entirety — the gallery world had no objectives, nothing forceloaded, nothing placed), `spike-jump-arc`'s `fallDamage`, `warden-probe`'s `doMobSpawning`/`randomTickSpeed`. **Known non-proof, stated in its docstring**: rule (2) only sees a `gamerule` with a LITERAL name and a literal value — a dynamic name (`gamerule ${g}`) is a probe it cannot judge — and line comments are stripped, which can hide a violation but never invent one. **(3)** `rcon.sh`'s refusal list and `rcon.mjs`'s must recognise the SAME reply shapes. The rule has to exist twice (shell sources, Node imports), and two copies of one truth is the very shape rules (1) and (2) exist to prevent — so they are compared, not trusted. Found on its first run: the area-effect-arrow spike's private copy knew `No targets matched`, `Malformed ` and a broad `Failed to `, which the shared rule did not, while the shared rule knew the `<--[HERE]` cursor, which the spike's did not. **Every private copy ever found was silent on exactly the refusals its own run never provoked**, so the shared list is the union and a shape leaves it only when the pinned server stops producing it. `docs/experiments/` is excluded (frozen record); a negative fixture may carry an inline `check-live-commands: allow (<reason>)`, and **every honoured exemption is printed with its reason on every run**. States all three binding counts every run; **zero live command sites, zero gamerule lines or zero compared shapes is a red**. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/lib/publishable.py` | library (CI gates) | Imported, never run. The repo's ONE answer to "which files does a stranger read on crates.io": every crate under `crates/*/` whose `[package] publish` is not `false`, resolved through its `[package] readme` key. `check-crates-io-readmes.py` and `check-reference-versions.py` both consume it, so a crate that becomes publishable inherits both gates with **no edit to either**. It globs the directory rather than reading `[workspace] members`, and that is load-bearing: `crates/render` is deliberately EXCLUDED from the workspace, so a members-only derivation would never see the one crate whose publishability is invisible from the workspace table. The glob is then cross-checked against the root manifest — a crate the root names but the glob cannot reach raises rather than silently shrinking the binding count. Every failure is a `DerivationError` raised, never an empty list returned: a gate that caught this and carried on would be the exact "it matched zero objects" vacuity CLAUDE.md names. Lives here for the same reason `rcon.{sh,mjs}` does — the rule that lives correct inside ONE caller leaves the next caller nothing to reuse |
| `tools/lib/gitbase.py` | library (CI gates) | Imported, never run. Resolves a gate's `--base` ref and owns the ONE remedy printed when it is absent. The remedy is **computed from the repository it will be run in**, never quoted from CI: a full clone gets `git fetch --no-tags <remote> <branch>:refs/remotes/<remote>/<branch>`, an already-shallow one gets the same line with `--depth=1`. The two are not interchangeable in either direction. `--depth=1` in a full clone converts it — and every worktree sharing its object store — into a shallow one, and a shallow repository does not fail, it **answers wrong**: on a branch 1 commit ahead of `origin/main` and 5 behind, `merge-base` returns nothing, `git merge origin/main` says "refusing to merge unrelated histories", and the ahead/behind counts come back 401 and 1. The refusal costs minutes; the counts are what someone resets or force-pushes on. A plain fetch in an already-shallow checkout is the opposite error, pulling a branch's whole ancestry (400 commits against 1, measured under CI's merge-preview shape) to no purpose. A `--base` that is not `<remote>/<branch>` gets prose rather than a command, because no single fetch is certain to install it. `check-numbered-doc-uniqueness.py` and `check-version-ledger-uniqueness.py` both consume it, so a third gate needing a base ref inherits the correct remedy with no decision to make. `tools/tests/test_gitbase.py` does not merely assert the wording — it RUNS the printed command against a throwaway clone and re-examines the repository, because the property under test is "this instruction does not damage the thing it is helping with". Lives here for the same reason `rcon.{sh,mjs}` does: the unsafe line existed twice because the first gate wrote it inline and the second copied it |
| `tools/check-structure-emitters.py` | CI | `python3 tools/check-structure-emitters.py` — **which sites owe the block-state rule, decided by discovery rather than by a list.** The rule (`prefabs/invariants.rs::assert_blocks_are_real`, `BlockRegistry::validate`) was placed at five tileset generators by hand; a sixth emitter — `hello-room`, which hand-built its own palette from an `examples/` target — was missed, so nothing judged the states it wrote. **(1)** Every tracked `.rs` naming `fastnbt::to_bytes` (the one way anything here produces NBT) must either name the block-state rule — following its own `mod` declarations, since a generator may split palette from emission — or be listed in the script's `NOT_EMITTERS` with a reason, which is printed on every run. The exemptions are enumerated file by file on purpose: a class exemption (“anything under `tests/`/`examples/` is a fixture”) is the exact assumption that hid the sixth emitter, which was production tooling living in `examples/`. The polarity is the point — a list of inclusions fails silently when it misses a site, a list of exclusions fails loudly. **(2)** Every emitter check (1) accepted must also derive its connection states — the piece goes through `connections::resolve`, which computes each `multipart` property from the blocks beside the cell — or be listed in `NOT_CONNECTION_EMITTERS` with its reason. Judging the palette is not enough: an omitted connection property and one written at the block's default both pass `validate`, and the default is *disconnected*, so completing a state from `BlockRegistry::default_state` ships the isolated post the author never meant and empties the `DW0735` predicate at the same time — the check going green by ceasing to bind. The rule and that defeater live seventy lines apart in one impl block, which is why the obligation is bound to the emitter rather than left to a doc line. **(3)** Every `prefabs/*/Cargo.toml` on disk is named by the `prefab-generators` job's cache list and both of its `for g in` loops, in both directions, so a new generator cannot be added without CI running it twice. All three checks print their binding count. |
| `tools/lib/rcon.sh`, `tools/lib/rcon.mjs` | library (agent + human) | Sourced/imported, never run. The repo's ONE definition of "the server refused that command", measured on the pinned 1.21.11 server: a PARSE failure (every Brigadier error carries a `<--[HERE]` cursor) and a REFUSAL (`That position is not loaded`, `No entity was found`, `No targets matched`, `Failed to `, …). The list is the **union** of every private copy that has been found, and the two halves are held equal by `check-live-commands.py` rule (3). Shell: `dw_rcon <container> <cmd>` asserts and returns non-zero on a refusal, `dw_rcon_probe` is the unjudged form, `DW_RCON_ARGS` carries extra `rcon-cli` flags. Node: `rconChannel(container).run(cmd)` throws, `.probe(cmd)` does not; `REJECTION`/`assertAccepted` are exported for a tool with its own transport (the death spike's pipelined channel). **Which channel a call uses is a statement about that call**: `probe` is for a liveness poll or a measurement whose subject IS the rejection, and nothing else. `tools/check-live-commands.py` is what makes the choice unavoidable |
| `tools/check-shell-redirect-dirs.py` | CI | `python3 tools/check-shell-redirect-dirs.py` — every `>`/`>>` in a repo `*.sh` that writes **into a directory** must have that directory guaranteed first: a `mkdir -p` covering it, a `mkdir` naming it exactly, a `mktemp -d`, a directory tracked in this repo, or an always-present one (`/tmp`, `/dev`, and `/data` — the itzg image's own data dir, written only from inside that image). Variables are resolved through their literal assignments, so hoisting the path into `LOG=` does not hide it, and `>` inside a quoted string is text, not a redirection. **Why**: the shell opens a redirect *before* running the command it captures, so on the v1.0.0 preflight — a runner with no build cache and therefore no `target/` — the redirect failed, `cargo package` never ran, and the else-branch `sed`ed the log whose absence was the finding, reporting "cargo package failed" about a command that had not been executed. The general form is **an error path must not depend on an artifact the error may have prevented from existing**; this gate removes the root cause, and the other half — a failure branch that names a missing or empty log instead of quoting it — is exercised by `tools/tests/test_check_shell_redirect_dirs.py`, since syntax cannot check a message. States its binding count |
| `tools/check-trial-verdicts.py` | CI | `python3 tools/check-trial-verdicts.py` — every judged verdict in `docs/trials/trial-*.md` declares what bounded it: `artifact-bound` (the instrument could frame the thing being judged) or `instrument-bound — <named blocker>` (it could not, so the answer is partly about the tooling and the blocker is a capability-gap finding under playtest-methodology rule 4). A rubric answer is a judgement, and a judgement three paragraphs away from its own disclaimer ships as a verdict: trial 0001's R1 for run 1 read `partial` while the same section recorded that no camera could take a square-on elevation — re-photographed with an aimed camera from the same delivered bytes, the answer is `yes`. Enumerates the entry points rather than trusting a checklist — every trial record, every `## Run N — result` section, every rubric row carrying a bolded verdict — so a record cannot gain a run or an answer without gaining the declaration. **Known non-proof, stated in its docstring**: it establishes that the declaration exists and is well-formed, never that it is true; the reviewer's question is *what would this verdict have to look like for the instrument to be unable to tell?* States its binding counts; a trial record yielding zero verdicts is a red, because the likely cause is a rubric table reformatted out from under the parser. Runs as a step of `docs (local link check)` — a step, not a job, since every job name is a required status context |
| `tools/extract-sound-registry.py` | maintenance | `python3 tools/extract-sound-registry.py <registries/data.min.json> <out.json>` — regenerates the compiler's sound registry for a new MC pin (positional args only, no `--help`) |
| `tools/extract-item-stack-sizes.py` | maintenance | `python3 tools/extract-item-stack-sizes.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-stack-sizes-1.21.11.json`, the item→`max_stack_size` table `DW0436` reads, for a new MC pin (positional args only). Pins and checks the source SHA-256; refuses to default a missing component rather than assuming 64 |
| `tools/extract-item-combat-stats.py` | maintenance | `python3 tools/extract-item-combat-stats.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-combat-1.21.11.json`, the item→`attack_damage`/`attack_speed`/`armor`/`armor_toughness`/`nutrition` table the spec-0023 winnability arithmetic reads (`DW0472`, `DW0474`), for a new MC pin (positional args only). Pins the source SHA-256 and refuses any non-`add_value` modifier rather than mis-summing it |
| `tools/extract-damage-types.py` | maintenance | `python3 tools/extract-damage-types.py <damage_type/data.min.json> <tag/damage_type/data.min.json> <out.json>` — regenerates `crates/compiler/data/damage-types-1.21.11.json`, the damage-type→`{bypasses_armor, scaling}` table `DW0473` reads (positional args only). The finding it pins: `damage-players` emits `/damage` with no attacker, so an Easy campaign's scripted hits are NOT halved — only `scaling: always` types scale |
| `tools/extract-entity-tags.py` | maintenance | `python3 tools/extract-entity-tags.py <tag/entity_type/data.min.json> <out.json>` — regenerates `crates/compiler/data/entity-tags-1.21.11.json`, vanilla's built-in `entity_type` tags, for a new MC pin (positional args only). Pins and checks the source SHA-256. These are Mojang's own answers to "which entity types do X", which is the only acceptable source for such a question here: `DW0496` reads `#minecraft:burn_in_daylight` from it rather than shipping a hand-written species table |
| `tools/extract-font-metrics.py` | maintenance | `python3 tools/extract-font-metrics.py <client.jar> …` — regenerates the font metrics behind the DW0330 text-fit lint (positional args only, no `--help`) |
| `tools/playtest-server.sh` | human | `tools/playtest-server.sh up <campaign-dir> [--lang L] [--prefabs D] [--delvec BIN] [--name N] [--out D]` / `down [--name N]` / `status` — builds a campaign and serves it as a local throwaway itzg container for the owner's direct-connect playtest (with `validation/owner-play.yaml`, one of the two sanctioned host-25565 bindings; validation ladders never bind it). `up` TAKES the 25565 mutex as `owner-play-session` (releasing it again if the build or boot fails) and `down` releases it by name. `up` rcon-verifies dw objectives + a `dw_npc` entity, clears the sidebar, installs the resource pack when `DELVEWRIGHT_RESOURCEPACKS_DIR` is set, and prints the connect address; `down` is the server-lifecycle teardown the moment feedback arrives. Refuses to start over an existing binding |
| `tools/worktree-reclaim.py` | planner operations (**not** an authoring tool — it enters no skill, because a creator building a delve never meets it; it is run by the hook below and at a merge) | `python3 tools/worktree-reclaim.py [--apply] [--after-merge BRANCH] [--tree PATH] [--lease PATH \| --release PATH] [--targets-only] [--free-below GIB] [--repo PATH]…` — **reclaims the worktrees whose work has landed, in both checkouts, and refuses to touch any other**. A worktree is created by a dispatch and destroyed by the merge that lands its work; while that obligation lived in a sentence in a document it held for about one dispatch in four, and the disk filled twice — each tree carrying a full `cargo target/`, until `df` itself was unrunnable. So it is **bound to the events, not to a checklist** (the UNRUN vacuity mode): `tools/planner-state.sh` runs it with `--apply` on `SessionStart` and on every stale `UserPromptSubmit`, which is the one thing a merge typed by hand, a pull request closed in a browser and a stopped worker whose branch someone else pushed all pass through; `--after-merge BRANCH` is the same proof applied to one tree in the same breath as the merge. **What counts as permission to delete** is the question `CLAUDE.md`'s sixth vacuity mode asks — could the thing this is meant to exclude produce it? Quiet cannot be evidence, nor mtime, nor "no process has its cwd there" (an agent between tool calls has no process), nor the commit being reachable from the remote (a worker reading pinned content at a detached commit has exactly that). The one key a live dispatch cannot forge is the **remote's own pull-request state**: merged or closed is asserted by an authority off this machine about work that has already landed there. That key is only worth what the QUESTION is worth, so the remote is asked **about the branch being decided, one query per branch**, never by fetching a list and looking the branch up in it: a bulk listing returns the most recent N and never says it stopped, so a branch whose request falls outside the window reads as "no pull request on the remote" — a fact about a fetch wearing the clothes of a fact about the remote, which is the UNTRAVERSED vacuity mode, and it also made the tool structurally unable to reclaim anything older than one page. Each per-branch answer proves it is not itself cut off: an answer that fills its own row limit is treated as no authority at all. The run reports how many branches it **asked**, which is the only figure that is coverage rather than the size of a fetch. It is necessary and never sufficient — every reclamation also demands a clean tree, **no** commit absent from every remote, that **nothing outside the tree links into it**, no lease, and git's own re-check (`worktree remove` is called without `--force`). Dirty or unpushed outranks everything, by any path, for any reason. **Reachability is part of liveness**: `campaigns` inside one worker's tree is how it reads content, so a sweep judging a tree in isolation deletes a checkout two live workers are reading and neither fails — the workers keep running and measure zero over a directory that stopped existing. A reverse-reference index over the whole scratch area is therefore built before any verdict, and every **dangling** symlink it meets is reported as a finding of its own, because that failure is silent by construction. A **detached** checkout is never swept: a pinned-content read tree and a spent verification tree are indistinguishable by every local signal, so it is reclaimed only when an operator names it with `--tree`, which prints what it overrode and still enforces every other key. A live dispatch removes itself from the question with `--lease` (stored in the worktree's git admin directory, so it cannot make the tree dirty and cannot outlive it); a lease is honoured **even when its window has elapsed** — an expiry that voided it would be "quiet means dead" wearing a timestamp — and a lease over a merged branch is reported, never silently resolved. Below `--free-below` GiB free (default 25) the run widens to **rebuildable output only**: `target/` directories identified by cargo's own `CACHEDIR.TAG` signature, never inside a leased, referenced or self tree. Also reports the harness's `worktree-agent-*` branches already contained in `origin/main`. Default mode is a **dry run**; every run states how many repositories it enumerated, worktrees it examined, symlinks it resolved, and the reason per kept tree, and a run that examined nothing says so as a FINDING. Nothing in it ever changes directory — a `cd` in a compound command persists through the rest of it, which is how `gh` and `git worktree` have been made to answer confidently about the wrong repository |
| `.github/scripts/mecha_crosscheck.py` (not under `tools/`) | CI | `python3 .github/scripts/mecha_crosscheck.py [<datapack-dir>]` (default `out/datapack`, positional only) — ADR-0011's independent cross-check: re-parses every emitted `.mcfunction` line against the pinned 1.21.11 command tree with `mecha==0.104.1` + `beet` (installed by the job, never a repo dependency), so a line the compiler's own first-party validator accepted and mecha rejects is a bug in one of the two. Never the emission path. Finding zero `.mcfunction` files under the directory is a red, not a pass. Runs as the CI job `mecha cross-check (PR only)` |

## 7. Validation stack (`validation/`)

Docker compose is the CI-equivalent environment (CLAUDE.md *Environments*). All
profiles boot the world the compiler declared, via the shared
`world-settings-entrypoint.sh`. Prose:
[`../../validation/README.md`](../../validation/README.md).

| Profile | Class | Command | What it is |
|---|---|---|---|
| `play` | human | `EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile play up` | the shipped delve image, joinable at `localhost:25565`. `owner-play.yaml` is what publishes that port and pins the `delvewright-server` name — `compose.yaml` alone publishes nothing, so no ladder can take the owner's address |
| `playtest` | human | `EULA=TRUE CREATOR_NAME=<mc-name> docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile playtest up --build` | `play` plus the creator overlay: `/trigger dw.note` stamps the log for `delve-harvest` |
| `validate` | agent | `EULA=TRUE validation/bot-run.sh --project dw-<id>` (the entry script; `--project` REQUIRED) | server + mineflayer critical-path bot. Three labelled ladder stages: `critical-path` (the whole delve, with bounded **combat-assist** windows at each encounter) and `die-retry` (≥2 scripted deaths per encounter, proving respawn → return → re-engage with no lost progress) once the build carries a `validation/combat-plan.json` (spec-0023); and **`death-loop`** once it carries a `validation/death-plan.json`. The death loop is the one mechanic no other tier can witness — a PackTest fake player is permanently undamageable, measured 2026-08-03 and again 2026-08-09 — so it walks a real client INTO every declared lethal volume, dies there, and asserts every consequence the campaign PROMISED against what it observed: the volume's own wording reaching that player, the declared forfeit leaving the currency ledger, the recovery stake standing at the anchor the compile-time placement table chose, the walk back from the respawn seat, an exact restore under a double right-click in one tick, and the retirement of the collected hardware. Every number comes from `death-plan.json` (the campaign's declarations plus the placement table), never from the emission, and the report's `death_loop.binding` states what was examined — `deaths_observed: 0` is a finding, not a pass. Two harness actions on the world, both named here and both reverted: the currency objective is put on the sidebar display slot (a vanilla server only broadcasts an objective it is tracking, and mineflayer 4.37's own scoreboard model never updates on 1.21.11 because it gates on a `packet.action` field the version no longer has), and the bot takes its respawn MANUALLY one second after dying instead of letting mineflayer answer the death packet in the same event-loop turn — no player respawns inside one tick, and the engine's death edge is specified on the corpse. The bot also treats every declared lethal volume as impassable when pathfinding, exactly as the compiler does, so the walk back from a death never routes through the hazard that caused it. The run writes `validation/run-out/<project>/run-report.json` (project-scoped) — an `encounters` block (per encounter: assist policy and the phase the run reached), every assist window with its encounter id and ticks, every death trial (recorded when the death is TAKEN, so an aborted run still carries it; each says whether its loop reached a verdict and what was waiting at the end of it — `outcome`: `re-engaged` (hostiles are back) and `cleared-before-retry` (nothing left to fight, objective already complete) both PASS, `stranded` (nothing left to fight, objective unfinished) is a soft lock and reds the run). The bot **performs the path's `rest` steps**: it walks to the bonfire, RIGHT-CLICKS the `dw_bonfire_<i>` affordance — which is what enables the `dw.rest` trigger; chatting the command alone is a silent no-op — then sends the step's command. `rests[]` in the report lists the fires actually rested at. Before scripting a death, the stage asserts the encounter's governing checkpoint is ARMED, and distinguishes three states. **Armed** → proceed. **Unarmed** (it sits on a bonfire nobody has rested at) → the run REDS with a precondition finding naming that bonfire and takes NO death; a death there would measure the delve against world spawn (bell round 3), which is the harness's own gap. **No governing checkpoint at all** (the plan names none fired before the fight — the truthful reading whenever the only nearby checkpoint is armed by the encounter's own kill step) → the death is skipped and the stage records the ADVISORY `no governing checkpoint — die-retry cannot prove safe death here`: every death there is a full restart of the delve, which is a content fact about where the campaign puts its rest points, and `DW0379`/`DW0315`/`DW0316` own that judgement rather than the bot. Both gaps also exclude the encounter from the coverage check — the precondition already says why the loop is unproven. The presence check counts BY TAG: it calls the compiler's `wave_census_<wave>` — named per encounter in `combat-plan.json`'s `census` block, never re-derived here — and reads the answer off the anchored marker channel (`[dw:census …]` totals, one `[dw:censusmob …]` per mob with position and health). It still SETTLES (up to 6s) rather than sampling the instant the walk back ends, because a re-seat takes ticks to land; `reengage.settle_ms` / `nearest_blocks` / `farthest_blocks` record what it waited for and where it found them. Before this the probe counted SILHOUETTES — every entity the client tracked, no distance filter, anything taller than half a block — so the drowned bell's ambush husks 57 blocks away at another encounter counted as members of whichever wave was being measured, and a 2-mob wave read as 4 standing. A census that never answers is an ABORTED trial naming the broken probe, never a zero: a silent zero would read as `stranded` and blame the delve for the harness's own fault. A `respawns_on_rest` wave additionally owes RE-SEAT FIDELITY: it must come back at the declared count, as all-new entities, at full health. A survivor carried across a life (`carried_over > 0`), a short count, or a mob below full health reds the run — a retry must never let the party chip a wave down one swing per death. `carried_over` is decided by IDENTITY: the ladder calls `wave_brand_<wave>` before each scripted death, stamping the wave's living mobs with a tag no re-summon can carry, and the next census counts how many still wear it. Health and its maximum come from the server's own `Health` and `max_health` inside that census, so `damaged` no longer depends on a max-health attribute vanilla never puts on the wire. The kill loop's own "this fight is over" tests are guesses made from shapes — a mob the bot hit winked out near the anchor; everything it engaged is down and nothing hostile is close — so none of them may END the step without the census agreeing. On the drowned bell the bot killed one of `ambush/the-rafters`' husks at the belfry, counted it as the Bellkeeper (`confirmed kill: husk#232 (1/1)`) and walked away from a live wither skeleton; `obj/the-keeper` never completed, so `quest/ring-it-home` was never armed, so the next step's `interact` click was adjudicated against an unarmed quest and spent. The guesses still DRIVE the fight (the bot can only swing at what it can see); the census is what ends it. The `die-retry` stage passes only when every planned encounter has its ≥2 COMPLETED trials — an encounter it engaged and proved nothing at, or never reached, is a red stage, never a silent pass. **Assist windows** (spec-0023 §3): the die-retry stage takes them too. It is assisted into melee range for the approach, for the mid-fight trade, and for the walk back plus the re-engage probe — every segment where the bot must SURVIVE to make a measurement — and takes the scripted death itself with the assist CLEARED, so `/damage @s 1000` is lethal without any argument about resistance arithmetic. Each segment is its own opened/closed/named window, so expect several per encounter and read `reason` to tell them apart. Before this, the stage walked to within 3 blocks of a live encounter bare: on the-drowned-bell run six the wave killed the bot before it could script death 1, the stage reported 0/2 trials beside `assist_windows: []`, and bot fencing skill was silently gating the one proof the stage exists to make. Fencing is telemetry, never the gate. **Trial field semantics** (every one of these is a MEASUREMENT, and the fields may never contradict each other): `respawn_pos` is the bot's own position read the instant the respawn settles, and `at_checkpoint` is derived from it — nothing between the respawn and that reading is allowed to move the bot, which is why the post-death re-arm only re-equips the kept kit and never replays `select-class` (`class_apply_<c>` ends in `teleport @s <campaign entry point>`, so replaying it warped the bot back to the start of the delve and made every `respawn_pos` a lie one second later). `kit_kept` says the kit survived the death — the delve seals `gamerule keep_inventory true`, so an empty bag reds the trial. `returned` is the walk from that measured respawn back to the encounter. `re_engaged` / `reengage` / `outcome` are observations taken AT the encounter and are recorded **only when `returned`**: a trial that never got back reports `re_engaged: false`, `reengage: null` and `outcome: unproven`, because "did not look" and "looked and found nothing" are different facts and neither is a pass. `completed` says only that the loop ran to its verdict; an abandoned trial is still in the array and still reds. The bot is opped for exactly three harness commands (`/damage @s`, `/effect give @s minecraft:resistance`, and `/function <ns>:wave_{census,brand,unbrand}_<wave>` — the compiler-owned census probe, whose ids come from the plan). `DELVEWRIGHT_DIE_RETRY=0` skips the stage for local iteration and the report records that it was SKIPPED, never that it passed. The report also carries the compiler's **floor-gate ledger verbatim** (`floor_gate.covered` / `floor_gate.not_covered`, each uncovered entry with the compiler's own reason) and one `actors[]` row per tier-declaring stage-5 actor — fought (with `outcome`, `swings`, `after_objective`) or not (with the reason). `floor_gate.present: false` means the build shipped no ledger at all (a `delvec` predating the ledger) and is deliberately distinct from an empty one: "this campaign bills nothing hard" and "this build cannot tell you" are different facts. When present, `floor_gate` also carries its own **binding count** (playtest-methodology.md rule 1): `examined` (`covered.len() + not_covered.len()`), `unbound` (`examined == 0`) and, exactly when unbound, `reason` — printed to stderr too (`combat plan: floor gate is UNBOUND …`), so a reader is never left to notice an empty `covered`/`not_covered` pair on their own to learn the gate matched nothing. A sibling top-level `actors_gate` states the same shape for `actors[]` itself — a DIFFERENT question (an `ordinary`-tiered actor binds `actors_gate` without binding `floor_gate` at all) — and is likewise `null` on a plan from a `delvec` too old to carry it. Every NAMED entity death this run observed lands in `named_entity_deaths[]`, classified `scripted_teardown` (a `despawn-actor style: vanish` relocates the body far below the floor before killing it, so the server broadcasts the same "<name> died" line a real loss would — see `harness/src/teardown.ts`) or `combat` — reclassified by depth, never suppressed, so a reader can tell the two apart without re-deriving it from raw coordinates (2026-08-06 island triage: five such deaths, two of them vanishes, were indistinguishable before this). A **trigger-driven step that times out** (`talk-to`, `interact`) now names which side swallowed it: the bot is opped, so vanilla's own answer to the `/trigger` it sent arrives on the chat stream, and the failure line repeats it — *the server ANSWERED …* means the trigger reached the delve and a datapack guard consumed it without completing anything (a re-used world whose scoreboard already carries the objective is the classic cause — `fresh-volumes.sh --project <id>`, then re-run, before suspecting the content: it cost three misattributed red runs in island round 13 and another round here), while *the server never answered …* means the command never got there and the fault is the harness's. Diagnostics only: the step still fails on its objective marker either way. Authoring note: an actor anchored inside a LATER objective's completion zone will complete that objective during the fight, which the endgame-discipline check then reds — stage the fight where the party already stands |
| `packtest` | agent | `EULA=TRUE validation/packtest-run.sh --project dw-<id> [--output <tree>]` (the entry script; `--project` REQUIRED) | headless PackTest suite on the tool server. `--output` (default `./delve-output`) boots a **different** build tree — the generated suite is per-campaign, so a template class is only proven live by a campaign that emits it (CI runs extra passes for template classes hello-world cannot emit: `crates/compiler/tests/fixtures/cast-ledger` for spec-0020's root-swap/bark/explicit-none templates, and `crates/dsl/fixtures/valid/keep-trial` for the `interact` verb templates — `verb_interact` and `verb_interact_held`, the held-vs-carried proof — since hello-world has no `interact` objective at all; `crates/compiler/tests/fixtures/souls-bonfire` for the spec-0016 §1 rest loop — `souls_bonfire_rest`/`_reseat`/`_options`, `souls_reseat_stationed` and `wave_census`; `crates/compiler/tests/fixtures/souls-td-lanes` for the §6 lane family — `souls_td_patrol_nbt`/`_lane_march`/`_lane_release`/`_lane_reseat`/`_aggro_edge`; and `crates/compiler/tests/fixtures/souls-timed-gate-disarm` for the timed-gate `disarm` rung — `souls_timed_gate`/`_disarm`/`_crush`, the claim being that no scheduled close ever re-seals a jammed span, which only a live server across cycle boundaries can prove). See `validation/README.md` "Running a second campaign through `packtest`" |

Shell entry points:

| Script | Class | Purpose |
|---|---|---|
| `validation/mutex.sh` | agent (only for host 25565) | guards exactly ONE resource: the owner's client port **25565**. `source validation/mutex.sh`, then `dw_mutex_acquire <name> [wait-s]` / `trap dw_mutex_release EXIT` / `dw_mutex_assert_not_owner_session`. A worker ladder does **not** take it — `compose.yaml` pins no container name and publishes no port, so ladders are isolated by their compose project and there is nothing to serialize (waiting on this lock to run a ladder means the ladder is wrong). The two things that DO take it are the sanctioned 25565 bindings: `validation/owner-play.yaml` sessions and `tools/playtest-server.sh` (which acquires as `owner-play-session` on `up` and releases on `down`). `dw_mutex_release` only works in the shell that acquired (agent tool calls never share shells) — cross-shell coordinators release with `dw_mutex_release_named <holder>`, which matches the HOLDER name exactly and refuses to free `owner-play-session` while ANY container still publishes 25565 (port, not container name — the two binders use different names). Acquisition is `mkdir`'s return value, never inferred from the lock directory existing; **`owner-play-session` is sacred** — never wait on it, never steal it. It shrank because the old stack-wide lock made worker ladders queue on each other: an island worker once waited 30+ min behind a holder with zero containers running. See [`../../validation/README.md`](../../validation/README.md) "Sharing the Docker host" |
| `validation/bot-run.sh` | agent (**the bot ladder entry**) | `EULA=TRUE validation/bot-run.sh --project dw-<id> [--output <tree>] [--run-out <dir>]` — the `validate` profile end to end: fresh-volumes the project, boot server + mineflayer bot, propagate the bot's exit code, tear the project down and prove it clean. `--project` is REQUIRED: the compose project is the only name the stack has now, so a missing id would land in compose's default project — a shared name by another route, and the collision that made ladders queue. Every `DELVEWRIGHT_*` run variable below is forwarded, so all of them can be set on this command line. `--output` selects the build tree the bot ladder boots. The delve image tag follows the project (`delvewright/delve:<project>`) for the same reason the compose project does — an image tag is global to the daemon, so two ladders building different trees into one tag race and the loser boots the other's delve. The run report lands in `validation/run-out/<project>/run-report.json` (project-scoped so two ladders from one checkout cannot overwrite each other's) |
| `validation/staging-admission.sh` | CI + agent (runs inside the compose `staging-admission` service) | `validation/staging-admission.sh <build-tree>` — refuses a build tree that carries no valid staging-gate admission token, or one minted for a DIFFERENT tree (it recomputes the `manifest.json` sha256). Announces an overridden admission loudly at boot. Dependency-free bash+coreutils on purpose: it runs inside the delve image, which must never gain tooling (ADR-0003). Both 25565-publishing services `depends_on` it with `service_completed_successfully`, so compose will not start them when it exits non-zero — verified live: the server container stays `State=created`, never starts, and binds no port. |
| `validation/packtest-run.sh` | agent (**the PackTest ladder entry**) | `EULA=TRUE validation/packtest-run.sh --project dw-<id> [--output <tree>]` — the `packtest` profile end to end, same contract as `bot-run.sh`: `--project` REQUIRED, own teardown, exit code = failed tests. `--output` selects the build tree (default `./delve-output`), which is how CI proves per-campaign template classes hello-world cannot emit. There is no `PACKTEST_CONTAINER` any more — the runner pins no container name. It also calls `server-bootstrap-cache.sh` (idempotent) and **copies the bootstrap overlay into this project's world volume before booting**, so the suite performs no live Mojang/Fabric fetch — measured: the whole suite runs green under `--network none`. It then asserts that binding on the boot log (the locally-provisioned-launcher line present, no download lines) and reds if the seed missed, because a seed that silently missed would leave the ladder exactly as fragile while reporting success |
| `validation/owner-play.yaml` | human | `docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile play\|playtest up` — the ONLY compose file that publishes host 25565 and pins `delvewright-server` / `delvewright-playtest`. Nothing else in `validation/` may (`tools/check-compose-isolation.py`) |
| `validation/ephemeral-port.yaml` | agent | an EPHEMERAL loopback publish for the flows that drive a bot from the HOST (`playtest-note-flow.sh`, `rehearsal-flow.sh`). Docker picks the number; read it back with `docker compose -p <id> … port <service> 25565`, never assume it |
| `validation/warden-probe.sh` | agent (spike) | `[POLL_SECONDS=n] [WATCH_SECONDS=n] [CONTAINER=name] validation/warden-probe.sh` — measures what a summoned 1.21.11 warden actually does (dig-down timing, `dig_cooldown`/`anger` NBT, difficulty effects) against a **throwaway** pinned server, never the shared stack. Refuses to run while the mutex reads `owner-play-session` |
| `validation/fresh-volumes.sh` | agent | `validation/fresh-volumes.sh --project <compose-project>` — tear ONE compose project down and **prove** its containers and volumes are gone. `--project` is REQUIRED: no default, and `COMPOSE_PROJECT_NAME` is deliberately not honoured, because an invisible default's cost is somebody else's live world. The old daemon-wide `--all` is GONE — it matched `server-data$` across every project and force-removed the pinned `delvewright-*` names, i.e. an outage rather than a teardown. It additionally refuses a project whose container publishes host 25565 (an owner-facing session, human possibly inside). Run it before any re-run of the bot ladder — the entry scripts do it for you: `docker compose -p <proj> … down -v` silently leaves `<proj>_server-data` behind whenever an exited container of that project still holds it, and the stale volume carries the scoreboard, so the re-run starts with objectives already complete and the bot reports a **false CONTENT failure** (three misattributed red runs, island round 13) |
| `validation/render-shots.sh <build-dir> [out-dir]` | agent | turn a build output into the Chunky scene set + shot index (`delve-render scene` + `panorama` + `index`), including the first-person POV shots and the whole-map release panorama (`<campaign>_panorama_se`) |
| `validation/playtest-note-flow.sh` | CI (tier 3) | `EULA=TRUE validation/playtest-note-flow.sh` — drives the whole spec-0006 note loop non-interactively and asserts the report. Runs in a per-invocation compose project (`dw-noteflow-$$`, override with `DW_COMPOSE_PROJECT`) on an ephemeral host port, so it needs no lock |
| `validation/rehearsal-flow.sh` | CI (tier 3) | `EULA=TRUE validation/rehearsal-flow.sh` — drives the whole spec-0019 calibration loop (`dw.aim`/`dw.faster`/`dw.mark`/`dw.done` → harvest → `delvec calibrate`) and asserts the patch resolves back to the cell the bot marked. Per-invocation compose project (`dw-rehearsal-$$`) on an ephemeral host port, like note-flow |
| `validation/branch-runs.sh` | agent (**required for a branching campaign**) + CI (tier 3) | `EULA=TRUE [DELVEWRIGHT_BRANCHES=…] validation/branch-runs.sh --project dw-<id> [--out <dir>]` (`--project`, or `DW_COMPOSE_PROJECT`, is REQUIRED) — spec-0025 §3 branch runs: walk every branch the tier selects, **each in its own fresh world** (party progress only moves forward, so a second branch needs a second world), and merge the per-branch run reports into `validation/run-out/<project>/branch-runs.json` — per branch: ran/skipped-with-reason/**INFRA-FAILED** and the result (an attempted branch whose compose run exited without writing any run report renders as an infra failure — a validation-infrastructure fault, distinct from a red run and from a tier skip). `--out` / `DW_RUN_OUT` relocates the merged + per-branch reports; the bot's own report is read from the compose mount, which is now project-scoped too (`DW_BOT_OUT`, so two loops from one checkout cannot overwrite each other's reports) and FILED under the out dir. The branch set and the selection come from the build's `validation/branch-plan.json` via `harness/src/branch-select.ts`, i.e. the same code the run uses, so a tier can never select a branch the run then refuses. Isolation is by construction: own compose project, no pinned container name, no host port, teardown via `fresh-volumes.sh --project`. One critical-path run proves ONE storyline; this is what makes "provably completable" quantify over branches |
| `validation/server-bootstrap-cache.sh` | CI (tier 2) + agent | `validation/server-bootstrap-cache.sh [--cache <dir>] [--force]` — performs **the one live Mojang fetch of a job** and leaves a `/data` overlay every server boot copies from. The jar is never baked into an image (ADR-0010, EULA), so each server bootstraps it at first boot, and `tier 2` boots SEVEN of them over seven fresh volumes (1 datapack-load + 6 PackTest suites) — seven independent chances for one Mojang blip to red a required check. This fetches `versions.toml`'s `server_jar_url` once, **refuses anything whose sha256 is not `server_jar_sha256`**, and materialises the Fabric bootstrap (launch jar + its manifest `Class-Path`) beside it in a throwaway toolserver container. Idempotent — a warm cache fetches nothing — so `packtest-run.sh` calls it unconditionally and every extra caller in the job is free. Retries are bounded and scoped to the bootstrap fetch alone; exhausted, it exits non-zero with an error naming the host, **before** any server boots, so a network outage can never read as a datapack failure. It also asserts the pinned toolserver's baked Fabric launcher still matches `[fabric].launcher_version`. Cache dir `validation/server-cache/` (gitignored, never baked into a layer) |
| `validation/check-versions.sh` | CI (tier 1) | fails if any Dockerfile/compose/workflow disagrees with `versions.toml` |
| `validation/check-world-settings.sh` | CI (tier 1) | fails if a server profile hardcodes world settings instead of deriving them from the build |
| `validation/world-settings-entrypoint.sh` | — | the shared entrypoint the above guards; not invoked by hand |

## 8. Harness (`harness/`) · CI

The mineflayer bot the `validate` profile runs, plus the spec-0006 note bot and
the spec-0019 shot-calibration bot. It contains zero campaign logic — it reads `critical-path.json` and asserts.

```
npm --prefix harness run typecheck      # tsc --noEmit
npm --prefix harness test               # node --test 'test/**/*.test.ts'
npm --prefix harness start              # node src/run.ts <critical-path.json>  (compose does this)
```

`harness/src/note-bot.ts` is driven by `validation/playtest-note-flow.sh` and
`harness/src/rehearsal-bot.ts` by `validation/rehearsal-flow.sh`, never by hand.

**Crosshair acquisition (`harness/src/crosshair.ts`).** Every interaction step —
`talk-to`, `interact`, `rest` — now proves the click was *available to a player*
before it acts. It casts the entity-pick ray vanilla casts (eye → box, nearest
hit wins, reach 3.0, pick radius 0) at every aim point on the target's hitbox,
from every standable cell the step's walk goal allows, and fails the step naming
**both** bodies if the target is unreachable from all of them. The trigger it
guards is still a chat command — a 1.21.11 dialog button is client-drawn and
mineflayer has no client, so actuation cannot change — but the bot may no longer
*aim* by entity id. This closes the divergence that let the owner's island
soft-lock past a green ladder: two NPCs on one cell are indistinguishable to a
crosshair and were invisible to a bot that targeted by id. Targeting by id
survives only in the combat paths, where no crosshair is modelled at all. The
compiler half of the same defect is `DW0489` (`compiler::crosshair`), which
proves the staging from the cast ledger at build time; the two are complementary,
not redundant — the compiler sees every scene the player can click in, the bot
sees only what the scripted path clicks, and only the bot sees actors and wave
mobs.

Run-shaping environment (all read by `src/run.ts`; the compose `validate` profile
forwards **every one of them**, so each can be set on a `docker compose` /
`bot-run.sh` command line).

A variable no `validation/*.yaml` declares arrives unset inside the container, and
setting it on a command line is SILENTLY DROPPED. `compose.yaml` therefore names
every one of them in the `bot` service's pass-through form (`VAR:` with
no value), which forwards the host's value and omits the key entirely when the host
has not set one — so an unset variable stays unset rather than becoming an empty
string. Re-measure the invariant with `comm -23` of the `DELVEWRIGHT_*` names in
`harness/src/` against those in `validation/*.yaml`; the only name that may legally
remain is `DELVEWRIGHT_NOTE_TEXT`, because `validation/playtest-note-flow.sh` runs
`node harness/src/note-bot.ts` on the HOST, never in a container:

| Variable | Effect |
|---|---|
| `DELVEWRIGHT_RUN_REPORT` | Path to write the spec-0023 run report to (compose sets it; the HOST side is `validation/run-out/<project>/`, scoped by `DW_BOT_OUT`). Unset = the pre-spec-0023 stderr-only run |
| `DELVEWRIGHT_DIE_RETRY` | `0` skips the die-retry stage (local iteration only). Default ON whenever a combat plan is present. **Read its binding count, not its verdict**: the run report's `die_retry_binding` states `declared_encounters`, `engaged`, `deaths_scripted`, `trials_completed`, `skipped_no_checkpoint` and `skipped_unarmed_checkpoint`, and `unbound: true` means ZERO scripted deaths were taken whatever `passed` says. The stage's per-encounter arithmetic runs over an already-emptied list when every encounter is excluded for want of a governing checkpoint, so it reports a pass having proved nothing — and measured 2026-08-11 that was the state of EVERY campaign and fixture in both repos (`keep-trial` / `hollow-vigil`: encounters with no checkpoint before them; `nobodys-cave-island`: zero mandatory encounters; the drowned bell: no stage documents). An unbound stage now says so on stderr and carries the reason in the artifact. `crates/compiler/tests/fixtures/die-retry` is the smallest campaign that lets it bind — one conversation that sets a checkpoint and summons one wave, one `kill` objective four blocks away — and `crates/compiler/tests/die_retry_fixture.rs` holds it to that shape |
| `DELVEWRIGHT_RETRY_ON_DEATH` | `1`/`true` lets the sequencer retry a step once after an unscripted death (spec-0008) |
| `DELVEWRIGHT_RUN_TIMEOUT_MS` | Hard wall-clock budget for the whole run (default 20 min, forwarded by the compose `bot` service). **Raise it when the die-retry stage is on**: two scripted deaths per encounter add a respawn, a re-arm and a walk back to every fight |
| `DELVEWRIGHT_BOT_USERNAME` | The bot's name; feeds the server's `DELVE_OPS_OFFLINE` seed (offline-UUID ops.json — never itzg `OPS`, which would resolve the name via Mojang PlayerDB and abort on an offline-only name) or the assist and scripted-death commands are silently refused |
| `DELVEWRIGHT_ACTOR_FLOOR` | `0` skips the **actor floor gate** (local iteration only); the report then records each tiered actor as skipped with that reason, never as measured. Default ON whenever the build's combat plan declares a tiered actor (`actors[]`, DSL v0.8). ON, the run gives every `elite`/`boss` actor whose `unleash-actor` beat hangs off an objective the path completes ONE honest unassisted attempt, right after that objective's marker arrives, and reports the outcome (`won-first-try` → the inverted floor-gate advisory; `lost` / `timed-out` / `body-not-found` say nothing, and never read as a pass). It takes **no assist**: nothing downstream waits on an actor fight, so there is no obligation to win one. An actor unleashed only by an ambient trigger (`strike`/`use`/`approach`/`strike-npc`) or by a quest completion is reported as NOT exercised with the reason — the campaign does not schedule those, so the bot may not invent a moment for them |
| `DELVEWRIGHT_DEATH_LOOP` | `0` skips the **death-loop stage** (local iteration only); the report then records it as SKIPPED with that reason, never as passed. Default ON whenever the build ships `validation/death-plan.json` — i.e. whenever the campaign declares a lethal volume, an `on_death` or a recovery stake. ON, the bot walks into every declared lethal volume, DIES there, and asserts what the campaign promised: the volume's own wording reaching that player, the declared forfeit leaving the currency ledger, the recovery stake standing at the anchor the compile-time placement table chose, the walk back from the respawn seat, an exact restore under a double right-click in one tick, and the retirement of the collected hardware. This is the ONLY tier that can witness a player death at all — a PackTest fake player is permanently undamageable (measured 2026-08-03 and 2026-08-09). A plan whose `binding.unbound` is true (a volume with no `on_death`, or an `on_death` with no volume) is reported as a finding and NOT walked |
| `DELVEWRIGHT_BRANCHES` | **Which branches this run is answerable for** (spec-0025 §3). `all` (default, the release tier: every enumerated branch), a comma-separated list of branch ids (the narrowed tier), or `from-diff` — the PR tier spec-0025 describes, which **refuses**: the diff→branches mapping is compiler-side and is not emitted yet, and degrading to `all` would lie about cost while degrading to nothing would lie about coverage. A branch this tier excludes appears in the run report with the reason it did not run; a skipped branch is NAMED, never silent. A list naming a branch the build does not declare is an error, not a silent skip. Ignored for a build with no `validation/branch-plan.json` |
| `DELVEWRIGHT_BRANCH` | **Which single branch THIS session walks.** The run then reads `validation/branch-path-<branch>.json` (the ordinary critical-path contract, computed under that branch) instead of `critical-path.json` — navigated leg-by-leg through that branch's own `validation/branch-waypoints-<branch>.json` (absent → single-goal fallback reported LOUDLY in stderr + the run report, never silently), and asserts the path really takes the choices that ENTER the branch — so a run cannot report branch coverage while having walked somebody else's storyline. One branch per invocation by construction: party progress only ever moves forward, so a second branch needs a second WORLD (`validation/branch-runs.sh` is that loop). Unset = the ordinary single-path run, unchanged. Refused if the branch is not in the build, or not one `DELVEWRIGHT_BRANCHES` selected |

An `interact` step whose `critical-path.json` entry carries `requires_item` puts
that item in the bot's **mainhand** before it sends the trigger
(`src/held-item.ts`), because `requires_item` is held, not carried
([`compiler.md` §objectives](compiler.md)). Actuation only: the guard stays in the
datapack, and a bot that cannot hold the item still fails the step on its objective
marker — but the log now says which of the two happened instead of showing a bare
30s timeout.

## 9. Prefab generators (`prefabs/*-generator`, `prefabs/generator`) · agent + CI

The tileset libraries are **generated, not hand-built**. Separate Cargo
workspaces, deliberately outside `crates/` so none of them can enter the shipped
`delvec` and no existing `.nbt` moves (ADR-0006). They share one CLI —
`<out_dir>`, which is the content repo's `prefabs/` when you mean to re-export. It must
already exist: a generator that creates its own destination cannot tell a fresh
library from a typo, and a piece written where nothing reads is indistinguishable
from a piece written correctly.

```sh
cargo run --release --manifest-path prefabs/<gen>/Cargo.toml -- <out_dir>
```

| `<gen>` | binary | tileset | doc |
| ------- | ------ | ------- | --- |
| `generator` | `keep-prefab-gen` | `keep-*` (the original interior set) | `prefabs/keep-tileset.md` |
| `cave-generator` | `cave-prefab-gen` | `cave-*` | `prefabs/cave-tileset.md` |
| `island-generator` | `island-prefab-gen` | `island-*` set-pieces | `prefabs/island-tileset.md` |
| `island-terrain-generator` | `island-terrain-gen` | `island-*` terrain | `prefabs/island-tileset.md` |
| `tidal-keep-generator` | `tidal-keep-gen` | `tk-*` (souls set) | `prefabs/tidal-keep-tileset.md` |
| `hello-room-generator` | `hello-room-gen` | `hello-room` (the M1 piece) | — |

Each tileset generator prints the `pool/*` block to merge into the content repo's
`pools.json` — printed, never written, because every `*.json` in that directory
is parsed as prefab metadata and a stray snippet is `DW0346`.

**The invariants are the point.** Every debugging lesson these tilesets have cost
is pinned as an `assert!` in the generator (route walkability, stair-flank
sealing, anchor sanity, sightlines, gravity substrate, redstone support), so
*running* a generator is the gate: it either emits or panics.

Invariants true of **every** tileset live once, in
[`../../prefabs/invariants.rs`](../../prefabs/invariants.rs), source-included by
every one of them (`#[path = "../../invariants.rs"] mod invariants;` — an include, not a
dependency, so the workspaces stay independent). Today: **distress embeds, it
never stacks** (`assert_distress_never_stacks`) — a walkable stair tread may
carry nothing but air or a declared attachment (railing, hardware, light fitting,
plant), because wear on a walked surface belongs *in* the surface, as a weathered
variant of the same shape (`invariants::weathered`), never as a lump on top of
it. Owner playtest, island round 13: stray stone sitting on the cave-mouth steps.
The shared file carries its own unit tests — including the cases that prove the
gate *fails* — run by the same CI job.

**Connections are derived, never defaulted.**
[`../../prefabs/connections.rs`](../../prefabs/connections.rs) is source-included
the same way and runs at the same emitters. Before the bytes are written it
fills every shape-carrying property a state *leaves out* — a fence's, a wall's,
a pane's or a bar's connections, a vine's or a lichen's absent faces — from the
blocks actually beside the cell, by the rule vanilla applies itself
(`FenceBlock.connectsTo`, `IronBarsBlock.attachsTo`, `WallBlock.connectsTo` /
`shouldRaisePost`, `MultifaceBlock.canAttachTo`). A value the generator wrote is
never overwritten, so a fully-specified state emits unchanged and an author who
means a lone post says so. Filling with the block's *defaults* would be the
opposite of this and worse than silence: the default of every connection
property is disconnected, so it would assert the isolated post rather than
merely fail to deny it.

Two emitter post-conditions come with it. `assert_shape_is_stated` is the
`DW0735` verdict where the bytes are made rather than at admission, which binds
to one moment per piece. `assert_attachments_are_supported` refuses a vine or a
lichen face with nothing behind it — vanilla deletes such a face at the first
block update, so it is in the template and not in the game.

**Where an attachable block may hold on is the same module's question.**
`attachable_faces(block, cell, at)` answers it: every face of *that block* with
a supporting neighbour, best first — a wall before the ceiling, the ceiling
before the floor. A decoration pass asks it and takes the first answer, and an
empty answer means the cell can hold no decal and none is placed. The faces come
from the pinned shape table, so a vine is asked about its five and a lichen about
its six, and each face is paired with the direction it looks in by the module
rather than by the caller. Both halves are load-bearing: a pass that pairs its
own offsets can name the face pointing *away* from the rock, and a pass that
lists its own faces lists the four horizontals — so a decal whose only rock is
overhead has nowhere to hang and is dropped instead of hung from it. This is a
query for a placer, not a repair: a multiface face is a placement decision, so
re-hanging an already-emitted state would turn the post-condition above into a
silent rewrite.

The one input vanilla publishes nowhere is `isFaceSturdy`: it is code, not data,
so `face_support` decides from the pinned tables plus a **declared** list of full
cubes and **refuses** — naming the block and the piece — outside it. There is no
conservative direction to guess in; connecting where vanilla would not and
failing to connect where it would are equally visible.

Debug flags, all
`tidal-keep-generator`: `TK_DEBUG_LIGHT=1` (per-region measured light + darkest
cell), `TK_PROBE=<salt>,<x>,<y>,<z>` (labelled block dump), `TK_DEBUG_STAIRS=1`
(every flank the seal pass closed).

CI (`prefab-generators` job, tier 1) runs every generator twice into separate trees
on every PR: a panic fails the job, and the two trees must be byte-identical
(ADR-0006). `tools/check-structure-emitters.py` holds that job's lists equal to the
`prefabs/*/Cargo.toml` workspaces on disk, so a generator it does not run is a red.
Wired 2026-08-03 — before that nothing in CI compiled these
workspaces, which is how a tileset with 132 reversed stair blocks (`DW0430`)
reached an owner playtest through a green pipeline. `clippy -D warnings` is not
yet part of that job (`prefabs/generator` carries two legacy style lints).

**Re-export loop**: edit the generator → run it into `campaigns/prefabs/` → the
`.nbt`/`.json` diff is content-repo work, the source diff is engine work, and the
two land as a pair.

## 10. Spikes (not the pipeline)

`tools/spike-jump-arc/run.sh` (`EULA=TRUE tools/spike-jump-arc/run.sh`) measures
1.21.11 jump kinematics on a throwaway server to feed
`docs/notes/jump-arc-model.md`. The compiler consumes the resulting **model**,
never this rig. Do not wire spikes into a skill.

`tools/spike-death-teleport/run.sh` (`EULA=TRUE tools/spike-death-teleport/run.sh
[--out <path>]`) measures, on the same throwaway pinned server, (a) which
pre-respawn death signals exist per death cause — `deathCount`, the
`entity_killed_player` / `entity_hurt_player` advancement triggers, the corpse's
`Pos`, and `LastDeathLocation` — and (b) how accumulated fall distance settles
when a falling player is teleported. Findings:
[`../notes/death-and-teleport-spike.md`](../notes/death-and-teleport-spike.md);
raw per-sample observations are committed next to the rig
(`tools/spike-death-teleport/observations.json`), as is the 1.21.11 gamerule
identifier list it extracts from the pinned jar
(`gamerules-1.21.11.txt`). It publishes an **ephemeral** loopback port and never
takes the 25565 mutex, so it runs alongside any ladder. Two design notes carried
by this rig and worth copying into the next one: every rcon response is checked
(`fill` into an unloaded chunk and a legacy camelCase `gamerule` both answer
politely and change nothing), and every sample batch is fenced by a `#sync`
scoreboard round-trip so a desynchronised read aborts instead of shifting every
later value by one.

`tools/spike-fluid-plane/run.sh` (`EULA=TRUE tools/spike-fluid-plane/run.sh
[--out <path>]`) measures, on the same throwaway pinned server booted with the
delve ocean-superflat generator literal, the fluid physics spec-0038 rests on:
plane-fill stillness and MSPT cost at 512×512 scale, edge behaviour at ticking
vs never-ticking chunk rims, the ambient sea's silent re-flood of a cleared
layer (fully healed ≤40 s with zero flowing block-states), saturated placement
vs flow-and-settle vs an interior gap in a closed basin, waterloggable blocks
under a rising and a falling level, the `/fill` block ceiling and
`max_block_modifications`, and what a rising water column or a solid runtime
fill does to the player standing in it. Raw data:
`tools/spike-fluid-plane/observations.json`; the findings live in spec-0038 §4
directly. Two hardenings this rig added over the death spike's channel, worth
keeping: a dead rcon channel rejects the pending read instead of leaving node
an empty event loop to exit 0 on, and channel start is a retried handshake
(the readiness probe passing and the next connection being accepted are two
events with a measured race between them).

`tools/spike-block-settling/run.sh` (`EULA=TRUE
tools/spike-block-settling/run.sh [--out <path>]`) measures, on a throwaway
pinned server booted on a DRY superflat, the two facts the `stair-shape`
(`DW0801`) and `fluid-contained` (`DW0800`) gates encode — so that neither rests
on a recalled reading of vanilla.

A field of 758 random stairs (two stair blocks, both halves, all four facings,
air holes) is placed, settled and read back cell by cell; the result rides in
`observations.json` and `crates/schem/tests/stair_shape_measured.rs` **replays
every cell of it** against `delvewright_schem::stairs::derive_shape` in CI, with
no server. Nine water rigs decide what "a body of fluid stays where it was
written" has to mean: a source sealed, a source with one open cell, a source
against a `waterlogged=false` stair (both orientations) and a grate, a source
each side of a dry waterloggable, a source above one, and a `waterlogged=true`
block beside open air both before and after a block update is forced next to it.

Two things this rig is worth keeping for. Its **settling pass** is the subtle
part: `/setblock` writes a state literally and `StairBlock` only re-derives its
shape on a HORIZONTAL neighbour update, so the obvious rig (set each stair to
air and back) settles a cell's neighbours and RESETS the cell — the first run
left 10 of 758 cells carrying their authored value, a number small enough to
read as "the implementation is wrong about ten corner cases" rather than "the
rig lied". The poke therefore never touches a stair: a temporary stone goes into
a NON-stair cell beside it and comes out again. And the rig carries its own
falsifier — the field is read twice with a further round of updates in between
and must not move, because a settled field is a fixpoint and an unsettled one is
exactly what moves.

`tools/spike-area-effect-arrow/run.sh` (`EULA=TRUE
tools/spike-area-effect-arrow/run.sh [--out <path>]`) answers whether a datapack
alone can give a projectile a non-block-breaking explosion or a splash-potion
area effect on impact, and **where that behaviour has to live**. It dumps the
pinned jar's own registry report (`--reports`) to `registries-1.21.11.json` — so
"no advancement trigger exists for a landed projectile" is read off the shipped
build — then measures the `minecraft:hit_block` enchantment effect against block
census, damage-versus-distance with a vanilla-TNT calibration row, decorations,
and the multi-player / restart / mob-shot / water / slab / entity-hit axes.
Findings: [`../notes/area-effect-arrow-spike.md`](../notes/area-effect-arrow-spike.md);
raw observations beside the rig (`observations.json`). It publishes an
**ephemeral** loopback port and never takes the 25565 mutex. Three things it
carries that the next rig should copy: a **positive control on every negative
claim** (the block census only means something because the `block_interaction:
"tnt"` row destroys 257 blocks with the same instrument); `fill()`/`killq()`
separating "nothing to do" from "rejected", since `No blocks were filled` and
`Could not set the block` are legitimate answers that sit inside the shared
rejection regex; and a `--phase 1`/restart/`--phase 2` split so "survives a
reload" is measured rather than assumed.
