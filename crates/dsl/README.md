# delvewright-dsl

The campaign format that the [`delvec`](https://crates.io/crates/delvec)
compiler reads: Rust types, validation, a canonical writer, and JSON Schema
export for the staged JSON documents that describe a **Minecraft Java Edition
1.21.11** adventure map.

A campaign is six documents — `world`, `npcs`, `classes`, `quest-plan`,
`quests`, `dialogue` — plus an optional `world-edits` script and one
`l10n/<code>.json` sidecar per translated language. Each is an envelope
`{ dsl_version, campaign_id, stage, content }` wrapping that stage's payload.
Later stages reference earlier ones and never the other way round, so a campaign
can be written and checked one stage at a time.

The JSON Schemas are generated from the Rust types, so the schema a document is
authored against and the parser that reads it cannot disagree.

## Use

```toml
[dependencies]
delvewright-dsl = "0.1"
```

```rust
use delvewright_dsl::{RawCampaign, check_campaign, parse_campaign, stage_schema};

// Six JSON strings in, diagnostics out.
let raw = RawCampaign { world, npcs, classes, quest_plan, quests, dialogue, world_edits: None };
let diagnostics = check_campaign(&raw);   // parse, then validate

// Or in two steps.
let campaign = parse_campaign(&raw)?;
let diagnostics = delvewright_dsl::validate_campaign(&campaign);

// The schema for stage 1, as serde_json::Value.
let schema = stage_schema(delvewright_dsl::Stage::World);
```

## What it provides

- **Types** — every stage payload as serde structs, all
  `#[serde(deny_unknown_fields)]`, so a typo is a diagnostic rather than a
  silently ignored key. Ids are type-prefixed kebab-case newtypes (`area/…`,
  `npc/…`, `class/…`, `quest/…`, `obj/…`, `dlg/…`, `anchor/…`, `prefab/…`).
- **Validation** — `validate_campaign` returns
  `Diagnostic { code, severity, stage, path, message }`, one stable `DW####`
  code per rule: id syntax and uniqueness, dangling and forward references,
  dialogue reachability, quest-graph cycles, cross-stage completeness, item and
  entity ids, translation coverage. `validate_campaign_with` takes registries so
  ids resolve against a real Minecraft registry and a real prefab library.
- **Canonical form** — `to_canonical_string` and `fmt`: two-space indent, sorted
  map keys, declaration field order, one trailing newline. Array order is
  semantic and is never reordered. Round-tripping a valid document is
  byte-identical, so an edit is a small diff instead of a whole-file rewrite.
- **JSON Schema** — `stage_schema` exports one full-envelope draft 2020-12
  schema per stage.
- **Registries** — `ItemRegistry`, `EntityRegistry`, `BlockRegistry`,
  `EffectRegistry` and `AnchorRegistry` traits, with small vendored
  implementations so the crate validates standalone, and injection points for
  the full ones.

All iteration is over ordered collections: nothing depends on hash order,
wall-clock time, or absolute paths.

## Compatibility

- **Campaign format**: `dsl_version` `0.2.0` through `0.14.0`. Each version is an
  additive superset of the one before, and a campaign is judged at the version
  it declares.
- **Minecraft**: Java Edition 1.21.11.
- **Rust**: 1.97.1 or newer.

## Documentation

- [Format reference](https://github.com/stellarfeline/delvewright/blob/main/docs/reference/compiler.md)
  — every stage's fields, every verb, and the complete `DW####` catalogue.
- [Project repository](https://github.com/stellarfeline/delvewright).

## Licence

GPL-3.0-only.
