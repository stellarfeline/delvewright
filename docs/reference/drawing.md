# The drawing — a place's detail as an ordered list of solids

For the agent that writes drawings. The engine is `delvec`; the document is
`drawings/<place stem>.json` inside the campaign; the verbs are
`delvec drawing check` and `delvec drawing execute`.

A drawing is executed in the place's box, **in the box's own frame**: `x` east,
`y` up, `z` south, the origin at the box's minimum corner, every coordinate a
cell. **A later operation overwrites an earlier one.** That is the whole of the
model: raise the wall, cut the window, lay the tracery over it.

```json
{ "dsl_version": "<the engine's one number>",
  "name": "gatehouse",
  "params":  { "handed/datum-y": 1, "bay": 5 },
  "palette": { "wall":  [ { "weight": 10, "block": "minecraft:stone_bricks" },
                          { "weight": 2,  "block": "minecraft:cracked_stone_bricks" } ],
               "tread": "minecraft:stone_brick_stairs[facing=north,half=bottom,waterlogged=false]",
               "bars":  "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]" },
  "defines": { "loop": { "params": { "h": 2 }, "roles": ["stone"], "body": [] } },
  "contract": { "entry": "passage", "spaces": {}, "edges": [] },
  "shown_faces": ["north"],
  "ops": [] }
```

## 1. The header

| Field | Meaning |
|---|---|
| `dsl_version` | **Required.** The engine's one campaign-format number (ADR-0024). Any other is `DW0102` at load. |
| `name` | Provenance label, as a program's. It names the DOCUMENT and is never the artifact's id. |
| `params` | name → integer: a declaration **and** a default, read by `{"expr": "param", "name": …}`. The `handed/…` names are bound by `delvec detail` from the allocation and by nothing else (`DW0882`). |
| `palette` | role → one block state, or a weighted list of `{weight, block}`. **Every state is written in the frame of the scope that paints it**; there is no world-frame spelling and no `local` wrapper. |
| `defines` | name → `{params, roles, body}` (§4). |
| `contract` | the program document's `Contract`, verbatim. Required wherever `delvec detail` binds the drawing (`DW0843`). |
| `shown_faces` | the sides that are finished exterior surface (`DW0885`), written through to the piece. |
| `ops` | the ordered list. |

An integer position or size anywhere in an operation is a JSON integer, **or**
an expression of the one algebra (`grammar.md` §2: literals, parameters, the
current scope's extents through `{"expr": "dim"}`, and `+ − × ÷ % max min`).
A bare integer is one more spelling of `{"expr": "int", "value": n}` at a
drawing's own fields, and nowhere else: inside an expression the operands are
the algebra's own tagged form. There is no second algebra, no string syntax and
no float.

## 2. The six solids

Every solid carries the same five fields beside its own:

| Field | Meaning |
|---|---|
| `role` | a palette role, or the reserved role **`air`** |
| `from`, `to` | the operation's box in its scope, **inclusive** corners. `from` omitted is the scope's minimum corner; `to` omitted is `from` where one was written and the scope's maximum corner where none was — so both omitted is the whole scope and `from` alone is one cell |
| `where` | optional list of roles. The operation overwrites only cells whose **current** role is listed. `air` is a role here; cells a `grammar` operation wrote answer to the reserved name `grammar` |
| `when` | optional guard — the `cmp` / `all` / `any` / `none_of` members of `Cond`. False: the operation does nothing. It is how a `define` says *not at this size* |
| `note` | one line for a reader. The engine never reads it |

`where` reads **the cell being painted and no other**. It is what makes
*replace role A by role B inside a box* and *only where nothing stands yet*
properties of every solid instead of verbs of their own — which is why there is
no `clear` and no `replace`.

Write `n = (nx, ny, nz)` for the operation's box extents and `i = (ix, iy, iz)`,
`0 ≤ i_a < n_a`, for a cell in it.

### `box`

Absent `faces`, every cell is in. With `faces` — a non-empty subset of
`west east down up north south`, the low and high faces of `x`, `y`, `z` in that
order — and thickness `t` (default 1), a cell is in when it lies within `t` of a
listed face: `i_a < t` for a low face of axis `a`, `i_a ≥ n_a − t` for a high
one. All six faces is the hollow shell; the four horizontal ones are a wall ring.

### `cylinder` and `sphere` — one integer rule

`cylinder` takes `axis` (`x`, `y` or `z`); its two other axes are **round**.
`sphere` has three round axes. Either takes an optional `flat`, a face of the
box on a **round** axis: the solid is then the half whose cut plane lies on that
face. For each round axis `a`:

| | `u_a` | `D_a` |
|---|---|---|
| not flattened | `2·i_a + 1 − n_a` | `n_a` |
| `flat` is `a`'s low face | `2·i_a + 1` | `2·n_a` |
| `flat` is `a`'s high face | `2·(n_a − 1 − i_a) + 1` | `2·n_a` |

**A cell is in when `Σ_a ( u_a² · Π_{b≠a} D_b² ) ≤ Π_a D_a²`**, the sums and
products over the round axes. That is the ellipse or ellipsoid inscribed in the
box, tested at cell centres in doubled coordinates, so an even extent and an odd
one obey one rule and nothing is ever halved. For an odd width `n = 2r + 1` it
reduces to `dx² + dz² ≤ r² + r`.

The arithmetic is exact: a round axis may not exceed **4096** cells (`DW0905`),
which keeps every term inside 128 bits. The bound is refused rather than the
arithmetic allowed to wrap.

With `t`, the solid is a tube or a shell: a cell is in when it is in the solid
above and **not** in the same solid taken over the box shrunk by `t` at every
round face that is not the `flat` one. A shrunk box with no cells removes
nothing, so a wall thicker than the radius is the solid disc and never an empty
one.

### `prism` and `pyramid` — courses that step in as they rise

Both take `rise` and `run` (each at least 1, default 1): the solid steps in
`run` cells for every `rise` courses. For the course `k = iy`,
**`inset(k) = ⌊k · run / rise⌋`**.

- **`prism`** takes `taper`, the horizontal axis (`x` or `z`) the span narrows
  along — the ridge runs along the other — and `sides`: `both` (default), `low`
  or `high`, the faces of that axis that step in. With `c` the cell's coordinate
  on `taper` and `n` the box's extent on it, the course spans `lo(k) ≤ c ≤ hi(k)`
  where `lo(k)` is `inset(k)` on a stepping low side and `0` otherwise, and
  `hi(k)` is `n − 1 − inset(k)` on a stepping high side and `n − 1` otherwise.
  `both` is the gable; one side is the wedge, the buttress and the lean-to.
- **`pyramid`** steps in on all four horizontal faces by the same `inset(k)`.
  With `section: "round"` each course is the ellipse above, inscribed in that
  course's own rectangle: the cone.
- A course whose span is empty has no cells. The box's height truncates the
  solid: a batter is a pyramid whose box ends before it closes.
- **`skin: true`** keeps only the surface: a cell of course `k` is kept when it
  lies on a **stepping** edge of course `k`'s own span, or when course `k + 1`'s
  span — computed by the same formula, whether or not the box reaches it — does
  not cover it. A side that does not step is not a stepping edge, which is what
  makes a one-sided `prism` with `skin` the bare slope rather than the slope
  plus a vertical wall at the ridge. Every side of a round course steps, so
  there the stepping edge is the ellipse's own boundary.

A roof whose slopes are stair blocks is two `prism`s with `skin`, `sides: low`
and `sides: high`, each painting a stair role that faces up its slope.

### `line`

From cell `A` to cell `B` (`from`, `to`; here `to` is an **end point**, not a
corner), with `d = B − A` and `n = max(|dx|, |dy|, |dz|)`. For `i = 0 … n` the
point is **`A_a + ⌊(2·i·d_a + n) / (2n)⌋`** on each axis, the division flooring
toward −∞; `n = 0` is the single cell. `brush: [bx, by, bz]` (default
`[1, 1, 1]`) paints a box of that size with its minimum corner on each point,
and the stamps overlap where the line turns. A half is always rounded up, so a
line from `B` to `A` may differ from the line from `A` to `B` by a cell; the
rule is stated rather than symmetrised.

The cells a `line` reaches are the points' own bounding box **grown by the
brush**, and that is what its scope must hold.

## 3. The five arrangers

Each carries a `body`, `from`/`to`, `when` and `note`. None paints.
**Every body-bearing operation makes its box its body's scope.**

### `scope`

`{ "op": "scope", "from": …, "to": …, "body": [ … ] }` — the body's coordinates
are local to the box, its extents are what `{"expr": "dim"}` reads, and an
operation that reaches outside it is `DW0902`. It is what lets a block of
operations move as one when the plan moves it.

### `define` / `use`

A define is `{ "params": {name: default}, "roles": [names], "body": [ … ] }`.

```json
{ "op": "use", "define": "loop", "from": [12, 9, 0], "to": [12, 11, 1],
  "turn": 1, "mirror": "x",
  "params": { "h": 3 }, "roles": { "stone": "dressed" } }
```

- The body runs in the `use`'s box under a **frame**: `mirror` (`x` or `z`,
  optional) reflects the body across the centre plane of its own local box, and
  then `turn` (0–3) turns it that many quarter-turns **clockwise seen from
  above**. A local cell `(lx, y, lz)` in a local box of extents `(Lx, Ly, Lz)`
  lands at:

  | `turn` | local `x` | local `z` | world extents |
  |---|---|---|---|
  | 0 | `lx` | `lz` | `(Lx, Ly, Lz)` |
  | 1 | `Lz − 1 − lz` | `lx` | `(Lz, Ly, Lx)` |
  | 2 | `Lx − 1 − lx` | `Lz − 1 − lz` | `(Lx, Ly, Lz)` |
  | 3 | `lz` | `Lx − 1 − lx` | `(Lz, Ly, Lx)` |

  A stair written `facing=north` inside a define therefore faces north, east,
  south, west as the turn goes 0, 1, 2, 3.
  **The vertical never moves: a drawing has gravity.**
- `params` may name only parameters the define declares, and **every** role the
  define lists is bound at the `use`: a parameter has a default and a role has
  none, so an unbound role names nothing (`DW0903`). Arguments are evaluated in
  the **enclosing** scope, before the frame is pushed.
- A define's body reads its own parameters, the `repeat` indices written inside
  it, and the document's `params`; nothing else.
- A define may `use` another. **A define that reaches itself, directly or
  through others, is refused with the chain named** (`DW0904`) at validation,
  before anything executes. There is no recursion, so there is no depth to
  limit.

### `repeat`

Two spellings of *how many*, one construct.

```json
{ "op": "repeat", "step": [0, 1, 1], "count": 10, "index": "k", "body": [ … ] }
{ "op": "repeat", "along": "x", "stride": 5, "item": 1, "remainder": "exact",
  "index": "k", "body": [ … ] }
```

- **Counted.** Instance `k = 0 … count − 1` runs the body with the scope's
  origin moved by `k · step`; the extents do not change. An instance that
  reaches outside the enclosing scope is `DW0902` with `k` named. A step with a
  vertical part is what makes a flight, a stepped arch and a corbel table one
  operation each.
- **Fitted.** Items `item` cells long stand every `stride` cells along the
  scope's `along` axis: `n = ⌊(E − item) / stride⌋ + 1` of them, `E` the scope's
  extent, leaving `r = E − ((n − 1)·stride + item)` cells over. Each instance's
  scope is its own `item`-long slice. **`remainder` is required** and says where
  `r` goes — `exact` (there must be none), `start`, `end`, `middle` (the
  lower-middle split, `⌊r/2⌋` before). A missing `remainder` is `DW0100`;
  `exact` with cells over, or a run that fits no whole item, is `DW0906`.
- `index` binds the instance number as a parameter for the body — what lets a
  course step in as it rises, or alternate two roles by `k % 2`.

A flight of ten treads climbing north, three wide:

```json
{ "op": "prism", "role": "step", "taper": "z", "sides": "high",
  "from": [0, 0, 0], "to": [2, 9, 9] },
{ "op": "repeat", "from": [0, 0, 9], "to": [2, 3, 9],
  "step": [0, 1, -1], "count": 10, "body": [
  { "op": "box", "role": "tread", "from": [0, 0, 0], "to": [2, 0, 0] },
  { "op": "box", "role": "air",   "from": [0, 1, 0], "to": [2, 3, 0] } ] }
```

### `mirror`

`{ "op": "mirror", "axis": "x", "from": …, "to": …, "body": [ … ] }` runs the
body, then runs it again reflected across the centre plane of the operation's
box on `axis`: a cell at `c` lands at `n − 1 − c`, exact for odd and even
extents alike. States reflect with it, so a door's hinge swaps. The unreflected
pass runs **first**, and where the two meet the second overwrites the first.
A `mark` inside takes `"index": "auto"` or is a name written twice (`DW0903`).

### `grammar`

```json
{ "op": "grammar", "program": "programs/colonnade.json", "rule": "arcade",
  "from": [4, 1, 20], "to": [27, 7, 22], "seed": 7,
  "params": { "bay": 4 }, "roles": { "stone": "minecraft:andesite" } }
```

The named program — a file, **relative to the drawing**, resolved by the program
loader with its `include` list and its path rules — is expanded into the
operation's box, from `rule` (default: the program's `start`), at the
**literal** `seed` the operation writes. Which variant is a judgement, so it is
an argument; nothing derives it.

**Non-air overwrites; air leaves what was drawn.** A grammar expansion is a
`VoxelModel` that starts as air and `void` writes air, so a voided cell and a
skipped one are the same byte: the rule here is the one the compiler's own
`fragment` stamp applies. Its marks join the drawing's anchors, rebased onto the
place's box. **The grammar does not call drawings.**

## 4. Anchors and regions are operations

### `mark`

`{ "op": "mark", "mark": { … } }` carries the program document's `Mark`
verbatim — `anchor`, `at` and its four forms, `facing`, `index`, `role` —
evaluated against the scope it is written in and put through that scope's frame,
a written `facing` included. It writes no block. It lands in the expansion's
anchors and reaches the prefab metadata through the path that writes every
grammar anchor today.

### `claim`, and the contract

`{ "op": "claim", "region": "gate", "from": …, "to": …, "body": [ … ] }` names
its box and runs its optional body inside it. Several claims of one name union.
The header's `contract` says what each name is — a space, an out-of-walk region,
an edge's `via`, a `bar`, a `way` — and is checked by the same reference checker
a grammar program is held to.

**ADR-0030's `region` and `way` operations are this one operation.** A gate a
story opens is a `barred` edge whose `bar` names a claimed region and a palette
role; the operation that claims the box is the operation that paints it, so the
gate's cells are typed once:

```json
{ "op": "claim", "region": "portcullis", "from": [10, 1, 3], "to": [14, 5, 3],
  "body": [ { "op": "box", "role": "bars" },
            { "op": "mark", "mark": { "anchor": "gate", "at": "floor_center" } } ] }
```

## 5. The executor

Operations run in document order; a `repeat`'s instances in ascending index; a
`mirror`'s unreflected body first; a `use`'s body in order. The canvas holds,
per cell, the role last painted and the frame-resolved paint it names. After the
last operation, in this order:

1. **Weighted paints are drawn, by position.** A cell whose paint is a weighted
   list takes the draw `out(n) % total`, where `n` is the cell's linear index in
   the place's box and `out(n)` is the `(n + 1)`-th output of the engine's one
   generator (splitmix64, seeded with the execution's seed). The draw is a pure
   function of the seed and `n`, so no stream is consumed and **editing one
   operation re-textures no other cell**.
2. **Derived state is written**: every stair's `shape` (§6).
3. The result is an `Expansion` — the model, the anchors, the resolved contract
   — which is what every gate the engine has already reads.

The seed is `detail`'s place-derived seed or `--seed` (default 0) under
`delvec drawing execute`. **Geometry has no seed**: the seed reaches weighted
paints and nothing else. No clock, no environment, no hash-ordered container;
two executions are byte-identical (ADR-0006).

### Every state is written in the scope's own frame

A state in a drawing's palette is resolved into the world through the frame of
the scope that paints it, by the one resolver a grammar's `local` paint uses
(`BlockRegistry::permuted_properties`), and refused where that resolver cannot
determine an image (`DW0738`, under the operation's address). At the top level
the frame is the identity and the state is what was written. So `DW0736` and
`DW0742` are unreachable from a drawing: no world-frame literal can be written
in one.

The frames a drawing produces all keep the vertical, and under those eight — the
four turns and their four mirrors — a 16-step `rotation` and a handedness
(`hinge`, a chest's `type`) have exact images: a turn sends a yaw `r` to
`(r + z) mod 16` where `z` is the yaw that turn sends `rotation` 0 to, and keeps
left and right; a mirror sends `r` to `(z − r) mod 16` and swaps them.

## 6. Derived block state

- **A stair's `shape` is the engine's.** After the last operation every stair
  cell, whatever wrote it, takes the shape `schem::stairs::derive_shape` gives it
  from its four neighbours inside the place's box, a cell outside the box
  reading as no stair. That function is replayed cell for cell against a field
  placed and settled on the pinned server; the drawing calls it and owns no
  second rule. **A drawing's paint that writes `shape` on a stair is refused**
  (`DW0907`) rather than silently overwritten, so `DW0801` is unreachable from a
  drawing.
- **Connection state — fences, walls, panes, bars — is NOT derived.** A drawing
  writes it in full, as every producer does today, and `DW0735` binds it exactly
  as it binds a program. The rule the tree holds is an unmeasured reading of
  vanilla's code over a hand-declared table of full cubes, outside `crates/`;
  spec-0072 criterion 2 scopes the measurement, and until it is taken this is a
  recorded debt and not a pass.
- Every property that is neither of those is written by the author in full, and
  `DW0737` binds as it does for a program. Filling an omitted `facing` from the
  block's default would hide the mistake the code exists to show.

## 7. Refusals

Every refusal is addressed by **the operation's path in the document the creator
wrote** — a JSON pointer, `/ops/12/body/3` — followed, inside a define, by the
chain of uses that reached it (`/defines/tower/body/4 ← /ops/7`), the `repeat`
indices in force (`k=3`) and the mirror side. A cell coordinate is added where
there is one; it is never the address.

| Code | Refuses |
|---|---|
| `DW0100`, `DW0102` | a document that does not parse — an unknown `op`, a missing `remainder`, a `turn` of 4, an `otherwise` or an `orientation` in a `when`; a `dsl_version` that is not the engine's |
| `DW0902` | an operation that reaches a cell its scope does not hold — a solid's box, a `line`'s brush, a `mark`, a `claim`, a `grammar` box |
| `DW0903` | a name that resolves to nothing, or to two things — a role, a define, a parameter, a region only one side names, an anchor two marks produce |
| `DW0904` | a `define` that reaches itself |
| `DW0905` | a value outside its range at the operation that evaluates it |
| `DW0906` | a fitted `repeat` that does not fit |
| `DW0907` | a paint that writes a property the engine derives |
| `DW0908` | a place with two media — `programs/<stem>.json` and `drawings/<stem>.json` both present |
| `DW0738` | a state whose image the frame does not determine |

`docs/reference/compiler.md` §5 carries the full catalog entry for each.

**Not a refusal, and printed on every run**: operations written, instances
executed, cells painted, cells surviving to the model, and every written
operation none of whose instances painted a cell, by address. An operation that
paints nothing is dead text in the document of record; it is listed, with its
count, rather than refused, because a `define` written for many sizes may
honestly have an operation with nothing to do at one of them.

## 8. The verbs

```text
delvec drawing check   drawings/gatehouse.json
delvec drawing execute drawings/gatehouse.json --region 27x56x20 -o out/
```

`check` validates without executing: the schema, every `DW0903` / `DW0904` /
`DW0907`, and the contract's reference integrity. It prints what it examined —
operations, defines, roles, params, claimed regions, marks — and names any
define no `use` names.

`execute` takes `--region XxYxZ`, `--out <dir>`, and optionally `--id`,
`--seed`, `--param k=v`, `--role r=state` and the gate opt-ins
(`--traversable`, `--allow-falls`, `--symmetric <axis>`, `--reachable-floor`).
It executes, judges with the same gates `delvec grammar expand` runs, and
freezes through the same freezer — so a piece a drawing produced is a piece of
the same shape, judged by the same refusals, carrying the same document. It
writes `<id>.nbt` (or a tile set), `<id>.json` and `<id>.report.json`.

A `--role` override is a **restyle**: every state in a drawing is already read in
the frame of the scope that paints it, so there is no frame to inherit and none
to lose. An undeclared `--param` or `--role` is refused rather than ignored.

The provenance row names the generator `drawing`, the document's `name`, the
SHA-256 of its canonical bytes **together with those of every program file a
`grammar` operation names**, the seed, the region, and the parameters and roles
overridden.

## 9. Comparing two pieces

```text
delvec prefab diff <a> <b> [--box x0,y0,z0,x1,y1,z1] [--at x,y,z]
```

Block-state equality, cell for cell, of a box of piece `a` against the
same-sized box of piece `b` at `--at` (default the origin), tiled pieces
included, plus equality of the point anchors inside the box. It reads two pieces
and nothing of any campaign. It prints cells compared and cells differing, names
the first few with **both** states, refuses a comparison that examined zero
cells, and exits 1 on any difference.

## 10. What is not here yet

- **Connection state is written, not derived** (§6). spec-0072 criterion 2.
- **`delvec detail` does not execute a drawing.** It refuses a place with two
  media (`DW0908`) and details from `programs/<stem>.json`. spec-0072
  criterion 8.
- **The gallery binds no unit of this surface yet**, so
  `tools/ci/check-gallery-coverage.py` reports every one of them unaccounted.
  spec-0072 criterion 9.
