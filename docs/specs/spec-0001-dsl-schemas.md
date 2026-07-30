# spec-0001: Campaign DSL schemas (staged)

- **Status**: v0.1 Approved & implemented (M1); **v0.2 Draft — owner approval
  pending** (implementation is M2 task #8)
- **ADRs**: 0001 (DSL→compiler), 0002 (staged), 0004 (prefab refs), 0006
  (determinism), 0012 (skill authors these documents)

## v0.1 → v0.2 changelog

Owner decisions of 2026-07-30, now normative:

1. **Dialogue moves out of stage 2 into a new final stage 6** — stage 2 becomes
   casting sheets; each NPC's complete dialogue tree (flavor + task options) is
   generated in one pass conditioned on the casting sheet plus every quest involving
   that NPC. All cross-stage references become strictly backward.
2. **`prefab_pool` is enabled in stage 1** (jigsaw multi-piece assembly, ADR-0004
   proper; layout semantics in spec-0002/task #9).
3. **Lighting profiles** codified in prefab metadata (contract below).
4. `dsl_version = "0.2.0"`; six stages; the hello-world fixture migrates.
5. **Quests stay mandatory-only** (owner confirmed 2026-07-30); optional quests
   remain reserved until M3.
6. **Structured persona** in stage 2 (owner decision 2026-07-30) — see stage 2.

## Shared conventions

- **Envelope**: every stage output is one JSON document:
  `{ "dsl_version": "0.2.0", "campaign_id": "<kebab>", "stage": "<name>", "content": {...} }`
- **IDs**: kebab-case, type-prefixed (`area/keep`, `npc/keeper`, `class/wanderer`,
  `quest/open-the-door`, `obj/talk`, `dlg/greeting`, `anchor/exit`), unique per
  campaign. All cross-stage references are by ID and **strictly backward**: a stage
  may only reference IDs from earlier stages. Unknown fields are rejected
  everywhere (`deny_unknown_fields`).
- **Source of truth**: the Rust types in `crates/dsl` (serde). JSON Schema is
  exported from them (`delvec schema`) for LLM authoring; schema files are build
  artifacts, not hand-maintained.
- **Seed**: `content.seed` (u64) in stage 1 is the only randomness source
  downstream (ADR-0006) — including jigsaw layout.
- **No runtime LLM** (owner decision 2026-07-29): every player-visible string and
  branch is authored at generation time. Dialogue is a pre-written
  branching-options tree mapping onto the 1.21.11 dialog system (emission:
  `run_command` → `/trigger`, spec-0002 amended contract).
- **Versioning**: pre-1.0, `dsl_version` bumps freely; campaigns recompile from
  source stages — no migration tooling. Migration policy is a 1.0 concern.

## Stage 1 — `world`

```json
{ "title": "…", "theme": "…", "premise": "…", "seed": 20260729,
  "target_minutes": 5,
  "areas": [
    { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" },
    { "id": "area/crypt", "name": "The Crypt",
      "prefab_pool": "pool/crypt-rooms", "pieces": { "min": 4, "max": 7 } } ] }
```

- `areas[]`: 1..N. Each area binds **exactly one of** `prefab` (single piece) or
  `prefab_pool` (+ `pieces` min/max) — jigsaw assembly with the campaign seed
  (layout semantics, connectivity guarantees, and the seed-stability experiment
  live in spec-0002 / M2 task #9).
- `target_minutes`: informational (pacing checks later).

## Stage 2 — `npcs` (casting sheets)

```json
{ "npcs": [ { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
  "area": "area/keep", "anchor": "anchor/keeper-stand",
  "base_entity": "minecraft:villager",
  "persona": {
    "archetype": "stoic gatekeeper",
    "speech_style": "Terse, formal, archaic; never uses contractions.",
    "demeanor": "Cold at first; warms only if the player is patient.",
    "motivation": "Keep the door sealed until a worthy traveler arrives.",
    "secret": "Blames himself for the moor swallowing the old road.",
    "backstory": "Has guarded the keep alone since the road vanished.",
    "relationships": [ { "npc": "npc/warden", "attitude": "distrusts her judgment" } ]
  } } ] }
```

- **No dialogue here.** The **structured persona** (owner decision 2026-07-30) is
  the character contract stage 6 must honor — structure lives in the keys, values
  stay free text. Required: `archetype`, `speech_style`, `motivation`; optional:
  `demeanor`, `secret`, `backstory`, `relationships` (same-stage NPC refs,
  validated within stage 2; `attitude` free text).
- `role`: enum `quest-giver | flavor` (v0.2); reserved: `vendor`, `boss`.
- NPCs are stationary (vanilla capability ceiling).

## Stage 3 — `classes`

Unchanged from v0.1: 1..4 classes, `kit[]` of vanilla item IDs (validated against
the 1.21.11 registry) + count + optional display `name`. Reserved for M3:
`lore`, `enchantments`, `attributes`.

## Stage 4 — `quest-plan`

Unchanged from v0.1: quest DAG skeleton (`depends_on` acyclic, all quests converge
on `finale` — DW0130/0131/0132), area/NPC involvement by backward ref,
`mandatory: true` required in v0.2.

## Stage 5 — `quests`

Unchanged mechanics from v0.1: triggers (`campaign-start | quest-complete`),
objectives (closed enum — v0.2 implements `talk-to`, `reach-anchor`; reserved
`kill`, `collect`, `interact`), `after` ordering, effects
(`open-gate`, `campaign-complete`; reserved `give-item`, `set-flag`,
`spawn-wave`). One change: **`talk-to` objectives no longer point at dialogue**
(dialogue doesn't exist yet at this stage) — they reference only the `npc`; the
completing option arrives in stage 6.

## Stage 6 — `dialogue` (new)

```json
{ "dialogues": [ { "npc": "npc/keeper", "root": "dlg/greeting",
  "nodes": [
    { "id": "dlg/greeting", "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
      "options": [
        { "label": "Who are you?", "next": "dlg/lore" },
        { "label": "Open the door, please.",
          "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] } ] },
    { "id": "dlg/lore", "text": "…", "options": [ { "label": "Back.", "next": "dlg/greeting" } ] }
  ] } ] }
```

- Exactly one dialogue tree per stage-2 NPC (flavor-only NPCs have trees with no
  effects). Node/option semantics identical to v0.1 (cycles allowed, all nodes
  reachable from root, `next` omitted = close dialog).
- `complete-objective` effects reference stage-5 objective IDs — **backward**.
- **Boundary checks** (moved/new here): dialogue-graph reachability (DW0120/0121),
  effect targets exist and are `talk-to` objectives on this NPC (DW0122), and
  **every `talk-to` objective in stage 5 has ≥ 1 reachable completing option**
  (the static half of DW0203's guarantee).

## Anchors (prefab metadata contract)

Unchanged from v0.1: prefabs declare named anchors (`spawn` mandatory, NPC stands,
gates, objective markers); the compiler resolves `anchor/…` refs against bound
prefabs and fails on any miss.

## Lighting contract (prefab metadata)

As refined 2026-07-30 — darkness must be a declared decision, never a default:

- Measured once at library admission (deterministic under sealed fixed time);
  metadata block: `"lighting": { "profile": "lit|dim|dark", "measured_min_light": n,
  "measured": "<date>", "rationale": "…(dim/dark)" }`.
- `lit`: floor light ≥ 7 (default requirement). `dim`: 3–6 + rationale. `dark`:
  < 3, valid only if analysis proves a mitigation (night vision in kit or granted
  by a provably-preceding quest) — new DW02xx check, M2.

## Validation rules (each rule gets a violating fixture in CI)

1. Envelope: wrong `stage`, unknown fields, bad `dsl_version` → reject.
2. ID syntax/uniqueness, dangling refs, **any forward reference** → reject.
3. Stage 6: unreachable node, unknown `next`, effect on unknown/foreign objective,
   uncovered `talk-to` objective, missing/duplicate tree per NPC → reject.
4. Stage 4: dependency cycle, unreachable finale, non-mandatory quest → reject.
5. Stage 5: `after` cycle, reserved enum value, unresolved anchor, unknown item →
   reject.
6. Cross-stage: stage-4 plan ↔ stage-5 expansion 1:1; stage-2 NPC ↔ stage-6 tree
   1:1; `prefab`/`prefab_pool` exclusivity; pool/prefab refs resolve against
   `prefabs/` metadata → reject.

## Acceptance criteria (v0.2)

- [ ] Migrated six-document hello-world fixture parses, validates clean, and
      round-trips byte-identically; compiled output passes the full M1 validation
      ladder unchanged in behavior (bot playthrough still green).
- [ ] Every rule above has a violating fixture yielding exactly its code; codes
      table updated in `crates/dsl/README.md`.
- [ ] `delvec schema --stage 6` exported and CI-verified against fixtures.
- [ ] A dark-prefab fixture without mitigation fails analysis; the same fixture
      with night-vision in the kit passes (new DW02xx).
- [ ] A multi-piece `prefab_pool` fixture compiles with seed-stable layout
      (task #9's experiment gates this criterion).

## Open

None — both v0.2 questions resolved by the owner 2026-07-30 (mandatory-only
confirmed; structured persona adopted).
