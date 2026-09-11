# Reference: when the prefab library has no piece you need

## Contents

- [1. The scene description](#1-the-scene-description)
- [2. The palette, measured](#2-the-palette-measured)
- [3. The grammar program](#3-the-grammar-program)
- [4. Expand, and let the machine judge](#4-expand-and-let-the-machine-judge)
- [5. Look at it](#5-look-at-it)
- [6. Admit it](#6-admit-it)
- [What the grammar cannot express](#what-the-grammar-cannot-express)

Follow `$DELVEWRIGHT_ENGINE/docs/reference/prefab-procedure.md` — it is the procedure, and these are
its mandatory steps, in order. Do not improvise around them. All four binaries
this needs are subcommands of the one binary Init I3 installed.

## 1. The scene description

1. **Write the scene description first** — one or two sentences: what a body does
   in the space, the material feeling, what the campaign will attach. Written
   after the render, it is a description of the render.

## 2. The palette, measured

2. **Choose the palette by measurement, never from memory** — and it is three
   steps, not one. A block's name is not its appearance (`packed_mud` is orange,
   142/107/80).

   **Screen** the shelf by constraints rather than by a guessed hex:

   ```sh
   python3 "$DELVEWRIGHT_ENGINE/tools/block-appearance.py" --screen --where full_cube --where 'L>=0.75' \
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

   The tool needs the pinned block registry from `$DELVEWRIGHT_ENGINE/crates/dsl/data/` **and** a
   1.21.11 client jar, and refuses by name when either is absent. That does not
   make the step optional: take role names from the corpus instead
   (`delvec grammar list`, then `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar show --program <nearest>`), which is
   a palette somebody already measured, and record where each name came from.
   Never invent one — a block that does not exist is refused at export, and one
   that exists but looks nothing like its name is caught only by eye at 5 below.

## 3. The grammar program

3. **Author a grammar program.** Read the **idiom index** first
   (`$DELVEWRIGHT_ENGINE/docs/reference/grammar.md` §2c): ten techniques with a runnable program each
   — repetition, `otherwise`, taper/arch/gable (one recursion), air-in-a-mix
   erosion, graded erosion, surface detail, symmetry without reflection, `skip`,
   light, and arguments (`bind` — one rule called with different content). It is
   the part of the language no type signature shows, and a scene that looks
   impossible is usually one of the ten. **Never copy a rule to change its paint,
   its size or its axis**: a caller passes a paint or a size with `bind`, an axis
   with `reorient`, and anything derivable from the box with an expression over
   `dim` — a copied rule family is one nothing keeps in step and no gate reads.

   ```sh
   delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar show --program idiom-shape
   delvec grammar list
   delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar show --program <nearest> > p.json
   delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar check --file p.json
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

   **Then say where a body goes — the spatial contract.** The rules say what
   blocks stand where; they never say which voids are rooms, where the doors are,
   or what a neighbour may mate with. That is a `claim` node per body of space
   plus one `contract` block classifying the names, written in the same document.
   **The authoring surface is `$DELVEWRIGHT_ENGINE/docs/reference/grammar.md` §2d** — that section is
   the only place that says how, and `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar show
   --program spatial-contract` prints a runnable one. Write one whenever the piece has more
   than one way in and out. Step 4's `traversable` judges a piece that has none —
   it derives the sides the piece opens on from the blocks — but only a contract
   turns that count into declared ways in, and only a contract can state a way
   out the blocks cannot show: a piece entered from **above** binds zero there
   and reds. Budget it as part of authoring: the moment a `contract` block is
   present, **nine obligations run with no flag** at both
   doors that read the piece — every name must resolve, every standable cell must
   lie in something declared, an `enclosed` space must be closed except at a
   claimed opening, every declared edge must hold on the bytes, and every anchor
   must land in a declared element. An `exterior` edge is **one claim per space,
   not one per door**: with no `via` it exports one face for every outer face its
   space reaches, so writing one edge per end on an L-shaped passage exports each
   end twice.

   **Anchors are part of this step too.** `mark` (`grammar.md` §2b) is the only
   way a campaign can name a place inside the piece, and it is also what gives
   step 5 its interior cameras — a piece with no anchor gets no eye shots at all.
   `at: floor_center` takes the lowest **world** Y of the scope it sits on, so a
   mark wrapping a column that includes its own floor slab lands *in* the floor
   and reds `contract-anchors`; mark the void, or use `at: offset` with the
   walkable Y. A `mark` may also carry `role` (`grammar.md` §2b, one term today:
   `entry`) — that is how a grammar-authored piece declares the 2A entry point
   without needing to write a name it structurally cannot write; one anchor per
   area may carry it (`DW0804`), and every other anchor still binds by name.
   For a hand-built or ingested piece, where no `mark` ever ran, the same role
   is given after the fact with `delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab anchor --role <term>` /
   `--no-role`
   (step 6) — run `--help` on the authoring pin's own binary before trusting this
   route name, since a later engine may fold it under a different command.

## 4. Expand, and let the machine judge

4. **Expand and let the machine judge**:

   ```sh
   delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar expand --file p.json \
       --region XxYxZ --seed N \
       --traversable --reachable-floor -o out/
   ```

   Pass `--traversable` for any passage, stair or route; pass `--reachable-floor`
   for any piece with an inside a body is meant to walk around. A red gate writes
   no `.nbt` (exit 4). **Read the `findings` in the report** — a gate that bound to
   zero objects, or a program that declared no anchors, is a finding, not a pass.

   `--traversable` asks the piece which faces it opens on, so a route is judged
   the same whichever way it runs and a passage that turns a corner is judged the
   same as a straight one. Where the piece declares a spatial contract those faces
   are its `exterior` edges and the binding count is doors; where it declares
   none they are the sides of the region its standable floor reaches, and the
   count is open sides — and a derived side is not a door, which the detail line
   says beside the number. Fewer than two is a refusal that names both repairs:
   open or declare the second way out, or stop claiming the piece is a route. A
   red here writes no `.nbt`, so it is not a warning to ship past. A piece
   entered from **above** lands there with a binding of **zero**, because a
   standable cell never lies on the region's top plane; that one is repaired by
   declaring the face at step 3, not by dropping the flag.

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
   `traversable` joins ground-level ways in and says nothing about the storeys
   above, so a building can pass every gate with half its floor stranded.
   Unreachable floor **under a roof** is a room with no way in, and the report
   gives you the box to go and look at. Unreachable floor open to the sky is a
   roof, and is nobody's defect.

   If the piece is one of a campaign's **zones**, its program belongs to the
   campaign: put it in `campaigns/<campaign>/design/programs/` and name it in
   `zones.json` there with the region, seed and gate claims it is built at
   (`traversable`, `allow_falls`, `reachable_floor`, `symmetric`).
   `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar audit --campaign-root .` judges every zone a
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

## 5. Look at it

5. **Look at it**:

   ```sh
   delvec --prefabs "$DELVEWRIGHT_PREFABS" render piece out/<id>.nbt -o shots/
   ```

   **The expand already measured this piece's light** and wrote the profile with
   the binding it was taken over — you do not run a probe by hand after an
   expansion. Read the `lighting` block: a `dark` piece renders, and it is telling
   you the room needs a light where the delve reaches night. This step refuses
   only a piece with floor in it that nobody measured (`DW0894`), which means it
   came from somewhere other than an expansion; `delvec --prefabs
   "$DELVEWRIGHT_PREFABS" prefab lighting <piece> --write` is the same
   measurement through the other door. Read the `DW0895` line too: it reports the
   roofed floor no body can walk to and the step the walk was refused at — a room
   with a ceiling and no way in renders exactly like a room with a door.

   and compare against the scene description from 1 above. The gates prove it is
   buildable and walkable; they
   say nothing about whether it is the scene you asked for. If the expand wrote a
   tile set instead of one `.nbt`, pass the manifest — `delvec --prefabs "$DELVEWRIGHT_PREFABS" render piece
   out/<id>.json` — which renders the assembled zone as one scene, eye shots
   included. Never review a single tile; the command refuses one anyway.

   **Open the `eye-<anchor>.png` frames FIRST.** They are the only cameras inside
   the piece — a body's eye at 1.62, at each declared anchor, looking the way that
   anchor faces. A piece that declares no anchor gets **none of them**, and the run
   says so in its binding count; that is step 3's `mark` still owed, not a render
   fault. The exterior orbit shots (`ext-*`, `door-*`, `anchor-*`) are fitted from
   outside, and on a roofed piece they are all the same picture of the same rock —
   but **`top` is a cutaway plan**, the roof taken off, and on a piece whose
   identity is a route rather than a face — a passage, a junction, a stair — it is
   the only planned camera that sees the route. Do **not** re-aim it by hand: a
   `--view name=…,face=up` is `"cutaway": false` at the same pitch and
   photographs the roof. Where a loud palette flattens the plan, add a `--view`
   square at each open face (`--view name=east-mouth,face=east`) and read the two
   together. Read `<id>-shots.json` beside the images for which cell each body is
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
   exterior frame. Keys: `$DELVEWRIGHT_ENGINE/docs/reference/tools.md` §4.

## 6. Admit it

6. **Admit it**: the `delvec prefab` chain, which for a generated piece is
   `audit` → `socket` → `lighting --write` → `audit` again.

   ```sh
   delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab audit out/<id>.nbt
   delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab socket out/<id>.nbt --pos X,Y,Z --facing <dir> \
       --opening 3,3 --name <ns>:<name> --target <ns>:<name> --pool pool/<name>
   delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab lighting out/<id>.nbt --write
   delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab audit out/<id>.nbt
   ```

   **Hand `audit` the `.nbt`, not the `.json`** — the metadata beside a single
   template is not a manifest and is refused (`DW0732`, exit 2). Only a tile set
   has a manifest, and then `audit` and `lighting` both take it and answer about
   one zone; handing any command a single tile is `DW0739`, and so is handing it
   a tile copied away from its manifest.

   Two subcommands are **not** on this route. `delvec prefab anchor` writes a place
   into an anchor whose producer could not — a hand-built or ingested piece; a
   grammar program already declared its anchors with `mark` at step 3.
   `delvec --prefabs "$DELVEWRIGHT_PREFABS" prefab catalog validate` reads a **catalog card**, the per-asset
   verification record of the ingestion route (`catalog/<asset-id>.json`), which
   is a different document from prefab metadata — run on a prefab's `.json` it
   correctly reports that file is not a catalog card.

   **`socket --pos` is the jigsaw cell: bottom-centre of the opening, in the wall
   plane.** The opening is built from it — width centred on it, height climbing
   from it — so read it off the piece: with a spatial contract, the metadata
   written beside it carries each exterior face's opening as a `from`/`to` cell
   pair (`spatial_contract.faces[]`), where step 4's report and `audit` name only
   the face's direction and cell count; with none, a `mark` whose scope is the
   mouth itself records that cell. `--facing` points **out** of the piece;
   `--name`/`--target` are what mate one piece to another; `--pool` is the
   `prefab_pool` the far side comes from. The carved socket leaves a
   `minecraft:jigsaw` block in the doorway carrying `final_state: minecraft:air`,
   so the world replaces it with air at placement — a re-run `audit` reporting one
   fewer cell on that face, and `minecraft:jigsaw` in the palette, is the marker
   being counted and not a blocked door.

   `socket` is also the **only** step that edits the blocks, so a piece that
   carries one no longer matches what its `license.generated_by` row regenerates;
   that row reproduces the `.nbt` as step 4 expanded it.

   A grammar prefab has **no connectors and no lighting** until this step, so it
   cannot enter a `prefab_pool` and will be dark, until you do it. `lighting`
   measures the darkest standable floor cell a body can walk to from a
   ground-level entrance, and reports the count it bound to.

   **Which sky the piece is measured under is the piece's own claim**, read off
   its `spatial_contract`. A piece that declares no contract, or one declaring
   any space `open` or `open_top`, is modelled standing in open air: sky light
   enters through its openings from the side, which is the only way a roof over
   open ground is ever lit, and without it a colonnade, a portico or a pier under
   a deck reads as pitch black. A piece whose contract declares **every** space
   `enclosed` claims no floor of its own stands under the sky, so it is measured
   with no sky at all and its figure is its own block light. That is the honest
   measurement for a piece a `detail-plan` row binds — its frame is the play
   space plus one floor course and the roof over it belongs to the whole — and it
   is why a `lit` verdict borrowed from open sky is worth nothing there: the same
   emitterless piece reds `DW0210` at the first build. The report says which sky
   was applied and why, in `assumed_sky.admits_sky` and `assumed_sky.why`, and the
   written `method` sentence says it again. Read them; a figure taken under the
   wrong sky is a number, not a verdict. Every piece a `details[]` row can bind
   declares a contract, because one that declares none is refused outright
   (`DW0843`).

   Two refusals to expect and not work around: `DW0752` means the
   probe bound to **zero** cells — usually a piece whose only way in is a socket
   that has not been carved yet, so run `socket` first; `DW0753` means there is no
   metadata to write into, and the fix is to create it, never to let the tool
   invent a `spdx: UNKNOWN` one.

## What the grammar cannot express

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
`delvec schem convert` and then the same admission chain with `resolve-jigsaw`
before `socket`. Never place an un-audited piece: `audit` is the licence and
code-injection gate, and the `DW0733` check that the blocks in it exist at all.
Flags in `$DELVEWRIGHT_ENGINE/docs/reference/tools.md` §2a and §3.
