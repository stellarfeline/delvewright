# spec-0026: Horizon library — five pseudo-open-world bases

- **Status**: Proposed (task #151; owner directive 2026-08-04: sky, flatland,
  valley, cherry-valley, summit; rulings same day: flatland enforcement =
  spec-0013 primitive, sky = archipelago of islands + bridges, sky backdrop
  layer parameterized incl. vanilla-seed and imported maps. Second-round
  rulings 2026-08-04: terrain/biome/tree layering confirmed; flatland is bare
  grass blocks — no decorative vegetation, no boundary cue (the empty surround
  IS the cue); perf is non-gating; summit README view-distance line,
  `fall: lethal` default, `walk_y` datum + migration round, and backdrop v1
  tiering all approved.)
- **ADRs**: 0003 (vanilla-first), 0004 (prefabs+jigsaw), 0006 (determinism),
  0010 (OCI — no region files, bootstrap places everything)
- **Depends on**: spec-0013 (generalizes its horizon/boundary; **supersedes its
  single global placement datum**), spec-0012 + PR #251 (checkpoint re-seat),
  spec-0010 (relight scope unchanged)
- **Research**: `docs/notes/horizon-library-dossier.md` (algorithms + licenses,
  perf math, solver analysis)
- Non-goal boundary with task #73 (M6 macro-terrain): this spec is single-scene
  surrounds; journey-graph landforms, seamless multi-scene blending and carved
  waterways stay in #73.

## Problem

One horizon exists (`ocean`), and its placement rests on a single global datum
(`plan::OCEAN_BASE_Y = 60`) authored for the island tileset. Task #149 (PR #251
"separate finding") showed the bug class: an interior piece whose walk plane
sits lower than the island convention lands under sea level, floods on boot,
and `DW0344`'s `waterline_y` exemption means no proof ever looks — the model is
wrong about walkability, lighting, and everything derived from them. New scenes
need sky/flatland/valley/summit surrounds, and the generalization must make the
#149 class impossible by construction, not add five more folklore datums.

## 1. DSL surface (stage 1, next `dsl_version`)

A horizon is a **composition of orthogonal axes**, not an enum of monoliths
(owner addition 2026-08-04): a **base** (what surrounds the scene), base
params, and — for `sky` only — a **backdrop** layer (what lies below the
islands) plus **placement** coordinates over it. `horizon` accepts the
existing strings (unchanged, byte-identical) or an object
`{base, …params, backdrop?, placement?, fall?}`. Strings are shorthands:
`"ocean"` ≡ `{base:"ocean"}`; `"cherry-valley"` ≡ `{base:"valley",
flora:"cherry", palette:"stone-petal"}`.

| base | Ambient (analytic generator) | Surround (compiler-generated prefab tiles) | Params (all optional, pinned defaults) |
|---|---|---|---|
| `void` | void superflat | none | — (unchanged) |
| `ocean` | pinned water superflat, sea level 62 | none | — (unchanged emission; placement re-datumed per §2) |
| `sky` | the declared **backdrop** (§4) | none; scene rooms become an island archipelago (§4) | `float_y` (walk-plane world y, default 160), `fall` (`lethal` default \| `return`), `backdrop` (default `void`), `placement {x,z}` (default 0,0) |
| `flatland` | grass superflat whose surface tops **exactly one block under the scene walk plane** (zero height difference by datum equation) | seam blend band (§3) | `blend_width` (1..=16, default 6 — 0 would be a hard edge, which the interpenetration ruling forbids) |
| `valley` | void superflat below the tile skirt | mountain annulus: total footprint `ratio`× the scene's, radial ridged-noise rim, flat gap floor between scene and slopes | `ratio` (2..=3, default 2.5), `rim_height` (default 48), `flora` (default `oak`), `palette` (default `stone-grass`) |
| `summit` | low superflat (gorge haze floor) | flat-topped plateau under the scene + surrounding range and gorges, every surround crest **below** the scene walk plane | `plateau_y` (default 208), `vista_radius` (≥192, default 208 — see below), `min_drop` (≥100, default 120) |

**Cherry-valley is a parameter row, not a base**: the compiler holds no
cherry-specific code path; a fixture proves the emissions differ only in
palette/flora blocks (acceptance criterion 6). Backdrop stays sky-only in this spec — blending a
valley/summit surround into a backdrop terrain is task #73's seamless
heightfield blending (Non-goals).

**Terrain, biome, and trees are three separable layers** (owner ruling
2026-08-04): a surround emits (i) its blocks, (ii) a **vanilla-native biome
paint** over its columns via `/fillbiome` in the bootstrap path — the same
channel vanilla uses for grass/foliage/water tint, ambience and sky; no
resource pack involved — and (iii) an optional **tree layer**: seeded
scatter (Bridson/Poisson-disk family, ideas-only; dossier §7) placing tree
*templates* like any other blocks. Vanilla `/place feature` is rejected for
trees — it draws on world RNG, which the determinism invariant cannot admit.
`flora` selects biome + tree species together; with the tree layer empty,
`flora` degrades to biome tint alone — differently-colored grass, vanilla's
own semantics. `cherry` ⇒ `minecraft:cherry_grove` + cherry templates;
flatland paints `minecraft:plains` and carries no tree layer.

All surround generation is seeded from the campaign seed + fixed per-horizon
stream labels (in-house position-addressed value-noise family; dossier §2). No
new dependency, no new ACKNOWLEDGEMENTS entry unless a dossier §7 candidate is
ported.

## 2. The world model: declared physical facts + per-area datum

Each horizon declares to `compiler::plan`/`nav`:

- **ambient**: analytic per-column contents outside every placed AABB (extends
  `nav::Ambient`; DW0322 gains a branch per horizon, same aggregation).
- **walk_ref_y**: the world y where a placed piece's walk plane must land
  (`ocean` 63, `flatland` = grass top + 1, `sky` = `float_y`, `valley` = gap
  floor + 1, `summit` = plateau top + 1).
- **hazard facts**: `flood_level` (ocean 62, else none), `lethal_below`
  (sky: −64), `fall_is_lethal_offscene` (sky, summit gorge side).

**Per-area datum (supersedes `OCEAN_BASE_Y`)**: each area's base y =
`walk_ref_y − walk_y`, where `walk_y` is the area's tileset walk-plane
convention, declared in pool/prefab metadata (island 3, keep 1, …). A piece
placed in any non-void horizon without `walk_y` metadata is a build error
(DW0367). This alone retires the #149 class: a keep-interior area (walk plane
local 1) gets base `63 − 1 = 62`, landing its walk plane at 63 — one block
above sea level, dry, with no author action; the island area (`walk_y = 3`)
keeps base 60, byte-identical to today. `walk_y` backfill across existing
tilesets is an in-milestone migration round (version-adoption discipline;
owner-approved 2026-08-04).

**Declarations position; proofs read reality.** After assembly the compiler
checks **empirical geometry**, never metadata:

- Every placed piece's standable cells vs `flood_level`: any walk cell at or
  below flood level is DW0364 (build, exit 3) — **no exemption for pieces that
  declare no `waterline_y`**, closing the DW0344 gap. DW0344 itself is retained
  for waterline-declaring pieces (shore mating at sea level).
- `DW0344`/`DW0364` messages name area, prefab, placed y, offset, and the
  datum equation term that disagrees.

## 3. Flatland seam (the no-pedestal rule)

- Height: zero difference by construction (datum equation §2) — never blended.
- Material: a `blend_width` band straddling the scene edge dithers grass and
  scene floor palettes by seeded noise threshold (dossier §2.4). Explicitly
  forbidden outcome: a clean vertical material wall (machine assertion,
  acceptance criterion 4).
- **Bare by design** (owner ruling 2026-08-04): flatland is grass *blocks*
  only — no decorative vegetation, tufts, or scatter of any kind.
- Boundary: spec-0013 return clock, **horizon-agnostic** (owner ruling
  2026-08-04) — the region derivation and clock never branch on horizon kind.
  **No visual boundary cue** (owner ruling, same day): the surround is
  deliberately empty of content and buildings, and that emptiness is itself
  the signal; nothing telegraphs the clock.

## 4. Sky archipelago (owner ruling 2026-08-04)

- Multi-room sky areas assemble as **independent floating islands connected by
  narrow bridges** — never one monolithic island. Bridges are risk terrain:
  falling = death (via the boundary catch below) → PR #251 checkpoint re-seat.
- **Solver**: prefab metadata gains a connection class `role: room | connector`.
  In a sky area the frontier attach enforces alternation (room sockets accept
  only connectors; connector–connector allowed). First-class solver rule, not
  socket-name convention. Non-sky pools declare no connectors and are
  byte-identical. An unmated connector socket is refused (a bridge mates both
  ends) unless the piece is marked `terminal` — the deliberately broken span,
  a legal dead-end whose lip meets the same edge proof as every sky edge.
- **Connector family** (sky tileset obligation): lengths ~8/16/24, deck width
  ≥2 (1 allowed only with a passing bot trial), styles parameterized, partial
  rails by design, `terminal` broken variants.
- **Falls ride the boundary primitive** (unifies vertical with horizontal
  OOB — the same spec-0013 clock the owner chose for flatland; dossier §5).
  Crossing below the region's y-envelope floor (lowest placed block − 8) is
  out-of-region; the consequence is the `fall` param: `lethal` (default —
  the catch applies `damage @s 1000 minecraft:generic`, vanilla death fires,
  the PR #251 re-seat lands the respawn on the armed checkpoint; full souls
  death costs, identical over every backdrop) or `return` (plain teleport
  back). Fall/void damage is never relied on: an ocean backdrop makes
  landings survivable, and the clock catches the faller well above any
  backdrop surface. The backdrop is unreachable **by invariant**.
- **Backdrop** (owner addition 2026-08-04; dossier §7): what lies below the
  islands, creator-selectable: `void` (default) | `superflat` | `ocean` |
  `vanilla {seed}` | `imported {ref}`; `placement {x,z}` shifts the whole
  scene grid over it (position the delve above a chosen landmark).
  - `vanilla`: delivered by **boot-time generation** — emitted
    `server.properties` carries `level-type` + the creator's `level-seed`
    (the exact channel superflat uses; shipped tree stays byte-identical,
    ADR-0010 region-file ban untouched; block-deterministic per pinned
    version + seed). Pregenerated-region shipping is rejected for v1: raw
    pregen is not byte-stable (timestamp tables, sector order, `LastUpdate`,
    entity UUIDs from unseeded RNG — dossier §7.2); a normalization tool is
    its own future spec if the Pi boot budget forces it.
  - **Terrain clearance is a runtime-layer proof** (ADR-0005): the compiler
    cannot model vanilla noise (no-hack — dossier §2.5); the validation
    ladder's booted world carries a clearance probe asserting
    `max surface y under scene footprint + margin < scene min y`; red =
    rejected. No static `float_y` is claimed safe over a vanilla backdrop.
  - `imported`: enters ONLY via the spec-0007 ingestion path with a
    **machine-enforced license gate** (ADR-0013 allowlist: CC0 / CC BY / MIT
    / Apache-2.0 / GPL-compatible; missing or unknown license = red, never a
    warning), provenance metadata (source, author, SPDX, content hash),
    pinned-version region compatibility, palette audit, size budget. Static
    hashed files = byte-identical by construction; beyond the imported
    extent a declared generator (void default) takes over.
  - **No mob ever spawns on backdrop surfaces**: the existing world-global
    suppression (server.properties + the sealing gamerule baseline) covers
    them, and scripted waves are `/summon`-ed — mobcap-exempt, so a city of
    spawnable roofs can neither spawn nor starve the waves. Asserted, not
    assumed: a generated PackTest samples backdrop surfaces after N ticks
    and proves no non-scripted mob exists.
- **New proof — no mid-air softlock ledge (DW0365, build, exit 3)**: every
  reachable walkable cell adjacent to an off-piece drop is fenced (1.5-tall
  barrier) OR its fall column is clear down to the region floor (below which
  the clock owns the faller); a landing surface above the floor and outside
  the reachable walk region is the violation. Applies identically to island
  rims and bridge deck edges (no island may sit in a bridge's fall shadow).
  Aggregated like DW0322.
- **Trials** (task #146 family, under `fall: lethal`): per connector style at
  PR tier, per placed connector at rc tier — step off the deck, assert death,
  assert re-seat onto the armed checkpoint.

## 5. Valley & summit soundness

- Surround tiles are placed pieces in the assembled voxel model: nav, DW0322,
  gravity settle and snapshots see them. They are **excluded** from the
  boundary-region derivation (region derives from content areas only) and
  carry no interior lighting obligations (DW0210/0211 scope unchanged).
- Valley: gap floor between scene and slopes is walkable ambient terrain
  inside the region margin; inner slopes are proven un-climbable by nav (no
  standable staircase from gap floor to crest), not by slope-angle promise.
- Summit ("一览众山小"): max surround surface y < scene walk plane y is a hard
  clamp verified empirically; ≥1 gorge drop ≥ `min_drop` along the vista ring;
  everything within −64..320.
- DW0366 (validation, exit 1): horizon params out of range (`ratio`, `min_drop`,
  `vista_radius` < 192, `blend_width` outside 1..=16, `plateau_y` overflowing
  build range after scene height, …).
- `vista_radius` definition (amendment 2026-08-04, resolving the W-A-reported
  default/floor contradiction): measured **outward from the scene bounding-box
  edge** — the guaranteed depth of generated terrain beyond any standpoint the
  player can reach. Floor 192 = the shipped summit `view-distance` (12, §6) ×16,
  so the fog line always lands inside generated terrain; default 208 (13 chunks,
  one chunk of slack over the floor; perf is non-gating per §6). `blend_width`
  1..=16: 0 would be a hard edge, which the flatland interpenetration ruling
  forbids; 16 bounds the dither band.
- DW0320 generalizes: any horizon whose ambient is enterable (ocean, flatland,
  valley, summit) requires `boundary`; `sky` requires it too (lateral clock).

- DW0368 (validation/ingestion, exit 1): backdrop declaration errors —
  `backdrop` on a non-sky base, `vanilla` without a seed, `imported` naming an
  asset that fails the ingestion gate (license, provenance, version, palette,
  size). The license half is never downgradeable to a warning.

Provisional codes DW0364–DW0368; numbers may shift at implementation — the DW
gate and `docs/reference/compiler.md` stay authoritative (spec-0013 precedent).

## 6. Parity, rendering, budgets

- **World parity (task #84)**: worldgen flows only through emitted
  `server.properties` (`level-type` + `generator-settings`), which the
  toolserver/delve images already consume — the PackTest world is the shipped
  world by construction. `check-world-settings.sh` extends to every horizon.
- **Render tier**: `render-plan.json` gains ≥1 establishing vista shot per
  horizon build (scene edge looking outward), joining the existing shot kinds.
- **Perf is non-gating** (owner ruling 2026-08-04: the Pi 5 comfortably ran
  100+-mod servers; no budget table, no spike gates). CI records shipped
  image size delta and first-boot time per horizon fixture *informationally*
  (dossier §4 + §7.5 keep the estimation math for reference). Summit ships
  server `view-distance` 12 and its campaign README states the client
  view-distance floor (owner-approved player-facing line, 2026-08-04).

## Acceptance criteria (machine-checkable)

1. Double-build byte-identity for one fixture per horizon kind (6 fixtures);
   `void`/`ocean`/absent campaigns byte-identical to the previous
   `dsl_version` output.
2. Negative fixtures assert DW0364, DW0365, DW0366, DW0367 by code; a
   #149-shaped fixture (interior walk plane below sea level, no `waterline_y`)
   is **red** with DW0364.
3. Ocean island fixture placement is unchanged by the per-area datum (island
   `walk_y=3` → base 60, byte-identical).
4. Flatland fixture: across the seam band, (a) every column's surface y equals
   the scene walk plane − 1 (zero height delta), (b) both palettes occur
   interleaved on both sides of the scene edge line (no column-aligned
   material wall).
5. Sky fixture: a room pool with connectors solves to ≥2 islands whose AABBs
   are disjoint with a connector mating each gap; a sky fixture with an
   unfenced ledge over a lower non-reachable piece is red (DW0365); PackTest
   parity — the packtest world boots with the sky generator-settings
   (`check-world-settings.sh` green).
6. Cherry-valley fixture: emission diff vs the same-seed valley fixture
   touches only palette/flora block ids (script-asserted) — parameterization,
   not a fork.
7. Summit fixture: empirical max surround y < scene walk plane y; ≥1 drop ≥
   `min_drop`; all blocks within −64..320.
8. rc-tier bot run on the sky fixture crosses every connector style and
   completes one die-retry fall trial under `fall: lethal` (death by the
   boundary catch → re-seat on the armed checkpoint), over a **non-void**
   backdrop fixture — proving the death does not depend on void damage.
9. Backdrop: (a) negative fixtures assert DW0368 by code, including an
   `imported` ref with unknown license (red, exit 1); (b) a `vanilla{seed}`
   fixture's emitted `server.properties` carries the declared seed and
   `check-world-settings.sh` is green; (c) the validation ladder runs the
   terrain-clearance probe on the booted `vanilla` backdrop and a
   deliberately-low `float_y` fixture is red; (d) the generated backdrop
   PackTest proves no non-scripted mob on sampled backdrop surfaces.
10. CI records shipped image size delta and first-boot time per horizon and
    backdrop fixture (informational — no thresholds; owner ruling
    2026-08-04).
11. `docs/reference/compiler.md` DW rows + `tools/check-dw-codes.py` green in
    the same PR as each landing.

## Non-goals

Vanilla density-function terrain as *modelled* surround (dossier §2.5 —
unmodellable by proofs; recorded so it is not relitigated; the sky `vanilla`
backdrop is the legitimate use: pure scenery behind a runtime-layer probe);
backdrops for non-sky bases (task #73 seamless blending); pregenerated-region
shipping + normalization tooling (own future spec, only if the Pi budget
forces it); carved waterways and journey-graph landforms (#73); hydraulic
erosion; ocean content; weather/lighting coupling beyond existing spec-0010
declarations; cross-area bridges (inter-area travel stays transport-based).
