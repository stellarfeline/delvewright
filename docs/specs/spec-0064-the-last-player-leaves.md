# spec-0064: The last player leaves, and the delve is built again

- **Status**: Accepted
- **Ground**: written against the engine at `dd46f01b` (`origin/main`), read
  only, and against the pinned delve base image
  `ghcr.io/stellarfeline/delvewright-base@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de`
  (`validation/Dockerfile.delve`), whose `org.opencontainers.image.revision`
  label names upstream revision `162bd9b5` of `itzg/docker-minecraft-server`.
  Every statement about what that image does was read from the scripts inside
  it (`/image/scripts/start-configuration`, `start-autostop`,
  `auto/autostop-daemon.sh`, `auto/autopause-fcns.sh`, `auto/stop.sh`,
  `start-setupDatapack`, `start-finalExec`) and from its OCI config, never from
  memory. The shape of this document is spec-0062's.
- **What it is for**: a publicly joinable showcase server. A stranger joins,
  is led through the delve, finishes or abandons it, leaves — and the next
  stranger arrives at a delve nobody has touched. Today that needs an operator
  at the host running spec-0024 §4's manual reset between visitors.
- **Research**: `docs/reference/container-reset-practice.md` is this spec's
  research record — what the base image already provides for *stop when the
  last player has left*, what the pinned server writes where, and what an
  established game-server practice for *pristine between sessions* is. Every
  rule below is marked **cited** (the record, an ADR or an upstream document
  requires it) or **authored** (this spec chooses).
- **Numbers**: none taken. No DW code (§8: nothing here is a compiler
  diagnostic), no ADR (§8: no settled decision moves), no `dsl_version` (§8:
  no DSL surface moves).
- **Non-goals**: hardening the shipped image for anonymous public access
  (§9, one line, a separate obligation); any DSL surface, field or verb; the
  guided-tour content; a status page or queue for players who arrive during a
  reset; the implementation.

## 1. What the tree already has, and the part that is missing

**Finding, from reading the tree.** The remedy exists and is trusted; the
event does not, and nothing inside the container can act on it.

- **The observation.** `validation/fresh-volumes.sh` opens with it: the itzg
  `/data` world volume persists player scoreboard state — a completed objective
  stays completed — across runs, and `docker volume rm` silently no-ops while a
  container of the project is not fully stopped. The script tears one compose
  project down and *proves* its containers, volumes and networks gone.
- **The manual reset.** spec-0024 §4 already states the product rule: *a delve
  is replayable by wiping world state, nothing else* — one line of `docker rm`
  plus volume removal. That rule is right and this spec does not change it; it
  binds the same wipe to an event instead of to an operator.
- **What the base image already provides.** The pinned base image carries
  upstream's **autostop** (`ENABLE_AUTOSTOP`): a daemon that waits for the
  server to listen, reads the online player count every `AUTOSTOP_PERIOD`
  seconds, and once the count has been zero for `AUTOSTOP_TIMEOUT_EST` seconds
  sends `SIGTERM` to `mc-server-runner`, which stops the server cleanly. That
  is the event — *the last player left and stayed away* — as a maintained state
  machine with the edge cases already handled (§2). Upstream's own document
  says what it does not do: the container then exits and *has to be manually
  restarted*.
- **What is missing.** Three things, all in the entrypoint the delve image
  already owns (`validation/world-settings-entrypoint.sh`, baked byte-identical
  into `Dockerfile.delve`): a flag that turns the daemon on; the wipe of world
  state, performed inside the container because on a `docker run` host there
  is no compose project outside it to tear down; and the loop that boots the
  server again after the stop instead of letting the container exit. Nothing
  in the compiler, the DSL or the datapack moves.

So the spec is small: **one flag, one wipe, one loop**, each held by a check.

## 2. The event: the server has said "zero" for the whole window

**Cited** for the reading (upstream's daemon, read at `162bd9b5`);
**authored** for the window's floor and the two properties it depends on.

The event is *not* "the count reached zero". It is: **every reading of the
online player count over a window of `W` seconds was zero.** Any non-zero
reading inside the window ends it and the wait starts again from the next
zero. That is upstream's machine exactly: state `E` (established) goes to `I`
(idle) on a zero reading and arms a deadline `W` seconds out; `I` returns to
`E` on any non-zero reading; only reaching the deadline while still in `I`
stops the server.

**How the count is read.** Not assumed and not grepped from a log. The daemon
runs `mc-monitor status --host localhost --port 25565 --show-player-count`
inside the container: a server-list-ping to the game port, whose reply carries
the server's own `players.online`. A ping that fails after its retries is
counted as **one player**, not zero — upstream's rule, and the fail-safe
direction: a lagging server with a party inside it is never reset.

**Two facts about that reading are load-bearing, and the spec states them:**

1. A player is counted from the moment vanilla places them in the world, not
   from the TCP connect. A player still in the login handshake is therefore
   invisible to the count for a few seconds. That is why the event is a window
   and not an edge: the case that matters — one player disconnects while another is
   loading in — reads zero for the handshake and non-zero the moment the second
   player is placed, well inside any window this spec admits. **The floor on
   `W` is 60 seconds** (authored): six readings at the daemon's 10-second
   period, an order of magnitude over a handshake, and past
   `pause-when-empty-seconds=60`, so the last readings of every window are
   taken from a *paused* server — the state a public delve idles in, and the
   state the ladder must prove the ping still answers from (§10.3).
2. The count exists only while the server answers status pings with a player
   list: `enable-status=true` and `hide-online-players=false`. Both are among
   the 55 keys the build leaves to the host (`docs/reference/compiler.md`,
   *Unpinned keys*). Left to chance, a host that hid its player list would turn
   every reading into a failed ping, every failed ping into "one player", and
   the flag would be silently inert — the unbound shape. So on the on-path the
   entrypoint sets the two through the base image's own property mapping
   (`ENABLE_STATUS=true`, `HIDE_ONLINE_PLAYERS=false`), and the ladder
   cross-checks the reading against a second instrument that shares no
   configuration with it: `list` over rcon through `tools/lib/rcon.sh` (§10.2).
   The two keys do **not** enter the compiler's pinned set: they are operator
   transport, not world state, which is the line `compiler.md` already draws.

**What the window cannot see, and it was measured rather than reasoned about.**
The daemon's state machine only leaves `II` (initial idle) when a *reading* is
non-zero, and this spec disables the initial-idle stop (§5). So a presence that
falls entirely between two readings — a visitor who joins and leaves inside one
`AUTOSTOP_PERIOD` — is never seen, the machine stays in `II`, and no reset
follows. Reproduced under a plain `docker run` with `DELVE_RESET_WHEN_EMPTY=60`:
a visitor joined, killed the cast, set a score and left within about fifteen
seconds; four minutes later the delve had not been rebuilt, and the world came
back only when the container was restarted. What the next arrival can see of such
a visit is bounded by what fits inside one reading: a player-data file and a
`usercache.json` entry under a UUID that is not theirs. The residual is stated
rather than engineered away, because both alternatives are worse — arming the
initial-idle stop rebuilds an unjoined delve every `W` seconds forever, and a
second observer inside the image is a second mechanism for one rule. A host who
wants it closed shortens nothing: the boot resets, so restarting the container
does.

**No new listener, and no private copy of the rejection rule.** The reading
goes to the game port the delve already publishes; the stop is a signal to a
process. Nothing inside the container issues a live command, so
`tools/lib/rcon.{sh,mjs}` is not copied into the image; the only live commands
this spec adds are the ladder's, host-side, through the shared rule. §6 says
what is proved about the listener the image already has.

**The hold.** Upstream honours a file: while `/data/.skip-stop` exists the
daemon logs that it is skipping and never stops. It is the one hatch, it is
upstream's, and the entrypoint's log states at every boot whether the file is
present. Nothing in this spec creates it.

## 3. What "reset" means: the world is removed and built again

**Cited**: the constitution's rule for a generated artifact — *reset wholesale
and regenerated, never three-way merged*. This spec applies it as written and
argues nothing against it.

A reset that **undoes** — kill the entities, reset the scores, clear the
storage, strip the inventories, reload — is a state machine with holes: every
carrier the undo list forgets survives, and the list is only ever as complete
as the last defect that lengthened it. A reset that **restores** has no list.
So:

**The reset removes the world directory and lets the boot make it again.**
The world directory is `/data/<level-name>`, and `level-name` is one of the 15
keys the build pins (`crates/delvec/tests/server_properties.rs`), with the
value `world` (`compiler::emit`). What the boot then produces is what a first
boot produces: the base image regenerates the superflat from the pinned seed,
copies `/delve/datapack` into the new world's `datapacks/` (the copy runs on
every boot, unconditionally, when `DATAPACKS` is set — read from
`start-setupDatapack`), and the datapack's own `setup` stamps the geometry
and sets `#placed dw.sys` to 1. No snapshot is taken and none is restored: a
saved copy of "the pristine world" would be a second authority for what the
world is, beside the image that already is one.

**When.** At **every boot on the on-path**, before the base image's start
chain runs — not only after an autostop. The boot is the single entry point
every path to a running server passes through (autostop's restart, a crash,
`docker stop`/`docker start`, `docker restart`, a host reboot with a restart
policy), so one site covers them all and no path can boot a continued world by
accident. The consequence is stated, not hidden: **with the flag on, no boot
ever continues a session.** An operator who restarts the container under a
party ends that party's run. That is the flag's meaning and the storybook's
host line will say so.

**What it is restored *to*, and how a reader checks it.** To the state a first
boot of this image produces. The check cannot be a byte hash of the save —
`validation/world-save.sh` records why: a world save carries wall clock in
`level.dat` and in every region file's timestamp table, so two boots of one
image never produce identical bytes, and nothing in the tree hashes a save. The
check is a **state fingerprint** taken through the shared rcon rule after
`#placed dw.sys` reaches 1, over the carriers a session can move:

| reading | instrument | binds to |
|---|---|---|
| the datapack in the world equals the image's | `sha256sum` over `/data/world/datapacks/<pack>` and `/delve/datapack`, inside the container | ADR-0006's byte identity, the half that is provable byte-for-byte |
| no player has data | count of entries under the world's player-data directory, enumerated by `find` | inventories, positions, tags |
| the scoreboard holds only what `setup` seeds | `scoreboard players list` | every `dw.*` objective's scores |
| the checkpoint mirror is the spawn seed | `data get storage dw:cp pos` | `storage dw:*` |
| the cast is at its entrance | `execute if entity @e[tag=dw_npc]` count | entities the session can kill, move or unleash |
| nobody is online | `list` | the event's own subject |

The fingerprint of the post-reset boot must equal the fingerprint of the first
boot, and **must differ from the fingerprint of the played world** taken just
before the stop — the second half is what proves the readings bind to state at
all (§10.4). The world's directory layout is **not** hand-written into the
check: the player-data path and every other carrier are found by enumerating
what a session changed (§4), so a layout that moves under a future pin moves
the enumeration, not a constant.

## 4. What comes back with it, and where each thing lives

**Cited** for what the world folder holds (the vanilla level format, per the
research record); **measured** for the enumeration, which the ladder performs
rather than this spec asserting it.

Three carriers a reader will ask about — quest state, scoreboards, inventories. All three are files the server writes **under
the world directory**, so one removal covers all three:

| carrier | where the pinned server writes it | covered by |
|---|---|---|
| quest state — scores on the `dw.*` system objectives and on every `obj/<id>` quest objective (`compiler::emit`) | the scoreboard file under the world's `data/` | removing `/data/world` |
| quest state — every `storage dw:*` namespace (`dw:cp`, `dw:region`, `dw:cs` among them) | command-storage files under the world's `data/` | removing `/data/world` |
| player inventories, positions, tags, spawn points | per-UUID files under the world's player-data directory | removing `/data/world` |
| advancements and statistics, the campaign-completion advancement included | per-UUID files under the world | removing `/data/world` |
| placed blocks, NPC bodies, hitboxes, markers, force-loaded chunks | region, entity, poi and chunk-ticket files under the world | removing `/data/world` |

What the server writes **outside** the world directory is enumerated once,
here, with the reset's verdict on each — and the ladder holds this list closed
(§10.5): a path the session changed that is neither removed nor named here is a
red, so the list cannot rot silently.

| path under `/data` | what it is | reset |
|---|---|---|
| `usercache.json` | the server's cache of visitor names and UUIDs | **removed** — it is about players |
| `ops.json` | the offline op seed the entrypoint writes | rewritten by the entrypoint at every boot, as today |
| `whitelist.json`, `banned-players.json`, `banned-ips.json` | the operator's lists | **kept** — an operator's ban outlives a visitor |
| `server.properties`, `eula.txt` | the boot's configuration and the operator's acceptance | **kept** — regenerated or unchanged by the base image at boot |
| the server jar, `libraries/`, `versions/` | the pinned server, fetched by checksum at first run (ADR-0010) | **kept** — removing it would make every reset a Mojang download and a network dependency |
| `logs/` | the server's own log | **kept** — the only record of what a visitor did; the reset's own lines are written into it |
| `.skip-stop` | upstream's hold (§2) | **kept** — an operator placed it |

The list is authored from what the base image and the pinned server are known
to write; the ladder's enumeration is the measurement, and a name that never
matches a changed path is reported as unbound, not silently carried.

## 5. The flag, its default, and what the build does when it is off

**Authored.**

- **Name and value**: `DELVE_RESET_WHEN_EMPTY=<seconds>`, an environment
  variable on the container, read by the delve entrypoint beside the two
  `DELVE_*` variables it already reads (`DELVE_SERVER_PROPERTIES`,
  `DELVE_OPS_OFFLINE`). The value **is the window `W`** of §2. One knob: there
  is no boolean beside a number, because a boolean with an implicit window is
  the shape whose default nobody reads.
- **Default**: unset. **Unset means off.**
- **Refused at boot, with the reason on stderr and a non-zero exit**: a value
  that is not a decimal integer; `0` (a zero window is the edge §2
  forbids); any value under the 60-second floor. The flag was asked for and
  cannot be honoured, so the safe path is to not start — a server that boots
  with the flag silently ignored is the defect §5's last bullet exists to
  forbid.
- **On**: the entrypoint (1) removes `/data/world` and `/data/usercache.json`,
  printing one line per path removed and one line for the hold file's
  presence; (2) exports `ENABLE_AUTOSTOP=true`,
  `AUTOSTOP_TIMEOUT_EST=$DELVE_RESET_WHEN_EMPTY`, `AUTOSTOP_TIMEOUT_INIT=2147483647`
  (upstream has no "never" for the initial-idle stop, and a server nobody has
  joined is already pristine — a stop there would only cycle the boot; the
  value is the largest the daemon's arithmetic is known to hold, and it is
  named here as the workaround it is), `ENABLE_STATUS=true`,
  `HIDE_ONLINE_PLAYERS=false`; (3) runs the base image's start chain **as a
  child** and waits for it; (4) when the child exits and no stop signal was
  received, loops to (1).
- **Off**: the entrypoint `exec`s the base image's start chain, as today. No
  daemon starts, no environment variable above is set, nothing is removed, and
  the container's process tree is the one the image has now (`mc-server-runner`
  as PID 1). **The requirement in a form a check can hold**: with the flag
  unset, the observation `fresh-volumes.sh` records must still reproduce — a
  scoreboard score written in one boot is present after a `docker stop` and
  `docker start` of the same container (§10.1). A flag whose absence changed
  that would be the silent default change §5 forbids, and that criterion
  is what reds it.

**What the flag is not.** It is not a compose profile, a Dockerfile `ARG` or a
build-time choice: the same image serves the owner's table and the public
showcase, and which one it is belongs to the person starting the container
(ADR-0010: the same image for validation, CI and prod).

## 6. Signals, and the listener the image already has

**Authored** for the loop; **cited** for what the image exposes (its OCI
config, read).

**Signals.** On the off-path nothing changes. On the on-path the entrypoint is
PID 1 with the start chain as its child, and PID 1 receives no default signal
handling, so the entrypoint traps `SIGTERM` and `SIGINT`, forwards the signal
to the child, waits for it, and **exits without looping**. `docker stop` under
a party therefore stops the server the way it does today (the base image's
`STOP_DURATION` grace), and the container exits and stays exited. The world it
leaves is disposable — the next boot removes it — so a hard kill after the
grace period loses nothing a soft stop would have kept.

**The listener.** This spec adds none. The reading uses the game port; the stop
is a signal. The image does carry one listener a public server should know
about, and this spec states it rather than assuming it: the base image enables
RCON by default (`ENABLE_RCON` defaults to `true` in `start-configuration`)
with a password generated per boot (`openssl rand -hex 12`) when none is given,
on port 25575. What is proved, not assured (§10.6): the image's OCI config
exposes exactly `25565/tcp` — read from the pinned base and from
`Dockerfile.delve`'s own `EXPOSE`, so `docker run -P` publishes nothing else;
the shipped compose service publishes no port at all
(`tools/check-compose-isolation.py`); and a TCP connect to port 25575 at the
host's published address is refused while the flag is on. **The residual is
named**: another container on the same Docker bridge network can reach 25575,
and RCON-on-by-default is a property of the image whether or not this flag is
set. That is §9's obligation, not this spec's.

## 7. The ladder and its gate

**Authored.** One script, `validation/reset-when-empty-run.sh --project
dw-<id>`, the shape of `validation/world-save.sh`: it builds
`crates/dsl/fixtures/valid/hello-world` (the compose default subject), boots
the shipped delve image in its own compose project with
`DELVE_RESET_WHEN_EMPTY=90` and no host port, fresh-volumes the project before
and after, and drives everything below through `tools/lib/rcon.sh` on the host
side. It prints one binding line:

```
reset-when-empty binding: 1 session played, 1 reset observed (stop→listening 00 s,
listening→placed 00 s); 6 fingerprint readings compared, 6 equal first-boot vs
post-reset, 6 moved by the session; 00 changed paths under /data enumerated:
00 removed, 00 kept by name, 0 unaccounted; count cross-check 1=1 then 0=0.
```

Every zero above is a placeholder the run fills; a run whose enumeration is
empty is a red, because a session that changed nothing under `/data` did not
happen.

The compose `server` service gains `DELVE_RESET_WHEN_EMPTY:` in compose's
short form, so an unset host variable omits the key and the bot ladder, the
owner's play profile and CI boot exactly as today.

## 8. Numbers this spec does not take, and why

**Authored.**

- **No DW code.** A DW diagnostic is the compiler refusing a document or a
  build; nothing here reads a document. The refusals in §5 are the entrypoint's,
  on the operator's own flag, and the entrypoint already refuses this way
  (`FATAL: … server.properties is missing`) without a code.
- **No ADR.** ADR-0010 says one `docker run` is a joinable dungeon and the same
  image serves every environment; this spec keeps both — the flag is a
  run-time choice on that one image, and no new artifact ships. ADR-0006's
  byte identity is invoked, not moved. ADR-0003 (no mods on the player-facing
  server) is untouched: autostop is a shell daemon of the base image the delve
  already ships, not a server modification.
- **No `dsl_version`.** Confirmed by what moves: no key of the 15 pinned
  properties, no stage document, no schema field, no emitted byte. The
  double-build byte-identity gate over a campaign is unchanged by this spec.
- **No demo-level row.** `docs/demo-levels.md` queues gameplay mechanics a
  player meets; this is a container behaviour a player never sees from inside
  the world, and the ladder over the hello-world fixture is where it is
  confirmed — never on a campaign.

## 9. Adjacent, open, and not folded in

Hardening the shipped image for anonymous public access is a separate
obligation: the 15-key reviewed set was reviewed for one to four known players,
and `enable-command-block`, `white-list`, `op-permission-level`,
`function-permission-level` and `enable-rcon` are among the keys a public
threat model would have to read — this spec names it and stops.

## 10. Acceptance criteria

Machine-checkable; each names its instrument and was checked against the tree
at `dd46f01b` before being written. Where the tree cannot yet satisfy a
criterion the verdict is recorded as a debt.

1. **Off is today.** With `DELVE_RESET_WHEN_EMPTY` unset, the ladder writes a
   score (`scoreboard players set #probe dw.sys 7` through the shared rule),
   `docker stop`s and `docker start`s the same container, and reads the score
   back as 7; the container's PID 1 is `mc-server-runner`; the log carries no
   `Autostop functionality enabled` line. *Tree: the persistence half holds
   today by the observation `fresh-volumes.sh` records; the check does not
   exist — debt.*
2. **The count is a measurement.** With the flag on and one bot connected, the
   daemon's reading (`mc-monitor status … --show-player-count`, run through
   `docker exec`) equals the count `list` reports through `tools/lib/rcon.sh`
   (1 = 1); after the bot disconnects, both read 0 (0 = 0). *Tree: debt.*
3. **The event is the window.** With `DELVE_RESET_WHEN_EMPTY=90`: a bot joins,
   leaves, and rejoins 30 seconds later — no stop within 90 seconds of the
   first leave (the log's `Client reconnected` line is present, no
   `Stopping Java process`); the bot then leaves and the server stops between
   90 and 100 seconds after the last reading of 1, with the last three
   readings taken after `pause-when-empty-seconds` elapsed. *Tree: debt.*
4. **The reset restores, and the fingerprint binds.** The six readings of §3
   taken on the first boot equal the six taken on the post-reset boot; the six
   taken on the played world before the stop differ in at least the player-data
   count, the scoreboard listing and the `list` count. *Tree: debt.*
5. **The keep list is closed.** Every path under `/data` created or modified
   between the first boot's `#placed dw.sys = 1` and the stop is either absent
   after the reset or named in §4's table; the ladder prints the three counts
   and `0 unaccounted`, and a §4 name matching no changed path is printed as
   unbound. *Tree: debt.*
6. **No new reachable listener.** With the flag on: the pinned base image's
   and the built delve image's `ExposedPorts` are exactly `{25565/tcp}`
   (`docker image inspect`); the compose service publishes no port
   (`tools/check-compose-isolation.py`, already green on the tree); a TCP
   connect from the host to the container's port 25575 at its published
   address is refused, asserted by exit status. *Tree: the two `ExposedPorts`
   facts hold today and were read; the connect check is debt.*
7. **Refusals.** `DELVE_RESET_WHEN_EMPTY` set to `0`, to `59`, and to `soon`
   each exit non-zero before the start chain runs, naming the value and the
   floor. *Tree: debt.*
8. **Signals.** `docker stop` with a bot connected exits the container within
   the base image's `STOP_DURATION` grace, the log carries the server's own
   stop, and no further boot follows (the container's state is `exited` ten
   seconds later and its log has one `Done (` line). *Tree: debt.*
9. **Dead time is printed.** The binding line's two intervals — from the
   daemon's `Stopping Java process` to the next boot's `Done (`, and from
   `Done (` to `#placed dw.sys = 1` — are measured on the run and printed;
   neither is an adjective anywhere in the record. *Tree: unknown today; no
   number for either interval exists in the repository. The only figure in
   hand is the base image's healthcheck start period of 120 seconds. The
   number on the prod host is read from the showcase server's own log lines,
   never measured by a test run there.*
10. **Perturbation toward the vacuous shape.** The ladder run against a scratch
    copy of the build context whose entrypoint has the removal step deleted
    reds at criterion 4 (the post-reset fingerprint equals the played one) and
    at criterion 5 (unaccounted paths); the same ladder with the flag unset
    reds at criterion 4 for the same reason. Both perturbations are made on a
    copy, never on the tree. *Tree: debt.*
11. **The pair stays byte-identical.** `validation/check-world-settings.sh`
    is green with the new entrypoint body in both places. *Tree: green today
    on the current body; re-proven when the body moves.*
12. **The record.** `docs/reference/tools.md` gains the ladder's row;
    `docs/reference/compiler.md`'s *Unpinned keys* paragraph names
    `enable-status` and `hide-online-players` as set by the entrypoint on the
    on-path only; the content storybook's host line gains the flag's one-line
    form with its consequence (§3: no boot continues a session) — in the pull
    request that lands the code. *Tree: debt.*
