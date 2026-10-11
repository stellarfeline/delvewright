# Step 10 — the machine ladder

## Contents

- [PackTest and the bot](#packtest-and-the-bot)
- [The written world](#the-written-world)
- [Reading the run report](#reading-the-run-report)
- [Branch runs](#branch-runs)
- [On a red](#on-a-red)

## PackTest and the bot

Docker required. Every entry script takes a **`--project <id>`** and it is
required everywhere: the validation stack pins no container name and publishes
no host port, so the compose project is the only name the stack has, and two
ladders with distinct ids run side by side with no lock and no queueing. An
entry script invoked without one fails loudly rather than landing in a shared
default. Use `dw-<campaign>-r<round>`.

Each script fresh-volumes its own project before and after every run, so a
persisted world cannot keep completed objectives completed and fail a "fresh"
playthrough for reasons that have nothing to do with the delve.

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/packtest-run.sh" --project dw-<campaign>-r1
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/bot-run.sh" --project dw-<campaign>-r1
```

Both must exit 0. Then read `$DELVEWRIGHT_ENGINE/validation/run-out/<id>/run-report.json` — it is
project-scoped, so two ladders can never overwrite each other's. Keep its path:
the staging gate in step 13 admits only on it (`--run-report`), and only while
its `build.manifest_sha256` is the build being staged.

## The written world

**Then the world the server built, against the world the engine models.** Every
proof walks the engine's model; this is the one comparison with what the pinned
server actually placed (`DW0955`). Mandatory on the build that ships:

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/world-save.sh" "$DELVEWRIGHT_ENGINE/validation/delve-output" \
    --project dw-<campaign>-r1-world
delvec --prefabs "$DELVEWRIGHT_PREFABS" written-world campaigns/<id> -o .out/written-world
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/ci/check-written-world.py" \
    "$DELVEWRIGHT_ENGINE/validation/delve-output" .out/written-world \
    "$DELVEWRIGHT_ENGINE/validation/delve-output/world" --record .out/written-world.json
```

`delvec written-world` writes the world the engine models at load from the
campaign and `--prefabs` alone — no camera record, no server boot — so a campaign
with no `design/cameras.json` walks this step the same way.

The check must exit 0. A red names every cell the server holds differently from
the model, by state pair: that is a toolchain bug (the engine's model or emission
is wrong), never a campaign to rearrange. Keep `.out/written-world.json` for the
staging gate (`--written-world`).

## Reading the run report

- The bot ladder has two labelled stages once the delve has mandatory combat:
  `critical-path` and `die-retry`. The die-retry stage adds two scripted deaths
  per encounter, so a combat-heavy delve needs a larger timeout than the
  20-minute default: `DELVEWRIGHT_RUN_TIMEOUT_MS=2400000` on the command.
- **Read the `encounters` block every time.** At a combat step the ladder reads
  the wave's LIVE bodies against what your document declared — count, health,
  attack damage, armour from the gear, the name over its head — and only then
  removes them so the run can go on. `muster.failures` is a delve that is not the
  document: an attribute or a piece of equipment that never reached the body, with
  the body's own reading beside it. It reds the run, and the fix is in the
  campaign, never in the ladder.
- **`declared_facts: 0` is the worst case, not the best**: the muster asked the
  bodies nothing, and the step would have been green whatever you shipped. Report
  that as **unbound**, never as a pass. `muster: null` means the step never got as
  far as reading, which is again not a pass.
- **Sculk, watchers and pulses are asserted on the walk, each with its own
  line.** `[sculk] <walk>: N sensor(s) predicted, N heard; M shrieker(s)
  predicted, M heard` per leg — a predicted vibration not heard fails the step —
  and `[sculk] darkness D, warden W` at the end of every run, where either
  non-zero fails it. `[watch] W watcher(s) in the record, K within reach on the
  proven path, K asserted …` — a watcher not facing the bot where the route
  passes in reach fails the critical path; one the path never reaches is named,
  not failed. `pulse binding: H of N pulse(s) heard at a listening station, S
  silent at a silent station`, every station a `pulse_stations` row of the run
  report; a pulse with no station reads `not_heard: no station`, never a pass.
  What each asserts: `$DELVEWRIGHT_ENGINE/docs/reference/playtest-methodology.md`.
- **The ladder does not fight, and says nothing about whether a fight can be
  won.** `staged_removals` lists every body the HARNESS took out of the delve and
  why; it is the harness acting, not your delve behaving. Whether the delve is too
  hard is the owner's hour.
- Reading one death trial: `respawn_pos` is where the bot actually came back and
  `at_checkpoint` is derived from it; `returned` is the walk back from exactly
  there. `re_engaged` and `outcome` are observed ONLY when `returned` — a trial
  that never got back reads `outcome: unproven`, which means the loop was never
  in a position to be judged, not that the fight vanished. `kit_kept: false` is a
  broken world seal, not a difficulty knob. A red `die-retry` is a content bug of
  the most serious kind: the delve is completable but dying is not safe. Never
  set `DELVEWRIGHT_DIE_RETRY=0` to get green — the report records a skipped stage
  as skipped, not as passed.

## Branch runs

**Branch runs, required whenever the build emitted `validation/branch-plan.json`.**
One critical-path run proves one storyline; a campaign that forks must have every
branch walked, each in its own fresh world.

```sh
EULA=TRUE "$DELVEWRIGHT_ENGINE/validation/branch-runs.sh" --project dw-<campaign>-r1
```

It writes `$DELVEWRIGHT_ENGINE/validation/run-out/<id>/branch-runs.json`: per branch, ran or
skipped-with-reason, and the result. `DELVEWRIGHT_BRANCHES=<ids>` narrows it for
local iteration, and **a narrowed run is not a validated campaign** — the report
says which branches it skipped.

## On a red

**On any red, triage before touching anything.**

- *Content bug* — your documents declare something wrong, unreachable or unlit:
  fix the campaign in the documents.
- *Toolchain bug* — the compiler or harness misbehaves on a campaign the
  diagnostics accept: **stop content work and report it**, with evidence. Never
  hand-edit compiler output, never restructure the campaign to dodge the bug,
  never weaken a check or reroll a seed to get green. A workaround that turns a
  toolchain bug green ships the bug to every future campaign. Escalating is
  success.

*Reference: when something goes red* maps the symptoms you are most likely to
meet to their actual causes.
