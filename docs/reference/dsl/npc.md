# `delvewright_dsl::npc`

The reference page for `crates/dsl/src/npc.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0195` | A `talk-to` targets an NPC despawned by a prerequisite quest. |
| `DW0197` | A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect anywhere (quest, trigger, nested timeline, or dialogue) summons it — the NPC never enters the world, so its dialogue tree and any `talk-to` on it are unreachable content. The staging dual of `DW0195`. Prescription: add the `spawn-npc` at the entrance beat, or drop `deferred`. |
| `DW0198` | A `talk-to` on a `deferred` NPC provably activates before the NPC exists: every `spawn-npc` for it fires in a quest that is a **strict DAG descendant** of the objective's quest. Conservative by construction — a spawn from a trigger, from dialogue, or from the objective's own quest is not DAG-ordered and suppresses the proof rather than risking a false positive (see §4 "Deferred-NPC staging order"). |

### DW0978/DW0979/DW0980 — a sheet is drawn to its own model's boxes (`compiler::skinparts` + `dsl::npc`; error)

This module's rows of a section whose prose is on the [`delvec::compiler::skinparts` page](../delvec/compiler/skinparts.md#dw0978dw0979dw0980--a-sheet-is-drawn-to-its-own-models-boxes-compilerskinparts--dslnpc-error).

| Code | Meaning |
|---|---|
| `DW0980` | **A body's `skin.hidden_layers` names one layer twice** (`dsl::npc::skin_layer_checks`, over `dsl::body::body_skin_sites`, validation tier). The list is the set of layers the mannequin does not draw; the message names the body and the layer. Prescription: name each layer once. |
