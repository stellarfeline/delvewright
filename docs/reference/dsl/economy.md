# `delvewright_dsl::economy`

The reference page for `crates/dsl/src/economy.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0901 — a purchase adds up where it is written (`dsl::economy`; error; exit 1)

**A price has no field, so the engine compares the copies of it.** spec-0032
rules that a price is a gate term and that an offer's refusal is authored: the
purchase behind `at-least <price>`, the apology behind `at-most <price − 1>`, the
charge an `add-state` of `−<price>`. That ruling stands, and its cost is that one
number is written four or five times — in the button's own words, on the gate of
what the player receives, on the gate of the charge, as the charge's `amount`,
and, minus one, on the arm that answers below it — with nothing else binding
them, so an offer gating on `at-least 15` and charging `16` would compile.

**The quantifier is every effect list whose effects charge a datum**, never a
shop: the rule reads the gate, so it binds wherever the shape occurs — a shop
offer, an environment trigger, a trap payload, a quest's `on_complete`, a
`sequence`'s step. A list is judged together with the gate of whatever it hangs
off (an offer's own gate, a trigger's or trap's arming gate, a nested list's
parent `when`), because both must hold for the charge to run; that is what lets
the correct spelling of a shop — gate the offer, leave the effects bare — pass
while gating the offer at 15 and charging 16 is refused.

| Code | Meaning |
|------|---------|
| `DW0901` | **A purchase whose literals do not add up** (spec-0071 §2). `dsl::economy::purchase_checks`, called from the one validation funnel, **validation tier (exit 1)**; the `DwCode` declares build tier, which is what happens if the rule refuses with a build under way. One code, three shapes, one rule — *a charge and the gate beside it are one price*. **A charge deeper than its floor**: an effect charges `n` of a datum while the floor its own `when` and its list's enclosing gate leave open is `m < n`, so at a balance of `m` the charge leaves the datum at `m − n` — below what the creator gated on. The message names `n`, `m`, the datum and the balance it lands on, and names the enclosing gate when one is read. **A charge nothing floors, in a list that prices the same datum elsewhere**: the copy is missing rather than wrong, and it fires at any balance including one that cannot pay. A list that says nothing else about the datum it moves is a design, not a purchase, and is not judged — a campaign may mean a datum to go below zero. **A refusal arm that does not meet the sale**: an effect gated `at-most k` on the datum and nothing else on it is the arm that plays when the player cannot pay, so `k` must be `m − 1`; a lower `k` leaves a **gap** (`k+1..m−1`, balances at which the button does nothing at all) and a `k` at or above `m` leaves an **overlap** (`m..k`, balances at which the player is charged *and* told they cannot afford it). An effect declaring both a floor and a ceiling wrote an interval and is left alone. **The arithmetic is the gate's own** (`gate::DatumSet::min`/`max`), the same interval-with-holes the satisfiability verdict is taken from, so the two cannot disagree about what a term means. **Its neighbour is `DW0527`**, which catches the same overlap arising from ORDER — an apology whose gate is read after the debit that moved it — where this one catches it arising from the literals; both prescribe moving something the author wrote, and neither prescribes a field. **Prescription**: raise the gate to the charge, lower the charge to the gate, or set the answering arm's ceiling one below the sale. There is no `price` field to add, and adding one would be a second comparison surface for one meaning. **Binding**: `purchase binding: C charge(s) over D datum(s) bind P (list, datum) pair(s) of L effect list(s) walked, G of them behind a numeric gate, R refused (DW0901).` on every validation run, zeroes included. Gallery probe: `a-key-that-charges-more-than-it-asks`. |

### DW0520–DW0527, DW0880 — trade and the recovery stake (`dsl::validate` / `compiler::stake`; spec-0032)

**There is no price diagnostic here, and its absence is the design.** A price is a
[`Gate`] term — the numeric comparison spec-0031 put in the shared gate rather than
in the verb that first asked for it — so everything that could go wrong with one is
already `DW0500`–`DW0503`: the datum must be declared, must be written somewhere,
must be read somewhere, and must be reachable at the scope the site evaluates at. A
shop that had grown a `price` field would have needed all four rules written a
second time, and the fifth consumer would have needed a fifth copy. `ShopOffer` is
therefore the **seventh gate consumer**, carrying `requires_flags` /
`forbids_flags` / `requires_state` like the other six and nothing of its own;
`crates/dsl/tests/gate_consumers.rs` asserts the positive half from the generated
schema and `crates/delvec/tests/v10_economy.rs` the negative half (no `price`,
`cost` or `compare` field exists anywhere in the shop's types).

`DW0520`–`DW0524` and `DW0527` are declaration and authoring rules: the first five live in `dsl::economy`, `DW0527` in `dsl::state`. `DW0525` and
`DW0526` are the **placement table's** proofs and live in `compiler::stake`,
because where a stake lands is a question about the solved layout — the same split
a lethal volume's `DW0512` and `DW0510`/`DW0511` make. `DW0880` lives there too,
and is about the marker rather than the anchor.

#### A marker is a PLACE, and a death leaves one place

The hardware is one class for the campaign — the `minecraft:interaction` box
tagged `dw_stk` and the glowing `minecraft:item_display` beside it, both summoned
by `stk_place` — and there is one of it at a place however many datums a death
forfeited there. That is forced rather than chosen. The placement table is keyed
on (respawn seat, death region) and never on the stake, so every stake one death
drops resolves to one anchor; and the rule degenerates to *leave it where the
player fell*, a position chosen at runtime that no compile-time separation can
reach. Four `1.0 × 2.0` boxes at one cell are coincident: any pick ray enters them
at the same distance and the client resolves the tie by entity iteration order,
which is exactly what `DW0878` refuses between two authored affordances.

So the place holds one box, and what was left there is counted in the per-player
ledger — where a wager always lived. One advancement fires one `stk_collect`,
which locates the place and offers it to every declared stake in turn, so one
right-click returns every datum that death left. **The place is the box the
player clicked**: the advancement only says some `dw_stk` box was used, so
`stk_collect` tags the player and runs `stk_pick` as every `dw_stk` interaction,
which keeps the one whose last user (`on target`) is that player with the latest
`interaction.timestamp` — the click just made — and the place's position is read
off it; no box, no collection. Choosing the box nearest the player instead would
offer the wagers at whatever place stood closest, so a player reaching past one
stake to click another would collect the near one (under `collect_by: anyone`,
another player's purse) and leave their own standing. `stk_ref` counts live wagers at
that position across every stake, and `stk_gc` — the one function permitted to
retire the hardware (`DW0421`) — deletes a place nobody has a wager at. Each
stake keeps its own forfeit rule, retention policy, collect rule, slots and
message: those are properties of a wager, not of a place.

What a place cannot hold is two answers to what it looks like, and that is
`DW0880`.

#### The placement rule, and why it is a table rather than a search

The rule: *the stake anchor is the point, on the walkable path from the respawn
point in force at the moment of death to the death point under the quest state in
force at that moment, that minimises distance to the death point.*

Read literally that is a runtime search, which ADR-0006 forbids. Read as a function
of three compile-time quantities it is a table, and every quantity already has an
owner: **walkable** is the same `nav::World` the completability proof runs on
(including a lethal volume's impassable cells, so "the near lip of the hazard"
falls out rather than being a second rule); **the quest state** is the DAG-indexed
sealing the region-write model establishes, swept over the seat's own arrivals by
`nav::reachable_under_every_quest_state`; and **the respawn point in force** is
engine state the runtime already keeps in `#cp dw.sys`.

The rule degenerates, which is why there is only one rule: a player who dies on
ground they can walk back to is at distance zero from themselves, so the anchor is
the death point. Only deaths whose position **cannot** host a stake need a row, and
there are exactly two kinds — a death inside a **lethal volume**, and a death on a
block **runtime can remove** (a lift car, a `close-gate` region, a `collapse`
floor; spec-0031's ruling that a stake left on the car would be deleted by the next
ride). Both are boxes, so the runtime lookup is a selector test on the corpse
(`@s[x=…,dx=…]`) rather than a search.

**What a region forbids an anchor is the region's own answer, and the two kinds
answer differently.** A runtime-mutable region acts on BLOCKS, so what it can
destroy is a marker in one of its own cells and cell containment is the whole
rule. A lethal volume acts on BODIES, through a vanilla selector the server
adjudicates against the body's whole hitbox — `@a[x=lo,dx=hi-lo,…]` covers
`[lo, hi + 1]` on each axis and matches on intersection, so it kills a
`metrics::PLAYER_WIDTH`-wide body whose feet cell is one outside the box.
`DeathRegion::holds_no_anchor` therefore refuses a lethal volume the shell of
cells `metrics::selector_reaches_body_in_cell` reports: one cell on every axis,
derived from the body and never chosen. An anchor inside that shell is a place
the delve invites the player to walk back to and then kills them for standing on,
and the bot ladder measured exactly that on the gallery — west pit
`[1,63,2]..[3,67,4]`, anchor `[1,65,5]`, three runs, three deaths at cell
`[3,65,5]` on the walk off it. The harness holds the same rule for the walk that
reaches the anchor (`volumeReachesCell`), so the two are written twice in two
languages and computed differently — a sweep of the cell's extent here, a clamp
to the nearest position there. Neither may drift: each side sweeps the same box
over the same 441-cell grid and states the same 175 reached
(`the_cell_rule_agrees_with_a_swept_body_box`,
`volumeReachesCell agrees with a swept body box, and with the compiler's count`),
so the agreement is a shared number rather than each doc comment asserting the
other's.

#### The three ways a stake can be pulled out from under itself

Two are `DW0526`'s, one is not, and the third is named rather than left silent.

| how | in scope? | why |
|---|---|---|
| **Runtime-mutable ground** — `close-gate`, `set-block`, `collapse`, a shortcut's or a timed gate's seal | yes | the case spec-0031's ruling was written for: a stake left on a lift car is deleted by the next ride. |
| **`fill-region` / `clear-region`** | yes, and it is *the same defect* | a `clear-region` deletes the block a marker stands on exactly as a departing car does. They enter through `QuestEffect::region_write` — the DSL's own answer to "which verbs rewrite a box" — so a later verb of that family is covered by existing rather than by being remembered. |
| **A `teleport`'s `from` box** | **no — a deliberate ruling; closed by `DW0545` one layer away** | a teleport moves *entities*, not blocks: the ground under the marker is untouched, and what moves is the marker itself, away from the position the collecting player's ledger recorded — after which `stk_gc` finds nobody holding a wager there and retires it, taking the wager with it. Different defect, different fix, and not one a box check on this axis could state — `DW0526` is about **footing**, and a marker's position is chosen at RUNTIME, so no compile-time geometry test knows where it will be. |

The teleport case cannot simply inherit the teleport's own `DW0542` either, and the
reason is the shape spec-0031 named when it refused to inherit `lethal_volumes[]`'s
exemption list into a verb that *moves* rather than *deletes*: `DW0542` tests the
affordance authority, which carries compile-time cells, and a stake has none to
offer it. Inheriting it would have produced a green that examined nothing.

**`compiler::stake` carries no teleport rule.** "The teleport exempts engine
machinery" would build a roster into one verb; "the stake ledger survives its
marker moving" would make the stake compensate for a selector that grabbed
something it should never have grabbed. The question is upstream of both —
*what does a region verb select?* — and a marker being a **place** is a property
of the marker, not of any verb. So the class is declared where the marker is
summoned and every box-narrowed selector reads it (`DW0545` above). That a
capability keyed to the object needs no cooperation from this module is the
point rather than a coincidence.

**Note the direction of the conservatism, because it is why this set is not
`Plan::region_events`.** The completability model deliberately drops a non-fill
write fired from an **optional** root — an optional firing may fill, never open —
because a route proof must not lean on a clear the party might never trigger. This
set needs the opposite: a `clear-region` in an `on_death` bundle the party may
never reach is still ground a stake must not stand on, because if they do reach it
the marker is gone. Same geometry, opposite direction, so the two lists cannot be
one.

**Two conservative simplifications, recorded rather than hidden.** spec-0032
already records one — *reachable under the quest state* stands in for *explored*,
which the engine does not track. The implementation adds a second: nothing
observable at runtime says which point of a respawn point's DAG span a death
happened at, so the reachable set used for a seat is the **intersection** over
every sealing configuration that can hold while that seat is in force. The anchor
is then reachable under all of them, which is strictly stronger than the rule as
written and needs no runtime discriminator for quest state at all. A campaign with
no `close-gate` has exactly one configuration and pays nothing.

The span swept is every critical-path arrival from the seat's own firing step to
`critical_path.len()` inclusive — **the arrivals past the last objective
included**, each of them carrying every objective on the path as fired (see *Every
arrival is keyed* above). A seat's configurations therefore hold the world as the
party leaves it at the end of the delve, which is the one a death after the last
beat respawns into.

#### What the runtime tiers can and cannot witness — stated, not implied

**A PackTest fake player is permanently undamageable and cannot die** (measured
twice, independently, on the pinned toolserver). So that tier cannot witness a
player death, and therefore cannot prove the edge from a death to a stake being
placed. Two templates are generated and both are honest about what they cover:
`v10_shop_purchase` drives an offer handler as its own dummy and proves the debit
and the refusal; `shop_enchanted_stack_<i>_<j>`, one per offer that hands over an
enchanted stack (spec-0075), empties its dummy, drives the offer's gate and each
stack's own `when` open as the buyer, probes the inventory before the purchase
(must read 0) and after (must read 1) for the item carrying exactly those
enchantments in the component the item writes them to; `v10_stake_<id>` drives `stk_drop_<id>` and the campaign's real
`stk_collect` and proves that the declared share leaves the purse, that a marker
really stands where the drop put it, that collecting returns **exactly** what was
taken, and that a second collection in the same breath returns nothing more; and
`v10_stake_two_datums` drives two forfeits from one position and proves that the
two leave **one** `minecraft:interaction` and one display, that one press returns
both datums, and that the place then retires. No template is generated for the
death edge itself — a template that bound to nothing and reported green is the
vacuity CLAUDE.md names, and it is worse than an absence because review cannot
see it.

**The death itself is a bot-tier proof.** spec-0032's acceptance criterion 9 —
*die in a lethal volume, respawn, walk back, collect, with the amount asserted* —
is carried by the harness's `death-loop` stage: it walks a real client into every
declared lethal volume, dies there, and asserts the volume's wording, the declared
forfeit, the stake's presence at the table's own anchor, the walk back, an exact
restore under a double right-click in one tick, the retirement of the collected
hardware, and the respawn seat — all against `validation/death-plan.json`, never
against the emission. It runs on every bot run whose build ships
`validation/death-plan.json` (`DELVEWRIGHT_DEATH_LOOP=0` skips it, and the report
records the skip); no CI job runs the bot ladder. Over the economy fixture:
`EULA=TRUE validation/bot-run.sh --project dw-death-loop --output
./delve-output-economy` over a build of `crates/delvec/tests/fixtures/economy`.

**The die-retry loop** — death → respawn at the governing checkpoint → walk back
→ re-engage — binds only where an ARMED checkpoint stands before a mandatory
encounter. `crates/delvec/tests/fixtures/die-retry` is the smallest campaign that
lets it bind, `crates/delvec/tests/die_retry_fixture.rs` holds it to that shape,
and the run report's `die_retry_binding` makes the zero loud on every other build.

**The first death is seeded.** `dw.death_seen` and `dw.death_ack` are `dummy`
objectives, so a player who has never died has no score in either — and `execute
if score @s A > @s B` with B unset does not fire (measured on the pinned 1.21.11
server; `scoreboard players add <e> <obj> 0` is what creates the entry at zero).
`cp_respawn_check` therefore seeds each acknowledgement it reads, ahead of the
comparison, so `on_death` and the checkpoint respawn dispatch fire on a player's
FIRST death; `v06_checkpoints` and `v10_on_death` assert the ORDER, not merely
the presence.

Binding (playtest-methodology rule 1): a campaign with a stake emits
`validation/stake-gate.json` — stakes declared, respawn seats and death regions the
table is keyed on (and how many of those regions are lethal volumes), quest-state
configurations enumerated, rows proved, distinct anchors resolved, runtime-mutable
cells excluded, stranded cells found, and how many of the declared stakes can
actually leave a marker. A campaign with no stake emits **no file
at all**, so a file that exists and reports zero is a finding rather than an
absence.

| Code | Meaning |
|------|---------|
| `DW0520` | **A stake that is not a personal wager.** A `stakes[]` entry's `state` names a datum the campaign never declares, or one declared `party`-scoped. Validation-tier (exit 1), `dsl::economy`. The scope half is stated as a rule because it is the multiplayer decision most likely to be made by accident: a shared purse turns a teammate's death into a penalty on everyone and nothing in the JSON would say so. Prescription: declare the datum `player`-scoped, or point the stake at one that is. |
| `DW0521` | **`drop-stake` names no declared stake.** Validation-tier (exit 1), `dsl::economy`. Prescription: declare it in `stakes[]`, or fix the id. |
| `DW0522` | **A stake nothing ever drops.** A declared stake that no `drop-stake` effect anywhere in the campaign leaves: its forfeit rule, its retention policy and its whole compile-time placement table describe a mechanism no beat can fire. Validation-tier (exit 1), `dsl::economy`. The vacuity rule `DW0502` states for a datum with no reader, applied to a whole feature. Prescription: drop it from a beat (`on_death` is the usual one), or delete the declaration. |
| `DW0523` | **A shop button that cannot answer.** A `shops[].offers[]` entry with no `effects` — drawn, pressable, inert — or a `shops[]` entry with no offers at all, which is worse than an empty shop: vanilla's 1.21.11 dialog codec rejects an empty action list at pack load. Validation-tier (exit 1), `dsl::economy`. A **refusal counts as an answer**, so an offer whose only effect is a `narrate` gated on `at-most <price − 1>` satisfies it — which is exactly the shape spec-0032 asks for. Prescription: give the offer effects, or delete it. |
| `DW0524` | **A forfeit above the whole purse.** A `forfeit` of kind `proportion` whose `percent` exceeds 100. Validation-tier (exit 1), `dsl::economy`. Prescription: 0–100, or `{"kind": "all"}`. |
