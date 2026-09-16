# Resetting a running delve between sessions — what the record holds

The research behind spec-0064. Reader: the engineer implementing or reviewing
the reset-when-empty flag. Three questions were asked and no more: what does
the pinned base image already provide for *the last player has left*; where
does the pinned server write the state a session moves; what is the
established practice for returning a game server to a pristine state between
sessions. Each answer states whether it is **cited** (read from a named
source at a named revision) or **from memory** (stated, not verified — a
reader who needs it verifies it before building on it).

## 1. What the pinned base image provides

**Cited.** Read from inside
`ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de`
(`validation/Dockerfile.delve`), whose `org.opencontainers.image.revision`
label is `162bd9b5` of `itzg/docker-minecraft-server`; the upstream documents
were read at that same revision (`docs/misc/autopause-autostop/autostop.md`,
`docs/configuration/auto-rcon-commands.md`, `docs/sending-commands/commands.md`).

| capability | what it does | where |
|---|---|---|
| **autostop** | `ENABLE_AUTOSTOP=true` starts a daemon after the server process exists. States: `INIT` until the server answers a status ping; `II` (initial idle) arms `AUTOSTOP_TIMEOUT_INIT` (default 1800 s); `E` (established) once a reading is non-zero; `I` (idle) arms `AUTOSTOP_TIMEOUT_EST` (default 3600 s) on a zero reading and returns to `E` on any non-zero one; reaching a deadline runs `stop.sh`, which is `pkill -f --signal SIGTERM mc-server-runner`. Period `AUTOSTOP_PERIOD` (default 10 s). A `/data/.skip-stop` file makes the daemon log and skip while present. Upstream's document: the container then exits and *has to be manually restarted*; it recommends a restart policy of `no`. Incompatible with autopause. | `/image/scripts/start-autostop`, `auto/autostop-daemon.sh`, `auto/stop.sh` |
| **the player-count reading** | `java_clients_connections`: while the java process runs, `mc-monitor status --host localhost --port $SERVER_PORT --retry-limit 10 --retry-interval 2s --show-player-count`; **a failed ping is reported as 1**, with upstream's own comment — *otherwise a laggy server with players connected could get paused*. `mc-monitor`'s flag help: `-show-player-count  show just the online player count`. | `auto/autopause-fcns.sh` |
| **RCON commands on events** | `RCON_CMDS_LAST_DISCONNECT` (and `_FIRST_CONNECT`, `_ON_CONNECT`, `_ON_DISCONNECT`, `_STARTUP`) run rcon commands from a daemon polling the same count every `RCON_CMDS_PERIOD` (10 s). Upstream's note: *on client connect we only know there was a connection, and not who connected*. | `auto/rcon-cmds-daemon.sh`, `start-rconcmds` |
| **RCON** | `ENABLE_RCON` defaults to `true`; with no `RCON_PASSWORD` given one is generated per boot (`openssl rand -hex 12`) and written to `$HOME/.rcon-cli.{env,yaml}` for `docker exec … rcon-cli`. Port `RCON_PORT` defaults to 25575. | `start-configuration` |
| **exposed ports** | The image's OCI config exposes exactly `25565/tcp` and declares the volume `/data`; its entrypoint is `/image/scripts/start`, which `exec`s down the chain to `mc-server-runner` (`start-finalExec`), so the server runner is PID 1 unless a wrapper keeps it as a child. | `docker image inspect` |
| **datapack copy** | With `DATAPACKS` set, every boot copies each entry into `/data/${LEVEL:-world}/datapacks` unconditionally; `REMOVE_OLD_DATAPACKS` is a separate, default-off knob. | `start-setupDatapack` |
| **healthcheck** | `mc-health` every 30 s, start period 120 s, 2 retries — the only boot-time figure the image states. | OCI config |
| **stop grace** | `mc-server-runner --stop-duration ${STOP_DURATION:-60}s`. | `start-finalExec` |

**Why spec-0064 uses autostop and not `RCON_CMDS_LAST_DISCONNECT`.** The
rcon-commands hook can only *issue commands into the running world*, which is
the undo shape the constitution's reset rule refuses; and it fires on the edge
(count fell to zero at one reading), not on a window. Autostop is the window
and ends in a process exit, which is what a wholesale reset needs.

## 2. What the pinned server writes, and where

**Cited, with a stated gap.** The wiki's *Java Edition level format* page was
read for the layout of a world save. At the time of reading it documents a
layout newer than the pinned 1.21.11 — `players/data/`, `players/advancements/`,
`players/stats/`, `data/minecraft/scoreboard.dat`, `dimensions/<ns>/<dim>/
{region,entities,poi}/` — and states that the scoreboard file holds
*objectives' definition, entities' scores, teams, and display slots*, that
player inventory and data are per-UUID files under the players tree, that
command storage lives in namespaced files under the data tree, and that forced
chunks live in `data/minecraft/chunk_tickets.dat` (previously `chunks.dat`).

**The gap, recorded against §4 of the spec.** No 1.21.11 world save was on the
workstation to read the pinned layout from, and the wiki's current page is not
that layout. So the spec transcribes **no per-file path** for 1.21.11; it
relies on the one fact both layouts share and the page states — every carrier
a session moves is written **under the world folder** — and it makes the
ladder enumerate what a session changed under `/data` rather than asserting
it. A reader who wants the 1.21.11 paths reads them from the save the ladder
produces.

**Outside the world folder, cited from `docs/reference/compiler.md`
(*Unpinned keys*) and from the entrypoint:** `usercache.json` (the server's
own name→UUID cache), `ops.json` (written by the delve entrypoint from
`DELVE_OPS_OFFLINE`), `server.properties` (regenerated by the base image from
the build's file plus env), `whitelist.json` and `banned-*.json` (the
operator's), `eula.txt`, the server jar with its `libraries/` and `versions/`,
and `logs/`.

**`server.properties` keys the reading depends on, cited from the wiki's
*Server.properties* page:** `enable-status` (default `true` — *whether the
server appears as "online" on the server list*), `hide-online-players`
(default `false` — *whether to disable sending the player list on status
requests*), `pause-when-empty-seconds` (default `60` — *how many seconds have
to pass after no player has been online before the server is paused*),
`player-idle-timeout` (default `0`), `enable-rcon` (vanilla default `false`;
the base image sets it `true`), `rcon.port` (default `25575`).

**From memory, unverified against decompiled 1.21.11 source:** the status
reply's `players.online` is the size of the server's player list, to which a
player is added when placed into the world after the login and configuration
phases, not at TCP connect. Spec-0064 §10.2–§10.3 are the measurements that
stand where this claim would otherwise be trusted.

## 3. Established practice for "pristine between sessions"

Three patterns are in use; the spec picks the first and the reason is stated.

1. **Disposable process, fresh world per session.** Match-based game servers
   run one server process per session and start a new process — with a fresh
   copy of the map — for the next; the finished process's state is simply
   discarded. **From memory**: this is the shape of hosted match-server
   lifecycles (a process serves one game session and is recycled) and of
   Minecraft minigame networks, which load a fresh copy of a template world
   per round rather than cleaning the played one. It is also the shape of
   `validation/fresh-volumes.sh` and spec-0024 §4 in this repository: the
   world is wiped, never repaired. spec-0064 is this pattern moved inside the
   container: the wipe is a directory removal at boot; the fresh copy is the
   boot itself, from the image (ADR-0006).
2. **Snapshot and roll back.** Keep a pristine copy of the world directory and
   copy it back over the played one. **From memory**: common in Bukkit-era
   minigame plugins that hold a read-only template world and clone it. Not
   chosen: a saved copy is a second authority for what the world is, beside
   the image; and a copy of a server-written save is exactly the artifact
   `validation/world-save.sh` records as non-reproducible.
3. **In-place undo.** Kill entities, reset scores, clear storage, strip
   inventories, `/reload`. This is what an rcon-command hook can do (§1). Not
   chosen: it is the state machine with holes the constitution's reset rule
   names — complete only up to the last carrier somebody remembered.

**Cited from upstream's own documents:** the autostop document's note that the
container *has to be manually restarted* is why pattern 1 needs the loop the
spec adds, and its recommendation of restart policy `no` is what the in-image
loop makes unnecessary — a reset that depends on a `docker run --restart`
flag the operator must also remember is two flags for one behaviour, and the
forgotten one fails silently.

## 4. What was not researched

The public-server threat model (which keys of the 15-key pinned set and the
55 host-decided keys a public host must read); what a player's client shows
during the dead time on each client version; the reset's cost on the prod
host. Each is out of spec-0064's scope and is named there.
