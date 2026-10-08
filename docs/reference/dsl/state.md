# `delvewright_dsl::state`

The reference page for `crates/dsl/src/state.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0919 — a currency stands where the sidebar can draw it (`dsl::state`; error; exit 1)

**A standing display is a claim about the one slot that stands, so a declaration the slot cannot draw as written is refused where it is written.** `state[].display: sidebar` (spec-0076) is optional on every datum; `dsl::validate::state_checks` judges it against the declaration itself, at `delvec validate`, before any build. What stands is three `setup` lines (the `state[].display` surface row): the objective headed with the datum's translated `name`, its value painted gold, and the objective put in the slot, once, at world init. The quantifier is every `state[]` entry carrying `display`.

| Code | Meaning |
|------|---------|
| `DW0919` | **A standing display the sidebar cannot draw as declared** (spec-0076 §7). Three shapes under one rule, the `DW0520` shape: (1) two datums both declare `display: sidebar` — the slot holds one objective, and the refusal names both rather than picking by order, because which purse the party reads between changes is the creator's decision; (2) the datum has no `name` — the slot's heading is the display name, and without one the objective's internal id would stand on every screen, the one thing the slot must never show; (3) the datum is `party`-scoped — its value lives on the `#party` holder, and the sidebar hides every `#`-prefixed holder, so the display would be a heading over nothing; mirroring the value onto a visible fake player is a name a real player could carry under a label the engine would have to invent, and is not done. Path `/content/state/<i>/display`; the message names the datum (both, for shape 1). Prescription: keep one `display`, give the datum a `name`, or declare it `player`-scoped; a `party` purse keeps its announcement and stands nowhere, the recorded gap. Validation tier (exit 1). |

### DW050x — runtime state (`dsl::state`; spec-0031)

Runtime state is a **declared** datum: a name, a scope (`player` / `party`) and
an initial value, written by `set-state`/`add-state`/`clear-state` and compared
against by `requires_state` in any gate. All four codes are validation-tier (exit
1), in `dsl::validate::state_checks`. A campaign that declares no datum emits
none of it: no scoreboard objective, no `state_seed` function, no tick clause,
no guard clause.

Both directions of the read/write ledger are errors, because each is a **vacuous
binding** in the CLAUDE.md sense and each is silent — the campaign compiles, the
datapack loads, and the delve plays as though the mechanism were live.

| Code | Meaning |
|------|---------|
| `DW0501` | **Read, never written.** A gate's `requires_state` reads a declared datum that no verb anywhere in the campaign ever writes, so it can only ever hold its declared `initial` and every comparison against it was decided when the campaign was written. The gate is a constant wearing a condition's clothes — the numeric form of a combat floor examining zero enemies. Prescription: write it somewhere, or drop the comparison and say what you meant unconditionally. Its emitted-layer sibling is `DW0495`, which asks the same question of the commands rather than of the campaign, and therefore reaches the engine-internal objectives no campaign can declare. |
| `DW0502` | **Never read.** A declared datum that no gate's `requires_state` anywhere in the campaign ever reads. Either some verb writes it and nothing ever asks (an inert write — a counter nobody consults), or nothing touches it at all (a dead declaration). Runtime state exists to be compared against; a datum with no reader is bookkeeping no player can observe. Prescription: gate something on it, or delete the declaration and its writes. |
| `DW0503` | **No acting player.** A `player`-scoped datum is read or written where emission has no `@s` to resolve it against. Every such place is a property of the SITE, never of the verb, and there are three kinds. (1) **The root.** Six of the nine effect roots run with an acting player and three do not — a trigger's `effects`, a trap's `payload` and a shortcut's `on_unlock` are polled on the tick from the server command source (`Audience::Scheduled`), while `on_objective_complete` / `on_complete` are dispatched `as @a`, `on_death` / a dialogue `on_respawn` are the dying-or-respawning player's own, a shop offer is the buyer's, and a fight's `on_kill` (spec-0074) is the credited killer's. The answer is `EffectRootKind::runs_with_acting_player`, bound by equality to `emit::root_audience` over the closed root set — **except that a trigger answers per declaration**: `audience: presser` is dispatched by the interaction advancement and does have an `@s`, so the check asks `EffectRootSite::runs_with_acting_player` (which consults the trigger) and the kind-level answer stays the class default. Asking the kind would refuse a `player`-scoped read the emitter can serve. (2) **The seams inside a bundle**, one statement read by `DW0357` too (`QuestEffect::nested_effect_dispatch`, spec-0085): a `move-npc`/`move-actor` `on_arrive` and a `bonfire`'s `on_rest` drop the actor; a `set-checkpoint` `on_respawn` and a `begin-stealth` `on_caught` restore it; a `sequence` step keeps whatever its timeline was started with — under a root with an actor the timeline carries it by a tag, so a step there has one, and under a polled root it has none. **A fourth shape** (spec-0085): an effect declaring `audience: actor` where emission has no acting player — the same rule, *no `@s` where emission has none*, with the same remedy: move the beat onto a site a player drives, or address the party. (3) **The gates emission evaluates against the party holder** — an objective's activation guard, a trigger's arming gate, a trap's arming gate. Reads and writes are treated alike: a per-player score named from a sourceless function is `@s` with nothing to resolve it to, whether the command is a `scoreboard players set` or an `execute if score`. Prescription: declare the datum `party`-scoped if the whole party shares it, or move the read/write onto a site a player drives — a dialogue option, a cast placement, `on_death`, or an effect on a beat a player completes. |

#### Which sites can touch a per-player datum, and why it is decidable

Two closed sets answer it, and neither is a list anybody maintains.

`GateConsumer::evaluates_per_player` answers for a gate's own site, and returns
`Option<bool>`: a dialogue option's availability is computed per player into
`dw.dmask` and its `/trigger` handler runs `as @s` (`Some(true)`); a cast
placement selects a scene into a per-player `dw.cast` (`Some(true)`); an
objective's guard, a trigger's arming gate and a trap's arming gate are party
predicates by construction (`Some(false)`). **`Effect` answers `None`** — an
effect's gate is evaluated wherever its bundle runs, and that belongs to the
root. The `Option` is deliberate: a plain `true` for `Effect` would be right
for `on_objective_complete` and wrong for four of the ten roots, silently. An
eighth consumer class cannot compile without answering.

`EffectRootKind::runs_with_acting_player` answers for the root, exhaustively, and
`emit::root_audience` is the single place the emitter chooses a bundle's
audience — one function rather than a literal per call site, bound to
the DSL's answer by equality in `emit::tests::root_audience_matches_the_dsl`. A
root whose emitted audience moved without that answer moving with it would turn a
validated per-player read into an `@s` in a sourceless function, with every check
green; a tenth root fails the bind until both sides name it.

#### One gate, three fields

`requires_flags` / `forbids_flags` / `requires_state` are one object
(`dsl::gate::Gate`), and every consumer answers `gate()`. Two things keep that
from decaying:

- `crates/dsl/tests/gate_consumers.rs` enumerates the gate-declaring object
  schemas **from the generated JSON Schema** — derived from the Rust types, so
  the enumeration is complete by construction rather than by diligence — and
  fails when any of them declares part of the gate and not the rest. It states
  its binding count (`GATE_SITES` declaring schemas, `GateConsumer::COUNT`
  consumer classes) and asserts it exactly, so a new gate consumer is a
  deliberate diff rather than a silent one.
- An effect's gate is one `Guard` under `QuestEffect::when`, carried by every
  verb, so every verb is gatable on identical terms and
  `tools/ci/check-capability-ownership.py`'s `MODIFIER_HOLES` holds no gate
  field. A gate is one object: giving its comparison a different carrier set
  than its flags would make "which verbs are gatable" two different answers.

**Where a comparison IS evaluated, and where it is not.** A `requires_state`
comparison stays out of the monotone producibility fixpoint, exactly as
`forbids_flags` does: that fixpoint has no notion of *when*, and a comparison is
entirely about when. The compensating stronger check is the **path replay**,
which does have a concrete order — so a numeric gate is evaluated there, against
the value the path itself has produced by the time the gate is read, and
`DW0879` refuses one the path has already made unsatisfiable. `DW0501` is the
other half and asks a different question: whether the datum is driven at all.

### DW0847 — a gate that can never open (`dsl::state`; every gate consumer)

| Code | Meaning |
|------|---------|
| `DW0847` | **A gate contradicts itself, so it can never open.** A flag on both `requires_flags` and `forbids_flags`, or `requires_state` terms on one datum that no integer satisfies (`at-least 5` with `at-most 3`, two different `equals`, a `not-equals` punching out the only pinned value). The thing carrying it — objective, effect, trigger, trap, dialogue option, cast placement, shop offer — is authored content that provably never happens. One rule over the whole closed consumer set (`dsl::gate::for_each_gate`), because satisfiability is a property of the **gate**, never of the verb that first needed the question answered — the first asker was the cast ladder's per-clause solver (`DW0846`), and a check written beside it would have left the other six classes with no surface. The arithmetic is `dsl::gate::DatumSet` (interval-with-holes intersection, exact emptiness), the same value-picker the solver drives generated `cast_ladder_*` phases from, so "can this open" and "at what value" have one authority. Validation tier (exit 1) — it judges an authored contradiction, a fact of the campaign alone. Distinct from `DW0501` (a satisfiable comparison whose datum nothing writes) and from the flow proofs' flag reachability: this is emptiness of the gate itself, before any question about what the campaign does at runtime. Prescription: fix the gate, or delete the thing it makes unreachable. |

### DW0520–DW0527, DW0880 — trade and the recovery stake (`dsl::validate` / `compiler::stake`; spec-0032)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw0520dw0527-dw0880--trade-and-the-recovery-stake-dslvalidate--compilerstake-spec-0032).

| Code | Meaning |
|------|---------|
| `DW0527` | **A comparison read after the bundle changed what it compares.** An effect's `requires_state` names a datum that an earlier effect in the same bundle writes **behind a gate on that same datum** — so the comparison is made on the far side of the boundary the bundle just tested. Warning-tier (exit 0), `dsl::validate`. Found in the emitted output of this feature's own first shop: written "purchase, then apology", buying your LAST coin debits it and the `at-most` apology — evaluated after the debit — then holds too, so the player is charged AND told they cannot afford it. The fix is always local: put every reading effect ahead of the write. An **unconditional** write followed by a comparison is deliberately NOT diagnosed — `set-state toll 0` and then a door gated on `toll at-most 0` is the ordinary sequenced idiom and plainly means the value the bundle just produced. **Its scope is ONE bundle's own effect list, and that is what it does not cover**: a write and a read four beats apart are two bundles, so a `clear-state` that empties a datum a later objective's gate depends on is invisible here. `DW0879` is that question, asked over the path rather than over a list. Prescription: reorder, or gate on something this bundle does not change. |
