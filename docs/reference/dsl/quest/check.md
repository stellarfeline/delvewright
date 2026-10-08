# `delvewright_dsl::quest::check`

The reference page for `crates/dsl/src/quest/check.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](../diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0140` | Objective `after` cycle. |
| `DW0150` | Planned quest (stage 4) has no stage-5 expansion. **Two readings, one code, and the discriminator is whether stage 5 declares any quests at all.** Where it declares some and this id is not among them, the refusal is per plan entry and names its two ordinary remedies — write the expansion, or drop the entry — plus how many quests stage 5 does declare, which is what says *mismatch* rather than *unwritten*. Where it declares **none**, the campaign is between the stage-4 plan and the stage-5 quests, every planned quest is unexpanded by construction, and the two remedies are both wrong: writing the expansions IS the next authoring step, and the plan is not a mistake to delete. That case is **one** diagnostic on `/content/quests`, on the model `DW0874` sets — it names every planned quest, says the state is an authoring state rather than a fault, says why the refusal still stands (a plan entry with no expansion has no trigger, no objective and no completion, so nothing of it is emitted), and says there is **no cheaper way out**: the schema-minimal stage-5 quest is refused again by `DW0481` once per quest and `DW0460` once per NPC live in it, so writing empty expansions raises the count instead of lowering it. That last sentence is a measurement and `crates/delvec/tests/plan_awaiting_expansion.rs` takes it; the wording is `crates/dsl/tests/dw0150_plan_awaiting_expansion.rs`. Severity, code and exit are identical in both readings — a plan awaiting expansion cannot build, and a warning would let an unbuildable campaign read as buildable at the step where the difference decides whether anyone writes stage 5. |
| `DW0151` | Stage-5 quest not planned in stage 4. |
| `DW0171` | A killed wave is never spawned by any `spawn-wave`. |
| `DW0357` | A `carrier: "one"` `give-item` sits in a nested bundle with no acting player — a `move-npc`/`move-actor` `on_arrive`, a `bonfire`'s `on_rest`, or a `sequence` step of a timeline started where nobody acted (a polled trigger, a trap, a shortcut) (spec-0018; the seams are `QuestEffect::nested_effect_dispatch`, spec-0085). Those run with the server command source, so the single prop would reach nobody. A `sequence` step under a root that has an actor keeps it (the timeline carries its actor), and `set-checkpoint.on_respawn` / `begin-stealth.on_caught` are dispatched per player and do have an `@s`. A root's own top level is not refused: a polled root lowers the give to the party. Validation-tier (exit 1), `dsl::validate`. Prescription: drop `carrier` (arm the whole party), or move the hand-off onto the beat a player completes. |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../../delvec/compiler/nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0329` | A `sequence` effect is nested inside another `sequence` (directly, or reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse (spec-0014). Validation-tier (exit 1), `dsl::validate`. Flatten the inner steps into the outer timeline (shift their `at_ticks`). |

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

This module's rows of a section whose prose is on the [`delvewright_dsl::loot` page](../loot.md#dw043x--geometry--container-proofs-stair-orientation-spec-0021-loot-collect-container-adoption).

| Code | Meaning |
|------|---------|
| `DW0437` | An `interact` declares `missing_item_hint` without a `requires_item`. Validation-tier (exit 1). The hint exists to answer a click that arrives without the required item **in hand**; with no item gate there is no such click, so the authored line is dead content that could never narrate — and an author who wrote one plainly meant to gate the interaction. Prescription: add the `requires_item` the hint is about, or drop the hint. |

### DW0540–DW0542 and DW0545 — status effects, the region teleport, and the fixture class (`dsl::quest::check` / `compiler::teleport` / `compiler::affordance`; spec-0031)

`DW0540` is the one rule in this family that is about a *pattern* rather than a
value, and it is the reason the surface is shaped the way it is. `give-effect`
has no infinite form: `seconds` is required and bounded, so a grant always ends
by itself. That can still be defeated by two effects that are individually fine —
grant blindness for an hour, clear it four ticks later — and then the clear is
the real removal, so any path that does not reach it (a logout, a crash, a death
mid-chain, a `sequence` whose remaining `schedule` never runs) leaves the player
blind for the rest of the hour.

The rule therefore fires on exactly the grants that are **still live** when their
clear arrives. Where the duration expires first, the duration is the removal and
there is nothing to say. "The same sequence" is mechanical: a bundle's own
timeline, where a plain member runs at offset 0 and a directly-nested
`sequence`'s members run at their step's `at_ticks` (nested sequences are
`DW0329`, so the expansion terminates). Conditional continuations — `on_arrive`,
`on_caught`, `on_respawn`, `on_rest` — are separate bundles with their own
timelines and are not folded in; a clear hanging off an arrival is strictly more
fragile than one on a fixed tick, and it is the mandatory duration, not this
rule, that keeps that case survivable.

`DW0542` is what stands where a runtime exemption list would otherwise be. A
`teleport`'s selector is total over bodies, so a volume drawn over an affordance
the engine anchored to a *block* would move the entity and leave the hardware: a
campfire, a lever or a sealed door still visible, still reachable, answering
nothing. The affordance set is not enumerated by this proof — it is
`eclipse::affordances`, the same authority `DW0359` measures bodies against, plus
the seal shells `DW0422` owns — so an affordance added to the engine enters this
proof by existing. Content bodies (NPCs, actor puppets, wave mobs) are
deliberately not refused: moving them is the mechanism working, and it is what
the cargo-lift ruling asks for.

Binding: `validation/teleport-gate.json` states how many teleports were declared
and resolved, how many cells their volumes cover, how many affordances were
examined, and how many PackTest templates were generated — a compile-time-only
green over a runtime mechanism is the vacuity that last number exists to make
visible. A campaign that declares no teleport emits no file at all, so a file
that exists and reports zero is a finding rather than an absence.

#### DW0545 — the fixture class: what a region verb selects

`DW0542` reaches every place whose cell the compiler knows. **A recovery stake's
marker has no such cell** — its position is the death point, or a row of the
compile-time placement table picked by the respawn seat in force. A lift that
carried the marker away from the position its ledger recorded would leave
`stk_gc` finding nobody holding a wager there on the next tick and retiring it:
the wager would not be uncollectable, it would be deleted.

The two obvious fixes are both defects CLAUDE.md names. *Teleport exempts engine
machinery* re-implements a general mechanism privately inside one verb; *the
stake ledger survives its marker moving* keys a capability to the wrong object,
making the stake compensate for a selector that grabbed something it should never
have grabbed. The question is upstream of both — **what does a content-authored
region verb select?** — and the measurement answering it is short:

| region verb | what its emitted selector reaches |
|---|---|
| `teleport` (`from`) | every **entity** in the box — the only verb with no filter at all |
| `lethal_volumes[]` (`region`) | every entity in the box minus six **types** (`@e`), plus every player (`@a`) |
| `give-effect` / `clear-effect` (`in`), `damage-players` (`in`), stealth zones, the night-vision area grant | **players only** (`@a`/`@s`) — no engine entity is reachable |
| `fill-region`, `clear-region`, `collapse`, `close-gate` | **blocks**; no entity selector exists |

So exactly two verbs quantify over non-player entities, and only they have the
question to answer.

The answer is a **class the object declares about itself**, not a roster any verb
holds. Every entity the engine summons carries one of two tags:

- **`dw_fixture`** — *a place.* Its position IS engine state: an affordance's
  `minecraft:interaction` hitbox, the `dw_marker` display beside it, a stake
  marker, a cutscene's return mark. Moving it does not move a thing, it rewrites
  a fact.
- **`dw_borne`** — *carried by a body.* Exactly one: an NPC's co-located
  dialogue hitbox, which must ride whatever its speaker rides.

A cutscene *camera* declares neither and that is deliberate: its own driver
re-asserts its position every tick, so it is a body the engine flies rather than
a place it recorded. Neither tag is authorable, and no campaign JSON can turn
either off.

Every box-narrowed entity selector then carries `tag=!dw_fixture` — **one negated
tag for the whole engine, forever**, which is what a type roster can never be. A
type cannot answer this question at all: an NPC's hitbox and a stake's marker are
both `minecraft:interaction`, and a teleport must move the first and leave the
second. `lethal_volumes[]` keeps its type roster as well, because that roster
makes a different and still-true claim — *do not aim `/damage` at a thing that
cannot take it*.

The two arms of the rule divide by **who can act on the defect**: a place whose
cell is known at compile time is *refused* (`DW0542`), because the author can
move it; a place only the runtime puts down is *skipped by the selector*
(`DW0545`), because nobody can.

`DW0545` is an emission self-check over the shipped datapack, in the `DW0420` /
`DW0421` family — it is `DW0421`'s rule (*only the owner may disturb an
affordance's hardware*) one verb wider, since moving hardware is disturbing it,
and one binding wider, since a region verb selects by box where `DW0421` reads a
tag. It fires on two clauses, and both are compiler defects rather than authoring
ones: a summon that declares neither class (the exclusion then protects nothing),
and a box-narrowed `@e` selector with no exclusion (the class exists and this verb
does not read it). It can never be caused or fixed by campaign JSON: it is an
engine self-check, run on every build.

**The runtime half is the only half that can witness a marker carried off**, and it
is generated rather than argued: one PackTest template per `teleport`, in a
campaign that declares a stake able to leave a marker, puts a real marker in a
real volume through the campaign's own `stk_fill_<id>`, rides the campaign's own
`teleport_<key>`, and asserts a plain body **left** the box while both halves of
the marker stayed. The body assertion is what stops it being one-directional —
without it, an engine whose teleport did nothing at all would pass. One template
per teleport rather than per (`teleport`, `stake`) pair because the marker is one
object: every stake summons the same two entities through the same `stk_place`,
so a second template for a second stake would race the first for one entity at
one position on the shared batch server. Every selector such a template writes
over the marker class is scoped to the place it is about, for the same reason.

Binding: `validation/fixture-gate.json` states how many entities declared each
class, how many box-narrowed selectors were examined, and how many runtime
templates were generated. Zero on either of the first two counts is reported as
`unbound` **with an `unbound_reason` naming which arm** — an empty class makes
every exclusion decorative, while zero selectors means the class is bound and the
clause the defect lives in is simply not exercised by this campaign. The two are
not the same finding and the ledger never makes a reader guess which one it is.

| Code | Meaning |
|------|---------|
| `DW0540` | **A grant whose removal is a later effect, not its own duration.** A `give-effect` is still live at the moment a `clear-effect` for the same effect fires in the same bundle. Validation-tier (exit 1), `dsl::validate`. The message carries both numbers the author needs — how long the grant runs, and how long the bundle actually needs it for. Prescription: set `seconds` to the span the effect should last and delete the `clear-effect`; a duration expires with no cooperation from anything. `clear-effect` is for effects this campaign did not grant. |
| `DW0541` | **A duration that is not a duration.** A `give-effect`'s `seconds` is zero or past `MAX_EFFECT_SECONDS` (50 000, derived from `MAX_POTION_DURATION_TICKS`), or its `amplifier` is past vanilla's unsigned byte. Validation-tier (exit 1), `dsl::validate`. Zero is the grant that never happens — the unbound-vacuity class as a number; the ceiling is vanilla's own field width, so a value above it is a duration typed in ticks or milliseconds. |

### DW0510–DW0512, DW0891, DW0922–DW0923 — lethal volumes (`compiler::nav` / `compiler::lethal` / `dsl::validate`; spec-0031, DSL v0.10; spec-0062)

This module's rows of a section whose prose is on the [`delvec::compiler::lethal` page](../../delvec/compiler/lethal.md#dw0510dw0512-dw0891-dw0922dw0923--lethal-volumes-compilernav--compilerlethal--dslvalidate-spec-0031-dsl-v010-spec-0062).

| Code | Meaning |
|------|---------|
| `DW0957` | **An `interact` prop a step fires.** An `interact` objective's `prop.block` is a pressure plate or the tripwire string — a block that tells the player to walk onto it — while the objective completes on a right-click, so the step does nothing. The set is `dsl::stepped_blocks`, the pinned registry's ids that `TrapTrigger::is_trigger_block` accepts for a plate or a tripwire (every `*_pressure_plate`, and `minecraft:tripwire`; the hook is clicked, not stepped), held equal to vanilla's `#pressure_plates` tag plus the string by `tests/stepped_blocks_tag.rs`. Validation tier (exit 1), `dsl::validate` beside the prop's `DW0193`. Prescription: give the objective a block a hand works (a lever, a button), or make the step the act — a `trigger` with `on: step` at an anchor whose cell holds the plate. |
