# `delvewright-compiler` (`delvec`)

The deterministic compiler (spec-0002, ADR-0001/0006/0011): staged campaign DSL
in, datapack + server assets out. Rust-native emission; command syntax checked
against the vendored 1.21.11 command tree; mecha re-validates in CI only.

## CLI

```
delvec validate <campaign-dir>            # stages 1–6 schema + referential checks
delvec analyze  <campaign-dir>            # quest-graph reachability (ADR-0005 static)
delvec build    <campaign-dir> -o <out>   # full deterministic build
delvec schema   --stage <1..6|all>        # export JSON Schema (LLM authoring aid)
delvec --version                          # delvec x.y.z, dsl 0.2.0, mc 1.21.11
```

Global: `--json` (one JSON diagnostic per line, the spec-0002 shape), `--prefabs
<dir>` (prefab metadata + `.nbt`, default `prefabs`).

- **Exit codes**: `0` ok · `1` validation failure · `2` analysis failure · `3`
  build failure (prefab/anchor resolution, emission) · `≥10` internal error.
- `build` implies `validate` + `analyze`; `analyze` implies `validate`. A
  validation failure short-circuits with exit 1 before analysis runs.

## Validation vs analysis

`validate` runs the DSL's v0.2/v0.3 rule groups (`DW01xx`, see
`crates/dsl/README.md`) with the **full** injected registries: the complete
1.21.11 item registry (`data/items-1.21.11.json`, 1505 ids), the complete entity
registry (`data/entities-1.21.11.json`, 157 ids — v0.3 wave mobs, `DW0173`) and
the real prefab metadata from `prefabs/` (anchors, pool existence, lighting).
(Note: item ids the DSL's 5-item vendored subset rejects but that are real
1.21.11 items — e.g. `minecraft:diamond_hoe` — validate here.)

`analyze` adds **deep semantic reachability** over the merged quest + objective +
stage-6 dialogue graph (distinct from the DSL's structural `DW0132`
convergent-sink check) plus the **dark-prefab lighting-mitigation** check. Codes
are a stable API; the CI matrix asserts them exactly.

### `DW02xx` analysis codes

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (unreachable / dead quest — its trigger's source never completes). |
| `DW0203` | Objective can never be completed (deadlock): an `after` chain that can never be satisfied, or a `talk-to` whose completing option is unreachable *through the trigger/`after` graph*. (The per-NPC static case — a `talk-to` with no reachable completing option in its own tree — is caught earlier by the DSL's `DW0123`.) |
| `DW0210` | A reachable `dark`-profile prefab has no proven light mitigation (spec-0001 "Lighting contract"). |

Model (fixpoint): a quest is *active* if `campaign-start` or `quest-complete(X)`
with X completable; an objective is *completable* if its quest is active, its
`after` prerequisites are completable, and its type is satisfiable (`talk-to` ⇒ a
reachable completing option in its NPC's stage-6 tree; `reach-anchor` ⇒ always);
a quest *completes* when active with all objectives completable; the finale is
reachable iff its quest completes. `tests/fixtures/unreachable-finale.json`
violates only this — the finale is triggered solely by its own completion, so it
never activates — and exits 2 / `DW0201`, proving analysis is not vacuous.

**Dark-prefab mitigation (`DW0210`).** A `dark` prefab (floor light < 3) is valid
only where analysis proves a mitigation. The **v0.2 sufficient mitigation is a
night-vision item in some class kit** (owner rule); `give-item` is still reserved,
so a quest-granted mitigation is not yet expressible and is out of scope for this
check. Because stage-3 kit items cannot yet carry potion-effect components, a
night-vision source is recognized by its item id or display name containing
`night_vision`/`night vision` — a static *policy gate* ("you declared darkness,
so declare a light source"), not a runtime guarantee. Every declared area is
treated as reachable in v0.2; pool-bound areas are skipped until jigsaw assembly
(M2 task #9). `tests/analyze.rs` is the dark/mitigated fixture pair.

## Command-tree validator depth (honest)

Every emitted `.mcfunction` line is checked against the vendored Brigadier tree
(`data/commands-1.21.11.json`). The validator checks **structure**, not argument
**values** (see `src/commands.rs` header):

- First token must be a known command root.
- `literal` nodes match exactly; `argument` nodes accept their tokens with a
  fixed per-parser arity (`vec3`/`block_pos` = 3, `vec2`/`column_pos`/`rotation`
  = 2, `message` / greedy `string` = rest, else 1 balanced token). Tokenizing is
  brace/bracket/quote-aware so NBT, block-states and selectors are single tokens.
- Matching **backtracks** across ambiguous argument branches (e.g. `teleport @s 5
  65 2` = targets + location, not teleport-to-destination), and follows
  `redirect`s (`… matches N` → `execute`, `run <cmd>` → tree root).
- A line is valid iff all tokens are consumed ending on an `executable` node.

It does **not** verify numeric coords, well-formed NBT/JSON, or that a block/item
id exists — that is mecha's job in the CI cross-check (ADR-0011), plus the DSL
item registry for kit items. This catches misspelled commands, wrong arity, and
bogus subcommand paths.

## Jigsaw layout solver (`src/solver.rs`, ADR-0004 amendment)

**The compiler is the jigsaw.** A `prefab_pool` area is assembled by the compiler,
not by runtime `/place jigsaw`: the solver grows a socket-graph layout from the
campaign seed and emits per-piece `/place template <piece> <pos> <rotation>`. This
keeps the shipped delve plain vanilla (ADR-0003), makes determinism trivial (one
campaign seed → a hand-rolled splitmix64 PRNG in named per-area streams, ADR-0006),
and gives the compiler full layout knowledge for anchors and the critical path.

- **Pool metadata** (`prefabs/pools.json`, compiler-owned): pool id → member
  pieces with `weight` + `role` (`entry` seeds the layout; `connector` fills the
  spine; `room`/`terminal` carry anchors). The prefab `.json` gains a
  `connectors[]` block (keep-socket-v1: `local_pos`, `facing`, `opening`).
- **Geometry**: `/place template <pos> <rotation>` places local `(0,0,0)` at
  `pos` then rotates about it. Two sockets mate when the child socket sits one
  block beyond the parent, facing opposite (`final_state=air` → clean 3×3
  passage); the mating rule fixes the child's rotation and position. All four
  cardinal rotations + AABB overlap rejection are implemented and unit-tested.
- **Growth**: two modes by required-terminal count. **Single terminal**
  (`grow_spine`) — a straight-line spine: entry → straight-preferring `connector`
  fillers → through-rooms inline → one dead-end terminal (`boss-hall` last). Kept
  byte-identical (`keep-crawl`). **Two+ terminals** (`grow_branching`, v0.3) — a
  branching tree: extend the trunk, fork with `tee`/`cross` branch pieces, and cap
  each terminal on its own branch socket. This lifts the old one-terminal limit —
  the harness pathfinds now, so branches/turns walk.
- **3D / vertical growth (M2)**: the mating + AABB machinery is fully 3D. A
  **stair** connector is a keep-socket-v1 piece whose two sockets sit at different
  local `y` (a +4 rise); mating it lifts the layout one elevation level. When the
  bound pool contains a stair and there is filler budget, growth **forces at least
  one** so the layout spans ≥2 levels (`keep-vertical`, `pool/vertical-keep`). A
  pool with no stair is byte-identical to before (`keep-crawl`, `keep-trial`).
- **Large-terminal robustness (M2)**: a single greedy branching pass often cannot
  fit a large terminal (`keep-boss-hall`, 11×13) once small branches crowd the
  space. `grow_branching` wraps the greedy pass in a **bounded, deterministic
  retry** (≤32 attempts): attempt 0 reproduces the pre-M2 growth byte-for-byte;
  each later attempt caps the largest-footprint terminal first and — the shared
  PRNG having advanced — draws fresh choices. Same seed → same attempt sequence →
  same layout. This lifted the hollow-vigil campaign shape from 3/40 to 40/40
  solvable seeds at `pieces {min 10, max 15}`.
- **Role-aware carrier selection (M2)**: each required anchor maps to a carrier
  piece, **never the `entry` piece** (an NPC on `anchor/exit`, which the entry
  spawn-hall already provides, no longer forces a duplicate spawn-hall), with
  **coverage-reuse** — an anchor already covered by an already-selected piece adds
  no second piece (so `anchor/objective` resolves to the boss-hall that
  `anchor/boss` forces, not a redundant shrine).
- **Guarantees**: connected; exactly one entry; every campaign-referenced anchor
  in the area (NPC stands, `reach-anchor` targets, `open-gate` anchors) provided
  by **exactly one** placed piece (else `DW0305`); piece count within the DSL
  `pieces {min,max}`.
- **Sealing**: every mated socket's jigsaw block is cleared to `air`; every
  unmated socket is filled with `stone_bricks` (wall). Emitted as `/fill` in the
  bootstrap after placement.
- **Multi-area transport**: areas sit `AREA_SPACING` apart across void and the
  bot has no pathfinder, so when the critical path crosses areas the compiler
  teleports the player to the next area's entry spawn as the earlier objective
  completes (`plan.transport`, emitted in that objective's completion function).

### `DW03xx` build/solver codes

Build failures (exit 3) carry a stable `DW03xx` code, printed like validation
diagnostics (and as one JSON object per line under `--json`, `stage: "build"`).

| Code | Meaning |
|------|---------|
| `DW0300` | Generic build/resolution failure (missing prefab metadata or `.nbt`, unresolved anchor, critical-path dependency cycle). |
| `DW0301` | The bound pool declares no `entry`-role piece (or no `connector` filler when fillers are needed). |
| `DW0302` | A campaign-referenced anchor is provided by **no** member of the pool (unsatisfiable required anchor). |
| `DW0303` | The `pieces {min,max}` range is too small to fit the entry plus the required anchor-bearing pieces. |
| `DW0304` | The solver could not place a required piece without overlap (after the bounded, deterministic retry), or a branching layout's pool declares no branch piece (tee/cross) to fork its terminals. (The old "more than one terminal" limit is lifted — `grow_branching`.) |
| `DW0305` | A campaign-referenced anchor is defined by **more than one placed piece** (ambiguous resolution) — reference a piece-unique anchor. Also the role-aware failure when a required anchor's only carrier is the `entry` piece and the entry does not already provide it. |
| `DW0306` | Gate-aware reachability deadlock (M2 fix 7): after the solver produces a layout, sealed gates are modelled as cut edges in the piece-connectivity graph (a gate splits its piece into the two halves its barred row divides). An objective whose anchor is only reachable through a gate that **no earlier objective in the quest/objective DAG order has opened** is a deadlock — the delve is unwinnable even though every anchor resolves (canonical case: a key chest sealed behind the very gate the key opens). `tests/fixtures/keep-trial-gate-deadlock.json` (gate opened by the late `interact` door, with the wave/key sealed behind it) proves the check fires; the shipped `keep-trial` (gate opened when the keeper is greeted) proves a clean layout passes. |
| `DW0307` | (v0.4 addendum) A `move-npc` destination is unreachable: A* over the solved voxel grid (`crate::nav`) finds no collision-free walked path from the NPC's home cell to a standable cell at the target anchor (e.g. the two lie in different areas across the inter-area void, or the anchor has no adjacent floor). The move never ships a wall-clipping teleport. |
| `DW0308` | (v0.4 addendum) A `cutscene` camera dolly path clips a solid block. Cameras fly (exempt from walkability) but the sampled polyline must pass only through non-solid cells; the diagnostic names the offending segment and block coordinate. |
| `DW0309` | (v0.4 skins) A mannequin NPC declares a `skin.texture_id` but the campaign dir has no `skins/<texture_id>.png` to bake into the resource pack (spec-0009). |

`tests/fixtures/keep-unsatisfiable-anchor.json` (→ `DW0302`) and
`keep-range-too-small.json` (→ `DW0303`) both pass validate + analyze (the DSL
cannot see pool-area anchors) and fail only at build, proving the solver
diagnostics are reachable. `keep-crawl` is the valid multi-area / multi-piece
fixture (gatehouse single prefab + `pool/stone-keep`, critical path crossing
piece **and** area boundaries).

## Build output (`<out>/`)

`manifest.json` (SHA-256 of the six input stage files + every other output,
sorted), `datapack/` (pack.mcmeta min/max_format `[94, 1]`; see note below), `packtest-datapack/`,
`server/` (void/superflat fixed-seed config), `critical-path.json` (amended
bot-interaction contract; DSL v0.4 adds `sneak: true` on a `stealth` step — the
bot walks that leg sneaking, sprint disabled — and `cutscene_seconds: <n>` on the
step whose completion triggers a `Cutscene`, which the harness sleeps through and
then verifies control returned), plus, for a skinned campaign, `resourcepack.zip`
+ `SKINS.md` (the pack SHA-1 is recorded in `manifest.json` as
`resource_pack_sha1`). Determinism (ADR-0006): all maps are `BTreeMap`/sorted;
JSON is `serde_json` pretty (sorted keys) + trailing newline; no wall-clock,
hostname, locale or absolute path enters any byte. The double-build byte-identity
test (`tests/cli.rs`) is the ADR-0006 gate.

## Emission design decisions (within spec-0002's allowed choices)

- **M2 presentation fixes (v0.3-gated).** Defects the M2 dress rehearsal surfaced,
  each gated on the campaign's dsl_version so v0.2 output (hello-world / keep-crawl)
  stays byte-identical:
  - *CustomName*: `CustomName` is a 1.21.11 **text component**, so a plain SNBT
    string (`CustomName:"Hedric of the Watch"`) is emitted — the old
    `'{"text":…}'` JSON-string form rendered literally (name tags + death
    messages). Applies to NPCs (v0.3) and all wave mobs.
  - *Interact & reach markers*: an `interact` anchor gets a visible, glowing,
    non-colliding `minecraft:item_display` (a lantern, `Glowing:1b`,
    `billboard:"center"`) named from the objective `title` (fallback: objective id)
    so a human can find it — it obstructs neither movement nor the interaction
    hitbox. A `reach-anchor` gets the same marker with a distinct `end_rod` (so the
    finale altar can't be triggered "by wandering"), tagged `dw_r_<obj>`.
  - *Objective feedback*: a titled objective announces its `title` + `hint` (chat +
    `block.note_block.pling`) once when it activates (guarded by a per-objective
    `dw.ann_<obj>` flag), confirms on completion (`entity.experience_orb.pickup`),
    and the finale plays a proper fanfare (`title` banner +
    `ui.toast.challenge_complete`).
  - *Reach region*: `reach-anchor` completes on a **block region** — the anchor cell
    with ±1 generosity on every axis (`x=ax-1,dx=2,…`, a 3×3×3 box) — instead of a
    tight `distance=..R` sphere, so a human standing on the altar completes it. See
    spec-0002 emission amendment.
  - *Wave equipment*: mobs whose natural spawns are armed get a default weapon via
    the component-era `equipment` NBT with a zero `drop_chances` (drop chance 0) so
    summoned combat isn't trivial. **Not legacy `HandItems`** — 1.21.11 silently
    ignores `HandItems`/`HandDropChances` on `/summon`, so it must be
    `equipment:{mainhand:{id:…,count:1}},drop_chances:{mainhand:0.0f}` (proven live
    via rcon). Static table: `wither_skeleton` → `stone_sword`; `skeleton`/`stray` →
    `bow`; everything else (zombie, drowned — a wild trident is not a default)
    unarmed. The generated `verb_kill` PackTest asserts the armed mob actually
    holds its weapon (`execute if items entity … weapon.mainhand …`) so a silent
    regression to the ignored form can't hide.
- **v0.3 gameplay verbs** (spec-0002 v0.3 addendum): `spawn-wave` summons a wave's
  mobs (AI enabled) tagged `dw_wave_<id>` and sets a countdown `#<id> dw.wave`; a
  `player_killed_entity` advancement decrements it, and the `kill` objective's
  per-tick check completes at zero. `collect` places a loaded chest at its anchor;
  an `inventory_changed` advancement runs a guarded completion. `interact` summons
  an interaction entity (tag `dw_i_<obj>`); the click advancement and the bot's
  `/trigger dw.i_<obj>` both feed one per-tick handler that applies the
  `requires_item` (`execute if items`) + flag guards. `set-flag` sets `dw.f_<flag>`
  (per-player); `requires_flags` ANDs those scores into every objective guard. Wave
  campaigns emit `difficulty=easy` (peaceful removes summoned mobs); wave-free
  campaigns stay `peaceful`, so hello-world / keep-crawl are byte-identical. Each
  verb gets a generated per-verb PackTest driving the mechanic on a dummy player.
- **NPC interaction**: a `minecraft:villager` (NoAI/Invulnerable/Silent, no
  profession) is the visual body; a co-located `minecraft:interaction` entity
  (tag `dw_npc_<npc>`) is the click target. An advancement
  (`player_interacted_with_entity`, matched by `type=minecraft:interaction` +
  exact `Tags` nbt) fires a reward function that revokes itself and shows the
  keeper dialog. The interaction entity avoids the villager trade screen and gives
  clean per-NPC detection. *(Runtime behaviour lands in the load task; if the
  interaction entity mis-detects, fall back to matching the villager directly.)*
- **Dialog ⇄ trigger bridge** (spec amendment): every dialog button is a
  `minecraft:run_command` firing `/trigger dw.class set N` (class) or
  `/trigger dw.dlg_<npc> set N` (dialogue option). A per-tick handler maps N →
  effect (navigation or `complete-objective`). Trigger objectives are
  `scoreboard players enable @a`-d every tick so the bot's chat command works too.
  The completion objective `dw.campaign` is `setdisplay sidebar` so the bot reads
  it.
- **World / level config**: `server/server.properties` uses
  `level-type=minecraft:flat` + `generator-settings={"biome":"minecraft:the_void",
  "layers":[]}` (a void world) with `level-seed=<stage-1 seed>`,
  `gamemode=adventure`, `difficulty=peaceful`. No region files are shipped; the
  `#minecraft:load` bootstrap places prefabs with `/place template` and summons
  NPCs, so byte-identity covers the whole tree. See `server/README.md` in output.
- **Coordinate scheme**: area *i* (stage-1 order) is placed at origin
  `[i·256, 64, 0]` (M1: one area → `[0,64,0]`); absolute anchor = origin + the
  prefab's local anchor. Players teleport to the first area's `spawn` anchor after
  class selection.
- **Naming**: scoreboard/function-safe names derive from DSL ids' local part
  (`dw.o_<obj>`, `dw.q_<quest>`, `dw.qa_<quest>` active, `dw.dlg_<npc>`, tag
  `dw_npc_<npc>`, dialog `<npc>_<node>`).

## Vendored data & dependency budget

`data/` holds the 1.21.11 item registry and command tree (see `data/PROVENANCE.md`).
Deps: `clap`, `sha2`, `serde`/`serde_json`, `delvewright-dsl`. The `delvec` binary
copies the committed prefab `.nbt` verbatim and never touches NBT, so `fastnbt`
and `flate2` are **dev-only** (the `gen_hello_room` example + prefab test).
`flate2` is beyond the spec budget and flagged: MC structure files are gzip-framed
NBT and neither NBT crate ships the gzip container.

## Live 1.21.11 verification (M1 load shakeout — resolved)

The following were confirmed/fixed against a live pinned 1.21.11 server (see the
`live_load_shakeout_fixes` + `packtest_suite_is_a_real_test` regression tests):

- `datapack/pack.mcmeta` emits `min_format`/`max_format` as `[94, 1]` (from
  `version.json`: `data_major 94, data_minor 1`). A bare `pack_format` is **rejected**
  for formats newer than 81 ("missing mandatory fields min_format and max_format").
- Dialog JSON (`minecraft:multi_action`, `run_command` action with a leading-slash
  `/trigger`) loads clean. The interaction advancement's `entity` condition must be
  the **single sub-predicate object** form, not a loot-condition list.
- `setup` **`forceload add`s** the prefab chunks before `place template` (else
  `place`/`summon`/`fill` silently no-op at `#minecraft:load` time while `#init` is
  still set) and `setworldspawn`s onto the prefab floor.
- `campaign_complete` broadcasts `[Delvewright] complete dw.campaign 1` — the bot's
  observation channel, because mineflayer 4.37.x cannot read 1.21.11 scoreboard
  scores (the sidebar objective is still displayed for the human/future).
- `packtest-datapack/` emits a real PackTest test at `data/<ns>/test/campaign.mcfunction`
  (misode/packtest 2.4.0) — `# @dummy` + `assert score` driving the completion chain.
  PackTest commands are exempt from the vanilla command-tree validator.
