# `delvec::compiler::hop`

The reference page for `crates/delvec/src/compiler/hop.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1103 — a hop passes through no block (`compiler::hop`; spec-0136; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1103` | **A hop carries its body through a solid block** (spec-0136 §3.1). A `hop` is walked by teleport on a body that stays `immovable` and collides with nothing, so a ceiling lower than the arc does not stop it: the body's head goes into the block. Build-tier (exit 3), `compiler::hop`, asked over the assembled world for every `hop` at every effect root and nesting depth, once per body it names. At the cell the body stands on when the hop fires — for an NPC whose hop the critical path fires (the flow journal names the hop's pointer at one step), the station the cast ledger gives it at the moment after that step, a planned walk's end included (`view::moment::cast_at` over `story_at`, the one answer the story pictures read), and not swept when the ledger has it out of the world; otherwise the cell it is placed on (its mark, `Plan::body_point`) — the sweep is every cell above the cell its feet stand in, up to the cell the top of its body reaches at the arc's peak — `dsl::moves::hop_arc`'s greatest height plus the body's height (`nav::entity_dims` of the entity it ships as) — in every column its width covers; a cell holding a block a body cannot pass (`blockshape::passes_body`) is the finding. The message names the hop's pointer, the body, its cell, the peak, the body's height, the solid cell and the cells of clearance under it. The first finding is the build's; the rest print beside it. Prescription: lower the hop's `height`, stand the body where the room is taller, or leave it out of the hop. Not caught: a body a `move-npc` / `move-actor` has walked to another cell (the sweep is at the placed cell), and a body a `resize` or a declared `scale` has changed (the size is its entity's). The cell the feet stand in is not swept: a slab or a carpet under a body is its floor. |

**Binding.** A build that assembles a world and declares a hop prints `hop binding: H hop(s) declared, B body arc(s) swept where the story stands the body when the hop fires over C cell(s), R refused (DW1103)` before the verdict is taken. A campaign that declares no hop prints no line.
