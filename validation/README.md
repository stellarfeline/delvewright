# validation/

Docker-compose entrypoint for running Delvewright the same way CI and prod do
(ADR-0005, ADR-0008, ADR-0010, spec-0003). "Works on my machine" is defined as
"this compose profile passes". Two profiles:

- **`play`** (available now) — a bare, pinned vanilla **1.21.11** server the owner
  can join by hand from her own client to check a delve at any time.
- **`validate`** (stub, comments in `compose.yaml`) — the full two-layer dynamic
  validation stack (server + PackTest runner + mineflayer bot, exit codes
  propagated to CI). Arrives with the rest of spec-0003 once the compiler emits a
  delve.
- **`playtest`** (spec-0006) — the same shipped delve image as `play`, plus the
  **creator overlay** mounted as an extra datapack (`/trigger dw.note` marks a spot
  in the server log). See "Creator playtest loop" below.

## One command (owner)

EULA acceptance is **your** action and is never hardcoded in this repo. Pass it in
your environment (this is Mojang's EULA: https://aka.ms/MinecraftEULA):

```sh
EULA=TRUE docker compose -f validation/compose.yaml --profile play up
```

Then join from a vanilla **1.21.11** client at `localhost:25565`. Stop with
`Ctrl-C`; `... --profile play down` removes the container (the world persists in the
`delvewright-play-data` volume). To join with a real Microsoft account instead of
offline mode, add `ONLINE_MODE=TRUE` (auth mode is still an open spec-0003 decision).

If `EULA` is unset, compose fails fast with a message telling you to set it — by
design.

## Creator playtest loop (spec-0006)

Build the delve, then run the `playtest` profile — the **same shipped delve image**
as `play`, plus the creator overlay mounted as an extra datapack and (optionally)
the creator opped:

```sh
delvec build crates/dsl/fixtures/valid/hello-world -o validation/delve-output

EULA=TRUE CREATOR_NAME=<your-mc-name> \
  docker compose -f validation/compose.yaml --profile playtest up --build
```

Join at `localhost:25565`. While playing, aim at something wrong and run
`/trigger dw.note` — the overlay stamps one machine-readable line into the server
log (`[DelveNote] pos=[x,y,z] area=… quests=… nearest_npc=…`) — then type your note
as a normal chat message. `CREATOR_NAME` ops you so you can `/tp` and inspect; leave
it unset to skip opping (the note trigger works either way). It must be a
**resolvable** Minecraft name — itzg looks the op up online, so a fake offline name
fails to boot.

The overlay is **playtest-only**: it is never baked into the shipped delve image
(`Dockerfile.delve` copies only `datapack/` + config), mounted here at runtime, and
CI asserts its absence from the image (same exclusion guarantee as PackTest).

After the session, turn the log into a report with the harvester:

```sh
docker compose -f validation/compose.yaml --profile playtest logs --no-color \
  > playtest.log
cargo run -p delvewright-orchestrator --bin delve-harvest -- \
  playtest.log validation/delve-output/creator-datapack/layout.json \
  -o playtest-report.json
```

`delve-harvest` pairs each `[DelveNote]` stamp with the nearest creator chat line
(±60s, preferring the line *after* the stamp) and resolves area→prefab and the live
objective states into per-quest `quest_state` via the overlay's `layout.json`. The
report is the contract input of the future `/revise-delve` skill (spec-0006 §4).

### Machine-tested end to end

`validation/playtest-note-flow.sh` runs the whole loop non-interactively — boots the
`playtest` server, drives one note capture with the mineflayer note-bot
(`harness/src/note-bot.ts`: join → `/trigger dw.note` → chat a multilingual fixture
→ disconnect), harvests the captured log, and asserts the report contains the note
with the correct area + quest state:

```sh
EULA=TRUE validation/playtest-note-flow.sh
```

**CI placement (spec-0006 acceptance).** This is a **tier-3 / local** test (wired in
`release.yml`), not tier 2: it boots a full server *and* a bot (~2–3 min), beyond
tier 2's ~2-min budget. Every-push coverage of the mechanism already lives in tier 1
— the harvester's parsing/pairing/report logic (`crates/orchestrator` unit tests,
incl. Chinese note text) and the overlay emission + byte-determinism
(`crates/compiler` tests). Only the live wiring is deferred to tier 3.

## What works today vs with M1 integration

**Today (bare pinned server):**

- Boots vanilla 1.21.11 via the `itzg/minecraft-server` image, which downloads the
  pinned server jar at runtime (never baked into a layer — the ADR-0010 EULA-safe
  pattern).
- Empty world generated on first boot. No delve is loaded yet: the compiler
  (spec-0002) does not exist, so there is nothing to mount.
- Port bound to `127.0.0.1` only — never world-reachable.

**Arrives with M1 integration:**

- The compiler's build output (`<out>/`: `manifest.json`, `datapack/`, `server/`,
  `packtest-datapack/`, `critical-path.json`) is mounted into the server so it
  loads the compiled delve. The placeholder mount path is documented (commented) in
  `compose.yaml`: `./delve-output/datapack -> /data/world/datapacks/<campaign-id>`.
- The `validate` profile is filled in: `packtest-runner` (exit code = failed tests)
  and `bot` (the `../harness` mineflayer runner reading `critical-path.json`,
  exiting 0/1). The tooling-mod overlay (PackTest + Fabric) is layered on only at
  compose time and must never leak into the shipped delve image.
- `docker compose --profile validate up` then reproduces CI locally with exit codes
  propagated (spec-0003 acceptance criteria).

## Harness

The mineflayer bot that the `validate` profile runs lives in [`../harness`](../harness).
It parses the compiler's `critical-path.json` contract and drives the run; it
contains **zero** campaign-specific logic (spec-0003). See that directory's code and
tests. Current status: parser + sequencer + a mineflayer executor skeleton
(`reach`/`assert-complete` implemented; `select-class`/`talk-to` stubbed pending the
dialog-vs-`tellraw` interaction-channel decision).
