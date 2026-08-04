# "tidal keep" prefab tileset

Six original prefabs staging a souls-mode delve on a drowned shore: the barrow
strand you land on, the gatehouse you time your way through, the parapet that
carries you to the courtyard hub, the flooded undercroft that holds the loop, and
the bell tower that ends it. Built deterministically by
`prefabs/tidal-keep-generator` (ADR-0006), a sibling of the cave / island
generators; all-original assets (ADR-0013).

Anchor names are the **only vocabulary the DSL has for these pieces** — the
inventory in "Anchors" below is the authoring surface. Nothing else about a
prefab is addressable from a campaign.

## Convention — `tk:socket`, on two datums

keep-socket-v1 geometry (3-wide × 3-tall opening, one `minecraft:jigsaw` at the
opening's bottom-centre wall cell, `joint = aligned`, `final_state = air`,
`name == target` so any two sockets mate) under a `tk` vocabulary. Two floor
datums, because the keep **rises**:

| datum | socket `local_pos.y` | solid floor top | walk plane | used by |
| --- | --- | --- | --- | --- |
| **shore** | 3 | local y=2 | local y=3 | `tk-barrow-field` (both), `tk-gatehouse` (south) |
| **keep plinth** | 11 | local y=10 | local y=11 | everything else |

The shore datum is the island convention (`island-tileset.md`): waterline local
y=2, walk plane y=3. Only **`tk-barrow-field` declares `waterline_y`** — it is
the one piece that authors sea, and under `horizon: ocean` it is the area's entry
piece placed at the y=60 datum, so its waterline lands at sea level (`DW0344`).
Every other piece omits `waterline_y` on purpose: they stand on the plinth, above
the tide, and a declared waterline would demand they sit at sea level.

**Walk-plane datum (spec-0026 §2)**: each piece declares its `walk_y` — the
feet-y of its **lowest** socket floor: shore pieces 3 (`tk-barrow-field`,
`tk-gatehouse`), plinth pieces 11 (everything else); generator-derived from the
piece's door list. The area datum reads the entry piece (`tk-barrow-field`,
walk_y 3 → base `63 − 3 = 60`, byte-identical to the retired global ocean
datum); the plinth pieces' values are declaration honesty for the empirical
flood proof (`DW0364`), which verifies every piece's standable cells sit above
sea level regardless of what anything declares.

**All vertical gain is authored inside a piece.** The solver has no vertical
socket (`Facing::parse` accepts cardinals only), so a piece's rise is exactly the
difference between its two sockets' local y — the `keep-stair` rule, applied at
building scale. `tk-gatehouse` carries the whole +8 climb from shore to plinth;
the rest are level-to-level and gain their height internally.

## Assembly

One area, one pool, six pieces, no fillers — a single deterministic spine:

```
tk-barrow-field ─N─ tk-gatehouse ─N─ tk-wall-walk ─N─ tk-courtyard-chapel ─E─ tk-cistern ─E─ tk-bell-tower
   (entry)          +8 rise            level             hub                  loop            terminal
```

```json
"pool/tidal-keep": { "members": [
  { "prefab": "prefab/tk-barrow-field",     "weight": 1, "role": "entry" },
  { "prefab": "prefab/tk-gatehouse",        "weight": 1, "role": "room" },
  { "prefab": "prefab/tk-wall-walk",        "weight": 1, "role": "room" },
  { "prefab": "prefab/tk-courtyard-chapel", "weight": 1, "role": "room" },
  { "prefab": "prefab/tk-cistern",          "weight": 1, "role": "room" },
  { "prefab": "prefab/tk-bell-tower",       "weight": 1, "role": "terminal" }
] }
```

Bind it with **`"pieces": { "min": 6, "max": 6 }`**. The pool has no
`connector`-role member, so any filler budget at all is `DW0301`; pinning min ==
max == 1 + required count removes the draw entirely and makes the layout
seed-independent.

**Piece order is controlled by anchor spelling, not by luck.** The solver threads
through-rooms in the order of the *lexicographically first* required anchor that
pulls each piece in. Every anchor here is prefixed with its level (`l0-`, `l1a-`,
`l1b-`, `l2-`, `l3-`, `l4-`), so the sorted order is the intended order of play.
Renaming an anchor without keeping the prefix reorders the keep.

## Pieces

| id | size (X×Y×Z) | sockets (`tk:socket`) | lighting | role |
| -- | ------------ | --------------------- | -------- | ---- |
| `tk-barrow-field` | 48×14×40 | N `[24,3,0]` | `lit` 15 (sky) | entry / L0 shore |
| `tk-gatehouse` | 28×24×46 | S `[14,3,45]`, N `[14,11,0]` | `lit` 7 | L1, +8 rise |
| `tk-wall-walk` | 16×16×34 | S `[7,11,33]`, N `[7,11,0]` | `lit` 15 (sky) | L1 parapet |
| `tk-courtyard-chapel` | 46×26×46 | S `[23,11,45]`, E `[45,11,23]` | `lit` 8 | L2 hub |
| `tk-cistern` | 42×22×40 | W `[0,11,19]`, E `[41,11,19]` | `dim` 5 | L3 undercroft |
| `tk-bell-tower` | 26×44×26 | W `[0,11,13]` | `lit` 7 | L4 terminal |

### `tk-barrow-field` — the shore

Beach landing on the ocean datum, rising into an open barrow field bounded by
coastal bluffs (no walkable cell borders the void; the strand is climb-out-able
from the sea, so `DW0322` has its step). Four sealed burial mounds, driftwood, a
stone fire-ring, and a leaning banner pole beside the mound the elite sleeps
against.

The piece exists to stage the **optional elite**. Its legibility is geometry, not
signage: the field is deliberately wide and empty on both flanks, and the
generator *proves* it — `assert_field_open` rejects any mound that comes within
one block of the centre desire line or either flank lane, and all three routes are
walked cell by cell before the piece is written. An elite whose flank ground is
blocked is not optional, and the bypass proof would be arguing about geometry the
prefab got wrong.

### `tk-gatehouse` — the timed gate and the boulder stair

Three beats in one piece, because they must fork and rejoin (see "Loops", below).

- **The portcullis** (`anchor/l1a-gate-timed`, a 5×3 `iron_bars` region on the
  z=36 plane) with the slot it retracts into authored above it. Six blocks out, a
  roofed **watch bay** is set into the east wall of the approach court with clean
  line of sight straight up the passage: the cycle is readable *before* anyone
  commits, which is what makes a timing gate a read rather than a coin flip.
- **The boulder stair** — a twenty-cell straight run, eight risers. The **wear
  gradient is the tell**: the centre lane the stone rolls down has lost its brick
  face and is polished `smooth_stone` / `polished_andesite`, while the untrodden
  flanks keep mossy and cracked brick. In plain sight, and free.
  `anchor/l1a-trap-boulder` is the plate row mid-run; its dispenser is set into an
  arch rib above and *ahead*, so the charge comes down the run at the climber.
- **The runout alcove** at the stair head holds the **spill shaft** — a kerbed
  well dropping into a water trough in the gatehouse undercroft, which walks back
  to the ward and BF1. The thing that kills you is the shortcut you learn.
- **The mural stair**, a narrow flank climb up the east wall: the boulder-free
  counterplay, paid for with an ambush doorway at mid-height
  (`anchor/l1a-mural-door`).

### `tk-wall-walk` — the parapet

A solid curtain wall (mass filled to the plinth datum) with a crenellated walk on
top, one collapsed merlon run, braziers, and a **roofless ambush turret** cut into
the west parapet. Whatever stands in its doorway is backlit by open sky and
readable from the far end of the run — the TEACH ambush is fair by sightline
alone, never by telegraph. Embrasures keep a `stone_brick_wall` course: the nav
model treats walls as 1.5-tall barriers you can neither pass nor stand on, so you
can look through a crenel and never fall out of it.

### `tk-courtyard-chapel` — the hub

A large open muster yard with **two physically distinct breach lanes** — a
collapsed section of the south curtain and one of the west curtain, each with its
own rubble ramp and its own three-waypoint chain, converging on
`anchor/l2-muster`. Consecutive lane waypoints are more than ten blocks apart
(the vanilla patrol goal re-rolls its target inside ten). Yard dressing is placed
by a predicate that shares one definition with the lane spines, so spoil can never
creep onto a lane.

The chapel occupies the **east range, so its flank wall is the piece's east face**
and the undercroft socket opens straight out of it — "the stair down is in the
chapel" is true in geometry, not just in prose. Inside: the hearth (BF2, the
regroup/dialogue stage), the cracked bell on a stone-and-timber frame with its
spill of shattered masonry, and the altar.

### `tk-cistern` — the undercroft and the loop

The souls loop is authored **entirely inside this piece** (see "Loops"). From the
vestibule one step off the courtyard:

- **long route** — the commit gate (`anchor/l3-commit-gate`, for a `close-gate`
  point of no return), the descent stair, the flooded hall with its pillar pairs
  and item alcove, the drowned side-cell behind a **visibly broken grate** (two
  bars gone, low and obvious, lantern-glow behind them — no illusory walls
  anywhere in this tileset), the east-wall ledge line, the dart gallery over the
  exit stair, and the far-side landing with `anchor/l3-unlock`;
- **short route** — `anchor/l3-shortcut-a`, sealed `iron_bars` from world-load,
  opening onto a straight upper gallery from the vestibule to that same landing.
  `anchor/l3-shortcut-a-sign` is the near-side plate spot ("this door does not
  open from this side").

**Water is authored as sunken bays whose surface sits level with the dry floor.**
It reads as a flooded undercroft, and every proven route stays dry. That is not a
style choice: the nav model treats water as impassable and never a floor
(`standable` requires a *solid* block below), so a wading route cannot be proven
at all. The bays are the hazard zone; the causeways and the ledge are the walk.

The **east-wall ledge** undulates by exactly one block — free but never flat.
Steps are ±1 cell and every full-block rise gets its head-sweep cell proved clear,
so it is "jumpy" within what the model can actually see.

The **dart gallery** sits over a deliberate two-tread landing in the exit stair.
The flat section exists so the wire duct behind the east wall can out-climb the
stair and put the dispenser genuinely overhead; without it the gallery would end
up at ankle height. Its disarm (`anchor/l3-dart-lever`) is a barred cage in the
landing's east corner — visible the moment the climb ends, reachable only by
walking round its north jamb. Seeing it is free; reaching it costs the loop.

### `tk-bell-tower` — rope room, loft, ring

Three stacked rooms around one vertical void.

- **Rope room** (walk 11) — BF3, deliberately *before* the fog line: the runback
  to the ring is two stair flights. The bell pit's water basin sits in its floor.
- **Bell loft** (walk 23) — four rafter perches, and the generator **proves every
  one of them is visible from the loft doorway** (`sightline_clear`, eye to
  silhouette). The rafter grid is shaped by that proof: two longitudinal purlins
  carry the perches, the transverse ties are kept short, and the mid-span tie is
  reduced to two wall braces, precisely so no rafter screens a perch. Change the
  grid and generation fails, loudly. The TWIST ambush is meant to be seen and
  beaten.
- **Boss ring** (walk 33) — an open annulus around the pit with pillar cover and a
  **raised outer walk on the south band, opposite the doorway**: the keeper is
  read from the door across open floor first, and the high ground has to be
  crossed for. No closets, no chokepoint. The belfry is **open to the sky**, so
  the arena is daylit — information before commitment.
- **The bell-rope drop** (`anchor/l4-rope-drop`, an `iron_bars` grate over the pit
  that the campaign's hub-opener clears) falls twenty-two blocks into a three-deep
  water basin beside the rope room's socket, one step from the courtyard. It is
  **one-way by geometry, not by script**: the shaft walls are sheer and nothing
  climbable is used. The stairs stay walkable both ways, so the model always has a
  proven return and nothing can strand.

## Anchors

Every anchor, by piece. `point` anchors are all proved standable at generation
time (air at feet and head, solid floor below); `region` anchors carry the `block`
their gate verbs fill with; `trap` anchors additionally carry the pre-wired
dispenser socket and the trigger blockstate.

One class is deliberately **not** a footing: a *slot* anchor (spec-0022
`volley.from_anchor`) marks the opening a summoned projectile spawns in, so it is
proved **clear and dry** instead of standable — the same condition `DW0446`
enforces one layer later. Asserting standability there would demand a floor under
an arrow loop.

**`tk-barrow-field`** — `spawn`; `anchor/l0-tide-line`, `anchor/l0-bonfire`
(BF1), `anchor/l0-elite-stand`, `anchor/l0-elite-dormant`, `anchor/l0-banner`,
`anchor/l0-flank-west`, `anchor/l0-flank-east`, `anchor/l0-barrow-1..4`,
`anchor/l0-reward`, `anchor/l0-gate-approach`.

**`tk-gatehouse`** — `anchor/l1a-approach`, `anchor/l1a-watch`,
`anchor/l1a-watch-corpse`, `anchor/l1a-gate-timed` *(region, `iron_bars`)*,
`anchor/l1a-ward`, `anchor/l1a-stair-foot`, `anchor/l1a-trap-boulder` *(trap;
dispenser `[14,12,15]`, trigger `minecraft:stone_pressure_plate[powered=false]`)*,
`anchor/l1a-volley-slot` *(slot; the arch-rib opening at `[14,11,15]`, one course
under the dispenser)*, `anchor/l1a-stair-run` *(the volley kill-zone centre,
`[14,8,19]`)*, `anchor/l1a-stair-head`, `anchor/l1a-runout`,
`anchor/l1a-spill-shaft`,
`anchor/l1a-undercroft`, `anchor/l1a-mural-foot`, `anchor/l1a-mural-door`,
`anchor/l1a-roof-door`.

**`tk-wall-walk`** — `anchor/l1b-parapet-south`, `anchor/l1b-parapet-mid`,
`anchor/l1b-parapet-north`, `anchor/l1b-ambush-door`, `anchor/l1b-breach-view`,
`anchor/l1b-lane-1..3`.

**`tk-courtyard-chapel`** — `anchor/l2-gate-door`, `anchor/l2-muster`,
`anchor/l2-bonfire` (BF2), `anchor/l2-cracked-bell`, `anchor/l2-altar`,
`anchor/l2-chapel-door`, `anchor/l2-undercroft-door`, `anchor/l2-tower-view`,
`anchor/l2-well`, `anchor/l2-breach-gate` *(region)*, `anchor/l2-breach-wall`
*(region)*, `anchor/l2-lane-gate-1..3`, `anchor/l2-lane-wall-1..3`.

**`tk-cistern`** — `anchor/l3-vestibule`, `anchor/l3-drop-ledge`,
`anchor/l3-commit-gate` *(region, `iron_bars`)*, `anchor/l3-shortcut-a` *(region,
`iron_bars`)*, `anchor/l3-shortcut-a-sign`, `anchor/l3-shallows`,
`anchor/l3-ambush-a`, `anchor/l3-ambush-b`, `anchor/l3-item-alcove`,
`anchor/l3-ledge`, `anchor/l3-secret`, `anchor/l3-trap-darts` *(trap; dispenser
`[39,9,28]`, trigger `minecraft:stone_pressure_plate[powered=false]`)*,
`anchor/l3-gallery-slot` *(slot; the shaft head at `[38,10,27]`, three treads
above the plate so the climber walks into the fire)*, `anchor/l3-dart-lever`,
`anchor/l3-unlock`, `anchor/l3-landing`.

**`tk-bell-tower`** — `anchor/l4-rope-room`, `anchor/l4-bonfire` (BF3),
`anchor/l4-rope-foot`, `anchor/l4-loft-door`, `anchor/l4-loft`,
`anchor/l4-perch-1..4`, `anchor/l4-ring-door`, `anchor/l4-boss`,
`anchor/l4-ring-west`, `anchor/l4-ring-east`, `anchor/l4-outer-walk`,
`anchor/l4-vantage`, `anchor/l4-rope-drop` *(region, `iron_bars`)*,
`anchor/l4-bell-hang`.

## Loops: why the fork lives inside a piece

The solver assembles a **tree**: every socket mates at most once and every unmated
socket is walled up (`seal_layout`). Two piece-level routes that rejoin — a fork —
cannot be expressed at all. So every fork in this tileset is authored *inside* one
prefab: the boulder stair and the mural stair both live in `tk-gatehouse`; the
long route and shortcut A both live in `tk-cistern`.

That is a design constraint, not a workaround, and it is why the gatehouse and the
cistern are terrain-scale pieces rather than three small ones each.

## Lighting

Sconces sit at **walk + 3**. Not for looks: every block the nav model does not
explicitly list is a full cube to it, torches included, so a "decorative" light at
head height silently walls off a route — and the jump head-sweep cell the model
checks on a full-block rise is walk + 2. Hanging lamps drop from ceilings on
masonry corbels rather than chains (see "Render notes").

- Open-air pieces (`tk-barrow-field`, `tk-wall-walk`) declare `lit` 15 honestly: a
  static block-light BFS is not applicable to a roofless structure. Braziers and
  lanterns supplement after dusk.
- Roofed pieces carry a measured minimum over the standable cells of their
  declared interior volumes (block light only; sky light through the belfry and
  the open yard is **not** counted — a conservative authoring value, and the
  compiler re-measures the assembled world under its darkest reachable sky).
- **`tk-cistern` is `dim` (measured 5) by design**, not `dark`. It sits above the
  compiler's `DARK_THRESHOLD`, so the undercroft keeps its gloom with **no
  night-vision grant and no relight declaration** — a campaign can simply leave
  the area undeclared. Declaring `lighting` on it would relight the gloom away.

## Palette

Keep masonry `stone_bricks` / `mossy_stone_bricks` / `cracked_stone_bricks` /
`cobblestone` / `chiseled_stone_bricks`; plinth `stone` / `andesite` / `tuff`;
shore `sand` / `gravel` / `coarse_dirt` over the plinth (never over air);
barrow-field `grass_block` / `podzol` / `moss_block` with `mossy_cobblestone`
mounds; undercroft `prismarine` / `dark_prismarine` / `prismarine_bricks` /
`prismarine_wall` bloom on rotted brick; the worn stair lane `smooth_stone` /
`polished_andesite` against `mossy_stone_bricks` / `cracked_stone_bricks` flanks.
Fixtures: `lantern`, `soul_lantern`, `sea_lantern`, `wall_torch`, `campfire`,
`iron_bars`, `chain`, `bell`, `barrel`. Trap hardware: `stone_pressure_plate`,
`redstone_wire`, `dispenser`. Greeble: `dead_bush`, `short_grass`, `seagrass`,
`moss_carpet`, `pointed_dripstone`, `oak_log` / `dark_oak_log`, `oak_fence`,
`black_wool`, `hay_block`. No command or structure blocks; no NBT-bearing block
entities beyond the dispenser sockets the trap contract requires.

## Generator invariants (what fails the build, not the QA hour)

Debug doctrine: each lesson is pinned as a generator assertion rather than prose.

- `assert_route_walkable` — every promised route (both barrow flanks, the boulder
  stair, the mural stair, the gatehouse undercroft, the parapet, both siege lanes,
  the chapel run, the cistern's long and post-unlock short routes, the tower climb
  and the outer-walk ramp) is walked cell by cell against the **current** nav
  model: standable feet+head over a solid floor, cardinal steps, |dy| ≤ 1, and a
  head-sweep check on every full-block rise.
- `assert_field_open` — no barrow may intrude on the elite's bypass lanes.
- `sightline_clear` — every rafter perch is visible from the loft doorway.
- `assert_anchors_sane` — every *footing* anchor is standable and every *slot*
  anchor is clear and dry, every anchor id is a legal `anchor/<kebab>` (or the
  reserved `spawn`), every trap marker's declared dispenser socket really is a
  dispenser, and every region is in bounds.
- `seal_stair_flanks` + `assert_stair_flanks_sealed` — **a flight is entered at
  its foot, never over its side rail.** Where a floor sits flush one block under
  a mid-flight tread, the nav model reads a perfectly legal side-step onto that
  tread; the tread then carries two climbs at once and whichever way it faces,
  one of them is backwards. The pass newels every such cell (deterministic
  collect-then-apply, ADR-0006) and the assert proves none is left, on all six
  pieces including the ones with no stairs today. Found five open flanks in
  `tk-bell-tower` (both flights rise through open room air; one of them off the
  x=19 purlin) and one at the head of the gatehouse mural stair.
- `wire_dust` — every redstone cell has a solid support below, and every up-step
  in a dust staircase has its connection cell clear.
- `assert_no_unsupported_gravity` — no gravity block over air (`DW0313`'s
  backstop, one layer earlier).

`TK_PROBE=<salt>,<x>,<y>,<z>` dumps a labelled block neighbourhood of any piece —
the tool that found the light finding below, and the one that placed both volley
slots. `TK_DEBUG_STAIRS=1` prints every cell `seal_stair_flanks` closed.

CI runs every generator in `prefabs/` twice into separate trees on each PR
(`prefab-generators` job): a panicking invariant fails the job and the two trees
must be byte-identical. Until 2026-08-03 nothing in CI compiled these workspaces
at all, which is how a tileset with 132 reversed stair blocks reached a playtest
through a green pipeline.

## Render notes

`delve-render piece` (Nucleation) has **no blockstate for `minecraft:chain`**: a
chain-hung lamp reads as a lamp floating in mid-air in every review shot. Lamp
stems are therefore masonry corbels; chain survives only in the bell tower's
ropes, where it *is* the subject. In-game and in Chunky the chains render
normally. Recorded, not worked around.

## Provenance / license

All-original Delvewright assets, per-item license recorded in each metadata JSON
(GPL-3.0-or-later pipeline-code license per `LICENSE-ASSETS.md`; ADR-0013). No
third-party material ingested — the generator reuses only Delvewright's own
cave/island generator primitives.

## Regenerating (deterministic, ADR-0006)

```sh
cargo run --manifest-path prefabs/tidal-keep-generator/Cargo.toml --release -- \
  <content-repo>/prefabs/
```

Byte-identical on every run (double-run hash-checked). The generator is a
standalone crate with its own `[workspace]`, **outside** `crates/`, so it never
enters the shipped `delvec` binary and no existing `.nbt` output moves.
