# spec-0001: Campaign DSL schemas (staged)

- **Status**: Approved (v0, owner-approved 2026-07-29)
- **ADRs**: 0001 (DSL→compiler), 0002 (staged), 0004 (prefab refs), 0006 (determinism)
- **v0 scope**: everything the hello-world delve (M1) needs, with envelopes and enums
  designed so M2+ extends values rather than reshaping structures. Reserved enum
  values are listed but rejected by v0 validation ("reserved, not yet implemented").

## Shared conventions

- **Envelope**: every stage output is one JSON document:
  `{ "dsl_version": "0.1.0", "campaign_id": "<kebab>", "stage": "<name>", "content": {...} }`
- **IDs**: kebab-case, type-prefixed (`area/keep`, `npc/keeper`, `class/wanderer`,
  `quest/open-the-door`, `dlg/keeper-greeting`, `obj/talk`, `anchor/exit`), unique per
  campaign. All cross-stage references are by ID; a stage may only reference IDs from
  earlier stages (dialogue/objective IDs are stage-local). Unknown fields are rejected
  everywhere (`deny_unknown_fields`).
- **Source of truth**: the Rust types in `crates/dsl` (serde). JSON Schema is
  *exported* from them (`delvec schema`, spec-0002) for LLM authoring; the schema
  files are build artifacts, not hand-maintained.
- **Seed**: `content.seed` (u64) in stage 1 is the only randomness source downstream
  (ADR-0006).
- **No runtime LLM** (owner decision 2026-07-29): every player-visible string and
  branch is authored at generation time. Dialogue is a pre-written branching-options
  tree mapping onto the 1.21.11 dialog system.
- **Versioning**: pre-1.0, `dsl_version` bumps freely and campaigns are recompiled
  from source stages — no migration tooling. Migration policy is a 1.0 concern.

## Stage 1 — `world`

```json
{ "title": "…", "theme": "…", "premise": "…", "seed": 20260729,
  "target_minutes": 5,
  "areas": [ { "id": "area/keep", "name": "The Keep",
               "prefab": "prefab/hello-room" } ] }
```

- `areas[]`: 1..N. v0: each area binds **one prefab by ID** (`prefab/<name>`,
  resolved against `prefabs/` metadata at compile time). M2 replaces `prefab` with
  `prefab_pool` + jigsaw parameters; both fields defined now, exactly one allowed,
  pools rejected as reserved in v0.
- `target_minutes`: informational in v0 (drives pacing checks later).

## Stage 2 — `npcs`

```json
{ "npcs": [ { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
  "area": "area/keep", "anchor": "anchor/keeper-stand",
  "base_entity": "minecraft:villager",
  "dialogue": { "root": "dlg/greeting", "nodes": [
    { "id": "dlg/greeting", "text": "Halt, traveler…",
      "options": [
        { "label": "Who are you?", "next": "dlg/lore" },
        { "label": "Open the door.", "effects":
            [ { "type": "complete-objective", "objective": "obj/talk" } ] } ] },
    { "id": "dlg/lore", "text": "…", "options": [ { "label": "Back", "next": "dlg/greeting" } ] }
  ] } } ] }
```

- `role`: enum `quest-giver | flavor` (v0); reserved: `vendor`, `boss`.
- `anchor`: a named marker the area's prefab must provide (see Anchors below).
- Dialogue: a graph of nodes; each option has optional `next` (node ref; omitted =
  close dialog) and optional `effects`. v0 effect enum: `complete-objective` only
  (forward ref into stage 5, validated at the stage-5 boundary). Cycles between nodes
  are allowed (menus); every node must be reachable from `root`.
  - **Amended 2026-07-30 (v0.1 bot-interaction contract):** the schema here is
    unchanged; note only the emission mechanism — the compiler renders each option as
    a dialog button with a `run_command` click action firing a compiler-assigned
    `/trigger` command, so one command surface serves both the human dialog GUI and
    the validation bot's chat path (see spec-0002 critical-path.json).
- NPCs are stationary (vanilla AI off, no scripted movement — see
  `docs/notes/vanilla-capability-ceiling.md`).

## Stage 3 — `classes`

```json
{ "classes": [ { "id": "class/wanderer", "name": "Wanderer",
  "blurb": "Sturdy boots, no questions.",
  "kit": [ { "item": "minecraft:iron_sword", "count": 1, "name": "Keeper's Gift" } ] } ] }
```

- 1..4 classes. `kit[]`: vanilla item ID (validated against the pinned 1.21.11 item
  registry) + count + optional display `name`. Reserved for M2/M3: `lore`,
  `enchantments`, `attributes`. Class selection UI is compiler-emitted from this
  stage (dialog at spawn); selecting grants the kit and marks the player classed.

## Stage 4 — `quest-plan`

```json
{ "quests": [ { "id": "quest/open-the-door",
  "goal": "Get the Keeper to open the door and leave the keep.",
  "area": "area/keep", "npcs": ["npc/keeper"],
  "depends_on": [], "mandatory": true, "act": 1 } ],
  "finale": "quest/open-the-door" }
```

- `depends_on` edges must form a DAG; `finale` must be reachable from the dependency
  roots. This stage is the input to reachability analysis (ADR-0005 static layer).
- **v0 scope choice (owner to confirm with this spec): `mandatory: true` required** —
  optional quests are reserved until M3.

## Stage 5 — `quests`

```json
{ "quests": [ { "id": "quest/open-the-door",
  "trigger": { "type": "campaign-start" },
  "objectives": [
    { "id": "obj/talk", "type": "talk-to", "npc": "npc/keeper" },
    { "id": "obj/exit", "type": "reach-anchor", "anchor": "anchor/exit",
      "radius": 2, "after": ["obj/talk"] } ],
  "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
  "on_complete": [ { "type": "campaign-complete" } ] } ] }
```

- `trigger`: enum `campaign-start | quest-complete(ref)` (v0 both implemented).
- **Objective types — closed enum.** v0 implements `talk-to` and `reach-anchor`;
  reserved: `kill`, `collect`, `interact`. Every implemented type defines BOTH:
  - *Emission*: `talk-to` → completion fires from a dialogue option's
    `complete-objective` effect; `reach-anchor` → proximity check (anchor position ±
    `radius`) once prerequisites (`after`) are met.
  - *Bot strategy* (consumed via critical-path.json, spec-0002): `talk-to` → walk to
    the NPC's anchor, open dialog, click the option path the compiler recorded;
    `reach-anchor` → pathfind to the anchor's absolute coordinates.
- `after`: intra-quest objective ordering (DAG, validated).
- Effect enum (v0): `open-gate` (anchor of a prefab-declared gate: swaps its blocks
  open, one-way), `campaign-complete` (final advancement + credits + scoreboard flag
  the harness asserts). Reserved: `give-item`, `set-flag`, `spawn-wave`.

## Anchors (prefab metadata contract)

Prefab metadata (in `prefabs/`, format owned by this spec) declares named anchors:
`spawn`, NPC stands, gates, objective markers — each a position (+ facing) inside the
structure. The compiler resolves `anchor/…` refs against the prefabs actually bound in
stage 1 and fails validation on any miss. Every area's prefab must declare `spawn`.

## Validation rules (each rule gets a violating fixture in CI)

1. Envelope: wrong `stage`, unknown fields, bad `dsl_version` → reject.
2. ID syntax, ID uniqueness, dangling refs (per stage boundary) → reject.
3. Dialogue: unreachable node, `next` to unknown node, effect referencing unknown
   objective (checked at stage-5 boundary) → reject.
4. Stage 4: dependency cycle, unreachable finale, non-mandatory quest (v0) → reject.
5. Stage 5: objective `after` cycle, reserved enum value, anchor unresolved against
   bound prefab, item ID not in 1.21.11 registry → reject.
6. Cross-stage: every stage-4 quest expanded exactly once in stage 5; stage-5 quest
   not planned in stage 4 → reject.

## Acceptance criteria

- [ ] `crates/dsl` parses the five hello-world fixture documents; round-trip
      (parse → serialize) is byte-identical.
- [ ] Each validation rule above has ≥1 fixture violating only it; `delvec validate`
      rejects it citing that rule's diagnostic code (spec-0002).
- [ ] `delvec schema --stage <n>` emits JSON Schema that accepts every valid fixture
      and rejects every invalid one (checked in CI with a generic JSON Schema
      validator — proves LLM-facing schemas match the Rust truth).
- [ ] The hello-world campaign (this spec's examples, completed) is committed as the
      canonical fixture and drives M1 end-to-end.
