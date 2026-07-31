# delve-admit

The prefab **admission** half of the spec-0007 asset pipeline (M3). Turns an
approved, `delve-schem`-converted `.nbt` candidate into a library-grade prefab, and
gates community contributions. Composes with `delve-schem` (conversion + the shared
code-injection NBT scan) and `delve-render` (renders); it does not duplicate them.

Deterministic (ADR-0006): no wall-clock, no unseeded RNG, no hash-order iteration —
`BTreeMap`/sorted maps, gzip mtime pinned to 0 on every structure write.

## CLI

```
delve-admit [--json] <command>

audit    <piece.nbt> [--allowlist <file>] [-o <report.json>]     # CI gate
resolve-jigsaw <piece.nbt>                                       # neutralize foreign worldgen jigsaws
socket   <piece.nbt> --pos x,y,z --facing <dir> [--opening w,h]  # carve a jigsaw socket
anchor   <piece.nbt> --name <id> (--pos x,y,z | --region a:b) [--facing d] [--block b]
lighting <piece.nbt> [--write] [--dark-threshold N]              # static light probe
catalog validate <card.json>...                                  # catalog card schema
gallery  <dir> -o <out> [--id <id>] [--cols N]                   # browse world
curate   <server.log> --layout <gallery-layout.json> [-o <r.json>]
curate-merge <curation.json> --catalog <dir>
```

**Exit codes**: `0` ok · `1` audit/validation failure · `2` input error · `3`
output error · `≥10` internal. Diagnostics (`DW073x..DW076x`) go to **stderr**, one
JSON object per line under `--json`; machine-readable reports go to **stdout** (or a
`-o`/`--report` file). Diagnostic codes are documented in `src/diag.rs`.

## 1. `audit` — the mechanical NBT palette audit (CI gate)

Two checks over a converted `.nbt`, producing a machine-readable `AuditReport`:

- **Hard-forbid** (`DW0731`): the code-injection vectors — command blocks,
  structure blocks, and **NBT-bearing spawners**, plus any block entity carrying an
  embedded `Command`. The recursive `Command`/spawn-NBT scan is the exact one the
  `delve-schem` conversion strip uses (reused, no drift).
- **Palette allowlist** (`DW0730`): every palette block name must be in the
  (configurable) allowlist, so a reviewer sees any surprising block.

**Jigsaw is intentionally not hard-forbidden here.** The conversion strip forbids
jigsaw on *raw community schematics* (contributors don't bring their own sockets);
but the admission audit runs on **library prefabs**, whose jigsaw blocks are the
legitimate sockets the compiler's solver mates — and a jigsaw block entity cannot
carry a `Command`. Verified: every shipped `campaigns/prefabs/*.nbt` passes.

The allowlist is a broad default vanilla **building + decoration** set (see
`src/allowlist.rs`) — stone/wood/glass/copper families, plus inert flora (grasses,
flowers, mushrooms, vines, coral, saplings), non-functional furniture/job-site
blocks, decorative mineral blocks + ores, and archaeology. It deliberately still
flags *surprising* blocks (redstone contraptions, tnt, note blocks) for review, and
is overridable with `--allowlist <file>` (`{ "allow": [...], "allow_suffixes": [...] }`).
The default was broadened for the first real Modrinth ingestion run (FLAGGED for
owner ratification in `src/allowlist.rs`).

## 2. Admission tooling — `resolve-jigsaw` / `socket` / `anchor` / `lighting`

**`resolve-jigsaw`** neutralizes the worldgen jigsaw markers a *community* structure
ships with (Modrinth building content is overwhelmingly worldgen datapacks). It
replaces each `minecraft:jigsaw` block with its block-entity `final_state` — exactly
the block the vanilla generator would bake in — then prunes the orphaned palette
entry. This is the **intended NBT primitive**, not a heuristic, and must run at
import **before** `socket` (our own sockets are jigsaw blocks with `final_state`=air;
resolving after carving would dissolve them).


- **`socket`** carves a `w×h` opening to air, drops a `minecraft:jigsaw` marker with
  the structure-form block entity, and appends the `connectors[]` entry — byte-for-
  byte the shape the generator emits, so the solver mates it like any library piece.
- **`anchor`** adds a named point/gate anchor to the metadata.
- **`lighting`** is a **static block-light BFS** (a faithful model of vanilla block
  light: a 6-neighbour flood decrementing 1 per non-opaque step) measuring the min
  over walkable floor cells. It is honest — the written `method` marks it a *static
  estimate*, never a live probe. Validated against the generator's live-probe values
  on the shipped tileset: exact or within ±2 on all 14 pieces (e.g. keep-gate-room
  reads 9, matching the live probe exactly).

Metadata (`<piece>.json`) is the **full generator shape** (`prefab_id`, `structure`,
`anchors`, `connectors`, `lighting`, `license`), so an admitted external piece is
indistinguishable consumer-side.

## 3. `catalog` — catalog cards

`catalog/<asset-id>.json` (spec-0007 step 2), validated with the same rigor as the
DSL stages: a `deny_unknown_fields` serde model, enum verdicts, a `1..=5` quality
bound, and a **license allowlist** (ADR-0013 — CC0 / CC BY / original / MIT /
Apache-2.0 / GPL-compatible; NC / ND / ShareAlike / unknown reject). A non-original
license must carry a source `url` ("free download" ≠ licensed).

## 4. `gallery` + `curate` — the browse world

`gallery` lays candidate pieces in a labelled grid (`text_display` name + asset-id),
copies the structures into a datapack that places them on first boot, wires in the
`dw.note` capture (spec-0006 reuse) with **per-asset AABBs** so a note resolves
`area=<asset-id>`, and writes a `gallery-layout.json` that is shape-compatible with
the orchestrator's `Layout`. `curate` then reuses the **exact** `delve-harvest`
server-log parser to group notes into a per-asset `CurationReport`;
`curate-merge` folds them into each catalog card's `curation` field.

The `dw.note` stamp/emit functions are byte-pattern-identical to the spec-0006
`creator.rs` channel, which is **live-verified on a pinned 1.21.11 server**
(`validation/playtest-note-flow.sh`). The gallery round-trip (log → `curate` →
per-asset report) is covered deterministically in `tests/gallery.rs` through the
**real** orchestrator `delve-harvest` parser; a dedicated live gallery boot is a
tier-3 follow-up (it needs an itzg datapack server + world env; keep it off the
shared compose to avoid touching the versions.toml manifest gate).

## Tests

```
cargo test -p delvewright-admit    # audit fixtures, socket/light, catalog, gallery, CLI
```

Fixtures (`src/fixtures.rs`) are built in code — no network: a clean piece, a
command-block piece, an NBT-bearing spawner, and a disallowed-palette piece.
