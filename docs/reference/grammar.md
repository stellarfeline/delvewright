# Box-split grammar back end — live behavior record

What `crates/grammar` (package `delvewright-grammar`) does **today**. spec-0027
is the decision record; this page is the behavior record, and any PR that
changes the crate's surface updates it in the same PR.

Phase 1 is a **library**, not a tool: no binary, no `delvec` path, nothing in
[`tools.md`](tools.md). It ships in no delve — generation-time only (ADR-0003).

## 1. Model

A **grammar program** is data: named rules over integer voxel boxes. Expanding
one against a box and a `u64` seed derives a **voxel model** — a dense grid of
full block states.

Every scope in a derivation is a box plus an **orientation**: a permutation
mapping the rule's local `X`/`Y`/`Z` onto world axes. That is what lets one rule
be reused turned 90°, and what `reorient` manipulates.

```text
Program ─ expand(program, region, {seed, limits, orientation}) ─▶ VoxelModel
```

## 2. Program surface

| Element | Form | Notes |
|---|---|---|
| `name` | string | provenance label |
| `start` | rule name | expanded into the whole region |
| `params` | name → i64 | size/kind controls; read by `{"expr":"param"}` |
| `palette` | role → paint | style controls; a paint is a block-state string or a weighted list |
| `rules` | name → `[alternative]` | each alternative is `{weight, when, body}` |

**Rule bodies** (`op`): `fill` (a role or an inline paint), `void` (air), `skip`
(leave as-is), `call`, `split`, `reorient`.

**`split`** cuts one local axis into pieces: `absolute` pieces take a fixed block
count, `relative` pieces share what is left. `rounding` (`truncate` — the
default and upstream's only behaviour — `start`, `end`, `middle`) says where the
indivisible remainder goes; `repeat` tiles the pattern across the axis and clamps
the last piece; `orient` hands every child a new orientation. Children are
matched to pieces in order, and cycled when `repeat` produced more pieces than
children.

**`reorient` / `orient`** name a child axis as `local_*`, `world_*`, `smallest`,
`largest`, or `split_axis` (the axis being cut; splits only). Unnamed axes are
completed to a permutation: keep an axis where possible, otherwise complete the
cycle the request started (asking for "my Z is the old X" swaps X and Z),
otherwise take the lowest free axis.

**Guards** (`when`): `always` (default), `otherwise`, `cmp` over integer
expressions of literals / params / scope dimensions with `+ - * / % max min`,
`all` / `any` / `none_of`, and `orientation` (matches an exact axis mapping — how
a directional stair or door picks its facing).

**Selection**: every non-`otherwise` alternative whose guard holds is a
candidate; if none hold, the `otherwise` alternatives are; among candidates the
seeded PRNG draws by `weight`. **Two guards that can hold at once are a
probabilistic choice, not a priority order** — guards meant as a decision must be
mutually exclusive.

The IR serialises to JSON (`serde`), which is the authoring form; block states
are their vanilla string, e.g. `"minecraft:oak_stairs[facing=east,half=top]"`.

## 3. Determinism (ADR-0006)

Same program + same region + same seed → byte-identical `VoxelModel`, asserted by
a double-expand test over every library program at five seeds, plus a
seed-sensitivity test over a probabilistic program. All randomness is one
splitmix64 stream from the caller's seed; all maps are `BTreeMap`; cells iterate
`x`, then `y`, then `z`; nothing reads the clock, the environment or a path.
`VoxelModel::canonical_bytes` is the comparison/hash form.

Expansion holds no global state — two programs cannot influence each other, which
is regression-tested.

## 4. Failure is loud

The interpreter has no silent degradation. `Program::validate` runs before any
expansion (unknown rule/role/param, empty rule or split, child/piece mismatch on
a non-repeating split, zero weights). During expansion: `NoApplicableRule`,
`Split{Overflow|ZeroStride}`, `Orient`, `BadSize`, `Eval`, and the `DepthLimit` /
`ScopeLimit` budgets that turn an unguarded recursive rule into a diagnostic
instead of a hang. Errors carry the rule name.

These are Rust error values, **not** `DW` diagnostics: the craft-rule diagnostics
of spec-0027 §4 are a later phase and will own a DW range then.

Consequence for authors: a region too small for a program's absolute sizes is an
error, not a building with pieces outside its box. Each library program documents
its minimum region.

## 5. Rule library

Ported from `yawgmoth/GDMC25` (BSD-3-Clause; see
[`ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md)) and reachable as
`library::{temple, castle, church}`.

| Program | Controls | Smallest region that expands (measured) |
|---|---|---|
| `temple` | `roof` (pitched/flat/capped/open), `column_height`, `column_size`; role `marble` | X ≥ `6 + 2*column_size`, Y ≥ `1 + column_height + roof height` (5 pitched / 3 flat / 1 capped / 0 open), Z ≥ 7 |
| `castle` | `large_tower`, `small_tower`, `great_hall`, `wall_height`, `wall_width`, `tower_height`; role `stone` | both horizontal extents ≥ `2*large_tower + 2`, Y ≥ `tower_height + 1` |
| `church` | guards only; roles `wall`, `glass`, four `roof_*` stair facings, two door pairs | height must follow width (the roof steps in 2 per course): Y ≥ 9 and Y ≳ X − 3; 15 × 16 × 30 is comfortable |

Ports are faithful except where a module says otherwise; the three substantive
divergences are recorded at their code: the temple's colonnade repeats to fit the
box instead of being fixed at four columns, the church's one-wide ridge course is
guarded (upstream splits it anyway and writes outside the region), and
constraint `largest` returns the maximum (upstream returned the minimum — a
copy/paste bug).

## 6. Not built yet

`.nbt` export + provenance row, `PrefabRegistry` admission, the §4 craft
diagnostics, the JSON schema stage in front of the IR, and the
contact-sheet/curation loop. Later phases of spec-0027.
