# `delvec::compiler::happening`

The reference page for `crates/delvec/src/compiler/happening.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

| Code | Meaning |
|------|---------|
| `DW1070` | **A step that says it opens something opens nothing (`compiler::happening`; error; exit 1).** spec-0124 §6.2, §7.2 (`happening::check`, validation, reads the documents alone). Every `happening` on an objective, a dialogue option or a story-node effect is counted; one whose `verb` is `opens` is judged against its node's own effects — an objective's completion bundle (`on_objective_complete[<id>]`), an option's effects, the effect itself, nested lists included — and passes when they carry an `open-gate` or an `open-way` and, where the `subject` names an `anchor/`, an `open-gate` on that anchor; `seals` passes on a `close-gate` the same way. A dialogue option's effects open nothing the world has. The message names the node, the verb, the subject and what the node does carry. Binding line: `happening binding (DW1070): H opens/seals happening(s) among T; n refused`. Prescription: add the opening effect on the thing named to the node's own effects, or state what the node does with another verb. |
