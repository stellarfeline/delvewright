# spec-0006: Creator playtest loop

- **Status**: Draft (owner approved direction 2026-07-30; details at PR review)
- **ADRs**: 0003 (overlay is a datapack, tooling-side), 0005 (validation layers),
  0012 (the skill consumes the report)

The last mile of the production line: when the owner playtests a delve and finds
something wrong, the finding must reach the LLM **fast and DSL-addressable** — not
as "somewhere in the map it was too dark", but as "area/keep, prefab/hello-room,
after obj/talk: creator reports insufficient lighting".

## Components

### 1. Creator overlay datapack *(compiler output, M2 core)*

A third compiler output directory, `creator-datapack/`, beside `packtest-datapack/`
— mounted only by the `playtest` compose profile, **never present in the shipped
delve image** (same exclusion guarantee as PackTest, CI-checked).

- **`/trigger dw.note`** — the mark. On fire, the overlay stamps one
  machine-readable line into the server log (`tellraw @a`, macro-expanded):
  `[DelveNote] pos=[x,y,z] area=<area-id> quests=<active/done summary> nearest_npc=<npc-id>`
  The creator then types the actual note as a plain chat message; capture done,
  keep playing.
- **Debug jumps** *(M3)*: compiler-derived `dw.debug` functions — complete a named
  objective, or jump to the post-step-N state of the critical path (set scoreboards
  + teleport). Level-select for QA: testing act 3 must not require replaying acts
  1–2.

### 2. Compose `playtest` profile *(M2 core)*

Same delve image as `play`, plus: the creator overlay mounted into the world's
datapacks, and the creator opped (name via env) so she can `/tp` and inspect.
One command, localhost only, offline by default — mirrors `play` in every other way.

### 3. Harvester → `playtest-report.json` *(M2 core)*

A thin CLI (orchestrator-glue territory, ADR-0012) that parses the server log after
a session: pairs each `[DelveNote]` stamp with the adjacent chat text from the
creator, resolves positions to area/prefab via the build manifest's layout data,
and emits a versioned `playtest-report.json`:

```json
{ "version": "0.1.0", "campaign_id": "…", "notes": [
  { "at": "<log-ts>", "text": "这个房间太暗了",
    "pos": [12, 65, 8], "area": "area/keep", "prefab": "prefab/hello-room",
    "quest_state": { "quest/open-the-door": ["obj/talk"] },
    "nearest_npc": "npc/keeper" } ] }
```

Note text is verbatim creator input (any language); all context fields are
DSL-addressable IDs.

### 4. Skill integration *(with M4's `/revise-delve`)*

`playtest-report.json` is the **contract input** of the future revision skill: the
LLM receives each note pre-bound to the DSL entities it concerns and proposes stage
document / prefab metadata edits, closing the loop:
generate → validate → playtest → report → revise → regenerate.

## Acceptance criteria

- [x] `creator-datapack/` output is byte-deterministic (ADR-0006 gate covers it —
      its bytes ride the same `BuildOutput` map + `manifest.json` hashes as the main
      datapack) and absent from the shipped delve image (CI check in `ci.yml`
      tier 2, same pattern as the PackTest exclusion).
- [x] End-to-end note flow is machine-tested: the note-bot fires `/trigger dw.note`
      + chats a fixture string; the harvester's report contains one note with the
      correct area, prefab, and quest state resolved. Verified live on a pinned
      1.21.11 server (`validation/playtest-note-flow.sh`, tier-3/local).
- [x] `playtest-report.json` has a versioned schema (`version: "0.1.0"`); the
      harvester emits it and the orchestrator unit tests assert its shape.
- [x] `docker compose --profile playtest up` is one command; overlay mounts + op
      work without manual steps beyond `EULA=TRUE` (+ `CREATOR_NAME` to op).
- [ ] Debug jumps *(M3)*: jumping to the final critical-path step and completing
      the delve yields the same campaign-complete state as a natural playthrough
      (PackTest-asserted).

## Implementation notes (M2, built 2026-07-30)

- **Overlay module**: `crates/compiler/src/creator.rs` — a self-contained emission
  module (one call from `emit::build`), so the concurrent dsl/compiler v0.2 rebase
  stays cheap. Its `.mcfunction`s are plain vanilla and flow through the command-tree
  validator like the main datapack.
- **Stamp channel — `say`, not `tellraw @a`.** A `tellraw`/system message to players
  is **not** written to the server stdout log the harvester parses; `say` is (both
  verified live). The line is macro-expanded exactly as `tellraw` would have been.
  Spec text updated intent-first: the requirement is "one machine-readable line in
  the server log", and `say` is the reliable vanilla command for that.
- **`pos`** is an entity-NBT macro read (`data get entity @s Pos[i]` → storage →
  `function … with storage`), rounded to block ints. **`area`** and **`nearest_npc`**
  are resolved **in-game** from compiler-baked AABBs / nearest-`dw_npc` selection, so
  the log line is self-describing. **`quests`** is the live per-objective scoreboard
  state. The harvester enriches each note from the overlay's `layout.json`
  (`area→prefab`, objective→quest `quest_state`).
- **Harvester** (`crates/orchestrator`, bin `delve-harvest`): the orchestrator's
  first real job (ADR-0012). Offline chat's `[Not Secure] ` prefix is stripped before
  pairing (verified live).

## Open

- ~~Note pairing rule~~ **(decided)**: ±60s window, **prefer the closest chat line
  *after* the stamp** (mark, then type), else the closest before; unpaired stamps
  still report with empty text. Documented in `crates/orchestrator/src/lib.rs`.
- Whether `dw.note` gets a hotbar item binding (nice-to-have, M3).
