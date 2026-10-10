# `delvewright_dsl::body`

The reference page for `crates/dsl/src/body.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW045x — body clearance and body traversal (`compiler::clearance` + `compiler::traversal` + `dsl::validate`; error + advisory)

This module's rows of a section whose prose is on the [`delvec::compiler::traversal` page](../delvec/compiler/traversal.md#dw045x--body-clearance-and-body-traversal-compilerclearance--compilertraversal--dslvalidate-error--advisory).

| Code | Meaning |
|------|---------|
| `DW0455` | **A declared locomotion the engine cannot hold the body to** — `aquatic`. Numbered in the 045x body family but **validation-tier (exit 1)**, like `DW0320`: it is raised by `dsl::validate_campaign_with` at pipeline step 3, refused at declaration time rather than accepted and ignored. `aquatic` is the one class that carries no exemption and governs no rule (it is a ledger label read off vanilla's own `#minecraft:aquatic` tag), so declaring it could never change a verdict and would land in `DW0454` every time; a value whose only possible outcome is another diagnostic is a trap, not a surface. The message NAMES the gap rather than leaving it to folklore (CLAUDE.md no-hack rule): routing has ONE reachability model, standable ground, and water-flooded cells are impassable and never floor for every body, so there is nothing for an aquatic claim to feed — routing has no water model (a capability gap). Prescription: remove the declaration — a route that crosses water is already governed by the flooded-cell rules, and a body vanilla itself calls aquatic still reaches the binding ledger under its derived class. |

### DW099x — a body watches (spec-0101; `dsl::validate` + `compiler::watching`; error)

A **watching body** turns, every tick, to face the nearest player its `watch.who` names within `watch.within` blocks (`compiler.md` §2 stage 2 `watch`, §3 "body watch"). The declaration is refused where it can never turn the body: when the class it names is one nobody can take (`DW0996`, here) and when no cell a player walks to stands within its reach (`DW0997`, on the [`delvec::compiler::watching` page](../delvec/compiler/watching.md)).

| Code | Meaning |
|------|---------|
| `DW0996` | **A watch for a class nobody plays** (spec-0101 §5.1). A body's `watch.who.class` names a class stage 3 does not declare. The watch line filters its player by the `dw_class_<c>` tag the class apply puts on whoever takes the class; a class nobody can take is a tag nobody wears, so the body could never turn. Judged over the whole campaign by `dsl::validate_campaign_with` (a stage-2 body may name a class written at stage 3, as `DW0197` judges a stage-2 body against stage 5), over `dsl::body_watch_sites`, so both body classes are judged by one rule. Build tier (exit 3). Names the body, the class and the classes the campaign declares. Prescription: name a declared class, or watch `"nearest"`. |
