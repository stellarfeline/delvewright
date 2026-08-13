# Making a prefab — the procedure

The one way a prefab is produced in this project, written as steps an agent
executes: one scene description to an admitted, rendered `.nbt`. Nothing here is
a plan; every step names a tool that runs today, and where one reaches less than
the whole step the step says so.

Behaviour references: [`grammar.md`](grammar.md) (what the back end does),
[`tools.md`](tools.md) (every binary and flag), [`compiler.md`](compiler.md)
(diagnostics).

## 0. Which back end

**The box-split grammar back end** (`crates/grammar`, spec-0027) — owner
decision, 2026-08-04. It is the default and this procedure is written for it.

When the scene is not a grammar scene, the route is decided by this table —
derived from the full technique survey (every approach this project researched,
probed, or rejected, with its tested evidence), not from recollection. A scene
that matches no row is **escalated, not improvised**.

| The scene is… | Route | Why this route — and what is proven about it |
|---|---|---|
| a new structural piece — a building, room, passage, stair; generic (T1) **or** a specific named referent (T2) | **grammar program** (this procedure, §1–§8) | The adopted production route for both tiers (owner, 2026-08-04). T2 is an input-modality property — the program is authored *against the referent* from the library corpus; the 2026-08-04 probe proved named referents recognizable this way, and the grammar's IR is what makes iteration converge where freehand geometry regresses. |
| a variation inside an existing hand-built tileset (another keep room in the keep's own conventions) | **grammar program**, matching the tileset's palette/conventions | The Rust generators are maintained, not extended (§9). A generator's conventions are data to imitate, not a surface to grow. |
| a named referent that plausibly exists as a **community build** | check licence-gated ingestion first (`delve-schem` + `delve-admit`, spec-0007); fall back to grammar | Availability is luck — the corpus audit found most schematic sources unverifiable or NC-tainted, so ingestion is an opportunistic shortcut, never the plan of record. Everything ingested passes the same audit/socket/lighting admission as generated pieces. |
| genuinely un-statable by axis-aligned boxes: a **smooth** curve, a diagonal, a profile whose step varies independently of the box, a vault bending on two axes at once, or noise/terrain (§6) | **in-house generator work** (Rust: value-noise fields, 4-5-rule cellular automata, the ported craft passes) — an engine task, not an authoring step | The grammar has no smooth curve, no diagonal, no noise, no terrain, by design. This route costs a worker dispatch and says so up front; it is the one row where "make a prefab" becomes "extend the engine". **Check §6 and `grammar.md` §2c before taking it** — a stepped arch, a gable, a spire, a batter and a tapered vault are all one recursion (idiom 3), two roofs meeting in a valley are that recursion peeling a ring (idiom 3), and any shape with a mirror plane is a rule body written mirrored (idiom 7). All three were mistaken for this row. |
| terrain or backdrop *around* the playable scene | **surround layer** (`horizon` — ocean/void today; the spec-0026 library when it lands), never a prefab | A surround is analytically known so the proofs can read it; modelling vanilla worldgen was evaluated and rejected (the compiler cannot see server terrain without re-implementing its noise — a folklore hack). |
| an atmosphere-tier set-piece whose power is composition — scale contrast, approach framing, lighting (T3) | **no current route — escalate to the owner** | Measured, not assumed: the landmark probe showed T3's missing layer is presentation (lighting/fog/camera), not geometry. Composition is art direction (owner ruling) and is M4 design work. Attempting it through any row above produces the geometry and loses the scene. |

And the composition rules — how the routes combine into an area:

- **The one assembly chain**: piece (any route above) → `delve-admit` admission
  → pool declaration → **compiler-as-jigsaw layout** (the compiler is the
  jigsaw; sockets are a connectivity vocabulary, not runtime mechanics) →
  edit scripts for L2/L3 fixes → relight → validation renders. There is no
  second chain.
- **Grammar pieces enter areas by direct placement only, today**: the export
  emits no jigsaw connectors, so a grammar prefab cannot join a
  `prefab_pool` until a socket is carved (§7) — and connector *emission* is an
  open design, flagged, not improvised.
- **Intent flows one way**: a reference image (`tools/refimg.py`) conditions
  the human and the program; a similarity score (`tools/refscore.py`) RANKS
  candidate expansions on the contact sheet and can never gate or veto —
  structurally enforced (`DW0725`/`DW0726`). Generate N seed-varied
  candidates, machine-order, **human picks one**. No image is ever
  voxelized into geometry (red line).
- **Fixes go through edit scripts, not regeneration**, once a piece is placed:
  the edit script is versioned, deterministic, and re-proves walkability and
  lighting after replay; hand-editing world files is not an authoring surface.

## 1. Fix the scene description first

One or two sentences, written down before any tool runs, naming: what a body
does in the space (walks through / drops into / fights in / is watched from),
the material feeling, and any element the campaign will need to attach to.

It is written first so that §5 has something to judge against. A description
invented after the render is a description of the render.

**Fix the region in the same breath** — not after the program is authored. It
is chosen from what the scene needs and from nothing else: there is no size a
design has to stay under, and a zone of any extent exports (§6). Record the
chosen region and seed beside the program — in the campaign's `GENERATION.md`,
since the program JSON does not yet carry its own region (queued engine
surface).

## 2. Choose the palette by MEASUREMENT

**Never name a block from memory.** Block names are not descriptions of block
appearance and repeatedly are not close: `packed_mud` is orange (142, 107, 80),
`lightning_rod` is signal orange (197, 111, 83), `dried_kelp_block` is a woven
olive-green (46, 55, 36).

```sh
python3 tools/block-appearance.py --near '#3a4038' -n 10 --full-cube-only
python3 tools/block-appearance.py --id minecraft:packed_mud \
                                  --id minecraft:deepslate_tiles   # --id repeats
```

Rules:

- Pick the target colour from the fiction, then take candidates from the ranked
  list. Record the measured hex beside each role in the program.
- `--full-cube-only` for anything structural: a wall made of a block whose model
  is mostly air is not a wall.
- The tool ranks; it cannot choose. A mean colour cannot see pattern or scale,
  and it has no idea what a block *is* — it will rank `structure_block` next to
  deepslate. Technical blocks are excluded by default; everything else you
  **see** at step 5 before believing.
- Biome-tinted blocks (`*_leaves`, grass, water) are flagged: their number is
  the untinted texture and the world will not look like it.

## 3. Author the program as JSON

**Read the idiom index first** (`grammar.md` §2c). It is nine techniques with a
runnable program each, and it is the part of the language that no type signature
shows: how a repetition, a taper, an opening, a decay gradient, a symmetric
aperture and a sconce are actually written. A scene that looks impossible is
usually one of the nine.

```sh
delve-grammar list                                # what exists — incl. `idiom-*`
delve-grammar show --program idiom-shape          # the technique, runnable
delve-grammar show --program store-room > my-piece.json
```

Then start from the corpus, never from the schema. The library is a few-shot
corpus this project legally owns (spec-0027 §2). Editing the nearest rule is
what made the worked example pass its first check; writing the IR from its
documentation is the slower path.

Then edit. The IR surface is `grammar.md` §2. Four things worth knowing before
you write:

- **Two guards that can both hold are a probability, not a priority.** A
  decision needs mutually exclusive guards — and the arm for "none of the above"
  is **`otherwise`**, which is the only precedence the language has. It is also
  what terminates a recursion: without one, a taper ends in `NoApplicableRule`
  the first time its guard fails.
- **`rounding` is owed by every surface, not only by floors.** The default
  truncates and never writes the remainder, and an unwritten cell is air — a
  floor with a hole at the far end, a wall with a slot of daylight along its top
  course, a ceiling open at one corner. No gate reads any of them. Use
  `"rounding": "start"` (or `end` / `middle`) on anything a body stands on,
  walks past or looks at. On a split with exactly one relative piece of weight 1
  it is inert, because the axis already divides exactly.
- **A palette role can be a weighted list, and `minecraft:air` is a legal
  member.** That is the whole of decay and rubble, and a role bound to a single
  block is why a piece renders as one flat material. The JSON is in `grammar.md`
  §2; a mix never moves a block, only what is written in it.
- **A block state with a `facing=` does not turn when the piece does.** A rule
  whose frame opens with `z(largest)` is handed two different orientations by
  two different boxes and paints the same state in both. Pick the state with an
  `orientation` guard, or the piece faces the wrong way with every gate green.

```sh
delve-grammar check --file my-piece.json          # structure only; fast
```

`check` finds unknown rules, unknown roles, split/child mismatches and
unmatchable guards without a region or a seed. Run it after every edit — it
costs nothing.

**It is a typo check, not a design review, and it will not once tell you the
piece is wrong.** Every defect it can see is a name or an arity: a role that is
not bound, a rule that is not defined, a split with the wrong number of
children. It has no region and no seed, so it never sees geometry. Call the
mirrored rule on both sides of a symmetric split and the aperture chamfers the
wrong way for half its height — `ok`. Move a sconce course five courses up the
wall — `ok`. Build a parapet two courses high so the anchor behind it looks
into stone — `ok`. Those are found at §4 by the gates, and at §5 by looking.
Budget `check` as insurance against a slipped keystroke and nothing else.

## 4. Expand, and let the machine judge

```sh
delve-grammar expand --file my-piece.json --region 9x6x21 --seed 1 \
    --traversable --id my-piece -o out/
```

Writes `<id>.nbt`, `<id>.json` (prefab metadata) and `<id>.report.json`.

**What `<id>` is.** It is the prefab's identity: it names all three files and
becomes the datapack structure path, so it may contain only lowercase letters,
digits and hyphens. `--id` sets it. Without `--id` it defaults to the library
program id (`--program`) or **to the input file's stem** (`--file`) — so
`var-B.json` asks for the id `var-B`, which is refused, before anything is
expanded or written. Name the file in lowercase kebab or pass `--id`.

The program's own `name` field is **not** the id and never becomes one. It
identifies the *program* in the metadata's provenance row
(`license.generated_by.program`), where an underscore is fine; the artifact is
named separately because one program expands into many prefabs at different
seeds, regions and parameters.

The id is knowable from the inputs alone, so an unusable one is refused up
front, with nothing written. (The region is not on this list: a region past the
48-block cap is not an error — it tiles. See §6.) A verdict line is printed only
once the prefab exists, so a `pass` never sits above a failure.

**Gates** (a red gate writes no `.nbt`; exit 4):

| Gate | Claim |
|---|---|
| `blocks-exist` | every block state the model paints exists in 1.21.11, properties and values included |
| `non-empty` | the expansion built something |
| `traversable` (`--traversable`) | a body can walk from the approach end to the exit end; add `--allow-falls` for a piece entered by stepping off a ledge |
| `reachable-floor` (`--reachable-floor`) | every cell of floor **under a roof** can be walked to from the grade entrance |

`--traversable` is opt-in because it is a claim about a *kind* of piece: a room
with one door has no far end and would fail it correctly and uselessly. **Pass
it whenever the piece is a passage, a stair or a route** — that is most of them,
and a route nobody proved walkable is the defect the gate exists for.

It is a claim about the **route only**. Both ends it joins are at ground level,
so a green `traversable` says nothing about the storeys above: a cathedral has
passed it with 45% of its floor reachable and nothing at all reachable above the
nave. **Pass `--reachable-floor` whenever the piece has an inside a body is meant
to walk around** — it is the gate that catches the upper level with no stair.

Every gate reports a **binding count**. A gate that examined zero objects is
printed as a finding, not folded into the pass; so is a program that declared no
anchors. Read the findings.

**Measurements** (numbers, no verdict — deliberately not dressed as gates): fill
ratio, distinct states, standable cells, footprint area and perimeter,
silhouette complexity (1.00 is a plain box), the five commonest blocks with
their shares, and **reachability**. Use the shares to see monoculture; the craft
gates that would *fail* on it are not built (see §6).

The reachability line runs on every expansion, flag or no flag, and reads:

```text
reachability   2267 of 4982 standable cell(s) reachable on foot from 182 grade
               entry cell(s) (45.5%) · 2128 sheltered · unreachable 237
               sheltered + 2478 open to the sky, in 57 pocket(s)
    pocket  84 cell(s), 84 sheltered — x 12..18 y 23..23 z 81..92
```

**Unreachable floor under a roof is a room with no way in, and it is raised as a
finding with the boxes to go and look at.** Unreachable floor open to the sky is
counted and left alone: a roof is standable and nobody walks it, and no engine
can tell a roof from a terrace. `entry_cells` of zero means nothing on a side
face at grade is standable — the measurement found no way in, which is a binding
of zero and not a building full of stranded rooms. Read the numbers before the
shots: 42% is a picture that renders perfectly.

If the region is wrong the tool refuses. A refusal is the correct outcome — a
region too small never yields a smaller building. Two refusal shapes today:
a **sized** rule that does not fit names itself and its requirement; a
**guard-exhaustion** refusal (`no alternative of rule "…" applies`) names only
the rule — the guards it tried are read from the program, not from the error.
Making that refusal print each alternative's failed inequality with its
evaluated operands is queued engine work; until it lands, budget one
read-the-rule round-trip per guard refusal.

## 5. See it before believing it

```sh
delve-render piece out/<id>.nbt   -o shots/ --size 640   # one structure template
delve-render piece out/<id>.json  -o shots/ --size 640   # a zone that shipped as a tile set
```

Which of the two the expand wrote is a fact about the region (§6); pass whichever
file is there. The manifest reassembles the tiles first, so every camera below —
the orbit shots and the eye shots alike — frames the whole zone and a body can
look straight across a cut. A single tile is refused: reviewing one would show a
building sliced at a packaging plane.

**Know what the set can and cannot show you before you judge anything by it.**
The cameras are fixed: yaw, pitch and field of view are properties of the shot
kind, and no flag aims one — `--size` and `--textures` change the pixels, not
the viewpoint. So the set gives you four corner three-quarters, a plan from
straight overhead, one view down at each socket and each anchor, and one level
view out of each anchor — and **no square-on elevation of any face**. Every
exterior camera sits on a corner bearing, and the only level camera stands
inside the piece. A west front, a gable end, a facade with a rose window in it:
nothing in this set photographs one flat-on, and a facade judged from a
three-quarter is judged at a slant.

The authority on what each camera did is `<id>-shots.json`, not this page: it
names every shot with its kind, yaw, pitch and field of view, and it is written
on every run.

Two kinds of camera, and only one of them answers this step's question.

**Orbit cameras** — four exterior three-quarters, a plan cutaway, one per socket,
and one per anchor showing where in the piece that anchor sits — are fitted to
the model from outside it. They show massing, silhouette and layout. On a roofed
piece they show the roof: eleven orbit shots of a 16×9×26 ward that is 81% solid
rock are eleven pictures of the same grey slab, and none of them can tell you the
piece has a corridor in it.

**Eye cameras** stand *inside*, at a body's eye height (1.62), at each declared
anchor, looking the way that anchor faces — `eye-<anchor>.png`. This is the shot
that shows the doorway's shape and proportion, what is in front of a body, how
the walls read, and whether an anchor is looking at the thing it is about. Read
these first.

The eye point is resolved, never assumed. A prefab is mostly solid, so an anchor
cell often holds a gate or a barrel; the camera then steps back along the facing
so the anchor's object stays in the foreground, and says so (`DW0727`). An anchor
with no body cell within three blocks gets **no** eye shot, and that is named too
— per anchor and in the run's binding count.

Every run writes `<id>-shots.json` beside the images: which file is which camera,
and for each eye shot the cell the body is standing in, how it was chosen, and
how many open cells lie ahead of it before something stops the view. A nudged
camera is invisible in its own frame, so it is written down instead.

Four shapes worth knowing when you read the set:

- A **flat grey rectangle** is outside the piece. A per-piece render has no
  neighbours, so a view that leaves the template shows background. An eye shot
  that is *nothing but* background is reported as an empty frame (`DW0727`):
  that anchor is aimed at nothing in this piece.
- Anchors are declared with a cardinal facing only, so an eye shot is level.
  A shot that is mostly near wall is telling you the anchor stands against one —
  the manifest's clearance count says how far ahead the first block is.
- **An anchor close to a tall front photographs the doorway, not the front.**
  The camera is level with a 70° field, so it reaches roughly `0.7 × distance`
  above the eye: three blocks out, the frame stops about two blocks up, and a
  twenty-block west front needs some twenty-six to fit. Stand the anchor that
  far back or the shot looks straight through the opening at eye height and
  returns background. Every other camera in the set points down; none tilts up.
- **A parapet is one course.** An eye stands 1.62 above the cell a body occupies,
  so one course tops out below it and the body looks over; a second course spans
  the eye line and stands across the view — the manifest reports zero clearance,
  stopped by the parapet itself.

Compare against the description from §1. The gates prove the piece is buildable
and walkable; they say nothing about whether it is the scene that was asked for.

## 6. What the grammar cannot do — escalate, do not improvise

Each of these was established by running it, except the two marked otherwise:

- **No block entities and no NBT.** No chest loot, no sign text, no spawner, no
  banner. Anything with a payload is bound by the *campaign* against an anchor
  (spec-0021), never by the piece. So the piece must **declare an anchor** for
  every such thing.
- **No jigsaw connectors.** The export emits none. A grammar prefab is usable as
  a single-`prefab` area as it stands; for a `prefab_pool` a socket is carved
  afterwards (§7).
- **No light.** The export declares `unmeasured` and it means it. §7 probes.
- **No axis limit.** A vanilla structure template holds 48 blocks per axis, and
  that cap is an internal packaging detail the toolchain absorbs: an expansion
  past it is written as a set of `≤48` tiles plus one manifest, cut
  deterministically from the region. It reaches neither the design nor the rest
  of the loop — `delve-render piece` and `delve-admit audit` take the manifest
  and treat the zone as one thing. *Established by running it.*
- **Axis-aligned boxes only**, and the true statement is narrower than it
  sounds. What is genuinely out of reach: a **smooth** curve (the steps are
  integers and integer arithmetic has no square root, so a circle is a polygon
  whatever you do), a diagonal, a profile step that varies independently of the
  box's own dimensions (there is no positional index), and a vault bending on
  two axes at once. A round tower and an organic cave wall are on that list.

  What is **not** on it, and is regularly mistaken for it:

  - a stepped arch, a gable, a ramp, a spire, a tapered vault and a battered
    wall are one recursion whose per-step extent is arithmetic on the remaining
    dimension — `grammar.md` §2c idiom 3 — and with the paint inverted the same
    program is the opening rather than the mass;
  - **any shape with a mirror plane.** An orientation is a permutation without
    reflection, so `reorient` cannot mirror a piece — but a rule *body* can be
    written mirrored, and a size list reversed is exactly that. Two such rules
    give a chamfered octagon that re-centres itself at any width (idiom 7);
  - **two roofs meeting in a valley.** Two prisms crossing union to a
    plus-shaped course, and a plus is a partition: the recursion peels the ring
    of its box instead of insetting it, and the two pairs of ring slabs are the
    same taper entered one step apart. Four rules give a cross-gable with a true
    valley at every re-entrant corner and both ridges at one height, at any
    size (idiom 3).

  *Read from `crates/grammar/src/orient.rs`, and the exceptions are demonstrated
  by `idiom-shape` and `idiom-mirror`.*
- **No terrain** — no noise, no heightfield; height variation comes from splits
  and recursion. *Same source.*
- **No craft gate.** spec-0027 §4's palette-role budget, gradient and depth rules
  are still not built, and what blocks them is named in
  `crates/grammar/src/gates.rs`: the budget is defined per *material family* and
  nothing here can decide what family a block is in. Until it exists, monoculture
  and flatness are caught by looking (§5), not by the machine.

## 7. Admit it

```sh
delve-admit audit    out/<id>.nbt
delve-admit socket   out/<id>.nbt --pos X,Y,Z --facing <dir> --opening 3,3 \
                     --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
delve-admit lighting out/<id>.nbt --write
delve-admit audit    out/<id>.nbt          # again, after the edits
```

`audit` is the gate that runs on the bytes rather than on the expansion:
hard-forbidden blocks (`DW0731`), blocks the pinned version does not have
(`DW0733`), block states that do not write every property the block has
(`DW0734`), and the palette allowlist (`DW0730`). A grammar prefab passes it by
construction for both block checks — the export refuses an unknown state and
writes every property of the ones it keeps — but a *hand-built* or ingested piece
does not, so `audit` is where those classes are caught for everything else.

`lighting --write` is a **static** estimate, not a live probe; it says so in the
metadata it writes. A piece it calls `dark` is dark because the program placed
no light — the grammar cannot warn you, so this step is where you find out.

**`audit` takes a tile set; `socket`, `anchor` and `lighting` take one structure
template.** Handed the manifest of a zone past the axis cap (§6) they refuse it
as unreadable bytes (`DW0732`, exit 2), so a zone that shipped as tiles has no
lighting step and carries `"profile": "unmeasured"` into the campaign. Its own
tiles are not a way round that: `lighting` on one **succeeds**, and `--write`
puts an anchorless `spdx: UNKNOWN` metadata document beside the zone's correctly
provenanced one — a second file claiming to describe a prefab that is one slice
of a building. Leave the tiles alone; the manifest is the only file that
describes the zone, and a lighting number for a slice of it would be a number
about nothing.

Each of these steps owns one block of the metadata — `socket` the connectors,
`anchor` the anchors, `lighting` the lighting — and leaves the rest of the
document exactly as it found it, provenance row included. Run them in any order,
as many times as the piece needs.

## 8. Where the files go

Generated `.nbt` + metadata live in the **content repo**
(`campaigns/prefabs/`), never in this one (ADR-0007). The grammar **program**
is the artifact of record and lives beside the campaign that uses it; the
`.nbt` is a snapshot of one expansion of it, and its metadata carries the
program hash and seed that regenerate those exact bytes (ADR-0006 — verified:
same inputs twice gives byte-identical `.nbt` and metadata).

Those four inputs are `license.generated_by { generator, program, program_hash,
seed }` — a **machine-readable** row, not only the prose `provenance` sentence
beside it. A shipped prefab carries it through every step above, so a tool can
answer "what regenerates this file" without a human reading the sentence. The
one prefab that legitimately has no row is one nothing can regenerate: an
ingested community build, or a hand-edited piece.

## 9. Hand-written Rust generators

`prefabs/*-generator` are standalone Cargo workspaces that predate the
grammar back end. They are maintained, not extended: a new piece is a grammar
program. Running one is `cargo run --release --manifest-path
prefabs/<gen>/Cargo.toml -- campaigns/prefabs/`, and every piece it emits goes
through `prefabs/invariants.rs` — including the block-registry check, so the
`DW0733` class is refused at that emitter too.
