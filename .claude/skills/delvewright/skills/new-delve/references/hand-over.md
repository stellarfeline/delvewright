# Step 13 — handing it over

## Contents

- [The storybook](#the-storybook)
- [The engine-version marker](#the-engine-version-marker)
- [The render-distance line](#the-render-distance-line)
- [The host line](#the-host-line)
- [The play server](#the-play-server)
- [The staging gate](#the-staging-gate)
- [The report](#the-report)

## The storybook

**The storybook.** Write `campaigns/<id>/README.md` — the
reader-facing introduction. Background and setting ONLY: premise, lore, public
NPC introductions (never a persona's `secret`), classes, playtime, the build and
play commands. No puzzle solutions, no quest structure, no endings. Images are
relative links into `media/`, small JPEGs, exterior or starting-scene shots
only, picked from the visual-review set — never interiors or late-game
locations. A localized `README.<code>.md` per declared language.

Storybook art is Chunky — the install from step 12 — in two passes. Draft with
`delvec snapshot` — fast, disposable, for judging *layout*: is the right thing in
frame, from the right side, at the right distance. Then produce the shipped
image with `$DELVEWRIGHT_ENGINE/validation/chunky.sh` from
`$DELVEWRIGHT_ENGINE/validation/render-shots.sh`'s scene set, plus `delvec --prefabs "$DELVEWRIGHT_PREFABS" panorama <build-dir> -o
<dir>` for the whole-map hero shot every release owes (`--bearing` picks the
corner). Never hand-edit a scene JSON: if the frame you want is not emittable,
that is a `delvec render` gap to report, not a file to patch.
The whole-map hero's distance and the sky: `$DELVEWRIGHT_ENGINE/docs/reference/showcase-shots.md` §2a–§2b.

## The engine-version marker

**Every edition opens with the engine-version marker**, on its own line directly
under the title. This is the one piece of internal machinery a storybook carries
— it is what a server host needs before running the delve — so it stays in this
exact form and nothing else internal joins it:

```
> **Requires delve engine <max per-stage dsl_version> or newer** — last verified with delvec <version> on Minecraft Java <mc version>.
```

The first number is the MAX `dsl_version` over the campaign's documents; the
second is `delvec --version`'s, from the build that just went green; the third
is the game version that same line prints, which is the one number a host needs
before they can join at all. The check parses the line anchored and whole, so a
marker missing the Minecraft clause is reported as MALFORMED rather than as
missing, and the expected line is printed back verbatim. The line is
byte-identical in every localized edition — it is a version stamp, not prose. A
translated gloss may follow on the next line but may not restate the numbers.

**Write no other version number anywhere in the storybook.** The marker is the
only one a check can keep true; every other is hand-typed and goes stale in
silence. So: no campaign-version stamp, and the host command names `:latest` —
that IS the storybook's claim — with one sentence sending a reader who wants an
exact version to the release page, where the tag is machine-written.

**That includes the connect line, which is where it bites.** The report's
wording names the game version because it is going into a chat message; written
into the storybook it is a second version literal and the check refuses it by
name. Point
at the marker instead — it already carries the number, and it is the one copy
anything keeps true:

```
Start the server, then in the Minecraft Java client at the version the marker
above names: Multiplayer → Direct Connect → `localhost:25565`.
```

`localhost:25565` and `-p 25565:25565` are safe to write: the check knows a port
from a version.

## The render-distance line

**The render-distance line, when the delve declares a view distance.** A
client draws the smaller of its own render-distance setting and the server's,
and nothing on the server can raise a client's; the pinned client's default is
12 chunks. So a storybook for a delve whose `world.view_distance` is above 12
carries, directly under the connect line, what the player sets — the number is
the build's `server/README.md`'s, copied, never typed from memory:

```
Set your render distance to at least <view_distance> chunks (Options → Video
Settings) — the far views this delve was designed with are not drawn below it.
```

Then prove it:

```sh
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/creator/check-storybook-version.py" --campaigns campaigns
```

Green before you report. A stale marker waves a host on an old engine straight
into a delve their engine cannot run.

## The host line

**The host line, for a server strangers join.** A delve does not clean itself, so a
storybook whose reader will leave it running for people they do not know owes them
one more line — with its consequence in the same breath, because that half is not
guessable:

```
Add `-e DELVE_RESET_WHEN_EMPTY=90` and the world is thrown away and built again
from the image once nobody has been online for 90 seconds, so the next arrival
starts a delve nobody has touched — but then EVERY start resets, and restarting the
container under a party ends that party's run. Leave it out and the world is kept.
The floor is 60 seconds; below it the server refuses to start.
```

## The play server

**The first time the user plays the delve is the finished first version**, so
what you hand over is a build that has passed everything a machine can check:
step 10's ladder green on it, step 11 where it applies, and your step 12 visual
review done. Nothing is handed over before those, and nothing is handed over
that they were not run on. Bring the server up once yourself before you report, by
either path below: a build that does not reach READY is not handed over, and
the staging gate it runs is what the report lists by class.

**One command does all of it** — it builds the campaign, runs the staging gate
against that exact tree, starts the container, and verifies over rcon that the
datapack actually loaded before it says READY:

```sh
"$DELVEWRIGHT_ENGINE/tools/creator/playtest-server.sh" up campaigns/<id> \
    --prefabs "$DELVEWRIGHT_PREFABS" --delvec "$(command -v delvec)" --out "$PWD/.out/delve" \
    --run-report "$DELVEWRIGHT_ENGINE/validation/run-out/dw-<campaign>-r1/run-report.json" \
    --written-world .out/written-world.json
```

**`--run-report` and `--written-world` are step 10's two records**, and the gate
admits only on them: the bot's critical path green, and the server's world equal
to the engine's model, each for a build with this build's manifest. The build
this command makes is that build when nothing changed since step 10 — the build
is deterministic. A record of another build, a red one, or none is refused, and
`--stage-anyway` does not reach that refusal.

**`--delvec` is optional; this prints it so the command is exact.** Without
it the script uses the `delvec` already on `PATH` when that binary IS this
engine — its `--version` equal to the engine checkout's `versions.toml`
`[engine].version`, which is what Init I3a installed — and otherwise builds
from source. Either way it prints, in one line, which binary it chose and why,
before it builds anything. Read that line: if it says it is building from
source, the `delvec` on `PATH` is a different engine from the checkout at
`$ENGINE_REF`, and that disagreement is worth stopping for.

It writes its own build tree wherever `--out` says, so nothing about this path
touches the engine's `validation/` directory, and it daemonizes — it prints the
connect line and gives you your shell back. `up` also TAKES the host-25565
mutex and holds it until `down`, so no automation can bind the port under the
user's feet. Its staging-gate report goes to `<out>.gate/staging-gate.md`,
beside the build tree, and it prints that path. The user takes it down with
`"$DELVEWRIGHT_ENGINE/tools/creator/playtest-server.sh" down --name <name>`,
which also frees the mutex; `--name` defaults to `dw-playtest`.

**A large campaign can need more than the heap the build states.** `up`
gives the server the ceiling the build computed for its declared view distance
(`server/resources.properties` `heap-max`, never below `versions.toml`
`[server].heap_max`, printed as `container heap:`) unless you pass
`--memory SIZE`; a build with many prefab tiles can still exhaust that (`docker logs` shows
`java.lang.OutOfMemoryError` and the readiness probe says so rather than
reporting a content defect) — re-run with `--memory 8G` or higher. A build,
boot or probe failure past this point removes the container it started and
releases the mutex on its own; nothing is left running for you to find later.

The **second path** is the compose pair, for a tree step 8 already left at
`"$DELVEWRIGHT_ENGINE/validation/delve-output"` — it serves that tree instead of
building a fresh one:

```sh
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/creator/staging-gate.py" --campaign campaigns/<id> \
    --build "$DELVEWRIGHT_ENGINE/validation/delve-output" \
    --run-report "$DELVEWRIGHT_ENGINE/validation/run-out/dw-<campaign>-r1/run-report.json" \
    --written-world .out/written-world.json --report .out/round-1-gate.md
EULA=TRUE docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile play up
```

**`--report` goes outside `--build`, and the gate refuses it otherwise.** The
report names every ledger row, so it prints the strings the ledger's own probes
search the build tree for; one written inside the tree is a file the next run
counts as evidence about the build. Above it lands in `.out/`, beside the tree
rather than inside it.

That form runs in the FOREGROUND and holds the terminal until it is stopped;
`docker compose … down -v` is its teardown. Either way the gate runs first —
`owner-play.yaml` refuses to start without an admission token minted for that
exact build tree, and `playtest-server.sh` calls the gate itself rather than
trusting anyone to remember.

## The staging gate

It is not optional and not skippable by going around it: `owner-play.yaml` is
the only file that publishes 25565, and it refuses to start the server without
a token the gate minted for *that exact build tree*.

**A site-plan build with no place detailed is refused outright** — with or
without `--strict`, and out of `--stage-anyway`'s reach. A campaign is staged
only once detailed; step 9 is what makes it so.

**A red gate is not a defect count.** It is the list of defect classes a
player is not protected from, drawn from every finding ever reported on any
campaign — so a campaign that contains none of the objects a row is about shows
as `UNBOUND`, and that is a fact about the ledger, not about your delve. Read
the list, put it in the report by class, and never backfill a weak check to turn
a row green.

**A class this build cannot exercise is not in the red list at all.** A row
whose class measured zero across the whole declared design is `INAPPLICABLE`,
and is printed in its own section. Those rows still go into the report, by class; they do not
refuse the build. A row in the RED list is worth stopping for.

To go in anyway on a build you know is red:

```sh
"$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_ENGINE/tools/creator/staging-gate.py" --campaign <dir> --build <out> \
    --stage-anyway "<why this session needs a red build>" --acknowledge-red <N>
```

`<N>` is the red count the run above printed, and it must match exactly — the
number is the acknowledgement, so it cannot become a flag typed from memory. Two
runs over one unchanged campaign and one unchanged build tree give the same
number, so the N you just read is the N to type; if it has moved, something
about the campaign or the ledger really did.

It prints every class being overridden, records the reason, and the server
announces it at boot — so anything hit from those classes in that session is the
override, not a new finding.

## The report

**Then report to the user** — this hand-over ends the run: the campaign
summary, the playtime estimate, the validation results, anything still open,
**what the delve asks of its
host** — the view distance it serves and the heap the build stated for it
(`server/resources.properties` `heap-max`, which the image and the playtest
server start with; the hosting side meets it, and a host that cannot runs the
image with `-e MEMORY=<size>` knowing what it gives up) — and the two commands
they will actually use, below. It also carries:

- **how to get in** — Minecraft Java 1.21.11 → Multiplayer → Direct Connect →
  `localhost:25565`. That wording is for the message you send them and nowhere
  else: the storybook's own connect line names no game version;
- **what the machine could not check**, by class: every class the staging gate
  reported red or could not exercise, and — per item — every finding still open
  from an earlier round that they must **not** test (see *Playtest rounds*, rule
  2). Every `world.textures[]` row is its own item, named as the player sees it
  (*the red moon*, *the drowned on the shore*), with the sheet `delvec textures`
  wrote beside it — no machine can confirm a replaced texture; and tell them to
  **accept the resource-pack prompt** when they join.

```sh

# play — one command: build, gate, serve, and print the connect line
"$DELVEWRIGHT_ENGINE/tools/creator/playtest-server.sh" up campaigns/<id> \
    --prefabs "$DELVEWRIGHT_PREFABS" --delvec "$(command -v delvec)" --out "$PWD/.out/delve" \
    --run-report "$DELVEWRIGHT_ENGINE/validation/run-out/dw-<campaign>-r1/run-report.json" \
    --written-world .out/written-world.json

# playtest, with in-game notes
EULA=TRUE CREATOR_NAME=<mc name> docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile playtest up
```

`owner-play.yaml` is what publishes `localhost:25565`; the base compose file
publishes nothing. Both paths run the staging gate first — see *The staging
gate* above.

---
