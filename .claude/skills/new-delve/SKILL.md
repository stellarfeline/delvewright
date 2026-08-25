---
name: new-delve
description: Generate a complete playable Minecraft delve from a creative prompt — staged DSL authoring with validation-loop self-repair, deterministic compile, machine validation, joinable output. Use when the user asks to create/generate a new delve or campaign. Args = the creative prompt (theme one-liner or detailed brief).
version: 1.6.0
requires:
  delvec: ">=1.0.0 <2.0.0"
verified_with: 1.1.0
---

# /new-delve — building a delve, end to end

You author a delve as a set of **JSON documents**. `delvec` turns them into a
datapack and a world. You never write mcfunction, dialog files or datapack JSON,
and you never hand-edit an `.nbt` — everything the player meets is compiled from
the documents you wrote.

Read `CLAUDE.md` first if you have not; its forbidden zones apply in full.

## The shape of the run

Everything below happens in this order, and the order is not a suggestion — each
step needs something the step before it produced.

```
Init            build the toolchain, once per machine        ── §Init
  ↓
Decide          areas[] or a site plan — one campaign, one    ── §Which placement model
  ↓
 1  workspace   the campaign directory and its documents
 2  placement   world.json areas[]   OR   brief → graph → plan
 3  story       npcs · classes · quest-plan
 4  GATE        the design walkthrough — someone says yes
 5  content     quests · dialogue
 6  fmt         delvec fmt              ← every campaign, not optional
 7  analyze     delvec analyze
 8  build       delvec build
 9  walk it     get into the world yourself, first thing
10  ladder      PackTest · bot · branch runs
11  chronicle   only when the plan declares branch_points
12  visual      the POV sequence, then the renders
13  detail      site-plan campaigns only, and only after the walk
14  hand over   storybook, staging gate, play commands
```

Steps 1–14 are printed below in that order, one after another, with nothing
between them. Everything else on this page is **reference**: what the DSL can
express, how to write the prose, what to do when the piece library has no piece
you need, what to do when something goes red. Reference sections come after the
steps, and a step names the one it needs.

Two branches change what you do, and both are decided before step 1:

- **areas[] or a site plan.** A campaign has exactly one placement authority.
  See *Which placement model* below — it is the one decision that cannot be
  changed later without redoing step 2.
- **Do you already have approved reference art?** A campaign being re-made from
  an approved design carries its images in `campaigns/<id>/design/`. If that
  directory exists, you need no image provider and Init step 5 is a read rather
  than a setup. See Init step 5.

## Init — build the toolchain before you author anything

Run all of this before writing a line of a campaign document. The floor is
deliberately low and always available: **clone the repository and build from
source.** A downloadable binary is an optimisation of that, never the guarantee,
so a tool that cannot be downloaded is built at the step that needs it rather
than skipped.

**If any step here cannot be completed, say so and stop.** Authoring against a
half-built toolchain produces a campaign whose visual half was never reviewed,
and nothing downstream reports that.

### 0. What has to be on the machine already

| | why | check |
|---|---|---|
| `git` | both repositories | `git --version` |
| Rust (stable) | every binary is built from source | `cargo --version` |
| Python 3.10+ | the checkers, the reference-image tool, the staging gate | `python3 --version` |
| Java 17+ | Chunky, the renderer for review and storybook frames | `java -version` |
| Docker | the machine ladder and the play server | `docker info` |
| A 1.21.11 Minecraft client jar | textures — never downloaded or redistributed by this toolchain | see step 3 |

### 1. Two repositories, and the link between them

Campaigns do not live in the engine repository. They live in
`delvewright-campaigns`, and the engine reaches them through a **symlink named
`campaigns/` at the engine repo root**. The symlink is the only mechanism —
there is no environment variable that stands in for it, and a fresh clone does
not have one.

```sh
git clone git@github.com:stellarfeline/delvewright.git
git clone git@github.com:stellarfeline/delvewright-campaigns.git
cd delvewright
ln -s ../delvewright-campaigns campaigns
```

Confirm: `ls campaigns/prefabs/pools.json` must print the path, not an error.
Without it, prefab lookups and two of the compiler's own tests fail in a way
that reads as a broken compiler.

### 2. One build, six binaries, one `PATH` line

There are **six** binaries and the page uses all of them. Five come from the
engine workspace; `delve-render` is its own cargo workspace and lands in a
**different** target directory. Both builds, then one `PATH` line that makes
every command on this page work as written:

```sh
cargo build --release --workspace
cargo build --release --manifest-path crates/render/Cargo.toml
export PATH="$PWD/target/release:$PWD/crates/render/target/release:$PATH"
```

| binary | what it is | lands at |
|---|---|---|
| `delvec` | the compiler, and the CPU render arms (`viewer`, `palette`, `scene`, `panorama`, `contact-sheet`, `index`, `snapshot`, `blocking-chart`) | `target/release/` |
| `delve-grammar` | writes a new prefab from a rule program | `target/release/` |
| `delve-admit` | admits a prefab into the library | `target/release/` |
| `delve-schem` | converts an outside schematic | `target/release/` |
| `delve-harvest` | turns in-game playtest notes into a report | `target/release/` |
| `delve-render` | the GPU arms (`piece`, `batch`, `fidelity-gate`) | `crates/render/target/release/` |

`--manifest-path`, not `-p`, for the last one: it is excluded from the engine
workspace on purpose, so `-p delve-render` resolves to nothing. Its build
fetches a git dependency, so it needs the network once.

Add the `export PATH` line to your shell profile, or re-run it in every new
shell. Everything below is written as `delvec …`, and it will be
`command not found` until you have.

Confirm both:

```sh
delvec --version          # delvec <x.y.z>, dsl <a.b.c>, mc 1.21.11
delve-render fidelity-gate
```

`delvec --version` must print an engine version inside this skill's declared
`requires.delvec` range, and `fidelity-gate` must exit 0. Either failing is a
hard stop. **Write down the `dsl` number it prints** — step 1 needs it.

### 3. The 1.21.11 client jar

Every picture in this pipeline is drawn with Minecraft's own textures, and this
toolchain never downloads, bundles or redistributes them. Supply your own,
from your own Minecraft installation:

```sh
mkdir -p ~/.chunky/resources
cp "<your minecraft dir>/versions/1.21.11/1.21.11.jar" ~/.chunky/resources/minecraft.jar
```

On macOS the launcher's directory is `~/Library/Application Support/minecraft`;
on Linux `~/.minecraft`. Any of three paths works and they are tried in this
order: `--textures <jar>` on the command, `$DELVEWRIGHT_CLIENT_JAR`, then
`~/.chunky/resources/minecraft.jar`.

Confirm the whole ladder answers, rather than discovering it at the review step:

```sh
delvec palette campaigns/prefabs/hello-room.nbt -o /tmp/palette.json
```

### 4. Chunky

Chunky renders every frame that has to *look* like Minecraft: the player-POV
review shots, the storybook art, the whole-map panorama. It is a separate
program — `delvec` writes the scene, Chunky renders it — and it is not
installed by anything above. Two commands, once per machine:

```sh
curl -LO https://chunkyupdate.lemaik.de/ChunkyLauncher.jar
java -jar ChunkyLauncher.jar --update snapshot
```

The launcher self-installs the pinned core into `~/.chunky/lib`. A snapshot
core is required — the stable line does not read 1.21.x worlds. Keep
`ChunkyLauncher.jar` somewhere you can name later; step 12 and step 14 both run
it.

Confirm: `java -jar ChunkyLauncher.jar --version` prints a launcher version, and
`--update snapshot` ends in either an install or "No updates found".

### 5. Reference images — read the two paths before you spend anything

The design gate at step 4 is confirmed on **pictures of the design**, and there
are two ways to have them. Decide which you are on now, because one of them
needs a paid third-party account and the other needs nothing at all.

**Path A — the campaign already has an approved design.** Look first:

```sh
ls campaigns/campaigns/<campaign-id>/design/
```

A campaign being re-made carries `design/README.md` (the approval date and the
approved names), `design/concept/` (one image per scene) and, when the map was
designed as a whole, `design/reference/` (the map views, their prompts, their
style note and their sidecars). **If that directory exists, the reference
exists.** Read it, author from it, judge against it, and present every later
choice beside it. You need no image provider, and Init is finished at step 6.
Do not re-draw an approved image; the approval is attached to the file that is
there.

**Path B — there is no approved design yet, and you are drawing one.**
`tools/refimg.py` draws reference images. It is stdlib Python and needs nothing
built, but it calls a **paid third-party image API**, and three things must be
in place before step 4 — establish them here, not at the gate:

- a `[refimg]` section in `delvewright.local.toml` at the repository root — the
  file is gitignored; copy the commented convention block out of
  `delvewright.toml`;
- the API key **exported in your shell**, in the environment variable that
  section's `api_key_env` names. The key never enters a file, so it is never in
  the repository and never in the config — which also means nothing carries it
  over from your last session;
- a confirmation that costs no call:

```sh
python3 tools/refimg.py --prompt "smoke test" --dry-run
```

Absent configuration exits 2 and says exactly what to add. A malformed one is a
hard error.

On path B this is a **hard prerequisite of the whole run**, not of one step. For
a site-plan campaign the map's own reference is the first thing written and
everything below is written against it; for an `areas[]` campaign the same wall
stands at step 4. Reaching either without a provider stops the line where
stopping is most expensive.

### 6. Python, only when an NPC needs a face of its own

Skip unless the design calls for a custom skin. The skin toolchain is a Python
package with dependencies, and `python3 --version` answering is not the same as
the package being importable — a missing skin is a build error, not a silent
skip. Make it a venv here so step 5 does not stop on it:

```sh
python3 -m venv .venv-skin
.venv-skin/bin/pip install -r tools/skin/requirements.txt
PYTHONPATH=tools/skin .venv-skin/bin/python -m delve_skin --help
```

The last line answering is the confirmation. Only the dependencies are
installed; the package itself is reached on `PYTHONPATH`, which leaves no build
artifacts in the repository. Use `PYTHONPATH=tools/skin .venv-skin/bin/python -m
delve_skin …` wherever this page says `python -m delve_skin`.

### Init is finished when every one of these answers

```sh
ls campaigns/prefabs/pools.json          # the content symlink
delvec --version                         # the compiler, and the dsl number
delve-grammar list                       # the workspace's other binaries are on PATH
delve-render fidelity-gate               # the GPU arms, in their own target dir
delvec palette campaigns/prefabs/hello-room.nbt -o /tmp/palette.json   # the client jar
java -jar ChunkyLauncher.jar --version   # Chunky
docker info                              # the ladder and the play server
```

`delve-admit`, `delve-schem` and `delve-harvest` came out of the same
`--workspace` build and sit beside `delve-grammar`; `command -v delve-admit`
answering is the whole check for them.

Path B adds `python3 tools/refimg.py --prompt "smoke test" --dry-run`; a custom
skin adds `PYTHONPATH=tools/skin .venv-skin/bin/python -m delve_skin --help`.

Nothing on this page needs `pytest`. It is the engine's own test dependency, not
an authoring one.

## Which placement model

**A campaign has exactly one placement authority, and choosing is the first
thing you do.** Declaring both is refused by name (`DW0839`), so this is not a
decision you can defer or revisit cheaply.

**`areas[]`** — pieces from the prefab library, seated on a fixed stride.
Take it when the campaign is a small number of rooms the library already has.

**A site plan** — the whole map's design of record, from which the engine
*derives* the geometry. Take it whenever the map is the point: when the brief
describes a place with a shape, when the party has to walk somewhere and the
walking is the content, when there is no prefab that is the building the story
is about.

The two branches differ only at step 2 and step 13. Everything else — quests,
NPCs, dialogue, gates, shortcuts, the whole ladder — is identical and does not
know the difference.

**Before you choose, look at a real one.** `delvec metrics --gym <dir>` writes
a complete, buildable, nine-document site-plan campaign in one command, with
nothing to configure:

```sh
delvec metrics --gym /tmp/gym
delvec build /tmp/gym -o /tmp/gym-out
```

Those nine documents are the worked example this page does not print: a real
`world.json`, `npcs.json`, `classes.json`, `quest-plan.json`, `quests.json`,
`dialogue.json`, `geometry-brief.json`, `layout-graph.json` and
`site-plan.json`, all in the shape the schema actually accepts. Read them
whenever a field on this page is not clear. It is also the campaign that
calibrates the metrics table, so its second job is the one described under
*The site plan* below.
---

# The steps

## 1. The workspace, and the documents you are going to write

Create `campaigns/campaigns/<campaign-id>/`. Everything of the campaign lives
there — the documents, the design record, the generation record, the storybook.
Build output goes beside them and is ignored by git there.

**The document names, all of them.** `delvec validate` reads a whole campaign
and hard-errors on the first one it cannot find, naming it — so if you do not
know the names it will spell them out one run at a time, but you should not have
to. Six are always required:

```
world.json  npcs.json  classes.json  quest-plan.json  quests.json  dialogue.json
```

Five more are conditional, and every one of them is a real campaign document
with its own schema:

| document | when |
|---|---|
| `geometry-brief.json` · `layout-graph.json` · `site-plan.json` | a site-plan campaign — step 2B; a site-plan campaign has no `areas[]` |
| `detail-plan.json` | step 13, optional, and only after the blockout has been walked |
| `world-edits.json` | whenever the map editor was used to fix terrain — see *Reference: tools by symptom* |

**Every document has the same envelope**, and it is four keys:

```json
{
  "dsl_version": "0.17.0",
  "campaign_id": "the-weighbridge",
  "stage": "world",
  "content": { }
}
```

`stage` is the document's own name — `world`, `npcs`, `classes`, `quest-plan`,
`quests`, `dialogue`, `world-edits`, `geometry-brief`, `layout-graph`,
`site-plan`, `detail-plan`. `content` is everything else.

**What number goes in `dsl_version`: the one `delvec --version` printed after
`dsl`.** A new campaign writes the engine's current number on every document.
The per-feature minimums this page states elsewhere ("needs `dsl_version`
0.10.0 on the quests stage") are the *floor* a surface became available at —
they exist so an old campaign keeps compiling unchanged, and a new campaign is
already above all of them. Write the current number and none of those sentences
applies to you.

**Get the shape from the engine, never from memory.** Before writing a
document:

```sh
delvec schema --stage world          # or npcs, quests, site-plan, walk-record, …
delvec schema --stage all            # every document at once
```

The schema is the authority on a document's form. Where anything else disagrees
with it, the schema is what parses. If you want a filled-in example rather than
a schema, `delvec metrics --gym /tmp/gym` writes nine complete documents that
build (see *Which placement model*).

Two more files belong to the campaign and are prose, not documents the compiler
reads:

- **`DESIGN.md`** — the authoritative design record: layout, dramaturgy beats,
  the branch/ending table. Every later round is judged against it.
- **`GENERATION.md`** — the prompt verbatim, the date, the `dsl_version`, the
  decisions you made, this campaign's **posture note** (see *Reference: writing
  craft* §B), and later the findings ledger and the chronicle citation table.

Commit the campaign in the content repository once validation passes.
Conventional message; do not push unless asked. The documents are the artifact
of record: the delve must rebuild byte-identically from them with no model in
the loop.

**While you are authoring incrementally, stub the later documents** and mark the
stubs clearly, so `delvec validate` can run at all. Remember that every stage-2
NPC needs a stage-6 dialogue tree (`DW0152`) and a declared language needs a
covering sidecar (`DW0180`) even at the stub phase.

## 2. Placement — where everything is

### 2A. `areas[]`

`world.json`'s `content.areas` seats pieces from the prefab library. Prefer
`prefab_pool` for real layouts; read `campaigns/prefabs/pools.json` and the
per-prefab metadata for the pools, anchors and lighting profiles available.
Respect the lighting contract — darkness only as declared design, with a
mitigation the quest graph provides.

**Two rules about multiple areas, and both are about pieces rather than about
your story.** Areas sit 256 blocks apart across void with no walkable link, so
crossing between them is not a walk — the compiler emits a one-way teleport on
the objective that crosses, which is what makes "the boulder seals the cave" a
fact about the geometry rather than an assertion. That crossing is emitted only
under two conditions:

1. **The campaign's first beat plays in the area the party starts in.** A
   crossing rides on an objective completing, and at the spawn nothing has
   completed yet — so there is no beat to hang the first crossing on. A delve
   whose opening move would be a teleport out of its own spawn area has put its
   spawn in the wrong area; move the spawn, or put a beat in the spawn area
   first.

2. **Every area a beat crosses into declares an entry point.** That is an
   anchor carrying `"role": "entry"` in the piece's metadata, or — for pieces
   admitted before the role existed — an anchor literally named `spawn` or
   `entry`. **Measured over the shipped library: 5 of 36 prefabs have one**
   (`cave-shore`, `hello-room`, `island-beach-camp`, `island-galley`,
   `keep-spawn-hall`), and in `pool/stone-keep` it is **1 of the 12 members**.
   So a multi-area campaign is a constraint on which piece each area may bind,
   not a free narrative move. Check before you design around it:

```sh
python3 - <<'EOF'
import json, glob, os
for f in sorted(glob.glob("campaigns/prefabs/*.json")):
    if os.path.basename(f) == "pools.json": continue
    a = json.load(open(f)).get("anchors") or {}
    if any(v.get("role") == "entry" for v in a.values()) or {"spawn","entry"} & set(a):
        print(os.path.basename(f)[:-5])
EOF
```

A crossing that was never emitted is not a quiet difference — it is a delve the
party cannot finish. See *Reference: when something goes red* for what it looks
like.

**Two more piece facts worth knowing before you place anything.** An anchor name
is unique per *area*, so binding the same prefab to two areas makes every anchor
it declares ambiguous (`DW0857`); the fix available to you is a **different
piece for one of the two areas**, not renaming an anchor in the shared library.
And if the jigsaw can seat a pool piece twice, the build says so at the pool
declaration (`DW0498`, advisory) and names every anchor that repeat makes
ambiguous — read that line before hanging an objective, NPC stand, gate or wave
spawn on one, because it is a hard `DW0305` the moment you do.

### 2B. The site plan

Three documents, in this order, each the input the next one needs. **The order
is the only order that compiles** — there is no blockout document and nothing to
author early, so no later document can reach green first.

**Before any of them: the whole map gets a reference of its own.** A composition
written without one is free invention with no criterion. On path A of Init step
5 it is already in `design/reference/` — read it. On path B, draw it now; the
form and the commands are in *Reference: drawing the map's reference*.

1. **`geometry-brief.json`** — the whole's written design reduced to *numbers*:
   `facts[]` of `{id, value, unit?, note}`. A fact is a number with a name.
   Write the numbers the design actually commits to — how far across the site
   is, how tall the thing the campaign is named after stands, how far the
   approach runs. Reference imagery is style authority and never dimensional
   authority: an identity binds to a number, never to a picture.

2. **`layout-graph.json`** — the space as a graph, **before any coordinate
   exists**. `nodes[]` are places (`{id, intent, size_class, note?}`); `edges[]`
   are connections (`walk | stair | drop | barred | vision`, with `gating`,
   `one_way`, `shortcut`, `opens_from`). Plus `entry`, `goal`, an authored
   `critical_path[]`, and `beats[]` binding every place-bound quest beat to the
   node it happens in.
   - `size_class` and every seam `opening` name an entry in the **metrics
     table**. `delvec metrics` prints it — 333 lines of JSON on stdout, and a
     summary plus its binding counts on stderr, so read them separately:
     `delvec metrics > table.json`. The keys are the names to write
     (`size-class.hall`, `opening.arch`); a name the table does not define is
     `DW0812`.
   - The graph is checked as a graph, cheaply, before geometry exists to make
     it expensive: every place reachable under gating (`DW0816`), the authored
     critical path actually a quest-legal path (`DW0817`), no one-way edge that
     strands a body (`DW0819`), no "shortcut" that closes no loop (`DW0820`).
   - `intent` is a free label no check keys on — write what the place is *for*.

3. **`site-plan.json`** — the geometric embedding of that graph. `region` (the
   whole map's one box, in world coordinates), `datums` (named ground planes),
   **one `boxes[]` entry per node**, **one `seams[]` entry per traversal edge**,
   `volumes[]` for mass the whole owns (the mountain a cave is inside), a
   `sightlines[]` entry per `vision` edge, optional `views[]` for the walk to
   judge the silhouette from, and `identities[]` binding the plan back to the
   brief's facts.
   - **Extent flows down.** The region comes from the brief and the boxes
     partition it. A box is never grounds to grow the region (`DW0826`): shrink
     or move the box, or change the brief's fact and re-derive, visibly.
   - **A box is the play space, and connected boxes sit exactly ONE CELL
     apart.** This decides every coordinate in the document, so get it right
     before placing anything. `extent` is the interior a body stands in; the
     walls are not inside it — they stand in the one-cell gap between two
     neighbours, so the plan never states a wall thickness anywhere. A box at
     `min: [4, 4]` with `extent: [4, 4]` occupies x 4..7, so its eastern
     neighbour's `min` x is **9**, never 8. Place two boxes flush and they have
     no wall for a seam to be cut through, and every pair of them is `DW0828`.
     `min` and `extent` are two horizontal numbers each, never three — the
     vertical position is `floor` and the vertical size is `ceiling`.
   - **Seams are allocated, not discovered.** A seam sits on a face the two
     boxes already share, at declared cells, at a standard opening (`DW0828`,
     `DW0829`). Two places that cannot mate is resolved here, while both boxes
     are still free.
   - A stair's rise is not authored — it is the difference between the two
     floors the plan already chose — and its `stair_in` names which box pays for
     the run (`DW0830`). Treads rise off a walk plane, so `stair_in` is always
     the LOWER place.

**A site-plan campaign has one area, `area/site`.** Quests, NPCs and waves name
it; `world.json`'s `areas[]` is empty, and declaring both authorities is
`DW0839`.

**The anchors are synthesized, and these are the only names there are** — there
are no prefabs to read anchor names out of:

| Anchor | Where |
| --- | --- |
| `spawn` | the entry node |
| `anchor/node-<place>` | the floor centre of each place — where NPCs, waves and `reach-anchor` objectives go |
| `anchor/seam-<edge>` | the gate region over a `barred` seam: what `open-gate` or a `shortcut` names |
| `anchor/unlock-<edge>` | the far-side affordance of a one-sided `barred` seam, where a `shortcut`'s `unlock` stands. Present only when `opens_from` is `a` or `b` |

`<place>` and `<edge>` are the part of the id after the `/` — `node/near-hall`
becomes `anchor/node-near-hall`. Every barred way must be opened by something
naming its own seam (`DW0818`), and a sealed door a player can push on owes an
answer (`DW0429`) — a `use` trigger anchored on the gate.

**The numbers the whole thing is built to are provisional** until the metrics
gym has been walked, and every build says so (`DW0813`). That is the gym's
second job: `delvec metrics --gym <dir>` builds a site-plan campaign out of the
table itself — one place per rung of the size-class ladder at each of its
bounds, every standard opening, both stair pitches, a designed fall at the drop
policy's cap. It reports what the table defines that it could not instantiate
(`DW0840`) — read that line, not just the green.

## 3. The story documents — `npcs`, `classes`, `quest-plan`

Write them in that order; each conditions the next. Get each one's shape from
`delvec schema --stage <name>` first, and run `delvec validate <campaign-dir>`
after each, fixing by diagnostic code. **Three failed repairs on the same code
means stop and look at the design**, not at the syntax.

- **NPCs**: personas per the schema — `archetype`, `speech_style` and
  `motivation` are required, and step 5 honours them in every line.
- **Classes**: pre-provided gear, no grind. If the campaign places a bonfire,
  **every class kit must declare a flask** (`DW0476`) — see *Reference:
  authoring pitfalls*.
- **Quest plan**: acts, dependencies, which area each quest belongs to,
  mandatory-only quests, paced to `target_minutes`.
- **Declare every story fork here.** If a choice forks who lives, where the
  party ends up, or which ending plays, it is a `branch_points` entry:
  `{id, opens_at, forks_on:[flags], branches:[{id, flags, leads_to}]}`.
  `leads_to` is one field — a `quest/…` the branches converge at, or an
  `ending/…` this branch runs to; the id prefix says which. Name each ending on
  the `campaign-complete` that fires it (`"ending": "ending/<slug>"`). A flag
  that gates casts, staging or quest structure and is not set on every
  playthrough must belong to a declared point (`DW0480`). Every declared branch
  must reach an ending (`DW0482`) and must be exclusive: no sibling's flag may
  be producible on it (`DW0484`).

If someone else gave you the brief, show them a 3–6 line summary of each
document — the summary, not the JSON — and wait, unless they asked for an
uninterrupted run.

## 4. The design gate — pictures, and someone says yes

**Stop here. Do not begin step 5 until this gate is passed.** Steps 1–3 settle
*what the delve is*; step 5 is where the expensive authoring happens, and every
problem this gate would have caught gets paid for twice once it is written.

Deliver a **walkthrough of the whole design**: the complete story, every scene's
design, and each scene carrying **both a near view and a far view**. Near view is
the scene as a player stands in it; far view is the same scene in its
surroundings, so staging and sightlines read. Not a document with pictures in it
— a visual walkthrough, in the medium the review happens in. A design the
reviewer cannot see is a design they cannot approve.

**A confirmation is an explicit yes, not the absence of an objection.** If you
are running the whole thing uninterrupted, this gate still happens — an
uninterrupted run removes the per-step pauses, not the one gate whose whole
purpose is human judgment.

**Which pictures these are.** At this gate they are **reference images**:
concept art drawn from the scene description *before any prefab exists*, so what
is confirmed is the design, not a build. A **render** is a candidate prefab
imaged by `delve-render`, and belongs to curation later. Two stages, two
producers; building prefabs first and rendering them inverts the gate.

- On path A of Init step 5, the images are already in
  `campaigns/<id>/design/concept/` and already approved, with `design/README.md`
  carrying the date and the approved names. The gate is: present the design
  beside them and confirm the design still is what they show.
- On path B, `tools/refimg.py` draws them; prompt iteration is the work, and a
  subject needing more than one view is drawn as a **sequence of single
  full-frame views**, never one canvas cut into panels — the form is in
  *Reference: drawing the map's reference*.

**The moment images are confirmed they become campaign files.** Copy them **and
their `.json` sidecars** to `campaigns/<id>/design/concept/`, one per scene,
named for the scene, and write `campaigns/<id>/design/README.md` carrying the
approval date, the approved names, and the sentence every later round is held
to: *author from the image, judge against it, present every choice beside it.*
Commit them with the campaign. `tools/refimg.py` writes to a gitignored working
directory, which is right for a draft and wrong for an approved one — **an
approval that lives only in a published page is bound to nothing.** The sidecar
travels with the image because it is what makes the image re-issuable with one
word changed: prompt, style note, resolved frame, anchor id. An image whose
prompt is gone can only be replaced, never edited.

**Every later step that asks anyone to choose reads `design/` first**, and
presents the choice beside that scene's image, under the approved name, saying
which element of the image the thing on offer corresponds to. This binds hardest
on contact-sheet curation, which is the step most likely to run in a later
session that never saw this gate.

Two tools help when a still image cannot answer the question:

- `delvec viewer <nbt|dir|manifest.json> -o <page.html>` — one self-contained
  page the reviewer drives: orbit, plan, a player point of view at eye height at
  every anchor, and a cutaway that takes the roof off. Every block is drawn from
  the pinned version's own models and textures, so a wall is a wall.
- `delvec contact-sheet <renders> -o <png>` — when candidate prefabs exist and
  someone is choosing between them, all of them on one page. `tools/refscore.py`
  can order the page by similarity to this gate's reference image; the score
  only **orders** the page, it never removes a candidate.

**A structural device enters a campaign only behind a green machine gate.** If a
shortcut loop, a one-way drop, an ambush reversal or a multi-path interlock has
no machine gate proving its class, it does not go in yet. Never author it now
and prove it later. When a design wants a device whose gate does not exist, that
is a capability gap: report it, and either the gate lands first or the design
does without it.

## 5. The content documents — `quests` and `dialogue`

This is the long step. **What the DSL can express is in *Reference: what a quest
can do*** — read it before writing, not after a refusal. **How the prose has to
be written is in *Reference: writing craft*** — run its section A over every
line before calling this step done.

The order inside `quests.json` matters: **write the `cast` block first, before
the objectives.** Every quest declares, for every NPC live in it,
`{at, doing, dialogue}` — position first, story second.

Then `dialogue.json`. Two rules that are not style preferences:

- **A dialogue option is a button caption, not a sentence.** Vanilla draws each
  option on a fixed 150-GUI-px button and *scrolls* a label that does not fit.
  Author to **≤20 Latin / ≤12 Han characters**; the compiler refuses over-long
  ones (`DW0331`). What does not fit belongs in the node's body text, which
  wraps, in the option's `tooltip`, or in the NPC's reply — never in the button.
- **Re-derive every node's option list from that node's situation.** Never carry
  an option list forward from an earlier node.

Loop `delvec validate <campaign-dir>` until clean after each.

If the campaign declares other languages, the localization stage is a **final
document stage after `dialogue`** — see *Reference: other languages*. It does
not change anything below.

## 6. `delvec fmt` — every campaign, every time

**Mandatory, for every campaign, whether or not it has a second language**, and
again after every later fix including every playtest-round repair.

```sh
delvec fmt campaigns/campaigns/<id>
```

It rewrites every document and l10n sidecar in canonical form: object keys
sorted, two-space indent, non-ASCII raw, one trailing newline. It exists because
a three-key insertion into a non-canonical file produces a hundred-line diff
that nobody can review. **Array order is semantic and it never touches it**
(`quests[]`, `objectives[]`, `effects[]` are ordered), and it proves that on
every file it writes — so running it is never a risk to the campaign. It states
its binding count: `examined N file(s); reformatted M, 0 unparseable`.

Exit 1 means something is wrong with the JSON itself, not with its layout:
`DW0770` unparseable (it prints `line:col`), `DW0771` a duplicate object key —
which means one of the two values is already being silently discarded, so fix
the document rather than the formatter. Never hand-sort a file, and never "fix"
a `DW0773` by editing: re-run `fmt`. CI runs `delvec fmt --check`, so a campaign
that skips this reds there instead.

## 7. `delvec analyze`

```sh
delvec analyze campaigns/campaigns/<id>
```

Quest-graph reachability, deadlock, dark-room mitigation. Fix findings in the
documents — never by weakening the campaign. A dead quest is a design bug.

## 8. `delvec build`

```sh
delvec build campaigns/campaigns/<id> -o validation/delve-output
```

Must exit 0. **Build into `validation/delve-output`** (or copy the tree there
afterwards): the ladder at step 10 builds a container image from that directory,
so a build tree outside `validation/` fails there with
`failed to read dockerfile`, which reads as a broken harness and is not one.

The build writes more than a datapack. Three things to read every time:

- `critical-path.json`, at the **root** of the output — the playthrough the
  proof found, step by step. A step that crosses areas carries a `transport`
  key; step 2A is about what puts it there.
- `render-plan.json` — the deterministic shot list, each shot with the `expect`
  line step 12 checks it against.
- For a site-plan campaign, the three hashes and the engine revision it prints
  at the end. Step 13 copies them.

**There is no blockout document and nothing to author early.** A site-plan
campaign's geometry is derived from the plan and the metrics table by this
command, which then runs the battery over the bytes it laid: every seam built
where it was allocated (`DW0836`), every place reached from the entry
(`DW0837`), and no crossing between places anywhere a seam was not allocated
(`DW0838`).

## 9. Walk it yourself

Get into the world **now**, before the ladder and before any review. A blockout
you have stood in tells you things no picture and no green check will: scale,
whether the route reads, whether the silhouette is the thing you designed.

```sh
python3 tools/staging-gate.py --campaign campaigns/campaigns/<id> \
    --build validation/delve-output --report round-1-gate.md
EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
    --profile play up
```

Then Minecraft → Multiplayer → Direct Connect → `localhost:25565`.

The staging gate is not optional here and not skippable by going around it:
`owner-play.yaml` is the only file that publishes 25565, and it refuses to start
the server without a token the gate minted for *that exact build tree*.

**A red gate is not a defect count.** It is the list of defect classes a
playtester is not protected from, drawn from every finding ever reported on any
campaign — so a campaign that contains none of the objects a row is about shows
as `UNBOUND`, and that is a fact about the ledger, not about your delve. Read
the list, put it in the round summary item by item, and never backfill a weak
check to turn a row green. To go in anyway on a build you know is red:

```sh
python3 tools/staging-gate.py --campaign <dir> --build <out> \
    --stage-anyway "<why this session needs a red build>" --acknowledge-red <N>
```

It prints every class being overridden, records the reason, and the server
announces it at boot — so anything hit from those classes in that session is the
override, not a new finding.

For a site-plan campaign **this walk is the campaign's first real gate**: scale,
pacing, route legibility, and the silhouette from the declared `views[]`. A
finding edits the graph or the plan and regenerates — there is no hand edit to
lose, because there was never a hand edit to make.

Tear the server down when you are out: `tools/playtest-server.sh down`, or
`docker compose … down -v` for the compose path.

## 10. The machine ladder

Docker required. Every entry script takes a **`--project <id>`** and it is
required everywhere: the validation stack pins no container name and publishes
no host port, so the compose project is the only name the stack has, and two
ladders with distinct ids run side by side with no lock and no queueing. An
entry script invoked without one fails loudly rather than landing in a shared
default. Use `dw-<campaign>-r<round>`.

Each script fresh-volumes its own project before and after every run, so a
persisted world cannot keep completed objectives completed and fail a "fresh"
playthrough for reasons that have nothing to do with the delve.

```sh
EULA=TRUE validation/packtest-run.sh --project dw-<campaign>-r1
EULA=TRUE validation/bot-run.sh      --project dw-<campaign>-r1
```

Both must exit 0. Then read `validation/run-out/<id>/run-report.json` — it is
project-scoped, so two ladders can never overwrite each other's.

- The bot ladder has two labelled stages once the delve has mandatory combat:
  `critical-path` and `die-retry`. The die-retry stage adds two scripted deaths
  per encounter, so a combat-heavy delve needs a larger timeout than the
  20-minute default: `DELVEWRIGHT_RUN_TIMEOUT_MS=2400000` on the command.
- **Read the `floor_gate` block every time.** It is the compiler's coverage
  ledger. `not_covered` names each fight the delve bills `elite`/`boss` that the
  gate cannot measure, with the reason — an empty findings list over an
  uncovered elite is silence, not a pass. **`covered`, `not_covered` and
  `actors[]` all empty is the worst case, not the best**: it means no body in the
  campaign declares a tier, the gate examined nothing, and it would have been
  green no matter what you shipped. Report that as **unbound**, never as a pass.
- An **empty `assist_windows`** is not evidence of anything on its own — read
  the `encounters` block beside it, which states each encounter's assist policy
  and the phase the run reached. Expect several windows per encounter.
- Reading one death trial: `respawn_pos` is where the bot actually came back and
  `at_checkpoint` is derived from it; `returned` is the walk back from exactly
  there. `re_engaged` and `outcome` are observed ONLY when `returned` — a trial
  that never got back reads `outcome: unproven`, which means the loop was never
  in a position to be judged, not that the fight vanished. `kit_kept: false` is a
  broken world seal, not a difficulty knob. A red `die-retry` is a content bug of
  the most serious kind: the delve is completable but dying is not safe. Never
  set `DELVEWRIGHT_DIE_RETRY=0` to get green — the report records a skipped stage
  as skipped, not as passed.

**Branch runs, required whenever the build emitted `validation/branch-plan.json`.**
One critical-path run proves one storyline; a campaign that forks must have every
branch walked, each in its own fresh world.

```sh
EULA=TRUE validation/branch-runs.sh --project dw-<campaign>-r1
```

It writes `validation/run-out/<id>/branch-runs.json`: per branch, ran or
skipped-with-reason, and the result. `DELVEWRIGHT_BRANCHES=<ids>` narrows it for
local iteration, and **a narrowed run is not a validated campaign** — the report
says which branches it skipped.

**On any red, triage before touching anything.**

- *Content bug* — your documents declare something wrong, unreachable or unlit:
  fix the campaign in the documents.
- *Toolchain bug* — the compiler or harness misbehaves on a campaign the
  diagnostics accept: **stop content work and report it**, with evidence. Never
  hand-edit compiler output, never restructure the campaign to dodge the bug,
  never weaken a check or reroll a seed to get green. A workaround that turns a
  toolchain bug green ships the bug to every future campaign. Escalating is
  success.

*Reference: when something goes red* maps the symptoms you are most likely to
meet to their actual causes.

## 11. The branch chronicle — only when the plan declares `branch_points`

Skip if it does not. If it does, this step is not optional and not delegable:
skip it and the campaign is not verified, however green the ladder is.

The compiler has compiled your documents **back into natural language**.
`<out>/validation/branch-chronicle-<branch>.md` is one branch's storyline in
compiled play order — every reachable node's `happening` line, first beat to
ending — and `validation/branch-plan.json` lists the branches. You compare like
with like: prose against prose. Nobody reliably compiles JSON in their head.

For **each** branch:

a. Read its chronicle **end to end, in order, in one pass.** Do not skim and do
   not sample: what this catches are contradictions in SEQUENCE ("Antiphos
   survives" at line 12, "Elpenor mourns Antiphos" at line 31).
b. Read it against `DESIGN.md`. Every beat the design promises on this branch
   must appear in the chronicle; every beat in the chronicle must be one the
   design licenses on this branch.
c. Read it against the dialogue **reachable on that branch**. **Every dialogue
   line touching branch-divergent state — who is alive, who is where, what was
   sealed, opened, lost or gained — must be LICENSED by a chronicle line of that
   branch.** An unlicensed line is a finding, not a matter of taste.
d. Write the **citation table into `GENERATION.md`**. Every finding and every
   clearance cites chronicle lines by number:

   | branch | claim reviewed (dialogue/design beat) | chronicle line(s) | verdict |
   |---|---|---|---|
   | `branch/flee` | Elpenor: "We lost him at the mouth." | 14 `departs` | cleared |
   | `branch/flee` | Kalliope: "Antiphos is dead." | — | **FINDING** — no chronicle line licenses a death on this branch |

The pass **fails** if any branch-divergent dialogue line has no citation, if a
branch has no table rows at all, or if any row's verdict is a finding. A finding
is fixed in the documents — move the line behind the right flag, swap the cast's
dialogue root for that branch, or fix the branch the beat is on — and the review
re-run. Never argued away, never left for the human QA hour.

## 12. Visual review

Yours to do, not a checklist to hand off: judging a frame is the whole task.

**Judge the player's eye first and the set second.** The question a playtest asks
is *what does a player walking in experience*, and only a first-person frame on
the actual assembled route answers it. The build emits those: a `pov` camera at
eye height on every corner-thinned critical-path waypoint, looking along the walk
and, at each leg's end, toward the objective it arrives at, each with its own
machine `expect` line. Every POV eye sits on a proven-standable waypoint.

**So read the POV sequence in route order before you open a single orbit
render, and treat it as the primary evidence.** A scene that photographs well
from outside and reads as a corridor of grey stone from the doorway is a
finding, not a pass.

```sh
validation/render-shots.sh validation/delve-output
```

That writes **Chunky scenes, not images** — one per shot, plus the shot index.
Turn the ones you want into pictures with the Chunky you installed at Init step
4, one process per scene, in parallel:

```sh
java -jar ChunkyLauncher.jar -scene-dir validation/delve-output/shots/scenes \
    -render <scene-name> -f -target 64
java -jar ChunkyLauncher.jar -scene-dir validation/delve-output/shots/scenes \
    -snapshot <scene-name> <out>.png
```

`<scene-name>` is the file stem without `.json`. `-target 64` is a look;
about 300 is a shipped frame. This step does not skip — a visual channel that
fails soft is a review that passed without looking.

Every camera in `render-plan.json` is proven to stand in open air (`DW0724`);
read `camera_eye_proof` for how many were examined and how many had to be pulled
in off a stand-off that was inside geometry, and treat a shot carrying
`camera.requested_pos` as a hint about the build — the room is tighter than the
shot wanted, which is worth a look while you are there.

**Then the pieces**, for an `areas[]` campaign. Render **the pieces your
campaign actually uses**, one at a time:

```sh
delve-render piece campaigns/prefabs/<piece>.nbt -o <workspace>/renders/<piece>
delve-render fidelity-gate      # must exit 0 before trusting any render
```

`delve-render batch <dir>` renders every prefab in a directory — 36 pieces and
435 shots for the shipped library — which is a library-curation tool, not a
campaign-review one. A site-plan campaign has no prefabs at this step at all.

Open the exterior/top/interior/anchor PNGs and check each against its `expect`
line: marker visible? room not dark? NPC facing the camera with its name as text
rather than JSON? seam clean? **Findings are document-level** — fix the campaign
(lighting profile, anchor, NPC facing, name string) and rebuild. Never hand-edit
output. Declared-dark interiors render faithfully dark; review those in-game
under the night-vision mitigation rather than brightening a scene to pass review.

`delvec viewer <nbt|dir|manifest.json> -o <page.html>` is the CPU half of the
same channel and needs no GPU. Read its fidelity list before handing the page to
anyone: it names every blockstate the page cannot draw as the game draws it — a
block the pinned version does not have (`DW0790`), and the one that reads as
fine and is not, a palette entry that leaves shape-carrying properties unwritten
(`DW0791`), where the shape comes from the version's default state rather than
from the file. That is a defect in the prefab, not in the page.

## 13. Detail — site-plan campaigns only, and only after the walk

Optional. A blockout is walkable and legible and made of concrete; detailing
replaces one place's massing with a real building. **Detail one place at a
time** — every unbound box is still massed, so the map builds, walks and renders
at every point between none detailed and all of them.

`detail-plan.json` has two fields and **there is no coordinate in it and no way
to write one** — no region, no extent, no datum, no seam, no offset. A `place`
and a `piece` is all a row can say:

```json
{
  "palette": { "role/wall": "minecraft:stone_bricks" },
  "details": [
    { "place": "node/near-hall",
      "piece": "prefab/near-hall",
      "anchors": { "anchor/node-near-hall": "hearth" } }
  ]
}
```

Where the piece goes is computed from the site plan's own box: the play space
plus the one floor course under it. The piece must be **exactly** that shape —
undersize is refused the same way oversize is (`DW0843`), because the box is the
footprint and a smaller building means a smaller box, which is a site-plan edit
and another walk.

1. **Record the walk you did at step 9.** Write `walk-record.json` beside the
   documents — `delvec schema --stage walk-record` is its shape. It is a
   campaign artifact rather than a stage document, so it carries no
   `dsl_version`, no `campaign_id` and no `stage`. Fill it with the three hashes
   every site-plan build prints (`site_plan_sha256`, `layout_graph_sha256`,
   `blockout_sha256`), the engine revision printed beside them, `verdict:
   "passed"`, and whatever the walk noted. **Copy all four out of the build
   output rather than computing them.** Nothing about detail compiles without it
   (`DW0841`), including asking for an allocation. **The first two hashes are
   the record's freshness key**: the whole a walk judges is derived from the
   plan AND the graph, so editing either one — even an edit that moves no block,
   such as which side a barred way opens from — re-opens this gate and asks for
   another walk.
2. **`delvec allocation <campaign-dir> <place>`** — the frame's extents, the
   datum, every seam with the face class it must be answered by, and the owed
   anchor names. Build the piece against that and nothing else. It is an input
   to nothing; ask again whenever you want it.
3. **Build the piece** — a grammar program's export or a piece admitted through
   `delve-admit`; the engine consumes the object, never the tool that made it.
   It must carry a spatial contract (`DW0843`), answer every seam, and open no
   way the plan did not allocate (`DW0844`, both directions). See *Reference:
   when the library has no piece you need*.
4. **Bind it**, re-binding each owed anchor name to one of the piece's own
   anchors (`DW0845`). That is what keeps the quest layer working: those names
   were bound to places before any detail existed, and detailing must never
   force a quest edit.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.

## 14. Hand it over

**The storybook.** Write `campaigns/campaigns/<id>/README.md` — the
reader-facing introduction. Background and setting ONLY: premise, lore, public
NPC introductions (never a persona's `secret`), classes, playtime, the build and
play commands. No puzzle solutions, no quest structure, no endings. Images are
relative links into `media/`, small JPEGs, exterior or starting-scene shots
only, picked from the visual-review set — never interiors or late-game
locations. A localized `README.<code>.md` per declared language.

Storybook art is Chunky, in two passes. Draft with `delvec snapshot` — fast,
disposable, for judging *layout*: is the right thing in frame, from the right
side, at the right distance. Then produce the shipped image with Chunky from
`validation/render-shots.sh`'s scene set, plus `delvec panorama <build-dir> -o
<dir>` for the whole-map hero shot every release owes (`--bearing` picks the
corner). Never hand-edit a scene JSON: if the frame you want is not emittable,
that is a `delve-render` gap to report, not a file to patch.

**Every edition opens with the engine-version marker**, on its own line directly
under the title. This is the one piece of internal machinery a storybook carries
— it is what a server host needs before running the delve — so it stays in this
exact form and nothing else internal joins it:

```
> **Requires delve engine <max per-stage dsl_version> or newer** — last verified with delvec <version>.
```

The first number is the MAX `dsl_version` over the campaign's documents; the
second is `delvec --version`'s, from the build that just went green. The line is
byte-identical in every localized edition — it is a version stamp, not prose. A
translated gloss may follow on the next line but may not restate the numbers.

**Write no other version number anywhere in the storybook.** The marker is the
only one a check can keep true; every other is hand-typed and goes stale in
silence. So: no campaign-version stamp, and the host command names `:latest` —
that IS the storybook's claim — with one sentence sending a reader who wants an
exact version to the release page, where the tag is machine-written. Then prove
it:

```sh
python3 tools/check-storybook-version.py --campaigns campaigns/campaigns
```

Green before you report. A stale marker waves a host on an old engine straight
into a delve their engine cannot run.

**Then report**: the campaign summary, the playtime estimate, the validation
results, and the two commands anyone will actually use.

```sh
# play
EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
    --profile play up

# playtest, with in-game notes
EULA=TRUE CREATOR_NAME=<mc name> docker compose -f validation/compose.yaml \
    -f validation/owner-play.yaml --profile playtest up
```

`owner-play.yaml` is what publishes `localhost:25565`; the base compose file
publishes nothing. Both paths run the staging gate first — see step 9.
---

# Reference

Everything below is looked up, not read in order. A step above names the section
it needs.

## Reference: turning a prompt into a campaign

A prompt is a **constraint set over the documents**: honour everything it pins
down — theme, specific levels, plot beats, NPCs, homages — and invent the rest.
Ask two or three clarifying questions only if the prompt is too thin to pick a
theme and a target length; otherwise proceed.

**A thin prompt is creative licence.** When a prompt pins down little — a
one-line theme, no detailed brief — treat it as a **showcase** brief and
deliberately exercise the breadth of what the engine supports, so that a
stranger playing the result discovers what it can do. Do not work from a written
feature list, which rots as the schema moves: **query the live schema**
(`delvec schema --stage all`) for the available verbs and effects, then aim to
include, wherever the story can carry them coherently: multi-area transport as a
narrative beat, flag-gated dialogue consequences, real props and set dressing,
at least one tuned combat or stealth encounter, narration beats, and varied NPC
presentation. **Coherence and pacing always win over feature count** — never
bolt on a mechanic the story cannot motivate.

**A detailed brief is the opposite**: honour exactly what it pins down and
showcase nothing extra.

**Every round changes only what was asked for.** A mechanics fix must not
incidentally rewrite story, staging or dialogue; an approved change that moves
the design updates `DESIGN.md` in the same commit; and every round ends with a
conformance review — diff the campaign's current behaviour against `DESIGN.md`
beat by beat and report any deviation nobody asked for instead of shipping it.
Drift found in review is restored to the design or escalated, never silently
kept.

## Reference: what a quest can do

The verbs, effects and fields available in `quests.json` and `dialogue.json`.
Get the exact shapes from `delvec schema --stage quests` and `--stage dialogue`;
this section is what they are *for* and the traps in each.

### Objectives

- **Author `title` and `hint` for every non-`talk-to` objective.** `title` is a
  short player-facing name ("Unbar the Deep Gate"); `hint` is one line of
  location or direction guidance ("Past the entrance hall, take the left passage
  to the barred door"). The compiler surfaces them in-game when the objective
  activates, in chat and with a sound; without them the player gets no guidance
  and cannot find interact/collect/reach targets. For `talk-to` they are
  **required whenever the target NPC is not already visible from where the
  previous objective completed** — a different room, down a corridor, across an
  area. Read the "may omit" allowance narrowly: an off-screen NPC 60 blocks away
  through an unfamiliar cave leaves the player with nothing.
- **Hint wording**: landmark-relative directions from places the player already
  knows — the entrance hall, the gate, a named NPC. Never room-shape jargon
  ("corner room", "L-shaped hall") and never solver-internal terms (anchor,
  piece or socket ids).
- **`interact.requires_item` is HELD, not carried**: the player must have the
  item in their **main hand** when they click — presenting it is the action.
  Author `missing_item_hint` whenever the empty-handed click deserves diegetic
  feedback (a sleeping giant mumbles in its sleep; a locked door rattles and
  holds). It is narrated in chat to that player, only while the objective is
  open, and without it the click is met with total silence, which reads as a
  broken affordance.

### Items, containers and loot

- **Furnish the containers.** A prefab's chests and barrels are empty until a
  `loot[]` entry fills them (`{id, anchor, items:[{item, count?, name?,
  enchantments?}]}`), and an empty chest reads as a bug to the player. Give
  every reachable container consumables or props; a named `name` is how a prop
  becomes a story object. The container must already exist in the piece at that
  anchor — **the compiler fills furniture, it never places it** (`DW0431`).
  Elites and set-piece actors take `equipment` in the same shape wave mobs use,
  enchantments included.
- **A `collect` has three shapes, and which are available to you depends on the
  library.** Give the item an `item_name` ("Cheese", "Tide Ledger") in all
  three: it is what the player reads on the stack, it translates like every
  other player-visible string, and an unnamed generic item says nothing about
  what the quest asked for.
  1. **`container: <anchor>`** — adopt a chest or barrel **the piece already
     placed**. The compiler fills that container and places nothing of its own;
     a floating chest conjured beside the barrel the player has been walking
     past is the defect this avoids. Set `fill_count` so the container reads
     plausibly full — it counts padding SLOTS after the objective's own stack,
     and `1 + fill_count` must fit the container's 27. The container must really
     be there in the piece (`DW0438`) and must not also be filled by a `loot`
     entry or another `collect` (`DW0435`).

     **Measured over the shipped library: exactly 1 of 36 prefabs can satisfy
     this** — `island-mountain`, through its four `anchor/cheese-barrel*`
     anchors. Five pieces contain a chest or barrel anywhere
     (`hero-galleon-oak`, `hero-standing-monolith`, `island-beach-camp`,
     `island-galley`, `island-mountain`), and only that one stands an anchor on
     one. **Two pieces declare an anchor literally named `anchor/chest` whose
     cell is air** (`cave-room-small`, `keep-room-small-a`) — the obvious thing
     to reach for, and it is `DW0438` every time. In `pool/stone-keep` there is
     no piece that can carry a `container` at all. Check rather than assume; the
     library moves.
  2. **`dropped_by: <wave>`** — the item comes off a body instead of out of a
     box. The compiler places no container and PROVES the chain: that the wave
     really yields the item (`DW0492`) and that its `kill` objective runs first
     (`DW0493`). `dropped_by` names a wave, never an actor — an actor's death is
     observable by no objective. This is the right shape whenever the story can
     carry it, and it needs nothing from the piece library.
  3. **Neither** — the compiler places its own chest at the objective's anchor.
     Legitimate when the room genuinely has no furniture, and the thing to avoid
     is a conjured chest standing *beside* a container the room already has.
  4. If the beat really needs a piece with a container in it and none exists,
     that is a piece to make: *Reference: when the library has no piece you
     need*.
- **An elite or boss leaves ONE thing behind, and you say which.** Give the
  fight's `drops[]` a declared subset — a `{"slot": "main_hand"}` for the axe
  the player watched swing, or a `{"item": …, "name": …}` for a quest token —
  never the whole kit. Only an `elite`/`boss` encounter may declare drops
  (`DW0491`); a slot must be one the same body's `equipment` really fills
  (`DW0490`).

### State, flags and gates

- **A number the world remembers is `state`, not a flag.** A flag is boolean,
  party-wide and one-way — nothing clears one — so it says "this happened" and
  nothing else. When a beat needs a *quantity* that goes down as well as up (a
  toll still owed, a floor a lift is at, whether a ride is in progress), declare
  it in the `state` list: `{"id": "state/<kebab>", "scope": "party" | "player",
  "initial": <n>, "note": "<what the number means>"}`. `scope` is required and
  never guessed — `party` is one shared value, `player` gives each member their
  own. Write it with `set-state` / `add-state` (signed: a negative `amount`
  counts down) / `clear-state` (back to `initial`).
- **Read it in the gate, never in a verb.** `requires_state: [{"state": …,
  "op": "equals"|"not-equals"|"at-least"|"at-most", "value": <n>}]` is accepted
  everywhere `requires_flags` is — an objective, any gatable effect, a trigger,
  a trap, a dialogue option, a cast placement — so "the door opens at zero" and
  "this line is withheld below two" are the same construct.
- **Every datum must be both written somewhere and read somewhere**: a gate
  reading a datum nothing writes is `DW0501`, and a datum no gate reads is
  `DW0502`. Both mean the mechanism is decoration.
- A `player`-scoped datum can only be touched where a player is acting — a
  dialogue option, a cast placement, an `on_death` beat, an effect on a quest
  beat a player completes, or a trigger declaring `audience: "presser"`, which
  runs as the player who clicked. These have **no** acting player and reject one
  (`DW0503`): an objective/trigger/trap *gate*, a party-audience trigger's
  `effects`, a trap's `payload`, a shortcut's `on_unlock`, a `sequence` step and
  a `move-npc`/`move-actor` `on_arrive`. Use `party` scope there.

### The story layer

- **Every story node declares a `happening`.** One line saying what the node
  does to the story: `{verb, text, subject?}`, where `verb` is one of `dies` /
  `survives` / `departs` / `arrives` / `learns` / `believes` / `gains` / `loses`
  / `opens` / `seals`. Required on every quest, every objective, every staging /
  wave / gate / `campaign-complete` effect, and every dialogue option that sets a
  flag — a missing one is `DW0481`. It is the event-flow twin of the cast
  ledger's `doing`: you cannot fill it without deciding what the beat *is*. Keep
  `subject` accurate (`npc/…`, `actor/…`, `wave/…`, `anchor/…`, or an `item/…`
  label) — the compiler reads only the verb and the subject, and uses them to
  catch a dead character who later acts, or a sealed gate later walked through,
  per branch (`DW0485`).

  `happening` belongs to the objects named above, and only those. It is not a
  field on an arbitrary effect: putting one on a quest-level `set-flag` is
  `DW0100`, and the refusal enumerates what that object does take.
- **The `cast` block, first in every quest.** Every quest declares, for every
  NPC live in it, `{at, doing, dialogue}`. `at` is an anchor, or `"offstage"` /
  `"dead"`, which must match a real `despawn-npc` — declaring a position does
  not move anybody (`DW0461`). `doing` is free prose and is the point: you
  cannot fill it without deciding the character's business in this beat, and the
  dialogue stage writes their lines against it. `dialogue` is a dialogue root
  id, `{"barks": [...]}`, `"unchanged"`, or `"none"`.
  - **A sleeping, working or background NPC gets a `barks` pool**, not
    `"none"`. Right-click then yields one inconsequential in-character line
    instead of dead silence. Use `"none"` only when the silence is the
    statement.
  - **Write `"unchanged"` when you are deliberately carrying dialogue forward**
    — never re-spell the same root id, and never omit `dialogue` hoping it
    defaults (it does not: `DW0463`). `"unchanged"` at an NPC's first
    appearance is `DW0466`.
  - **Treat the `DW0467` staleness warning as a design smell, not a nuisance.**
    It means an NPC's right-click never learns that the story moved. Give it a
    scene that changes, or make it a bark-pool background character — do not
    silence it by shuffling spellings.
  - Omitting a live NPC is `DW0460`: an unaccounted NPC is how a crew member
    ends up standing forgotten in an alcove while the player escapes.
- **Post-fork casts are per branch, every quest.** After a fork opens, an NPC
  whose situation differs by branch declares a **list** of placements, each
  gated by the flags of the branch it belongs to — in *every* later quest, not
  just the first. Leaving one ungated as a fallback is `DW0483`: later
  declarations win, so the fallback keeps governing the branch that already has
  its own, and the fork moves the ledger without moving the bodies.

### Dialogue

- **`button = caption, tooltip = the full line.`** When the caption cannot carry
  what the character actually says — the wine beat, where "Pour it out." stands
  for a whole sentence — author the option's optional `tooltip`: vanilla shows it
  in a hover box beside the button. It **wraps** (no `DW0331`, no width budget),
  so it takes a full sentence. Use it for the *said line*, not for hints or
  mechanics; the button still has to be readable on its own, since a player on a
  controller or reading fast never hovers. It translates under its own key.
- **Premise and exposition options must retire once their moment passes**, via
  the cast ledger's dialogue swap (declare a later root) or a flag gate. A "who
  are we" / "what is that thing" option must be **impossible** at the finale.
  The defect this prevents: after a climactic escape, a crew NPC still offering
  "Tell me what he is." and "Is there another way out?" — questions the
  character has already lived through the answers to.
- Flavour NPCs get real trees too.

### Things that change the world

- **A region can be filled or cleared while the delve runs, and no gate need be
  involved.** `fill-region {region{anchor,extent}, block}` writes a block over a
  declared box; `clear-region {region{anchor,extent}}` empties it.
  `open-gate`/`close-gate` are the same operation with the box and the block
  read off a prefab gate anchor — reach for those when a prefab already declares
  the threshold, and for these when the box is yours: a bridge that
  materialises, a floor that sinks, a wall that opens, a platform summoned under
  the party. The completability proof honours them from the point in the quest
  graph where the effect fires: a fill the only route must cross afterwards
  fails the build (`DW0311`), and a clear is credited as passable, so a route may
  legitimately depend on one. **Two things it will not model, so do not build on
  them**: a clear that opens a box into water (the water flows back in and the
  proof does not know), and a clear over rubble another mechanism dropped there
  (a `collapse` debris field, a shut timed gate) — those stay solid.
- **A piece can ship BROKEN and have the campaign repair it.** A zone whose
  spatial contract declares a `way` on one of its edges is severed there as
  built — a stair whose treads are missing, a bridge that is not down, a doorway
  packed with rubble — and `open-way {piece, way}` is what opens it:
  `{ "type": "open-way", "piece": "prefab/z7-bell-tower", "way": "broken-flight" }`.
  This is how a beat *changes the building*: the collapsed stair the party
  rebuilds, the drawbridge the lever lowers, the shortcut opened once from the
  far side and open forever. **You write the reference and nothing else** — the
  cells, the block and the direction all come from the piece's own metadata, so
  there is no geometry to get wrong and no second place for it to drift. Two
  rules follow, both build errors rather than surprises in play: the piece must
  be placed exactly once for the reference to name it (`DW0547`), and anything
  the party is required to reach beyond the break — an objective anchor, an
  NPC's stand, a wave's spawn — needs a **forced** `open-way` on an objective the
  quest graph puts *before* it (`DW0548`). A way nothing opens is fine and stays
  shut: a door that never opens is content. Every staged way's fate is listed in
  `validation/ways.json` after a build.
- **A prefab's gate is SHUT until the campaign opens it.** A gate anchor's
  region holds whatever the prefab authors there — `hello-room`'s doorway is
  iron bars, the island's cave mouth is air — and the compiler measures which. If
  anything the party must reach lies past a barred gate, some objective they are
  **forced** to complete has to `open-gate` it first, or the build fails naming
  the anchor (`DW0317`). "Forced" excludes every optional bundle: a trap payload,
  `on_death`, a shop offer, a shortcut's far-side unlock. To spell "the party
  walks up to this and the door opens", use an environment `trigger` — that one
  counts.
- **A teleport selects a REGION, never a block.** `teleport {from {anchor,
  extent}, to}` moves **everything** inside the box to the destination anchor —
  players and entities alike, which is what makes a cargo platform the same
  mechanism as a passenger one. Nothing is exempt, so do not draw the volume
  over an affordance the engine anchors to a block (an interact objective, a
  click trigger, a bonfire, a shortcut lever, a disarm, a sealed gate): the
  hitbox would ride and the hardware would stay, and the build refuses it
  (`DW0542`). Two things to design AROUND rather than against, both measured on
  the pinned server: **a teleport is not a rescue** — accumulated fall distance
  carries across it unchanged and is charged in full at the destination, so a
  platform arriving under a falling player past ~20 blocks is the surface they
  die on; and **nav does not know about it** — a route that exists only through
  a teleport still fails the completability proof, so keep a walked route to
  anything the critical path needs.

### Danger, death and money

- **A place that kills is DECLARED, never faked with the art.** A cliff whose
  fall must be fatal, a lava pit, an acid pool, an out-of-bounds plane: all one
  declaration, `lethal_volumes[] {id, region{anchor,extent}, message,
  damage_type?}`. Never obtain the behaviour by changing the world instead —
  making the horizon `void` so the fall kills serves exactly one fiction and is
  never the move here. `message` is REQUIRED and is what the player reads as
  they die (blank is `DW0512`); `damage_type` words vanilla's own broadcast
  (`fall`, `on_fire`, …). The volume is geometry the completability proof
  honours: if the party's only route to an objective crosses it the build fails
  naming the volume (`DW0510`), and nothing the campaign POSTS — the entry spawn,
  a checkpoint, a bonfire, an NPC's anchor, a `cast` placement, an actor — may
  sit inside one (`DW0511`). Put the volume where a player can SEE what will
  happen before they commit to it; a killing box nobody can read is 初见杀 with
  no lesson in it.
- **What happens when a player dies is content, not engine behaviour.** The
  quests document takes a campaign-wide `on_death`: a bundle of ordinary effects
  that runs at the moment a player dies, for that player. One per campaign — it
  is not a field on a checkpoint, because dying is true everywhere in the delve;
  put `requires_flags`/`forbids_flags` on the effects inside it if the beat
  should only land in some phase of the story. Do NOT write a death beat the
  mainline depends on: nothing inside it is credited as a flag producer,
  deliberately, so a door it alone opens is a door only a corpse can open.
- **A death that costs something leaves a stake, and the engine decides where.**
  `stakes[] {id, state, forfeit?, max_live?, on_full?, collect_by?,
  collected_message, marker_item?}`, dropped by a `drop-stake` effect in
  `on_death`. The datum must be `player`-scoped (`DW0520`) — a stake is one
  player's wager, never the party's. You do **not** choose where it lands: the
  compiler computes the point, on the walkable way back from the respawn point
  in force, nearest to where they died, so a death in a lethal volume leaves its
  stake at the near lip rather than inside the hazard, and a death on a lift car
  leaves it on solid ground. If your geometry can strand one — a one-way drop
  with no shortcut back — the build fails naming the place (`DW0525`), and the
  fix is a route back or a `lethal_volume`, never deleting the stake. Souls
  behaviour is `max_live: 1, on_full: "replace"`; no death cost at all is
  `max_live: 0`; a memorial at every death site is a larger `max_live` with
  `on_full: "keep"`.
- **A currency is a NAMED datum, and a price is a GATE.** There is no
  `currencies` section and no `price` field, on purpose. Give a `state[]` datum
  a `name` and it becomes a purse the player reads: the engine states
  `<name>: <value>` on that player's action bar on every write, translated like
  any other line. A shop is `shops[] {id, anchor, title, marker_item?,
  offers[{label, tooltip?, effects[], + the ordinary gate}]}`, and its prices are
  `requires_state` comparisons — exactly the ones a door or a dialogue line
  would use. **Write the refusal yourself**: put the purchase behind `at-least
  <price>` and an apology `narrate` behind `at-most <price − 1>`, both as gated
  effects of the same offer, so a player who cannot afford something is told
  rather than left pressing a dead button. An offer with no effects at all is
  `DW0523`. **Order matters and the compiler will tell you (`DW0527`)**: put the
  refusal and any confirmation BEFORE the debit. Sibling effects are consecutive
  commands, so a gate written after the debit reads the balance the debit just
  produced — buy your last coin and you are charged and apologised to in the same
  breath.

### Bodies

- **`base_entity` accepts any entity id, and NPCs are inert by construction.**
  Every NPC is summoned `NoAI,Invulnerable,Silent,NoGravity,PersistenceRequired`
  plus a separate interaction hitbox, and there is no registry validation on the
  field — so any mob id becomes a talking statue that cannot move or hurt
  anyone. This is how a villager-sized cast can include a giant: e.g.
  `minecraft:warden` as a Cyclops you must slip past. *Caveat:* `Silent:1b` also
  suppresses that entity's ambient sounds (the Warden's heartbeat), and the
  emitted `VillagerData` tag is inert on a non-villager.
- **A body that moves unlike its species DECLARES it, and the build holds it to
  the claim.** `traversal { locomotion: ground|climber|flier }` on an NPC or an
  actor. By default the compiler reads locomotion off the entity id — spiders
  climb, ghasts fly, everything else walks and is checked — so a walked leg that
  goes OVER a wall line instead of round to its opening is `DW0453`. If that is
  your fiction (a sheep that climbs), declare it and the advisory is answered. It
  is **not** an off switch: a declaration that changes no verdict is refused
  (`DW0454`), so you may only claim a climber where the route really climbs;
  `aquatic` is refused outright (`DW0455`) because nothing in the model could
  hold a body to it; and no declaration touches the error tier — a declared
  climber still cannot walk through a closed fence gate (`DW0452`). Declare it on
  the body, never on the beat.
- **A status effect is a verb — and it ends by expiring, never by being
  cleared.** `give-effect {effect, seconds, amplifier?, hide_particles?, in?}`
  grants any pinned-1.21.11 status effect; `in {anchor, extent}` narrows it to
  the players inside a box, so "blind whoever is riding" does not blind the
  delve. `seconds` is REQUIRED and there is no infinite form, on purpose: an
  effect whose only removal is a later step is one the player keeps forever
  whenever that step does not run — a logout, a crash, a death mid-chain. So **do
  not write "grant, then clear at the end"**; write a duration that covers the
  beat plus slack and let it expire. Pairing a live grant with a `clear-effect`
  of the same effect in the same bundle is `DW0540`. `clear-effect {effect?,
  in?}` exists for effects the campaign did NOT grant (a potion the player
  drank, a `wither` a mob applied); omit `effect` to clear everything.

### Sealed things, and pacing

- **Anything you seal, you must say what it says.** A `shortcut`'s barred door
  and a `close-gate`'s wall are both things the party walks up to and pushes on,
  and one with nothing to say is `DW0429` — the build refuses. The compiler will
  not decide the tone of your door or your wall for you and then not tell you it
  did. One rule for both: they are two objects of the same class.
  - A `close-gate` discharges it either way — `"sealed_hint": "<what the wall
    says>"` on the effect, or a trigger. `sealed_hint` is only the *wording*.
  - A `shortcut` has no wording field, deliberately: its line is a trigger.
- **Write the reply with the general verb:**

  ```json
  {"id": "trigger/…", "at": "<the gate anchor>", "on": {"on": "use"},
   "once": false, "audience": "presser", "effects": [{"type": "narrate",
   "style": "actionbar", "text": "The door cannot be opened from this side."}]}
  ```

  Anchoring it on the gate is what makes it *ride* the sealed body's own
  hitboxes instead of summoning a co-located second one (`DW0422`), and on a
  shortcut door the body stands on the sealed side only — so the line fires where
  it is true and nowhere else, and retires when the door opens. Once you write
  one, the compiler supplies nothing: one press, one answer. ANY `use` trigger on
  the gate discharges `DW0429`, whatever it does — but a `strike` does not,
  because pressing a thing is a right-click.
  - `audience: "presser"` addresses the one player who clicked, and works on
    `on: use` only — vanilla can attribute right-clicks and nothing else
    (`DW0427`). Leave it out and the beat addresses the whole party, which is
    right for a lever that opens a gate and wrong for a reply.
  - `style: "actionbar"` is the reply strip above the hotbar: it does not
    interrupt, does not stack, and is not width-checked. Use it for replies; use
    `title`/`subtitle` for beats.
  - Trigger ids starting with `dw-` are reserved for the compiler (`DW0428`).
  - Two `close-gate`s on one anchor must still agree on the wording (`DW0423`).
- **A beat that can FAIL the player must not arm before they could have read
  it.** Any fail-able beat — follow-an-NPC, escort, timed escape, stealth onset —
  arms only after a grace window long enough to read the on-screen prompt that
  explains it: the player must never be failable before they could have
  understood what is being asked. Where the DSL has an explicit knob, set it
  consciously rather than inheriting the default (`begin-stealth`'s
  `grace_ticks`); where the pacing is authored, put the first enforcing step late
  enough in the `sequence`'s `at_ticks`. Budget the window from the prompt's
  length, not from a habit — a two-line chat prompt is several seconds of reading
  before the first step is taken. The defect this prevents: the flock the player
  is told to follow leaves while they are still reading the instruction, and the
  beat then fails them for it.
- Pace to `target_minutes`; no grind; mandatory-only quests.

## Reference: writing craft

Everything a player reads is prose: dialogue, objective titles and hints,
narration beats, item and area names, the storybook. This is the craft checklist
for all of it, and section A is run over every line before step 5 is called done.

These are **pattern warnings, not technique bans.** They govern *automatic*
writing — the phrasing that arrives before you have decided anything, the hand
reaching before the mind does — not the device itself. A simile is not
forbidden; the simile you did not choose is. Banning a technique outright
produces stilted avoidance, which is its own tell.

### A. Automatic-phrasing tells

1. **Observation + verdict.** A line, then the text grading it — "*…, more
   statement than question*", "*…, and it was not a request*". The verdict
   instructs the player how to hear what they just read. Cut the verdict; if the
   line cannot stand without it, the line is wrong.
2. **Standalone simile fragments.** "*Like a blow to the chest.*" — a comparison
   set alone as though it were the feeling. Test every simile: does it make the
   player see the **thing** more sharply, or make them notice the author? The
   second kind goes.
3. **Stock intensity moves.** The air growing thick or heavy; time slowing;
   silence stretching; words left hanging in the air; a breath the character did
   not know they were holding. These are the default gestures at "this moment
   matters", and every generated delve reaches for them unprompted.
4. **Repetition as intensity.** Saying it again, louder — "*more than tired:
   hollow*"; three-beat lists where two beats carry all the meaning.
5. **Correction pairs.** Naming a false label in order to knock it down — "*not a
   warning, a promise*"; "*it stopped being a door and became a mouth*". Once per
   campaign is a rhetorical choice; three times is a signature.
6. **Purposeless gesture.** A nod, a tightening jaw, a hand moving to a hilt,
   costing nothing to delete. A gesture signifies by contrast with what that
   character usually does. If it can be cut with no loss, cut it.
7. **Explaining your own subtext.** An NPC says the hard thing, then the next
   line paraphrases what it meant. Trust the player; they have already read it.

Applies hardest where the text is shortest: `hint`, `title`, bark pools and
`missing_item_hint` have no room to recover from a wasted clause.

### B. Convergence is the real tell — vary the posture per campaign

StoryScope (arXiv:2604.03136) separates human from AI fiction at **93.2%
macro-F1 from narrative structure alone, with every stylistic signal withheld**,
and span-level style editing of the prose moves that number by 1.6 points. So
the AI tell is not a phrase you can scrub. It is **convergence**: five different
models occupy one tight region of narrative space while human stories are
dispersed around it (mean rarity 0.49 vs 0.71). Section A is hygiene; this
section is the actual defence.

Measured gaps worth authoring against (AI vs human in that corpus):

| axis | the machine default |
|---|---|
| thematic explicitness | the narrator states the story's point — 77% vs 52% |
| emotion rendering | somatic: tight chest, cold sweat — 81% vs 38%. **Humans name the feeling outright 29% of the time; AI 8%.** |
| plot shape | no subplots 79% vs 57%; protagonist-driven resolution 69% vs 46% |
| resolution | closes on internal understanding or acceptance, 47% vs 27% |
| time order | strictly chronological; humans jump, flash back, withhold |
| morality | morally ambivalent protagonist 38% vs 59% |
| address | humans break the fourth wall (67% vs 39%) and address the audience (28% vs 7%) |

**Claude specifically** is the most distinctive of the models measured, and its
fingerprint is restraint: *the flattest event escalation of any source*, the most
uniform narrative voice, epilogues over avalanche endings, and reverence toward
genre convention rather than subversion (62% vs 39–56%). Read that as a standing
instruction: **the default delve escalates too evenly and ends too quietly.**
Give a campaign a beat that is disproportionate to what came before, and let at
least one thing end badly or unresolved.

Operationally, per campaign:

- Pick **at least three** axes above and push them off the default *for this
  campaign* — a delve told out of order; a cast whose antagonist is right; an
  ending that refuses to explain itself; an NPC who names their fear in plain
  words instead of clenching a fist.
- Record the choice as a one-line **posture note** in `GENERATION.md`: which
  three axes, and how. It is a design commitment, not a report.
- Vary them **between** campaigns. A fixed counter-recipe applied every time just
  builds a second cluster — dispersion is the human signal, not any particular
  pole.
- Corollary, and it inverts the usual advice: **"show, don't tell" is a machine
  default here.** Somatic rendering is what the pole looks like. Sometimes let a
  character say they are afraid.

### C. HARD RULE — dialogue options are labels, not sentences

**A dialogue option is a button caption.** Vanilla draws each option as a
fixed-width button; a label wider than the button *scrolls* rather than wrapping
or shrinking, and a shelf of scrolling captions is a miserable thing to read and
pick from. This is not a style preference — it is the widget.

The geometry, so the budget is arithmetic and not taste. The compiler emits each
node as a `minecraft:multi_action` dialog with `columns: 1` and **no `width`
override**, so every option button is vanilla's default **150 GUI px**, leaving
roughly **146 px** for the label after the widget's inset. Dialog buttons draw at
pose scale ×1, so one font pixel is one GUI pixel — unlike `narrate` titles,
which `DW0330` budgets at ×4/×2.

**Width is measured in font pixels, not characters**, because `i` and `W` differ
by 3× and a Han glyph (advance 9) is 1.5× a Latin one (typical advance 6), so any
character count is unfair to whichever script it was not tuned for. The character
counts below are the authoring rule of thumb derived from those advances — the
pixel budget is the real rule:

| | scroll threshold | **author to** |
|---|---|---|
| English | ~24 characters (146 px ÷ ~6 px average advance) | **≤ 20 characters** |
| Chinese (`zh-*`) | ~16 characters (146 px ÷ 9 px Han advance) | **≤ 12 characters** |

Author to the target, not the threshold: the English is the source a translation
grows from, and a label at the English limit has nowhere to go in `zh-cn`.

```
BAD   "I don't know — are you sure there isn't another way out of the cave?"
GOOD  "Another way out?"

BAD   "我不太确定，你是说这座洞窟还有别的出口吗？"     (20 chars ≈ 180 px — scrolls)
GOOD  "还有别的出口吗？"                              (8 chars ≈ 72 px)
```

The content that does not fit belongs in the node's body text, which wraps
normally, or in the NPC's reply — not in the button. This applies to every
`.opt.<n>.label`, in the English source **and** in every l10n sidecar.

`DW0331` enforces this at compile time on the same font-pixel measurement
`DW0330` uses. Author to the target and it never fires.

### D. HARD RULE — a name spelled the same way IS the same name

Every name you write over a body is translated, and **whether two bodies share
one translation is decided by whether you spelled them identically** — not by
whether you meant the same character. Apply this while you are naming, because it
is unrecoverable later: by the time a translator sees the list, your intent is
gone and only the spelling is left.

**Bodies that are one character: spell the name byte-identically.** A character
usually occupies more than one declaration — an NPC that stands and talks, plus
one actor puppet per cutscene pose it is staged in. Written identically, all of
them are one name: the translator is asked once and every body renders the same
way, in every language.

```
GOOD  npc/polyphemus            "Polyphemus"
      actor/polyphemus-walker   "Polyphemus"      ← same character, same spelling
      actor/polyphemus-roused   "Polyphemus"
      actor/polyphemus-blinded  "Polyphemus"

BAD   npc/polyphemus            "Polyphemus"
      actor/polyphemus-roused   "Polyphemus "     ← a trailing space is a second
      actor/polyphemus-blinded  "polyphemus"        character, and the giant is
                                                    renamed mid-cutscene
```

Differ by a space, a case, or a `the` and the player meets two characters — one
of whom may be called something else entirely in Chinese. Copy the NPC's name; do
not retype it.

**Bodies that are genuinely different: spell them differently.** The rule runs
both ways. Two unrelated NPCs you both called `Guard` are one name and will be
translated once, so if they must read as two people, write two names.

**Wave mobs are the exception, and it is the one to plan around.** A wave mob's
name is *not* pooled with anything: three waves whose mobs you both named
`Drowned of Poseidon` are three separate names, asked of the translator three
times, and free to come back as three different Chinese strings — the same squad
under three names, in one delve. So:

- If several waves really are **one creature**, still write the identical string
  — it is the honest source, and the localization stage carries a glossary that
  holds proper nouns steady across batches. Then **say so in the campaign's
  posture note**, so the localization stage knows those rows must agree.
- If they are **not** one creature, give them names that differ. Do not reuse a
  name for flavour across waves that the fiction treats as distinct — you get the
  cost of a shared name with none of the benefit.

Fewer distinct names is the cheaper delve in every language. A name you reuse
deliberately is free; a name you reuse accidentally is a defect the English build
can never show you.

### E. Plain-prose baseline (Strunk 1918, public domain)

Two rules carry most of the load for text rendered into a chat line:

- Rule 12, "Use definite, specific, concrete language" — the objective hint that
  names a landmark beats the one that names a mood.
- Rule 13, "Omit needless words": *"Vigorous writing is concise. A sentence
  should contain no unnecessary words, a paragraph no unnecessary sentences…"*
  His substitutions are still live — `owing to the fact that` → since, `in spite
  of the fact that` → though, `he is a man who` → he, `in a hasty manner` →
  hastily. And: *"In especial the expression `the fact that` should be revised
  out of every sentence in which it occurs."*

Concision is not the same as flatness. Cut the padding, keep the beat.
## Reference: drawing the map's reference

Needed at step 2B on path B of Init step 5. On path A the views are already in
`design/reference/` and this section is how to *extend* the series, not how to
start one.

**The form is several views of the one subject, each its own image, generated in
sequence.** Front, side, straight-down plan, a named angle — whatever the shape
needs. **Not several views divided into one canvas**: a fixed canvas cut into
four spends three quarters of its resolution on gutters and neighbours, and the
detail a reference exists to preserve is the first thing to go. It also makes
the unit of judgement wrong — one unusable panel forces the whole sheet to be
re-rolled, where one unusable view is re-rolled alone for the cost of one image.

1. Write the **style note** once: what this place is, in what hand, plus the
   sentence that each image is ONE single full-frame view and never a sheet, a
   grid, a panel or an inset. It is held constant across the series
   (`--style-note`), and it is recorded in every sidecar, so a later round can
   extend the series instead of starting one.
2. **View 1 is generated from the prompt alone**, and confirmed for style before
   anything else is drawn. Frame it for what it shows.
3. **Every later view is generated from the prompt plus VIEW 1** — pass view 1's
   interaction id to `--chain-from`, read out of view 1's sidecar (`.id`).
   **Anchor every one of them on the FIRST image, never on the one before it**:
   chaining view to view compounds the drift instead of bounding it. Frame each
   for what it shows — a straight-down site plan is `--aspect-ratio 1:1`, an
   elevation is not — which is per call and never a config edit.

```bash
# the style note, written once and held constant for the whole series
STYLE="Halgrave, in the same hand throughout: <palette, light, brushwork>. Each
image is ONE single full-frame view of that place, filling the frame edge to
edge: never a sheet, never a grid, never a panel, never an inset or a caption."

# view 1 — from the prompt alone, framed as an elevation
python3 tools/refimg.py --prompt-file v1-front.txt --style-note "$STYLE" \
    --aspect-ratio 16:9 --out .refimg/map-view1-front

# read view 1's interaction id out of its sidecar — this is the series anchor
V1=$(python3 -c 'import json;print(json.load(open(".refimg/map-view1-front.json"))["id"])')

# every later view: the same anchor, its own prompt, its own frame
python3 tools/refimg.py --prompt-file v2-west.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 16:9 --out .refimg/map-view2-west
python3 tools/refimg.py --prompt-file v3-plan.txt  --style-note "$STYLE" \
    --chain-from "$V1" --aspect-ratio 1:1  --out .refimg/map-view3-plan
```

`$V1` is the same string in every later call, and that is where the "anchored on
view 1" claim is checked — the `chain_from` field in the sidecars, not a
sentence in a report. A frame the configured provider cannot honour is refused
rather than dropped, so a wrong flag stops the call instead of returning a
correctly-styled picture of the wrong shape.

**The trade, stated because it is why this step has a check in it.**
Co-generating views in one canvas is what guaranteed they agreed about the
*geometry* of the subject; generating them in sequence guarantees only *style*.
So **the geometric facts live in the written brief, and a drift is checked
against text rather than eyeballed** — which is exactly what `geometry-brief.json`
is, and why reference imagery is style authority and never dimensional
authority. Read each view against the brief's facts, not against your memory of
the last picture.

The check this exists to make possible: a zone program once exported as a flat
chain of rooms with no climb, no belfry and no bell, under the name of the tower
its campaign was named after, and it survived until somebody held it against the
zone's own image. A silhouette drawn from three sides is legible enough that the
same collapse cannot pass.

**When the views are confirmed they become campaign files** — copy them and their
sidecars to `campaigns/<id>/design/reference/`, named for the view, and commit
them with the campaign. An approval that lives only in a gitignored working
directory is bound to nothing, and the sidecar is what makes a view re-issuable
with one word changed: it carries the prompt, the style note, the resolved frame
and the anchor id.

## Reference: when the library has no piece you need

Follow `docs/reference/prefab-procedure.md` — it is the procedure, and these are
its mandatory steps, in order. Do not improvise around them. All four binaries
this needs were built at Init step 2.

1. **Write the scene description first** — one or two sentences: what a body does
   in the space, the material feeling, what the campaign will attach. Written
   after the render, it is a description of the render.

2. **Choose the palette by measurement, never from memory** — and it is three
   steps, not one. A block's name is not its appearance (`packed_mud` is orange,
   142/107/80).

   **Screen** the shelf by constraints rather than by a guessed hex:

   ```sh
   python3 tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \
       --where 'C_mean<0.02' --where 'texture_range<=0.30'
   ```

   That takes 1146 blocks to a handful (`L` = Oklab lightness, `C_mean` = how
   coloured, `texture_range` = how loud the pattern; `form=`, `family=`,
   `not tinted`, `not gravity` are facets too). Then **measure the mix**:
   `--mix 'a=3,b=3,c=4'` or `--program p.json` reports `chroma_mass`,
   `chromatic_area`, the **named** `loudest_member` with its area share, and
   `dominant_hue` — never a mean as the verdict, because a mean cannot see that
   60% of a wall is one loud family when the craft rule gives it 10%. A member
   carries its block state, properties and all (`--mix 'deepslate[axis=y]=3,stone=1'`).
   **Read the binding line before the colours**: it reads `examined of declared`,
   declared being the palette's own role count plus each inline fill, so
   `18 of 18` is a measurement of the palette and `8 of 18` is not — a declared
   paint the tool could not read is named with its reason and exits 2.

   Then **LOOK**: `--sheet` writes `.sheets/palette/swatches.png`, every survivor
   tiled and every mix rendered as its seeded weighted tiling — **read that PNG
   before binding anything.** A shortlist is not a choice, and the screen will
   hand you blocks that are right on every measured axis and wrong for the job (a
   light source, a gravity block, wool). Record the measured hex beside each role.

   The tool needs the pinned block registry from `crates/dsl/data/` **and** a
   1.21.11 client jar, and refuses by name when either is absent. That does not
   make the step optional: take role names from the corpus instead
   (`delve-grammar list`, then `delve-grammar show --program <nearest>`), which is
   a palette somebody already measured, and record where each name came from.
   Never invent one — a block that does not exist is refused at export, and one
   that exists but looks nothing like its name is caught only by eye at 5 below.

3. **Author a grammar program.** Read the **idiom index** first
   (`docs/reference/grammar.md` §2c): ten techniques with a runnable program each
   — repetition, `otherwise`, taper/arch/gable (one recursion), air-in-a-mix
   erosion, graded erosion, surface detail, symmetry without reflection, `skip`,
   light, and arguments (`bind` — one rule called with different content). It is
   the part of the language no type signature shows, and a scene that looks
   impossible is usually one of the ten. **Never copy a rule to change its paint,
   its size or its axis**: a caller passes a paint or a size with `bind`, an axis
   with `reorient`, and anything derivable from the box with an expression over
   `dim` — a copied rule family is one nothing keeps in step and no gate reads.

   ```sh
   delve-grammar show --program idiom-shape
   delve-grammar list
   delve-grammar show --program <nearest> > p.json
   delve-grammar check --file p.json          # after every edit
   ```

   You write JSON — never Rust, and never blocks by hand. Four traps the
   procedure names: two guards that can both hold are a **probability, not a
   priority** (the "none of the above" arm is `otherwise`, and it is also what
   stops a recursion); **`rounding` is owed by every surface, not only floors** —
   the default truncates and an unwritten cell is air, which no gate reads; a
   palette role may be a **weighted list with `minecraft:air` in it**, which is
   the whole of decay and the cure for a piece that renders as one flat material;
   and a `facing=` block state **does not turn when the frame turns and does not
   flip when it reflects** — `oriented-fills` (`DW0736`) refuses the piece rather
   than shipping it facing the wrong way. Say which axes the state is written in:
   wrap it as `{"local": "minecraft:iron_bars[east=true,…]"}` and its directions
   mean the scope's own, so one palette role gives the right state at every frame,
   reflections included. Where the whole rule BODY differs by frame, use an
   `orientation` guard instead — one alternative per frame, naming the reflection
   as well as the axes.

   **Decide the split order before the first rule.** A split's children copy the
   parent box on the two axes it does not cut, so siblings of a split are the only
   two things guaranteed to line up, and there is no way to say "this opening is
   the same cells as that one". Hence: **the last axis you split is the only axis
   on which two things are guaranteed to meet — split last on the axis your
   openings run through**, and write a hole as a piece of that split whose
   siblings are the two things that must meet (best as the *absence* of a sibling,
   which cannot be misaligned). Within one axis, pin a course to a band's end and
   not to a height: `[relative 1, absolute 1]` is *the last course of this band* at
   any band height, where `[absolute 5, absolute 1]` is a computed height that also
   refuses a short band. Every constant you do not eliminate this way fails
   silently.

   One more refusal to expect: **`repeat` clamps the last tile but does not rescue
   a box too short for the first one** — one pass of the pattern is resolved before
   any tiling, so a repeat whose absolutes sum to 8 across a 7-deep box is a hard
   refusal. Guard the extent and give the short box an `otherwise` arm.

4. **Expand and let the machine judge**:

   ```sh
   delve-grammar expand --file p.json --region XxYxZ --seed N \
       --traversable --reachable-floor -o out/
   ```

   Pass `--traversable` for any passage, stair or route; pass `--reachable-floor`
   for any piece with an inside a body is meant to walk around. A red gate writes
   no `.nbt` (exit 4). **Read the `findings` in the report** — a gate that bound to
   zero objects, or a program that declared no anchors, is a finding, not a pass.

   Three of the always-on gates are about how a block state is SPELLED —
   `shape-complete` (`DW0735`), `states-complete` (`DW0737`) and `oriented-fills`
   (`DW0736`). Write every property of every block state you paint, including the
   ones whose default looks obvious: a state that omits one means whatever a
   running server decides, and the render you are about to check the piece against
   cannot know which.

   **`oriented-fills` has three answers, not two.** `undecided` (`DW0742`) means
   the piece is not wrong at this region and was not checked at it either: a scope
   reoriented by `largest`/`smallest` stands in the identity frame only while this
   box's axes happen to rank the way the request already names, and at another
   region the same state would be refused. It writes the `.nbt` and it is not a
   red — but it is the one verdict that will change under you the day the zone's
   region does. The named fills are in the gate's detail; wrap each as
   `{"local": …}` and the answer becomes `pass` at every region.

   **Read the `reachability` line too**, which prints whether you asked or not:
   `traversable` joins two ground-level faces and says nothing about the storeys
   above, so a building can pass every gate with half its floor stranded.
   Unreachable floor **under a roof** is a room with no way in, and the report
   gives you the box to go and look at. Unreachable floor open to the sky is a
   roof, and is nobody's defect.

   If the piece is one of a campaign's **zones**, its program belongs to the
   campaign: put it in `campaigns/<campaign>/design/programs/` and name it in
   `zones.json` there with the region, seed and gate claims it is built at
   (`traversable`, `allow_falls`, `reachable_floor`, `symmetric`).
   `delve-grammar audit --campaign-root <content repo>` judges every zone a
   campaign declares, and CI in both repositories runs it — a program that
   directory carries and the manifest does not name is a red.

   **One design the gate cannot be told about: a one-way descent.** A level a body
   drops into and does not climb back out of is unreachable on foot on purpose,
   and nothing in the CLI, the report or the metadata can state that claim. So do
   **not** pass `--reachable-floor` on such a piece — it fails and a red gate
   writes no `.nbt`, so the flag ships nothing rather than shipping a known red.
   Expand without it, read the always-on reachability line, and record in the
   campaign's `GENERATION.md` that the `unreachable_sheltered` pocket it names is
   the drop and not a room with no way in. That verdict is bounded by the
   instrument, and this is the step at which to say so.

5. **Look at it**:

   ```sh
   delve-render piece out/<id>.nbt -o shots/
   ```

   and compare against the scene description from 1 above. The gates prove it is
   buildable and walkable; they
   say nothing about whether it is the scene you asked for. If the expand wrote a
   tile set instead of one `.nbt`, pass the manifest — `delve-render piece
   out/<id>.json` — which renders the assembled zone as one scene, eye shots
   included. Never review a single tile; the command refuses one anyway.

   **Open the `eye-<anchor>.png` frames FIRST.** They are the only cameras inside
   the piece — a body's eye at 1.62, at each declared anchor, looking the way that
   anchor faces. The orbit shots (`ext-*`, `top`, `door-*`, `anchor-*`) are fitted
   from outside, and on a roofed piece they are all the same picture of the same
   rock. Read `<id>-shots.json` beside the images for which cell each body is
   standing in: a camera whose anchor cell held a gate or a barrel steps back
   along the facing and says so (`DW0727`), and an anchor with no body cell gets no
   eye shot at all — the run states that count. A flat grey frame is outside the
   piece, and a shot that is *only* that is reported as an empty frame: the camera
   is aimed at nothing.

   **When the piece is a building whose identity is one elevation** — a west front,
   a gatehouse, an approach face — add the camera for it: `--view
   name=west-front,face=north` (repeatable) appends a level, square-on shot of that
   face of the model, and no planned camera is square-on at a face. `of=` aims it
   at a declared anchor instead of the whole model; `zoom=` tightens or backs off.
   Do not build a forecourt and stand an anchor on it: a 70° eye camera reaches
   only ≈0.7 × its distance above eye height, so it looks through the doorway
   instead of at the façade, and the forecourt shrinks the building in every
   exterior frame. Keys: `docs/reference/tools.md` §4.

6. **Admit it**: the whole `delve-admit` chain (`audit` → `socket` → `anchor` →
   `lighting --write` → `catalog validate`), then `audit` again. For a tile set,
   `audit` and `lighting` both take the **manifest** and answer about one zone;
   handing any command a single tile is `DW0739`, and so is handing it a tile
   copied away from its manifest.

   A grammar prefab has **no connectors and no lighting** until this step, so it
   cannot enter a `prefab_pool` and will be dark, until you do it. `lighting`
   measures the roofed floor a body can walk to from outside and reports the count
   it bound to. Two refusals to expect and not work around: `DW0752` means the
   probe bound to **zero** cells — usually a piece whose only way in is a socket
   that has not been carved yet, so run `socket` first; `DW0753` means there is no
   metadata to write into, and the fix is to create it, never to let the tool
   invent a `spdx: UNKNOWN` one.

**What the grammar cannot express — escalate, do not work around**: block
entities of any kind (chest loot, sign text, spawners — bind those in the
campaign against an anchor the piece declares), **smooth** curves, diagonals, a
profile step that varies independently of the box, a vault bending on two axes at
once, and terrain. **Neither a stepped arch nor a symmetric shape is on this
list** — the first is idiom 3 (one recursion whose step is arithmetic on the
remaining dimension, and the same program inverted is the opening), the second is
idiom 7 (a rule body written mirrored, since `reorient` permutes and never
mirrors). Check §2c before escalating. **Size is not on this list** either: a
region of any extent expands, and one past the 48-per-axis structure-template cap
is written as a tile set plus a manifest at `<id>.json`. Never shrink a scene to
fit a file format. **Tiling changes nothing downstream of the export**: the
campaign binds `prefab/<id>` in the same line it binds any piece, its anchors are
the zone coordinates the program marked, and world assembly places the whole
zone. Nothing an author writes says a piece is tiled, and a document that mentions
a tile is wrong.

A piece that comes from **outside** — a community schematic — instead enters via
`delve-schem convert` and then the same admission chain with `resolve-jigsaw`
before `socket`. Never place an un-audited piece: `audit` is the licence and
code-injection gate, and the `DW0733` check that the blocks in it exist at all.
Flags in `docs/reference/tools.md` §2a and §3.

## Reference: other languages

Needed only when the brief asks for one — or when the brief arrives in a
non-English language **and asks for localized in-game text** (中文文本 etc.). It is
a **final document stage after `dialogue`**, once the English campaign is
complete. Everything in the campaign documents stays English, always; other
languages are delivered as sidecars.

1. Declare the codes in `world.json`: `"languages": ["zh-cn", …]` (BCP-47-style;
   `en` is implicit and canonical and is **never** listed). Each code must be one
   the compiler can map to a Minecraft lang-file name (`zh-cn` → `zh_cn`); an
   unmapped code is `DW0184` at validate time, never a language quietly missing
   from the shipped pack.
2. **Who translates.** If `delvewright.toml`/`delvewright.local.toml` has an
   `[i18n]` section AND the environment variable it names (`api_key_env`) is set:

   ```sh
   python3 tools/i18n-translate.py <campaign-dir> --lang <code> --reflect
   ```

   `--reflect` is the three-step translate → critique → revise pass and is where
   translationese actually dies — always pass it. It writes and validates the
   sidecar for you; then go to 4 below. Otherwise translate yourself, 3 and 4
   below.
   Generation-time only either way — a shipped delve never calls a model.
3. Yourself: `delvec l10n-inventory <campaign-dir> --lang <code>` gives the exact
   key inventory as JSON (key, English, speaking NPC, existing translation).
   **Translate FROM the finished English** — never author a language natively —
   honour each NPC's `persona.speech_style`, keep a Minecraft-appropriate
   register, cover the inventory **exactly**. Run the **same three-step pass the
   tool runs**: draft; then re-read the draft against the English and write down
   what is wrong on accuracy / fluency (including the target language's
   translationese habits — for zh: 名词化, 弱动词, 的的不休, over-marked 被,
   front-loaded modifiers) / style-register / terminology; then revise, leaving
   lines that were already right byte-identical. Write `l10n/<code>.json`:
   `{ dsl_version, campaign_id, kind: "l10n", lang: "<code>", content: { <key>: … } }`.
4. Re-`validate` until zero `DW0180`/`DW0181`. **The default build ships every
   declared language and the client picks its own**: `delvec build` emits each
   authored string as `{"translate": key, "fallback": English}` and writes
   `assets/delvewright/lang/<mc_code>.json` per language into the delve's
   resource pack. A player whose locale you do not ship — or who declines the
   resource-pack prompt — reads the English fallback. Nothing extra to run.
   `delvec build --lang <code>` produces the single-language bake for local dev;
   the release path does not use it. `critical-path.json` is language-neutral
   either way, so the ladder is unchanged.

Then re-run step 6, `delvec fmt` — it covers the sidecars too.

**`fx.` keys are POSITION-derived** (`fx.<quest>.oc.<obj>.<index>…`). Inserting
an effect into a list SHIFTS every sibling's key and silently re-attaches old
translations to the wrong lines. When editing effect lists on a localized
campaign, APPEND rather than insert where order allows, and after any structural
edit re-check every shifted key's translation against its new English source —
exact-key coverage (`DW0180`/`DW0181`) cannot see a stale value.

## Reference: tools by symptom

Two classes, one rule each:

- **Workflow tools are steps, not options.** Where a step above says "always" or
  "mandatory", skipping it is skipping validation.
- **Human-in-the-loop tools are offered, never required.** When the flow reaches
  the marked point, say in one line that the tool exists and what it would catch
  — then keep going. Never block or wait on a use/don't-use answer.

The full inventory — every binary, script and flag that exists today — is
`docs/reference/tools.md`. Check it before assuming a capability is missing.

- **Judging any visual outcome** (cutscene framing, set dressing, terrain):
  `delvec snapshot` (`--camera x,y,z,yaw,pitch`, `--at <anchor> --dist`,
  `--shot <render-plan id>`, `--labels`) and `delvec blocking-chart` (per-floor
  cutaways). *Always* for cutscenes: render start, mid and end of every dolly
  segment and **look at the frames** before calling the step done — `DW0308`
  proves the path is air, not that the shot is pointed at the subject, and an
  inside-out cinematic can be fully DW-green.
- **Terrain or visual fixes beyond swapping prefabs**: `delvec edit` — the map
  editor loop (edit script batch → replay → snapshot). The script is a campaign
  document, `world-edits.json`: batches of declared edits, replayed
  deterministically, with the post-batch invariants enforced. Never hand-patch
  `.nbt` or invent block edits outside it.
- **Handing a build to a playtester**: the playtest note flow — `/trigger dw.note`
  in-game, then `delve-harvest` → `playtest-report.json`. Human-optional.
- **Delivering or revising a cutscene**: shot calibration — in-game
  `/trigger dw.mark set <s>` (stand where the camera should be), `dw.aim set <s>`
  (look at the subject), `dw.faster`/`dw.slower set <s>`, then `/trigger dw.done`
  once; `delve-harvest` writes `rehearsal-report.json` and `delvec calibrate
  <report> --layout <out>/creator-datapack/layout.json` turns it into an
  anchor+offset patch you apply and rebuild. Human-optional. (Beat replay —
  `dw.beat` / `dw.shot` / `dw.free` — does not exist; do not promise it.)
- **A prefab library needing human taste, not machine checks**: `delve-admit
  gallery` (browse world) → a reviewer walks it and leaves notes → `delve-admit
  curate` / `curate-merge` fold them into the catalog cards. Human-optional.
- **Several candidate prefabs for one slot, and a human has to pick**: `delvec
  contact-sheet <renders> -o <png>` — all the candidates on one page, each
  labelled with its rank and id, with `tools/refscore.py` optionally ordering the
  page by similarity to the design gate's reference image. Human-optional. Say
  plainly that the score only orders the page: every candidate is on it, and the
  low scorer is present, last — the human is the selector, the number is not.
- **A picture cannot say what a prefab is like to be inside**: `delvec viewer
  <nbt|dir|manifest.json> -o <page.html>` — one self-contained HTML page with a
  camera the reviewer drives: exterior, plan, a player point of view at eye
  height (1.62) standing at every declared anchor and doorway, plus a cutaway
  slider that takes the roof off. A zone that ships as several tiles and a
  manifest shows as one building. Pass a directory to put a whole library on one
  page. Human-optional; read its fidelity list first (step 12).
- **A picture of the whole map** (storybook hero image, release asset): `delvec
  panorama <build-dir> -o <dir> [--bearing se|sw|ne|nw] [--spp N]` — a 45° oblique
  scene framing the entire layout, computed from the plan. Never hand-edit a scene
  JSON to get one.
- **An NPC needs a look no vanilla mob gives you**: the skin toolchain,
  `PYTHONPATH=tools/skin .venv-skin/bin/python -m delve_skin all <cast.json>
  --skins-dir … --preview-dir … --catalog-dir …`, in the venv from Init step 6. **Look at the previews**, and
  always set `model` (`wide`/`slim`) — an omitted model renders slim and distorts
  a wide skin. The compiler bakes the PNG into the delve's resource pack from
  `campaigns/<id>/skins/`.
- **Cleaning up a ladder or a play session by hand**: `validation/fresh-volumes.sh
  --project <id>`. `--project` is required everywhere and there is no daemon-wide
  mode. It reclaims what the project owns — containers, volumes and networks —
  and proves it. The `--profile play` stack from step 9 pins a fixed container
  name, so tear that one down with `docker compose … down -v` or
  `tools/playtest-server.sh down` rather than with `fresh-volumes.sh`.

## Reference: when something goes red

The symptoms most likely to stop you, and what they actually mean.

**`internal error: cannot read campaign dir: <name>.json` (exit 10).** Not a
compiler fault — a document is missing. `delvec validate` is whole-campaign and
needs all six required documents to exist; stub the ones you have not written
yet. The names are in step 1.

**`DW0311`: "the player cannot walk from [x] to [y] … no collision-free path",
and the two points are hundreds of blocks apart.** They are in different areas,
and the crossing between them was never emitted. The message's own suggestions —
a wedged doorway seam, a void gap, a fence ring, a `close-gate` to reopen — are
about the same-area case and will not apply. Read step 2A: **the destination
area's piece must declare an entry point**, and only 5 of 36 shipped prefabs do.
Bind that area to one of them, or make a piece that has one.

**The build is green, PackTest is green, and the bot fails its FIRST step with
"No path to the goal!"** The party's first objective is in an area they did not spawn in,
and no crossing carries them there — the delve is not completable and nothing
before the bot said so. Read step 2A rule 1: put a beat in the spawn area first,
or move the spawn. Confirm by reading `critical-path.json` at the build output's
root: the first `reach`/`talk-to` step should be at coordinates inside the spawn
area, and a step that crosses areas carries a `transport` key.

**`DW0438`: a `collect` container "holds `minecraft:air`, not a container".**
The anchor you pointed at is not standing on a chest or barrel — an anchor named
`anchor/chest` is not evidence there is one, and two shipped pieces declare
exactly that over air. Read *Reference: what a quest can do* → *Items,
containers and loot*: use `dropped_by`, or drop the `container` field and let the
compiler place one, or make a piece that carries the container. Do not point the
field at a different anchor and hope.

**`DW0857`: an anchor "is provided by 2 of this campaign's areas".** You bound
one prefab to two areas. The remedy available to you is **a different piece for
one of them** — not renaming an anchor, which belongs to the shared library and
would change it for every campaign.

**A bot step times out with "objective … did not complete".** Read the rest of
that line before touching the campaign. The bot reports the server's own answer
to the `/trigger` it sent: *the server ANSWERED …* means the trigger reached the
delve and a datapack guard consumed it — a re-used world whose scoreboard already
carries the objective does exactly this, so run `fresh-volumes.sh --project <id>`
and re-run before believing the content is at fault. *The server never
answered …* means the command never got there and the failure is the harness's,
not the delve's.

**`failed to read dockerfile: open Dockerfile.delve` from a ladder script.** The
build tree is outside `validation/`. Build into `validation/delve-output`, or
copy the tree there — see step 8.

**`refimg: no delvewright.local.toml` (exit 2).** Init step 5, path B. It names
the file, the section, and where the template is. If the campaign already has an
approved `design/` directory you are on path A and do not need this at all.

**The staging gate refuses with a long UNBOUND list.** Not a defect count — see
step 9. Read it item by item into the round summary; override deliberately if
the session needs a red build.

**`delvec metrics` "prints 333 lines of JSON, not a table".** It prints the
table as JSON on stdout and its human summary and binding counts on stderr.
Redirect them separately: `delvec metrics > table.json`.

**A red that came from the toolchain rather than the campaign.** Stop content
work and report it, with evidence. Never hand-edit compiler output, never
restructure the campaign to dodge it, never weaken a check or reroll a seed to
get green. Escalating is success.

## Reference: authoring pitfalls

- **Difficulty is declarable** (`world.difficulty`: `easy`/`normal`/`hard`).
  Absent, the compiler derives `easy` for a wave campaign — which HALVES the
  damage players take (`min(dmg/2+1, dmg)`), the setting behind "the enemies are
  too weak". A souls-style brief almost certainly wants `normal` or `hard`; when
  you change it, retune the combat arithmetic (mob `attributes`, class gear, wave
  sizes) rather than only flipping the keyword. `peaceful` is rejected
  (`DW0468`) — it deletes every hostile. Scripted `actors` take the same
  `attributes` block wave mobs do, so an elite can be tuned on both its staged
  puppet and its unleashed twin.
- **The machine proves the LOOP, not the win.** Three things are checked about
  every mandatory encounter, and it is worth authoring toward them rather than
  discovering them as red builds:
  1. *Winnability arithmetic* (`DW0470`–`DW0473`): a required hostile must be
     damageable (Resistance amplifier 4 is total immunity — use at most 3, or put
     the durability in `attributes.max_health`), must have a standable cell beside
     it to be fought from, must fall inside the time-to-kill budget, and no
     `damage-players` in a quest bundle may land ≥ 20 (a full-health player) —
     that is a scripted death, not difficulty. A hit the party can dodge (a trap
     payload, a stealth `on_caught`, a `damage-players` with a `within` zone) is
     deliberately outside the check.
  2. **Declare `attributes.max_health` on every mandatory wave stack.** Vanilla
     publishes no per-entity default attributes, so an undeclared stack gets no
     numeric bound at all and the build warns `DW0475`.
  3. *The die-retry ladder stage*: the bot rests at every bonfire on the path (a
     fire only ARMS on arrival — the respawn point moves when the party RESTS),
     then deliberately dies twice at every encounter and proves respawn at the
     governing checkpoint → the route back → the encounter is still finishable →
     no completed objective was lost. Author with that in mind: every encounter
     needs a checkpoint or bonfire that governs it, and a wave the party must be
     able to re-fight wants `respawns_on_rest`. Leaving it off is legitimate — a
     won fight stays won, and the stage records that as `cleared-before-retry` and
     passes it. What it reds is `stranded`: nothing left to fight AND the
     objective unfinished, so the party can neither complete the encounter nor
     re-fight it. Turning `respawns_on_rest` ON buys a stricter check: the wave
     must come back WHOLE — declared count, all-new mobs, full health — because a
     retry must never let the party grind a fight down one swing per death.
  4. *The inverted floor gate*: mark a set-piece fight `tier: "elite"` or
     `"boss"` — on the **wave** or on the **actor**, same three keywords. The
     ladder then gives it one UNASSISTED bot attempt; if the bot — a poor fencer
     by design — wins cold, the run reports the fight as too easy for its billing.
     Leave ordinary pressure waves unmarked: they carry no such expectation.
     Marking is how you opt into the scrutiny, so mark honestly. **Mark the actor
     when the elite IS an actor** — the kneeling armoured thing that stands up
     when struck is a `spawn-actor` + `unleash-actor` beat, not a wave, and an
     unmarked one is a boss no proof ever looks at.
  5. *A tier the gate cannot measure is said out loud, not swallowed*: the gate
     warns on a first-try win and is silent otherwise, so an encounter nobody
     fought would look exactly like one that was fought and lost. The compiler
     therefore warns `DW0477` — and records `floor-gate: not covered (reason)` in
     `validation/combat-plan.json` — for a tiered actor no `unleash-actor` beat
     ever wakes (an `Invulnerable` puppet is scenery; a `vulnerable` one is `NoAI`
     and never swings back), and for a tiered wave no critical-path `kill`
     objective names. If you meant it as a fight, add the unleash or the `kill`
     objective; if you meant it as set dressing, drop the tier.
  Ordinary fights run the ladder under a bounded, logged combat assist, so bot
  fencing skill never caps how hard the delve is allowed to be.
- **Bonfires owe the party a flask.** Right-clicking a `bonfire` opens exactly two
  options — *rest and save* (full restore: health, hunger, negative effects
  cleared, flask refilled, checkpoint moved, `respawns_on_rest` waves re-seated,
  `on_rest[]` fired) and *save only* (the checkpoint, nothing else). The
  replenished item is a class-kit entry marked `"flask": true`, and **every class
  kit in a campaign that places a bonfire must declare one** — a bonfire campaign
  with a flaskless kit is the build error `DW0476`. Author it as a real recovery
  consumable with the per-rest budget you tuned against as its `count`: resting
  sets the stack back to exactly that number, up or down, so the flask is a budget
  and never a stockpile.

  **A potion must say what is in it.** A `minecraft:potion` (or splash/lingering
  potion, or tipped arrow) with no `contents` is vanilla's *Uncraftable Potion* —
  it heals nothing however you name it — so declaring one is the build error
  `DW0487`. Either name a vanilla brew or list the effects:

  ```json
  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": { "potion": "minecraft:strong_healing" } }

  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": {
      "effects": [
        { "effect": "minecraft:instant_health", "amplifier": 1 },
        { "effect": "minecraft:regeneration", "duration": 200, "amplifier": 0 }
      ],
      "color": "#ff9c30"
    } }
  ```

  `potion` is a 1.21.11 potion id, where strength and duration are part of the id
  (`minecraft:strong_healing`, `minecraft:long_night_vision`) rather than separate
  fields. `duration` is in **ticks** (20 = one second) and is required for every
  lasting effect — and forbidden on the instantaneous ones
  (`instant_health`/`instant_damage`), which land once on drinking. `amplifier` is
  0 = level I. Anything vanilla cannot pour is `DW0486`. The bonfire's three
  dialog strings default to canonical English; author `prompt`/`rest_label`/
  `save_label` only when the fiction wants its own words, and keep the two labels
  button captions (`DW0331`).
- **Place a bonfire OUT of every hostile's reach.** A rest point is where the
  party respawns and where every `respawns_on_rest` wave is put back on its feet,
  so a fire inside a hostile's `follow_range` delivers the party straight into
  combat on arrival — the build error `DW0478`. The clearance is measured against
  where the force actually IS: a wave's seated spawn cells, and for a `lane` wave
  the whole marched polyline (a lane wave walks its corridor while you are
  elsewhere, so a fire beside the far end of a lane is a fire in the lane).
  Fighting actors — anything `unleash-actor`ed, or staged `vulnerable` — count
  too, at their staging anchor. Put fires in side rooms, past the threshold, or
  beyond the end of the lane; never buy the clearance by shrinking `follow_range`,
  which retunes the fight to hide the placement. A re-seated wave always comes
  back **stationed** — a lane wave at its lane start, a plain wave at its anchor —
  so the safe zone stays true across every rest and every death.
- **Wave tuning**: `follow_range` below ~16 means distant wave mobs never engage;
  a kill objective whose mobs idle is unfinishable-in-practice even though
  machine-valid. Undead waves burn in daylight — the ONLY sanctioned fix is a
  helmet on the mob: `equipment.head`, any head item, on every burning stack the
  party is asked to fight. **Never `set-time`**; the delve's hour is a pacing
  decision, and moving it to save a mob spends a beat. The compiler enforces this
  (`DW0496`): a species in vanilla's `#minecraft:burn_in_daylight` staged for a
  `kill`-adjudicated fight whose walkable ground reaches open sky, under a pinned
  clear daytime hour, with an empty head slot, is a build error naming the sunlit
  cell. Roofing the arena clears it too. One species the helmet does not save — a
  phantom burns through it — so an open-air phantom fight has to be roofed or
  restaged. Never route wave mobs like actors: waves are native AI; if a beat
  needs lane-then-fight movement, that is the routed-then-feral primitive, which
  does not exist — not a `follow_range` trick.
- **Open-air by default**: stage scenes in the open unless a beat NEEDS enclosure
  (a cave passage, an interior puzzle, a reveal). The horizon — surround terrain,
  sky, backdrop — is part of the composition; a campaign of enclosed boxes wastes
  it. When an enclosed beat is necessary, prefer routing the player back into the
  open between beats over chaining interiors.

## Playtest rounds

Generation is round 1. Everything after it is an iteration round against the
playtester's findings, and the playtest hour is the scarcest resource in the
pipeline. Full derivation: `docs/reference/playtest-methodology.md`. Mandatory
here:

1. **Keep a findings ledger in `GENERATION.md`** — one row per finding: number,
   its wording as reported, the round it was reported, status. Status is
   `fixed@rN`, `open`, `engine` (blocked on a capability gap), or `ruled` (closed
   with no code change). This table is the campaign's memory; a finding that lives
   only in chat is a finding that will be reported to you twice.
2. **Triage every finding the day it arrives**, as *content* or *capability gap*.
   A capability gap — the DSL has no way to express what was asked for — is never
   patched downstream and is therefore a **staging blocker**: either the engine
   work lands before the next playtest, or the round summary says, per item, that
   it is still open and not to test it. A finding that survives more than one
   round is a capability gap; staging a build while those rows are open is what
   makes a playtester meet the same defect twice.
3. **Close each finding twice: the instance, and the general form.** After fixing
   the instance, ask what rule it is an instance *of*, and file that rule as a
   diagnostic (the DW code is minted for you — never mint one yourself). When the
   diagnostic exists, **re-run it against the current build**: that sweep is the
   deliverable, not the code, and it routinely finds a second live instance the
   moment it lands. Where no diagnostic is possible, write that down; it becomes a
   risk item at the next staging review.
4. **Append every finding to the engine repository's `docs/playtest-findings.json`**,
   the same day, with its general form and the check that carries it — this is the
   cross-campaign ledger, and `GENERATION.md`'s table is the per-campaign view of
   it. A finding recorded only in the campaign is a finding the NEXT campaign
   learns nothing from.
5. **Audit the FULL ledger from round 1 before staging any build** — never from
   the last round, and never by reading. You do not have to remember to: the
   staging paths REQUIRE it (step 9). Run it yourself first so the red list is in
   the round summary before anyone is invited.
6. **Pre-flight, in this order, before the invitation**: full ladder green
   (PackTest → bot critical path + die-retry → every branch run) → staging gate →
   localized builds and a double-build that is byte-identical → server boots and
   self-checks → then invite. Not "the build compiled, come look".
7. **Update `DESIGN.md` in the same round and run its conformance review.** A
   design record left unupdated across rounds accumulates changes nobody asked
   for, and the audit that catches up finds them all at once.
8. **Close the round in `GENERATION.md` with its machine record**, not just prose:
   how many validation-loop iterations it took to reach green, and every DW code
   the round hit **with its count** (`DW0205 x3, DW0483 x3, DW0450 x1`). Write it
   even when the count is zero — a round that hit nothing is the datum that says
   the gates had nothing to say. It is the only source from which rounds-to-green
   can be read afterwards; a round summarised in prose alone is a round whose cost
   cannot be recovered.

## Hard rules

- Persist the campaign documents before validation, not after — a crash must
  never lose the campaign.
- **Apply a ruling at the scope it was given.** If a wider rule seems right,
  propose it in one line and wait — generalizing a ruling is a design decision,
  not an inference to make silently. A one-beat pacing ruling read as a
  campaign-wide ceiling is a campaign nobody approved.
- **Unrequested change is a rejection cause on its own**, independent of whether
  the change is good. Author what the round asked for; anything else you believe
  the campaign needs is a proposal in the round summary.
- Every player-visible string in the campaign documents stays **English** —
  always. Other languages are sidecars. A brief written in Chinese still yields
  English documents; add a `zh-cn` sidecar only when localized in-game text is
  asked for.
- **Commit only canonically formatted JSON.** The last thing before any `git add`
  of a document or sidecar is `delvec fmt <campaign-dir>`; CI runs
  `delvec fmt --check`. A diff that rewrites a file nobody edited is the defect
  this closes.
- Homages: original text only. Cultural reference, never asset ingestion.
- If a mechanic the brief wants has no DSL verb, do NOT fake it with adjacent
  verbs silently — say what is missing and offer the closest authorable
  alternative. A change to the language itself is not made from inside a campaign.
- Never weaken a check, a test or a threshold to get green, and never reroll a
  seed. A red check is information. Fix the cause or escalate; escalating is
  success.
