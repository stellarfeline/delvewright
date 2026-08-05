# Box-split grammar back end — live behavior record

What `crates/grammar` (package `delvewright-grammar`) does **today**. spec-0027
is the decision record; this page is the behavior record, and any PR that
changes the crate's surface updates it in the same PR.

It is a **library**, not a tool: no binary, no `delvec` path, nothing in
[`tools.md`](tools.md). It ships in no delve — generation-time only (ADR-0003).
The engine depends on it nowhere; `crates/compiler` names it as a *dev*-dependency
only, to test the export seam of §7 from both sides.

## 1. Model

A **grammar program** is data: named rules over integer voxel boxes. Expanding
one against a box and a `u64` seed derives a **voxel model** — a dense grid of
full block states.

Every scope in a derivation is a box plus an **orientation**: a permutation
mapping the rule's local `X`/`Y`/`Z` onto world axes. That is what lets one rule
be reused turned 90°, and what `reorient` manipulates.

```text
Program ─ expand(program, region, {seed, limits, orientation}) ─▶ VoxelModel
        ─ export_prefab(program, region, options, id) ──────────▶ .nbt + .json
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
(leave as-is), `call`, `split`, `reorient`, `mark`.

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

## 2b. `mark` — anchor declarations

An anchor is **metadata**, not geometry. No composition of `fill` / `split` can
express "this cell is where the boss stands", and reading one back out of the
block pattern afterwards is a guess — which the layering rule forbids. So the
rule that shapes a space declares it while it still has the box in hand.

`mark` wraps a body (like `reorient`) rather than being a statement, because a
rule body is one node: that way a mark can sit on any child of any split and
annotate exactly the piece that child owns. Use `{"op": "skip"}` as the body when
the declaration is all that is wanted.

```json
{ "op": "mark", "mark": { "anchor": "courtyard", "at": "floor_center" },
  "body": { "op": "void" } }
```

| Field | Meaning |
|---|---|
| `anchor` | kebab-case stem. The exported key is `anchor/<stem>`, i.e. the DSL's `anchor/<kebab>` id — a mark cannot name an anchor the DSL could not reference. |
| `at` | which cell (flattened into the mark object, see below) |
| `facing` | `north`/`south`/`east`/`west`. Omitted, it is **derived**: a grammar orientation is a permutation without reflection, so the derived facing is the negative direction of the world axis the scope calls local `Z` — `north` when that is world `Z`, `west` when it is world `X`. A scope whose local `Z` is *vertical* has no cardinal facing and says so rather than guessing. |
| `index` | `unique` (default) → `anchor/<stem>`; `auto` → `anchor/<stem>-<n>`, `n` counting from 1 per stem in expansion order — how a rule that runs once per tower gives every tower an anchor without knowing how many there are. Matches the hand-built `anchor/alcove-1…` convention. |

`at` is one of:

| `at` | Cell |
|---|---|
| `corner_min` | the scope's minimum corner |
| `floor_center` | lowest **world** `Y`, centred on world `X`/`Z`. Gravity is a world fact, so this one position ignores the scope's local axis names |
| `face_center` (+ `axis`, `side`) | the given **local** axis pinned to `min`/`max`, the other two centred |
| `offset` (+ `x`, `y`, `z` expressions) | **local** cells from the minimum corner |

Centres round down on an even extent (the lower-middle cell) — it has to be one
of the two, and the same one every time (ADR-0006).

Marks collect into `Expansion::anchors` (a `BTreeMap`, keyed by exported name),
**not** into the `VoxelModel`: a mark writes no blocks, and folding metadata into
the block grid would change what `canonical_bytes` means. The export writes them
into the prefab metadata's `anchors` map in the hand-built `{pos, facing}` shape,
`pos` local to the structure; `PrefabRegistry` reads a grammar prefab's anchors
with the same code path as a hand-built one (`crates/compiler/tests/grammar_prefab.rs`).

Refusals: a non-kebab stem is a `Program::validate` error (before any expansion);
a mark aimed outside its own scope, an underivable facing, and two marks
producing the same name are expansion errors — the collision names both rules.
Two marks on the same **cell** under different names are legal, as in the
hand-built prefabs.

## 3. Determinism (ADR-0006)

Same program + same region + same seed → byte-identical `VoxelModel`, asserted by
a double-expand test over every library program at five seeds, plus a
seed-sensitivity test over a probabilistic program, and over the declared
anchors (names, cells and per-stem numbering alike). All randomness is one
splitmix64 stream from the caller's seed; all maps are `BTreeMap`; cells iterate
`x`, then `y`, then `z`; nothing reads the clock, the environment or a path.
`VoxelModel::canonical_bytes` is the comparison/hash form.

Expansion holds no global state — two programs cannot influence each other, which
is regression-tested.

The same promise is asserted one layer out, on the bytes that actually ship: a
double-**export** test over every library program at four seeds compares the
`.nbt` and the metadata JSON byte for byte (§6).

## 4. Failure is loud

The interpreter has no silent degradation. `Program::validate` runs before any
expansion (unknown rule/role/param, empty rule or split, child/piece mismatch on
a non-repeating split, zero weights, an `orientation` guard that is not a
permutation — a guard nothing could ever match — and a `mark` whose anchor stem
is not kebab-case). During expansion: `NoApplicableRule`,
`Split{Overflow|ZeroStride}`, `Orient`, `BadSize`, `Eval`, `PaletteFull` (more
than 65 536 distinct block states in one model), `MarkOutsideScope`,
`MarkFacingNotCardinal`, `AnchorCollision`, and the `DepthLimit` / `ScopeLimit` /
`VolumeLimit` budgets. Errors carry the rule name and print as prose, never as a
`Debug` struct.

The three budgets live on `Limits` and are inputs, never silent clamps:
`max_depth` and `max_scopes` turn an unguarded recursive rule into a diagnostic
instead of a hang; `max_volume` (default 2²⁴ cells) is checked *before* the dense
model is allocated, so an absurd region is an error rather than an OOM kill —
the one failure mode that reports nothing at all. A caller who means to build
something enormous raises the limit explicitly.

Writing outside the model's own region is a caller defect, not an input:
`VoxelModel::set` asserts it in a debug build and drops the write (never wraps
it) in a release one.

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
| `castle` | `large_tower`, `small_tower`, `great_hall`, `wall_height`, `wall_width`, `tower_height`; role `stone`; declares `anchor/courtyard` | both horizontal extents ≥ `2*large_tower + 2`, Y ≥ `tower_height + 1` |
| `church` | guards only; roles `wall`, `glass`, four `roof_*` stair facings, two door pairs | height must follow width (the roof steps in 2 per course): Y ≥ 9 and Y ≳ X − 3; 15 × 16 × 30 is comfortable |

Ports are faithful except where a module says otherwise; the three substantive
divergences are recorded at their code: the temple's colonnade repeats to fit the
box instead of being fixed at four columns, the church's one-wide ridge course is
guarded (upstream splits it anyway and writes outside the region), and
constraint `largest` returns the maximum (upstream returned the minimum — a
copy/paste bug).

Two library claims are pinned by arithmetic rather than prose, because prose
rots: the temple's colonnade really does follow the box, and a nine-deep box at
`column_size` 1 really does give upstream's four columns
(`tests/library.rs`).

## 6. Export — freezing an expansion as a prefab

`export::export_prefab(program, region, options, id)` produces the two files a
prefab library holds: `<id>.nbt` (a vanilla structure template) and `<id>.json`
beside it. It takes the *program*, not a finished model, and expands it itself —
which is what makes the provenance row unforgeable, since the hash and seed in
the metadata cannot describe a different expansion than the one that produced
the bytes.

The `.nbt` comes from `delvewright-schem`'s `build_region`, the emitter the
`.schem` asset pipeline already uses: one structure writer, one set of
determinism guarantees (sorted palette, `x`→`y`→`z` cell order, gzip mtime 0).
A structure template is local-coordinate, so the region's **origin** does not
reach the output; its **size** does, and is the declared `structure.size`.

The metadata is the hand-built shape, minus what expansion cannot know:

```json
{
  "prefab_id": "prefab/grammar-temple",
  "structure": { "file": "grammar-temple.nbt", "id": "grammar-temple",
                 "size": [13, 14, 21], "data_version": 4671,
                 "generator": "crates/grammar" },
  "anchors": { "anchor/courtyard": { "pos": [20, 0, 12], "facing": "north" } },
  "lighting": { "profile": "unmeasured" },
  "license": { "source": "original", "spdx": "GPL-3.0-or-later",
               "note": "…", "provenance": "…",
               "generated_by": { "generator": "grammar", "program": "temple",
                                 "program_hash": "sha256:…", "seed": 7 } }
}
```

- **`generated_by`** is the spec-0027 §2 provenance row. `program_hash` is
  `sha256` over the program's canonical serde JSON bytes — content-addressed, so
  a program built in Rust and the same program parsed from JSON hash alike.
- **`anchors`** is exactly what the program's `mark` declarations produced (§2b),
  in the hand-built `{pos, facing}` shape with `pos` local to the structure.
  Nothing infers one from the block pattern afterwards — that is precisely the
  downstream folklore the no-hack rule forbids — so a program that marks nothing
  exports `{}`, and an anchorless prefab loads and indexes normally.
- **No `connectors` key.** Jigsaw socketing of grammar prefabs waits on the
  tileset conventions; a guessed socket is worse than none.
- **`"profile": "unmeasured"`.** A lighting profile is a *measurement*, taken by
  the live 1.21.11 probe. Expansion places blocks, not photons, so it declares
  the true thing and admission to a campaign still runs the probe. `unmeasured`
  is not a synonym for an absent `lighting` block: absence means legacy metadata
  predating the field, this is a positive statement that a measurement is owed.
  A `lit`/`dim`/`dark` declaration still cannot omit `measured_min_light` /
  `measured`, and an `unmeasured` one may not carry them (`delvewright-dsl`
  refuses both at parse).

Refusals, all loud: an `id` that is not a lowercase-kebab path segment, an empty
region, a region past the vanilla 48-per-axis structure cap (tiling a prefab
into parts is a jigsaw design, not an export detail), and a model containing a
block the structure safety strip would replace with air — a grammar that asked
for a command block meant to, so shipping a silent hole is refused instead.

`PrefabRegistry` (the engine's reader) loads the result with no diagnostics;
`crates/compiler/tests/grammar_prefab.rs` tests that seam from both sides.

## 7. Not built yet

The §4 craft diagnostics, jigsaw connector emission, the JSON schema stage in
front of the IR, and the contact-sheet/curation loop. Later phases of spec-0027.

`mark` declares point anchors only. Gate-region anchors (`region` + `block`),
trap anchors (`dispenser`, `trigger_block`) and the entry names the engine
treats specially (`spawn`, `entry`) are expressible in prefab metadata but not
yet by a rule — each needs its own declaration, not a widened `mark`.
