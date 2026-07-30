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

- [ ] `creator-datapack/` output is byte-deterministic (ADR-0006 gate covers it)
      and absent from the shipped delve image (CI check, same as the PackTest
      exclusion).
- [ ] End-to-end note flow is machine-tested: the harness bot fires
      `/trigger dw.note` + chats a fixture string; the harvester's report contains
      one note with the correct area, prefab, and quest state resolved.
- [ ] `playtest-report.json` has a versioned schema; the harvester's output
      validates against it.
- [ ] `docker compose --profile playtest up` is one command; overlay + op work
      without manual steps beyond `EULA=TRUE`.
- [ ] Debug jumps *(M3)*: jumping to the final critical-path step and completing
      the delve yields the same campaign-complete state as a natural playthrough
      (PackTest-asserted).

## Open

- Note pairing rule (chat line before vs after the stamp; time window) — the
  implementer picks the most forgiving heuristic and documents it.
- Whether `dw.note` gets a hotbar item binding (nice-to-have, M3).
