# Playtest methodology — how an owner round is run

The owner's playtest hour is the scarcest resource in the pipeline. This document
records what 22 rounds on `nobodys-cave-island` actually taught about spending
it, derived from that campaign's findings ledger and round
records (content repo, `campaigns/nobodys-cave-island/GENERATION.md`).

Audience: agents. It governs every campaign iteration round; `/new-delve` carries
its mandatory steps inline.

## The measured record

Owner findings per playtest, by the round they were **first** reported in (the
ledger's `Reported` column; a re-report of an already-open finding is not counted
again):

| r3 | r4 | r5 | r7 | r8 | r11 | r12 | r13 | r14 | r15 | r16–r22 |
|----|----|----|----|----|-----|-----|-----|-----|-----|---------|
| 8  | 9  | 4  | 5  | 4  | 2   | 9   | 7   | 1   | 3   | **0**   |

Three eras, each ended by giving the machine a model of a dimension it previously
could not see — not by fixing more carefully:

1. **r3–r6, missing primitives.** Cutscenes could not aim; strikes did not
   register; text overran its box; night vision was a renamed water bottle; the
   ocean sat below the shoreline; singleplayer had no entry point. Ended by new
   DSL verbs and the diagnostics that guard them.
2. **r7–r11, no spatial model.** Walls, seams, a giant standing inside the
   mountain, sheep scattered across a cavern. Ended by geometry proofs
   (`DW0450`/`DW0451`/`DW0359`), the world-edit loop, and render review.
3. **r12–r15, no narrative-structure model.** Premise questions offered after the
   finale; beats armed before their prompt could be read; one ending for three
   names; a half-built branch. Ended by the cast ledger (r13, spec-0020),
   happenings (r18), branch points (r19), actor tiers (r20).

The r12/r13 spike is not a regression: it is the moment a *new dimension* became
visible to the owner, before the machine could see it at all.

Engine-first root-causing was NOT the differentiator — round 3 alone produced
eight engine PRs, and the "fix the twin, not just the instance the owner stood
on" lesson was already written down in round 9. Both were in place while the
churn continued.

## What the ladder asserts at a combat encounter

**The machine verifies mechanism. It does not fight, and it makes no claim about
whether a fight can be won.** Whether a delve is too hard or too easy is the
owner's hour: the gap between a skilled and an unskilled Minecraft fighter is
design material, and a bot's fencing is not a measurement of it. Every earlier
attempt to have the ladder hold an opinion here produced advisories that read as
mechanism defects and were not.

A `kill` step does three things, in order, and only the first is a measurement.

1. **The muster.** `wave_muster_<wave>` is called and every live body of the wave
   states its own `max_health`, `armor`, `armor_toughness`, `movement_speed`,
   `attack_damage` and `follow_range`, plus a bitmask of the identity facts that
   hold on it — its `CustomName` component and each declared `equipment.<slot>`
   item, answered by the SERVER through `execute if data`. A declared attribute is
   read with `attribute … base get`, because `attribute … get` is the total after
   a weapon's modifier and vanilla's own random spawn bonus. The harness compares
   the multiset of what it read against the multiset the plan declares. A
   declaration the bodies CONTRADICT fails the `critical-path` stage. A muster
   that finds nothing standing at the step's open is provisional: a wave an
   approach trigger seats is not there until the party walks in, so the step walks
   its route to the anchor, as a player does, and reads again (a body that comes
   to the bot on the way is read by the damage handlers first). Nothing standing
   at the anchor fails the stage as one zero-binding red: a declared wave whose
   bodies nobody read ships with every declared fact unchecked.
2. **The staged clear.** `wave_strike_<wave>` fells one body per call until the
   census says nothing of the wave stands. This is staging and the run artifact
   says so: every removal is in `staged_removals` with its reason. The blow is
   `player_attack by @p` and never `kill`, because `on_kill`, the wave countdown
   and a declared drop all pay on a PLAYER's kill.
3. **The wiring the kill drives**, unchanged: the objective completing, `on_kill`
   paying, the declared drops dropping, the health bar, the re-seat, and the walk
   to the anchor that proves the route.

spec-0023 §1's **die-retry** stage is the load-bearing bot proof: scripted deaths, respawn at the governing checkpoint, a walkable route
back, an encounter that re-engages, no progression lost. It runs between the
reading and the clear. A die-retry stage that cannot finish reds on its own
coverage and does not end the run: suppressing every measurement behind one fight
is how a ladder learns least from a red.

**A walk off the path meets a re-seated wave before it starts.** The death loop's
approach to a volume and its walk back to the stake are no legs the compiler
measured, and the deaths before each (the die-retry stage's, then the trial's own)
have put back every `respawns_on_rest` wave. Before either walks, every such wave this
run cleared in an earlier seating than the one in force, nearest the bot first and
with the anchor's chunk held, is asked of its census; one something of which
stands is read (1) and cleared (2) by the same code the kill step runs. The anchor
walk (3) is the kill step's and is not taken. A death on that walk outside the
volume's reach is the approach's failure (`approach_failure`), never a verdict on
the volume; a death on the walk back is `walk_back_failure`, never a verdict on
where the stake was placed.

**Outside a scripted death, a body never kills the bot.** A hostile that lands a
hit the server attributes to it is an enemy and is removed by a staged blow. What
the run must read before it removes the body is decided by which wave the body is
of, and that is asked of the server — each candidate wave's census, by tag, matched
to where the body stands — never guessed from a radius around an anchor:

- a body of a wave whose current seating the run has not read (its step is ahead,
  or a rest or respawn put a cleared wave back for a run-back) is read where it
  stands — the muster's facts do not depend on position — and then removed; the
  step's own muster does not read that seating a second time, because the second
  reading would count the run's removal as a body the server never seated;
- a body of the wave the die-retry stage is proving is removed when that wave
  re-seats on respawn: the next scripted death brings it back whole, the fidelity
  verdict is read at that landing, and a body the party fells after the landing is
  counted (by the census's credit) as the fight re-engaging — except the LAST body
  standing (by the census at the hit), which is left standing: removing it clears
  the encounter, and the clear fires the objective's completion, which can act on
  the anchor while the next re-seat lands and be read as a wounded re-seat;
- a body of a wave that does NOT re-seat is left standing while the die-retry stage
  proves it — it persists across both lives and is the fight the next life must
  find. These two holds are the only places a body can still kill the bot outside
  a scripted death, and the log says so each time it hits.

A removed body's blows are refunded: the health the server named that body as
taking is given back with instant health, rounded UP to vanilla's units
(4 × 2^amplifier) — at most 3.9 points more than the body took, which is the
whole of what a refund can hide — and named in `staged_removals`. Route falls and lethal volumes are
mechanism, not bodies: nothing they take is refunded, and they still kill the bot.

A body the server has already announced dead is not standing and is never struck: what hit the bot is a shot it loosed as it fell, and `/damage` on a dying body is refused (`Target is invulnerable to the given damage type`). Its blow is still refunded. A blow refused because the body died between the hit and the blow is named in `staged_removals` as `the body died before the blow landed`.

Every command whose refusal the harness reads is sent between two `/tellraw @s` markers written in one synchronous turn (`harness/src/command-reply.ts`). The server answers one player's commands in arrival order, so the lines between a command's markers are its reply and never the reply of another command in flight. A reply whose closing marker does not arrive within 5 s is reported as unobserved, never as accepted.

On a walk leg the bot is held at full health: it is restored on entry to every
walk and after every drop while the walk is in progress, whatever dealt the drop,
by one instant-health effect that covers the whole deficit, named in
`staged_removals`. A hostile's first blow lands before its body can be removed and
lands on whatever health the bot has, and a per-body refund only gives back what
it could attribute. Whether
the bot survives a walk is not what a walk leg is for. Off a walk the refund rule
above applies; on one no refund is added. A single blow of 20 or more, a lethal
volume and a crush gate still kill.
A scripted death waits until the respawned bot has sent `player_loaded`, which
ends the server's client-load window (at most 60 ticks, counted on the server's
clock, for a bot that cannot be asked), and is taken at the fight: a stage that recovers from an
unscripted death walks back before it scripts the next one. A walk back that ends
in a death is reported as a death on the leg, never as unwalkable geometry.

**The binding count.** `encounters[].declared_facts` is how many declared facts
the muster put a question to, stated per encounter even when the muster never
ran; `muster_findings` is what it could not establish. Zero declared facts over a
campaign with waves is an unbound probe, and the run says so before it starts.

## What the ladder asserts at a climb

**The bot climbs the climb the compiler proved, and nothing else** (spec-0099).
A leg whose proven route climbs carries `climbs[]` in
`validation/critical-path-waypoints.json`: where the body takes hold, where it
lets go, the column it holds in, the block and a ladder's facing. The hop between
the two ends is driven by the climb executor (`harness/src/executor/climb.ts`),
never handed to the pathfinder: into the column's centre; then, going up, push
toward the block the ladder hangs on holding jump until the feet clear the let-go
floor, or, going down, hold nothing and slide; then step onto the let-go cell,
inside a budget of six seconds plus one a block of height. mineflayer-pathfinder
2.4.5 walks every other hop. It can plan a ladder itself, and on a walk
that matches no proven leg it does — which is why a climb the bot finished is not
evidence that the climb was driven.

**A walk along a ladder's face.** The route model walks through a ladder cell on
the floor beneath it, as vanilla lets a body do. The pathfinder aims a node in a
ladder cell at the top of the panel, a block above the feet, and a body that does
not push into the panel never rises to it, so a walk along the face to a climb's
foot stops at the cell's edge. The harness re-aims every such node at the feet on
each `path_update` — a node entered level and left level or lower; one the body
climbs into or on from keeps the library's aim — and prints
`[ladder] N ladder node(s) walked through on the floor, aimed at the feet: <cells>`
(`holdWalkedClimbableNodes`, `harness/src/movement.ts`; proven on the real
pathfinder over the client physics in `harness/test/ladder-approach.test.ts`).

**The binding count.** Every walk whose leg carries a climb prints
`[climb] <walk>: N climb(s) on the proven leg, M driven as a climb hop`, and each
driven hop prints where it let go. `N > M` says a climb was walked by the
pathfinder, unproven; a run on a campaign with a climb that prints no such line
walked it with no proven leg at all, and the line's absence is the finding.

**A body that watches** (spec-0101 §5.4). The critical path carries
`watchers[]`: each watching body's selector, where it can stand (its summon point
and every walk's end), whom it watches and how far it sees. The compiler draws a
watcher from the dense route cells, and the exported leg keeps only their corners,
so before a proven leg is walked the bot adds a stop at the route's nearest proven
cell to every stand no corner is within reach of, when that cell is
(`watchStops`, over the shared route rule `harness/src/route.ts`; a climb hop and a
hop through a timed gate are never split), and prints
`[watch] <walk>: N stop(s) inserted on the proven route; it passes within reach of
<ids>`. At every waypoint and stop of the proven path where some stand is within
reach, the bot stops, lets the game run
its watch line, and reads in one bracketed turn its own position and each body's
live position as the server holds them (a body walking, removed or not yet
summoned reads empty and is not judged) and whether it wears the class a class
watch names. For each body it can draw that stands within `within − 0.25` of it
and at least one block away horizontally, it asks the server whether
`@e[<body>,tag=dw_watch,y_rotation=<yaw±2>]` matches, `yaw` the game's own bearing
from the body's feet to the bot's (`judgeWatchers`, `harness/src/executor/watch.ts`;
the arithmetic in `harness/src/watch.ts`). A denial fails the critical-path stage
with the body's yaw read beside the bearing; it is recorded, never thrown into the
walk, so the walk's recovery never reads it as a stalled hop. Only the critical
path's walks judge — the death loop's walks are not the proven path. The binding
line is printed at load and after the path:
`[watch] W watcher(s) in the record, K within reach on the proven path, K asserted
(J judgement(s)); never within reach: <ids>`, and the run report carries it as
`watch_binding` when the record holds a watcher. A watcher the path never comes
within reach of is reported, never failed; on the gallery the `gallery bot` job
requires `K ≥ 1`.

**What it cannot climb.** The bot's client physics (prismarine-physics) climbs
`ladder` and `vine` and nothing else. A proven climb on a weeping, twisting or
cave vine is refused by name at the hop — a harness gap, not a route defect — and
is the owner's hour until the physics carries it.

## What the ladder asserts at a sculk device

**The bot hears what the compiler predicted, and never a warden** (spec-0100).
A leg whose proven route sets a sculk sensor off carries `vibrations[]` in
`validation/critical-path-waypoints.json`: each sensor's cell and the shriekers
that answer it, predicted by the compiler (four route cells within `distSqr ≤
49`, no sneak, no dampening, no occluder; a shrieker within `distSqr ≤ 64` of its
sensor). The harness predicts nothing: the parse refuses a sensor more than 8
blocks from its leg's route — the polyline through the leg's waypoints, on which
every route cell the compiler predicted from lies, so a straight leg exported as
two waypoints is measured along its length (`harness/src/route.ts`, the one route
rule the watch stops and the pulse stations also read) — and a shrieker more than
8 from its sensor (`harness/src/waypoints.ts`). For each predicted sensor it waits for a
`blockUpdate` at the sensor's cell whose new state has `sculk_sensor_phase=active`;
for each predicted shrieker a `world_event` packet with id 3007 at its cell —
from the leg's start to three seconds after its end (`harness/src/sculk.ts`). The
leg starts when the harness first walks any part of it: a run-back approach or a
pulse station walk along the leg's proven cells, run before the step's own walk,
opens the window, so a sensor between the leg's start and a station clicks
inside it. A
device the previous leg set off is deaf for its busy span — a sensor's active
and cooldown ticks (2 s), a shrieker's `shrieking` (4.5 s) — so an event within
that span before the leg's start answers for it. A predicted event not heard
fails the step.

**The binding count.** Every walk whose leg carries vibrations prints
`[sculk] <walk>: N sensor(s) predicted, N heard; M shrieker(s) predicted, M heard`.
At the end of every run, passed or failed, `[sculk] darkness D, warden W`
counts every `darkness` effect on the bot and every `warden` that spawned; either
non-zero fails the run, because a shrieker with `can_summon=false` applies no
darkness and summons nothing. The build's own `sculk walk:` line says how many of
the world's sensors and shriekers some leg predicts; a device off the walk is
stated there, never asserted.

## Rule 1 — a green gate that binds to nothing must report VACUOUS, not pass

Most of the early "green" was vacuous. Three distinct ways this happens, all
observed on the island:

- **Unbound** — the gate ran and matched zero objects. The island's combat
  coverage ledger examined zero enemies for nineteen rounds and was green every
  time, because every list it printed was empty at once and nothing stated a
  count. The per-wave **muster** states `declared_facts` per encounter for
  exactly this reason.
- **Unfenced** — the campaign's `dsl_version` had not reached the surface the gate
  keys off, so the whole proof was inert. Branch reachability, the chronicle and
  the six branch proofs did not exist for this campaign until round 19 declared
  `branch_points`. "All four branch runs green" was physically impossible before
  then.
- **Unemitted** — declared, compiled green, and never emitted. `wave/storm-shore`
  and `wave/storm-fire` silently never spawned until a wave-machinery emission
  gap in the engine was closed; the round-22 bot run was the first time that fight
  existed at all, for machine or human. Guarded now by the dangling-function
  check (`DW0497`: no emitted `function <ns>:<name>` may point at a function that
  was never emitted).

**Obligation.** Every validation artifact states its binding count, and a zero
binding is a finding. Reading a report is not enough — an empty coverage set and
a clean coverage set look identical to a reader who is not counting. When a
campaign has waves whose stacks declare nothing, the muster is unbound; say so in
the round summary rather than reporting a pass.

**Which zero it is decides what the finding says, and only one of them blocks.**
A check binds where the object is. The zero above is the one that blocks: the
objects are THERE — hostile bodies, dialogue nodes, a flask — and the check is
inert over them, so the class is present in this build and nothing on it would
catch that defect a second time. The other zero is a campaign that declares
none of the class's objects at all, and that is not a hole in the engine's
coverage: the surface is optional and using it not at all is a design choice.
A surface the DSL *requires* cannot be absent from a build that compiled, so
every zero of the second kind is an optional surface. Refusing on it would make
a gate whose pass condition is "resemble the campaigns we happened to test" —
unsatisfiable by a creator's first delve, forever, for reasons about other
people's campaigns. So it is counted, named per class in the round summary, and
it does not block. Which of the two a zero is is always a MEASUREMENT (rule 7),
never an assumption in either direction, and the measurement is anchored in the
campaign SOURCE, which no emission defect can rewrite.

**The mirror image, read the same way.** A run can go RED for a reason that is not a
verdict on the delve either. When the harness itself dies, the run report carries
`harness_crash: {stage, reason}` and the bot exits 4 — distinct from a failed step
(1) and a bot death (3). `harness_crash` is stated as `null` on every run that
reached a verdict, so a non-null value says in one field that no stage beside it
decides anything about the content. A round reporting a red stage says which of
the two it was; a crashed harness is a finding about the harness, and the delve is
unmeasured rather than failed.

The server can die the same way. A server out of Java heap logs
`java.lang.OutOfMemoryError`, loses the structure templates it was loading, and
either goes quiet or takes the bot down before it spawns. The ladders name it:
`validation/packtest-run.sh` stops at the first OOM line with exit 125 and at its
`--timeout` with exit 124, neither of them a test count; `validation/bot-run.sh`
and `validation/branch-runs.sh` read the server log after a red run and say OOM
instead of the bot's socket error. Every server gets the heap ceiling in
`versions.toml` `[server].heap_max` unless its operator sets one
(`tools/tests/test_server_heap.py` enumerates the entry points). An OOM red is a
finding about the infrastructure, and the delve is unmeasured rather than failed.

## Rule 2 — a finding is not closed until its general form is a diagnostic

An instance fix leaves every other instance of the same defect in the build,
waiting for the owner to hit one. Measured latency on the island:

- The owner reported clicks landing on the wrong entity at the fire pit in **r7**.
  Fixed in **r10** by moving one anchor and adding a `strike-npc` trigger — the
  instance.
- The general rule became `DW0489` ("the crosshair is a ray") **eleven rounds
  later**. On its first run against the real build it immediately
  found a second instance: Antiphos at the cave mouth, separation `0.00` — which
  the owner had by then independently lost a click to.
- Same shape for `DW0205`: its test table's first row is the owner's
  muster/surf softlock verbatim, and on arrival it found **three** live instances
  in the campaign.

**Obligation.** Every owner finding produces two deliverables: the instance fix,
and the general form as a diagnostic (or a declared, justified reason none is
possible). When the diagnostic lands it is **re-run against the current build** —
that sweep is the point, not the code. A finding closed with only an instance fix
is recorded as such, and that record is a risk item at the next staging review.

## Rule 3 — declare the machine-readable structure before authoring content

The four declaration surfaces that let the machine see the story arrived at
rounds 13, 18, 19 and 20 of a 22-round campaign. Everything they proved was
unprovable before them.

**Obligation.** A new campaign declares the cast ledger, happenings, branch
points and actor/wave tiers as it authors each stage — never as a later adoption
round. `/new-delve` requires the first three outright, and the compiler fails a
build that omits one (`DW0460` / `DW0481` / `DW0480`). The fourth it can only
**ask** for: no diagnostic demands a tier, so an untiered set-piece fight
compiles green and rule 1's unbound-gate report is its whole backstop. A campaign
missing one is not a campaign whose gates mean anything.

## Rule 4 — a capability-gap finding blocks staging, not just the backlog

Every island finding that stayed open across more than one round was blocked on a
missing first-class primitive. None was a forgotten task:

| Finding | Reported → closed | Blocked on |
|---|---|---|
| Cheese must fill the room's OWN barrel, and be named | r12 → r18 | `collect` had no `container`/`item_name`/`fill_count`; the compiler stamped its own chest |
| Boulder hint should answer right-click too | r12 → engine change | co-located click triggers had to merge onto one hitbox |
| Wait branch: a body vanishes and walks back | r15 → engine change | a walk must start where ITS branch left the body (`DW0488`) |
| Ending night-vision expires and flickers | r15 → engine change | granted sight must outlast the camera it has to survive |

Refusing to hack these downstream was correct (CLAUDE.md, *No hacks at any
layer*); round 13 explicitly STOPped on two of them rather than shipping a
workaround. The mistake was staging builds for the owner while those rows were
still open — which is how she saw the same defect twice and, in round 16, said so.

**Obligation.** Triage every finding as *content* / *capability gap* on the day it
is reported. A capability gap is a **staging blocker**: either the engine work
lands before the next playtest, or the round summary tells the owner, per item,
that it is still open and not to test it. The full findings ledger is audited from
round 1 — never from the last round — before any build is staged.

## Rule 5 — the design record is authoritative, and unrequested changes are rejected

`DESIGN.md` went unupdated from round 3 to round 11 while the implementation
drifted; the round-12 audit that made it v2 found **seven deviations traceable to
no owner request at all**. The protocol adopted then — every round updates it,
every round ends with a conformance review, unrequested changes are forbidden —
was first enforced in round 21, when a worker's entire round was **rejected
wholesale and never merged** for carrying unrequested extras.

**Obligation.** Unrequested change is a rejection cause on its own, independent of
whether the change is good. Re-do the round from the sanctioned mechanisms.

## Rule 6 — execute a ruling as stated; generalizing it is a proposal

Round 16 read a ruling on one beat as a campaign-wide 3–4 second ceiling and set
the blinding beat to 4 s. The correction: that beat gets **no pause at all** — the giant standing up blind *is* the signal. The ceiling governs
places where a reading pause exists; it was never a target.

**Obligation.** Apply a ruling at the scope it was given. If a wider rule seems
right, propose it in one line and wait — a generalization is a design decision,
not an inference to make silently.

## Rule 7 — the ledger is a machine-readable artifact, and a gate reads it

Rules 2 and 4 were written down and obeyed by hand, which meant they were obeyed
exactly as well as whoever remembered them. Both are now enforced:
`docs/playtest-findings.json` is the ledger — **every finding reported on any
campaign** — and
`tools/creator/staging-gate.py` refuses to stage a build while any row's
general form is not a live, binding check on THAT build.

The gate asks a question no other check in this repo asks, and it is the reason
a green ladder does not discharge the standing rule that **a playtest is content
QC only**. Most island findings were things **no check
existed for at the time**, so "everything is green" and "she will not find a
mechanical bug" are different claims and only the first was ever measurable.
The gate re-runs nothing. Per row it asks: does a general-form check exist, and
does it BIND — non-zero — here.

**Four reds, one per way a green has really lied**, because folding them together
would be the fifth. Each says the same thing in a different voice: an object of
the class is HERE and nothing this build carries would catch the defect again.

| Verdict | What it means | Its real instance |
|---|---|---|
| `NO-GENERAL-FORM` | the instance was fixed, the class never built | rule 2's `DW0489`, eleven rounds late |
| `MISSING-CHECK` | the ledger names a check this engine no longer has (absent from source, undocumented, or asserted by no test), or a stage document the COMPILER read that this gate holds no parsed copy of | four rows in the ledger's own first run named invariants that did not exist under those names |
| `UNBOUND` | the check matched zero objects, and objects that could have carried the defect are there — or nobody has measured whether they are | rule 1's coverage ledger, nineteen rounds |
| `NO-SOURCE` | the campaign has no stage JSON, so nothing can be measured | a campaign directory holding design records and no stage documents |

**The refusal is on presence, not on absence.** `INAPPLICABLE` — zero binding
**and** a MEASURED zero precondition, so the campaign declares none of the
objects the class needs — is counted, never refused. The island has no trap, so
no volley-saturation proof can say anything about it, and holding the build
until someone authors a trap is not coverage work. Absence of an OPTIONAL
surface is a design choice; a surface the DSL requires cannot be absent from a
build that compiled, so every zero the gate can see is an optional one. What the
gate still refuses is the mirror: the class is present and no check binds to it.

This is what makes it a general engine's gate rather than one campaign's. The
ledger records defects found on particular campaigns, most from one — so a creator's first delve contains almost none of
those object classes. A pass condition of "resemble the campaigns we happened to
test" would hold every new delve unstageable forever.

Counted is not folded away. The count is in the gate's headline in its own
words, the rows are listed in their own section, their ids go into the admission
token, and the verifier announces the number at boot: *this session cannot meet
these classes.* A pass is admission, never coverage, and rule 4's obligation is
kept by naming them in the round summary class by class. `--strict` does not add
them either — that flag is the floor for rows a DECLARATION excused, and nothing
declared this one.

**Which zero it is, is always a measurement.** Two things can measure it. A
declared `applies_when` probe, for a binding that counts a DECLARATION inside
carriers that may exist anyway (a `has`/`has_any` predicate, a `contains`
glob, an `artifact` or `out` probe over derived output) — that zero is
genuinely ambiguous, it is the unbound ledger's shape, and a row of that shape
owes a probe. Or the binding's OWN SHAPE, where the probe counts the object
class itself and nothing stands one step behind it: a `dsl` predicate
selecting by identity (`eq`/`in`/`prefix`, with `not_in` excluding values, or an `any_of` every arm of which is
one of those) across the declared design,
and a `campaign` glob with no `contains`, where the file IS the object. Such a
row may not declare an `applies_when` at all — it could only name its own
binding, which the ledger loader refuses outright — so the gate reads the
shape on every subject and reports the measured `INAPPLICABLE` rather than
`UNBOUND`, whose whole content is that nobody looked.

**A quantifier that names a population the object cannot belong to is not a
check weakened, it is a check made true.** A precondition for "mandatory
combat" that counts every `spawn-wave`/`spawn-actor` effect and every `actor/`
id counts populations the general form does not name: a guided tour whose only
bodies are invulnerable puppets that walk out of a gate and are removed is
adjudicated as a campaign owing a combat plan, and refused for shipping none,
while `compiler::emit` is right not to write one. The precondition is the class the
row's own words name: mandatory combat is a `kill` objective or a body turned
loose (`combat::mandatory_fights`). A general form can name a population the
DSL spells more than one way — an `unleash-actor` effect and the `ambushes[]`
entry the compiler expands into one are members of a single class — so the
predicate language carries `any_of`, a disjunction of predicates, and a checker
reads a document the way its consumer reads it. A narrowing is proved in both
directions on one campaign in two states varying one variable: with one
`unleash-actor` beat added to the ceremony and the build held fixed, the row
reds `MISSING-CHECK` again.

The precondition counts the objects that can carry the DEFECT, never the
objects that can carry the DECLARATION. An `interact` with no `requires_item`
completes on any click, so it carries no item-gate defect: the item-gate rows
count the held-item tests the build emits, the server's own adjudication of an
item gate, beside a binding counted over the source, and a held-item test the
source binding does not count reds `UNBOUND`. A cast ledger is keyed by NPC id,
so a campaign with no NPC carries no cast-ledger defect: the cast rows count
NPCs. A demo level is a campaign and is judged by its own objects like any
other; one that skipped the design gate is refused by the design-gate rows
exactly as a campaign is.

**And at least one of the two counts is taken over the campaign SOURCE.** That
is the property the non-refusal is secured by, and it is one the defect cannot
supply: a zero counted in the build tree is exactly what an emission defect
manufactures — stop writing the ledger and the class "disappears" from a build
whose campaign still declares it. A `dsl` predicate reads the stage documents
the author wrote and a `campaign` glob reads the campaign directory; only the
author can move either number, and moving it is the design choice the whole rule
is about. A double zero counted only in derived output stays `UNBOUND` and
refuses. No live ledger row is of that shape; the demand binds against the
row nobody has written yet, and both directions are driven from fixtures.

**The first non-red a row can DECLARE** is rule 2's own escape, no wider: a row may close
`DECLARED-UNCOVERABLE` with a `disposition` (`no-machine-form` / `not-a-defect`)
**and** a substantive justification. The island rows that qualify are every one
a judgement — prose register, pacing, whether a space reads as open. A bare
label buys nothing; the gate checks the justification is there and says
something. Their count is in the headline because rule 4 makes each a standing
risk item at that staging review.

**A pre-detail blockout is not a subject at all.** A campaign is staged only
once detailed: the first time a player meets it is its finished first version,
so a site-plan campaign whose only geometry is the derived massing (spec-0049)
is never handed to one. The gate refuses such a subject before any row is
adjudicated, with or without `--strict`, and `--stage-anyway` does not reach
the refusal, because the override admits a red list and this is not one. The
subject is a blockout when EITHER instrument says so — the campaign places by
site plan (`site-plan.json`; DW0839 makes the placement authorities exclusive)
with no `detail-plan.json`, or the build's **compiler-written manifest** lists
the site plan among its inputs and no detail plan — so a build compiled before
the campaign was detailed is refused too.

One consequence, stated because it is measured rather than hidden: an
unemitted validation artifact whose row declares an `applies_when` that
measures zero reads `INAPPLICABLE` on an assembled campaign, not
`MISSING-CHECK` — "the compiler emits this ledger over zero objects" is a
different fact from "the check no longer exists", and the remedy differs.

### An ABSENT optional stage document is a count, not a shrug

The optional stage documents are `compiler::load::OPTIONAL_FILES` that the
build reads as inputs: `world-edits.json`, the four map-pipeline documents and
`design.json`. A campaign that ships
none of them is not a campaign missing a document — it declares no such stage,
and a probe over one is measuring an object class the campaign has zero of.

**The fact that separates that from "I could not read one" is the compiler's,
never the filesystem's.** `manifest.json` `inputs` is written from the bytes
`load_campaign_dir` actually read, hashed per document. A document in no
build's `inputs` was never read, so the probe counts zero; a document that IS
in `inputs` and that this gate holds no parsed copy of is format rot and reds
`MISSING-CHECK`. A build tree that cannot answer — no manifest, or an `inputs`
that is not a mapping — restores "I could not look" and reds, because guessing
zero there is the whole defect this tool exists to refuse.

The filesystem cannot be asked, and that is the load-bearing half: a directory
or an unreadable file standing where a document belongs looks exactly like
absence on disk. `load::optional` treats **only** `NotFound` as absent, so
every one of those campaigns fails to build — no build tree, no manifest, and
`--build` is mandatory. The defect cannot present this witness.

The zero this produces is adjudicated by the rules above and by nothing else:
the counted `INAPPLICABLE`. No verdict,
row field, disposition or operator flag is added anywhere. Its cost is one obligation the ledger now
carries: a `dsl` probe may only name a document this gate reads, refused at
load time, because a mistyped filename appears in no manifest and would
otherwise measure zero on every campaign forever.

### The gate is wired to the staging EVENT, not to a doc line

"No build is handed to the owner until the gate has been run" is a process
obligation, and a process obligation is what **UNRUN** is made of: a correct
gate — right verdicts, fails in the direction that drifts — that nothing
calls.

**A doc line is not an invocation.** So the staging surface requires the gate's
output rather than asking for it. The surface is exactly the set of paths that
put a build in front of the owner:

| Staging path | How the gate is bound to it |
|---|---|
| `tools/creator/playtest-server.sh up` (throwaway `docker run`, binds 25565 — the one she actually runs) | runs the gate itself between `delvec build` and `docker run`; a refusal dies before any container exists |
| `docker compose -f validation/compose.yaml -f validation/owner-play.yaml --profile play\|playtest up` (the other sanctioned 25565 binder) | `owner-play.yaml` adds a `staging-admission` service that both port-publishing services `depends_on: service_completed_successfully` |
| The content repository's `release` workflow (`stellarfeline/delvewright-campaigns`, on a `release/<campaign>/v<semver>` tag, spec-0024 §1) → multi-arch delve image to GHCR + a GitHub Release (she runs the image on the Pi) | **NOT BOUND.** That workflow runs its own ladder and publishes without ever calling this gate. To bind it, add a step in the content repository between `delvec build` and its `docker/login-action` that runs `python3 <engine checkout>/tools/creator/staging-gate.py --campaign <campaign dir> --build <build tree>` — the engine is already checked out there, at the revision `versions.toml` pins |

The compose path cannot run the gate itself — the gate needs the campaign
SOURCE, which the build tree does not carry, and Python, which the delve image
must never gain (ADR-0003). So the gate **mints an admission token** into the
build tree and `validation/staging-admission.sh` verifies it. The token binds
the sha256 of `manifest.json`, the compiler's reproducibility index over the
whole output tree, which closes the obvious bypass: run the gate green on one
tree and serve another. A refusal **deletes** any existing token, so a tree that
was green once and is red now carries nothing.

Not covered, and deliberately: `validation/playtest-note-flow.sh` and
`rehearsal-flow.sh` boot a server for a *bot* on an ephemeral port, and the
worker ladders (`--profile validate`, plain `compose.yaml`) never name
`owner-play.yaml`. None of them is a path to her client, and gating them would
slow every ladder to protect nobody.

### The override, and why it is shaped the way it is

She will sometimes want to look at one beat mid-work. That is legitimate, and an
override that did not exist would simply be routed around. It is
`--stage-anyway "<reason>" --acknowledge-red <N>`, and it is deliberately
awkward: the reason must be substantive, and **`N` must equal the current red
count exactly**. The count moves as the ledger does, so it cannot be typed from
memory — the failure mode being designed against is not "someone overrides
once", it is "the override becomes how the tool is run". It prints every class
being overridden, stamps the reason into the token, and
`staging-admission.sh` re-announces it at boot: *anything she hits from those
classes in this session is the override, not a new finding.*

**Obligation.** Every playtest APPENDS its findings to the ledger, the same day,
with the triage rule 4 requires. The gate's red list is carried into the round
summary item by item — a red is not permission to stop, it is the list of
classes she is not protected from — and so is its `INAPPLICABLE` list, which is
the list of classes this build cannot present to her at all. The gate over a
CONTENT campaign is deliberately **not** a CI status check: the ledger is red on
those by design, and making an honest red list blocking would force the one
move CLAUDE.md forbids. Its falsification suite is in CI instead
(`tools/tests/test_staging_gate.py`), including a tripwire asserting that both
owner-facing paths still require admission — so the UNRUN shape reds here rather
than waiting for a reviewer to notice it again.

### The one subject a push can ask this about is the gallery

A gate bound only to the staging event first speaks at the end of the pipeline.
The engine can grow a campaign it would refuse, and nothing before a creator's
own build says so — a rule obeyed exactly as well as whoever remembers it, one
step earlier.

The subject a push HAS is the gallery: engine-owned, built by every revision,
holding one instance of every surface the DSL declares (spec-0039). So
`tools/ci/check-gallery-stageable.py` runs this gate on **every point of the gallery
domain**, as a step of the `gallery baseline (emission, warnings, served points)` job. The points
are enumerated from `gallery/baseline/manifests.json`, the ladder's own build
ledger, and cross-checked against the gallery directory; judging fewer points
than the ladder builds is the `unbound` vacuity, so a disagreement between the
two refuses rather than shrinking the subject.

The gallery is still never STAGED (spec-0039 §2). Judging a build is a question
about coverage, not an act of handing anything to anybody — so every admission
token that step mints goes to its work directory, never into a build tree, and a
`staging-admission.json` found inside one is a red naming the point.

What it makes true on every push: *the engine's own campaign declares nothing it
has no live, binding check for.*

### What this ledger is reconstructed from, and what is missing

Stated because a findings ledger that silently starts at round 12 is precisely
the defect the gate exists to prevent. Sources: the island's own 52-row ledger
and round records (`GENERATION.md`, rounds 3–22); the private notes (gitignored,
`docs/notes/private/`) — the island ledger audit and the evidence log, read end
to end — and the two session handoffs; `hollow-vigil`'s `GENERATION.md`; the bell's
records on `campaign/the-drowned-bell-r3` and `REMAKE.md`, the on-disk task
archive (`~/.claude/tasks/`), and the
diagnostics catalogue in `compiler.md`, which is the closest thing the repo has
to a finding→diagnostic index.

Known gaps, each a reason a row may be missing rather than closed:

- **hollow-vigil's round-1 findings are not enumerated anywhere.** Four are
  recoverable from `spec-0002`.
- **The island ledger audit's r17 addendum was lost** and is recorded as lost.
- **The bell's round-6 batch of eight owner findings** is carried as pending
  work with no diagnostic coverage for any of the eight; only those with a clear
  general form are in the ledger as rows.
- **`the-wake` and `the-toll-road` were never staged for the owner**, so their
  records hold worker findings only — deliberately not in this ledger.
- The record contained one **wrong DW number** for a real finding (the
  branch-aware walk origin was written as `DW0486`, which is a different rule;
  it is `DW0488`). The catalogue is authoritative over any narrative log, and a
  ledger row citing a code must be checked against it — the gate reports the
  mismatch as `MISSING-CHECK` only when the cited code is absent entirely, so a
  wrong-but-existing code passes silently. That is the gate's own known blind
  spot.

## Rule 8 — a judged verdict declares whether the instrument or the artifact bounded it

Rules 1–7 govern gates, which answer by themselves. A round also produces
**judgements** — does this read as the thing, is this interior right, is this
fight too hard — and a judgement is bounded twice: by the artifact, and by
whatever took the picture. Only the first is worth recording.

The two are separable and the separation is cheap. Trial 0001 answered R1
`partial` for its second run and, in the same section, recorded that the shot set
was four fixed 45° orbits with no square-on elevation of any face, and that this
"alone is the whole of R1's `partial`". Both statements are true and three
paragraphs apart. The verdict is the half later rounds cite. Re-photographed
square-on from the same delivered bytes with an aimed camera, the answer is
`yes`: the record understated its own result, and the number that was wrong was
the headline one.

**Obligation.** Every judged verdict in a round or trial record carries, beside
the verdict, one of two declarations:

- **artifact-bound** — the instrument could frame the thing being judged, and
  the answer is about the artifact. Name the instrument anyway; a later reader
  re-takes the shot to disagree.
- **instrument-bound — `<blocker>`** — it could not, and the blocker is named.
  A named blocker is a capability-gap finding and rule 4 applies to it: it lands
  before the next round, or the summary says the verdict is not to be trusted
  yet. An instrument-bound verdict is re-taken when its blocker closes; it is
  never left standing as though it were about the artifact.

`tools/ci/check-trial-verdicts.py` enforces this over `docs/trials/`, in the docs
job. It enumerates the entry points rather than trusting a checklist — every
trial record, every `## Run N — result` section in it, every rubric row that
carries a bolded verdict — so a record cannot gain a run, or an answer, without
gaining the declaration. It reds three ways: a verdict with no declaration, an
`instrument-bound` declaration that names no blocker, and a record whose rubric
yields zero verdicts, which means the gate has bound to nothing.

The general form of the failure is wider than photographs, and the review
question is the one the gate cannot ask: **what would this verdict have to look
like for the instrument to be unable to tell?** A bot that cannot jump reports
every ledge as impassable. A probe that measures the region box reports every
free-standing building as dark. A judgement is not evidence about the artifact
until that question has an answer.

## Why the final round was clean

Not because one round fixed everything. Because:

1. Rounds 17–21 ran with no owner exposure at all (prose, happenings, branch
   declaration, version adoption, the terminal round). Round 16 *was* an owner
   playtest — four items, every one of them a finding she had already reported,
   which is the rebuke rule 4 comes from.
2. By r21 all four declaration surfaces had landed, so the *existing* gates
   finally had something to bite.
3. Round 21 is recorded as "red frames, in the order the machine produced them —
   every fix admitted by a red first": a campaign green for twenty rounds
   produced **eight classes of machine red in one round** (`DW0205`×3, `DW0469`,
   `DW0483`×3, `DW0489`, `DW0132`, `DW0180`/`DW0181`, `DW0310`, `DW0450`). Those
   eight are the findings the owner would otherwise have reported.
4. Round 22's bot died in the storm gauntlet — the machine making a difficulty
   judgement that had previously required a human.
5. Only then: full ledger audit, localized build, determinism, server self-check,
   and the invitation.

The owner found nothing because everything machine-findable had already been
found. That is the target state for every campaign, and rules 1–8 are how a round
gets there without spending twenty-two of her hours discovering them again.
