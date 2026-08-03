# spec-0019: Cutscene rehearsal + in-game shot calibration

- **Status**: Draft, direction approved (owner 2026-08-02: chose "rehearsal mode
  + in-game calibration" over external preview tooling, then two rulings on
  review — rehearsal must replay the **full performance**, not camera-only,
  because staging defects are what playtests actually catch; and shot timing is
  **demonstrated**, not authored: the creator's own flight pace between
  calibration marks becomes the shot's `seconds`. Triggered by island round-7 —
  the seal cinematic shipped inside-out and every review pass over offset
  numbers missed it)
- **ADRs**: 0003 (overlay is tooling-side), 0012 (reports feed the agent loop)
- **Builds on**: spec-0006 (creator overlay, `playtest` profile, harvester),
  spec-0008/0014 (the `cutscene` effect this rehearses)

LLMs are demonstrably bad at authoring camera positions as `anchor + offset`
numbers: three island QA rounds shipped shots that pointed the wrong way. The
fix is to move the judgment to the medium it belongs in — the running game —
twice over: **review** (play any shot on demand, without touching story state)
and **authoring** (stand where the camera should be, mark it, and get DSL back).

The DSL stays the artifact of record. Calibration produces `anchor + offset`
shot data; it never bypasses it. A calibrated shot re-enters `delvec build`
and passes the same proofs as a hand-written one (`DW0308` air corridors,
`DW0347` angular budget, clear-eye checks) — the tool makes authoring easier,
never less checked.

## 1. Rehearsal mode (creator overlay, `playtest` profile only)

New overlay triggers, emitted per campaign by `creator.rs` from the same
effect-bundle data the real emission reads:

- **`/trigger dw.beat set <b>`** — replay beat `b` — the cutscene **and its
  entire surrounding effect bundle** (actor spawns/walks/despawns, sounds,
  narrates, gate fills, the sequence timeline), exactly as emitted. Camera-only
  rehearsal is explicitly rejected (owner, 2026-08-02): the island's shipped
  defects — an actor entering from the wrong side, a flock that never moved —
  live in the staging, and a camera-only replay cannot show them. The overlay
  `say`-stamps the beat roster with ids on join; `dw.shot set <s>` replays a
  single shot's camera when only framing is in question.
- **State restore is automatic and derived at compile time.** Before the
  replay the overlay snapshots story state (flag/objective/quest scoreboards
  → shadow objectives); after it, a compiler-emitted **inverse function**
  undoes the beat: replayed actors are killed by tag, NPCs the beat despawned
  are re-summoned, `close-gate`/`open-gate` fills are re-run in reverse
  (both are deterministic region fills), and the scoreboard snapshot is
  restored. Every inverse is derivable statically from the bundle — no
  runtime guessing. A beat containing an effect with no sound inverse (e.g.
  `give-item` into a player inventory) restores state around it and
  `say`-stamps what it could not undo.
- **`/trigger dw.free set 1`** — detach from the cutscene dolly for the next
  replay: the performance runs, the creator flies freely and watches from
  outside (the vantage that catches a wrong-side entrance). Default (0)
  rides the real emitted dolly — same spectator path, same
  `teleport_duration` interpolation, same easing. On beat end the creator is
  restored to prior position and gamemode.

## 2. In-game calibration

- **`/trigger dw.mark set <s>`** — append the creator's current **eye position
  + view direction**, with a **gametime timestamp**, as the next waypoint of
  shot `s`'s *proposal* (first call = path start, second = path end, further
  calls = intermediate waypoints). `set -<s>` discards shot `s`'s proposal.
- **`/trigger dw.aim set <s>`** — record the block the creator is currently
  looking at (raycast) as shot `s`'s proposed `look_at`.
- **Timing is demonstrated, not authored** (owner ruling): the creator marks
  the start, flies the path **at the pace the shot should play**, and marks
  the end — `delvec calibrate` turns the elapsed time between a shot's first
  and last mark into its `seconds` (rounded, clamped to 2–30s; the field
  stays hand-editable afterwards). The camera moves at the speed the human
  demonstrated, never a guessed number.

Each mark is appended to `dw:rehearsal` data storage **and** `say`-stamped as
one machine-readable log line (`[DelveShot] cut=<c> shot=<s> kind=mark|aim
pos=[x.x,y.y,z.z] rot=[yaw,pitch]`) — the exact channel and pairing model
`[DelveNote]` already uses (spec-0006 §3; `tellraw` never reaches the log,
`say` does).

## 3. Write-back

- The **harvester** (`delve-harvest`) parses `[DelveShot]` lines into a
  versioned `rehearsal-report.json` beside `playtest-report.json`.
- A converter (`delvec calibrate <report>`) snaps each mark to the DSL
  vocabulary: nearest declared anchor (from the build's resolved-anchor
  manifest) + integer offset, and derives `look_at` from the aim mark (or, with
  no aim mark, from the recorded view direction's first block intersection).
  Output is a ready-to-apply JSON patch per shot (path array + look_at),
  printed with the distance error introduced by integer snapping.
- The agent applies the patch to the stage document, reruns `delvec build`,
  and the normal proofs gate it. Nothing writes to stage documents directly
  from the game.

Loop: playtest → mark shots in-game → harvest → `delvec calibrate` → patch
DSL → rebuild → re-render/rehearse to confirm.

## 4. Explicitly out of scope

- No runtime shot editing in shipped delves (overlay never ships — existing
  CI exclusion covers `creator-datapack/`).
- No free-floating world-coordinate shots in the DSL: calibration emits
  `anchor + offset` only. If no anchor is within a sane snap radius (16
  blocks), the converter says so and the fix is a prefab-metadata anchor,
  not a raw coordinate.
- No new DSL fields.

## Acceptance criteria

- [ ] Overlay rehearsal functions (beat replay + inverse + shot replay) are
      emitted for every declared cutscene/beat, byte-deterministic (ADR-0006
      gate covers `creator-datapack/`), and absent from the shipped image
      (existing tier-2 exclusion check).
- [ ] PackTest: `dw.beat` on a fixture beat runs the staging (actor spawned,
      gate filled) and, after the inverse, world and story state equal the
      pre-replay snapshot: replayed actors gone, gate region back to its
      prior blocks, every flag/objective scoreboard restored, runner back at
      prior position + gamemode.
- [ ] PackTest: a beat containing a non-invertible effect still restores
      scoreboards and stamps a machine-readable "not undone" line.
- [ ] PackTest: `dw.mark` / `dw.aim` append records carrying `Pos` +
      `Rotation` + gametime to `dw:rehearsal` storage and stamp a parseable
      `[DelveShot]` line.
- [ ] Harvester: fixture log → `rehearsal-report.json` with versioned schema;
      unit-tested shape.
- [ ] Converter round-trip property: a mark at a known world position yields
      `anchor + offset` that resolves back to the same block cell; an aim at a
      known block yields a `look_at` within 1 block of it; two marks N ticks
      apart yield `seconds = N/20` rounded and clamped to 2–30. Marks farther
      than the snap radius from every anchor are reported, never silently
      snapped.
- [ ] Live tier-3 check in the `playtest-note-flow.sh` pattern: scripted bot
      fires the triggers; harvested report matches the fixture positions and
      timings.
