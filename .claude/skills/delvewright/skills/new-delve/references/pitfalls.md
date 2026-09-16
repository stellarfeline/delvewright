# Reference: authoring pitfalls

## Contents

- [Difficulty](#difficulty)
- [What the machine proves about a fight](#what-the-machine-proves-about-a-fight)
- [Bonfires, flasks and potions](#bonfires-flasks-and-potions)
- [Where a bonfire may stand](#where-a-bonfire-may-stand)
- [Wave tuning](#wave-tuning)
- [Open air by default](#open-air-by-default)

## Difficulty

- **Difficulty is declarable** (`world.difficulty`: `easy`/`normal`/`hard`).
  Absent, the compiler derives `easy` for a wave campaign — which HALVES the
  damage players take (`min(dmg/2+1, dmg)`), the setting behind "the enemies are
  too weak". A souls-style brief almost certainly wants `normal` or `hard`; when
  you change it, retune the combat arithmetic (mob `attributes`, class gear, wave
  sizes) rather than only flipping the keyword. `peaceful` is rejected
  (`DW0468`) — it deletes every hostile. Scripted `actors` take the same
  `attributes` block wave mobs do, so an elite can be tuned on both its staged
  puppet and its unleashed twin.
## What the machine proves about a fight

- **The machine proves the LOOP, not the win.** Three things are checked about
  every mandatory encounter, and it is worth authoring toward them rather than
  discovering them as red builds:
  1. *Winnability arithmetic* (`DW0470`–`DW0473`): a required hostile must be
     damageable (Resistance amplifier 4 is total immunity — use at most 3, or put
     the durability in `attributes.max_health`), must have a standable cell beside
     it to be fought from, must fall inside the time-to-kill budget, and no
     `damage-players` in a quest bundle may land ≥ 20 (a full-health player) —
     that is a scripted death, not difficulty. A hit the party can dodge (a trap
     payload, a stealth `on_caught`, a `damage-players` with a `within` zone) is
     deliberately outside the check.
  2. **Declare `attributes.max_health` on every mandatory wave stack.** Vanilla
     publishes no per-entity default attributes, so an undeclared stack gets no
     numeric bound at all and the build warns `DW0475`.
  3. *The die-retry ladder stage*: the bot rests at every bonfire on the path (a
     fire only ARMS on arrival — the respawn point moves when the party RESTS),
     then deliberately dies twice at every encounter and proves respawn at the
     governing checkpoint → the route back → the encounter is still finishable →
     no completed objective was lost. Author with that in mind: every encounter
     needs a checkpoint or bonfire that governs it, and a wave the party must be
     able to re-fight wants `respawns_on_rest`. Leaving it off is legitimate — a
     won fight stays won, and the stage records that as `cleared-before-retry` and
     passes it. What it reds is `stranded`: nothing left to fight AND the
     objective unfinished, so the party can neither complete the encounter nor
     re-fight it. Turning `respawns_on_rest` ON buys a stricter check: the wave
     must come back WHOLE — declared count, all-new mobs, full health — because a
     retry must never let the party grind a fight down one swing per death.
  4. *The inverted floor gate*: mark a set-piece fight `tier: "elite"` or
     `"boss"` — on the **wave** or on the **actor**, same three keywords. The
     ladder then gives it one UNASSISTED bot attempt; if the bot — a poor fencer
     by design — wins cold, the run reports the fight as too easy for its billing.
     Leave ordinary pressure waves unmarked: they carry no such expectation.
     Marking is how you opt into the scrutiny, so mark honestly. **Mark the actor
     when the elite IS an actor** — the kneeling armoured thing that stands up
     when struck is a `spawn-actor` + `unleash-actor` beat, not a wave, and an
     unmarked one is a boss no proof ever looks at.
  5. *A tier the gate cannot measure is said out loud, not swallowed*: the gate
     warns on a first-try win and is silent otherwise, so an encounter nobody
     fought would look exactly like one that was fought and lost. The compiler
     therefore warns `DW0477` — and records `floor-gate: not covered (reason)` in
     `validation/combat-plan.json` — for a tiered actor no `unleash-actor` beat
     ever wakes (an `Invulnerable` puppet is scenery; a `vulnerable` one is `NoAI`
     and never swings back), and for a tiered wave no critical-path `kill`
     objective names. If you meant it as a fight, add the unleash or the `kill`
     objective; if you meant it as set dressing, drop the tier.
  Ordinary fights run the ladder under a bounded, logged combat assist, so bot
  fencing skill never caps how hard the delve is allowed to be.
## Bonfires, flasks and potions

- **Bonfires owe the party a flask.** Right-clicking a `bonfire` opens exactly two
  options — *rest and save* (full restore: health, hunger, negative effects
  cleared, flask refilled, checkpoint moved, `respawns_on_rest` waves re-seated,
  `on_rest[]` fired) and *save only* (the checkpoint, nothing else). The
  replenished item is a class-kit entry marked `"flask": true`, and **every class
  kit in a campaign that places a bonfire must declare one** — a bonfire campaign
  with a flaskless kit is the build error `DW0476`. Author it as a real recovery
  consumable with the per-rest budget you tuned against as its `count`: resting
  sets the stack back to exactly that number, up or down, so the flask is a budget
  and never a stockpile.

  **A potion must say what is in it.** A `minecraft:potion` (or splash/lingering
  potion, or tipped arrow) with no `contents` is vanilla's *Uncraftable Potion* —
  it heals nothing however you name it — so declaring one is the build error
  `DW0487`. Either name a vanilla brew or list the effects:

  ```json
  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": { "potion": "minecraft:strong_healing" } }

  { "item": "minecraft:potion", "count": 5, "name": "Ashen Flask", "flask": true,
    "contents": {
      "effects": [
        { "effect": "minecraft:instant_health", "amplifier": 1 },
        { "effect": "minecraft:regeneration", "duration": 200, "amplifier": 0 }
      ],
      "color": "#ff9c30"
    } }
  ```

  `potion` is a 1.21.11 potion id, where strength and duration are part of the id
  (`minecraft:strong_healing`, `minecraft:long_night_vision`) rather than separate
  fields. `duration` is in **ticks** (20 = one second) and is required for every
  lasting effect — and forbidden on the instantaneous ones
  (`instant_health`/`instant_damage`), which land once on drinking. `amplifier` is
  0 = level I. Anything vanilla cannot pour is `DW0486`. The bonfire's three
  dialog strings default to canonical English; author `prompt`/`rest_label`/
  `save_label` only when the fiction wants its own words, and keep the two labels
  button captions (`DW0331`).
## Where a bonfire may stand

- **Place a bonfire OUT of every hostile's reach.** A rest point is where the
  party respawns and where every `respawns_on_rest` wave is put back on its feet,
  so a fire inside a hostile's `follow_range` delivers the party straight into
  combat on arrival — the build error `DW0478`. The clearance is measured against
  where the force actually IS: a wave's seated spawn cells, and for a `lane` wave
  the whole marched polyline (a lane wave walks its corridor while you are
  elsewhere, so a fire beside the far end of a lane is a fire in the lane).
  Fighting actors — anything `unleash-actor`ed, or staged `vulnerable` — count
  too, at their staging anchor. Put fires in side rooms, past the threshold, or
  beyond the end of the lane; never buy the clearance by shrinking `follow_range`,
  which retunes the fight to hide the placement. A re-seated wave always comes
  back **stationed** — a lane wave at its lane start, a plain wave at its anchor —
  so the safe zone stays true across every rest and every death.
## Wave tuning

- **Wave tuning**: `follow_range` below ~16 means distant wave mobs never engage;
  a kill objective whose mobs idle is unfinishable-in-practice even though
  machine-valid. Undead waves burn in daylight — the ONLY sanctioned fix is a
  helmet on the mob: `equipment.head`, any head item, on every burning stack the
  party is asked to fight. **Never `set-time`**; the delve's hour is a pacing
  decision, and moving it to save a mob spends a beat. The compiler enforces this
  (`DW0496`): a species in vanilla's `#minecraft:burn_in_daylight` staged for a
  `kill`-adjudicated fight whose walkable ground reaches open sky, under a pinned
  clear daytime hour, with an empty head slot, is a build error naming the sunlit
  cell. Roofing the arena clears it too. One species the helmet does not save — a
  phantom burns through it — so an open-air phantom fight has to be roofed or
  restaged. Never route wave mobs like actors: waves are native AI; if a beat
  needs lane-then-fight movement, that is the routed-then-feral primitive, which
  does not exist — not a `follow_range` trick.
## Open air by default

- **Open-air by default**: stage scenes in the open unless a beat NEEDS enclosure
  (a cave passage, an interior puzzle, a reveal). The horizon — surround terrain,
  sky, backdrop — is part of the composition; a campaign of enclosed boxes wastes
  it. When an enclosed beat is necessary, prefer routing the player back into the
  open between beats over chaining interiors.
