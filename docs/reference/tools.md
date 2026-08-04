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
| `tools/i18n-translate.py` | agent | `python3 tools/i18n-translate.py <campaign-dir> --lang <code> [--config f] [--delvec cmd] [--batch-size n] [--dry-run] [--force] [--no-validate] [--reflect\|--no-reflect]` — external OpenAI-compatible API, generation-time only; `--reflect` runs the three-step translate → critique → revise pass; see [`i18n.md`](i18n.md) |
| `tools/skin/` (`delve_skin`) | agent | `python -m delve_skin all <cast.json> --skins-dir D --catalog-dir D --preview-dir D [--id ID] [--scale N]`, or the `build` / `preview` / `catalog` stages individually. Needs its own venv (`pip install -r tools/skin/requirements.txt`); see [`../../tools/skin/README.md`](../../tools/skin/README.md) |
| `tools/check-dw-codes.py` | CI | `python3 tools/check-dw-codes.py` — asserts the DW catalog in `compiler.md` matches `crates/**/*.rs` both ways, and that every code has a test |
| `tools/check-doc-dupes.py` | CI | `python3 tools/check-doc-dupes.py [path …]` — merge-artifact gate over `docs/**/*.md` + `README.md`: no two body rows in one markdown table share a first-cell key, no heading repeats within a file, no git conflict markers. Kills the class that put `shortcuts[]` in the stage-5 table twice (owner finding 2026-08-03). Same-key rows in *different* tables are fine; a genuine same-table collision means restructure the table, not allowlist it |
| `tools/check-worker-override.py` | CI | `python3 tools/check-worker-override.py` — worker-isolation coverage gate: every service in `validation/compose.yaml` that pins a `container_name` or publishes `ports` must be reset (`!reset`) in `validation/worker-override.yaml`. Container names and host ports are Docker-GLOBAL, so `-p dw-worker-<x>` does not isolate them; the omission cost a run twice (`server` #190, then `bot`) |
| `tools/check-harness-dsl-version.py` | CI | `python3 tools/check-harness-dsl-version.py` — sync gate: the compiler's `SUPPORTED_DSL_VERSION` (`crates/dsl/src/envelope.rs`) must be a member of the harness's `SUPPORTED_DSL_VERSIONS` allowlist (`harness/src/critical-path.ts`). Nothing else relates the two files; spec-0026 moved the compiler to `0.9.0` while the harness allowlist still ended at `0.8.0`, and the bot tier refused every campaign at the version gate after the server booted and the bot connected (task #157) |
| `tools/check-storybook-version.py` | CI + agent (**mandatory** at the `/new-delve` storybook step) | `python3 tools/check-storybook-version.py [--campaigns <dir>]` (default `campaigns/campaigns`) — every campaign storybook (content repo `campaigns/<id>/README.md` + one `README.<code>.md` per declared language) opens with `> **Requires delve engine <X> or newer** — last verified with delvec <Y>.`, within the first 10 lines, exactly once, byte-identical across editions. `<X>` must equal the MAX `dsl_version` over the campaign's six stage documents — the drift this gate exists for (owner directive, task #147: the marker is the ONE internal-machinery item allowed in a player-facing README, so it must be TRUE); `<Y>` may not exceed the engine's own `DELVEC_VERSION`. Missing, malformed, buried, duplicated, or mismatched = red; an empty campaigns root is red too (a vacuous pass is worse than a failure). Campaigns blocked by an in-flight content PR sit in the script's `ALLOWLIST` with the blocking PR and its removal condition, are PRINTED on every run, and go red the moment their marker becomes correct. Runs in CI as `campaign storybooks (engine-version marker)`; the content repo's own campaign CI (task #137) can run this same script against a pinned engine checkout, the way `prefab-audit.yml` there already builds `delve-admit` from one |
| `tools/extract-sound-registry.py` | maintenance | `python3 tools/extract-sound-registry.py <registries/data.min.json> <out.json>` — regenerates the compiler's sound registry for a new MC pin (positional args only, no `--help`) |
| `tools/extract-item-stack-sizes.py` | maintenance | `python3 tools/extract-item-stack-sizes.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-stack-sizes-1.21.11.json`, the item→`max_stack_size` table `DW0436` reads, for a new MC pin (positional args only). Pins and checks the source SHA-256; refuses to default a missing component rather than assuming 64 |
| `tools/extract-item-combat-stats.py` | maintenance | `python3 tools/extract-item-combat-stats.py <item_components/data.min.json> <out.json>` — regenerates `crates/compiler/data/item-combat-1.21.11.json`, the item→`attack_damage`/`attack_speed`/`armor`/`armor_toughness`/`nutrition` table the spec-0023 winnability arithmetic reads (`DW0472`, `DW0474`), for a new MC pin (positional args only). Pins the source SHA-256 and refuses any non-`add_value` modifier rather than mis-summing it |
| `tools/extract-damage-types.py` | maintenance | `python3 tools/extract-damage-types.py <damage_type/data.min.json> <tag/damage_type/data.min.json> <out.json>` — regenerates `crates/compiler/data/damage-types-1.21.11.json`, the damage-type→`{bypasses_armor, scaling}` table `DW0473` reads (positional args only). The finding it pins: `damage-players` emits `/damage` with no attacker, so an Easy campaign's scripted hits are NOT halved — only `scaling: always` types scale |
| `tools/extract-font-metrics.py` | maintenance | `python3 tools/extract-font-metrics.py <client.jar> …` — regenerates the font metrics behind the DW0330 text-fit lint (positional args only, no `--help`) |
| `tools/playtest-server.sh` | human | `tools/playtest-server.sh up <campaign-dir> [--lang L] [--prefabs D] [--delvec BIN] [--name N] [--out D]` / `down [--name N]` / `status` — builds a campaign and serves it as a local throwaway itzg container for the owner's direct-connect playtest (the ONE sanctioned host-25565 binding; validation workers never bind it). `up` rcon-verifies dw objectives + a `dw_npc` entity, clears the sidebar, installs the resource pack when `DELVEWRIGHT_RESOURCEPACKS_DIR` is set, and prints the connect address; `down` is the server-lifecycle teardown the moment feedback arrives. Refuses to start over an existing binding |

## 7. Validation stack (`validation/`)

Docker compose is the CI-equivalent environment (CLAUDE.md *Environments*). All
profiles boot the world the compiler declared, via the shared
`world-settings-entrypoint.sh`. Prose:
[`../../validation/README.md`](../../validation/README.md).

| Profile | Class | Command | What it is |
|---|---|---|---|
| `play` | human | `EULA=TRUE docker compose -f validation/compose.yaml --profile play up` | the shipped delve image, joinable at `localhost:25565` |
| `playtest` | human | `EULA=TRUE CREATOR_NAME=<mc-name> docker compose -f validation/compose.yaml --profile playtest up --build` | `play` plus the creator overlay: `/trigger dw.note` stamps the log for `delve-harvest` |
| `validate` | agent | `EULA=TRUE docker compose -f validation/compose.yaml --profile validate up --build --abort-on-container-exit --exit-code-from bot` | server + mineflayer critical-path bot. Two labelled ladder stages once the build carries a `validation/combat-plan.json` (spec-0023): `critical-path` (the whole delve, with bounded **combat-assist** windows at each encounter) and `die-retry` (≥2 scripted deaths per encounter, proving respawn → return → re-engage with no lost progress). The run writes `validation/run-out/run-report.json` — an `encounters` block (per encounter: assist policy and the phase the run reached), every assist window with its encounter id and ticks, every death trial (recorded when the death is TAKEN, so an aborted run still carries it; each says whether its loop reached a verdict and what was waiting at the end of it — `outcome`: `re-engaged` (hostiles are back) and `cleared-before-retry` (nothing left to fight, objective already complete) both PASS, `stranded` (nothing left to fight, objective unfinished) is a soft lock and reds the run). The bot **performs the path's `rest` steps** (compiler #220): it walks to the bonfire, RIGHT-CLICKS the `dw_bonfire_<i>` affordance — which is what enables the `dw.rest` trigger; chatting the command alone is a silent no-op — then sends the step's command. `rests[]` in the report lists the fires actually rested at. Before scripting a death, the stage asserts the encounter's governing checkpoint is ARMED, and distinguishes three states. **Armed** → proceed. **Unarmed** (it sits on a bonfire nobody has rested at) → the run REDS with a precondition finding naming that bonfire and takes NO death; a death there would measure the delve against world spawn (bell round 3), which is the harness's own gap. **No governing checkpoint at all** (the plan names none fired before the fight — post-#223 the truthful reading whenever the only nearby checkpoint is armed by the encounter's own kill step) → the death is skipped and the stage records the ADVISORY `no governing checkpoint — die-retry cannot prove safe death here`: every death there is a full restart of the delve, which is a content fact about where the campaign puts its rest points, and `DW0379`/`DW0315`/`DW0316` own that judgement rather than the bot. Both gaps also exclude the encounter from the coverage check — the precondition already says why the loop is unproven. The presence check counts BY TAG (task #123): it calls the compiler's `wave_census_<wave>` — named per encounter in `combat-plan.json`'s `census` block, never re-derived here — and reads the answer off the anchored marker channel (`[dw:census …]` totals, one `[dw:censusmob …]` per mob with position and health). It still SETTLES (up to 6s) rather than sampling the instant the walk back ends, because a re-seat takes ticks to land; `reengage.settle_ms` / `nearest_blocks` / `farthest_blocks` record what it waited for and where it found them. Before this the probe counted SILHOUETTES — every entity the client tracked, no distance filter, anything taller than half a block — so the drowned bell's ambush husks 57 blocks away at another encounter counted as members of whichever wave was being measured, and a 2-mob wave read as 4 standing (#230). A census that never answers is an ABORTED trial naming the broken probe, never a zero: a silent zero would read as `stranded` and blame the delve for the harness's own fault. A `respawns_on_rest` wave additionally owes RE-SEAT FIDELITY: it must come back at the declared count, as all-new entities, at full health. A survivor carried across a life (`carried_over > 0`), a short count, or a mob below full health reds the run — a retry must never let the party chip a wave down one swing per death (owner ruling 2026-08-03). `carried_over` is decided by IDENTITY: the ladder calls `wave_brand_<wave>` before each scripted death, stamping the wave's living mobs with a tag no re-summon can carry, and the next census counts how many still wear it. Health and its maximum come from the server's own `Health` and `max_health` inside that census, so `damaged` no longer depends on a max-health attribute vanilla never puts on the wire. The kill loop's own "this fight is over" tests are guesses made from shapes — a mob the bot hit winked out near the anchor; everything it engaged is down and nothing hostile is close — so since task #124 none of them may END the step without the census agreeing. On the drowned bell the bot killed one of `ambush/the-rafters`' husks at the belfry, counted it as the Bellkeeper (`confirmed kill: husk#232 (1/1)`) and walked away from a live wither skeleton; `obj/the-keeper` never completed, so `quest/ring-it-home` was never armed, so the next step's `interact` click was adjudicated against an unarmed quest and spent. The guesses still DRIVE the fight (the bot can only swing at what it can see); the census is what ends it. The `die-retry` stage passes only when every planned encounter has its ≥2 COMPLETED trials — an encounter it engaged and proved nothing at, or never reached, is a red stage, never a silent pass. **Assist windows** (spec-0023 §3, corrected by task #121): the die-retry stage takes them too. It is assisted into melee range for the approach, for the mid-fight trade, and for the walk back plus the re-engage probe — every segment where the bot must SURVIVE to make a measurement — and takes the scripted death itself with the assist CLEARED, so `/damage @s 1000` is lethal without any argument about resistance arithmetic. Each segment is its own opened/closed/named window, so expect several per encounter and read `reason` to tell them apart. Before this, the stage walked to within 3 blocks of a live encounter bare: on the-drowned-bell run six the wave killed the bot before it could script death 1, the stage reported 0/2 trials beside `assist_windows: []`, and bot fencing skill was silently gating the one proof the stage exists to make. Fencing is telemetry, never the gate. **Trial field semantics** (task #120 — every one of these is a MEASUREMENT, and the fields may never contradict each other): `respawn_pos` is the bot's own position read the instant the respawn settles, and `at_checkpoint` is derived from it — nothing between the respawn and that reading is allowed to move the bot, which is why the post-death re-arm only re-equips the kept kit and never replays `select-class` (`class_apply_<c>` ends in `teleport @s <campaign entry point>`, so replaying it warped the bot back to the start of the delve and made every `respawn_pos` a lie one second later). `kit_kept` says the kit survived the death — the delve seals `gamerule keep_inventory true`, so an empty bag reds the trial. `returned` is the walk from that measured respawn back to the encounter. `re_engaged` / `reengage` / `outcome` are observations taken AT the encounter and are recorded **only when `returned`**: a trial that never got back reports `re_engaged: false`, `reengage: null` and `outcome: unproven`, because "did not look" and "looked and found nothing" are different facts and neither is a pass. `completed` says only that the loop ran to its verdict; an abandoned trial is still in the array and still reds. The bot is opped for exactly three harness commands (`/damage @s`, `/effect give @s minecraft:resistance`, and `/function <ns>:wave_{census,brand,unbrand}_<wave>` — the compiler-owned census probe, whose ids come from the plan). `DELVEWRIGHT_DIE_RETRY=0` skips the stage for local iteration and the report records that it was SKIPPED, never that it passed. The report also carries the compiler's **floor-gate ledger verbatim** (`floor_gate.covered` / `floor_gate.not_covered`, each uncovered entry with the compiler's own reason) and one `actors[]` row per tier-declaring stage-5 actor — fought (with `outcome`, `swings`, `after_objective`) or not (with the reason). `floor_gate.present: false` means the build shipped no ledger at all (a `delvec` older than #222) and is deliberately distinct from an empty one: "this campaign bills nothing hard" and "this build cannot tell you" are different facts. A **trigger-driven step that times out** (`talk-to`, `interact`) now names which side swallowed it (task #144): the bot is opped, so vanilla's own answer to the `/trigger` it sent arrives on the chat stream, and the failure line repeats it — *the server ANSWERED …* means the trigger reached the delve and a datapack guard consumed it without completing anything (a re-used world whose scoreboard already carries the objective is the classic cause — `fresh-volumes.sh --project`, then re-run, before suspecting the content: it cost three misattributed red runs in island round 13 and another round here), while *the server never answered …* means the command never got there and the fault is the harness's. Diagnostics only: the step still fails on its objective marker either way. Authoring note: an actor anchored inside a LATER objective's completion zone will complete that objective during the fight, which the endgame-discipline check then reds — stage the fight where the party already stands |
| `packtest` | agent | `EULA=TRUE docker compose -f validation/compose.yaml --profile packtest up --exit-code-from packtest` | headless PackTest suite on the tool server. `DELVE_OUTPUT` (default `./delve-output`) + `PACKTEST_CONTAINER` boot a **different** build tree — the generated suite is per-campaign, so a template class is only proven live by a campaign that emits it (CI runs extra passes for template classes hello-world cannot emit: `crates/compiler/tests/fixtures/cast-ledger` for spec-0020's root-swap/bark/explicit-none templates, and `crates/dsl/fixtures/valid/keep-trial` for the `interact` verb templates — `verb_interact` and `verb_interact_held`, the held-vs-carried proof — since hello-world has no `interact` objective at all; `crates/compiler/tests/fixtures/souls-bonfire` for the spec-0016 §1 rest loop — `souls_bonfire_rest`/`_reseat`/`_options`, `souls_reseat_stationed` and `wave_census`; and `crates/compiler/tests/fixtures/souls-td-lanes` for the §6 lane family — `souls_td_patrol_nbt`/`_lane_march`/`_lane_release`/`_lane_reseat`/`_aggro_edge`). See `validation/README.md` "Running a second campaign through `packtest`" |

Shell entry points:

| Script | Class | Purpose |
|---|---|---|
| `validation/mutex.sh` | agent (**mandatory**) | the only sanctioned way to claim the validation stack. `source validation/mutex.sh`, then `dw_mutex_acquire <name> [wait-s]` / `trap dw_mutex_release EXIT` / `dw_mutex_assert_not_owner_session`. `dw_mutex_release` only works in the shell that acquired (agent tool calls never share shells) — cross-shell coordinators release with `dw_mutex_release_named <holder>`, which matches the HOLDER name exactly and refuses to free `owner-play-session` while the play-profile container is running. Acquisition is `mkdir`'s return value, never inferred from the lock directory existing; the lock names its holder in `HOLDER`, and **`owner-play-session` is sacred** — refuse all Docker work, never wait on it, never steal it. Pair with worker isolation: own compose project (`-p dw-worker-<unique>`), no 25565 host binding, tear down only your own project — and prove that teardown with `validation/fresh-volumes.sh --project dw-worker-<unique>`, since `down -v` on its own leaves the world volume alive whenever an exited container still holds it. See [`../../validation/README.md`](../../validation/README.md) "Sharing the Docker host" |
| `validation/warden-probe.sh` | agent (spike) | `[POLL_SECONDS=n] [WATCH_SECONDS=n] [CONTAINER=name] validation/warden-probe.sh` — measures what a summoned 1.21.11 warden actually does (dig-down timing, `dig_cooldown`/`anger` NBT, difficulty effects) against a **throwaway** pinned server, never the shared stack. Refuses to run while the mutex reads `owner-play-session` |
| `validation/fresh-volumes.sh` | agent | tear a stack down and **prove** its world volumes are gone. Two modes and no default: `--project <compose-project>` removes only that compose project's containers and volumes (what worker isolation requires; honours `COMPOSE_PROJECT_NAME`), `--all` is the daemon-wide sweep and refuses while the mutex reads `owner-play-session`. With neither flag it exits 2 rather than guess — the daemon-wide sweep matches `server-data$` across **every** project and force-removes the pinned `delvewright-*` container names, i.e. it destroys the owner's and other workers' worlds. Run it before any re-run of the bot ladder: `docker compose -p <proj> … down -v` silently leaves `<proj>_server-data` behind whenever an exited container of that project still holds it, and the stale volume carries the scoreboard — so the re-run starts with objectives already complete and the bot reports a **false CONTENT failure** (three misattributed red runs, island round 13) |
| `validation/render-shots.sh <build-dir> [out-dir]` | agent | turn a build output into the Chunky scene set + shot index (`delve-render scene` + `index`), including the first-person POV shots |
| `validation/playtest-note-flow.sh` | CI (tier 3) | `EULA=TRUE validation/playtest-note-flow.sh` — drives the whole spec-0006 note loop non-interactively and asserts the report |
| `validation/rehearsal-flow.sh` | CI (tier 3) | `EULA=TRUE validation/rehearsal-flow.sh` — drives the whole spec-0019 calibration loop (`dw.aim`/`dw.faster`/`dw.mark`/`dw.done` → harvest → `delvec calibrate`) and asserts the patch resolves back to the cell the bot marked |
| `validation/branch-runs.sh` | agent (**required for a branching campaign**) + CI (tier 3) | `EULA=TRUE [DELVEWRIGHT_BRANCHES=…] [DW_COMPOSE_PROJECT=dw-worker-<x>] validation/branch-runs.sh` — spec-0025 §3 branch runs: walk every branch the tier selects, **each in its own fresh world** (party progress only moves forward, so a second branch needs a second world), and merge the per-branch run reports into `validation/run-out/branch-runs.json` — per branch: ran/skipped-with-reason/**INFRA-FAILED** and the result (an attempted branch whose compose run exited without writing any run report renders as an infra failure — a validation-infrastructure fault, distinct from a red run and from a tier skip; task #117). `DW_RUN_OUT` relocates the merged + per-branch reports; the bot's own report is always read from the compose-mounted `validation/run-out` and FILED under `DW_RUN_OUT` (the mount does not follow the env var). The branch set and the selection come from the build's `validation/branch-plan.json` via `harness/src/branch-select.ts`, i.e. the same code the run uses, so a tier can never select a branch the run then refuses. `DW_COMPOSE_PROJECT` runs it under worker isolation (own project + `worker-override.yaml`, teardown via `fresh-volumes.sh --project`). One critical-path run proves ONE storyline; this is what makes "provably completable" quantify over branches |
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

Run-shaping environment (all read by `src/run.ts`; the compose `validate` profile
forwards every one of them, so they can be set on the `docker compose` command line):

| Variable | Effect |
|---|---|
| `DELVEWRIGHT_RUN_REPORT` | Path to write the spec-0023 run report to. Unset = the pre-spec-0023 stderr-only run |
| `DELVEWRIGHT_DIE_RETRY` | `0` skips the die-retry stage (local iteration only). Default ON whenever a combat plan is present |
| `DELVEWRIGHT_RETRY_ON_DEATH` | `1`/`true` lets the sequencer retry a step once after an unscripted death (spec-0008) |
| `DELVEWRIGHT_RUN_TIMEOUT_MS` | Hard wall-clock budget for the whole run (default 20 min, forwarded by the compose `bot` service). **Raise it when the die-retry stage is on**: two scripted deaths per encounter add a respawn, a re-arm and a walk back to every fight |
| `DELVEWRIGHT_BOT_USERNAME` | The bot's name; feeds the server's `DELVE_OPS_OFFLINE` seed (offline-UUID ops.json — never itzg `OPS`, which would resolve the name via Mojang PlayerDB and abort on an offline-only name) or the assist and scripted-death commands are silently refused |
| `DELVEWRIGHT_ACTOR_FLOOR` | `0` skips the **actor floor gate** (local iteration only); the report then records each tiered actor as skipped with that reason, never as measured. Default ON whenever the build's combat plan declares a tiered actor (`actors[]`, DSL v0.8). ON, the run gives every `elite`/`boss` actor whose `unleash-actor` beat hangs off an objective the path completes ONE honest unassisted attempt, right after that objective's marker arrives, and reports the outcome (`won-first-try` → the inverted floor-gate advisory; `lost` / `timed-out` / `body-not-found` say nothing, and never read as a pass). It takes **no assist**: nothing downstream waits on an actor fight, so there is no obligation to win one. An actor unleashed only by an ambient trigger (`strike`/`use`/`approach`/`strike-npc`) or by a quest completion is reported as NOT exercised with the reason — the campaign does not schedule those, so the bot may not invent a moment for them |
| `DELVEWRIGHT_BRANCHES` | **Which branches this run is answerable for** (spec-0025 §3). `all` (default, the release tier: every enumerated branch), a comma-separated list of branch ids (the narrowed tier), or `from-diff` — the PR tier spec-0025 describes, which **refuses**: the diff→branches mapping is compiler-side and is not emitted yet, and degrading to `all` would lie about cost while degrading to nothing would lie about coverage. A branch this tier excludes appears in the run report with the reason it did not run; a skipped branch is NAMED, never silent. A list naming a branch the build does not declare is an error, not a silent skip. Ignored for a build with no `validation/branch-plan.json` |
| `DELVEWRIGHT_BRANCH` | **Which single branch THIS session walks.** The run then reads `validation/branch-path-<branch>.json` (the ordinary critical-path contract, computed under that branch) instead of `critical-path.json` — navigated leg-by-leg through that branch's own `validation/branch-waypoints-<branch>.json` (task #117; absent → single-goal fallback reported LOUDLY in stderr + the run report, never silently), and asserts the path really takes the choices that ENTER the branch — so a run cannot report branch coverage while having walked somebody else's storyline. One branch per invocation by construction: party progress only ever moves forward, so a second branch needs a second WORLD (`validation/branch-runs.sh` is that loop). Unset = the ordinary single-path run, unchanged. Refused if the branch is not in the build, or not one `DELVEWRIGHT_BRANCHES` selected |

An `interact` step whose `critical-path.json` entry carries `requires_item` puts
that item in the bot's **mainhand** before it sends the trigger
(`src/held-item.ts`), because `requires_item` is held, not carried
([`compiler.md` §objectives](compiler.md)). Actuation only: the guard stays in the
datapack, and a bot that cannot hold the item still fails the step on its objective
marker — but the log now says which of the two happened instead of showing a bare
30s timeout.

## 9. Prefab generators (`prefabs/*-generator`, `prefabs/generator`) · agent + CI

The tileset libraries are **generated, not hand-built**. Five separate Cargo
workspaces, deliberately outside `crates/` so none of them can enter the shipped
`delvec` and no existing `.nbt` moves (ADR-0006). All five share one CLI —
`<out_dir>`, which is the content repo's `prefabs/` when you mean to re-export:

```sh
cargo run --release --manifest-path prefabs/<gen>/Cargo.toml -- <out_dir>
```

| `<gen>` | binary | tileset | doc |
| ------- | ------ | ------- | --- |
| `generator` | `keep-prefab-gen` | `keep-*` (the original interior set) | `prefabs/keep-tileset.md` |
| `cave-generator` | `cave-prefab-gen` | `cave-*` | `prefabs/cave-tileset.md` |
| `island-generator` | `island-prefab-gen` | `island-*` set-pieces | `prefabs/island-tileset.md` |
| `island-terrain-generator` | `island-terrain-gen` | `island-*` terrain | `prefabs/island-tileset.md` |
| `tidal-keep-generator` | `tidal-keep-gen` | `tk-*` (souls set) | `prefabs/tidal-keep-tileset.md` |

Each generator prints the `pool/*` block to merge into the content repo's
`pools.json` — printed, never written, because every `*.json` in that directory
is parsed as prefab metadata and a stray snippet is `DW0346`.

**The invariants are the point.** Every debugging lesson these tilesets have cost
is pinned as an `assert!` in the generator (route walkability, stair-flank
sealing, anchor sanity, sightlines, gravity substrate, redstone support), so
*running* a generator is the gate: it either emits or panics.

Invariants true of **every** tileset live once, in
[`../../prefabs/invariants.rs`](../../prefabs/invariants.rs), source-included by
all five (`#[path = "../../invariants.rs"] mod invariants;` — an include, not a
dependency, so the workspaces stay independent). Today: **distress embeds, it
never stacks** (`assert_distress_never_stacks`) — a walkable stair tread may
carry nothing but air or a declared attachment (railing, hardware, light fitting,
plant), because wear on a walked surface belongs *in* the surface, as a weathered
variant of the same shape (`invariants::weathered`), never as a lump on top of
it. Owner playtest, island round 13: stray stone sitting on the cave-mouth steps.
The shared file carries its own unit tests — including the cases that prove the
gate *fails* — run by the same CI job. Debug flags, all
`tidal-keep-generator`: `TK_DEBUG_LIGHT=1` (per-region measured light + darkest
cell), `TK_PROBE=<salt>,<x>,<y>,<z>` (labelled block dump), `TK_DEBUG_STAIRS=1`
(every flank the seal pass closed).

CI (`prefab-generators` job, tier 1) runs all five twice into separate trees on
every PR: a panic fails the job, and the two trees must be byte-identical
(ADR-0006). Wired 2026-08-03 — before that nothing in CI compiled these
workspaces, which is how a tileset with 132 reversed stair blocks (`DW0430`)
reached an owner playtest through a green pipeline. `clippy -D warnings` is not
yet part of that job (`prefabs/generator` carries two legacy style lints).

**Re-export loop**: edit the generator → run it into `campaigns/prefabs/` → the
`.nbt`/`.json` diff is content-repo work, the source diff is engine work, and the
two land as a pair.

## 10. Spikes (not the pipeline)

`tools/spike-jump-arc/run.sh` (`EULA=TRUE tools/spike-jump-arc/run.sh`) measures
1.21.11 jump kinematics on a throwaway server to feed
`docs/notes/jump-arc-model.md`. The compiler consumes the resulting **model**,
never this rig. Do not wire spikes into a skill.
