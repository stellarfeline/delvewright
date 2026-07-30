# spec-0001: Campaign DSL schemas (staged)

- **Status**: Skeleton
- **ADRs**: 0001 (DSL→compiler), 0002 (staged), 0004 (prefab refs), 0006 (determinism)

The heart of the project. Five stage schemas (JSON Schema, versioned together as one
`dsl_version`), each generated/validated in order; later stages reference earlier
stages' IDs only.

## Shared conventions (to specify first)

- ID scheme: kebab-case, stage-prefixed (`npc/keeper`, `quest/find-the-key`), unique
  per campaign; all cross-references are by ID and validated at stage boundaries.
- Every stage output is a single JSON document with `dsl_version`, `campaign_id`,
  `stage`, and `content`.
- The campaign **seed** lives in stage 1 and is the only randomness source downstream
  (ADR-0006).
- Text fields carry player-visible strings verbatim (no templating in v0).
- **No runtime LLM** (owner decision 2026-07-29, current-stage policy): every
  player-visible string and every branch is authored during generation; nothing is
  generated at play time. Dialogue is a **pre-written branching-options tree** —
  nodes with NPC text + a closed set of player choices, mapping directly onto the
  1.21.11 dialog system's button UI. Branch effects (scoreboard flags, quest
  triggers) are declared per option and validated like any other cross-stage ref.
- Versioning/migration policy for `dsl_version` bumps: TBD (owner input).

## Stage 1 — World/setting

Theme, tone, story premise, location palette (list of *named areas* mapping to prefab
pools — ADR-0004), campaign seed, target session length.

## Stage 2 — NPCs

Cast list: ID, name, role (quest-giver / vendor / flavor / boss), home area (stage-1
ref), dialogue voice notes, and **dialogue trees** (pre-written branching options per
the shared convention above), keyed for stage-5 reference.

## Stage 3 — Classes & gear

1–4 playable classes: ID, name, flavor, starting equipment (vanilla item IDs +
components/enchantments), class-specific quest hooks. Constraint: gear must be
expressible as vanilla `/give`-able items on the pinned version.

## Stage 4 — Campaign quest plan

Quest-line skeleton as an explicit DAG: quest IDs, one-line goals, dependency edges,
act/milestone grouping, which area (stage-1) and NPCs (stage-2) each involves,
mandatory vs optional flag. This DAG is the input to reachability analysis (ADR-0005).

## Stage 5 — Quest expansion

Per quest: trigger (what starts it), objectives (typed: reach-area / talk-to /
kill / collect / interact), rewards, dialogue references (stage-2), placement
(prefab-pool anchors), completion effects. Objective types are a **closed enum** — the
compiler must know how to emit and how to *bot-walk* every type (ADR-0005).

## Acceptance criteria (to be made precise in Draft)

- [ ] Each stage schema rejects: unknown fields, dangling cross-stage refs, duplicate IDs.
- [ ] A valid 5-stage fixture campaign exists and round-trips (parse → serialize →
      byte-identical).
- [ ] An invalid-fixture suite exists: each schema rule has at least one fixture that
      violates only it, and validation reports that rule.
- [ ] The hello-world delve (M1) is expressible in v0 schemas.
- [ ] Every stage-5 objective type has a defined compiler emission AND bot-navigation
      strategy documented in this spec.

## Open (owner input wanted)

- Optional quests in v0, or mandatory-only until M3?
