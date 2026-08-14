# DSL validation fixtures

The corpus `tests/matrix.rs` and `tests/schema.rs` run against. Every `DW01xx`
diagnostic has at least one fixture here that asserts its code.

## `valid/hello-world/`

The canonical complete campaign: six stage documents (`world`, `npcs`,
`classes`, `quest-plan`, `quests`, `dialogue`). It validates with zero
diagnostics, is byte-identical under the canonical writer, and is what every
invalid fixture is a patch against.

## `invalid/`

One self-describing patch file per rule, named `<code>-<slug>.json`:

```json
{
  "description": "npc references an unknown area",
  "expect": "DW0112",
  "documents": { "npcs": { "...": "full replacement envelope for this stage" } }
}
```

| Key | Meaning |
|---|---|
| `description` | What the fixture violates, in one line. |
| `expect` | The single diagnostic code the fixture must produce — and the filename prefix. |
| `documents` | Stage name → the **full** stage envelope replacing the valid one. |
| `schema_reject` | Optional, default `false`: the overridden document must also be rejected by that stage's exported JSON Schema. Set it only for schema-level violations. |

A fixture must violate exactly one rule: `matrix.rs` asserts the produced code
set equals `{expect}`, so a patch that also trips a cross-stage rule fails.
Replace more than one stage where that is what it takes to keep the violation
isolated — `DW0132` does.
