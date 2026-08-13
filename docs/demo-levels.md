# First-party demo levels — the mechanic showcase backlog

Owner directive (2026-08-03): **every new mechanic gets a small first-party
level that verifies and shows it off.** Not necessarily right when the
mechanic lands — but it must be queued here, and this queue is the planning
agent's **standing idle work**: no urgent development task + owner not
responding → build the next level in this list (full ladder green, content-repo
PR, playable via the play profile).

Rules of the queue:

- A demo level is SMALL: one mechanic in the spotlight, 10–20 minutes, minimum
  cast. It doubles as living documentation — a stranger (or a future /new-delve
  session) plays it and understands the mechanic's intended use.
- "Covered by" means a shipped full campaign already showcases the mechanic
  prominently; no separate demo needed unless the coverage rots.
- Levels are authored via /new-delve like any campaign (dogfooding — friction
  found here is toolchain work, which is the point; owner principle 2026-08-03:
  polishing levels is the driver for toolchain/prompt improvement).
- New mechanics: the PR that lands a mechanic adds a row here (same-PR rule,
  like docs/reference sync).

## Mechanic demos

| Mechanic (spec) | Demo concept | Status |
|---|---|---|
| Traps: stair-volley / dart gallery / trapped chest (0011) | **The Toll Road** — a short fortified pass where every trap type guards one alcove of loot; disarm levers teach counterplay | pending |
| Checkpoints + stealth zones (0012) | covered by nobodys-cave-island (blind-giant climb) | covered |
| Actors, staging, cutscenes (0014) | **The Wake** — a funeral procession level that is 80% staging: mourners walk, a eulogy sequence, one player choice redirects the procession | pending |
| Bonfire rest + wave re-seat (0016 §1) | covered by the-drowned-bell | covered |
| Shortcut loop-back (0016 §2) | covered by the-drowned-bell (chapel door) | covered |
| Ambush (0016 §3) | covered by the-drowned-bell (wall-watch, rafters) | covered |
| Timed gate (0016 §4) | **Tide Mill** — a mill race where the water gate cycles; the whole level is timing runs through 3 gates, escalating windows | pending |
| TD lanes + aggro-edge (0016 §6) | **Hold the Causeway** — pure defense: three waves down two lanes at a barricade the party pre-walks | pending |
| Party division of labor (0018) | **Two Keys** — a 2-player vault where progression is provably split (carrier:one items, AND-join objectives); solo-completable per DW0356 floor | pending |
| Point of no return: close-gate + inter-area transport | covered by nobodys-cave-island (the boulder) | covered |
| strike-npc + vanilla-warden combat consequence | covered by nobodys-cave-island (round 11) | covered |
| Bark pools + scene ledger (0020, landing) | **Market Day** — a town-square level dense with background NPCs; one real quest threads through a crowd of bark-pool characters; the cast ledger is the star | pending (blocked on 0020) |
| Cutscene rehearsal/calibration (0019, landing) | tooling, not a level — its demo is a GENERATION.md walkthrough of calibrating one shot | pending (blocked on 0019) |
| i18n zh-cn sidecar | covered by both shipped campaigns | covered |
| Map-editor terrain pass (0017) | covered by nobodys-cave-island (de-walling, beach seam) | covered |
| Ocean horizon / pseudo-open-world boundary (0013) | covered by nobodys-cave-island | covered |
| Block palette selection — screened shelf + mix report (0035) | **Two Naves** — one grammar program, one region, one seed, expanded twice under two palettes whose mean colours sit 13.5 RGB units apart and whose chromatic areas are 60% and 30%. The player walks from one into the other; the point is that a number said they were the same room and the eye says they are different buildings. Ships with its swatch sheet and both mix reports beside it, so the level IS the argument for measuring area share instead of a mean | pending |

## M5 theme suite (owner-approved 2026-08-03, all five)

Genre-diverse levels beyond the Greek-myth and souls lines; each exercises a
distinct authoring register:

1. **Mystery** — a manor whodunit: flags-as-clues, dialogue trees as
   interrogation, the accusation is a branching finale. Zero combat; tests
   whether our dialogue/flag machinery carries a level alone.
2. **Horror** — one long stealth crescendo in a lightless mine (declared
   darkness + mitigation): sound design via play-sound, a pursuer on the
   vanilla-warden pattern, scarce checkpoints tuned merciful.
3. **Tower defense** — Hold the Causeway grown into a full night: build-phase
   walks, three sieges, NPC militia as staged actors.
4. **Heist** — timed gates + patrol routes + party split: get in, get the
   relic, get out before the watch cycles; loud fallback if caught.
5. **Pastoral / cozy** — no fail state at all: a festival day of fetch-and-talk
   beats, bark-pool crowds, one cutscene sunset. Tests whether the engine can
   be gentle.
