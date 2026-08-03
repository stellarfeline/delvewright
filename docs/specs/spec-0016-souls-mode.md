# spec-0016 — Souls-mode mechanics (M4)

- **Status**: Draft (planner-personal, 2026-08-01; owner's souls direction
  from spec-0011/0012 closing notes; consumes the jump-arc spike
  (docs/notes/jump-arc-model.md) and the TD-routing spike (docs/notes/td-routing-spike.md, PR #147))
- **Vision** (owner-corrected 2026-08-02): death IS the teacher. 初见杀 —
  the un-telegraphed first-encounter kill — is a legitimate, essential
  souls beat: you die uninformed once, and the SECOND attempt is where
  the design pays off (positioning, luring, thrown items, routing
  around). The engine's job is not to sand the edges off — it is to make
  retry cheap and the world's answers consistent, so dying is always an
  investment, never a tax.

## Mechanics

### 1. Bonfires (rest points)
`set-checkpoint` gains sibling verb **`bonfire{anchor}`**: places a rest
interact at the checkpoint anchor (campfire prop affordance). Resting fires
**`on_rest[]`** hooks (superset of today's `on_respawn`): reset scene state —
re-arm `reset: rearm` traps, re-seat cleared waves (declared per wave:
`respawns_on_rest: true`), restore deferred-NPC/actor postures. Death =
respawn at last-rested bonfire with the same hooks. `on_respawn` remains for
non-bonfire checkpoints (island-style). Proofs: bonfire placement inherits
DW0316 (standable) + DW0315 (no stranding); a wave marked respawns_on_rest
on the critical path re-runs its completability proof post-rest.

### 2. Shortcut doors ("the door does not open from this side")
The owner's definition of the pattern (2026-08-02), which this section is
now built around: between two bonfires there are TWO routes — a **short
route** (fast, no enemies or hazards) that starts SEALED, and a **long
route** full of enemies and mechanisms. Reaching the far side the hard
way and interacting with a specific mechanism **permanently** opens the
short route. That loop-back moment is the point of the design.

DSL: **`shortcut{gate, unlock}`** — `gate` is sealed from world-load;
`unlock` is an interact anchor on the FAR side whose completion fires the
permanent open (plus its own on_unlock[] beat — the bar lifting, the
elevator descending). Proofs: (a) the unlock interact is reachable ONLY
via the long route while the gate is sealed (analyzer frontier proof —
otherwise the shortcut leaks); (b) post-unlock, the short route is
genuinely short: proven traversal time from the previous bonfire through
the opened gate is < the long route's (the loop must pay); (c) the gate
never re-seals (permanence is structural — no close-gate may target a
shortcut gate: compile error). `close-gate` behind the player (point of
no return) remains available as plain staging, unchanged by this spec.

### 3. Ambushes
Deferred NPC/actor + `approach`/`strike` trigger + `spawn-actor`/`unleash`
at corner/doorway anchors — all landed (v0.6). Missing sugar, added here:
**`ambush{at, actors[], trigger, telegraph?}`** — one declaration
expanding to the deferred spawns + trigger. `telegraph` is OPTIONAL
(owner ruling 2026-08-02): the un-telegraphed ambush — the shove off the
cliff you could not have known about — is core souls vocabulary; the
creator has full freedom. What the engine still owes the player is
COUNTERPLAY on the retry: the compiler proves every ambush is avoidable
or defeatable by an informed player (a route exists that survives it:
luring ground, a positioning line, or an exit — the existing
trap-avoidability machinery generalized), and determinism guarantees the
second attempt meets the same ambush in the same place.

### 4. Timing gates
`schedule`-driven oscillating gates: **`timed-gate{gate, open_ticks,
closed_ticks, phase?}`** emitting a deterministic clock over the gate region
(open-gate/close-gate alternation). Proof (owner ruling 2026-08-02): NOT
all-phase passability — a gate that punishes bad timing is the point.
The requirement: the set of entry phases from which a walking player
crosses the span before the gate closes covers **≥ 20% of the cycle**
(computed from the nav model's crossing time over the gate span). Below
20% the gate is a coin flip, not a timing read — compile error.

**Addendum — the portcullis crushes** (owner directive 2026-08-03). A
player inside the gate region at the closing tick **dies**. Optional
`crush: true` (default `false`, so pre-addendum campaigns stay
byte-identical); the closing edge deals lethal `damage` by command to
every player whose position intersects the region, before the fill. By
command and not by suffocation: vanilla's in-wall damage is slow,
gear-dependent and escapable, which makes a portcullis a suggestion. The
20% window proof above is what earns this — the window is provably fair,
so the penalty for misreading it may be absolute. Zero per-tick cost is
unchanged: the judgement rides the closing tick of the existing schedule
ping-pong.

**Addendum — affordance hardware** (drowned-bell playtest, 2026-08-02).
An `unlock` is not just a hitbox. The engine emitted shortcut unlocks,
trap disarms and bonfires as bare `minecraft:interaction` entities —
invisible — and left the visible lever to whatever the tileset happened
to carry. The drowned bell's did not, so the unlock cell was bare air;
the only visible thing there belonged to an unrelated `reach-anchor`
objective that killed its own marker on completion, and the delve
soft-locked. The compiler now owns every affordance's visibility
(`DW0420`) and forbids any machinery but the affordance's own
consumption from retiring it (`DW0421`).

### 5. Lethal parkour
Jump edges per the measured model (docs/notes/jump-arc-model.md): cardinal
jump edges, sprint-runway requirement, clearance 3/3/2, content envelope =
measured-max − 1 (flat gap ≤ 2, +1 rise gap ≤ 1). Area opt-in
`parkour: true`; two diagnostics (required-but-unjumpable; final-world jump
self-check). Fall-damage routes death to the bonfire; checkpoint-density
rule: ≤ 45 s of traversal between a lethal jump sequence and its bonfire.

### 6. TD lanes (routed-then-feral)
Mechanism (spike-proven live on 1.21.11, dossier
`docs/notes/td-routing-spike.md`): vanilla's **Raider patrol system** is
the intended primitive. Lane mobs spawn as a patrol squad
(`Patrolling:1b`, one leader, snake_case int-array `patrol_target`); a
compiler tick clock advances them through lane waypoints, and per mob a
player-proximity check releases `Patrolling:0b` — from that instant the
mob is a plain native hostile. Combat-preempts-routing is engine
semantics (the vanilla patrol goal is hard-gated on having no target):
the owner's rule — march while distant, fight with NATIVE AI once
aggroed, never brush past — falls out of the primitive, unforced.

DSL surface: **`wave.lane { waypoints[], aggro_radius }`**.
`aggro_radius` = release radius = the wave's `follow_range` attribute
(they MUST be equal: patrolling raiders hold ground against targets they
cannot engage). Compiler parameters (measured): waypoint spacing 12
(> the 10-block vanilla arrival re-roll), advance radius 8, re-assert
every 20–40 ticks (~1 ms MSPT for a squad), march ~1.8 blocks/s.

Diagnostics (error tier): lane species must be raider-family (pillager /
vindicator / evoker / ravager / witch — all verified marching); squad
size ≥ 2 (a lone patroller self-cancels); lane mobs must be armed
(unarmed pillagers deadlock on target acquisition — the wave `equipment`
field is mandatory for lanes); waypoints standable (existing cell
proofs) and spaced > 10. Emission trap pinned by the spike: the legacy
`PatrolTarget` compound key is silently dropped by 1.21.11 — only the
snake_case form routes.

**Non-raider waves: aggro-edge summoning** (owner design 2026-08-02).
Species without patrol AI never march a lane — instead
**`wave.summon: aggro-edge`** spawns them AT the boundary of their own
`follow_range` from the nearest player (or the defended anchor,
whichever is nearer), so they acquire a target the instant they exist
and close in under pure native AI — no routing, no brushing past.
Narrative frame: spirit-summoned (they materialize at the edge of
perception, which is exactly where the mechanic puts them). Proofs:
candidate spawn cells are precomputed standable cells (existing wave
machinery) restricted to the ring `follow_range ± tolerance` around the
defended anchor, inside the arena, with line-of-sight to the target
region — a wave whose ring has no valid cell is a compile error, not a
silent no-spawn (the round-1 kill-less-wave lesson).

### 7. Death-as-learning pacing rules (design contract, linted)
One warning-tier lint survives the owner's 2026-08-02 corrections: **retry
cost** — bonfire → point of failure ≤ 60 s traversal on the proven path
(dying must be an investment, not a commute). Struck by owner ruling: the
telegraph rule (初见杀 is core vocabulary — §3) and the escalation rule
(a powerful OPTIONAL enemy near the start — the Tree Sentinel pattern:
fight it or walk around it — is legitimate and our campaign should pay
homage; the compiler's only obligation is proving a route AROUND every
optional elite exists). New in its place: **optional-elite proof** — an
enemy not on the critical path must have a proven bypass route that a
player can walk without entering its aggro radius, or the "optional" is a
lie.

## Acceptance

1. Planner-built multi-level souls campaign using every mechanic above,
   full ladder green, owner-played.
2. **Skill-completeness prompt suite** (owner directive): planner-authored
   prompts the OWNER runs through /new-delve — per genre a one-line
   creative-freedom prompt AND a detailed fidelity brief with checkable
   criteria. Genres: tower defense; one-map siege (attacker + defender
   campaigns); stealth heist; horror escape; detective mystery; moving
   escort; time-attack parkour; boss rush; puzzle dungeon; horde survival;
   branching moral drama; negotiation adventure. The suite is Appendix A.

## Non-goals
Difficulty settings (a delve is tuned once); PvP; procedural difficulty
scaling; any runtime LLM.

## Appendix A — skill-completeness prompt suite (owner-run)

**Protocol.** The owner runs each prompt through `/new-delve` verbatim, with
no planner assistance during the run. Each genre has two prompts: a
**free** one-liner (tests creative competence — does the skill produce a
coherent delve of the genre from almost nothing?) and a **brief** (tests
fidelity — every criterion is checkable on the generated campaign). A genre
passes when the free run yields a full-ladder-green playable delve
recognizably of the genre, and the brief run additionally satisfies every
criterion. Failures are skill/toolchain findings, never prompt findings —
fix the skill, don't tune the prompt.

1. **Tower defense** — free: "Make me a tower-defense delve: monster waves
   march down fixed lanes toward a heart I defend."
   Brief: ≥3 lanes converging on one defended anchor; ≥5 waves, escalating
   composition + equipment; lane mobs march routed while distant and fight
   with native AI once aggroed (§6); between-wave build/rest phase with a
   visible timer; defeat = heart destroyed → checkpoint, victory = final
   wave cleared; defense resources issued by the story, never farmed.

2. **One-map siege** (Bannerlord-style, owner-named) — free: "One fortress,
   two campaigns: in one I storm it, in the other I hold it."
   Brief: one shared prefab set/world, two campaign DSLs on the SAME seed;
   attacker: breach → courtyard → keep with ≥2 proven breach routes;
   defender: hold 3 fallback lines against timed assault waves; mirrored
   cast (the commander you serve in one is the boss of the other); both
   campaigns full-ladder green independently.

3. **Stealth heist** — free: "A heist: get in, take the prize, get out —
   without being seen."
   Brief: patrol actors + dark zones with declared mitigation; caught =
   telegraphed consequence (death→checkpoint or alarm escalation, not
   nothing); the prize is a collect objective; ≥2 infiltration routes both
   analyzer-proven; zero mandatory combat.

4. **Horror escape** — free: "Something hunts me through this place; I
   can't kill it, only get out."
   Brief: an unkillable pursuer (unleashed actor / puppet handoff); ≥2
   hide beats; one-way doors sealing the route behind the player; darkness
   as a managed resource (declared dark areas + light the story issues);
   the ending is escape, no kill objective on the pursuer.

5. **Detective mystery** — free: "A murder on a small island; everyone has
   a story; I name the culprit."
   Brief: ≥5 interrogatable NPCs with flag-tracked branching dialogue; ≥4
   physical clue collects; the accusation dialogue gated on evidence
   flags; a wrong accusation has consequences but is never a soft-lock;
   two suspects stay viable until the final clue (red-herring provable).

6. **Moving escort** — free: "I escort someone precious across dangerous
   ground; they must arrive alive."
   Brief: escort walks a real route (move-npc/actor) across ≥2 areas; ≥2
   scripted ambushes on the route, both telegraphed (§3); escort waits at
   contested points until the player clears them; escort death →
   checkpoint; arrival cutscene (multi-shot).

7. **Time-attack parkour** — free: "A race against the clock across jumps
   that can kill me."
   Brief: parkour areas within the measured envelope (§5); visible
   schedule-driven timer; ≥1 timed-gate section (§4); falls route to a
   bonfire, never strand (DW0315); bonfire density per §7.

8. **Boss rush** — free: "No filler: a chain of boss arenas, each worse
   than the last."
   Brief: ≥3 arenas chained by one-way doors (§2); each boss = staged
   waves + actor set-piece with telegraphed patterns; bonfire between
   arenas; a pre-fight cutscene per boss; no trash mobs between arenas.

9. **Puzzle dungeon** — free: "A dungeon that attacks my mind, not my
   health bar."
   Brief: ≥4 distinct puzzle idioms (flag logic, item use, timing gate,
   interact ordering); no combat required; every puzzle's solution on the
   analyzer-proven path; gates actually seal (no walk-around brute force);
   an optional hint NPC.

10. **Horde survival** — free: "Hold this position; the night is long and
    they keep coming."
    Brief: one defensible position, ≥6 waves with rest phases between;
    wave equipment + composition escalation; final wave led by a named
    elite (actor); visible wave counter; defeat restarts the current wave
    at the bonfire (respawns_on_rest, §1).

11. **Branching moral drama** — free: "A story where my choices cost
    something and the ending is mine to answer for."
    Brief: ≥3 endings from accumulated choice flags; no ending marked
    correct; earlier choices echoed later via flag-gated dialogue
    variants; ≥1 irreversible choice; endings as fullscreen titles; l10n
    complete across all branches.

12. **Negotiation adventure** — free: "Three factions on the edge of war;
    my only weapon is talk."
    Brief: the central conflict resolvable entirely through dialogue; ≥3
    factions with stances tracked as flags; failed talks degrade to a
    harder path, never a soft-lock; a final summit where every earlier
    concession is referenced; the pacifist route analyzer-proven.

**Coverage.** Together the twelve stress every subsystem the skill can
reach: lanes/waves (1, 2, 10), shared-world multi-campaign (2), stealth +
consequence (3, 4), actor pursuit/escort/set-pieces (4, 6, 8, 10), deep
dialogue/flag machinery (5, 11, 12), souls mechanics §§1–7 (7, 8, 10),
puzzle/gate logic (9), endings + l10n breadth (11, 12). A subsystem no
genre exercises is a gap in this suite — extend the suite, don't shrink
the claim.
