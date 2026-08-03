# validation/

Docker-compose entrypoint for running Delvewright the same way CI and prod do
(ADR-0005, ADR-0008, ADR-0010, spec-0003). "Works on my machine" is defined as
"this compose profile passes". Four profiles, all driven by one `delvec build`
output tree (`validation/delve-output`):

- **`play`** — the shipped delve image, pinned vanilla **1.21.11**, joinable by
  hand from the owner's own client.
- **`playtest`** (spec-0006) — the same shipped delve image as `play`, plus the
  **creator overlay** mounted as an extra datapack (`/trigger dw.note` marks a spot
  in the server log). See "Creator playtest loop" below.
- **`validate`** — server + the mineflayer critical-path bot; the bot's exit code
  is the profile's (`--exit-code-from bot`).
- **`packtest`** — the generated PackTest suite on the pinned tool server
  (`--exit-code-from packtest`).

The tooling-mod overlay (PackTest + Fabric) and the creator overlay are layered on
at compose time only and never leak into the shipped delve image; CI asserts their
absence. Every tool in the repo, including the scripts below, is indexed in
[`../docs/reference/tools.md`](../docs/reference/tools.md).

## Sharing the Docker host (mutex + isolation)

One Docker host, one port 25565, one `delvewright-server` container name — and
sometimes a human playing on it. Two rules, both enforced by
[`mutex.sh`](mutex.sh), keep agents from colliding with each other or with the
owner:

**1. Take the mutex.** `mutex.sh` is the only sanctioned way to claim the stack:

```bash
source validation/mutex.sh
dw_mutex_acquire "my-name" || exit 1   # exits non-zero if someone else holds it
trap dw_mutex_release EXIT             # idempotent; releases only your own lock
```

Acquisition is the return value of `mkdir` — never inferred from the lock
directory existing, which is true precisely when *someone else* holds it. The
lock carries a `HOLDER` file naming its owner. **`owner-play-session` is
sacred**: `dw_mutex_assert_not_owner_session` refuses all Docker work while a
human is playing, and acquire will not wait on it or steal it, however stale it
looks. Never install a teardown trap before acquisition succeeds.

**2. Isolate your project.** The mutex serialises access; isolation is what makes
a mistake survivable. Any worker live-server work runs in its **own compose
project** (`docker compose -p dw-worker-<unique>`, or `docker run` with a unique
`--name`), publishes **no host binding on 25565** (use the compose network,
`docker exec … rcon-cli`, or a distinct high port — 25565 belongs to the owner's
client), and tears down **only its own project** (`docker compose -p
dw-worker-<unique> down -v`) — never a bare `docker compose down`, never
`docker rm` on a container it did not create.

Both exist because of a real incident (2026-08-02): a hand-rolled waiter whose
"did I get the lock?" guard tested directory existence fell through against the
owner's held lock and ran a teardown that — via the pinned `container_name` —
destroyed her live play session and its world volume mid-playtest.

## World fidelity (all profiles)

Every server here boots the world the **compiler** declared: `world-settings-
entrypoint.sh` reads `difficulty`, `level-seed`, `level-type` and
`generator-settings` out of the build's `server/server.properties` and exports them
before itzg starts. `Dockerfile.delve` bakes that script into the shipped image; the
`packtest` service mounts the same file. No profile may hardcode those four —
`validation/check-world-settings.sh` (CI tier 1) fails on drift or on a re-hardcoded
value. This exists because hardcoding has twice made a server run a world the
campaign never declared: the shipped image booting a `horizon: ocean` delve as a
void, and the PackTest runner testing a void superflat while the delve shipped an
ocean one.

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

## What the stack does today

- Boots vanilla 1.21.11 via the `itzg/minecraft-server` image, which downloads the
  pinned server jar at runtime (never baked into a layer — the ADR-0010 EULA-safe
  pattern). Port bound to `127.0.0.1` only — never world-reachable.
- Every profile serves one compiler build output (`manifest.json`, `datapack/`,
  `server/`, `packtest-datapack/`, `critical-path.json`), so `--profile validate`
  and `--profile packtest` reproduce CI's dynamic tiers locally with exit codes
  propagated (spec-0003 acceptance criteria).
- **Re-runs**: `validation/fresh-volumes.sh` tears the stack down and proves the
  world volumes are gone. A persisted volume keeps completed objectives completed,
  which fails a "fresh" playthrough for reasons unrelated to the delve — run it
  before every repeat playthrough.
- **Shot sets**: `validation/render-shots.sh <build-dir> [out-dir]` turns a build
  output into the Chunky scene set plus the shot index (`delve-render scene` +
  `index`) for visual review, including the first-person player-POV shots.

## Harness

The mineflayer bot that the `validate` profile runs lives in [`../harness`](../harness).
It parses the compiler's `critical-path.json` contract and drives the run; it
contains **zero** campaign-specific logic (spec-0003). See that directory's code and
tests. Current status: parser + sequencer + a mineflayer executor skeleton
(`reach`/`assert-complete` implemented; `select-class`/`talk-to` stubbed pending the
dialog-vs-`tellraw` interaction-channel decision).
