# spec-0072: The drawing — a place's detail as an ordered list of solids the engine executes

- **Status**: Draft
- **Ground**: written against engine `d92ba25a` (the branch that carries
  ADR-0030 as a draft), read only — `crates/delvec/src/grammar/`
  (`ir.rs`, `model.rs`, `expand.rs`, `export.rs`, `settle.rs`, `gates.rs`,
  `contract.rs`, `document.rs`, `version.rs`, `rng.rs`, `cli.rs`),
  `crates/delvec/src/schem/stairs.rs`, `crates/dsl/src/blocks.rs`,
  `crates/dsl/src/prefab.rs`, `crates/delvec/src/detail.rs`,
  `crates/delvec/src/compiler/detail.rs`, `prefabs/invariants/src/connections.rs`,
  `tools/spike-block-settling/`, `tools/ci/check-gallery-coverage.py`,
  `docs/reference/grammar.md` §2, §2b, §2d, §4b, spec-0036, spec-0042,
  spec-0050, spec-0058 — and against the two generators ADR-0030 measured:
  the released castle's (content repository `7318202`,
  `design/programs/build_castle.py` and `castle/*.py`) and an unreleased
  campaign's, read as a copy. Nothing was built or run except `delvec schema`
  and the `--help` of the verbs named. Every game fact below is marked
  **measured** (it has a replay test in the tree), **read** (a reading of
  vanilla's code that nothing re-checks) or **to measure**; nothing is cited
  from memory. The shape of this document is spec-0068's.
- **What it is for**: a creator who wants a specific building writes down the
  solids it is made of, in the order a builder would raise them, and the
  engine builds exactly that — with the corners of its stairs settled, its
  gate declared by the operation that paints it, and every mistake refused at
  the operation that made it.
- **Scope**: ADR-0030 §§1–4 and §6 — the document, its operations, its reuse
  constructs, the executor, derived block state, anchors and regions as
  operations, the `grammar` operation, and the verbs that execute one drawing
  into the prefab path that exists. **Not here, and named as the next specs**:
  ADR-0030 §5 (buildings, one owner per cell, the allocation of net cells,
  the fit-out refusal) and §7 (the authoring-time spatial queries). §9 says
  what §5 will bind and why no field of a drawing moves when it does.
- **Numbers**: no spec or ADR beyond this one. **Seven new DW codes**,
  `DW0902` … `DW0908` (§8). **`dsl_version` moves**, once, in the change that
  lands the document (§2.1); the number is the authority's
  (`crates/dsl/Cargo.toml`) and is not restated here. **The grammar-program
  ledger does not move**: the drawing reuses the program document's types and
  adds nothing to a program.
- **Non-goals**: a host language, a loop over data, a function that returns a
  value, recursion; reading the canvas inside an expression; a second
  expression algebra; a seed for geometry; block-by-block import of a finished
  build (that is `delvec schem`); any refusal of a partition-shaped `Program`
  (ADR-0030 §8 leaves it undecided and so does this).

## 1. The finding, from the tree

**Read.** A place's detail is bound today at one address,
`programs/<place stem>.json` (`crates/delvec/src/detail.rs`, `PROGRAMS_DIR`),
and the only thing that can stand there is a box-split `Program`. Both
campaigns that built a designed castle wrote a painter in Python and
serialised its grid into that program; ADR-0030's context carries the
measurement. Three facts about the tree decide what the drawing has to be:

1. **Everything after the blocks already takes an expansion, not a program.**
   `gates::judge(&Expansion, …)` reads a `VoxelModel`, the declared anchors
   and the resolved contract; `contract::check`, the stair and fluid gates
   (`settle.rs`), the light probe and `detail`'s steps 4–9 read the same
   things. Only `export_prefab` / `export_zone` take a `&Program`, and they use
   it for four things: to call `expand`, to hash it, to write its provenance
   row and to copy `shown_faces` through. A second producer of an `Expansion`
   therefore inherits every gate the engine has.
2. **`delvec detail` refuses a piece with no spatial contract** (`DW0843`,
   spec-0058 §3), and answers a seam through a declared exterior face
   (`DW0844`). A drawing that is bound where a program is bound owes the same
   contract, so it needs the two things a program says it with: a `contract`
   block and a way to name a box (`claim`).
3. **A gate is already declarable without touching a manifest.** A `barred`
   edge names a `bar` region some rule claims; a `mark` inside that region
   exports with `resolves_to: "bar:<region>"`, and `PrefabMeta::gate_anchor`
   reads that as the gate (`crates/dsl/src/prefab.rs`,
   `grammar/contract.rs::resolves_to`). The generator that wrote its gate
   regions into the manifest after every expansion did so because its
   generated program declared no contract at all — not because the surface had
   none. ADR-0030's sentence is corrected in §11.

## 2. The document

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph and
ADR-0030 §1.

### 2.1 What it is, where it lives, what numbers it

A drawing is one JSON document per place, at
**`drawings/<place stem>.json`** inside the campaign — the address derived from
the place exactly as `programs/<place stem>.json` is (spec-0058 §2.1). It is
executed in the place's box, in the box's own frame: `x` east, `y` up, `z`
south, the origin at the box's minimum corner, every coordinate a cell.

```json
{ "dsl_version": "<the engine's one number>",
  "name": "gatehouse",
  "params":  { "handed/datum-y": 1, "bay": 5 },
  "palette": { "wall":  [ { "weight": 10, "block": "minecraft:stone_bricks" },
                          { "weight": 2,  "block": "minecraft:cracked_stone_bricks" } ],
               "tread": "minecraft:stone_brick_stairs[facing=north,half=bottom,waterlogged=false]",
               "bars":  "minecraft:iron_bars[waterlogged=false]" },
  "defines": { "loop": { "params": { "h": 2 }, "roles": ["stone"], "body": [ … ] } },
  "contract": { "entry": "passage", "spaces": { … }, "edges": [ … ] },
  "shown_faces": ["north"],
  "ops": [ … ] }
```

- **It is a campaign document, so it carries `dsl_version`** and is refused at
  any other number (`DW0102`, ADR-0024). It is not a `Stage`: a stage is one
  file named `<stage>.json`, and a campaign holds one drawing per place. It is
  exported by `delvec schema --stage drawing` and is **part of `--stage all`**,
  because `all` is the enumeration of what a creator can write and the gallery
  coverage gate reads its units from it (§10). That is the difference from
  `prefab-metadata`, which `all` leaves out because no creator writes it.
- **`dsl_version` moves**, once, in the change that lands the document: the
  schema export gains a document class, and ADR-0024 makes any change to that
  export a move of the one number. Every document the repository holds moves
  with it, as for any surface change.
- **The types are shared, not copied.** `Expr`, `Cond`, `Paint`, `Mark`,
  `Contract` and `BlockState` are the program document's
  (`crates/delvec/src/grammar/ir.rs`, `block.rs`). The drawing uses them by
  reference and they gain a `JsonSchema` derive, which moves no byte of any
  program. The drawing's own types live beside the grammar's, in
  `crates/delvec/src/drawing/`, because that is where the shared types are and
  `delvewright-dsl` cannot depend on `delvec`; the schema export is produced in
  the binary, which can reach both. No private copy of a shared type exists in
  either direction.

### 2.2 Header fields

| Field | Meaning |
|---|---|
| `name` | provenance label, as a program's |
| `params` | name → integer. A declaration and a default, read by `{"expr":"param"}`. The `handed/…` names of spec-0058 §2.3 are declared here and bound by `detail` exactly as they are for a program; `DW0882` applies unchanged |
| `palette` | role → `Paint` — a block-state string or a weighted list. **Every state in a drawing is written in the frame of the scope that paints it** (§5.2); there is no world-frame spelling and no `local` wrapper |
| `defines` | name → `{params, roles, body}` (§4.2) |
| `contract` | the program document's `Contract`, verbatim (§6.2). Required wherever `detail` binds the drawing (`DW0843`) |
| `shown_faces` | as a program's (`DW0885`), written through to the piece |
| `ops` | the ordered list. **A later operation overwrites an earlier one** |

An integer position or size anywhere in an operation is an **`Int`**: a JSON
integer, or an `Expr` object of the existing algebra (`grammar.md` §2:
literals, parameters, the current scope's extents, `+ − × ÷ % max min`, floor
division and Euclidean remainder). A bare integer is one more spelling of
`{"expr":"int"}` at the drawing's own fields; inside an `Expr` the operands
are the algebra's own tagged form. There is no second algebra, no string
syntax and no float.

## 3. The operations that paint

**Authored.** Six solids. Every one carries:

| Field | Meaning |
|---|---|
| `role` | a palette role, or the reserved role **`air`** |
| `from`, `to` | the op's box in its scope, inclusive corners, each `[Int, Int, Int]`. `to` omitted is one cell; both omitted is the whole scope |
| `where` | optional list of roles. The op overwrites only cells whose **current** role is listed. `air` is a role here; cells a `grammar` operation wrote answer to the reserved name `grammar` |
| `when` | optional guard — the `cmp` / `all` / `any` / `none_of` members of `Cond`. False: the op does nothing. It is how a `define` says *not at this size* |
| `note` | one line for a reader. The engine never reads it |

`where` reads the cell being painted and no other. It is what makes
*replace role A by role B inside a box* and *only where nothing stands yet*
properties of every solid instead of a verb of their own.

### 3.1 ADR-0030 §2, operation by operation

| ADR-0030 names | Here | Reason |
|---|---|---|
| `box` (solid, hollow, faces) | **`box`** with optional `faces` + `t` | specified, §3.2 |
| `clear` | **struck** — `role: "air"` on any solid | a verb that clears only boxes leaves a round room with nothing to clear it; the role reaches every solid |
| `replace` | **struck** — `where` on any solid | the capability belongs to the act of painting, not to one box-shaped verb |
| `cylinder`, `disc` | **`cylinder`**; `disc` struck | a disc is a cylinder one cell long; a second verb for one solid is the defect |
| `sphere`, `dome` | **`sphere`**; `dome` struck — `flat` | a dome is the half of a sphere whose cut lies on the box's floor; `flat` also gives the apse and the barrel vault from `cylinder` |
| `prism` | **`prism`** | specified, §3.4 |
| `pyramid` | **`pyramid`**, with `section: "round"` for the cone | specified, §3.4; a round tower's roof is the same solid with a round course |
| `flight` | **struck** — `repeat` with a step vector (§4.3) | a flight is a building part: a wedge of mass, and a tread with the air over it stamped along a diagonal. It is a `prism` and a `repeat`, and written once as a `define` it is one `use`. The engine's knowledge about stairs is the derived `shape` (§5.3), which reaches a stair however it was painted |
| `line` | **`line`**, with an optional `brush` | specified, §3.5 |

Added beyond ADR-0030 §§2–3, each with its reason: `where` and `when` (above);
`scope` (§4.1); the step **vector** and the **index** on `repeat` (§4.3);
`roles` on `use` (§4.2). Nothing else.

### 3.2 `box`

Write `n = (nx, ny, nz)` for the box's extents and `i = (ix, iy, iz)`,
`0 ≤ i_a < n_a`, for a cell in it. Absent `faces`, every cell is in. With
`faces` — a non-empty subset of `west east down up north south`, the low and
high faces of `x`, `y`, `z` in that order — and thickness `t` (default 1), a
cell is in when it lies within `t` of a listed face: `i_a < t` for a low face
of axis `a`, `i_a ≥ n_a − t` for a high one. All six faces is the hollow shell;
the four horizontal ones are a wall ring.

### 3.3 `cylinder` and `sphere` — one integer rule

`cylinder` takes `axis` (`x`, `y` or `z`); its two other axes are **round**.
`sphere` has three round axes. Either takes an optional `flat`, a face of the
box on a round axis: the solid is then the half whose cut plane lies on that
face. For each round axis `a`:

| | `u_a` | `D_a` |
|---|---|---|
| not flattened | `2·i_a + 1 − n_a` | `n_a` |
| `flat` is `a`'s low face | `2·i_a + 1` | `2·n_a` |
| `flat` is `a`'s high face | `2·(n_a − 1 − i_a) + 1` | `2·n_a` |

**A cell is in when `Σ_a ( u_a² · Π_{b≠a} D_b² ) ≤ Π_a D_a²`**, the sums and
products over the round axes. That is the ellipse or ellipsoid inscribed in
the box, tested at cell centres in doubled coordinates, so an even extent and
an odd one obey one rule and nothing is ever halved. The arithmetic is exact:
a round axis may not exceed 4096 cells (`DW0905`), which keeps every term inside
128 bits.

With `t`, the solid is a tube or a shell: a cell is in when it is in the
solid above and **not** in the same solid taken over the box shrunk by `t` at
every round face that is not the `flat` one. A shrunk box with no cells
removes nothing.

For an odd width `n = 2r + 1` the rule reduces to `dx² + dz² ≤ r² + r`, which
is the disc one of the two generators wrote for its rose window
(`d2 <= radius * radius + radius`); the other generator's corner rounds use
`dx² + dz² ≤ 16` for `r = 4`, which is a different disc (§7).

### 3.4 `prism` and `pyramid` — courses that step in as they rise

Both take `rise` and `run` (each an `Int ≥ 1`, default 1): the solid steps in
`run` cells for every `rise` courses. For the course `k = iy`,
**`inset(k) = ⌊k · run / rise⌋`**.

- **`prism`** takes `taper`, the horizontal axis (`x` or `z`) the span narrows
  along — the ridge runs along the other — and `sides`: `both` (default),
  `low` or `high`, the faces of that axis that step in. With `c` the cell's
  coordinate on `taper` and `n` the box's extent on it, the course spans
  `lo(k) ≤ c ≤ hi(k)`, where `lo(k)` is `inset(k)` on a stepping low side and
  `0` otherwise, and `hi(k)` is `n − 1 − inset(k)` on a stepping high side and
  `n − 1` otherwise. `both` is the gable; one side is the wedge, the buttress
  and the lean-to.
- **`pyramid`** steps in on all four horizontal faces by the same `inset(k)`.
  With `section: "round"` each course is the ellipse of §3.3 inscribed in that
  course's rectangle: the cone.
- A course whose span is empty has no cells. The box's height truncates the
  solid: a batter is a pyramid whose box ends before it closes.
- **`skin: true`** keeps only the surface: a cell of course `k` that lies on a
  stepping edge of course `k`'s own span, or that course `k + 1`'s span —
  computed by the same formula, whether or not the box reaches it — does not
  cover. With `rise = run = 1` that is one ring per course, which is the spire
  the second generator wrote; with `rise = 2` it is that generator's
  bell-tower spire.

A roof whose slopes are stair blocks is two `prism`s with `skin`, `sides:
low` and `sides: high`, each painting a stair role that faces up its slope.

### 3.5 `line`

From cell `A` to cell `B` (`from`, `to`; here `to` is an end point, not a
corner), with `d = B − A` and `n = max(|dx|, |dy|, |dz|)`. For `i = 0 … n` the
point is **`A_a + ⌊(2·i·d_a + n) / (2n)⌋`** on each axis, the division
flooring toward −∞; `n = 0` is the single cell. `brush: [bx, by, bz]`
(default `[1, 1, 1]`) paints a box of that size with its minimum corner on
each point. A half is always rounded up, so a line from `B` to `A` may differ
from the line from `A` to `B` by a cell; the rule is stated rather than
symmetrised.

## 4. The operations that arrange

**Authored**, against ADR-0030 §3. Each carries a `body` — a list of
operations — and `when` and `note`. None paints.

### 4.1 `scope`

`{ "op": "scope", "from": …, "to": …, "body": [ … ] }` — the body's
coordinates are local to the box, its extents are what
`{"expr":"dim"}` reads, and an operation that reaches outside it is refused
(`DW0902`). It exists so that a block of operations moves as one when the plan
moves it, and it is the carrier the three constructs below share: **every
body-bearing operation takes `from` / `to` and makes that box its body's
scope.**

### 4.2 `define` / `use`

A define is `{ "params": {name: default}, "roles": [names], "body": [ … ] }`.
`use` instantiates it:

```json
{ "op": "use", "define": "loop", "from": [12, 9, 0], "to": [12, 11, 1],
  "turn": 1, "mirror": "x",
  "params": { "h": 3 }, "roles": { "stone": "dressed" } }
```

- The body runs in the `use`'s box under a **frame**: `mirror` (`x` or `z`,
  optional) reflects the body across the centre plane of its own local box,
  and then `turn` (0–3) turns it that many quarter-turns clockwise seen from
  above. A local cell `(lx, y, lz)` in a local box of extents `(Lx, Ly, Lz)`
  lands at:

  | `turn` | world `x` | world `z` | world extents |
  |---|---|---|---|
  | 0 | `lx` | `lz` | `(Lx, Ly, Lz)` |
  | 1 | `Lz − 1 − lz` | `lx` | `(Lz, Ly, Lx)` |
  | 2 | `Lx − 1 − lx` | `Lz − 1 − lz` | `(Lx, Ly, Lz)` |
  | 3 | `lz` | `Lx − 1 − lx` | `(Lz, Ly, Lx)` |

  Frames compose as signed permutations of the two horizontal axes. The
  vertical never moves: a drawing has gravity.
- `params` may name only parameters the define declares, `roles` only roles it
  lists (`DW0903`) — closed, as `bind` is. A body reads its own parameters, the
  indices bound around it, and the document's `params`; nothing else.
- A define may `use` another. **A define that reaches itself, directly or
  through others, is refused with the chain named** (`DW0904`), at validation,
  before anything executes. There is no recursion, so there is no depth to
  limit.

### 4.3 `repeat`

Two spellings of how many, one construct:

```json
{ "op": "repeat", "step": [0, 1, 1], "count": 10, "index": "k", "body": [ … ] }
{ "op": "repeat", "along": "x", "stride": 5, "item": 1, "remainder": "exact",
  "index": "k", "body": [ … ] }
```

- **Counted.** Instance `k = 0 … count − 1` runs the body with the scope's
  origin moved by `k · step`. The scope's extents do not change, and an
  instance that paints outside the enclosing scope is `DW0902` with `k` named.
  A step with a vertical part is what makes a flight, a stepped arch and a
  corbel table one operation each.
- **Fitted.** Items `item` cells long stand every `stride` cells along the
  scope's `along` axis: `n = ⌊(E − item) / stride⌋ + 1` of them, `E` the
  scope's extent, leaving `r = E − ((n − 1)·stride + item)` cells over. Each
  instance's scope is its own `item`-long slice. **`remainder` is required**
  and says where `r` goes — `exact` (there must be none), `start`, `end`,
  `middle` (the lower-middle split), the words `split`'s `rounding` already
  uses. A missing `remainder` is refused by the schema at the operation
  (`DW0100`); `exact` with cells left over, or a run that fits no item, is
  `DW0906`, naming `E`, `stride`, `item` and `r`.
- `index` binds the instance number as a parameter for the body — what lets a
  course step in as it rises, or alternate two roles by `k % 2`. The
  grammar's `repeat` cannot know how far along it is (`grammar.md` §2); a
  drawing's can, because a drawing has no seeded stream for the index to
  disturb.

A flight of ten treads climbing north, three wide — the wedge of masonry it
stands on, then a tread and the air over it, stamped up the diagonal:

```json
{ "op": "prism", "role": "step", "taper": "z", "sides": "high",
  "from": [0, 0, 0], "to": [2, 9, 9] },
{ "op": "repeat", "step": [0, 1, -1], "count": 10, "body": [
  { "op": "box", "role": "tread", "from": [0, 0, 9], "to": [2, 0, 9] },
  { "op": "box", "role": "air",   "from": [0, 1, 9], "to": [2, 3, 9] } ] }
```

Written once as a `define`, it is one `use` per flight, turned to climb any
way.

### 4.4 `mirror`

`{ "op": "mirror", "axis": "x", "from": …, "to": …, "body": [ … ] }` runs the
body, then runs it again reflected across the centre plane of the operation's
box on `axis`: a cell at `c` lands at `n − 1 − c`, exact for odd and even
extents alike. States reflect with it (§5.2). A `mark` inside takes
`index: "auto"` or is refused as a name written twice (`DW0903`).

### 4.5 `grammar` — the grammar stays for what it was adopted for

```json
{ "op": "grammar", "program": "programs/colonnade.json", "rule": "arcade",
  "from": [4, 1, 20], "to": [27, 7, 22], "seed": 7,
  "params": { "bay": 4 }, "roles": { "stone": "minecraft:andesite" } }
```

The named program — a file, resolved by the program loader with its `include`
list and its path rules (`document.rs`) — is expanded into the operation's
box, from `rule` (default: the program's `start`), at the **literal** `seed`
the operation writes. Which variant is a judgement, so it is an argument;
nothing derives it. The expansion writes through into the drawing's canvas:
the program's `fill` and `void` overwrite, its `skip` leaves what was drawn.
Its marks join the drawing's anchors; its claims join the drawing's regions,
and the drawing's `contract` classifies them, which is the rule an included
program already lives under (`grammar.md` §5c). The expander's refusals are
printed in its own words under the operation's address. **The grammar does
not call drawings.**

## 5. The executor

**Authored**, except where marked.

### 5.1 Order, and what determinism rests on

Operations run in document order; a `repeat`'s instances in ascending index;
a `mirror`'s unreflected body first; a `use`'s body in order. The canvas holds,
per cell, the role last painted and the frame-resolved paint it names. After
the last operation, in this order:

1. **Weighted paints are drawn, by position.** A cell whose paint is a
   weighted list takes the draw `out(n) % total`, where `n` is the cell's
   linear index in the place's box (`VoxelModel`'s own order) and `out(n)` is
   the `(n + 1)`-th output of the engine's one generator, splitmix64
   (`grammar/rng.rs`), seeded with the execution's seed — which is a pure
   function of the seed and `n`, so no stream is consumed and **editing one
   operation re-textures no other cell**. A list of one member draws nothing.
2. **Derived state is written** (§5.3).
3. The result is an `Expansion`: the `VoxelModel`, the anchors, the resolved
   contract, the derivation's counts. Everything downstream is what exists.

The seed is `detail`'s place-derived seed (spec-0058 §2.4) or
`--seed` (default 0) under `delvec drawing execute`. **Geometry has no seed**:
the seed reaches weighted paints and nothing else. No clock, no environment,
no hash-ordered container; two executions are byte-identical (ADR-0006).

### 5.2 Every state is written in the scope's own frame

A state in a drawing's palette is resolved into the world through the frame of
the scope that paints it, by the one resolver the grammar's `local` paint uses
(`BlockRegistry::permuted_properties`), and refused where that resolver cannot
determine an image (`DW0738`, in its own words, under the operation's
address). At the top level the frame is the identity and the state is what
was written. So a `define` that paints `facing=north` stairs faces wherever
its `use` turns it, and `DW0736` / `DW0742` are unreachable from a drawing: no
world-frame literal can be written in one. The `oriented-fills` gate binds to
every painting instance the executor ran, all of them resolved from the frame.

**One extension is owed to that resolver, and it is arithmetic, not a game
fact.** `property_image` determines a 16-step `rotation` and a handedness
(`hinge`, a chest's `type`) only under the identity and the bare `x↔z`
transposition; every `turn` of §4.2 is a transposition *with* a reversed axis,
so today a door or a standing banner inside a turned `use` is refused. For the
eight frames that keep the vertical the images are exact: a frame of
determinant +1 (a turn by `t` quarter-turns) sends a yaw `r` to
`(r + 4t) mod 16` and keeps left and right; a frame of determinant −1 sends
`r` to the reflection of its direction and swaps them. The classifier gains
those cases in place — one classifier, read by `DW0736`, `DW0738` and the
drawing alike — and the grammar's `Reorient::turned` gains them with it.

### 5.3 Derived block state

- **A stair's `shape` is the engine's — measured.** After the last
  operation every stair cell, whatever wrote it, takes the shape
  `schem::stairs::derive_shape` gives it from its four neighbours inside the
  place's box, a cell outside the box reading as no stair (`settle.rs`'s own
  rule). That function is replayed cell for cell against a field placed and
  settled on the pinned server (`tools/spike-block-settling/`,
  `crates/delvec/tests/schem_stair_shape_measured.rs`); the drawing calls it
  and owns no second rule. **A drawing's paint that writes `shape` on a stair
  is refused** (`DW0907`) rather than silently overwritten, so `DW0801` is
  unreachable from a drawing.
- **Connection state — fences, walls, panes, bars — is to measure, and until
  it is, a drawing writes it.** The tree holds a derivation,
  `prefabs/invariants/src/connections.rs` (`resolve`): a **reading** of
  vanilla's `FenceBlock.connectsTo`, `IronBarsBlock.attachsTo` and
  `WallBlock.connectsTo`, resting on a hand-declared table of 25 full cubes
  because `isFaceSturdy` is published nowhere, refusing every neighbour it
  was not told about, and replayed against nothing. It lives in the
  generators' workspace, where `delvec` cannot reach it. `observations.json`
  holds a stair field and water rigs and no connection rig. The released
  gatehouse stands a rack of iron bars against `dark_oak_planks`, which is
  not in that table, so the derivation as it stands would refuse the port at
  its first three cells. So:
  1. The spike gains a connection rig (§12, criterion 2): for one
     representative of each class — `oak_fence`, `nether_brick_fence`,
     `iron_bars`, `cobblestone_wall` — placed beside **every block of the
     pinned registry in its default state**, every state of every slab, stair
     and trapdoor, and every other connecting block, the settled state is read
     back; and a wall's `up` and `low`/`tall` under each of those from above.
     The same rig answers the second question: **does a written connection
     survive** — a state placed literally, then given a block update beside
     it, read back.
  2. The derivation moves into `crates/dsl` beside `fluid` — block knowledge
     lives where every reader can reach it — with the measured support table
     as data in place of the declared one, replayed by a test in the shape of
     the stair one. `prefabs/invariants` re-exports it; there is one rule.
  3. From then on a drawing's paint **omits** the properties that rule
     derives and the executor fills them, after stair shapes, because a
     stair's face is one of the things a fence joins. A written value stands,
     which is `resolve`'s rule today — and whether it *may* stand is what the
     second measurement decides: if the pinned server re-derives a written
     connection the way it re-derives a stair's shape, the general form is a
     gate beside `stair-shape`, and it is a finding for its own spec, not a
     rule invented here.
  4. **In the meantime** — between this spec landing and criterion 2 — a
     connecting block's paint writes every connection property, as every
     producer does today, and `DW0735` binds it as it binds a program.
- Every property that is neither of those is written by the author in full,
  and `DW0737` binds as it does for a program. Filling an omitted `facing`
  from the block's default would hide the mistake the code exists to show.

## 6. Anchors and regions are operations

**Authored**, against ADR-0030 §4 and the reading in §1.

### 6.1 `mark`

`{ "op": "mark", "mark": { … } }` carries the program document's `Mark`
verbatim — `anchor`, `at` and its four forms, `facing`, `index`, `role`
(`grammar.md` §2b) — evaluated against the scope it is written in and put
through that scope's frame, a written `facing` included. It writes no block.
It lands in `Expansion::anchors` and reaches the prefab metadata through
`anchor_metadata`, the path that writes every grammar anchor today.

Whether a body can *stand* at a mark is not refused here: what an anchor is
for is known only to the campaign that binds it, and the codes that judge it
(`DW0314`, `DW0850`, the bodies' clearance) read the assembled world. What is
knowable here is already a gate — `contract-anchors`: every mark lands in a
declared contract element.

### 6.2 `claim`, and the contract

`{ "op": "claim", "region": "gate", "from": …, "to": …, "body": [ … ] }` names
its box, as the program's `claim` does, and runs its optional body inside it.
Several claims of one name union. The header's `contract` says what each name
is — a space, an out-of-walk region, an edge's `via`, a `bar`, a `way`
(spec-0036, spec-0042) — and is checked by `contract::check` unchanged.

**ADR-0030's `region` and `way` operations are this one operation.** A gate a
story opens is a `barred` edge whose `bar` names a claimed region and a
palette role; the operation that claims the box is the operation that paints
it, so the gate's cells are typed once:

```json
{ "op": "claim", "region": "portcullis", "from": [10, 1, 3], "to": [14, 5, 3],
  "body": [ { "op": "box", "role": "bars" },
            { "op": "mark", "mark": { "anchor": "gate", "at": "floor_center" } } ] }
```

A laid bridge or a cleared rockfall is a `way` on its edge, the same way. A
second, anchor-level spelling of a gate (`region` + `block` on the mark) is
not offered: `gate_anchor` refuses a piece whose two spellings disagree, and a
surface that cannot disagree is better than a refusal.

## 7. What the two generators needed — the early warning

**Measured by reading**, one row per `def` of both generators; the rubric is
§3–§6 exactly. *Dissolves* means the function exists only for the partition
encoding, the palette plumbing, file output, a private copy of an engine
judgement, or a derivation §5.3 owns — there is nothing left to express.

**Instrument and strength.** One reader per generator enumerated the `def`s
with `grep -n "^def \|^    def \|^class "` and classified each against the
rubric above; the gatehouse file, the vocabulary file and every hash and
canvas-read site were read a second time for this spec. The counts are one
pass, not cross-checked by a second method, and are handed on as numbers to
verify. Cell counts quoted in the rows were taken by running the generators in
memory, writing nothing.

| | defs | dissolve | literal operations | `define` / `use` / `repeat` / `mirror` | **not sayable** |
|---|---|---|---|---|---|
| released castle (`build_castle.py`, `castle/*.py`) | 82 | 39 | 14 | 19 | **10** |
| unreleased campaign's generator | 89 | 28 | 18 | 33 | **10** |
| **both** | **171** | 67 | 32 | 52 | **20** |

**Bottom line: 151 of 171 functions are sayable or have nothing left to say;
of the 104 that paint anything, 84 are sayable. All 20 that are not are
ground and weathering. No function of either generator that builds a room, a
stair, a wall, a roof, a gate or an anchor is among them, and no function of
`castle/gatehouse.py` is** — the building criterion 10 ports.

What dissolves, by kind: the split-tree serialiser and the column-span
machinery (both `build_castle.py` files, `Col`, `lay`, `column_node`); palette
and canvas bookkeeping (`Palette`, `state`, `stairs`, `slab`, `block`, `Grid`'s
storage); box and stride predicates whose painter becomes a box or a `repeat`
(`inside`, `on_road`, `crenel`, `wall_face`, `in_curtain`, `murder_hole`,
`pit`); `settle_stairs` (§5.3); the manifest edit for gates (§6.2); and one
private checker — *does this anchor have room to stand* — which dissolves into
ADR-0030 §7 and is therefore **not** dissolved by this spec alone.

The second generator's whole vocabulary maps: `room` is four boxes in a
`define`; `crenels` and `machicolate` a `repeat` under a `mirror`; `flight` the
`repeat` of §4.3; `gable` two skinned one-sided `prism`s and two end planes;
`pyramid` the `pyramid` skin and a cap cell; `lancet` two boxes and a `when`;
`rose` a `cylinder`, a smaller one and four `line`s under `where`; `tower` a
`define` of those. `machicolate` is exact under `mirror` and not under `turn`
(its merlon phase reverses on an even span), which is a fact about that
function, not a defect.

The rows that are not sayable, each with what it costs:

| Function | Why | Cost |
|---|---|---|
| `grounds.py::_hash`, `_meadow`, `_joint` | a positional hash keyed to a 2×2 patch: clumped ground cover and turf joints | the density is a weighted paint; the **clumping is lost**. The generator's own comment gives partition merging as half the reason for the clumps, and that reason is gone |
| `grounds.py::courtyard_column` | a tuft stands only **over** a turf joint — a test of the cell below | `where` reads one cell. 115 tufts are typed or dropped |
| `grounds.py::_tree_centre`, `_tree_span`, `outside_column` | hashed presence, position, species and trunk height of a multi-cell object | an oak and a spruce `define`, and 21 hand-placed `use`s — which is what a designed site does with its trees |
| `grounds.py::is_parapet`, `is_rail`, `wall_column` | *a parapet wherever a neighbouring cell is off the wall*: a predicate over other cells | the author types the result: 156 parapet cells in 13 runs, 142 rail cells in 12 |
| second generator: `crag`, `outcrop`, `trees` | float-scaled hash heights correlated over 2×2; hashed tree positions | the largest loss — the look of the rock. Boulders as explicit solids, or a plain plinth in a weighted paint |
| `shrine`, `ring` | a random height per wall column; ruin notches 1–4 deep, and corbels placed only under a wall top that survived (a read of a different cell) | the height profile typed as runs (about 46 columns; 143 notch columns), after which the corbels follow by order |
| `chapel`, `throne_hall`, `stables` | a hashed hole cut through the **whole** roof column — one draw, several cells | a few explicit `air` boxes. A per-cell weighted `air` is not the same thing: it leaves speckle, not holes |
| `great_hall` | a float ellipse with a hashed ragged edge — the fallen roof | an `air` `cylinder`; the ragged edge is lost |
| `pool` | ceiling height minus 0, 1 or 2 by hash | minus one is a weighted top course; minus two is dropped |

Two things that are sayable and **not byte-equal**, recorded so nobody
discovers them in a port. A disc written as `dx² + dz² ≤ R` for an arbitrary
`R` (`≤ 16` for the released castle's corner rounds, `≤ 26` for two openings
of the second generator) is not the inscribed disc of any box; it is exact as
rows of boxes and approximate as a `cylinder`. And the second generator's
rose-window rim is the inscribed ring only at radii where no cell has
`d² = r(r − 1)` — which includes `r = 6`, the one radius it is called with.
Neither occurs inside the gatehouse's box.

One thing the reading found that is not about expressibility. **By the
unmeasured reading of §5.3, 40 of the 106 connection-bearing cells in the
released gatehouse are written differently from what their neighbours would
derive** — ten table legs and three stacked spears written as isolated posts
in a row, a folded iron gate written free of the wall it stands against, four
rail ends written joined to nothing. Most are deliberate. They are why a
written connection must be allowed to stand until the measurement says whether
the game lets it (§5.3, item 3), and why criterion 10 compares states, not
intentions: the port writes those cells as the generator did.

## 8. Refusals, where entered

**Authored.** Every refusal is addressed by the **operation's path in the
document the creator wrote** — a JSON pointer, `/ops/12/body/3` — followed,
inside a define, by the chain of uses that reached it
(`/defines/tower/body/4 ← /ops/7`), the `repeat` indices in force (`k=3`) and
the `mirror` side. A cell coordinate is added where there is one; it is never
the address.

| Code | Refuses | Fires | Says |
|---|---|---|---|
| `DW0100`, `DW0102` | a document that does not parse — an unknown `op`, a missing `remainder`, a `turn` of 4; a `dsl_version` that is not the engine's | load | the path and the schema's own words |
| **`DW0902`** | **an operation that reaches a cell its scope does not hold** — a solid's box, a `line`'s brush, a `mark`, a `claim`, a `grammar` box, outside the place's box or the enclosing `scope` / `use` / `repeat` slice | at that operation, before it paints | the operation, the scope it left, both boxes |
| **`DW0903`** | **a name that resolves to nothing, or to two things** — a `role` (on a solid, in `where`, in `use.roles`, in the contract's `block`), a `define`, a parameter, a region the contract names and nothing claims or the reverse, an anchor two marks produce | validation; the anchor at the second mark | the kind, the name, every name of that kind the document declares |
| **`DW0904`** | **a `define` that reaches itself** | validation | the chain |
| **`DW0905`** | **a value outside its range at the operation that evaluates it** — `to` below `from`, a `t`, `rise`, `run`, `stride` or `item` below 1, a negative `count`, division or remainder by zero, arithmetic past 64 bits, a `flat` that is not a face of a round axis, a round axis over 4096 cells, a place past the expander's `max_volume`, more than the executor's instance ceiling | evaluation | the field, the value, the range, the parameters that produced it |
| **`DW0906`** | **a fitted `repeat` that does not fit** — `exact` with cells left over, or no whole item | evaluation | `E`, `stride`, `item`, `r`, and the three remainder words that would accept it |
| **`DW0907`** | **a paint that writes a property the engine derives** — `shape` on a stair; after criterion 2, nothing more unless that measurement says a written connection does not survive | validation | the role, the property, and that omitting it is the repair |
| **`DW0908`** | **a place with two media** — `programs/<stem>.json` and `drawings/<stem>.json` both present | `detail`, before either is opened | both paths |
| `DW0738`, `DW0735`, `DW0737`, `DW0882`, `DW0843`–`DW0845`, `DW0848`, the contract gates, the settle gates, the audit | what they refuse today | where they fire today, with the operation's address added wherever the executor knows it | their own words |

Not a refusal, and printed on every run: **operations written, instances
executed, cells painted, cells surviving to the model, and every written
operation none of whose instances painted a cell**, by address. An operation
that paints nothing is dead text in the document of record; it is listed, with
its count, rather than refused, because a `define` written for many sizes may
honestly have an operation with nothing to do at one of them.

## 9. Bound where a program is bound — and what §5 will bind

**Authored**, against spec-0058.

- **`delvec detail`** reads a place's medium from its address: a drawing at
  `drawings/<stem>.json`, a program at `programs/<stem>.json`, both is `DW0908`.
  Steps 1–9 of spec-0058 §2.2 are unchanged; step 3 executes instead of
  expanding. The `handed/…` names are bound into `params` (`DW0882`), the
  plan's palette rebinds the roles the drawing declares (spec-0058 §2.5), the
  seed is the place's, the row is the same row. `--all` walks both
  directories in site-plan order.
- **`delvec drawing check <file>`** validates without executing: the schema,
  every `DW0903` / `DW0904` / `DW0907`, the contract's reference integrity.
  **`delvec drawing execute <file> --region XxYxZ --out <dir> [--id] [--seed]
  [--param k=v] [--role r=state]`** executes, judges and freezes, printing and
  writing what `delvec grammar expand` prints and writes, `<id>.report.json`
  included. Both are subcommands of the one binary (ADR-0023).
- **The freeze is shared.** `export_prefab` / `export_zone` are split where §1
  says they are joined: a producer half that yields an `Expansion` (a program
  expanded, or a drawing executed) and a freezing half that takes the
  expansion, the provenance row and `shown_faces`. The provenance names the
  generator `delvec drawing`, the drawing's name, the SHA-256 of its canonical
  bytes together with those of every program file a `grammar` operation
  names, the seed, the region, and the parameters and roles overridden.
- **`delvec prefab diff <a> <b> --box x0,y0,z0,x1,y1,z1 [--at x,y,z]`** — new,
  general, and the instrument of criterion 9: block-state equality, cell for
  cell, of a box of piece `a` against the same-sized box of piece `b` at
  `--at` (default the origin), tiled pieces included, plus equality of the
  point anchors inside the box. It prints cells compared, cells differing and
  the first differing cells with both states; zero cells compared is a
  refusal; any difference is exit 1. It reads two pieces and nothing of any
  campaign.

**What ADR-0030 §5 will bind, and why the document does not move.**

| A later allocation hands | It binds to | Already shaped for it |
|---|---|---|
| the place's **net cells** instead of a box | the executor's notion of *the cells a scope holds*, which `DW0902` is written against | `DW0902` says *a cell its scope does not hold*, never *outside the box*; a mask is a narrower holding, handed as an executor input. No operation names its owner |
| per face, the **openings and seams** the place looks onto | `params`, under `handed/…` | it is spec-0058's prefix; new names are new keys |
| the split into a **building's** drawing and its places' | which document `detail` executes for which node | a building's base build and a room's fit-out are the same document class; nothing in a drawing says which it is |
| defines **shared** across a building's documents | `defines` | names are a keyed map a later `include` can qualify by prefix, as `grammar.md` §5c does for rules. Not built here |

## 10. What the gallery, the demo queue, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). The site-plan overlay details a third
  place through the verb: **`node/far-hall`**, from
  `gallery/overlays/site-plan/drawings/far-hall.json`, generated at build time
  by `tools/ci/gallery-prefabs.py` exactly as the annex is, the written row
  asserted equal to the committed one. The one drawing binds every unit the
  document class declares: each of the thirteen operations, every field of
  each, every variant of every enum (`faces`, `flat`, `sides`, `section`,
  `remainder`, `turn`, `mirror`), a weighted paint, a stair whose corner the
  executor derives, a `barred` edge whose bar is claimed by the operation that
  paints it, and a `grammar` operation naming the overlay's own
  `programs/annex.json` rules at a literal seed. **Bound by perturbation**:
  for each unit, one declared edit of the drawing moves a byte of the emitted
  piece, and a unit whose edit moves none is reported as a zero binding. The
  coverage gate walks every `drawings/*.json` of a materialised point against
  the `drawing` schema; where the class's files live is read from the export,
  not listed in the tool (`gallery_units.stage_files`'s own rule), and the
  gate's `detail` phase runs wherever a point carries `drawings/` as it does
  for `programs/`.
- **The probes.** One per new code, each the primary plus one declared edit of
  `far-hall.json`, refused by `delvec detail` with the code it names:
  `an-op-that-leaves-its-box` (`DW0902`), `a-role-nobody-declared` (`DW0903`),
  `a-define-that-uses-itself` (`DW0904`), `a-wall-thinner-than-one` (`DW0905`),
  `a-bay-that-does-not-fit` (`DW0906`), `a-stair-with-its-shape-typed` (`DW0907`),
  `a-place-with-two-media` (`DW0908`, which ships a program beside the drawing).
- **The demo level.** A row in `docs/demo-levels.md`: **The Drawn Gate** — one
  small gatehouse authored as one drawing and nothing beside it: a round
  tower and its cone, a domed niche, a gable skinned in stairs, a chain hung
  on a `line`, a parapet from a fitted `repeat`, a colonnade from a `grammar`
  operation, and a portcullis the story raises, claimed by the operation that
  paints it. What the level shows a stranger: the stairs' corners are right
  and nobody typed them; the gate opens and no manifest was edited; the
  campaign directory holds no script. Then the refusals, as transcripts.
- **The record.** A new live record, `docs/reference/drawing.md`: the
  document, each operation with its integer rule, the frame table, the
  executor's order, derived state and what was measured, the refusals — in the
  present tense, for the agent that writes drawings. `docs/reference/compiler.md`
  gains the seven codes, the document class in the surface section, and the
  execution binding line. `docs/reference/tools.md` gains `delvec drawing` in
  the `delvec grammar` chapter's neighbourhood, `delvec prefab diff` under
  `delvec prefab`, and the spike's connection rig under *Spikes*.
  `docs/reference/grammar.md` §2c stops being the route to a designed building
  and says where that route is. `docs/ACKNOWLEDGEMENTS.md` gains nothing: no
  algorithm is ported — the line rule is stated here in integers and credited
  to nobody because it copies no one's code.
- **The skill.** `.claude/skills/delvewright/skills/new-delve/references/detail.md`
  — the place-detail step — is rewritten around the drawing in the change
  that makes it work: write `drawings/<stem>.json`, run `delvec detail`, read
  the refusal at the operation it names. A computed `Program` stops being
  presented as a way to build a place (ADR-0030 §8). `tools-by-symptom.md` and
  `when-red.md` gain the new codes' rows, stating behaviour, since
  `tools/ci/check-skill-page.py` holds the page to the engine it pins.

## 11. Where ADR-0030 is corrected by the tree

Proposed as an edit to the draft ADR, in a commit of its own beside this spec.

1. **Context, the manifest edit.** The draft says gate regions were written
   into the manifest by hand *because `mark` declares points only*. The
   surface declares a gate through the contract (§1, item 3); the generated
   program declared no contract. §4's `region` and `way` operations are
   therefore one operation, `claim`, classified by the contract the document
   already has (§6.2).
2. **§3, *neither computes anything the algebra cannot*.** True of the
   buildings, false of the ground: both generators carry a positional hash
   (`grounds.py::_hash`; `grid.py::hsh`) that decides heights, clumps and
   where a tree stands, and the second reads its own canvas at a *different*
   cell to decide a cell (§7). The claim is narrowed to the buildings, and the
   ground is named as what the drawing does not say.
3. **§4, connection state *from the pinned block-state registry*.** The
   registry carries which properties are legal, not the rule. The rule in the
   tree is an unmeasured reading outside `crates/` (§5.3). Stair shape is
   derived on landing; connection state after a measurement this spec scopes.
4. **§6, *a drawing has no seed*.** Both generators paint weighted roles. A
   drawing's geometry has no seed; its weighted paints draw by position from
   the seed the verb derives for the place (§5.1).
5. **Context table, 2207 lines.** That is `castle/*.py`; with
   `build_castle.py`, which the row names, it is 2395.
6. **§2's operation list.** Five of its operations are fields or compositions
   of the others; §3.1 gives the reason for each.

## 11a. Where this spec is corrected by the tree

Written by reading, with no build run (§Ground). These are the points the
implementation found, each with the site that decides it. The spec's sentence is
corrected here rather than designed around.

1. **§2.2, the palette binds `Paint`.** It binds
   [`States`](../../crates/delvec/src/grammar/ir.rs) — one block state or a
   weighted list. `Paint` (`crates/delvec/src/grammar/ir.rs`, `pub enum Paint`)
   **is** the world-frame / local-frame choice, and §5.2 removes that choice:
   every state in a drawing is written in the frame of the scope that paints it.
   Binding `Paint` would offer a `local` wrapper that means nothing and a
   world-frame spelling no operation could honour. The table's own sentence
   beside it — *a block-state string or a weighted list* — already describes
   `States`.
2. **§4.5, `fill` and `void` overwrite and `skip` leaves what was drawn.** The
   tree cannot tell those apart. A grammar expansion is a `VoxelModel` that
   starts as air (`crates/delvec/src/grammar/model.rs`, `VoxelModel::new`) and
   `Node::Void` writes air, so a voided cell and a skipped one are the same
   byte. The rule implemented is **non-air overwrites; air leaves what was
   drawn** — the one the compiler's own `fragment` stamp already applies
   (`docs/reference/compiler.md` §2, stage 7) — and it is stated rather than
   approximated.
3. **§4.5, a `grammar` operation's claims join the drawing's regions.** They
   cannot arrive: `Program::validate` refuses a claim the program's own contract
   does not classify (`crates/delvec/src/grammar/ir.rs`,
   `ProgramError::UnclassifiedRegion`), and `expand` consumes every claimed
   region into the program's own resolved contract
   (`crates/delvec/src/grammar/expand.rs`, `resolve_contract`), so an
   `Expansion` carries no unclassified claim for a drawing to adopt. A program
   under a `grammar` operation is therefore self-contained about its regions.
   Its **marks do** join the drawing's anchors, rebased onto the place's box,
   and a name two of them produce is `DW0903`. Whether a composed contract
   should merge is a question for ADR-0030 §5's spec, not a thing to invent
   here.
4. **§8, `DW0905` refuses arithmetic past 64 bits.** The expression algebra
   **saturates** rather than wrapping
   (`crates/delvec/src/grammar/eval.rs`, `saturating_add`/`_sub`/`_mul`), so
   there is no overflow to observe. What is implemented is a bound on the value:
   a coordinate or an extent past what a cell of a world can be (`±i32::MAX`) is
   refused at the operation that evaluated it, naming the parameters in force.
   That refuses every saturated value and is strictly tighter than the spec's
   sentence.
5. **§2.1, the schema export is produced in the binary, which can reach both.**
   True, and not sufficient: the export's `$defs` is **one flat namespace over
   every document a creator writes**, and `tools/ci/gallery_units.py` refuses by
   name when one name is two things. Five collided — `Mark`, `Edge`, `Facing`,
   `Axis`, `Face`. Three were the same concept declared twice and the copies are
   gone: the drawing uses `delvewright_dsl::siteplan::Face` and `PlanAxis`, and
   `grammar::geom::Axis` is now a re-export of `delvewright_dsl::siteplan::Axis`.
   Three are different things that shared a name, and the grammar's three take
   an exported name — `AnchorMark`, `SpatialContractEdge`, `AnchorFacing` —
   while keeping their Rust names.
6. **§3.4, `skin` keeps a cell on a stepping edge of its own course.** The
   sentence does not say whether the outer edge of a side that does **not** step
   is one. It is not, and the difference is visible: a one-sided `prism` with
   `skin` is the bare slope under the strict reading and the slope plus a
   vertical wall at the ridge under the loose one, and §3.4's own example — two
   one-sided skinned prisms making a stair-block roof — wants the former. Every
   side of a round course steps, so there the stepping edge is the ellipse's own
   boundary.
7. **§9, `delvec prefab diff` is "the instrument of criterion 9".** It is
   criterion **10**'s: criterion 9 is the gallery. The verb is built as the spec
   describes it.
8. **§5.1, the `(n + 1)`-th output of splitmix64 seeded with the execution's
   seed.** Implemented by advancing the one generator
   (`crates/delvec/src/grammar/rng.rs`) once per cell in `VoxelModel`'s own
   order, which **is** that output at linear index `n`. Computing it in closed
   form would be a private copy of splitmix64's internals, which is the defect
   the one generator exists to prevent.

## 11b. What this spec's criteria do not yet have

Recorded as debts, never as passes (§12's own rule).

- **Criterion 2** — the connection-state measurement rig and the derivation's
  move into `crates/dsl` are not built. A drawing **writes** its connections in
  full, as every producer does today, and `DW0735` binds it as it binds a
  program. Criterion 6's connection half is a debt with it.
- **Criterion 8 — met.** `delvec detail` reads the medium off the address, binds
  the `handed/…` names by the one rule, executes at the frame, runs every gate
  it runs today, freezes through the shared freezer and writes the same row;
  `--all` walks both directories. `crates/delvec/tests/detail_verb.rs` generates
  the same room in both media from the allocation and nothing else, so what its
  five new tests measure is the address and not a different building.
- **Criterion 9** — the gallery binds no unit of the `drawing` schema.
  `tools/ci/check-gallery-coverage.py` on this tree: **1148 units enumerated,
  895 bound, 4 refusal-proven, 249 in NEITHER state**, exit 1. Every one of the
  249 is a unit of the new document class. The gate was not weakened and no unit
  was special-cased.
- **Criterion 10** — the port is the content repository's.
- **Criterion 11** — `docs/reference/drawing.md`, `compiler.md`, `tools.md` and
  `grammar.md` §2c are written. The `/new-delve` page is **not** rewritten
  around the drawing, and deliberately: a validated pipeline enters the skill
  with the change that makes it work, and `delvec detail` does not execute a
  drawing yet (criterion 8). `tools/ci/check-skill-page.py` is green, which it
  is because the page still describes the medium the verb still reads.
- **Criterion 12** — the demo row is a later round's, queued with the gallery
  element it would be built from.

**And one defect this spec does not own.** A drawing's refusal at load now
carries the pointer of the value that failed (§8). **The `delvec grammar`
program loader does not**, and the same mistake planted in a program's `mark`
proves it: `error: parse p.json: invalid type: map, expected variant identifier
at line 6 column 65` — no rule, no node, no pointer. It is **not the same
code**: `grammar::document::load` has its own loader and its own error type, and
the mechanism a drawing uses needs an **exported schema** to read the accepted
forms from, which the program document does not have (`delvec schema` exports
the stage documents, the drawing, the walk record and a prefab's metadata, and
no program). Recorded here rather than in `docs/playtest-findings.json`: that
ledger's rows are defects of a built campaign and its consumer refuses a
staging while a row's general form is not a binding check on the build being
staged, which is a judgement it cannot make about an authoring tool's message.

## 12. Acceptance criteria

Machine-checkable; each names its instrument. `delvec` is the tree's own
build. A criterion the implementation cannot yet meet is recorded under it as
a debt, never as a pass.

1. **The surface.** `delvec schema --stage drawing` exports the document;
   `--stage all` carries it under `drawing`; the operation union's `oneOf`
   names thirteen operations — `box`, `cylinder`, `sphere`, `prism`,
   `pyramid`, `line`, `scope`, `use`, `repeat`, `mirror`, `grammar`, `mark`,
   `claim` — and no `clear`, `replace`, `disc`, `dome` or `flight`. The
   schema's `Expr`, `Cond`, `Paint`, `Mark` and `Contract` are generated from
   the program document's own types: a test fails if the drawing module
   defines a type of any of those names.
2. **Connection state is measured before it is derived.**
   `tools/spike-block-settling/` gains the rig of §5.3;
   `observations.json` carries its field and the *written connection after a
   block update* reading; a test replays every observed cell through the one
   derivation in `crates/dsl`, and `prefabs/invariants` holds no second copy
   (`tools/ci/check-structure-emitters.py` still green). The record states the
   second reading's answer in one sentence. Until this criterion is met,
   criterion 6's connection half is a recorded debt and a drawing writes its
   connections in full.
3. **Determinism.** Executing the gallery's drawing twice yields
   byte-identical `.nbt`, metadata and report; executing it at two seeds
   yields models equal at every cell whose paint is not a weighted list
   (ADR-0006). `tools/ci/determinism-subject.sh` covers the verb.
4. **Rasterisation is pinned by value, not by picture.** For each of
   `cylinder`, `sphere`, `prism`, `pyramid`, `line` and each variant
   (`flat` on each face, `t`, `sides`, `section: round`, `skin`, `rise`/`run`
   ≠ 1, a `brush`), a test asserts the exact cell set of a small instance
   written out in the test, including an even and an odd extent; a second
   test evaluates §3.3's inequality in arbitrary-precision arithmetic over
   every box up to 12×12×12 and asserts the executor agrees at every cell.
5. **Frames.** Over the eight frames of §4.2 and every block of the pinned
   registry carrying a `facing`, `axis`, `rotation`, `hinge`, `type` or a
   connection property, a painted state either resolves to a state the
   registry accepts or is refused `DW0738`; a door and a standing banner
   resolve under all four turns and both mirrors, and four quarter-turns of
   any state are the state.
6. **Derived state.** A drawing that paints two stair runs meeting at a corner
   exports the corner shapes `derive_shape` gives, and the `stair-shape` gate
   reports them bound and none mismatched; the same drawing with `shape`
   typed is `DW0907`. After criterion 2: a run of fence between two posts of
   masonry exports joined, with no connection property in the document.
7. **Refusals.** Each of `DW0902` … `DW0908` is asserted by a test that reads the
   operation's address out of the message — for `DW0902`, from inside a `use`
   inside a `repeat`, so the chain and the index are both asserted — and by
   its probe (§10). `tools/ci/check-dw-codes.py` green.
8. **Bound where a program is bound.** `delvec detail --all` over the
   materialised site-plan overlay details `node/annex` from its program and
   `node/far-hall` from its drawing in one run, writes both rows, and the
   whole's battery (spec-0058 step 9) is green; the three spec-0058 probes
   are still refused with the codes they name.
9. **The gallery.** `tools/ci/check-gallery-coverage.py` reports every unit of
   the `drawing` schema bound or refusal-proven, **0 in neither state**, with
   the count of `drawings/*.json` walked stated and non-zero; for every bound
   unit the declared perturbation moves an emitted byte; the baseline
   regenerates with every moved row attributed to this element.
10. **The port — ADR-0030's experiment, first half.** In the content
    repository, on the experiment's own branch, the directory
    `ports/gatehouse/` holds exactly: `drawing.json`, `sentinels.json`,
    `measure.py`, `README.md`. It is not a campaign and imports nothing.
    - *The reference voxels.* The generator's output is its checked-in
      program, `design/programs/castle.json` at `7318202`; no Python of the
      campaign is run. **Instrument check first:** `delvec grammar expand
      --file castle.json --region 104x56x120` at the seed the shipped piece's
      provenance row records reproduces `prefabs/doune-castle.*.nbt`
      byte for byte (`delvec prefab diff` over the whole piece: 0 differ). If
      it does not, that is recorded, and the instrument is the engine
      revision the campaign pins, built in a scratch tree. **Then the
      reference:** the same expansion with every weighted role of the program
      rebound by `--role` to the distinct single block `sentinels.json` names
      for it — the README states the count of weighted roles and the count of
      overrides, and they are equal — so no cell's identity depends on a
      draw.
    - *The comparison.* `delvec drawing execute drawing.json --region
      27x56x20` under the same `--role` overrides, then `delvec prefab diff
      <reference> <port> --box 65,0,30,91,55,49` prints
      **`30240 cell(s) compared, 0 differ`** and the 14 point anchors inside
      the box equal by name, cell and facing.
    - *Shorter.* `measure.py` — standard library only, reading the generator
      as text through `tokenize` and executing none of it — prints two byte
      counts: `castle/gatehouse.py` (678 lines, 28 686 bytes as committed)
      with comments, docstrings, blank lines and indentation removed; and
      `drawing.json` as compact JSON with every `note` removed. **Pass: the
      second is smaller than the first.** The generator's share is taken as
      `gatehouse.py` alone — not `common.py`, not the emitter — which is the
      reading least favourable to the drawing, while the drawing carries its
      whole palette. It also prints, as information: both files' line counts
      in their committed form, the number of operations, and the number of
      `Expr` objects with their share of the bytes (§13).
    - *No script, nothing typed that the engine owns.* The README's build
      commands are `delvec drawing execute` and nothing else; the directory
      holds no other executable; `drawing.json` is green, which by `DW0907`
      means no stair `shape` is written in it.
    - **A zero binding, stated.** The released gatehouse declares no gate: its
      piece carries 31 point anchors, no region anchor and no contract. The
      *gate regions are typed nowhere* half of ADR-0030's pass condition
      therefore binds to nothing in the port. It is bound on the gallery
      element and the demo level instead (criteria 9 and 12), where a bar is
      claimed by the operation that paints it and the exported piece's gate
      anchor resolves through `gate_anchor` with no file edited after the
      verb.
11. **The record and the skill.** Every document §10 names, in the change that
    lands the code; `tools/ci/check-doc-dupes.py`,
    `check-reference-versions.py` and `check-skill-page.py` green.
12. **The demo row** is queued when the code lands —
    `tools/ci/check-demo-levels.py` green.

## 13. Not settled here

- **Whether the algebra's JSON spelling is readable at scale.** `w − 1` is
  ninety bytes. A place's top-level operations are literals against a known
  box, so the cost falls on `define`s; criterion 10 reports the `Expr` share
  so the answer is a number. A terser spelling would be a change to the one
  algebra, in both documents, and would move the program ledger — it is not
  taken here.
- **The ground.** Noise heights, clumped scatter and randomly placed trees are
  not sayable (§7). Per-cell scatter is — a weighted paint with `air` in it.
  Whether terrain belongs to a drawing at all is ADR-0030 §5's question about
  the site's base build.
- **A predicate over neighbouring cells** — *a parapet wherever the next cell
  is off the wall* — is not sayable, by design: `where` reads one cell. The
  cost is that the author types the runs. If the port shows that cost is the
  document, that is the experiment's answer, not a reason to let an
  expression read the canvas.
- **Whether a written connection may stand** (§5.3, item 3) — decided by the
  measurement, then by its own spec if the answer is no.
- **Sharing defines between documents** (§9).

## 14. Decisions for the owner

- A place's detail may be a **drawing**: palette, defines, a contract, and an
  ordered list of thirteen operations — six solids, five arrangers, `mark` and
  `claim` — executed into the same model every gate already reads.
- **Five of ADR-0030's operations are struck** — `clear`, `replace`, `disc`,
  `dome`, `flight` — each because a field on the remaining operations says it
  for every solid instead of for one.
- **Stair shapes are derived now; fence, wall, pane and bar connections only
  after they are measured on the pinned server**, because the rule in the tree
  is an unmeasured reading with a 25-block table. Until then a drawing types
  its connections, as every producer does today.
- **A gate is a claimed box the contract calls a bar** — the surface that
  exists — not a new operation.
- **Geometry has no seed; weighted paints draw by position**, so an edit
  re-textures nothing else.
- **Every refusal names the operation in the document the creator wrote.**
  Seven new codes.
- **`dsl_version` moves**; the grammar-program ledger does not.
- The experiment's first half is judged by **`delvec prefab diff` printing
  zero** against the released program re-expanded under sentinel roles, and by
  **bytes**, with the generator's share taken as `gatehouse.py` alone.
