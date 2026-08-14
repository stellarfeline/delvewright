# Horizon library research dossier

Research base for spec-0026: five horizons beyond
`ocean` — sky, flatland, valley, cherry-valley, summit. This dossier holds the
algorithm survey (with licenses), the MC 1.21.11 constraint audit, the perf
budget math, and the analyses behind the spec's open owner decisions. The spec
is the contract; this file is the evidence.

Terminology is aligned with the M6 macro-terrain research line (journey-graph
landform layout, seamless heightfield blending, carved waterways, the vanilla
density-function ceiling). This work is the first concrete slice of that line:
**single-scene surrounds**, not journey-scale landform graphs. Carved waterways
and multi-scene blending stay there.

## 1. How ocean does it today (the path we generalize)

From `docs/reference/compiler.md` and `crates/compiler/src/plan.rs`:

- `horizon: ocean` swaps the emitted `generator-settings` for a pinned superflat
  (`bedrock×1, stone×118, water×8` → water top at y=62). The **ambient** — what
  any column outside a placed piece's AABB holds — is therefore analytically
  known, and `nav::Ambient` states the DW0322 boundary-safety proof against it.
- Areas are placed at a **single global datum** `plan::OCEAN_BASE_Y = 60`
  (= `SEA_LEVEL − ISLAND_WATERLINE_Y`), authored for the island tileset (walk
  plane local y=3). `DW0344` checks `piece.y + waterline_y == 62` but exempts
  every piece that declares no `waterline_y`.
- The validation/prod images read `level-type` + `generator-settings` from the
  **emitted server.properties** (`validation/Dockerfile.delve`,
  `check-world-settings.sh`) — the world-settings parity fix. Any new horizon
  that flows exclusively through this channel inherits PackTest world parity for
  free.

**The datum bug class**: the single global datum is
an island-tileset constant applied to every tileset. An interior `keep-*` piece
(walk plane local y=1) placed at base 60 puts its walk plane at world 61 — one
block under sea level — and the exemption clause means DW0344 never looks. The
assembled model says dry; the delivered world floods on boot; every proof
downstream of placement (walkability, lighting, checkpoints, POV, PackTest) is
derived from the wrong placement and stays green. Design consequence for the
spec: **the datum must be computed per area from the pieces' own walk plane**,
and the flood check must read **empirical assembled geometry**, never an
optional declaration.

## 2. Algorithm survey

Delvewright already owns a deterministic noise family: **position-addressed
value noise**, seeded per stream label, used by the island/cave/tidal-keep
generators and ported into `crate::edit` (see ACKNOWLEDGEMENTS: the
frightful_hobgoblin techniques were re-implemented ideas-only). Policy for this
spec: **compose the in-house family first**; the external citations below are
candidate upgrades, each with its license verdict recorded now so a future port
PR only has to copy the row into `docs/ACKNOWLEDGEMENTS.md`.

### 2.1 Heightfield base (all terrain horizons)

Fractional Brownian motion (fBm): sum of N octaves of value noise,
`h(x,z) = Σ aᵢ · noise(fᵢ·x, fᵢ·z)`, lacunarity ~2, gain ~0.5. Deterministic by
construction: noise is a pure function of (position, stream label, campaign
seed); no iteration-order or wall-clock dependence. DSL exposure: never raw
octaves — horizons expose **intent parameters** (relief amplitude, roughness
0..1) that the compiler maps to fixed octave tables.

| Candidate source | License (verdict) | Technique |
|---|---|---|
| Perlin, *An Image Synthesizer* (SIGGRAPH 1985); *Improving Noise* (SIGGRAPH 2002) | ACM-copyrighted papers; reference impl carries no license → **ideas-only** | Gradient noise; the improved fade curve `6t⁵−15t⁴+10t³` (formula, uncopyrightable) |
| Simplex noise (Perlin 2001) | US patent 6,867,776 **expired 2022** (20y from 2002 filing). KdotJPG **OpenSimplex2** repo: public domain (UNLICENSE) — verify at port time | Lower-artifact gradient noise if value-noise grid alignment ever shows |
| Musgrave, in Ebert et al., *Texturing & Modeling: A Procedural Approach* (3rd ed. 2003) | Copyrighted book → **ideas-only** | **Ridged multifractal**: `ridge = (1 − |noise|)^k`, octave weights scaled by previous ridge — sharp crests for valley rims and summit ranges |
| Quilez, iquilezles.org articles (fBm, domain warping) | Article code license **unverified** (historically mixed) → treat **ideas-only** unless verified at port time | Domain warping `h(p + w·noise₂(p))` to break radial symmetry of rings |
| Fournier, Fussell, Carpenter, *Computer Rendering of Stochastic Models* (CACM 1982) | Classic published technique | Diamond-square — **rejected**: axis-aligned artifacts, grid-size coupling; fBm strictly better here |

### 2.2 Radial ring composition (valley / cherry-valley)

Own construction, no external source needed: the rim is a radial profile
modulated by ridged noise. With `d` = distance from the scene AABB (Chebyshev or
smoothed-rectangle distance, so the ring follows the scene's footprint, not a
circle):

```
h(x,z) = floor_y                                    for d < gap
       + rim_height · S(d; gap, crest) · R(x,z)     ramp up to the crest
       − decay beyond the crest toward the ambient   (outer skirt)
```

`S` = smoothstep ramp, `R` = ridged multifractal in [0.3, 1] so no rim segment
degenerates to a walkable gap. Domain-warp `d` slightly so the ring reads as
mountains, not a stadium wall. **Un-climbability is proven, not parameterized**:
the assembled surround is in the voxel model, so "the inner slope has no
standable staircase from gap floor to crest" is a nav check, not a slope-angle
promise.

Cherry variant: identical composition; only the surface palette table and the
`plant` species parameter change (tree = `cherry`, understory = pink petals).
The existing `plant` verb's lean-or-grow canopy rules and `scatter`'s
noise-ordered spacing (ported from the island terrain generator, see
compiler.md §7 map-editor verbs) are reused as-is. Poisson-disk sampling
(Bridson, SIGGRAPH 2007 sketch, ideas-only) is a candidate upgrade but the
in-house spacing idiom is already deterministic and adequate.

### 2.3 Plateau shaping (summit)

Own construction: plateau = flat top at `plateau_y` with a noise-perturbed rim
falling into gorges; surrounding range = ridged fBm with amplitude clamped
**below** `plateau_y − clearance` (the "一览众山小" invariant is a hard clamp,
not a tuning outcome — machine-checkable: max ambient-surround surface y <
scene walk plane y). Gorge floors from a low base level; drop depth is
`plateau_y − gorge_floor_y ≥ 100` by parameter, verified empirically on the
assembled model along the vista ring.

### 2.4 Edge dithering (flatland seam)

The owner's requirement: grass and scene floor **interpenetrate** at the
boundary; explicitly no stone pedestal in a meadow. Same algorithm family as
the tileset round-2 edge work (frightful_hobgoblin A1 value-noise weathering
with edge-distance bias — already in ACKNOWLEDGEMENTS, ideas-only):

- Zero height difference by construction: the ambient superflat's grass top and
  the scene's walk plane share one world y (a datum equation, not a blend).
- Material blend band of width `w` (default 6) straddling the scene edge: per
  column, `P(scene material) = clamp(0.5 + s/w + j·noise(x,z), 0, 1)` where `s`
  = signed distance into the scene. Threshold the noise → deterministic
  speckle; no column changes height, only surface material (grass_block ↔ the
  scene's floor palette), plus grass/fern tufts scattered back across the seam
  both ways.
- Bayer ordered dithering (Bayer 1973, classic published technique) is the
  structured alternative; **rejected** — its regular matrix reads as a printed
  pattern at block scale; blue-ish value-noise threshold matches the tileset
  family and looks organic.

### 2.5 Vanilla density functions — evaluated, not adopted (the macro-terrain ceiling)

The macro-terrain line flags MC's density-function worldgen. For single-scene
surrounds it is the wrong tool, and the spec should record why so it is not relitigated:

- **The proofs cannot see it.** Our center of gravity is model soundness: nav,
  DW0322, lighting, stranding all read the assembled voxel model. Density
  functions generate server-side at chunk load; making the compiler know the
  result means re-implementing vanilla's noise bit-exactly — a folklore-grade
  hack the no-hack doctrine excludes. (The ocean superflat is the degenerate
  case where the ambient IS analytically known; noise terrain is not.)
- **Coordinate pinning is a hack.** "A valley exactly here" via density
  functions means baking world coordinates into worldgen JSON — downstream
  folklore of the worst kind.
- What density functions remain good for (macro-terrain scope, unresolved there):
  infinite un-modelled backdrop beyond the proof horizon. Out of scope here.

**Verdict**: surround terrain is generated **compiler-side into prefab tiles**
(the greenfield/island precedent), placed by the same bootstrap `/place
template` path as scene prefabs, fully present in the assembled model. The
ambient beyond the tiles stays an analytic generator per horizon (superflat or
void). Zero new emission machinery.

## 3. MC 1.21.11 constraint audit

- **Build range**: −64..320 (384 blocks). Summit fits: gorge floors ~y 40–80,
  plateau ~y 190–230, surround crests below plateau; headroom for scene
  structures under 320 verified by the existing assembly bounds.
- **Superflat surface height** is set by layer count — the flatland ambient can
  put its grass top at any y, which is what makes the zero-height-difference
  datum a config equation. Superflat accepts a `biome` (plains for flatland →
  vanilla grass color; `the_void` preset for sky/valley/summit ambient).
- **Sky ambient** = the void preset already emitted for `horizon: void`
  (`{"biome":"minecraft:the_void","layers":[]}` per `validation/Dockerfile.delve`).
  Vanilla void damage below y −64 is the lethal mechanism — no datapack kill
  plane needed (vanilla-first).
- **Structure templates**: tiles authored at ≤48×48 XZ (the library's safe
  template envelope), trimmed vertically per tile to its local terrain span
  (structure NBT stores every cell of the declared size; vertical trim is the
  size lever that matters).
- **View distance**: server `view-distance` 10 → 160 blocks. A summit vista
  must be generated out to at least the shipped view distance or peaks pop out
  of existence at the fog line; conversely every generated block beyond it is
  dead weight on the Pi. This couples `vista_radius` to the shipped
  server.properties `view-distance` — an explicit spec parameter, and the
  campaign README should state the client floor (owner decision; player-facing
  docs may carry it per the audience-separation rule).
- **PackTest parity**: the toolserver/delve images derive worldgen
  from emitted server.properties. New horizons emit `level-type` +
  `generator-settings` through the same channel and extend
  `check-world-settings.sh` coverage; the packtest world is then the shipped
  world by construction.
- **Lighting**: vanilla lights placed chunks at first boot (one-time cost,
  counted in the boot budget). The compiler-side relight (spec-0010) and
  DW0210/0211 proofs stay scoped to the reachable region — surround tiles add
  voxel-model volume but no new lighting obligations.

## 4. Perf budget math (prod = Raspberry Pi, arm64)

Model: gzip'd structure NBT for run-heavy terrain ≈ 1–3 B/block (measured
against the existing island prefab library's ratio at implementation time — the
numbers below are sizing estimates, to be replaced by a measured spike before
budgets become binding). Reference scene: 96×96 footprint.

| Horizon | Surround volume (est.) | Shipped delta (compressed) | Tiles | First-boot delta (Pi) | view-distance |
|---|---|---|---|---|---|
| ocean | none (ambient only) | 0 | 0 | baseline | 10 |
| sky | none (void ambient) | ~0 | 0 | ~0 | 8 |
| flatland | seam band: perimeter×6×3 ≈ 7k blocks | < 1 MB | ≤ 4 | negligible | 10 |
| valley / cherry-valley | annulus to 2.5× footprint (240² − 96²) ≈ 48k columns × ~30 modelled depth ≈ 1.5M blocks | ≤ 25 MB | ≤ 48 | ≤ +120 s | 10 |
| summit | vista disc r=176 ≈ 124k columns × ~60 depth ≈ 7.5M blocks | ≤ 60 MB | ≤ 96 | ≤ +300 s | 12 (floor) |

Summit is the perf outlier by ~5× and its budget must not become binding
without a measured spike (generation, image size, first boot on arm64, chunk
lighting time). Voxel-model memory at summit scale (~352×384×352 ≈ 47M cells)
is workstation-fine but worth a compile-time RSS check in the spike.

## 5. Boundary enforcement analysis (per horizon)

**Every horizon reuses the spec-0013 boundary primitive** (derived region + 1 s
return-to-checkpoint clock), exactly as the island ships it. The spec's job is to generalize that primitive as
horizon-agnostic — the region derivation and clock never branch on horizon
kind; only the region's *vertical extent* does (sky, below). Flatland is not an
open decision.

| Horizon | Natural boundary (fiction layer) | Enforcement (one mechanism) |
|---|---|---|
| ocean | water + no climb-out beyond beaches | spec-0013 return clock (unchanged) |
| sky | fall = death (consequence parameterized; see below) | return clock — **vertical OOB unified with horizontal** |
| valley / cherry-valley | un-climbable inner slopes (nav-proven) | return clock backstop in the gap zone |
| summit | gorges (100+ drops; fall = death outward) | return clock backstop on the plateau |
| flatland | none — zero height difference is the point | return clock (decided) |

**Sky falls ride the boundary primitive, not physics** (revised after the
backdrop ruling, §7). The first-draft design let the region hang unbounded
downward so vanilla void damage would kill the faller — sound over a void
backdrop, broken over any other: an ocean or water-feature backdrop makes the
landing survivable and leaves the player alive inside non-interactive scenery.
The unified rule: crossing below the scene's y-envelope (spec-0013's region
floor, lowest placed block − 8) is out-of-region like any horizontal exit, and
the same 1 s clock owns it. The **consequence** is the horizon parameter
`fall` — `lethal` (default: the catch applies
`damage @s 1000 minecraft:generic`, vanilla death fires, and the checkpoint
re-seat lands the corpse's respawn on the armed checkpoint — full souls death
costs, identical over every backdrop) or `return` (plain teleport back, the
flatland behavior). Fall-time check: from `float_y = 160` a faller crosses the
region floor within ~1 s and the clock catches ~20–40 blocks below the scene —
well above any backdrop surface, so the player never lands in scenery. The
environmental-death trials cover representative edges under `fall: lethal`.

**Flatland visual pre-warning (advisory recommendation, not a decision
gate)**: the ocean announces its own edge; flush grassland does not — a
wanderer meets the return clock with zero forewarning. Cheap cues, all inside
the already-modelled seam/cue band and all riding existing machinery
(`scatter`/`plant` idiom + the same value-noise family):

- **Density thinning**: flowers/tufts/trees scattered with density falling to
  zero over the last ~24 blocks before the region edge — the meadow visibly
  "runs out" (one extra distance term in the existing scatter density gate).
- **Paths fading out**: any scene-edge path material dithers into grass over
  the band (the §2.4 blend, applied to path palettes).
- Optional sparse boulder/shrub line at the region edge itself, as a landmark
  rather than a wall.

Recommendation: ship density thinning by default (near-zero cost, pure
parameterization); the rest is campaign dressing.

## 6. Sky archipelago — rooms as islands, bridges as gameplay

A multi-room sky scene is **independent floating islands connected by
narrow paths/bridges**, never one monolithic island. Traversal is intended
souls-flavored risk terrain: falling off a bridge = death → checkpoint
re-seat. This is a spec requirement, not an option.

### 6.1 What the solver does today (verified, `crates/compiler/src/solver.rs`)

- Areas sit on a 256-block grid (`plan::AREA_SPACING`) and connect by
  **transports**, not geometry (DW0311 accepts an inter-area transport in
  place of a walkable link). Cross-area bridges are out of scope; the
  archipelago ruling binds **within an area**.
- Within an area, the solver mates sockets **flush** (child socket one block
  beyond parent, facing back) and "reads socket geometry only" — there are
  **no socket compatibility classes**: any socket mates with any socket, and
  pieces form a contiguous footprint chain. A bridge is representable today as
  an ordinary long, thin piece (its own airspace IS the room gap, exactly the
  jigsaw semantics the ruling anticipates) — but nothing stops the solver from
  mating room directly to room, so alternation cannot be guaranteed by the
  tileset alone.

### 6.2 Compiler-side work identified

1. **Piece roles + mating rule**: prefab metadata gains a connection class
   (`role: room | connector`). In a sky-horizon area the solver's frontier
   attach enforces alternation: a room socket accepts only connector pieces
   and vice versa (a connector–connector mate is legal for long spans). This
   is a solver constraint, deliberately NOT socket-name folklore — the no-hack
   rule wants it first-class. Non-sky horizons ignore the class (existing
   pools carry no connectors and are unaffected byte-for-byte).
2. **Connector sealing semantics**: an unmated **room** socket seals with wall
   as today; an unmated **connector** socket is refused by the solver (a
   bridge must mate both ends) — except a connector piece whose metadata marks
   it `terminal` (a deliberately broken/partial bridge: a dead-end over the
   void, narrative material — the ruined span the party sees but cannot
   cross). A terminal connector is a dead-end piece like any shrine cap; its
   broken lip is subject to the same edge proof as every sky edge.
3. **Spatial separation falls out**: with alternation enforced, adjacent room
   AABBs are separated by ≥ the connector's length; no new placement math.

### 6.3 Bridge/connector prefab family (sky tileset obligation)

Generated family, parameterized like every tileset: lengths (~8/16/24), deck
widths (2 recommended floor, 3 for set-piece spans), styles (plank + rope
posts, stone arch, chain-hung), plus the `terminal` broken variants. Rails are
partial by design — a fully-railed bridge is not risk terrain; rail gaps are
where the fall proof earns its keep. Deck width 1 is representable but the
harness bot must prove it walkable (nav corner-cutting is already structurally
prevented; mineflayer drift on 1-wide decks is the risk — trial-gated, not
banned).

### 6.4 Proof obligations (feed the spec)

- **Narrow-walkway walkability**: the critical-path A* already routes per-cell
  over the assembled model, so a 1–2 wide deck is provable as-is; the
  rc-tier mineflayer run must include at least one bridge crossing per bridge
  style in use (bot navigation is the real risk, not the model).
- **Every bridge edge cell = lethal fall**: the sky edge proof (spec DW0365)
  applies to connector deck edges identically — any unfenced walkable edge
  cell must have a void-clear fall column (no mid-air softlock ledges under
  bridges: no island may sit in another bridge's fall shadow unless the
  landing is inside the reachable walk region).
- **Die-retry fall trials per bridge**: for each placed
  connector, one environmental-death trial — step off the deck edge, assert
  death by void, assert the checkpoint re-seat lands the player on the armed
  checkpoint. Representative-edge sampling per style is acceptable at PR tier;
  every connector at rc tier.

## 7. Sky backdrop layer

The sky horizon parameterizes what lies **below** the islands (the
backdrop/背景板): (a) void, (b) superflat, (c) ocean, (d) a vanilla-generated
map from a **specified seed**, (e) an imported pre-built third-party map
("sky islands above a giant city"); plus creator-specified placement
coordinates so the scene can sit over a chosen landmark.

### 7.1 Factoring consequence

This breaks the horizon into orthogonal axes — **base** (ocean | flatland |
valley | summit | sky) × **backdrop** (sky-only, the five options) ×
**placement datum/coords** — and the spec's data model should say so:
cherry-valley already made the same argument (valley × flora/palette params,
not a sixth enum). Backdrop stays sky-only in this spec: ocean/flatland ARE
their backdrop, and valley/summit surrounds would need seamless blending into
a backdrop terrain — exactly the macro-terrain line's "seamless heightfield
blending", deferred there.

### 7.2 Backdrop (d) — vanilla seed: the determinism verdict (honest)

Two delivery routes exist; they differ completely in ADR-0006 exposure.

**Route 1 — boot-time generation (recommended v1)**: ship no terrain at all;
the emitted `server.properties` carries `level-type=minecraft:normal` +
`level-seed=<creator seed>` — the exact channel the ocean superflat already
uses, consumed by the same image wrapper (world-settings parity intact). The
**shipped tree stays byte-identical trivially** (one properties line); vanilla
worldgen is block-deterministic for pinned version + seed, so the booted world
is block-identical across boots, which is the same guarantee every current
delve relies on. ADR-0010's "no region files" is untouched. Cost: first-boot
chunk generation + lighting on the Pi (budget row, §4).

**Route 2 — pregenerated region files in the image**: raw pregen output is
**NOT byte-stable across runs**. Known nondeterminism sources in the anvil
format (to be confirmed by spike if this route is ever taken): the region-file
header's per-chunk **timestamp table** (wall clock); **sector allocation /
chunk ordering** (save-order and thread-scheduling dependent); per-chunk
`LastUpdate` (game tick at save); `InhabitedTime` (player-proximity
accumulator — zero only if pregen runs playerless); pending `block_ticks` /
`fluid_ticks` (whatever was scheduled when the save hit); the `entities/`
region files (generation-baked passives are seed-deterministic as *spawns*,
but their **UUIDs come from unseeded `java.util.Random`** — nondeterministic
bytes); `poi/` files; zlib compressor settings. A normalization pass
(canonical chunk order, zeroed timestamps/`LastUpdate`, playerless pregen,
stripped-or-reseeded entities, stripped scheduled ticks, pinned compression)
is feasible with the in-house fastnbt stack but is a real tool with its own
proof burden. **Verdict: Route 1 for v1; Route 2 only if the Pi boot budget
forces it, gated on its own spike + spec.**

**Terrain clearance is a runtime-layer proof, not a compiler proof.** The
compiler cannot know vanilla surface heights without re-implementing vanilla
noise bit-exactly — the same folklore hack §2.5 excludes (no-hack doctrine).
Per ADR-0005 the second validation layer owns it: the validation ladder
already boots the real world, so a **terrain-clearance probe** asserts
`max surface y under (scene footprint + margin) + clearance ≤ scene min y` on
the booted world, red = build rejected. Vanilla peaks reach ~y 250+, so no
static default is claimed safe over a vanilla backdrop; the probe is the gate
and the creator picks `float_y`/coords (advisory: ocean/plains landmarks).

### 7.3 Backdrop (e) — imported maps: the license gate is machine-enforced

CLAUDE.md forbidden zone: CC0 / CC BY / MIT / Apache-2.0 / GPL-compatible
only; **unknown-license maps are excluded, period**. Prior art to reuse: the
spec-0007 two-track asset pipeline and its ingestion gates (allowlist +
provenance + palette audit, the DW07xx family). An imported backdrop enters
ONLY through ingestion: provenance metadata (source, author, license SPDX
from the ADR-0013 allowlist, content hash) recorded in the backdrop's
metadata; CI red on missing/unknown license — never a warning. Technical
gates at the same choke point: pinned-version compatibility (region files
must be 1.21.11-native; datafixer upgrades are not trusted silently), a
block-palette audit, and a size budget. Shipped as static hashed files, an
imported backdrop is byte-identical by construction; chunks beyond the
imported extent fall to a declared generator (void default) — the edge seam
is scenery at distance, recorded as a known aesthetic limit.

### 7.4 Backdrop is scenery: two soundness consequences

- **Mob spawning must be impossible on backdrop surfaces.** Today's delves
  suppress ambient spawning **globally** (world-level, not region-scoped:
  `generate-structures=false` + the spawn-suppression gamerule in the sealing
  baseline, per compiler.md's ocean-horizon emission), and scripted waves are
  `/summon`-ed, which **bypasses the mobcap entirely** — so a city backdrop
  full of spawnable roofs can neither spawn (suppressed) nor starve the waves
  (summons don't compete for caps). The mechanism generalizes as-is; what the
  spec must add is the assertion: a PackTest samples backdrop surfaces after
  N ticks and proves no non-scripted mob exists. One residue on backdrop (d):
  generation-**baked** passive animals (part of chunk gen, not ticking
  spawns) may exist in the scenery; harmless, optionally swept by a setup
  kill below the scene y-envelope.
- **Falls cannot rely on fall/void damage** — an ocean backdrop makes them
  survivable, stranding a live player inside non-interactive scenery. Hence
  the §5 unification: below-envelope = out-of-region = the spec-0013 clock,
  consequence parameterized (`fall: lethal` default). The backdrop is thereby
  **unreachable by invariant**, not by fiction.

### 7.5 Perf (fold into §4 budgets)

| Backdrop | Shipped delta | First-boot delta (Pi) |
|---|---|---|
| void / superflat / ocean | 0 | ~0 (superflat gen is trivial) |
| vanilla seed (Route 1) | 0 | full vanilla chunk gen + lighting for the spawn/view area — **spike required** on arm64 before the budget binds; provisional ceiling +240 s |
| imported | region files ≤ 100 MB (proposal) | placement only; lighting already baked |

## 8. Candidate ACKNOWLEDGEMENTS entries (if ported later)

Copy-ready rows, licenses verified as of this dossier (re-verify at port time):

| Source | License | Technique |
|---|---|---|
| KdotJPG/OpenSimplex2 | Public domain (UNLICENSE-style dedication in repo) | Simplex-class gradient noise (patent 6,867,776 expired 2022) |
| Musgrave ridged multifractal (Ebert et al. 2003) | ideas-only (book copyrighted) | Ridge composition for rims/ranges |
| Perlin improved-noise fade curve (SIGGRAPH 2002) | ideas-only (formula) | Interpolant if gradient noise is adopted |
| Bridson, *Fast Poisson Disk Sampling* (SIGGRAPH 2007 sketch) | ideas-only (paper) | Blue-noise placement, only if the in-house spacing idiom proves insufficient |

Everything actually planned for the first implementation composes the in-house
value-noise family plus already-acknowledged ideas-only techniques — **no new
ACKNOWLEDGEMENTS entry is required unless one of the rows above is ported**.
