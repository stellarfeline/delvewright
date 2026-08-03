# spec-0022: Traps v2 — physical triggers, command-driven consequences

- **Status**: Draft (owner directive 2026-08-03, dictated during the
  drowned-bell playtest; supersedes spec-0011's redstone signal-path half —
  spec-0011's trigger-hardware layer and its completability proofs stand)
- **ADRs**: 0001 (compiler emits everything), 0003 (vanilla-first)

## The ruling

Redstone keeps exactly one job: **the trigger** — a pressure plate, a tripwire
hook, a trapped chest; something physical the player can see, suspect, and
learn to read. Everything downstream of the trigger — signal transmission and
the consequence itself — is **commands**: the compiler already owns a
detection tick and an effect vocabulary, so a trap's payload is authored like
any other effect bundle, not built out of dust and repeaters.

Why this is strictly better, in the owner's framing: a command payload is
*more* expressive (redstone cannot make a tripwire delete the ceiling and
drop gravel on your head; commands can) and *less* complex (no hidden wiring
to route through prefabs, no quasi-connectivity folklore, no piece-local
redstone budget). Expressiveness ceiling moves from "what dust can carry" to
"what the effect vocabulary can say". Presentation stays diegetic: a
command-spawned arrow materializing from a dark gallery slot reads as the
dungeon shooting you, not as a command running.

## Surface (stage 5 `traps[]`, revised)

A trap becomes `{id, trigger, payload}`:

- `trigger`: as spec-0011 — `pressure-plate` / `tripwire` / `trapped-chest` at
  a hardware anchor the prefab metadata declares. The compiler's detection is
  the same observer-free machinery as today. Disarm affordances unchanged.
- `payload`: an ordered effect list, the SAME effect vocabulary quests use,
  plus trap-payload verbs:
  - `volley {projectile, from_anchor, kill_zone, salvos?, interval?}` —
    command-summoned projectiles with real velocity vectors. **Saturation,
    not sniping** (owner ruling 2026-08-03): a volley must blanket its
    declared kill zone — every standable cell of the zone receives fire, and
    the pattern repeats for `salvos` rounds (default 3) at `interval` ticks
    (default 10) — so a player moving through the zone cannot dodge it by
    accident; escaping means LEAVING the zone, a decision, not a lucky
    strafe. One aimed shot at the triggering player's fire-time position is
    additionally included per salvo (punishes standing still), but coverage
    is the contract. Solves the stair-volley's "wrong height, can't hit"
    outright: trajectories are computed per target cell, not built from
    dispensers.
  - `collapse {region_anchor, falling_block?, then_floor?}` — delete the
    region's blocks and summon falling-block entities (gravel/sand/anvil) over
    the player's column: the buried-alive trap.
  - `damage-players` (existing, region-scoped variant), `play-sound`,
    `narrate`, `set-flag`, `spawn-wave` — already exist.

## Proofs

- Trigger hardware proofs carry over from spec-0011 unchanged (hardware
  present, disarm completability, DW0363 gating surface).
- `volley`: `from_anchor` must have line-of-sight to EVERY standable cell of
  the kill zone in the assembled world (the aim rays are checkable — same
  machinery as cutscene clear-eye), else error naming the uncovered cell.
  This is the compile-time form of "the gallery slot can actually hit a
  player anywhere on the stairs" — coverage is proven, not hoped.
- `collapse`: the region must sit above standable cells of the trigger's
  vicinity; the critical path must remain completable with the region
  collapsed (the post-trap world joins the completability model, like
  shortcut seals do).
- Payload effects run through the standard effect validation (flags, anchors,
  l10n inventory).

## Migration

The two shipped trap payload emitters (stair-volley's dispenser wiring, dart
gallery) are re-emitted as command payloads; the tileset pieces lose their
hidden redstone (dispenser blocks may stay as *scenery* where visible). No
DSL-breaking change: existing `traps[]` fields keep meaning; `payload`
replaces the fixed per-type effect wiring.

## Acceptance criteria

- [ ] `volley` PackTest: dummy on the trigger → projectiles cover every
      standable cell of the kill zone (entity count and positions asserted)
      across the declared salvos; a dummy anywhere in the zone takes damage,
      including one moved to a different zone cell between salvos.
- [ ] `volley` line-of-sight proof: a blocked gallery slot is a build error
      naming the uncovered cell.
- [ ] `collapse` PackTest: region blocks removed, falling blocks land, dummy
      takes suffocation/impact damage; completability proof with the region
      gone stays green.
- [ ] Byte-identity for campaigns without traps; the drowned-bell rebuilds
      with command payloads and its ladder stays green.
- [ ] compiler.md traps section rewritten; spec-0011 marked superseded (signal
      half) with a pointer here.
