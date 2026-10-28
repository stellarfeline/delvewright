# spec-0125: A plot is planned with what it holds — one box per holding, its parts declared at the cut, its size derived from them, and its author free in the outline

- **Status**: Proposed
- **Ground**: written against the branch that carries spec-0123 and `docs/reference/place-craft.md` §7 (`feature/a-place-reads-as-what-it-is` @ `a99a13a48`), read only — `docs/reference/compiler.md` (*The map pipeline — `site-plan`*: `boxes[]`, `ceiling`, `sides`, `roof`, `identities[]`; *The blockout*), `docs/reference/grammar.md` §2d (the spatial contract: `spaces`, `no_body`, `edges`) and §4d (reachability), `docs/reference/delvec/compiler/burial.md` (`DW0885`), `crates/delvec/src/compiler/blockout.rs` (the stand-in is raised only for a place no piece fills; the leak check's commons rule), `crates/delvec/src/compiler/render_plan.rs` (the `threshold` shots), spec-0098 §14, spec-0120 §3, spec-0121 §2.3, spec-0122 §7.4, spec-0123 §1, §2, §3.4, §5; the skill's `references/placement.md`. Evidence: the demo The Four Trades (content `demo/the-four-trades` @ `4d60d3e`, four `clearance: 4` boxes of exactly 14 × 14, every building a plain rectangle) and the hillside comparison (`demo/saltstair-a-1140` @ `c4f3db2`, one box per building; `demo/saltstair-b-1140` @ `75643b0`, two open boxes of 41 × 54 and 41 × 50, in which the agent built an open-walled smithy and a cartographer with a reception room before its map room). The research is `docs/reference/place-craft.md` §8, whose scale is the player building community's: 58 documented community builds measured by their stated dimensions, the community's own rules of ceiling and room, and real-world holdings read for arrangement only.
- **What it is for**: the ruling of the side-by-side playtest of the two comparisons, taken as given and marked **[ruling]**: the one-box-per-building cut won on layout and seams, but *make each place larger* was carried out as *a bigger box per building*, and that was wrong. Every other statement is **[cited]** (a reading of the tree, a constitution rule, a research page) or **[authored]** (this spec chooses).
- **Numbers**: spec `0125`. Two diagnostics, written `DW-new-a-plot-cannot-hold-its-parts` (§3.4) and `DW-new-a-part-not-built-as-declared` (§5.3) until the planner allocates them. `nodes[].holds[]` (§3) changes what a layout graph says and the contract's `part` key (§5.1) changes what a program says; the `dsl_version` and the grammar document version each move to the next the planner allocates. The ledger and the shots (§5.4, §6.2) change no document and take no number.
- **Non-goals**: a building type anywhere in the surface (§3.2: the kind is a mechanism, the use is the author's words); a part's position or outline declared in the plan (the author builds the parts freely, §1); a coverage, setback or storey band the engine refuses on (§4.3: stated, read, never refused); a plot that spans two boxes; a part shared between two plots; a plot's parts as graph nodes (§13).

## 1. The ruling **[ruling]**

A **plot is a whole holding**. One plot holds one or more structures that are closely related — logically, or by design — together with the open spaces they share. A town lot holds its house, its front ground, its back ground and its outbuilding; a forge holds its shop, its work yard and its store; a farmstead holds its house, barn, byre and yard; a shrine holds its gate, approach, basin and halls; a quay holds its apron, warehouse and shed; a camp holds its rampart strip, headquarters, barracks and granary. The suburban lot is one example among these and names nothing in the rule. The plot's built outline is therefore naturally not one rectangle.

**The contents are planned at the cut.** When the site is cut, each plot is planned with what it contains: which structures; which open spaces, and whether each is walked or only seen; what share each takes.

**The plot's size follows from its contents.** It is never just room for the main structure, and never bigger for its own sake: an oversized plot spreads its structures too far apart, or makes one too large against a player.

**Inside the plot, the author builds the planned contents freely, in any outline.** A plot of one structure and no open ground is legitimate.

**The scale is the player building community's.** The game's own village houses are too small to hold story content and are at most a lower bound; the primary source for a plot's and a structure's size, style and composition is what recognised builders and build teams make and teach; real-world lot planning says only which parts belong together and how they are arranged, and never sets a size, because Minecraft differs too much from reality.

## 2. What the tree does today **[cited]**

A site plan cuts a map into `boxes[]`, one per graph node (`compiler.md`, `DW0824`). A box with `ceiling: {"clearance": n}` is one roofed interior whose outline is the rectangle; a box with `ceiling: {"open": n}` is a free sky-open volume whose piece builds anything inside it, within `n` courses of air. `references/placement.md` tells the creator to cut one box per building or connector, to cut a yard as its own place, and to stand every trade building on at least 12 × 12 or 14 × 14 of play space bound through `identities[]` (`DW0833`); `place-craft.md` §2 authored those figures from the *twice the room* ruling. The Four Trades did exactly that — four 14 × 14 lidded boxes, four plain rectangles — and the B cut of the hillside, with no rule at all inside its two open boxes, produced the freer forms. Nothing in the tree lets a plan say what a place holds: `intent` and `note` are free text no check reads; `stations[]` are anchors, not spaces; `references[]` are pictures of other people's builds; `sides` says which walls stand; `roof` is refused on an open box (`DW0988`); `identities[]` bind one measure of a box to one number. The handout (spec-0121 §2.3, spec-0122) carries a place's rows, frame, seams and neighbours, and nothing about what stands inside it, because nothing declares it.

## 3. The program surface — `nodes[].holds[]` **[authored]**

### 3.1 Why the node

A plot's program says what the place *is*, before any coordinate exists; the box's extent is then derived from it (§4). That is the layout graph's job — *the space as a graph, before any coordinate exists* (`placement.md`) — and the node is the object class a place's facts belong to (`intent`, `stations[]`, `reached`, `references[]`). The plan's box is checked against the node's program as it is checked against the node's existence (`DW0824`). A program on the box would put a pre-geometric fact in the geometric document; a program on the piece would be the author checking itself; a program in the brief's `facts[]` would be numbers with no kinds. One surface, on the place.

### 3.2 The shape

```json
{ "id": "node/corner-lot", "intent": "a dwelling on its own ground",
  "holds": [
    { "id": "part/house",  "kind": "structure", "use": "the family's house, two storeys", "cells": 130, "height": 14 },
    { "id": "part/garage", "kind": "structure", "use": "a cart shed on the lane, one storey", "cells": 24, "height": 6 },
    { "id": "part/front",  "kind": "ground",    "use": "a strip of garden between the street and the door", "cells": 45 },
    { "id": "part/back",   "kind": "ground",    "use": "the back yard: washing line, vegetable patch, the shed's door", "cells": 120 },
    { "id": "part/bed",    "kind": "ground",    "use": "a flower bed under the front windows, behind a low hedge", "cells": 12, "reached": false }
  ] }
```

`holds` is optional on a node. A node declaring none is today's node: a building that is its box, a road, a flight, a square. Each entry is a **part**:

| Field | Meaning |
|---|---|
| `id` | `part/<kebab>`, unique within the node, the ordinary `DW0110`/`DW0111` id rules; what the piece's contract names (§5.1) and what every line about the part is keyed by |
| `kind` | `structure` — a roofed, walled volume a body may enter — or `ground` — open space under the sky. The mechanism, nothing else: no `house`, no `shed`, no `yard`, and the schema admits no third word (`DW0100`) |
| `use` | the author's words for what the part is for; never player-facing, never inventoried; what the handout hands the author and what the review's `expect` reads |
| `cells` | the part's **play space** in cells — the cells a body could stand in, exactly as a box's `extent` is a play space: a structure's interior floor, a ground part's open cells. Walls, hedges, posts and the ground between parts are the plot's and belong to no part. A whole number, at least 1 |
| `height` | optional, on a `structure`: courses from the walk plane to the ridge. A design fact the handout states and the fit check holds (§3.4); absent, only the box's courses of air cap the structure |
| `reached` | optional, default `true`: a body walks this part. `false` declares a part that is seen and not entered — a garden strip behind a hedge, a closed inner hall, a store nobody opens — the node's own `reached: false` vocabulary at the part. It relieves the walk obligation of §5.3 and asks nothing: a body that steps into a flower bed is refused by nothing |

A part carries **no position, no outline and no neighbour**: where the house stands, which way the shed faces, whether the two share a wall are the author's (§1). A plot of one part — `holds: [{"id": "part/chapel", "kind": "structure", "use": "…", "cells": 72}]` — is legitimate, and so is a plot of ground parts only (a square with no structure, a camp's parade); a declaration of zero parts is `holds` absent.

### 3.3 What a program says across themes

| Plot | Parts |
|---|---|
| a dwelling on its lot | one structure for the house (80–220 cells); a structure for the shed or cart house (8–30); ground at the front (walked), ground at the back (walked), a bed or strip (seen) |
| a trade on its frontage | one structure for the shop and workroom (120–320 cells); ground for the work yard (walked, about the shop's size); a structure for the store (seen, 8–30) |
| a farmstead | structures for the house (80–220), the barn and the byre (90–270 each); ground for the yard (walked, about the barn's size), the midden or garden (seen) |
| a precinct | a structure for the gate (8–30), one for the hall a body enters (190–460), one for the inner hall (seen, smaller than the hall before it); ground for the approach (walked), the basin's court (walked) |
| a quay | ground for the apron along the water (walked); structures for the warehouse and the boat shed; ground for the mooring (seen) |
| a fortified camp | ground for the strip inside the wall (walked, two to three cells deep) and the parade (walked); structures for the headquarters (190–460), two barracks (90–270 each), the granary (90–270) |
| one structure, no ground | one structure part whose `cells` is the plot's play space |

The `cells` in the example above are the community's: a dwelling's play space sits in the corpus's middle half of 80 to 220 cells and its ridge at about 16 blocks, a shed at 8 to 30, a back yard about the size of the structure it serves (`place-craft.md` §8.2, §8.5); so the plot above is about 22 × 20 of play space with its walls and ways, and a plan that gave the house 50 cells would be refused by nothing and read by the review as the game's village scale, which the ruling rejects.

The `use` strings carry the theme; the surface carries two kinds, a number and a flag.

### 3.4 What is refused at validation — `DW-new-a-plot-cannot-hold-its-parts`

A plot that **cannot hold what it declares** is refused where the plan is written, validation tier, one code with its shapes named in the message:

1. *under a lid*: the node declares `holds[]` and its box's `ceiling` is a `clearance` — a lid is one structure, and a plot's structures stand under the sky between them; the remedy named is an `open` ceiling, or no `holds[]` and rooms instead;
2. *too small*: the sum of the parts' `cells` exceeds the box's play space, `extent[0] × extent[1]`, naming the sum, the area and the shortfall;
3. *too low*: a structure's `height` exceeds the box's `open` courses, naming the part;
4. *the id is not a part*: a part id that is not `part/<kebab>`, or two parts with one id, are the ordinary `DW0110`/`DW0111`; a `kind` the schema cannot spell, `cells` of zero, `height` on a `ground` part, are `DW0100`.

Binding line, on every validation of a plan: `plot program: P plot(s) over N node(s), K part(s) — S structure(s), G ground(s), R seen; cells declared C of A play space`. A campaign with no plot writes `0 plot(s) over N node(s)`.

**Rejected [authored]**: *refusing a plot whose parts cover less than a band* — an author who declares 60 cells of parts on 400 is wrong by §4 and the review says so, but the declaration is theirs, and a check never restricts; *inferring a plot from an open box with several enclosed spaces* — a declaration nobody made is nothing to confirm.

## 4. Size derived from the program

### 4.1 The derivation, in the guide **[cited: `place-craft.md` §8.5]**

The guide gives the order the research supports: name the parts; say which are walked; give each its play space in cells from the community's bands — the middle half of 58 measured community builds, a dwelling 80 to 220 cells, a trade structure 120 to 320, a barn or store 90 to 270, a chapel or hall 190 to 460, a shed 8 to 30 (`place-craft.md` §8.2); add one ring per structure for its walls and ground a body walks, at least two wide, from the entry to every walked part; write the box last, its extent that sum and its courses of air clearing the tallest ridge. The size test before the box is written: can a body at the entry name every part and say what it does in each; does any part fall under the community's lower quartile or exceed its upper quartile by half again; is the remainder over a third. `references/placement.md` carries the order and the test (§8).

### 4.2 What the engine confirms **[authored]**

§3.4's fit: the parts fit the play space, the tallest fits the air, the box is open. These confirm the author's declarations against each other and nothing else.

### 4.3 What the engine states and never refuses **[authored]**

Per plot, in `validation/plot-program.json` and one line of the build's index (§5.4): the **coverage** — structure parts' built cells over the play space; the **programmed share** — every part's built cells over the play space; the **remainder** — play space no part claims, in cells and as a share; per structure part its **setback** on each of the four sides (the least distance from its built cells to the box's edge on that side), its **gaps** to each other structure part, each bound `enclosed` space's **clearance**, and its **ridge** (the highest cell a body cannot be in over its columns, in courses over the walk plane); per part the **roofed share** — the fraction of its columns with a cell a body cannot be in above head height, within the box's air. Scale against a body is read from these: a clearance of two reads as cramped and three to four as a room (`place-craft.md` §3), a setback of zero at the front is a shopfront and of two to four a dwelling, a gap of one is two buildings and of zero one (`place-craft.md` §8.5). The community's bands are the reviewer's; a plot that stands outside them is a finding at the plan, never a refusal, because a dense frontage and a wide close are each a declared intent.

## 5. The piece binds its parts

### 5.1 The surface — `part` on a contract space and on an out-of-walk region **[authored; cited: `grammar.md` §2d]**

```json
"contract": {
  "entry": "front",
  "spaces": { "front":  { "envelope": "open",     "part": "part/front" },
              "hall":   { "envelope": "enclosed", "part": "part/house" },
              "upper":  { "envelope": "enclosed", "part": "part/house" },
              "shed":   { "envelope": "enclosed", "part": "part/garage" },
              "yard":   { "envelope": "open",     "part": "part/back" } },
  "no_body": { "bed": { "reason": "a flower bed behind the hedge; nobody walks it", "part": "part/bed" } },
  "edges": [ { "a": "exterior", "b": "front", "class": "walk", "via": "gate" },
             { "a": "front", "b": "hall", "class": "walk", "via": "door" },
             { "a": "hall", "b": "upper", "class": "climb", "rise": 4, "via": "stair" },
             { "a": "front", "b": "yard", "class": "walk", "via": "side-path" },
             { "a": "yard", "b": "shed", "class": "walk", "via": "shed-door" } ]
}
```

`part` is optional on a space and on a `no_body` region, naming a `part/<id>` of the place's node. Several spaces may name one part — a ground floor and the storey over it, two rooms of one house (the L of the demo is two `enclosed` spaces and a `walk` edge through a doorway with a door, spec-0123 §3.1). A space naming no part is the plot's unprogrammed ground or an interior the plan did not list; it is counted in the remainder and refused by nothing. On a piece whose node declares no `holds[]`, a `part` key is refused as naming a part the plan does not declare (§5.3 shape 2). `delvec detail` hands the node's parts into the expansion beside the open sides (`ExpandOptions`), the way the allocation hands `sides` (spec-0123 §5.3).

### 5.2 What is measured **[authored]**

A part's **built play space** is the union of the xz projections of every region bound to it — the cells its spaces and `no_body` regions claim — counted in columns. A part's columns are those cells. The measurement is of the piece's own declarations, which the contract gates then prove against the bytes (`contract-closure`, `DW0844`, spec-0036); nothing here reads a wall off a block.

### 5.3 What is refused at detail — `DW-new-a-part-not-built-as-declared`

Judged by `delvec detail --check` over the piece and again by the whole build, detail tier, one code with its shapes named:

1. *unbuilt*: a declared part no space and no `no_body` region names;
2. *undeclared*: a `part` naming an id the node does not declare, or any `part` on a piece whose node has no `holds[]`;
3. *not walked*: a `reached` part none of whose columns holds a cell the piece's own flood reaches (`grammar.md` §4d, from the contract's `entry`); a `reached: false` part asks nothing;
4. *out of size*: a part whose built play space is under three quarters or over five quarters of its declared `cells`, naming both numbers.

The message names the part, its `use`, the shape and the number; the remedy is the piece's — bind the space, rename the part, open the way, resize — or the plan's, when the plan was wrong, and the message says both.

### 5.4 The ledger and the lines **[authored]**

`validation/plot-program.json`, written by the whole build beside `clear-width.json` and `piece-census.json`: one entry per plot with its node, box extent, every part with declared and built cells, kind, `reached`, `height` and ridge, roofed share, setbacks and gaps, each bound region by name, and the plot's coverage, programmed share and remainder; a plot still massed writes its declaration with `built: null` and the reason *massed*. The build's index prints `plot program: P plot(s), K part(s) built B of K; coverage min–max %, remainder min–max %`. `delvec detail --check` prints the piece's own line, `parts: B of K bound, W of R walked`, before its verdict.

## 6. The handout and the frames

### 6.1 The handout **[authored; cited: spec-0121 §2.3, spec-0123 §3.3]**

`scripts/handout.py` gains one section after *What this place is, and what it is built from*, written only for a place whose node declares `holds[]`: **What this plot holds**. It states the ruling's sentence — *this place is a plot: it holds the parts below and the ground between them; build them in any outline, inside the frame* — then one row per part: id, kind, `use`, `cells` with its share of the play space, `height` where declared, and *walked* or *seen*; then the binding rule in one sentence — *name each part on the contract spaces and out-of-walk regions that are it, with `part`; every part is named; a walked part has a cell the body reaches; a part's claimed cells stay within a quarter of its declared* — and the guide's shapes of `place-craft.md` §8.5 step 5 as four lines (the entry side is the front; an outbuilding behind or beside, one cell off; a yard enclosed by the structure it serves; a precinct a sequence from the gate). *The frame* says the box is sky-open and that the roof of every structure is the author's, within the stated courses of air; the neighbour summary states a neighbour's parts by `use` where the neighbour is a plot, so a facing author knows a work yard is across the lane and not a wall. The binding counts gain `parts`.

### 6.2 A frame per part **[authored; cited: spec-0123 §3.4, `render_plan.rs` `threshold_shots`]**

The `threshold` shot kind gains the plot's case. For every plot, one shot per part from each of the plot's crossings: eye on the plot's side of the seam's crossing — the middle cell of a portal's sill, or of a contact's span — at `1.62` over the walk plane, aimed at the centre of the part's built columns at eye height; `expect` reads *the part reads as its use: <use>*; `shows` lists the part and the plot. A plot with no crossing (entered by a `contact` onto the commons) stands the eye at the middle of its entry side. A plot still massed draws none and the index says so. The index line gains `, Q part(s)`: `threshold shots: T over P place(s), D door(s), O open side(s), Q part(s)`. A plot's crossing is a `contact` more often than a portal — a front open to the lane — and spec-0123 §3.4 draws no shot for a contact, so without this case a plot's structures would have no threshold frame at all.

### 6.3 The review **[authored; cited: spec-0123 §3.4, `references/visual-review.md`]**

The blind read of spec-0123 §3.4 gains a fourth line for a part frame: **the holding** — before the index entry is opened, the reviewer writes what the part is for, in one phrase, and whether it belongs with what else is in frame (*a shed behind a house*; *a store beside a forge yard*; *a hall and a hut that have nothing to do with each other*); then reads `expect`. A use that cannot be named, or a part that does not belong, is a finding at the plot's **program** — the plan's `holds[]` and the piece together — repaired by a fresh author with the finding and `place-craft.md` §8.5 in its handout, and the ledger's entry (§5.4) quoted beside the finding so the author sees the setbacks, gaps and shares the reviewer saw. The index states `threshold read: … holding misses H`.

## 7. What stays a review judgement **[authored]**

Said plainly: whether the parts belong together as one holding; whether each ground part answers *what does the player do in here*; whether a structure reads as its use from the entry; whether the plot stands at a density its street means — a dense frontage, a loose farmstead, an empty close; whether an outline is interesting or a wing added for variety (WesterosCraft, `place-craft.md` §8.2). The reader is the step-12 reviewer on the part frames of §6.2 with the ledger beside them; a miss produces a finding at the plot's program and a fresh author (§6.3). The engine confirms that what was declared was built and states the numbers; it decides none of these.

## 8. Reconciling the guide and spec-0123 **[authored]**

`references/placement.md` carries one rule, and the two bullets *Cut the site with one box per building or connector* and *Give a building room for rooms* are **replaced**, never kept beside the new text:

- **Cut the site with one box per plot or connector.** A plot is a place that holds one or more related structures and the ground they share — a house with its yards and shed, a forge with its work yard, a farmstead, a precinct, a quay, a camp — and a building standing alone is a plot of one structure. A road, a flight of steps and a bridge are each a place; a square's centrepiece and a yard belonging to no plot are open-sided places (`sides`). Never cut two unrelated buildings into one box, nor a terrace of buildings into one place, nor one holding into several boxes.
- **Plan a plot with what it holds, and size it from that.** Before its box is written, list its parts — each structure, each open space, whether a body walks it or only sees it, and its play space in cells from the community's bands — then add one ring of wall per structure and walked ground at least two wide joining the entry to every walked part; the box's extent is that sum, its ceiling `open`, its courses of air clearing the tallest ridge. Inside it the place's author builds the parts in any outline. Never size a plot as room for its main structure, and never larger than its parts: a plot whose parts and ways cover less than half of it spreads its structures apart. A structure that is the whole plot with no ground may stay a `clearance` box. The bands and the order are `docs/reference/place-craft.md` §8.5.

The 12 × 12 and 14 × 14 figures and the `identities[]` binding of a trade building's box are removed from the page; `identities[]` keep their job for the whole's facts. Once the release carrying `holds[]` moves the pin, the second bullet names the surface and its two codes (`check-skill-page.py` rule 17).

**spec-0123, on the same branch and Proposed, is amended in place** so that no two authorities hold the cut: §1 states one box per plot or connector and points here; §2 keeps the *twice the room* ruling and replaces the figures and the `identities[]` binding with the per-part cells of this spec; §4 names a yard as a plot's ground part or, alone, an open-sided place; criterion 1 asserts the amended page. Its §3.5 (one style per building) and its trade rules (§3.2, §3.3, §3.4) **agree with this spec** and are untouched: the four-part style rule binds every structure of a plot exactly as it binds a building in a box of its own, the threshold test is extended by §6.2 and §6.3 here and narrowed nowhere, `references[]` are gathered per place and a plot is a place, and the census of §3.5 is written per bound piece, a plot's piece included. Its §5 (`sides`) is what a plot's open front uses. Its demo The Four Trades is read as the case this spec corrects, and the demo of §12 is where the correction is confirmed.

## 9. Whether a non-rectangular structure inside an open box passes today's checks **[cited, read from the tree]**

The question is answered by reading, and the demo (§12) is where the reading is demonstrated:

| Check | What it does to an L-shaped structure, set back inside a sky-open box with its yards |
|---|---|
| **the stand-in and the ring** (`blockout.rs`, `delvewright_dsl::bound_places`; `compiler.md` *The blockout*) | The derivation raises a ring wall and floor only for a place **no piece fills**; a bound plot's cells are written by its piece alone. Nothing demands a wall in the ring: the piece decides what stands at the plot's line. |
| **the leak check `DW0838`** (`blockout.rs`, the commons rule) | A body leaving the plot into the **commons** is admitted on an `open` site; a body crossing into a **neighbour** outside an allocated seam is the leak. So a plot's line toward a neighbour plot must hold a body in — fence, hedge, wall — and toward the commons may be open. A front onto a street is a `contact` or a portal, as any place's. |
| **a front fence with a gate** (`DW0836`, `DW0986`, spec-0123 §3.1) | A fence line at the street with a gate in it is a `wall` side and a portal seam of the gate's width; a fence gate in the opening is a cell a body passes in either state, so every opening cell is passable and the way leads through. A front with no fence is an `open` side with a `contact`. |
| **exposure `DW0885`** (`burial.md`) | Judged over the air the party can be in, bounded two cells out: the plot's outward solid boundary cells in open air — its fence, hedge or wall at the line — are declared by the piece's `shown_faces`, written by the program (`grammar.md` §2f). The structure inside, set back from the line, is not a boundary of the piece and is asked nothing. |
| **light `DW0210`** (spec-0122 §7.4) | Judged over every detailed place and the whole's own cells; the yards at night and the structure's rooms need the author's light, as any place's do; the outline is irrelevant. |
| **reach** (`DW0837`, `DW0921`, the route proofs) | Over the bytes, from the entry, by the nav model's one predicate; an L is two rooms and a door, which the far hall of the gallery already proves (spec-0123 criterion 19). |
| **clear width `DW1057`, `DW1065`** (spec-0120 §3, spec-0123 §5.4) | Binds to the plot's **ways** (`way`), its seams' approach cells and its declared **open sides**; a structure inside the plot is in none of them unless it stands in an open side's ring or a declared way. A plot's piece ways — a declared `way` on a contract edge — are judged like any piece's. |
| **the roof** (`DW0988`, `compiler.md` `boxes[].roof`) | `roof` is refused on an open box; each structure's roof is the piece's, drawn within the box's `open` courses and the claim's one-cell ring; a structure set back from the line has its eaves free. |
| **the contract** (`grammar.md` §2d, `contract-closure`, `DW0844`) | An L is two `enclosed` spaces joined by a `walk` edge through a doorway, each space's boundary confirmed; the yards are `open` spaces, or `open_top` where walled; the plot's crossing is answered by an `exterior` edge whose `via` is the seam's cells or the open side's. |
| **the threshold frame** (spec-0123 §3.4) | Drawn per portal and per open side only, so a plot entered by a contact gets **none** today; §6.2 is the gap this spec closes. |

**Conclusion [cited]**: today's checks admit a non-rectangular structure inside an open box; what the tree lacks is any declaration of what the box holds, any check that it was built, and a frame to look at it from. **What the demo must prove**: an L-shaped structure with a shed and two yards in one open box builds green on the whole ladder with `0 under 0` stand-ins in the `DW0210` line, the plot's part frames drawn, and the two codes reddening on their probes.

## 10. Pages

`references/placement.md`: §8's two bullets, the figures removed. `references/detail.md`'s contract: `part` on a space and a `no_body` region, the four shapes of §5.3 in one line each, and `place-craft.md` §8 named. `scripts/handout.py`: §6.1's section, the frame sentence, the neighbour's parts, the `parts` count. `references/visual-review.md`: §6.3's fourth line and the `holding misses` count. `SKILL.md` step 2: the plot planned before its box; step 12: the part frames. `docs/reference/compiler.md`: `nodes[].holds[]` with its fields, the two codes, the validation and detail binding lines, `validation/plot-program.json` with its fields, the `threshold` kind's part case and its index count, `ExpandOptions`' parts; `docs/reference/grammar.md` §2d: `part` on `spaces` and `no_body`, and the version. `docs/reference/tools.md`: the handout's new section. A page names `holds[]`, `part` and the two codes only once the release that carries them moves the pin; the guide text of §8 names no new surface and lands with this spec's implementation.

## 11. Gallery element and refusal probes **[authored]**

**Gallery** (`gallery/overlays/site-plan/`): `node/exit` — the sky-open court with three open sides — declares `holds[]` of three parts: `part/court` (`ground`, walked, the court's own cells), `part/well-house` (`structure`, `height` stated, a small roofed room a body enters) and `part/bed` (`ground`, `reached: false`, a planted strip); it gains a program `programs/exit.json` whose contract binds `court` as an `open` space, `well-house` as an `enclosed` space and `bed` as a `no_body` region, each with `part`; the whole builds, the ledger lists the three with built cells inside the tolerance, the court's coverage and remainder stated, the validation line reads `1 plot(s) over 10 node(s), 3 part(s) — 1 structure(s), 2 ground(s), 1 seen`, and the build draws three part frames from the court's crossings. Two perturbations prove the element bound: moving one cell from `court` to `well-house` in the program moves `built` by one in two rows; raising the well-house roof one course moves its `ridge`.

**Probes**, each the site-plan point plus one declared edit:

| Probe | Code | Edit |
|---|---|---|
| `a-plot-under-a-lid` | `DW-new-a-plot-cannot-hold-its-parts` | `node/exit`'s box given `ceiling: {"clearance": 4}` |
| `a-plot-smaller-than-its-parts` | `DW-new-a-plot-cannot-hold-its-parts` | `part/court`'s `cells` raised past the box's play space |
| `a-part-taller-than-the-sky` | `DW-new-a-plot-cannot-hold-its-parts` | `part/well-house`'s `height` set to the box's `open` plus one |
| `a-part-nobody-built` | `DW-new-a-part-not-built-as-declared` | `exit.json`'s `well-house` space loses its `part` |
| `a-part-the-plan-does-not-declare` | `DW-new-a-part-not-built-as-declared` | `exit.json`'s `court` space names `part/garden` |
| `a-part-built-half-size` | `DW-new-a-part-not-built-as-declared` | `part/court`'s `cells` doubled in the graph |
| `a-seen-part-nobody-reaches-passes` | none — a green probe is not a probe | not written: the `reached: false` bed is the gallery element itself |
| `a-kind-the-schema-cannot-spell` | `DW0100` | `"kind": "yard"` on `part/court` |
| `a-height-on-the-ground` | `DW0100` | `"height": 3` on `part/court` |

## 12. Demo level **[authored]**

One row for `docs/demo-levels.md`: **The Lane of Holdings** — one lane with four plots on it, each planned with its program before its box and each built to it by a fresh author from the handout. A dwelling on its lot: an L-shaped house of two rooms and a storey, a cart shed on the lane behind it, a front strip a body walks to the door, a back yard with the shed's door in it, and a bed behind the hedge nobody walks. A forge on its frontage: a shop and workroom at the line with its front open over a counter, a work yard beside it, a store. A chapel that is the whole of its plot — one structure, no ground, the outline free — beside the same chapel cut as a lidded box, so the two spellings stand side by side. A byre and its pen at the lane's end, the farm's edge. Looked at: do the four read as four holdings from the lane; does the L read as one house with a wing that has a job; does each yard answer what a player does in it; is the lane's density right; the part frames read blind for the use and the holding; and the ledger's coverage, setback and gap lines against the frames. Beside it the refusals as transcripts: the dwelling's plot under a lid, its parts' cells past its play space, its shed's `part` dropped from the program, and its yard built at half its declared cells.

## 13. Rejected **[authored]**

- *Parts as graph nodes with boxes of their own.* That is the cut the ruling refuses: a house and its shed as two places, one cell apart, each a rectangle, their yard a third.
- *A `type` or `role` word on a part* (`house`, `shed`, `yard`, `garage`, `barn`). A design decision the engine has no business making; the kind is the mechanism, the use is the author's (CLAUDE.md: a primitive encodes a mechanism, never what it is for). The ruling: nothing in the surface names a building type.
- *A declared position or footprint rectangle per part.* The ruling frees the outline; a rectangle per part is four boxes in one box.
- *Shares as percentages.* A share needs the plot's area first, which is what the parts derive; cells are the number the handout, the fit check and the built comparison all read.
- *Refusing on coverage, setback, storey or gap.* Each is a legitimate intent at some density; the engine states them (§4.3) and the review reads them.
- *A roofed-share refusal on `kind`.* A yard under a tree and an open-sided market both cross the line a byte-read would draw; the share is stated.
- *Deriving the box's extent from `holds[]`.* The extent stays the author's declaration and the fit is confirmed, so an author may size a plot wide on purpose and the review may say so; a derived extent would hide the judgement.

## 14. Open questions and limits

- **The coverage band as an opt-in.** If the blind read keeps finding oversized plots, the next shape is a declared `coverage` on the node the engine confirms against the built share, never a band the engine applies unasked.
- **A part across two plots.** A shared wall between two neighbours' structures, or a yard two holdings share, is two plots meeting at a seam today; whether a part may be the seam's is undecided.
- **The part frame's eye for a plot with several crossings** draws one frame per part per crossing; a plot with four crossings and six parts draws twenty-four, and whether the review wants all of them or the nearest is decided on the demo.
- **`height` against the box's air** confirms a declaration; whether an undeclared ridge that reaches the box's top course should be stated as a finding is the demo's question.

## Acceptance criteria

1. `layout-graph.json` accepts `nodes[].holds[]` of `{id, kind, use, cells, height?, reached?}` with `kind` one of `structure` and `ground`; `"kind": "yard"`, `cells: 0` and `height` on a `ground` part are `DW0100`; a duplicate `part/` id is `DW0111`; `delvec fmt` writes the fields in the order of §3.2; a `crates/dsl` test asserts each.
2. `DW-new-a-plot-cannot-hold-its-parts` is raised at validation for a plot under a `clearance` ceiling, for parts whose `cells` sum past the box's play space, and for a `height` past the box's `open` courses, each naming its shape and numbers, and not for a plot whose parts fit; asserted by a test; listed in `compiler.md`; bound by the three probes of §11.
3. Every validation of a plan prints `plot program: P plot(s) over N node(s), K part(s) — S structure(s), G ground(s), R seen; cells declared C of A play space`, with `0 plot(s) over N node(s)` on a graph declaring none; a test asserts both.
4. The grammar's contract accepts `part` on a space and on a `no_body` region; `contract-well-formed` refuses `part` on an edge; the document version moves; `grammar.md` §2d states the key; a test asserts each.
5. `DW-new-a-part-not-built-as-declared` is raised by `delvec detail --check` and by the build for the four shapes of §5.3 — unbuilt, undeclared (a part on a piece whose node has no `holds[]` included), not walked, out of size — naming the part, its use, the shape and the numbers, and not for a `reached: false` part no flood reaches; asserted by a test over a two-box plan whose plot's piece binds three parts; listed in `compiler.md`; bound by the three probes of §11.
6. `validation/plot-program.json` carries, per plot, every field of §5.4; the index prints `plot program: P plot(s), K part(s) built B of K; coverage min–max %, remainder min–max %`; `detail --check` prints `parts: B of K bound, W of R walked`; a test asserts the ledger over the gallery's site-plan point and that moving one claimed cell between two parts moves two `built` values by one.
7. `delvec detail` hands the node's parts into the expansion; a test asserts the handed set equals the node's `holds[]` on `node/exit` and is empty on `node/annex`.
8. `scripts/handout.py` writes *What this plot holds* for a node with `holds[]` and not for one without, with one row per part carrying id, kind, use, cells and share, height where declared, walked or seen; the binding sentence; the four placement lines; the frame's sky-open sentence; and a neighbour's parts by use; its binding counts include `parts`; `tools/tests/test_handout.py` asserts each on a fixture plot of three parts and on a fixture with none.
9. `render-plan.json` carries one `threshold` shot per part per crossing of every detailed plot, eye and aim as §6.2 states, `expect` naming the part's use; the index prints `threshold shots: T over P place(s), D door(s), O open side(s), Q part(s)`; a `crates/delvec` test asserts `Q` over the gallery's site-plan point and `0 part(s)` on a plan with no plot.
10. `references/visual-review.md` states the fourth blind line for a part frame and the `holding misses H` count; `SKILL.md` step 12 names it; `check-skill-page.py` stays green.
11. `references/placement.md` states the two bullets of §8 and no longer states *one box per building*, *a yard is its own place*, *12 × 12* or *14 × 14*; `grep -c` over the page for each of the four is stated with the file as denominator and is zero; `check-skill-page.py` stays green; spec-0123 §1, §2, §4 and criterion 1 read as §8 states.
12. `references/detail.md`'s contract states `part` and the four shapes of §5.3 and names `place-craft.md` §8; `check-skill-page.py` stays green.
13. The gallery's `node/exit` declares the three parts of §11 and binds them from `programs/exit.json`; `detail --check` exits 0; the whole builds; the ledger lists three parts built inside the tolerance; the validation line reads `1 plot(s) over 10 node(s), 3 part(s) — 1 structure(s), 2 ground(s), 1 seen`; three part frames are drawn; the eight probes of §11 red with their named codes under `check-gallery-coverage`; the two perturbations move the bytes they name.
14. `docs/demo-levels.md` carries the row of §12; `docs/reference/compiler.md` and `docs/reference/grammar.md` carry every change of §10 in the same PR; `docs/reference/tools.md` names the handout's section.
15. Executed once on the demo of §12 through step 9's loop as spec-0122 §6.3 runs it: four plots planned with `holds[]` before their boxes, each built by a fresh author from its handout, the whole ladder green with `0 under 0` in the `DW0210` line, the part frames drawn and read blind, and the ledger's coverage, setbacks and gaps in the commit body that lands this spec, with the L-shaped dwelling's two rooms, shed and yards each bound and built inside the tolerance.
