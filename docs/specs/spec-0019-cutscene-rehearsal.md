# spec-0019: Cutscene rehearsal + in-game shot calibration

- **Status**: Draft, design approved (owner 2026-08-02, three rulings: rehearsal
  replays the **full performance**, not camera-only — staging defects are what
  playtests actually catch; the whole adjust-and-replay loop happens **inside
  one game session** with a single harvest at the end — never mark → harvest →
  recompile → rejoin per iteration; and shot timing is **converged by watching**
  — an LLM-computed initial value nudged live, never derived from wall-clock
  gaps between the creator's commands, which are input/thinking noise.
  Triggered by island round-7 — the seal cinematic shipped inside-out and every
  review pass over offset numbers missed it)
- **ADRs**: 0003 (overlay is tooling-side), 0012 (reports feed the agent loop)
- **Builds on**: spec-0006 (creator overlay, `playtest` profile, harvester),
  spec-0008/0014 (the `cutscene` effect this rehearses)

LLMs are demonstrably bad at authoring camera positions as `anchor + offset`
numbers: three island QA rounds shipped shots that pointed the wrong way. The
fix is to move the judgment to the medium it belongs in — the running game —
twice over: **review** (replay any beat on demand, story state restored after)
and **authoring** (adjust the shot live until it looks right, then harvest once).

The DSL stays the artifact of record. In-game adjustments mutate a **proposal**
in data storage, never the datapack; the harvested proposal becomes an
`anchor + offset` DSL patch that re-enters `delvec build` and passes the same
proofs as a hand-written shot (`DW0308` air corridors, `DW0347` angular budget,
clear-eye checks) — the tool makes authoring easier, never less checked.

## 1. The shot proposal — one session, no recompiles

The mechanism that makes the live loop possible: the creator overlay carries a
**macro-function dolly player** (vanilla 1.20.2+ `$(…)` macros — an intended
data-driven primitive, not a hack). Every shot's parameters (path points,
`look_at`, `seconds`) are baked into `dw:rehearsal` data storage at load time
from the compiled DSL values; **replay reads the storage proposal, adjustments
mutate it** — so reposition/re-aim/re-time and replay cycle indefinitely inside
one game session, no harvest, no rebuild, no rejoin.

The shipped cutscene emission is untouched: static compiled functions, exactly
as today. The macro player exists only in the creator overlay, and an
**equivalence PackTest** pins the two implementations together (same
parameters → same camera position at every sampled tick) so what the creator
approves in rehearsal is what the player sees shipped.

## 2. Rehearsal — replay the performance

- **`/trigger dw.beat set <b>`** — replay beat `b`: the cutscene **and its
  entire surrounding effect bundle** (actor spawns/walks/despawns, sounds,
  narrates, gate fills, the sequence timeline). Camera-only rehearsal is
  explicitly rejected (owner): the island's shipped defects — an actor
  entering from the wrong side, a flock that never moved — live in the
  staging, and a camera-only replay cannot show them. Shot cameras play from
  the **current storage proposal**; staging replays as compiled. The overlay
  `say`-stamps the beat roster with ids on join; `dw.shot set <s>` replays a
  single shot when only framing is in question.
- **State restore is automatic and derived at compile time.** Before the
  replay the overlay snapshots story state (flag/objective/quest scoreboards
  → shadow objectives); after it, a compiler-emitted **inverse function**
  undoes the beat: replayed actors killed by tag, NPCs the beat despawned
  re-summoned, `close-gate`/`open-gate` fills re-run in reverse (both are
  deterministic region fills), scoreboards restored from the snapshot. Every
  inverse is derivable statically from the bundle — no runtime guessing. A
  beat containing an effect with no sound inverse (e.g. `give-item` into a
  player inventory) restores state around it and `say`-stamps what it could
  not undo.
- **`/trigger dw.free set 1`** — detach from the dolly for the next replay:
  the performance runs, the creator flies freely and watches from outside
  (the vantage that catches a wrong-side entrance). Default (0) rides the
  dolly — same interpolation and easing as shipped, per the equivalence
  test. On beat end the creator is restored to prior position and gamemode.

## 3. Calibration — adjust the proposal live

All of these mutate `dw:rehearsal` storage only; the next replay shows the
result immediately:

- **`/trigger dw.mark set <s>`** — append the creator's current **eye position
  + view direction** as the next waypoint of shot `s`'s proposal (first call
  = path start, second = path end, further calls = intermediate waypoints).
  `set -<s>` resets shot `s`'s path proposal to the compiled values.
- **`/trigger dw.aim set <s>`** — set shot `s`'s proposed `look_at` to the
  block the creator is looking at (raycast).
- **`/trigger dw.faster set <s>` / `dw.slower set <s>`** — scale shot `s`'s
  proposed `seconds` by ∓20%, clamped to 2–30 s. The **initial** value is
  computed by the LLM from path length at a default dolly speed when the shot
  is authored; convergence is by watching replays, never by measuring the
  creator's wall-clock (command gaps include typing, observing, and retries —
  noise, not pace; owner ruling).
- **`/trigger dw.done`** — the single harvest: the overlay `say`-stamps the
  **entire current proposal** as machine-readable `[DelveShot]` lines (one
  per shot: path, look_at, seconds) — the same log channel and pairing model
  `[DelveNote]` uses (spec-0006 §3; `tellraw` never reaches the server log,
  `say` does).

## 4. Write-back

- The **harvester** (`delve-harvest`) parses `[DelveShot]` lines into a
  versioned `rehearsal-report.json` beside `playtest-report.json`.
- A converter (`delvec calibrate <report>`) snaps each proposal to the DSL
  vocabulary: nearest declared anchor (from the build's resolved-anchor
  manifest) + integer offset, `look_at` likewise, `seconds` carried through.
  Output is a ready-to-apply JSON patch per shot, printed with the distance
  error introduced by integer snapping.
- The agent applies the patch to the stage document, reruns `delvec build`,
  and the normal proofs gate it. Nothing writes to stage documents directly
  from the game.

Loop: playtest → adjust + replay in-session until satisfied → `dw.done` →
harvest → `delvec calibrate` → patch DSL → rebuild once → final rehearsal to
confirm the compiled result matches the approved proposal.

## 5. Explicitly out of scope

- No runtime shot editing in shipped delves (overlay never ships — existing
  CI exclusion covers `creator-datapack/`; the macro dolly player ships only
  in the overlay).
- No free-floating world-coordinate shots in the DSL: calibration emits
  `anchor + offset` only. If no anchor is within a sane snap radius (16
  blocks), the converter says so and the fix is a prefab-metadata anchor,
  not a raw coordinate.
- No new DSL fields.

## Acceptance criteria

- [ ] Overlay rehearsal artifacts (storage defaults, macro dolly player, beat
      replay + inverse functions) are emitted for every declared cutscene,
      byte-deterministic (ADR-0006 gate covers `creator-datapack/`), and
      absent from the shipped image (existing tier-2 exclusion check).
- [ ] **Equivalence PackTest**: for identical parameters, the macro dolly and
      the compiled cutscene dolly produce the same camera position at every
      sampled tick (within interpolation tolerance) — the rehearsal preview
      is provably what ships.
- [ ] PackTest: `dw.mark`/`dw.aim`/`dw.faster`/`dw.slower` mutate only
      `dw:rehearsal` storage; an immediately following `dw.shot` replays with
      the mutated values — **no datapack reload between adjust and replay**.
- [ ] PackTest: `dw.beat` on a fixture beat runs the staging (actor spawned,
      gate filled) and, after the inverse, world and story state equal the
      pre-replay snapshot: replayed actors gone, gate region back to its
      prior blocks, every flag/objective scoreboard restored, runner back at
      prior position + gamemode.
- [ ] PackTest: a beat containing a non-invertible effect still restores
      scoreboards and stamps a machine-readable "not undone" line.
- [ ] PackTest: `dw.done` stamps one parseable `[DelveShot]` line per shot
      reflecting the current proposal (not the compiled defaults, when they
      differ).
- [ ] Harvester: fixture log → `rehearsal-report.json` with versioned schema;
      unit-tested shape.
- [ ] Converter round-trip property: a proposal at a known world position
      yields `anchor + offset` that resolves back to the same block cell; an
      aim at a known block yields a `look_at` within 1 block of it; `seconds`
      survives the round trip. Proposals farther than the snap radius from
      every anchor are reported, never silently snapped.
- [ ] Live tier-3 check in the `playtest-note-flow.sh` pattern: scripted bot
      adjusts a shot, replays, fires `dw.done`; harvested report matches the
      adjusted values, not the originals.
