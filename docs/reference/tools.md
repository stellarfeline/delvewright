# Tool surface — every runnable tool in this repo

Live inventory of what an authoring, admission or validation session can actually
run today (CLAUDE.md *Tooling sync*). Nothing aspirational is listed: every
invocation below was executed. Semantics live in the per-tool references —
[`compiler.md`](compiler.md) for `delvec`, [`i18n.md`](i18n.md) for translation,
the crate READMEs for the rest; this page is the index and the flag surface.

Each entry carries a **class**, which decides how it enters a skill:

- **agent** — an LLM-facing workflow step. When the symptom appears, running it is
  not optional.
- **human** — human-in-the-loop. A skill mentions it in one line and moves on;
  never blocks.
- **CI** — a gate; a session runs it only to reproduce a red check locally.
- **spike** — one-off measurement rigs, not part of the shipped pipeline.

Rust binaries run from repo root as
`cargo run -q -p <package> --bin <bin> -- <args>` (packages below), or from a
`cargo build` target directory.

---

## 1. `delvec` — the compiler (`crates/compiler`, package `delvewright-compiler`) · agent

The only path from DSL to datapack (ADR-0001). Full behavior:
[`compiler.md`](compiler.md).

| Subcommand | Purpose | Key flags |
|---|---|---|
| `validate <dir>` | stage schema + referential validation | — |
| `analyze <dir>` | quest-graph reachability (implies `validate`) | — |
| `build <dir> -o <out>` | full deterministic build (implies `analyze`) | `-o/--out` (required) |
| `schema --stage <n\|all>` | export a stage's JSON Schema | `--stage` (required) |
| `l10n-inventory <dir>` | l10n key inventory as JSON (translation input) | `--lang` |
| `snapshot <dir>` | one draft frame + scene manifest (spec-0015) | `--camera x,y,z,yaw,pitch[,fov]`, `--at <anchor>`, `--orbit <deg>`, `--dist <n>`, `--shot <id>`, `--labels`, `--width 960`, `--height 540`, `-o snapshot.png`, `--timing` |
| `blocking-chart <dir>` | per-elevation cutaway floor plans (spec-0015) | `-o blocking-chart`, `--timing` |
| `edit apply <dir>` | replay the stage-7 edit script, persist a green candidate | `--batch <file>`, `-o edit-shots` |
| `edit preview <dir>` | same replay + renders, never writes the campaign | `--batch <file>`, `-o edit-shots` |
| `calibrate <report>` | harvested shot proposals → `anchor + offset` DSL patch (spec-0019) | `--layout <creator-datapack/layout.json>` (required), `-o shot-patch.json` |

Global flags on every subcommand: `--json`, `--prefabs <dir>` (default
`campaigns/prefabs`), `--lang <code>` (default `en`), `--version`.
Exit codes and the `--json` diagnostic shape: [`compiler.md` §1](compiler.md).

## 2. `delve-schem` — schematic import (`crates/schem`, package `delvewright-schem`) · agent

Converts a Sponge schematic (`.schem`, v2/v3) into a vanilla structure `.nbt`.
Step 1 of prefab admission. See [`../../crates/schem/README.md`](../../crates/schem/README.md).

```
delve-schem convert <input.schem> -o <out.nbt>
    [--split 48]          # max part size per axis (structure cap); oversize input
                          # is tiled into parts + a <base>.split.json manifest
    [--palette-report]    # print the full input block-state palette (audit feed)
    [--json]
```

## 3. `delve-admit` — prefab admission (`crates/admit`, package `delvewright-admit`) · agent + human

The gate every prefab passes before the library will place it: mechanical palette
audit (ADR-0013 licence discipline + code-injection forbid), socket carving,
anchors, lighting, catalog cards. See [`../../crates/admit/README.md`](../../crates/admit/README.md).

Admission order for an imported piece (**`resolve-jigsaw` runs before `socket`**):

```
delve-admit audit <nbt> [--allowlist <json>] [-o report.json]   # CI gate
delve-admit resolve-jigsaw <nbt>                                # neutralize foreign worldgen markers
delve-admit socket <nbt> --pos x,y,z --facing north|south|east|west
                         [--opening 3,3] [--name keep:socket]
                         [--target keep:socket] [--pool keep:pool]
delve-admit anchor <nbt> --name anchor/<id>
                         [--pos x,y,z] [--facing <kw>]
                         [--region x1,y1,z1:x2,y2,z2] [--block <id>]
delve-admit lighting <nbt> [--write] [--dark-threshold 3]       # probe -> declared profile
delve-admit catalog validate <card.json ...>
```

Gallery curation is the **human** half — the owner walks a browse world and leaves
notes; the agent only builds and harvests:

```
delve-admit gallery <dir-of-nbt> -o <out> [--id <gallery-id>] [--cols 4]
delve-admit curate <server.log> --layout <gallery-layout.json> [-o report.json]
delve-admit curate-merge <report.json> --catalog <catalog-dir>
```

## 4. `delve-render` — render layer (`crates/render`, package `delvewright-render`) · agent

Textured prefab shot sets, the missing-texture fidelity gate, and Chunky scene
emission for whole-scene / player-POV review. Needs the 1.21.11 client jar via
`--textures` or `$DELVEWRIGHT_CLIENT_JAR`. See
[`../../crates/render/README.md`](../../crates/render/README.md).

```
delve-render piece <nbt> -o <dir>            # deterministic multi-angle set for one prefab
delve-render batch <prefab-dir> -o <dir>     # the same for a whole library
delve-render fidelity-gate [-o <dir>]        # FAIL if any missing-texture placeholder renders
delve-render scene <build-dir> -o <dir> [--world world]   # Chunky scene JSONs from render-plan.json
delve-render index <build-dir> -o <file>     # image <-> expect pairs for a reviewing agent
```

Global: `--json`, `--textures <path>`, `--size 1024`. Exit codes and the dark-shot
review policy: [`compiler.md` §5](compiler.md).

## 5. `delve-harvest` — playtest note harvester (`crates/orchestrator`, package `delvewright-orchestrator`) · human

Pairs in-game `[DelveNote]` stamps with the creator's chat notes into
`playtest-report.json` (spec-0006). The capture half is human — the owner plays and
runs `/trigger dw.note`; the agent runs the harvester afterwards.

The same pass harvests spec-0019 `[DelveShot]` stamps (`/trigger dw.done`) into
`rehearsal-report.json`, written **only** when the session actually stamped a
shot proposal — feed that report to `delvec calibrate`.

```
delve-harvest <server.log> <creator-datapack/layout.json> [-o playtest-report.json]
                                                          [--rehearsal-out rehearsal-report.json]
```

Full loop, including how the log is captured:
[`../../validation/README.md`](../../validation/README.md).

## 6. Python tooling (`tools/`)

Never shipped inside a delve.

| Tool | Class | Invocation |
|---|---|---|
| `tools/i18n-translate.py` | agent | `python3 tools/i18n-translate.py <campaign-dir> --lang <code> [--config f] [--delvec cmd] [--batch-size n] [--dry-run] [--force] [--no-validate]` — external OpenAI-compatible API, generation-time only; see [`i18n.md`](i18n.md) |
| `tools/skin/` (`delve_skin`) | agent | `python -m delve_skin all <cast.json> --skins-dir D --catalog-dir D --preview-dir D [--id ID] [--scale N]`, or the `build` / `preview` / `catalog` stages individually. Needs its own venv (`pip install -r tools/skin/requirements.txt`); see [`../../tools/skin/README.md`](../../tools/skin/README.md) |
| `tools/check-dw-codes.py` | CI | `python3 tools/check-dw-codes.py` — asserts the DW catalog in `compiler.md` matches `crates/**/*.rs` both ways, and that every code has a test |
| `tools/extract-sound-registry.py` | maintenance | `python3 tools/extract-sound-registry.py <registries/data.min.json> <out.json>` — regenerates the compiler's sound registry for a new MC pin (positional args only, no `--help`) |
| `tools/extract-font-metrics.py` | maintenance | `python3 tools/extract-font-metrics.py <client.jar> …` — regenerates the font metrics behind the DW0330 text-fit lint (positional args only, no `--help`) |

## 7. Validation stack (`validation/`)

Docker compose is the CI-equivalent environment (CLAUDE.md *Environments*). All
profiles boot the world the compiler declared, via the shared
`world-settings-entrypoint.sh`. Prose:
[`../../validation/README.md`](../../validation/README.md).

| Profile | Class | Command | What it is |
|---|---|---|---|
| `play` | human | `EULA=TRUE docker compose -f validation/compose.yaml --profile play up` | the shipped delve image, joinable at `localhost:25565` |
| `playtest` | human | `EULA=TRUE CREATOR_NAME=<mc-name> docker compose -f validation/compose.yaml --profile playtest up --build` | `play` plus the creator overlay: `/trigger dw.note` stamps the log for `delve-harvest` |
| `validate` | agent | `EULA=TRUE docker compose -f validation/compose.yaml --profile validate up --build --abort-on-container-exit --exit-code-from bot` | server + mineflayer critical-path bot |
| `packtest` | agent | `EULA=TRUE docker compose -f validation/compose.yaml --profile packtest up --exit-code-from packtest` | headless PackTest suite on the tool server |

Shell entry points:

| Script | Class | Purpose |
|---|---|---|
| `validation/fresh-volumes.sh` | agent | tear the stack down and **prove** the world volumes are gone. Run before any re-run of the bot ladder — a stale volume keeps completed objectives completed and fails a fresh playthrough for reasons unrelated to the delve |
| `validation/render-shots.sh <build-dir> [out-dir]` | agent | turn a build output into the Chunky scene set + shot index (`delve-render scene` + `index`), including the first-person POV shots |
| `validation/playtest-note-flow.sh` | CI (tier 3) | `EULA=TRUE validation/playtest-note-flow.sh` — drives the whole spec-0006 note loop non-interactively and asserts the report |
| `validation/rehearsal-flow.sh` | CI (tier 3) | `EULA=TRUE validation/rehearsal-flow.sh` — drives the whole spec-0019 calibration loop (`dw.aim`/`dw.faster`/`dw.mark`/`dw.done` → harvest → `delvec calibrate`) and asserts the patch resolves back to the cell the bot marked |
| `validation/check-versions.sh` | CI (tier 1) | fails if any Dockerfile/compose/workflow disagrees with `versions.toml` |
| `validation/check-world-settings.sh` | CI (tier 1) | fails if a server profile hardcodes world settings instead of deriving them from the build |
| `validation/world-settings-entrypoint.sh` | — | the shared entrypoint the above guards; not invoked by hand |

## 8. Harness (`harness/`) · CI

The mineflayer bot the `validate` profile runs, plus the spec-0006 note bot and
the spec-0019 shot-calibration bot. It contains zero campaign logic — it reads `critical-path.json` and asserts.

```
npm --prefix harness run typecheck      # tsc --noEmit
npm --prefix harness test               # node --test 'test/**/*.test.ts'
npm --prefix harness start              # node src/run.ts <critical-path.json>  (compose does this)
```

`harness/src/note-bot.ts` is driven by `validation/playtest-note-flow.sh` and
`harness/src/rehearsal-bot.ts` by `validation/rehearsal-flow.sh`, never by hand.

## 9. Spikes (not the pipeline)

`tools/spike-jump-arc/run.sh` (`EULA=TRUE tools/spike-jump-arc/run.sh`) measures
1.21.11 jump kinematics on a throwaway server to feed
`docs/notes/jump-arc-model.md`. The compiler consumes the resulting **model**,
never this rig. Do not wire spikes into a skill.
