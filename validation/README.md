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
  is the profile's. Entry point: `validation/bot-run.sh --project <id>`.
- **`packtest`** — the generated PackTest suite on the pinned tool server. Entry
  point: `validation/packtest-run.sh --project <id> [--output <tree>]`. `--output`
  boots a **different** build tree (default `./delve-output`) — a campaign only
  exercises the templates it actually emits, so proving a template class means
  running the profile over a campaign that has one. CI does exactly that for
  spec-0020's cast-ledger templates; see "Running a second campaign through
  `packtest`" below.

The tooling-mod overlay (PackTest + Fabric) and the creator overlay are layered on
at compose time only and never leak into the shipped delve image; CI asserts their
absence. Every tool in the repo, including the scripts below, is indexed in
[`../docs/reference/tools.md`](../docs/reference/tools.md).


## The owner-play path is gated

`validation/owner-play.yaml` is the only compose file that publishes host 25565,
which makes it the only compose path to the owner's client — so it carries a
`staging-admission` service that `server` and `playtest` both
`depends_on: service_completed_successfully`. It runs
`validation/staging-admission.sh`, which refuses any build tree not admitted by
`tools/staging-gate.py` for THAT EXACT tree (the token binds the tree's
`manifest.json` sha256). Mint the token on the host first:

```sh
python3 tools/staging-gate.py --campaign <campaign-dir> --build validation/delve-output
EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
    --profile play up
```

Worker ladders never name `owner-play.yaml` and are unaffected. Full rationale —
including why this is bound to the staging event instead of a checklist line —
is `docs/reference/playtest-methodology.md` rule 7.

## Sharing the Docker host (isolation by construction)

**Run your ladder in its own compose project. There is nothing to queue for.**

```bash
EULA=TRUE validation/packtest-run.sh --project dw-worker-<id>
EULA=TRUE validation/bot-run.sh      --project dw-worker-<id>
EULA=TRUE validation/branch-runs.sh  --project dw-worker-<id>
validation/fresh-volumes.sh          --project dw-worker-<id>   # teardown, proven
validation/reclaim-ladder-images.sh                             # sweep, dry by default
```

`--project` is **required** on every one of them; an invocation without it exits
non-zero and says why. That is the whole protocol.

### Why there is no lock any more

`docker compose -p <project>` isolates containers, volumes and networks. It does
**not** isolate the two things that are global to the Docker daemon: pinned
container names and published host ports. `compose.yaml` used to carry both
(`container_name: delvewright-server`, `127.0.0.1:25565:25565`), so every caller
aimed at the same container and the same port and had to be serialized behind
[`mutex.sh`](mutex.sh) — a lock over the entire validation stack.

That lock cost more than it bought: worker ladders queued on each other for no
reason, and an island worker once waited **30+ minutes** behind a holder whose
session had **zero containers running**. The lock outlived the work and nothing
could tell.

So `compose.yaml` now pins no container name and publishes no port, and
`tools/check-compose-isolation.py` (CI) fails if one comes back. Two ladders on
one host are independent by construction. `mutex.sh` is left guarding exactly one
resource — host port **25565**, the owner's client address — and a worker ladder
does not take it at all. *If you find yourself waiting on that lock to run a
ladder, the ladder is wrong, not the lock.*

Two files may add a global name back, both by name:

| file | what it adds | who uses it |
|---|---|---|
| [`owner-play.yaml`](owner-play.yaml) | `127.0.0.1:25565` + the `delvewright-server` / `delvewright-playtest` names | the owner's `play` / `playtest` session, and nothing else |
| [`ephemeral-port.yaml`](ephemeral-port.yaml) | an **ephemeral** loopback port Docker picks | the two flows that drive a bot from the host (`playtest-note-flow.sh`, `rehearsal-flow.sh`) |

Reach a worker's server over the compose network, or with
`docker exec "$(docker compose -p <id> -f validation/compose.yaml ps -q server)" rcon-cli …`
— never through localhost.

### Teardown

Tear down **only your own project**, and prove it:

```bash
validation/fresh-volumes.sh --project dw-worker-<id>
```

Never a bare `docker compose down`, never `docker rm` of a container you did not
create. `down -v` alone is not self-verifying: it leaves `<project>_server-data`
behind whenever an exited container of the project still holds the volume, and the
stale world carries the scoreboard into the next run — objectives already complete,
so the bot fails and the failure looks like a content bug (three misattributed red
runs, island round 13). `fresh-volumes.sh` force-removes that project's containers,
volumes, networks **and the images it built** and then asserts each class is
gone. The images are the class that used to leak permanently: both tags a ladder
mints are project-scoped by design (`delvewright/delve:<project>` and compose's
own `<project>-bot:latest`), so nothing ever reused them and nothing removed
them. It removes only names your ladder minted — a tag it did not mint, such as
the shared `delvewright/delve:local`, is kept and named, as is any image a
container still holds — and an image it cannot remove is reported rather than
reddening your run. It has no daemon-wide mode: the old
`--all` swept every project's world volumes and force-removed the pinned
`delvewright-*` names, which is an outage, not a teardown. It also refuses outright
to touch a project whose container publishes 25565 — that is an owner-facing
session with a human possibly inside it.

### What earlier runs left: `reclaim-ladder-images.sh`

```bash
validation/reclaim-ladder-images.sh              # lists; removes nothing
validation/reclaim-ladder-images.sh --apply      # removes
```

The teardown bounds what your run leaves. This is the backstop for everything
left before it. A project is swept only when it holds no container, no volume and
no network — anything holding one is skipped as mid-run, so a sibling ladder is
never touched — and only images built by a service this compose file declares,
under a project named the way these scripts name one, are in scope at all.
Compose's default project (`validation`, where the owner's play session lands) is
swept only when named with `--project validation`. Build cache is left alone: it
is content-addressed and global, so no project owns one.

### The 25565 mutex, for the two things that bind it

`owner-play.yaml` and [`../tools/playtest-server.sh`](../tools/playtest-server.sh)
are the only sanctioned bindings of the owner's port. `playtest-server.sh up`
takes the lock as `owner-play-session` and `down` releases it:

```bash
source validation/mutex.sh
dw_mutex_acquire "owner-play-session" || exit 1  # non-zero if someone else holds it
trap dw_mutex_release EXIT                       # idempotent; releases only your own lock
```

Acquisition is the return value of `mkdir` — never inferred from the lock
directory existing, which is true precisely when *someone else* holds it. The
lock carries a `HOLDER` file naming its owner. **`owner-play-session` is
sacred**: acquire will not wait on it or steal it, and `dw_mutex_release_named`
refuses to free it while any container still publishes 25565. Never install a
teardown trap before acquisition succeeds.

The rules are written the way they are because of a real incident: a
hand-rolled waiter whose "did I get the lock?" guard tested directory existence
fell through against the owner's held lock and ran a teardown that — via the then
pinned `container_name` — destroyed a live play session and its world volume
mid-playtest. Isolation by construction is what makes that mistake unreachable
today: a worker stack has no container name and no port in common with the
owner's.

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

## Server bootstrap: one fetch per job, not one per boot

The Mojang server jar is never baked into any image (ADR-0010, EULA), so a server on
a fresh `/data` volume bootstraps it live at first boot. Isolation gives every ladder
its own fresh volume — which is right — but it also meant every boot ran its own live
bootstrap, and a `tier 2` run boots **seven** servers (the datapack-load check plus
six PackTest suites). Seven independent chances for one Mojang blip to red a required
status check for a reason with nothing to do with the delve. A run died exactly
there, on the 5th of 6 suites, before any datapack was evaluated.

`validation/server-bootstrap-cache.sh` performs **one** fetch per job into
`validation/server-cache/` (gitignored), refuses anything whose sha256 is not
`versions.toml`'s `server_jar_sha256`, and materialises the Fabric bootstrap beside
it. `packtest-run.sh` calls it (idempotent — a warm cache fetches nothing) and copies
the overlay into that project's world volume before booting. Measured: with the
overlay in place both server types reach `Done (` and the whole PackTest suite passes
under `--network none`.

Three properties worth keeping when touching this:

* **A copy, never a bind mount of the jar.** itzg and Fabric both replace the jar by
  rename, and a rename cannot target a bind-mounted file ("Device or resource busy") —
  the failure that killed the 2026-07-30 jar cache. The overlay is mounted read-only
  at `/seed` and copied into `/data`.
* **The gate keeps its teeth.** Retries are bounded and scoped to the bootstrap fetch
  alone. Nothing here can turn a server that genuinely will not start, or a datapack
  that genuinely fails, into a pass — a network outage reds a step named `bootstrap`,
  before any server boots.
* **The seed states its binding.** `packtest-run.sh` asserts on the boot log that the
  seed was actually used, and reds if it was not. A seed that silently missed would
  leave the ladder as fragile as before while reporting success.

## One command (owner)

EULA acceptance is **your** action and is never hardcoded in this repo. Pass it in
your environment (this is Mojang's EULA: https://aka.ms/MinecraftEULA):

```sh
EULA=TRUE docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
  --profile play up
```

`owner-play.yaml` is what publishes `localhost:25565` and names the container
`delvewright-server` — `compose.yaml` alone publishes nothing, so a worker ladder
can never take your port (see "Sharing the Docker host"). Then join from a vanilla
**1.21.11** client at `localhost:25565`. Stop with `Ctrl-C`; the same command with
`down` removes the container (the world persists in its named volume). To join with
a real Microsoft account instead of offline mode, add `ONLINE_MODE=TRUE` (auth mode
is still an open spec-0003 decision).

If `EULA` is unset, compose fails fast with a message telling you to set it — by
design.

## Running a second campaign through `packtest`

**A change to PackTest emission is verified with `packtest-all.sh`, not with this
section.** The gate runs many projects, and picking one — or several — is how a
round covers a strict subset of it and reports green:

```sh
cargo build --bin delvec
EULA=TRUE validation/packtest-all.sh          # every project the gate runs
validation/packtest-matrix.py                 # …and what those are, read from ci.yml
```

The rest of this section is why the surface has the shape it does; it is not a
list to work down by hand.

The generated PackTest suite is per-campaign: `delvec` emits a template only for a
campaign that uses the feature it proves. hello-world — one quest, one NPC —
declares no cast ledger, so spec-0020's cast-ledger templates (`cast_ladder_<npc>`,
`cast_bark_cycle`, `cast_none_silent`) simply do not exist in its output. It has no
`interact` objective either, so the `interact` templates (`verb_interact` and
`verb_interact_held`, the held-vs-carried proof) are likewise absent — CI therefore
runs `crates/dsl/fixtures/valid/keep-trial` as its own pass. It has no bonfire and
no wave, and no lane anywhere in the repo's tier-2 set, so the whole souls retry
loop (`souls_bonfire_rest`/`_reseat`/`_options`, `souls_reseat_stationed`,
`wave_census`) and the whole TD-lane family (`souls_td_patrol_nbt`,
`souls_td_lane_march`, `souls_td_lane_release`, `souls_td_lane_reseat`,
`souls_td_aggro_edge`) were emitted and never executed — including the codec-trap
test, whose entire reason to exist is that a wrong `patrol_target` key is
invisible to every static proof. `crates/delvec/tests/fixtures/souls-bonfire`
and `crates/delvec/tests/fixtures/souls-td-lanes` are two more. They are two fixtures and not one because `DW0478` forbids a bonfire
inside a hostile's aggro range, and the lane fixture's corridor tileset has no
cell more than 16 blocks off its own lane.

`--output` boots a second tree; the run tears its own project down and proves it
clean, so there is no separate teardown command to forget:

```sh
delvec build crates/delvec/tests/fixtures/cast-ledger \
  -o validation/delve-output-cast --prefabs campaigns/prefabs

EULA=TRUE validation/packtest-run.sh --project dw-cast --output ./delve-output-cast
```

There is no `PACKTEST_CONTAINER` any more: the runner pins no
container name, so the compose project is the only name there is, and
`--project` is required rather than defaulted. `validation/delve-output*/` is
gitignored, so extra trees need no bookkeeping. This is exactly how CI's tier-2
job runs its extra passes — add a step there alongside the existing ones, with its
own `--project`, when a new feature's templates need live execution rather than
shape verification.

## Creator playtest loop (spec-0006)

Build the delve, then run the `playtest` profile — the **same shipped delve image**
as `play`, plus the creator overlay mounted as an extra datapack and (optionally)
the creator opped:

```sh
delvec build crates/dsl/fixtures/valid/hello-world -o validation/delve-output

EULA=TRUE CREATOR_NAME=<your-mc-name> \
  docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
    --profile playtest up --build
```

Join at `localhost:25565` (`owner-play.yaml` is what publishes it). While playing, aim at something wrong and run
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
docker compose -f validation/compose.yaml -f validation/owner-play.yaml \
  --profile playtest logs --no-color > playtest.log
cargo run -p delvec --bin delvec -- harvest \
  playtest.log validation/delve-output/creator-datapack/layout.json \
  -o playtest-report.json
```

`delvec harvest` pairs each `[DelveNote]` stamp with the nearest creator chat line
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

It runs in its own per-invocation compose project (`dw-noteflow-$$`, overridable
with `DW_COMPOSE_PROJECT`) on an **ephemeral** host port, so it needs no lock and
cannot collide with another ladder or with a live owner session.

**CI placement (spec-0006 acceptance).** This is a **tier-3 / local** test (wired in
`release.yml`), not tier 2: it boots a full server *and* a bot (~2–3 min), beyond
tier 2's ~2-min budget. Every-push coverage of the mechanism already lives in tier 1
— the harvester's parsing/pairing/report logic (`crates/orchestrator` unit tests,
incl. Chinese note text) and the overlay emission + byte-determinism
(`crates/compiler` tests). Only the live wiring is deferred to tier 3.

## What the stack does today

- Boots vanilla 1.21.11 via the `itzg/minecraft-server` image, which downloads the
  pinned server jar at runtime (never baked into a layer — the ADR-0010 EULA-safe
  pattern). The only host binding anywhere is the owner's `127.0.0.1:25565` in
  `owner-play.yaml` — never world-reachable, and never published by a ladder.
- Every profile serves one compiler build output (`manifest.json`, `datapack/`,
  `server/`, `packtest-datapack/`, `critical-path.json`), so `bot-run.sh` and
  `packtest-run.sh` reproduce CI's dynamic tiers locally with exit codes
  propagated (spec-0003 acceptance criteria).
- **Re-runs**: `validation/fresh-volumes.sh --project <your compose project>`
  tears that stack down and *proves* its world volumes are gone. A persisted
  volume keeps completed objectives completed, which fails a "fresh" playthrough
  for reasons unrelated to the delve. `bot-run.sh` / `packtest-run.sh` /
  `branch-runs.sh` run it for you, before and after. The project name is required
  and there is no daemon-wide mode — a teardown that can reach another project is
  an outage, not a teardown. It reclaims the project's images too, and states
  what it examined, removed and kept.
- **The world first**: `EULA=TRUE validation/world-save.sh <build-dir> --project
  dw-<id>` boots the shipped delve image for that tree once, waits for the
  datapack to report `#placed dw.sys = 1` over rcon, and copies the world save it
  stamped into `<build-dir>/world/`. A build output carries no world — the
  geometry is placed on the first ticks of a server boot — and a Chunky scene
  over a missing world renders an empty sky at exit 0.
- **Shot sets**: `validation/render-shots.sh <build-dir> [out-dir]` turns a build
  output into the Chunky scene set plus the shot index (`delvec scene` +
  `index`) for visual review, including the first-person player-POV shots. It
  refuses a tree with no `world/`, naming `world-save.sh`.

## Harness

The mineflayer bot that the `validate` profile runs lives in [`../harness`](../harness).
It parses the compiler's `critical-path.json` contract and drives the run; it
contains **zero** campaign-specific logic (spec-0003). See that directory's code and
tests. Current status: parser + sequencer + a mineflayer executor skeleton
(`reach`/`assert-complete` implemented; `select-class`/`talk-to` stubbed pending the
dialog-vs-`tellraw` interaction-channel decision).
