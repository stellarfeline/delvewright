# spec-0019: Cutscene rehearsal + in-game shot calibration

- **Status**: Draft (owner direction 2026-08-02: chose "rehearsal mode + in-game
  calibration" over external preview tooling; triggered by island round-7 — the
  seal cinematic shipped inside-out and every review pass over offset numbers
  missed it)
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

New overlay triggers, emitted per campaign by `creator.rs` from the same shot
data the real cutscene emission reads:

- **`/trigger dw.cut set <c>`** — select cutscene `c` (1-based, campaign
  declaration order; the overlay `say`-stamps the roster with ids on join).
- **`/trigger dw.shot set <s>`** — play shot `s` of the selected cutscene on
  yourself: the **exact emitted camera path** (same spectator dolly, same
  `teleport_duration` interpolation, same easing), camera moves only — none of
  the surrounding effect bundle (no despawns, no narrate, no flags). `set 0`
  plays the whole cutscene's shot chain. Replay freely; story state is never
  touched. On shot end you are restored to your prior position and gamemode.

Rehearsal functions are camera-only derivations, so a cutscene that stages
actors (walkers, sheep) rehearses against whatever is live in the world — the
overlay does not spawn stand-ins (staging rehearsal = trigger the real beat;
this mode is for framing and movement).

## 2. In-game calibration

- **`/trigger dw.mark set <s>`** — append the creator's current **eye position
  + view direction** as the next waypoint of shot `s`'s *proposal* (first call
  = path start, second = path end, further calls = intermediate waypoints).
  `set -<s>` discards shot `s`'s proposal.
- **`/trigger dw.aim set <s>`** — record the block the creator is currently
  looking at (raycast) as shot `s`'s proposed `look_at`.

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

- [ ] Overlay rehearsal functions are emitted for every declared cutscene
      shot, byte-deterministic (ADR-0006 gate covers `creator-datapack/`),
      and absent from the shipped image (existing tier-2 exclusion check).
- [ ] PackTest: `dw.shot` on a fixture cutscene moves the test runner along
      the shot (position sampled mid-flight within tolerance) and restores
      position + gamemode at the end; story scoreboards are untouched.
- [ ] PackTest: `dw.mark` / `dw.aim` append records carrying `Pos` +
      `Rotation` to `dw:rehearsal` storage and stamp a parseable
      `[DelveShot]` line.
- [ ] Harvester: fixture log → `rehearsal-report.json` with versioned schema;
      unit-tested shape.
- [ ] Converter round-trip property: a mark at a known world position yields
      `anchor + offset` that resolves back to the same block cell; an aim at a
      known block yields a `look_at` within 1 block of it. Marks farther than
      the snap radius from every anchor are reported, never silently snapped.
- [ ] Live tier-3 check in the `playtest-note-flow.sh` pattern: scripted bot
      fires the triggers; harvested report matches the fixture positions.
