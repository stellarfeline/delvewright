## A place's detail is a drawing (spec-0072 core, ADR-0030 §§1–4)

A drawing is a JSON document per place — a palette of roles, named bodies, a
spatial contract, and an ordered list of **thirteen operations** over the
place's box in its own frame. **A later operation overwrites an earlier one**,
which is what a designer does and what a box-split partition cannot do: a
partition gives every cell to exactly one leaf, so a rose window in a gable in a
wall has to be reached by one split tree that anticipated all three.

```
delvec drawing check   drawings/gatehouse.json
delvec drawing execute drawings/gatehouse.json --region 27x56x20 -o out/
delvec prefab diff <a> <b> [--box x0,y0,z0,x1,y1,z1] [--at x,y,z]
```

Six solids — `box`, `cylinder`, `sphere`, `prism`, `pyramid`, `line` — five
arrangers — `scope`, `use`, `repeat`, `mirror`, `grammar` — and `mark` and
`claim`. The five ADR-0030 named that a *field* says for every solid are struck:
`clear`, `replace`, `disc`, `dome`, `flight`.

### What makes this the same artifact, not a second pipeline

spec-0072 §1's finding is that everything after the blocks already takes an
`Expansion`, and only the export took a `Program` — for four things, of which
three are arguments. So `grammar::export` splits: `freeze_prefab` and
`freeze_zone` take an expansion, a provenance row and `shown_faces`, and
`export_prefab` / `export_zone` are those halves with `expand` in front. A piece
a drawing produced is judged by the same gates, refused by the same refusals and
carries the same document.

The rules a second producer needs are **extracted, not copied**:
`grammar::place` (a state read in the scope's axes, a mark's cell, a mark's
facing, the frame label), `ir::check_contract_references`,
`expand::resolve_declared_contract`, and `settle::derive_stair_shapes` beside
the judge it derives from.

### What the engine knows, the creator does not type

A stair's `shape` is derived after the last operation from the measured
derivation, and a drawing that types one is refused (`DW0907`) — so `DW0801` is
unreachable from one. **Connection state is not derived**: the rule the tree
holds is an unmeasured reading over a hand-declared table, so a drawing writes
its connections in full and `DW0735` binds it as it binds a program. That is
spec-0072 criterion 2 and is recorded as a debt, not a pass.

### Determinism

Geometry has no seed. Every rasterisation rule is integers over a box's extents;
the seed reaches **weighted paints** and nothing else, and it reaches them **by
position** — the draw at a cell is a function of the seed and the cell's linear
index, so editing one operation re-textures no other cell.

### The frame classifier gains the eight frames that keep the vertical

`property_image` determined a 16-step `rotation` and a chirality only under the
identity and the bare `x↔z` transposition, so a door or a standing banner inside
a turned scope was refused. Every quarter-turn is a transposition *with* a
reversed axis. `horizontal_frame` reads the four turns and their four mirrors as
one turn of the horizontal plane — arithmetic, not a game fact, and computed
from the frame rather than tabulated — so the rule reaches every block the pin
carries. It is the one classifier, so `DW0736`, `DW0738`, `DW0742`, the
grammar's `reorient` and `mirror` and the drawing's `turn` all move together.

The residue test's boundary moves with the rule, and the direction is stated: it
now refuses **strictly less** — only where the vertical moves or runs backwards.
The states it stops refusing are the ones the new table computes exactly, and
the judge/resolver agreement sweep is unchanged.

### Refusals, addressed where the mistake was made

`DW0902`…`DW0908`, each addressed by the operation's JSON pointer in the
document the creator wrote, the chain of uses that reached it, the `repeat`
indices in force and the mirror side:

```
DW0902: this `box` covers 0,0,0 .. 0,0,5, and the scope it is written in holds
0,0,0 .. 2,2,2 (3x3x3). — at /defines/post/body/0 ← /ops/0/body/0 (k=0)
```

### `dsl_version` 0.30.0

The schema export gains a document class, and ADR-0024 makes that a move of the
one number. Three files by hand, 288 by tool.

**One flat `$defs` namespace, so one name is one thing.** The drawing brought
five collisions. Three were the same concept declared twice and the copies are
gone — the drawing uses `delvewright_dsl::siteplan::Face` and `PlanAxis`, and
`grammar::geom::Axis` becomes a re-export of `delvewright_dsl::siteplan::Axis`.
Three are different things that shared a name, and the grammar's three take an
exported name (`AnchorMark`, `SpatialContractEdge`, `AnchorFacing`).

### Evidence

- **57 new tests** across `drawing_surface` (5), `drawing_raster` (13),
  `drawing_refusals` (18), `drawing_execution` (16) and `drawing_cli` (5), plus
  three in `dsl`: the eight-frame yaw/handedness table, the registry-wide frame
  sweep, and the four-quarter-turns identity.
- The rasterisation is pinned **by value** per solid and variant, and
  cross-checked by an **arbitrary-precision oracle** that shares no code with
  the executor's inequality — different integers, different grouping — over
  **5434 solids and 1 498 310 cells**, 1 086 334 of them inside, with a
  perturbation test that reds the oracle.
- `cargo fmt --check`, `clippy --workspace --all-targets -D warnings` clean.
  `cargo test --workspace --no-fail-fast`: 267 suites ok; the failure set is
  this machine's baseline (`grammar_campaign_zones` 2, `seating_cli` 3,
  `view_viewer` 1 — the shared `campaigns/` checkout is ahead of the pinned
  content revision).
- 30 CI gates run green on the tree, `pytest tools/tests` 1397 passed.

### The one expected red

`tools/ci/check-gallery-coverage.py` exits 1: **1148 units enumerated, 895
bound, 4 refusal-proven, 249 in neither state** — every one a unit of the new
document class. The gallery element is spec-0072 criterion 9, a later round on
this branch. The gate was not weakened and no unit was special-cased.

### Debts, recorded in the spec (§11b) and never as passes

Criterion 2 (the connection measurement), criterion 6's connection half,
criterion 8 (`delvec detail` executing a drawing — only the two-media refusal
landed), criterion 9 (the gallery), criterion 10 (the port), criterion 11's
skill page (a validated pipeline enters the skill with the change that makes it
work, and `delvec detail` does not execute a drawing yet) and criterion 12 (the
demo row).

### Where the tree corrected the spec

spec-0072 §11a, eight points, each with the site that decides it: the palette
binds `States` and not `Paint`; a grammar expansion cannot tell `void` from
`skip`; a program's claims cannot arrive unclassified; the algebra saturates
rather than overflowing; the `$defs` namespace is flat; a non-stepping side is
not a stepping edge; `prefab diff` is criterion 10's instrument; and the
splitmix64 draw is taken by advancing the one generator.

### For the owner

`CLAUDE.md`'s repository-layout list of `docs/reference/` does not name the new
`drawing.md`. Neither constitution file is edited without confirmation, so it is
left alone and named here.
