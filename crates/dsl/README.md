# `delvewright-dsl`

serde types, validation, canonical serialization and JSON Schema export for the
staged campaign DSL (spec-0001). This crate is the **source of truth** for the
DSL: JSON Schemas are exported from the Rust types, never hand-maintained.

## What it provides

- **Types** (`stages`, `envelope`, `ids`): serde types for the envelope
  `{ dsl_version, campaign_id, stage, content }` and the **six** stage payloads
  (v0.2: stage 2 is a structured `persona`, stage 6 is `dialogue`), every struct
  `#[serde(deny_unknown_fields)]`. IDs are type-prefixed kebab-case newtypes
  (`area/…`, `npc/…`, `class/…`, `quest/…`, `dlg/…`, `obj/…`, `anchor/…`,
  `prefab/…`, `pool/…`) that parse permissively and expose `is_valid_syntax()`.
- **Validation** (`validate::validate_campaign`): all spec-0001 v0.2 rule groups
  plus the v0.3 verb/wave/flag group (`DW0170`–`DW0173`, gated on `dsl_version`),
  returning `Diagnostic { code, severity, stage, path, message }` in the
  spec-0002 `--json` shape.
- **Canonical serialization** (`canonical::to_canonical_string`): the single
  canonical writer — 2-space pretty, struct-declaration field order, sorted map
  keys (`BTreeMap`), trailing newline. Round-tripping any valid fixture is
  byte-identical (enforced by `tests/roundtrip.rs`).
- **JSON Schema export** (`schema::stage_schema`): one full-envelope JSON Schema
  (draft 2020-12) per stage, via `schemars`.
- **Registries** (`registry`): `ItemRegistry`, `EntityRegistry` (v0.3 wave mobs)
  and `AnchorRegistry` traits, with small vendored v0 implementations (see
  [Registries](#registries)).

Determinism (ADR-0006): all iteration is over `BTreeMap`/`BTreeSet` or slices;
nothing depends on hash order, wall-clock, or absolute paths.

## Entry points

```rust
use delvewright_dsl::{RawCampaign, check_campaign, parse_campaign, validate_campaign};

// From six raw JSON strings (compiler input):
let diags = check_campaign(&raw); // parse (DW0100 on failure) then validate

// Or in two steps:
let campaign = parse_campaign(&raw)?;          // Result<Campaign, Vec<Diagnostic>>
let diags = validate_campaign(&campaign);      // Vec<Diagnostic>
```

`validate_campaign` uses the vendored v0 registries. The compiler injects full
registries via `validate_campaign_with(&campaign, &items, &anchors)`.

## Diagnostic codes (`DW01xx`)

Codes are a **stable API**; the CI fixture matrix asserts exact codes. Each code
has ≥1 invalid fixture under `fixtures/invalid/` that violates only that rule.

| Code | Rule group | Meaning |
|------|-----------|---------|
| `DW0100` | 1 Envelope | Document does not conform to its stage schema (unknown field / wrong type / malformed value, incl. a **missing required persona field**). Reported at parse time. |
| `DW0101` | 1 Envelope | `stage` field does not match the document's slot (e.g. `world.json` says `stage: "npcs"`). |
| `DW0102` | 1 Envelope | Unsupported `dsl_version` (only `0.2.0` in v0.2). |
| `DW0103` | 1 Envelope | `campaign_id` differs across stages. |
| `DW0110` | 2 IDs | Malformed id syntax (not kebab-case, or wrong/missing type prefix). |
| `DW0111` | 2 IDs | Duplicate id within its namespace (incl. **two dialogue trees for one NPC**). |
| `DW0112` | 2 IDs | Dangling reference (incl. a **persona relationship** to an unknown NPC, or any **forward/undeclared** reference — references must be strictly backward). |
| `DW0120` | 3 Dialogue (stage 6) | Dialogue node unreachable from `root`. |
| `DW0121` | 3 Dialogue (stage 6) | Dialogue `root`/option `next` references an unknown node. |
| `DW0122` | 3 Dialogue (stage 6) | Dialogue effect targets an objective that is unknown, not a `talk-to`, or a `talk-to` **on a different NPC** (foreign effect). |
| `DW0123` | 3 Dialogue (stage 6) | A stage-5 `talk-to` objective has **no reachable completing option** in its NPC's tree (static half of the compiler's `DW0203`). |
| `DW0130` | 4 Quest plan | Quest `depends_on` graph contains a cycle. |
| `DW0131` | 4 Quest plan | `finale` is not a declared quest. |
| `DW0132` | 4 Quest plan | `finale` is not the convergent sink of the plan (see [note](#dw0132)). |
| `DW0133` | 4 Quest plan | Non-mandatory quest (`mandatory: false`), reserved until M3. |
| `DW0140` | 5 Quests | Objective `after` ordering contains a cycle. |
| `DW0141` | 2–5 Reserved | Reserved enum value used (see [Reserved](#reserved-values)). |
| `DW0142` | 5 Quests | Anchor not provided by the area's bound prefab. |
| `DW0143` | 5 Quests | Item id not in the pinned 1.21.11 registry. |
| `DW0150` | 6 Cross-stage | Planned quest (stage 4) has no expansion in stage 5. |
| `DW0151` | 6 Cross-stage | Stage-5 quest is not planned in stage 4. |
| `DW0152` | 6 Cross-stage | Stage-2 NPC has no stage-6 dialogue tree. |
| `DW0153` | 6 Cross-stage | Stage-6 dialogue tree references an NPC not declared in stage 2. |
| `DW0160` | 6 Prefab binding | Area binds neither or both of `prefab` / `prefab_pool` (exactly one required). |
| `DW0161` | 6 Prefab binding | Area `prefab_pool` references a pool absent from `prefabs/` metadata. |
| `DW0170` | 5 Waves (v0.3) | A `kill` objective or `spawn-wave` effect references a `wave/<id>` not declared in the stage-5 `waves` section (dangling wave ref). |
| `DW0171` | 5 Waves (v0.3) | A declared wave is referenced by a `kill` objective but is never spawned by any `spawn-wave` effect (a wave must be spawned before its kill objective is reachable). |
| `DW0172` | 5 Flags (v0.3) | A `requires_flags` entry references a `flag/<id>` that no `set-flag` effect ever produces (dangling flag ref). |
| `DW0173` | 5 Waves (v0.3) | A wave mob `entity` is not a known vanilla entity id. Item-id checks for `collect.item`, `interact.requires_item` and `give-item.item` reuse `DW0143`; their anchors reuse `DW0142`. |

`severity` is `error` for every v0 code; `warning` exists in the shape for
future advisory rules. `path` is a JSON-pointer-ish location within the stage
document (map-key segments are not `~1`-escaped — it is a locator, not a strict
pointer). `DW0100`'s path is the document root, since serde parse errors are not
path-addressable.

### DSL versions (0.2.0 and 0.3.0)

v0.3 is an **additive superset** of v0.2 (`SUPPORTED_DSL_VERSIONS`): a v0.2
campaign remains valid and compiles byte-identically. The new stage-5 verbs
(`kill`/`collect`/`interact`), effects (`give-item`/`set-flag`/`spawn-wave`),
the `waves` section and `requires_flags` are gated on `dsl_version` 0.3.0 — the
gate is the **quests-stage** version (all the v0.3 surface lives in stage 5).
The `v03_checks` group (`DW0170`–`DW0173`, plus reuse of `DW0142`/`DW0143`) runs
only under 0.3.0; under 0.2.0 those verbs/effects are still rejected as reserved
(`DW0141`). A campaign is expected to use a uniform version across its six
documents; the mixed-version invalid fixtures are a testing convenience.

### Reserved values

`DW0141` covers enum values that are not yet implemented **for the campaign's
`dsl_version`**:

- `npcs`: `role: vendor` / `role: boss` (reserved in both 0.2.0 and 0.3.0).
- `quests` **under 0.2.0 only**: objective `type: kill | collect | interact`;
  effect `type: give-item | set-flag | spawn-wave`. Under 0.3.0 these are
  implemented (see [DSL versions](#dsl-versions-020-and-030)).

(`prefab_pool` is no longer reserved in v0.2 — it is a real stage-1 binding,
validated by `DW0160`/`DW0161`.)

These **parse** (so authors get a clean diagnostic instead of an opaque error)
and are rejected by validation. The reserved kit-item fields (`lore`,
`enchantments`, `attributes`) are the exception: they are intentionally *not*
defined as fields, so a document using them is rejected as an unknown field
(`DW0100`).

## Fixtures

### Valid — `fixtures/valid/hello-world/`

The complete canonical hello-world campaign, six documents (`world`, `npcs`,
`classes`, `quest-plan`, `quests`, `dialogue`) — v0.2: `npcs` carries the
keeper's structured `persona`, `dialogue` carries his tree. It validates with
zero diagnostics and is byte-identical under the canonical writer.

### Invalid — `fixtures/invalid/`

Self-describing patch files named `<code>-<slug>.json`:

```json
{
  "description": "npc references an unknown area",
  "expect": "DW0112",
  "documents": { "npcs": { "...": "full replacement envelope for this stage" } }
}
```

- `expect`: the single diagnostic code the fixture must produce.
- `documents`: a map of stage name → the **full** stage envelope that replaces
  the valid one. Most fixtures replace exactly one stage; a few (e.g.
  `DW0132`) must replace two to violate their rule *in isolation* without
  tripping the cross-stage 1:1 rule.
- `schema_reject` (optional, default `false`): the overridden document must also
  be rejected by the exported JSON Schema (schema-level violations only, e.g.
  `DW0100`).

`tests/matrix.rs` walks the matrix: every invalid fixture yields exactly its
`expect` code, and the valid campaign yields zero. `tests/schema.rs` validates
every valid fixture against its exported schema and checks that every
`schema_reject` fixture is rejected.

## Registries

Item-id and anchor checks go through the `ItemRegistry` / `AnchorRegistry`
traits. This crate ships small **vendored v0** implementations covering only what
the M1 fixtures use:

- `VendoredItemRegistry::v1_21_11()` — the item ids in `data/items-1.21.11.json`.
  **The full 1.21.11 item registry is vendored in the compiler task (spec-0002);**
  the compiler injects it via `validate_campaign_with`.
- `VendoredEntityRegistry::v1_21_11()` — the hostile-mob ids in
  `data/entities-1.21.11.json`, used to validate v0.3 wave mobs (`DW0173`). A
  full 1.21.11 entity registry is injected by the compiler (currently the same
  vendored subset, pending the wave-emission task).
- `VendoredAnchorRegistry::hello_world()` — the anchors the hello-world prefab
  declares (`data/anchors.json`) plus a fixture pool (`data/pools.json`) so
  prefab-pool existence checks are non-vacuous. Real prefab anchor metadata lives
  in `prefabs/` (ADR-0004) and is resolved by the compiler; the trait lets the
  compiler inject it.

`AnchorRegistry` is the **prefab-metadata surface** DSL validation resolves refs
against. Beyond `anchors_for`, it exposes `has_pool` (prefab-pool existence,
`DW0161`) and `lighting_for` (the typed `Lighting` / `LightingProfile` block,
consumed by the compiler's `dark`-mitigation analysis, `DW0210`). The compiler's
`PrefabRegistry` implements all three from `prefabs/` metadata.

## Spec notes / resolved ambiguities

<a id="dw0132"></a>
- **`DW0132` (finale reachability).** In a valid DAG with all references
  present, a finale is *always* reachable by dependency traversal, so the rule
  cannot be violated in isolation under a literal reading. v0 implements the
  concrete, independently-testable reading: **every planned quest must be a
  transitive dependency of `finale`** — the plan converges on the finale. This
  is a *structural* stage-4 check and is distinct from the compiler's deeper
  semantic reachability (`analyze`, exit 2, `DW0201` in spec-0002); the compiler
  may run both.
- **Objective/dialogue id uniqueness.** spec-0001 calls dialogue/objective ids
  "stage-local". v0 enforces: dialogue node ids unique within each NPC's graph;
  objective ids unique across all of stage 5 (so cross-stage `complete-objective`
  refs resolve unambiguously).
- **`quest-complete` trigger.** The reference field is named `quest`
  (`{ "type": "quest-complete", "quest": "quest/…" }`).
- **Anchor resolution** is performed here via `AnchorRegistry`, but prefab
  metadata itself (including the `spawn` requirement) is owned by the compiler
  and `prefabs/`; the vendored registry only knows the hello-world prefab.

## Dependencies

`serde`, `serde_json`, `schemars`, `thiserror` (all MIT/Apache-2.0). Dev-only:
`jsonschema` (MIT) with default features **disabled** (its HTTP/TLS resolver
tree is not needed for offline validation and is excluded).
