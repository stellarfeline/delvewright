# spec-0032: Currency, trade, and the recovery stake

- **Status**: Draft
- **ADRs**: 0001 (the compiler emits everything), 0003 (vanilla-first)
- **Depends on**: spec-0031 (runtime state; the `on_death` effect root; the
  region, effect and teleport verbs)

## What was commissioned

A currency the player holds; a way to spend it; a loss of it on death; and one
chance to recover the loss before the next death takes it. Souls-shaped, and
therefore exactly the case where the mechanism must be separated from the genre:
the loop is a *mechanism*, and "souls" is one campaign's dressing of it.

## Currency is a ledger, not an item — and vanilla already decided this

The engine sets `gamerule keep_inventory true`. So an item currency does not
drop on death today, and cannot be made to drop *selectively*: `keepInventory`
is all-or-nothing, and switching it off would also drop the pre-provided class
kit in a map whose whole premise is zero grind.

The loss must therefore be engine bookkeeping regardless of representation, and
once it is, a score ledger is strictly better than items: not losable to lava or
void, not transferable between players by any player command, no inventory
pressure. Vanilla's scoreboard is 32-bit signed, world-level, and survives
death, logout and restart.

**Recorded because a default made this choice silently.** It is written down
here so a future session finds the reasoning rather than the outcome.

## Trade rides the machinery that already ships

The rest flow is already a shop in every structural respect: an interaction
entity, the player-interaction advancement that supplies the acting player, a
multi-action dialog, buttons that run `/trigger`, and tick dispatch. A shop is
that shape with different buttons, and its prices are numeric gates from
spec-0031.

**Villager `Offers` is excluded**, for three independent reasons, any one of
which is sufficient: a trade cost can never be a scoreboard value (item-for-item
only); right-click on a villager body is already allocated to dialogue; and the
data-driven trade registry post-dates the pinned 1.21.11.

Vanilla forces two presentation facts worth stating once: a dialog body cannot
render a live balance (no score / nbt / selector components), so the balance
goes on the action bar, which is already proven; and `/trigger` is the only
command a non-op player may run, so it is the sole player-to-datapack channel
and inherits its arm/disarm discipline.

## The recovery stake

### Form

Not a vanilla item drop. A **glowing interaction point, collected by
right-click**: an interaction entity for the hitbox, a glowing item display for
the rendering, and `data storage` as the authority — entities in unloaded
chunks are neither ticked nor selectable, storage is.

It also removes three failure modes an item entity has: despawn after 6000
ticks, destruction by lava or void, and pickup by another player.

It inherits the diagnostics that already govern this hardware class: the
invisible-affordance softlock check, and the rule that the function permitted to
retire the hardware must be named — otherwise a collected stake does not vanish.

### Placement — the rule

The stake is placed at the point **nearest the
death point** on the walkable route the player has available between their
respawn point and where they died.

Stated so the compiler can prove it:

> The stake anchor is the point, on the walkable path from the respawn point in
> force at the moment of death to the death point under the quest state in force
> at that moment, that minimises distance to the death point.

Each term has an existing owner: walkability is the reachability graph the
completability proof already builds; "under the quest state" is the same
DAG-indexed passability `close-gate` already established; and the respawn point
in force is engine state (`set-checkpoint`, `bonfire`).

It is therefore a compile-time table — (death region) × (respawn point) ×
(quest state) → anchor — and a runtime lookup. No runtime search, no
nondeterminism.

**The rule degenerates correctly.** For an ordinary death on walkable ground the
nearest point on the route *is* the death point, so no second rule is needed for
the ordinary case. For a death in a lethal volume — the case that motivated it —
the stake lands at the near lip of the hazard rather than inside it.

**One honest imprecision, recorded rather than hidden.** "Explored" is runtime
knowledge the engine does not track; the specification above uses *reachable
under the quest state*, which is a superset — a player may have unlocked an area
without walking it. The substitution is safe because the stake is confined to
the route between the respawn point and the death point, which is the way the
player must come back regardless. Honouring "explored" literally would require
recording visited cells at runtime, a mechanism of a different order, and it is
not being built.

**A stake anchor may never sit on a block that runtime can remove.** A stake
left on a lift car would be destroyed by the next ride. Compile-time check.

### Loss and retention policy

The forfeit rule is configurable, and the souls behaviour is one setting of it:
what is dropped (a currency, a proportion, nothing), how many stakes may exist
at once, and what a new death does to an existing one (replace, keep, drop
none). A creator who wants no death cost, or a permanent memorial at every death
site, configures it; they do not fork the engine.

### Scope

**Per-player, not party-shared** (stated explicitly rather than left to
emerge): the stake is a personal wager, and one
shared purse turns a teammate's death into a penalty on everyone. Recorded
explicitly because an existing test forces party scope on anything classified as
progression, which is the multiplayer decision most likely to be made by
accident. Whether another player may collect a stake that is not theirs is a
declared option, default no.

## Acceptance criteria

1. A campaign declares one or more currencies; each has a name that enters the
   l10n inventory, and a declared scope.
2. A price is expressed as the spec-0031 numeric gate, not as a shop-only field;
   a test asserts the shop reuses the shared gate struct and adds no comparison
   surface of its own.
3. A purchase that cannot be afforded is refused at runtime and says so, and a
   shop offering an item the campaign never defines is refused at compile time.
4. `on_death` carrying a currency forfeit produces exactly one stake, whose
   anchor equals the compile-time table's entry for that (death region, respawn
   point, quest state).
5. The stake anchor is walkable, is reachable from the respawn point in force,
   and sits on a block no runtime effect removes — each proved at compile time,
   each with a test that fails when the property is violated.
6. Collecting a stake restores the exact amount recorded, retires the hardware,
   and is idempotent under a double right-click in one tick.
7. A second death applies the declared retention policy, and every policy value
   is exercised by a test.
8. A campaign in which a stake could be placed with no walkable route back fails
   to compile, naming the death region and the quest state.
9. The full loop — die in a lethal volume, respawn, walk back, collect — is
   exercised by the bot tier on a fixture, with the amount asserted.
10. Every gate above states its binding count; a zero binding is a failure.

## Settled by live measurement (pinned 1.21.11)

The single load-bearing unknown in this spec resolves **favourably**, and the
question it was phrased as was the wrong one. Full derivation in spec-0031;
data in `docs/notes/death-and-teleport-spike.md`.

The engine's death edge is not an advancement — it is a `deathCount` scoreboard
criterion. Measured over 5 causes × 3 repeats, the edge is armed **on the
corpse, pre-respawn, for every cause including void, fall, drowning and lava**;
`LastDeathLocation` is written on the same tick; and the corpse's position is
the death position and is stable while the death screen is up.

So the recovery stake can be placed for every death cause a souls-shaped delve
produces, which is what this spec needed and could not assume. The compile-time
placement table stands unchanged — the measurement only confirms that the
runtime lookup has a death position to key on.

**One measured constraint the stake must respect.** A respawned player is
invulnerable for 59 ticks (~3 s), and `/kill` reports success while doing
nothing during that window. A retention policy that reacts to a *second* death
must not assume a death can be forced or observed inside those three seconds.
