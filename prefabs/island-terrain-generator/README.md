# prefabs/island-terrain-generator — "nobodys-cave island" TERRAIN generator

Deterministic generator for the **terrain** half of the nobodys-cave-island remake
(design brief §1, §5): the greenfield connectors and the mountain terminal. A
sibling of `prefabs/island-generator` (the set-piece gen — beach camp + galley) and
`prefabs/cave-generator`: its own `[workspace]`, outside `crates/`, so it never
enters the shipped `delvec` binary and the keep/cave/set-piece `.nbt` output stays
byte-identical (ADR-0006). It reuses the cave-generator primitive family
(splitmix64 PRNG, trilinear value-noise palette field, vanilla-structure `.nbt`
emit, keep-socket geometry, the gravity-substrate invariant, a derived static
block-light estimate); no third-party material ingested.

```sh
# from repo root — writes island-{greenfield,greenfield-bend,mountain}.{nbt,json}
cargo run --manifest-path prefabs/island-terrain-generator/Cargo.toml --release -- \
  <content-repo>/prefabs/
```

Byte-identical on every run (double-run hash-checked). Aligns to the shared island
convention in `../island-tileset.md`: **`island:socket` at `floor_y=2`, walk plane
at local y=3, waterline y=2** — every piece is built at the ground datum then
lifted +2 onto a solid substrate (`lift_substrate`), so all sockets/anchors land on
the shared datum and mate with the beach camp's north socket.

## Pieces

| id | role | size (X×Y×Z) | sockets | interior |
| -- | ---- | ------------ | ------- | -------- |
| `island-greenfield` | connector | 17×10×15 | S, N (`island:socket` floor_y=2) | open meadow |
| `island-greenfield-bend` | connector | 17×10×15 | S, E | open meadow |
| `island-mountain` | terminal | 36×28×42 | S (base) | **30×14×24 cavern** |

- **greenfield / greenfield-bend** — open-air, sky-lit grazing meadow in a shallow
  grassy dell: flat walkable floor, a worn dirt path spine between the two sockets,
  scattered scrub oaks, poppy/daisy/cornflower flowers, and a low mossy-cobblestone
  **empty sheep fold** (foreshadowing — the sheep are the Cyclops'). The bend variant
  elbows S→E for layout flexibility. Anchors: `anchor/meadow`, `anchor/fold`.
  Oaks keep the corridor clear **by shape, never by cutting** (see below).
- **island-mountain** — a solid rock massif built **fill-then-carve**. A terraced
  **switchback path** (grass-to-stone gradient + a coarse-dirt trail, stair treads on
  every riser so it is walked natively) climbs the south face to a **cave-mouth
  ledge** with a **boulder gate region** (`anchor/boulder`, basalt) beside the mouth.
  The mouth opens into ONE tall-wide **cavern hall** (30 wide × 14 tall × 24 deep,
  NOT rooms-and-corridors): a cheese store by the entry, a central fire pit, a
  **rock-shelf ramp** (no ladders) up to an empty upper sheep pen, four dark **shadow
  alcoves** (stealth), dripstone + moss dressing, and **two ceiling light shafts**
  open to the sky. Anchors: `mouth`, `boulder`, `cheese-store`, `fire-pit`,
  `ramp-top`, `pen`, `alcove-1..4`, `checkpoint-1..3`, `shaft-1..2`.

## Determinism, gravity & lighting

- **Determinism** (ADR-0006): every stream seeded from a per-piece PRNG + value
  noise; gzip mtime pinned to 0; fixed iteration order. Same seed → byte-identical
  `.nbt`.
- **Gravity substrate**: every gravity block (sand/gravel floor dressing) rests on
  solid rock by construction; `assert_no_unsupported_gravity` fails generation if any
  floats over the void (the pitfall is automated out of existence at the tooling
  layer — the compiler's `DW0313` is the authoritative gate).
- **Lighting** (derived, honest): greenfield is open-air → `lit` 15. The mountain
  cavern is declared **`dark`** with a rationale — firelit at the central pit, dark at
  the vault edges and in the four shadow alcoves **by design** (the stealth beats).
  Abundant rock fixture sites (walls/ceiling within relight radius of every cell) and
  two sky-open shafts exist; the compiler re-measures the assembled world and relights
  declared areas minimally (spec-0010). The per-piece estimate is block-light only
  (sky shafts not counted) — a conservative authoring value, not a live probe.

## Greenfield canopy rule (owner QA round 4)

Oak trunks are always planted off the walk corridor, but an earlier version kept the
corridor clear by **skipping** any leaf that fell over it — which left vertically
sheared half-trees with a flat plane of leaves flush against the path ("half-sliced
trees are not a fix", owner). The corridor is now respected **structurally**, and a
canopy is only ever grown as a whole blob:

1. **Lean** — an oak whose leaf ball would reach the corridor leans its blob one block
   directly away from the nearest corridor cell (oaks crowd off trodden ground; the
   offset blob still caps the trunk). If that clears it, the oak keeps its natural
   3–4-log height.
2. **Grow** — if leaning is not enough, the oak grows tall instead: the trunk rises so
   the *entire* canopy sits at `G_WALK_Y + G_CANOPY_CLEARANCE` (walk plane +3) and
   arches over the path as a leafy ceiling.

`G_CANOPY_CLEARANCE = 3` — the nav model needs 2 cells (player height 1.8); the third
keeps the canopy reading as a ceiling rather than a hat, and leaves margin for the
compiler's `DW0311` walk. Only the walk plane is constrained: overhang **above** +3 is
wanted, not merely tolerated.

`assert_greenfield_corridor_clear` is the generator-side proof (debug doctrine —
the lesson is pinned as a generator invariant, not prose): every corridor cell must sit
at the meadow datum and be free of any block, dressing or otherwise, up to the
clearance height. Generation fails rather than shipping an obstructed or re-sliced
corridor. `greenfield_on_walk` is the single corridor definition all three consumers
(scatter, canopy shape, invariant) share, so they cannot drift.

## Walkability (verified)

A 3D nav-style flood from the base entry socket reaches every mountain anchor —
switchback → mouth → cavern → rock-shelf ramp → upper pen — with ≤1-block steps
(stair-dressed risers); both greenfield sockets are through-connected. The compiler's
`DW0311` critical-path check is the authoritative gate at campaign assembly.
