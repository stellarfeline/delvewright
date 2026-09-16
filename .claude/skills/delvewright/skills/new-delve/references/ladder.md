# Step 10 — the machine ladder


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
project-scoped, so two ladders can never overwrite each other's.

- The bot ladder has two labelled stages once the delve has mandatory combat:
  `critical-path` and `die-retry`. The die-retry stage adds two scripted deaths
  per encounter, so a combat-heavy delve needs a larger timeout than the
  20-minute default: `DELVEWRIGHT_RUN_TIMEOUT_MS=2400000` on the command.
- **Read the `floor_gate` block every time.** It is the compiler's coverage
  ledger. `not_covered` names each fight the delve bills `elite`/`boss` that the
  gate cannot measure, with the reason — an empty findings list over an
  uncovered elite is silence, not a pass. **`covered`, `not_covered` and
  `actors[]` all empty is the worst case, not the best**: it means no body in the
  campaign declares a tier, the gate examined nothing, and it would have been
  green no matter what you shipped. Report that as **unbound**, never as a pass.
- An **empty `assist_windows`** is not evidence of anything on its own — read
  the `encounters` block beside it, which states each encounter's assist policy
  and the phase the run reached. Expect several windows per encounter.
- Reading one death trial: `respawn_pos` is where the bot actually came back and
  `at_checkpoint` is derived from it; `returned` is the walk back from exactly
  there. `re_engaged` and `outcome` are observed ONLY when `returned` — a trial
  that never got back reads `outcome: unproven`, which means the loop was never
  in a position to be judged, not that the fight vanished. `kit_kept: false` is a
  broken world seal, not a difficulty knob. A red `die-retry` is a content bug of
  the most serious kind: the delve is completable but dying is not safe. Never
  set `DELVEWRIGHT_DIE_RETRY=0` to get green — the report records a skipped stage
  as skipped, not as passed.

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
