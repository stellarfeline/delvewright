# TD routing spike — routed while distant, feral once aggroed (phase 1)

Live-verified 2026-08-01 on a throwaway pinned itzg 1.21.11 server (`dw-spike-td`,
rcon-driven, mineflayer probe player). Feeds the M4 tower-defense spec. All numbers
below are measured, not estimated. No engine code changed.

## Verdict

**Vanilla's Raider patrol system is the intended primitive**, and it is sufficient:
`Patrolling:1b` / `PatrolLeader:1b` / `patrol_target:[I;x,y,z]` NBT on raider-family
mobs, steered by a compiler-emitted tick function that walks the squad through lane
waypoints, and **released to fully native AI on player proximity**. The patrol goal is
hard-gated on `getTarget() == null` in vanilla, so "combat preempts routing" is engine
semantics, not our bolt-on — and the handover IS native aggro.

Mechanism (all commands are plain `execute`/`data merge`, emitted per wave):

1. **Spawn** wave mobs as a patrol squad: one `PatrolLeader:1b`, rest followers, all
   `Patrolling:1b` + `patrol_target` = first waypoint, **armed** (see gotchas), with
   `follow_range` attribute = the designed aggro radius.
2. **Tick function** (every 20–40 ticks), per mob:
   - `execute as <mob> at @s if entity @a[distance=..R] run data merge entity @s {Patrolling:0b}`
     — release: native targeting + combat own the mob entirely.
   - `execute as <mob> at @s unless entity @a[distance=..R] run data merge entity @s {Patrolling:1b,patrol_target:[I;X,Y,Z]}`
     — re-assert lane (also defeats vanilla's random re-roll on arrival and the
     lone-patroller self-cancel; inert during combat since the goal won't restart).
   - Advance the waypoint when the leader is within ~8 blocks of it.
3. **Aggro radius = `follow_range`** (existing wave `attributes` surface). It must
   equal the release radius R: a patrolling raider that targets a player it cannot
   engage *holds ground* (vanilla patrol behavior) instead of marching or charging.

## Measured parameters (recommended defaults)

| Parameter | Value | Evidence |
|---|---|---|
| Waypoint spacing | 12 blocks | must exceed vanilla's 10-block arrival re-roll radius |
| Advance radius | 8 | with 1.5 s re-assert: zero stalls over 6 waypoints |
| Re-assert period | 20–40 ticks | 1.5 s cycle used live; 2 cmds/mob/cycle |
| March speed | ~1.7–1.9 blocks/s | 72-block L-lane in 44–45 s, leader modifier 0.9 |
| Lane fidelity | leader mean 2.4, p90 4.5, max 5.5; followers mean ≤3.2, max 7.9 | drift vs lane polyline, 116 samples — reads as a loose warband, not glitching |
| Aggro radius (`follow_range`) | 16 tested | release→first hit 5.6 s (crossbow stance at 7.5 blocks, melee closed to contact), kill +6.7 s |
| Combat resume | ~3 s after kill | squad regrouped and completed the lane |
| Squad size | ≥2 always | a lone patroller sets `Patrolling:0b` itself (no companions within 16) |
| Server cost | 0.5–1.0 ms MSPT total | 4-mob squad + controller on flat world |

**Species coverage**: patrol NBT is raider-wide — pillager, vindicator, evoker,
ravager, witch all live-verified marching. The TD lane roster is the illager warband,
which is also the right fiction.

**1.21.11 gotchas (validation-critical, all live-confirmed)**
- The save key is **`patrol_target:[I;x,y,z]`** (snake_case int-array). The legacy
  `PatrolTarget:{X,Y,Z}` compound is **silently dropped** by the strict codec — the
  mob then patrols to vanilla-rolled random targets, which looks like working-but-drunk.
  `Patrolling`/`PatrolLeader` keep their camelCase names. Compiler must emit exactly this form.
- **Lane mobs must be armed** (compiler already arms wave species): an unarmed
  pillager that acquires a target deadlocks — patrol blocked by the target, no
  runnable attack goal — and freezes in place indefinitely.
- Spike servers without players freeze: 1.21.x `pause-when-empty-seconds` must be -1.
- Concurrent RCON sessions interleave responses cross-connection; sample sequentially.

## Rejected candidates (no-hack doctrine)

- **(a) Periodic `tp` nudges**: rejected as a hack — commandeers movement the engine
  intends to own. Measured anyway: 1.5-block snaps at 0.5 Hz read as stutter-sliding,
  net speed ~0.8 blocks/s (tp resets the pathfinder, *slower* than patrol), and it
  provides no native combat handover — aggro would need hand-rolled detection on top.
- **(a') Invisible temptation/breeding-item leaders**: rejected unmeasured — folklore
  workaround by definition, and raiders have no tempt goal anyway.
- **(b) Village/bell/raid attraction**: no arbitrary-point primitive — raid pathing
  needs an active raid; bed/bell interest is species-specific ambience. The patrol
  system *is* this family's general, exposed form.
- **(c) Leash to a routed puppet**: `leash:[I;x,y,z]` does round-trip on a hostile
  pillager in 1.21.11 (verified — knot at a fence), but there is no vanilla *moving*
  anchor (knots are block-fixed, displays can't hold leads), and tether physics
  fights the exact thing we want at handover — the mob must be cut loose to fight.
  Dragging hostiles on invisible leads reads as glitch. Rejected.

## Proposed DSL surface (for the M4 spec)

```jsonc
"waves": [{
  "id": "north-assault",
  "anchor": "area/gate/wave",            // spawn as today
  "mobs": [{ "entity": "pillager", "count": 4 }],
  "lane": {                               // NEW, optional — absent = today's behavior
    "waypoints": ["area/gate/wp1", "area/court/wp2", "area/keep/wp3"],
    "aggro_radius": 16                    // compiler sets follow_range AND release radius
  }
}]
```

Compiler obligations: lane species must be raider-family (diagnostic), squad ≥2
(diagnostic), auto-arm lane species, waypoints validated standable/reachable on the
assembled nav world (same machinery as wave seating), waypoint spacing >10
enforced by resampling, one emitted per-wave tick function implementing §Verdict.
Existing `attributes` surface keeps `follow_range` overridable per mob.
