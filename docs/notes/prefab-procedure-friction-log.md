# Friction log — walking the prefab procedure as a person

Written live, as it happened. Instrument: engine worktree at
`dd287282723cfd34bec87ca181afae4ab55ce4f0`, branch
`docs/the-prefab-procedure-friction-log`.

Stance: a person who has this repository, `.claude/skills/new-delve/SKILL.md`,
`docs/reference/prefab-procedure.md`, and nothing else. Subject: the smallest
plausible missing piece.

(entries appended below as they happen)

---
## Init (SKILL.md "Init", steps 1–2 and the finish checklist)

Ran every line of the Init checklist. All of it worked, first try, as written:
the symlink check, `cargo build --release --workspace` (exit 0, 38s),
`cargo build --release --manifest-path crates/render/Cargo.toml` (exit 0, 1m17s),
the `export PATH` line, `delvec --version` (`delvec 1.1.0, dsl 0.18.0, mc
1.21.11`), `delve-render fidelity-gate` (`PASSED`, exit 0), `delve-grammar list`
(36 programs), `command -v delve-admit`, and the client-jar ladder via
`delvec palette campaigns/prefabs/hello-room.nbt`.

**No friction.** Recorded because it is the control: the pages were followed
literally and the toolchain came up. Everything below happens on a working
install, so nothing below is an install problem.

One triviality, logged and not a finding: `delve-grammar list | head -50`
ends in

```
thread 'main' (22702219) panicked at .../stdio.rs:1166:9:
failed printing to stdout: Broken pipe (os error 32)
```

That is Rust's SIGPIPE default, and `list` still exits 0. A person paging a
36-entry list would see a panic trace and might think the tool crashed.
Severity: **mild friction**.

---

## §1 — Fix the scene description first

**What the page says** (`prefab-procedure.md` §1):

> One or two sentences, written down before any tool runs, naming: what a body
> does in the space (walks through / drops into / fights in / is watched from),
> the material feeling, and any element the campaign will need to attach to.

and

> **Fix the region in the same breath** … Record the chosen region and seed
> beside the program — in the campaign's `GENERATION.md`, since the program JSON
> does not yet carry its own region (queued engine surface).

**What I did.** Picked the dullest plausibly-absent piece. `delve-grammar list`
has `stair-flight`, `store-room`, `tee-passage`, `threshold-motif` — but no
corner. Subject:

> A plain corridor that turns one right angle. A body walks in at the north end,
> turns once, and walks out of the west face. Dry-laid stone, undressed, plain
> floor and plain ceiling, no light. The campaign attaches nothing but the two
> ends.

Region fixed in the same breath: **9×5×9** — a 3-wide walkway with a course of
wall each side, two legs of 9, headroom 3 under a 1-course ceiling on a 1-course
floor.

**Friction: one, small.** The page says to record the region and seed "in the
campaign's `GENERATION.md`". This whole section of the skill is titled *"when
the library has no piece you need"* — the piece may well be a **library** piece
with no campaign yet, and then there is no `GENERATION.md` to write into and the
page names no other home. A person would either invent a file or skip the
record; skipping is silent, and the region is exactly the fact the program JSON
does not carry.

*Would have unstuck them:* one clause naming where the record goes when the
piece is not a campaign zone.
Severity: **mild friction**.

---
## §2 — Choose the palette by measurement

**What the page says** (§2): three steps, "Do not stop after the first" —
screen, measure the mix, then look at the sheet.

**What happened.** All three ran first try and the page's own worked example
reproduced exactly:

```
$ python3 tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \
      --where 'L<=0.95' --where 'C_mean<0.02' --where 'texture_range<=0.30'
all candidate blocks   1146
...
14 survivor(s).
```

The page claims "The example above takes 1146 blocks to 14." It does.

My own screen (`L>=0.35 … L<=0.62`, neutral, quiet, `not gravity`) → 48
survivors. Mix measured:

```
$ python3 tools/block-appearance.py --mix 'cobblestone=6,stone=3,cobbled_deepslate=1'
binding: 1 of 1 declared paint(s) examined, 1 mix(es) with >= 2 members
   60.0%  minecraft:cobblestone …
   10.0%  minecraft:cobbled_deepslate …
  loudest_member minecraft:cobbled_deepslate at 10% of area
```

`--sheet --seed 7` wrote `.sheets/palette/swatches.png`; I opened it and the mix
tile reads as damp dry-laid cobble, which is the scene. Under a second, no GPU.

**No friction at all.** This is the strongest stretch of the procedure walked in
this round: the instructions are executable verbatim, the numbers in the prose
match the numbers the tool prints, and the "now LOOK at it" line is printed by
the tool itself rather than left to the page.

---
## §3 — Author the program as JSON

**What the page says** (§3): read the idiom index first (`grammar.md` §2c), then
`delve-grammar show --program <nearest> > my-piece.json`, edit, and
`delve-grammar check --file my-piece.json` after every edit.

**What happened.** `delve-grammar show --program tee-passage` printed a 365-line
runnable program. I read `grammar.md` §2b (`mark`), §2c (idioms + the split-order
section) and §2 (the paint / weighted-list form) — all three named by the page —
and wrote a four-rule program: one `z` split into approach band / turn band /
solid, each band an `x` split, one shared `lane_column` doing the `y` split.

```
$ delve-grammar check --file corner-passage.json
corner-passage: ok — 4 rule(s), 2 param(s), 1 role(s). Structure only: expand it
to learn whether it fits a region.
```

Green first try. **No friction.** The split-order section of §2c
("the last axis you split is the only axis on which two things are guaranteed to
meet") is the single most useful paragraph in the whole reference for a person
writing a first program, and it is where it needs to be.

One honest note about my own conduct: my two legs' lanes line up only because
`[relative 1, absolute 3, relative 1]` and `[relative 1, absolute 3]` both put
the lane at x 3..5 **at this region**. That is a computed constant, which §2c
says is exactly the thing nothing checks. The page warned me and I did it anyway
because it is the obvious way to write an L. Worth knowing that the warning does
not prevent the mistake it names.

---

## §4 — Expand, and let the machine judge — **THE STOP**

**What the page says** (§4, and SKILL.md step 4 in the same words):

> `--traversable` is opt-in because it is a claim about a *kind* of piece … **Pass
> it whenever the piece is a passage, a stair or a route** — that is most of them,
> and a route nobody proved walkable is the defect the gate exists for.

My piece is a passage. I passed it, with `--reachable-floor` as SKILL.md's own
command line shows.

**What happened** — command and output, verbatim:

```
$ delve-grammar expand --file corner-passage.json --region 9x5x9 --seed 1 \
      --traversable --reachable-floor --id corner-passage -o out/
corner-passage: fail
  blocks-exist    pass  bound 4 …
  shape-complete  pass  bound 4 …
  states-complete pass  bound 4 …
  oriented-fills  pass  bound 8 …
  non-empty       pass  bound 405    324 filled cell(s) of 405 in the region
  traversable     FAIL  bound 3      0 standable cell(s) at the approach end, 3 at
                  the exit end; walking does NOT connect them. This piece declares
                  no spatial contract, so the binding count is standable CELLS on
                  two faces of the region, not declared ways in — declare exterior
                  edges and it counts doors
  reachable-floor pass  bound 27     27 standable cell(s) under a roof; 0 of them
                  have no walking route from the 6 grade entry cell(s)
  …
  reachability   27 of 27 standable cell(s) reachable on foot from 6 grade entry
                 cell(s) (100.0%) · 27 sheltered · unreachable 0 …
  finding: this piece declares no spatial contract …
error: corner-passage: a machine gate went red; no prefab was written.
expand_exit=4
```

**Read those two lines together.** `reachability` says **100% of the floor is
reachable from 6 grade entry cells** — my corridor is completely, provably
walkable end to end, and the always-on measurement says so in the same report.
`traversable` says it is not, and a red gate writes **no `.nbt`**. Exit 4,
nothing on disk.

The reason is that `traversable` is *a claim about two opposite faces of the
region box*. My two ends are on **perpendicular** faces — the north face and the
west face — because the piece turns a corner. That is what a corner passage IS.
The gate picked an axis, found the far face solid, and failed.

**What a person concludes.** Not "my piece is wrong" — the report told them the
floor is 100% reachable one line below. They conclude **the engine is broken**,
or that they have misunderstood what a passage is. And the page gave them no
warning: §4 lists exactly one piece to leave a flag off for ("The one piece to
leave it off: a one-way descent", for `--reachable-floor`), which reads as an
exhaustive list of the exceptions. There is no such note for `--traversable`,
and the instruction is the unqualified "**pass it whenever the piece is a
passage**".

**Severity: stops her dead.** Exit 4, no `.nbt` written, on the smallest and
dullest piece anyone would ever make, following the page literally.

**What would have unstuck them:** one sentence in §4 saying `traversable` is a
claim about two *opposite* faces, so a piece whose ends are on perpendicular
faces cannot satisfy it without a spatial contract. That sentence does not exist
on either page.

*(Continued below: I follow the diagnostic's own prescription rather than
skipping to what I know.)*

---
### §4 continued — following the diagnostic's prescription literally

The `traversable` failure prescribes, in its own words:

> declare exterior edges and it counts doors

So I went and declared exterior edges. Two things about that instruction:

**(a) Neither page tells you how, or that this is a thing you do.**
`prefab-procedure.md` §3 — "Author the program as JSON", the step where you would
write one — never mentions `claim`, `contract`, or the spatial contract at all.
The contract appears on that page only at §7 (as something `delve-admit audit`
checks) and §9 (as a metadata field a *reader* consumes). SKILL.md's whole
"when the library has no piece you need" section never mentions it either. The
only authoring documentation is `grammar.md` §2d, which the procedure page names
once, at the top, as a general "behaviour reference".

A person following the two pages has, at this point, a red gate, a prescription
they cannot act on, and no pointer. **Severity: stops her dead** — this is the
same wall as the stop above, and it is the wall she is standing at *after* she
does what the message said.

**(b) "declare exterior edges" is not a small thing.** `grammar.md` §2d:

> It runs whenever a piece declares a contract; there is no flag.

Declaring one edge turns on **nine** gates — well-formed, coverage, closure,
edge-proof, no-body, reachability, anchors, exterior-faces, no-body-majority —
each with its own obligations (every standable cell in a declared element, every
boundary cell of an `enclosed` space non-passable except a claimed opening, every
anchor resolving, and so on). The prescription reads like "add two lines"; what
it actually asks is "adopt the spatial-contract subsystem".

**What I wrote.** One `claim` node on the shared `lane_column`'s void child, plus:

```json
"contract": {
  "entry": "passage",
  "spaces": { "passage": { "envelope": "enclosed" } },
  "edges": [ { "a": "exterior", "b": "passage", "class": "walk" },
             { "a": "passage",  "b": "exterior", "class": "walk" } ]
}
```

**It passed, first try, all nine gates green:**

```
corner-passage: pass
  traversable     pass  bound 4  4 declared way(s) in or out — west walk, west
                  walk, north walk, north walk — and a walk connects every pair
  contract-well-formed pass  bound 3
  contract-coverage    pass  bound 27
  contract-closure     pass  bound 123
  contract-reachability pass bound 27
  contract-anchors      pass bound 2
  contract-exterior-faces pass bound 2   2 exterior edge(s) export 4 face(s)
  contract-no-body-majority pass bound 27
out//corner-passage.nbt + corner-passage.json + corner-passage.report.json
expand_exit=0
```

That is a genuinely good outcome once you know to do it. **The whole cost of
this stop is documentation, not engine.** Two sentences in §3 — "a piece whose
ends are not on opposite faces declares a spatial contract; see `grammar.md` §2d"
— would have removed it entirely.

### A defect found on the way: two exterior edges export duplicate faces

Not what I was looking for; found because the report printed its list.

`grammar.md` §2d's own worked example declares one exterior edge per door:

```json
{ "a": "exterior", "b": "near", "class": "walk" },
…
{ "a": "far", "b": "exterior", "class": "walk" }
```

I copied that shape — two exterior edges, one per end. The report:

```
contract-exterior-faces pass  bound 2   2 exterior edge(s) export 4 face(s):
                                        west walk, west walk, north walk, north walk
```

Each edge exported **both** faces, so the shipped `<id>.json` carries four
entries in `spatial_contract.faces`, in two byte-identical pairs:

```json
{ "space": "passage", "class": "walk", "dir": "west",
  "opening": { "from": [0,1,3], "to": [0,3,5] } },
{ "space": "passage", "class": "walk", "dir": "west",
  "opening": { "from": [0,1,3], "to": [0,3,5] } },   <- identical
```

Cut back to **one** exterior edge (probe, same region and seed): also `pass`,
`2 face(s): west walk, north walk`, no duplicates.

So the edge declaration is not the authority on which face — the checker derives
faces from the geometry — and the edge *count* multiplies the export. One edge
and two edges are both green and produce different artifacts.
`docs/reference/prefab-procedure.md` §9 names `spatial_contract.faces` as
something `delvec` consumes.

**What a person concludes:** nothing. Every gate is green, so they ship it. This
is the silent-wrong-artifact direction.
*Would have unstuck them:* nothing on a page — this one wants a gate, or the
declaration wanting a `face`/`dir` so an author says which end they mean.
**Severity: costs an hour** — later, to whoever finds it downstream.

---
## §5 — See it before believing it

**What the page says** (§5): `delve-render piece out/<id>.nbt -o shots/ --size
640`, then

> **Open the `eye-<anchor>.png` frames FIRST.** … This is the shot that shows the
> doorway's shape and proportion … **Read these first.**

**What happened.** The command ran, 9 shots, one warning:

```
DW0727 [warning] corner-passage/eye-west_end: the eye shot for `anchor/west-end`
is an EMPTY frame (1 distinct colour(s)) — a body standing at [0, 1, 4] and
looking west sees nothing but flat background. … If this anchor is meant to face
outward (an approach, a threshold), what it is about lives in the assembled
world … the fix is the anchor or the geometry, never the camera
rendered 9 shot(s) … (2 eye-level shot(s) over 2 anchor(s), 2 of them eye-eligible)
```

That diagnostic is excellent — it names the cause, names the legitimate case,
and forecloses the wrong fix. The `<id>-shots.json` manifest is likewise exactly
what the page promises (`clearance_open_cells: 5, clearance_stopped_by:
minecraft:stone` on the north eye).

**But: not one of the nine shots shows that this passage turns a corner.**

- `eye-north_end` — a corridor looking at the far wall. Correct, uninformative.
- `eye-west_end` — empty frame (correctly reported).
- `ext-ne/se/sw/nw` — four grey slabs. The page warns about exactly this
  ("eleven pictures of the same grey slab, and none of them can tell you the
  piece has a corridor in it"), and it is right.
- `top` (`"cutaway": true`) and `anchor-*` — featureless rock from above.

I could not confirm from any picture that the piece was the L I designed. **What
actually confirmed it was the §4 contract report**, which printed the openings as
cell ranges:

```
contract: exterior face: walk on the west side, via space "passage", 9 cell(s)
… "opening": { "from": [0,1,3], "to": [0,3,5] }   (west)
… "opening": { "from": [3,1,0], "to": [5,3,0] }   (north)
```

**A measurement, not a look.** The step named "see it before believing it" was
settled by reading numbers.

**The page's own remedy, tried.** §5 offers `--view` for "a building whose
identity is one elevation". A corner passage's identity is its **plan**, not an
elevation, so nothing on the page points a person here — but I tried it anyway:

- `--view name=west-face,face=west` → **this works.** A square-on 3×3 doorway
  with the north leg visible as a dark slot inside it. The first picture in the
  whole set that shows anything.
- `--view name=from-above,face=up` → a flat 9×9 roof. `face=up` is not a plan;
  a person reaching for "let me look down at it" gets the roof again.

**What a person concludes.** They would either ship a piece they never actually
saw, or spend the time I spent hunting for a camera. Neither page tells them
`--view name=…,face=<the open face>` is the shot for a passage — §5 attaches
`--view` to elevations and facades.

*Would have unstuck them:* one clause in §5 — "a piece whose identity is its
plan (a passage, a junction, a stair) has no planned camera that shows it
either; aim a `--view` at each open face." And, separately: there is no cutaway
plan camera in the set, though `top` is marked `"cutaway": true` in the manifest.

**Severity: costs an hour** — and it is the *quiet* kind: every gate was green,
so a person in a hurry ships without ever seeing the piece.

---

## §6 — What the grammar cannot do

Nothing to run; read it. Accurate about my piece: no block entities, no
connectors, no light, and my region was well under the 48-per-axis cap. **No
friction.**

---
## §7 — Admit it

### 7.1 The page's own first admission command is refused

**What the page says**, §7, first line of the code block, comment included:

```sh
delve-admit audit    out/<id>.json         # or out/<id>.nbt — audit takes either
```

**What happened:**

```
$ delve-admit audit out/corner-passage.json
DW0732 [error] out/corner-passage.json is a single-template prefab's metadata,
not a tile-set manifest — pass the `.nbt` beside it
audit_exit=2
```

The comment says "audit takes either". It does not. `audit` takes the `.nbt`
for a single template and the manifest for a tile set — which the page says
correctly three paragraphs later, and which SKILL.md step 6 also says
correctly. Only the code block is wrong, and the code block is the part people
copy.

*What a person concludes:* the diagnostic is good enough that they do the right
thing. But the very first command of the admission step, copied from the page,
errors.
*Would have unstuck them:* delete the comment, or make it
`# a tile set passes out/<id>.json instead`.
**Severity: mild friction** — loud, prescriptive, exit 2. Recovers in seconds.

Following the diagnostic: `delve-admit audit out/corner-passage.nbt` → `pass`,
`contract.state: judged`, `failed_gates: 0`, 7 obligations over 211 objects.

### 7.2 `--pos` for a socket: the page gives no rule, and a wrong one is silent

**What the page says**, in full, about where a socket goes:

```sh
delve-admit socket   out/<id>.nbt --pos X,Y,Z --facing <dir> --opening 3,3 \
                     --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
```

That is the whole guidance. There is no sentence anywhere on either page about
what cell `--pos` should name relative to the opening, what `--name` /
`--target` / `--pool` mean, or how a target string comes to match anything.

I guessed the centre of each doorway, from the openings the §4 contract report
printed: `--pos 0,1,4 --facing west` and `--pos 4,1,0 --facing north`. Both
carved, exit 0.

**What that silently did.** Re-running `audit` afterwards:

```
before:  contract: exterior face: walk on the west side, via space "passage", 9 cell(s)
after:   contract: exterior face: walk on the west side, via space "passage", 8 cell(s)
```

and the audited palette gained a member:

```
palette: ['minecraft:air', 'minecraft:cobbled_deepslate', 'minecraft:cobblestone',
          'minecraft:jigsaw', 'minecraft:stone']
```

The socket is a `minecraft:jigsaw` block standing in the cell a body walks
through, and the piece's own declared doorway lost a cell. Verdict: `pass`. No
warning, no finding.

**What I ESTABLISHED:** the face count fell from 9 to 8, `minecraft:jigsaw` is in
the shipped palette at the doorway cell, and `audit` says `pass`.
**What I did NOT establish:** whether this is wrong. `delve-admit` has a
`resolve-jigsaw` subcommand and §0 says "the compiler is the jigsaw", so vanilla
may well substitute it at placement. I could not decide that from the two pages.

*What a person concludes:* nothing — they cannot tell either, and the tool says
`pass`. They pick a `--pos` by feel.
*Would have unstuck them:* one sentence in §7 saying where a socket cell sits
relative to a declared opening, and what happens to that cell at placement.
**Severity: costs an hour**, and it is the silent direction.

### 7.3 `lighting --write` — the best step on the page

```
$ delve-admit lighting out/corner-passage.nbt --write
  "binding": { "entry_cells": 6, "measured_cells": 27, "standable_cells": 27 },
  "profile": "dark", "measured_min_light": 0, "min_light_daylight": 9,
DW0751 [warning] dark interior at sky light 4 (a clear night, the darkest the
engine models): min light 0 < 3 over 27 floor cell(s) a player can walk to
(darkest at 2,1,4); by day it is 9 — this is a piece the sky reaches, and it
needs a light only where the delve reaches night
wrote lighting profile `dark` (bound to 27 cell(s)) into out/corner-passage.json
```

Binding count, both minima, the darkest cell's coordinates, and the exact
sentence §7 promised ("black at night, lit by day is the sentence to act on").
**No friction.**

### 7.4 The two pages disagree about what the admission chain IS

SKILL.md step 6:

> **Admit it**: the whole `delve-admit` chain (`audit` → `socket` → `anchor` →
> `lighting --write` → `catalog validate`), then `audit` again.

`prefab-procedure.md` §7's chain is `audit` → `socket` → `lighting` → `audit`.
Two steps SKILL.md names are absent from "the procedure" it tells you to follow.

`anchor` is harmless — my program declared its anchors, so there was nothing to
annotate. `catalog validate` is not:

```
$ delve-admit catalog validate out/corner-passage.json
DW0732 [error] out/corner-passage.json: invalid catalog card: unknown field
`prefab_id`, expected one of `asset_id`, `description`, `tags`, `style_fit`,
`quality`, `renders`, `demand_categories`, `license`, `curation` at line 2 column 13
cat_file_exit=1
```

A **catalog card is a different document** from the prefab metadata, living at
`catalog/<id>.json`. Neither page says that, neither page says what a card
contains, and neither page says how to write one. A person following SKILL.md's
chain literally runs this on the file they have and is told their prefab is an
invalid catalog card.

*What a person concludes:* that they have broken their metadata — the message
names their own file and their own field. They would go back and start editing a
document that is correct.
*Would have unstuck them:* either drop `catalog validate` from SKILL.md's chain,
or a §7 paragraph saying what a catalog card is and where it comes from.
**Severity: stops her dead** — the last step of the last authoring stage, named
by the skill, cannot be performed from anything either page contains.

*(My own error, recorded: I first read this as "exit 0 on an error" because I
measured `$?` through a `| head -20`. Taken on its own line it is exit 1 both
for a directory and for a file. The tool is right; my instrument was wrong.)*

---

## §8 — Where the files go, and a claim that the procedure itself falsifies

I did **not** perform §8: the piece is a throwaway and this round does not touch
the content repository. But §8 makes a checkable claim, and the round's own
artifact answers it.

**What the page says** (§8):

> the `.nbt` is a snapshot of one expansion of it, and its metadata carries the
> program hash and seed that **regenerate those exact bytes** (ADR-0006 —
> verified: same inputs twice gives byte-identical `.nbt` and metadata).

The shipped metadata carries exactly that row:

```json
"generated_by": { "generator": "grammar", "program": "corner_passage",
  "program_hash": "sha256:558172df…c917", "seed": 1 }
```

Re-expanding from those recorded inputs:

```
$ shasum -a 256 < out/corner-passage.nbt      # the piece as §7 left it
66bba39e79c7886f99cccf182a3a39ce567fa084bf2f1d30977bd47f65523cbf
$ shasum -a 256 < regen/corner-passage.nbt    # regenerated from program+seed
a359a76acbcab9751ede3cf9bb8bb75397f08cec0775008d354d445b7e4091e4
```

They differ. Determinism itself is fine — a second regeneration is byte-identical
to the first (`a359a76a…` twice), so the difference is attributable to
`delve-admit socket`, which is **§7, the step immediately before this claim, and
the procedure prescribes it.**

So for any piece that carries a socket — which §0 says is every piece that enters
a `prefab_pool` — the recorded provenance row does not regenerate the shipped
bytes, and §8's sentence is false as written.

**What I ESTABLISHED:** the two hashes, and that expansion is deterministic.
**What I did NOT establish:** that anything downstream depends on the claim.

*Would have unstuck them:* §8 saying the row regenerates the bytes **as
expanded**, before the §7 edits.
**Severity: costs an hour** — for whoever later trusts the row.

---
## Across the whole walk — "the library" names at least three different things

`SKILL.md`'s section is titled **"when the library has no piece you need"**. That
library is the **prefab** library: `.nbt` + metadata pairs in the *content*
repository, reached through the `campaigns/` symlink.

`prefab-procedure.md` uses the word 8 times, for at least three different objects:

| line | "library" means | where it lives |
|---|---|---|
| §2 163, §3 193, §4 258 | the **rule/program** library — `delve-grammar list`, `show --program` | the **engine** repo |
| §9 702, §9 728 | the **prefab** library — the checked-in `.nbt` + `.json` pairs | the **content** repo |
| §4 302 | "library-internal" — a Rust crate's private predicate | source |
| §0 28 | "the spec-0026 library" — a surround library that does not exist yet | nowhere |

Re-derived at this revision, both instruments named:

```
$ ls campaigns/prefabs/*.nbt  | wc -l      →  36     (prefab library, content repo)
$ ls campaigns/prefabs/*.json | wc -l      →  37     (36 + pools.json)
$ delve-grammar list | head -1             →  "36 program(s) in the rule library"
```

**Both are 36.** A person who counts one and reads about the other gets a
number that agrees for the wrong reason.

The gallery is worse: `prefab-procedure.md` never mentions it at all, and
`SKILL.md` uses "gallery" once (line 2061) for `delve-admit gallery`, a
*browse world for reviewers* — a third sense, unrelated to the engine's own
`gallery/` campaign or to `prefabs/gallery-generator`, both of which exist in
this repository and neither of which either page names.

*What a person concludes:* when §3 says "start from the corpus, never from the
schema. The library is a few-shot corpus this project legally owns", a newcomer
who has just read SKILL.md's heading looks for it in `campaigns/prefabs/` — the
wrong repository — and finds `.nbt` files with no programs in them.

*Would have unstuck them:* naming the two consistently — "the **rule library**"
(engine, what `delve-grammar list` prints) and "the **prefab library**" (content
repo, what a campaign binds) — and one line in §0 or §1 saying which repository
each is in.
**Severity: costs an hour**, once, to every new reader.

---

## §0 — the route table

Used it: "a new structural piece — a building, room, passage, stair; generic
(T1)" → grammar program, §1–§8. Unambiguous, first read, no friction. It is the
right thing to put at the top of the page.

---
# Verdict

## Could a person with this repository and these two pages produce a piece the engine will admit?

**Yes — with three interventions**, all of them documentation, none of them
engine work.

The piece exists. It passed thirteen gates at `expand`, `pass` at
`delve-admit audit` with `contract.state: judged` and `failed_gates: 0`, and
carries two carved sockets and a measured `dark` lighting profile. The
underlying toolchain did not fail once in the whole walk: every binary built,
every command that was correctly formed did what its page said, and every
diagnostic that fired was accurate and told me what to do next.

The three interventions:

1. **The `traversable` exception.** §4 says "pass it whenever the piece is a
   passage" and lists exactly one exception (for a *different* flag). A passage
   whose ends are on perpendicular faces fails it. Nothing on either page says so.
2. **The spatial contract as an authoring step.** The `traversable` failure
   prescribes "declare exterior edges". Neither page's authoring stretch
   mentions the contract, `claim`, or `grammar.md` §2d. A person is told what to
   do and given no way to do it.
3. **`catalog validate`.** SKILL.md names it as the last link of the admission
   chain. It operates on a document neither page describes, at a path neither
   page names, in a schema neither page gives.

Without (1) and (2) the run **stops at §4 with exit 4 and no `.nbt` on disk**.
Without (3) it stops at the end of §7.

## Ranked, in her terms

**Stops her dead**

1. **§4 `traversable` fails on a piece that turns a corner**, one line above a
   `reachability` line reading `100.0%`. Exit 4, nothing written. She will
   conclude the engine is broken, because the report says her floor is entirely
   walkable and the gate says it is not.
2. **The prescription she is then given cannot be acted on from either page.**
   "Declare exterior edges" — the word "contract" does not appear in either
   page's authoring steps. The one document that explains it (`grammar.md` §2d)
   is named once, at the top, as general background, and what it actually asks
   for is adoption of a nine-gate subsystem.
3. **`catalog validate` on the prefab metadata**, which SKILL.md's chain leads
   straight into. The error names her own file and her own field, so she will go
   back and edit a document that is correct.

**Costs an hour**

4. **Nothing in the render set shows a corner passage is a corner passage.** All
   gates green, nine shots, none of them informative; the piece was confirmed
   from cell ranges in a report. `--view name=…,face=<open face>` is the answer
   and §5 attaches `--view` to elevations and facades, not to plans.
5. **Two exterior edges export duplicate `faces`** into the shipped metadata that
   `delvec` consumes — following §2d's own worked example. Green everywhere.
6. **`--pos` for a socket has no rule on either page**, and the socket I placed
   put `minecraft:jigsaw` in the doorway and dropped the declared face from 9
   cells to 8. Verdict `pass`, no warning. I could not tell from the pages
   whether that is wrong.
7. **§8's "the program hash and seed regenerate those exact bytes" is false
   after §7**, which the same page prescribes: shipped `66bba39e…` vs
   regenerated `a359a76a…`, with determinism itself confirmed (`a359a76a…`
   twice).
8. **"The library" names at least three things**, and the prefab library and the
   rule library both currently hold 36 items.

**Mild friction**

9. §7's first code line — `delve-admit audit out/<id>.json # audit takes either`
   — is refused (`DW0732`, exit 2) for a single-template piece.
10. §1 says record the region in "the campaign's `GENERATION.md`"; a library
    piece has no campaign, and the page names no other home.
11. `delve-grammar list | head` ends in a Rust broken-pipe panic trace at exit 0.

## What I used that a person does not have

The stance was: this repository, `SKILL.md`, `prefab-procedure.md`, nothing else.
Where I went outside it:

- **`docs/reference/grammar.md` §2, §2b, §2c, §2d.** Not a violation for §2/§2b/§2c
  — `prefab-procedure.md` §3 names §2c and §2 by section number and tells you to
  read them. **§2d IS the violation**, and it is the finding: I read it because I
  already knew the spatial contract existed and would satisfy the diagnostic.
  **A person without that knowledge**, handed "declare exterior edges" by a red
  gate, has a page (`prefab-procedure.md`) whose authoring steps never mention
  the contract and a skill page that never mentions it either. Their next move
  is `grep -ri contract docs/` — which returns §7 and §9 of the procedure page,
  both describing the contract as something a *reader* consumes, not something
  an author writes. **I do not believe they would find §2d.** That is the single
  most valuable line in this log.
- **Engine source: none.** I did not open `crates/` at any point. Every command
  I ran, and every flag, is named on one of the two pages.
- **The brief's own claim of 36 pieces / 37 metadata files** — re-derived rather
  than taken (`ls campaigns/prefabs/*.nbt | wc -l` → 36; `*.json` → 37).

### Established vs. not contradicted

**Established** (each with the command that established it, above): the §4
`traversable` red on perpendicular ends; that declaring a contract clears it;
the duplicate `faces` in the shipped metadata and that one edge produces two
where two produce four; the `DW0732` refusal of §7's first line; that
`catalog validate` rejects prefab metadata by schema; that the socket adds
`minecraft:jigsaw` and drops the face count 9 → 8; the two differing hashes and
that expansion itself is deterministic; the two 36s.

**Not contradicted, and NOT established**: that the jigsaw-in-the-doorway is a
defect (it may be resolved at placement — `delve-admit resolve-jigsaw` exists);
that anything downstream reads the duplicated `faces`; that anything downstream
depends on §8's regeneration claim. Each of those is a question for a round that
is allowed to read the compiler. This one was not.

### Instruments

Engine worktree `dd287282723cfd34bec87ca181afae4ab55ce4f0`, built from source in
that tree: `delvec 1.1.0, dsl 0.18.0, mc 1.21.11`; `delve-grammar`,
`delve-admit`, `delve-render` from the same two `cargo build --release` runs
(exit 0 each, asserted on its own line). Content library read through the
`campaigns/` symlink to the shared checkout, which is mutable and may have moved
— the two counts above are as of this walk. Client jar
`~/.chunky/resources/minecraft.jar`. The piece was made in scratch and is not
committed anywhere.
