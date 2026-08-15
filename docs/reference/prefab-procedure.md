# Making a prefab — the procedure

The one way a prefab is produced in this project, written as steps an agent
executes: one scene description to an admitted, rendered `.nbt`. Nothing here is
a plan; every step names a tool that runs today, and where one reaches less than
the whole step the step says so.

Behaviour references: [`grammar.md`](grammar.md) (what the back end does),
[`tools.md`](tools.md) (every binary and flag), [`compiler.md`](compiler.md)
(diagnostics).

## 0. Which back end

**The box-split grammar back end** (`crates/grammar`, spec-0027). It is the
default and this procedure is written for it.

When the scene is not a grammar scene, the route is decided by this table —
derived from the full technique survey (every approach this project researched,
probed, or rejected, with its tested evidence), not from recollection. A scene
that matches no row is **escalated, not improvised**.

| The scene is… | Route | Why this route — and what is proven about it |
|---|---|---|
| a new structural piece — a building, room, passage, stair; generic (T1) **or** a specific named referent (T2) | **grammar program** (this procedure, §1–§8) | The adopted production route for both tiers. T2 is an input-modality property — the program is authored *against the referent* from the library corpus; named referents are proven recognizable this way, and the grammar's IR is what makes iteration converge where freehand geometry regresses. |
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

## 2. Choose the palette by MEASUREMENT — screen, measure the mix, then LOOK

**Never name a block from memory.** Block names are not descriptions of block
appearance and repeatedly are not close: `packed_mud` is orange (142, 107, 80),
`lightning_rod` is signal orange (197, 111, 83), `dried_kelp_block` is a woven
olive-green (46, 55, 36).

Three steps, in this order. Do not stop after the first.

**2a. Screen the shelf.** State the fiction as constraints on measured axes, not
as a guessed hex. Constraints eliminate; they never score.

```sh
python3 tools/block-appearance.py --screen \
    --where full_cube --where 'L>=0.75' --where 'L<=0.95' \
    --where 'C_mean<0.02' --where 'texture_range<=0.30'
```

`L` is Oklab lightness, `C_mean` is how coloured the block is (0.03 is the
shelf's own 30th percentile — below it a block reads as a neutral), and
`texture_range` is how loud its pattern is (`white_concrete` 0.006,
`stone_bricks` 0.221, `dried_kelp_block` 0.419). `form=slab`,
`family=minecraft:sandstone`, `not tinted` and `not gravity` are facets too. The
example above takes 1146 blocks to 14.

`--near '#rrggbb'` still ranks by colour when you genuinely have a target hex,
and `--id` still answers "what colour IS this".

**2b. Measure the mix, and never trust its mean.** A weighted paint is reported
by four numbers:

```sh
python3 tools/block-appearance.py --mix 'sandstone=3,smooth_sandstone=3,andesite=4'
python3 tools/block-appearance.py --program my-piece.json   # every role + inline fill
```

`chroma_mass`, `chromatic_area` (what fraction of the wall is coloured rather
than neutral), `loudest_member` **named with its area share**, `dominant_hue`,
and `void_area` — `minecraft:air` is a member like any other, so a role that is
45% holes says so instead of reporting a solid wall's numbers. The mean is printed and is never the verdict: swapping half a
sandstone mix for calcite and polished diorite moves the mean 13.5 RGB units —
nothing — while the chromatic area falls 60% → 30%, which is a different
building. The craft rule the numbers serve is 60/30/10: **the loud member gets
10%, not 60%.** Every report states its binding count, and a zero binding is a
finding, not a pass.

**2c. Look at it.** A shortlist is not a choice.

```sh
python3 tools/block-appearance.py --screen --where full_cube --where 'L>=0.75' \
    --mix 'calcite=6,diorite=3,white_concrete=1' --sheet --seed 7
```

writes `.sheets/palette/swatches.png` — every survivor tiled and labelled, and
every candidate mix as its seeded weighted tiling, which is the wall at distance
zero. No GPU, no world, under a second. **Then read the PNG.** Measurement can
prove a mix is not warm; only a look decides it is right.

What the numbers cannot decide, stated so you do not wait for them to:

- **Whether the palette reads as the referent.** "Île-de-France limestone" vs
  "Egyptian sandstone" is cultural reference; no statistic contains it.
- **Role fitness.** The screen above returns a light source, a gravity block,
  wool and a metal — all right on every measured axis, all wrong for a wall.
  Light emission lives in game code and is in no vanilla data branch at all.
- **Pattern at distance.** Whether `stone_bricks` still reads as masonry twenty
  blocks away is a render question — step 5's contact sheet.
- Biome-tinted blocks (`*_leaves`, grass, water) are flagged: their number is
  the untinted texture and the world will not look like it. `--exclude-tinted`
  drops them.

**When the tool cannot run.** It needs two things that are not always there: the
pinned block registry at `crates/dsl/data/blocks-1.21.11.json`, and a
1.21.11 client jar (`--jar`, `$DELVEWRIGHT_CLIENT_JAR`, or
`~/.chunky/resources/minecraft.jar`). Missing either one is a named refusal that
says which. **The step does not become optional** — it becomes a different
source of measured names: the library corpus is a palette somebody already
measured, so take roles from it (`delve-grammar list`, then `delve-grammar show
--program <nearest>`) and bind by editing a role that already exists rather than
by recalling a block. Record where each name came from beside the role, as you
would record a hex. The one thing you still may not do is invent a name: an id
that is not in 1.21.11 is refused at export by `blocks-exist` (`grammar.md` §4b),
and an id that exists but looks nothing like its name will pass every gate and be
caught only at §5, by eye.

## 3. Author the program as JSON

**Read the idiom index first** (`grammar.md` §2c). It is ten techniques with a
runnable program each, and it is the part of the language that no type signature
shows: how a repetition, a taper, an opening, a decay gradient, a symmetric
aperture, a sconce and one rule called with different content are actually
written. A scene that looks impossible is usually one of the ten.

**A second instance of a shape is never a second copy of its rules.** Three
things a caller can hand a callee, cheapest first: nothing, because an
`absolute` size takes an expression over the scope's own extents; a turned frame
via `reorient`; and a paint, a size or a role via `bind` (idiom 10). Copying a
rule to change one of those is how a program grows a family that nothing keeps in
step.

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

`check` finds unknown rules, unknown roles, split/child mismatches, unmatchable
guards, an unknown document `version`, and a construct newer than the version the
program declares — all without a region or a seed. Run it after every edit — it
costs nothing. A program started from `show` already declares the current
version, so the version refusals only fire on one hand-edited by someone who
lowered it.

**It is a typo check, not a design review, and it will not once tell you the
piece is wrong.** Every defect it can see is a name, an arity or a version: a
role that is not bound, a rule that is not defined, a split with the wrong number
of children. It has no region and no seed, so it never sees geometry. Call the
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

Writes `<id>.json` (prefab metadata), `<id>.report.json`, and the blocks: one
`<id>.nbt` for a region within 48 on every axis, or a set of `<id>.x<i>y<j>z<k>.nbt`
tiles for a region past it, in which case `<id>.json` is the manifest (§6). Which
one you got is a fact about the region, and the rest of the loop asks for
whichever file is there.

**What `<id>` is.** It is the prefab's identity: it names every file above and
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
| `shape-complete` | every placed state writes its shape-carrying (`multipart`) properties, so no wall, fence or pane places as an isolated post (`DW0735`) |
| `states-complete` | every placed state writes **every** property its block has (`DW0737`). An omitted property means whatever a running server decides, and nothing that reads the piece before it runs — the render you check it against, the walk, the diff — can know which |
| `oriented-fills` | an orientation-sensitive state was filled only under a frame that leaves it alone, a passed `orientation` guard, or the scope's own axis frame — `{"local": …}` on the paint, which resolves its directions through the scope at fill time (`DW0736`; an image the pinned vocabulary cannot determine is refused as `DW0738`). The one gate with three answers: a world-frame state whose scope stood in the identity frame only because a reorientation request resolved to a no-op at THIS region's proportions is `undecided`, not passed (`DW0742`) — it refuses nothing, and the fix is either mechanism above |
| `non-empty` | the expansion built something |
| `stair-shape` (only when the piece holds a stair) | every written stair `shape` is the one vanilla derives from that stair's own neighbours (`DW0801`). A stair's `shape` is not stored — the world recomputes it on the first horizontal block update — so a wrong one survives the `.nbt`, the render and the contact sheet, and resets in play. A stair that writes no `shape` makes no claim and nothing can disagree with it |
| `fluid-contained` (only when the piece holds fluid) | every fluid cell is a source, and no source has an open cell beside or below it (`DW0800`). A run direction that leaves the piece's own outer face is counted and never judged — a shoreline piece's water is the sea — and reported as a finding on every piece that has one |
| `traversable` (`--traversable`) | a body can walk from the approach end to the exit end; add `--allow-falls` for a piece entered by stepping off a ledge |
| `symmetric` (`--symmetric x\|y\|z`) | the piece is its own mirror image across the mid-plane of that world axis, compared by presence rather than by block state |
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

**The one piece to leave it off: a one-way descent.** A level a body drops into
and does not climb back out of is unreachable on foot *by design*, and the engine
has no way to be told — the predicate that would answer it is library-internal,
so no flag, no report field and no metadata carries the claim. So
`--reachable-floor` is not a gate such a piece can satisfy: `drop-shaft` at
9×12×9 seed 1 fails it with 28 of 63 roofed cells unreached, and a red gate
writes **no** `.nbt`, so passing the flag anyway does not ship a piece with a
known red — it ships nothing. Expand without it and read the always-on
reachability line instead, where the lower level appears as an
`unreachable_sheltered` pocket with its bounding box. That pocket is the design,
and nothing here can tell it from a room with no way in, so say which it is in
the §1 scene description and the report has a reader who knows.

Every gate reports a **binding count**. A gate that examined zero objects is
printed as a finding, not folded into the pass; so is a program that declared no
anchors. Read the findings.

**A zone belongs to its campaign, and its campaign runs the same gates over it.**
A program that becomes one of a campaign's zones goes to
`campaigns/<campaign>/design/programs/`, and is named in `zones.json` beside it
with the region, the seed and the optional gates it claims. `delve-grammar audit
--campaign-root <content repo>` then expands and judges every zone there, and
both repos' CI run it. A program file that directory carries and the manifest
does not name is a finding — without that, a zone nothing checks and a zone
nobody wrote look the same.

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
The planned cameras are fixed: yaw, pitch and field of view are properties of
the shot kind, and `--size` and `--textures` change the pixels, not the
viewpoint. So without asking you get four corner three-quarters, a plan from
straight overhead, one view down at each socket and each anchor, and one level
view out of each anchor — and **no square-on elevation of any face**. Every
planned exterior camera sits on a corner bearing, and the only planned level
camera stands inside the piece. A west front, a gable end, a facade with a rose
window in it: nothing in the planned set photographs one flat-on, and a facade
judged from a three-quarter is judged at a slant. `--view` is the flag that aims
one, and it is the third kind below.

The authority on what each camera did is `<id>-shots.json`, not this page: it
names every shot with its kind, yaw, pitch and field of view, and it is written
on every run.

Three kinds of camera. Two are planned for you; the third you aim.

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

**Views** are the cameras you aim, `--view` per camera, appended to the set under
a name you choose. Neither planned camera is square-on at a face: the exteriors
are corner three-quarters and the eye shots are inside the piece, so a building
whose identity is one elevation — a west front, a gatehouse, an approach face —
has no picture until you ask for one.

```sh
delve-render piece out/<id>.json -o shots/ --size 640 \
    --view name=west-front,face=north \
    --view name=long-flank,face=west
```

A view is a bearing plus a subject box: `face=<north|south|east|west|up|down>`
(or `yaw=<deg>`), on `of=model` by default or on any anchor the piece declares.
A `face=` view frames **that face** — a 93-block-deep nave does not push its own
west front into the distance — so `zoom=` is a choice, not a number you have to
find. The full key list is [`tools.md` §4](tools.md).

Do not reach for a forecourt anchor instead. A level eye camera with a 70° field
reaches about `0.7 × distance` above eye height, so a 20-block front needs some
26 blocks of standoff; an anchor on the parvis looks straight through the doorway
and never sees the façade, and building a forecourt long enough to frame it
shrinks the whole piece in every exterior shot.

A view is refused before anything renders if it names a subject the piece does
not declare or a name a planned shot already has. A view that comes back as flat
background is an empty frame (`DW0727`) and is re-aimed, never read as an answer.

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
  - **any shape with a mirror plane.** A frame carries a direction as well as a
    mapping, so `reorient`'s `mirror` hands a body its own reflection: one rule
    and a reflection of it give a chamfered octagon that re-centres itself at any
    width (idiom 7), and `--symmetric <axis>` gates the claim;
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
delve-admit audit    out/<id>.json         # or out/<id>.nbt — audit takes either
delve-admit socket   out/<id>.nbt --pos X,Y,Z --facing <dir> --opening 3,3 \
                     --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
delve-admit lighting out/<id>.nbt --write
delve-admit audit    out/<id>.json         # again, after the edits
```

A tiled zone has no `out/<id>.nbt` at all — its blocks are the
`out/<id>.x<i>y<j>z<k>.nbt` files and `out/<id>.json` is the manifest — so on such
a zone every line above names `out/<id>.json`, for the reason spelled out three
paragraphs down. `socket` is the exception: a socket is carved into one tile's
bytes, so it is the one step a tiled zone still does not have.

`audit` is the gate that runs on the bytes rather than on the expansion:
hard-forbidden blocks (`DW0731`), blocks the pinned version does not have
(`DW0733`), block states that omit a property carrying the block's shape
(`DW0735`), and the palette allowlist (`DW0730`). A grammar prefab passes it by
construction for both block checks — the export refuses an unknown state and an
under-specified shape alike — but a *hand-built* or ingested piece does not, so
`audit` is where those classes are caught for everything else.

A zone past the 48-per-axis cap hands its **manifest** to `audit` and `lighting`
instead of an `.nbt`; both reassemble the tiles and answer about the whole
building. Handing either one tile is `DW0739`, and so is handing it a tile that
has been copied away from its manifest.

`lighting --write` is a **static** estimate, not a live probe; it says so in the
metadata it writes. A piece it calls `dark` is dark because the program placed
no light — the grammar cannot warn you, so this step is where you find out.

The minimum is taken over the **roofed floor a body can walk to from a
ground-level entrance**, and the report states how many cells that was, out of
how many are standable in the region box. Those two filters are what make the
number readable: a free-standing building stands in a box with ground around it,
and a minimum over the whole box is the unlit outdoors whatever the design does.
A binding of zero is `DW0752` and fails the step — carve the sockets before
probing a piece whose only way in is one. `--write` without metadata beside the
piece is `DW0753`: the measurement still prints, but nothing is written, because
a manufactured `spdx: UNKNOWN` skeleton is worse than an error.

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

## 9. The metadata document

A prefab is a **pair** of files: `<id>.nbt` and `<id>.json` beside it. The JSON
is the document below, and it has exactly one definition —
`delvewright_dsl::prefab` (`crates/dsl/src/prefab.rs`). Every producer and every
reader uses that type; nothing declares a local copy of the shape.

It lives in the DSL crate because `delvec` is published to crates.io and may only
depend on published crates, so that is the one crate every reader can reach.
`delvewright_schem::prefab` re-exports it under the path the asset-pipeline tools
use.

### Fields

| Key | Required | What it is |
| --- | --- | --- |
| `prefab_id` | yes | `prefab/<id>` — the id a campaign binds. |
| `structure` | yes | `{file, id, size[3], data_version, generator?}` — the `.nbt` half. |
| `anchors` | no (`{}`) | Named places, keyed by DSL anchor name. |
| `connectors` | no (`[]`) | Jigsaw sockets `{name, target, local_pos[3], facing, opening[2], joint}`. |
| `lighting` | no | `{profile, measured_min_light?, measured?, rationale?, method?}`. |
| `license` | no | `{source, spdx, note, provenance, generated_by?}`. |
| `waterline_y` | no | Local y of the piece's top authored water block. Checked against the ocean datum by `DW0344`; an ocean world where no placed piece declares one raises `DW0364` rather than passing on an empty check. |
| `spatial_contract` | no | The piece's declared spaces, out-of-walk regions, edges and faces (ADR-0020). |

An **anchor** is `{pos?, facing?, region?, block?, resolves_to?, dispenser?,
trigger_block?}` — one object class covering a point, a gate region and a trap's
pre-wired hardware, each writing only the keys it means.

`lighting.profile` is one of `unmeasured` | `lit` | `dim` | `dark`. The three
measured profiles must carry both `measured_min_light` and `measured`;
`unmeasured` must carry neither — a claim and its absence cannot both be true.
This is the same type the compiler validates a campaign's lighting claims with,
so a probe result that will not survive the compiler is refused where it is
written.

### Reading is total, writing preserves

Every field a producer may legitimately omit is optional, and an absent optional
is **omitted, never `null`** — so a legacy piece still loads, and a piece nothing
has probed does not have to invent a measurement. Field order on write is the
order of the table above, which is the order the checked-in library already uses.

A key the reading version does not model is **kept** and written back out. That
matters because these files are read-modify-written: `delve-admit socket` edits
`connectors`, `lighting` edits `lighting`, `anchor` edits the four place fields
(`pos`, `facing`, `region`, `block`) of the one anchor it names, and each leaves
the rest of the document as it found it. A type that models fewer fields than the
document has deletes the rest on the way out, silently, while every test it has
passes.

The depth matters as much as the breadth. A step that owned "`anchors`" would be
licensed to replace an anchor whole, which deletes the `dispenser` cell and
`trigger_block` a trap's hardware lives on, the `resolves_to` the exporter
derived from the piece's own contract, and any anchor key the tool does not
model — none of which the operator typed and all of which is the anchor's.
`crates/admit/tests/metadata_preservation.rs` holds every step to the paths it
declares, on a real export carrying each field at risk, and refuses to classify
a subcommand it has never been told about.

### `deny_unknown_fields`: where it belongs and where it does not

- **Campaign stage documents keep it.** They are authored against a versioned
  schema, a typo there is exactly the bug it catches, and forward compatibility
  is the `dsl_version` fence's job.
- **This document does not have it, on any struct.** Every reader of a prefab is
  a *consumer*, not the document's owner, and a new key here is not a typo — it
  is a content library newer than the engine reading it, which is the normal
  state of a mixed-version pair. Refusing turns a forward addition into a hard
  failure at the layer with the least context.
- **Unknown keys are reported, not ignored**: `delvec` warns `DW0543`, naming
  every key it does not model, at the document root and per anchor. That is the
  typo-catching the attribute used to do, at a severity that does not stop a
  build.

Capture is at those same two levels — the document root and each anchor — which
are where this document has grown every time (`waterline_y`, `spatial_contract`;
`resolves_to`, `dispenser`, `trigger_block`). A key added *inside* a connector,
a licence block or a spatial contract is accepted and ignored, not preserved; the
day one is added, it is captured at that level too.

**The `lighting` block is the one exception, and it is a known cost.** Its type
is the DSL's own `Lighting`, which is a closed schema because its job is a rule
about *values*: a measured profile must carry its measurement and an
`unmeasured` one must not, and a misspelled measurement key there is a claim
quietly becoming its own absence. The price is that a key added inside
`lighting` — and only there — is still a hard parse failure (`DW0346`) for an
older engine. Adding one is therefore a `dsl_version` matter, not a metadata
edit.

### Who reads it

| Reader | Uses |
| --- | --- |
| `delvec` (`compiler::registry`) | the whole document; consumes anchors, connectors, lighting, `waterline_y`, `spatial_contract.faces` |
| `delve-admit` | the whole document, read-modify-write |
| `delve-grammar` | writes it (single template) and the tile-set manifest (several) |
| `delve-render` | a narrow view — `anchors`, `connectors`, `lighting` — built from the document's own leaf types, because it must also read a tile-set manifest, which names `structure_set` instead of `structure` |
| `delvewright_schem::split` | one key, `structure_set`, to tell the two shapes apart |
| `prefabs/*-generator` | write it, serialize-only (separate Cargo workspaces; they never read a prefab back) |

## 10. Hand-written Rust generators

`prefabs/*-generator` are standalone Cargo workspaces that predate the
grammar back end. They are maintained, not extended: a new piece is a grammar
program. Running one is `cargo run --release --manifest-path
prefabs/<gen>/Cargo.toml -- campaigns/prefabs/`, and every piece it emits goes
through `prefabs/invariants.rs` — including the block-registry check, so the
`DW0733` class is refused at that emitter too.

`prefabs/connections.rs` runs at those same emitters, just before those gates.
It fills the shape-carrying properties a state leaves unwritten — connections
for a fence, wall, pane or bars; absent faces for a vine or a lichen — from the
piece's own neighbours, by vanilla's rule, and never overwrites a value the
generator wrote. The `DW0735` verdict and a vine/lichen attachment check are
emitter post-conditions there, so a generator cannot write a disconnected
grille and wait for admission to notice.

The derivation asks each neighbour whether it presents a full face, and a
**stair** answers from its own `shape`: a straight stair is full on the side it
faces, a corner stair is refused outright because its second quarter moves which
faces are full. So a stair that writes no `shape` has no answer to give, and a
fence, wall or bars beside it stops the generator with a red naming the cell
rather than guessing. This is narrower than the `stair-shape` gate above, where a
stair writing no `shape` makes no claim and nothing disagrees with it: that gate
judges a claim, this one needs a fact. A stair standing alone may still say
nothing; a stair standing next to something that joins must say what it is.

A pass that *places* a vine or a lichen asks the same module where the block may
hold on — `attachable_faces(block, cell, at)`, which returns every supported
face of that block, best first, and an empty list when the cell can hold
nothing. Ask it rather than walking the neighbours by hand: the module owns
which faces the block has and which direction each looks in, so a scan can
neither name the face pointing away from the rock nor overlook that rock
overhead is rock to hang from.

A fence gate in a run of fencing takes its `facing` from the run, not from
taste. A gate is joinable only across `facing.getClockWise()`, so a gate whose
`facing` lies *along* its rail spans the perpendicular axis, joins neither
neighbour, and opens a permanent gap. Read the axis the rail runs along at the
gate's cell and pick a `facing` perpendicular to it; where both work, the one a
player walking in from the approach side would place is the one to write.

A regeneration replaces the `.nbt` only. The `.json` beside it is the document
of record and several carry anchors no generator models, so rewriting one to
pick up a `.nbt` change deletes campaign content; regenerate metadata only when
the generator is what changed it.
