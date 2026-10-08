# `delvewright_dsl::trigger`

The reference page for `crates/dsl/src/trigger.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0194` | Environment-trigger id malformed/duplicated, or `approach` `range` 0. |
| `DW0350` | A `use` trigger anchored where an NPC stands. Right-click on an NPC already belongs to its dialogue advancement; a second interaction hitbox in the same cell makes the client's entity ray-pick ambiguous, and whichever entity loses the tie is silently dead — the soft-lock class that starved the giant's dialogue of every right-click. Left-click triggers are exempt (a left-click has no dialogue meaning): they ride the NPC's own hitbox instead of summoning a second one. Validation-tier (exit 1), `dsl::validate`. Prescription: move the trigger to its own anchor, express the interaction as a dialogue option, or — if the NPC's body is genuinely the target — use `on: strike-npc`, which takes no anchor at all. |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../delvec/compiler/nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0427` | **A press answer addressed to a click vanilla cannot attribute**. A trigger declares `audience: presser` on something other than an `on: use` or an `on: step` (`EnvTrigger::attributes_its_actor`). `minecraft:player_interacted_with_entity` is the only vanilla criterion that runs a function as the player who clicked, and it fires on right-clicks alone; a step is a player standing in the cell, which the poll's own selector names; a left-click is recorded in the interaction entity's `attack` NBT as a UUID no command can become, and an `approach` is not attributed. Approximating it — polling the record and assuming the nearest player is the striker — is exactly the downstream folklore CLAUDE.md's no-hack rule excludes, so the capability is refused rather than faked. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: make it an `on: use` or `on: step` trigger, or drop `audience` and let the beat address the party. |
| `DW0428` | **An authored trigger id in the compiler's reserved `dw-` namespace**. The compiler synthesizes triggers of its own — the press answer every sealed gate and shortcut door gives (`trigger/dw-press-seal-<anchor>`, `trigger/dw-press-door-<shortcut>`) — and two triggers sharing an id would share one `dw_trig_…` tag and one emitted function, so one of them would silently disappear. Reserving the prefix makes the collision impossible by construction rather than improbable. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: rename it; any kebab id not opening with `dw-` is the campaign's. |
| `DW0429` | **A sealed body the campaign never answers**. A `shortcuts[]` door bars a gate from world-load, or a `close-gate` seals a wall, and nothing says what it answers when the party presses it — no `use` trigger anchored on it, and for a `close-gate` no authored `sealed_hint`. A player who walks the long way round, arrives at the wrong side of a door and pushes on it is told nothing; that is the press a shortcut loop most invites, and a sealed wall is the same defect one verb over. **One rule for both**, because two objects of one class with two defaulting policies is exactly the "capability keyed to the verb" defect this surface is CLAUDE.md's worked example of. The compiler had every ingredient to invent a line here and deliberately does not: a baked default decides the door's tone on the author's behalf and never discloses that it did, while an error makes the author say it (the no-hacks rule at a new site). It binds at every `dsl_version` the engine accepts (ADR-0024: there is one). Discharged by ANY `use` trigger on the body — `QuestsContent::answers_press_at`, the same predicate the synthesis reads — or, for a `close-gate`, by an authored `sealed_hint`; not by a `strike`, which is a different gesture. `dsl::validate::press_obligation_checks`, validation tier (exit 1). Prescription: the message carries the trigger JSON verbatim, and a test parses that prescription and asserts it clears the diagnostic, so it cannot come to name a field the schema does not have. |
