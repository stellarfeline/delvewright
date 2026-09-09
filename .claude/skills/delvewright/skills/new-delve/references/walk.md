# Step 9 — the walk


**You have no body in the game.** You bring the world up and the user walks it;
a blockout somebody has stood in tells them things no picture and no green check
will: scale, whether the route reads, whether the silhouette is the thing that
was designed.

It happens **now**, before the ladder and before any review — every step after
this costs more to redo than to defer.

Start the server. **One command does all of it** — it builds the campaign,
runs the staging gate against that exact tree, starts the container, and
verifies over rcon that the datapack actually loaded before it says READY:

```sh
"$DELVEWRIGHT_ENGINE/tools/playtest-server.sh" up campaigns/<id> \
    --prefabs "$DELVEWRIGHT_PREFABS" --delvec "$(command -v delvec)" --out "$PWD/.out/delve"
```

**`--delvec` is optional now; this prints it so the command is exact.** Without
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
user's feet.

The **second path** is the compose pair, and it is the one to take when step
8's tree is already sitting at `"$DELVEWRIGHT_ENGINE/validation/delve-output"`
and the ladder is coming next anyway — it serves that tree instead of building
a fresh one:

```sh
python3 "$DELVEWRIGHT_ENGINE/tools/staging-gate.py" --campaign campaigns/<id> \
    --build "$DELVEWRIGHT_ENGINE/validation/delve-output" \
    --report .out/round-1-gate.md
EULA=TRUE docker compose -f "$DELVEWRIGHT_ENGINE/validation/compose.yaml" \
    -f "$DELVEWRIGHT_ENGINE/validation/owner-play.yaml" --profile play up
```

That form runs in the FOREGROUND and holds the terminal until you stop it.
Either way the gate runs first — `owner-play.yaml` refuses to start without an
admission token minted for that exact build tree, and `playtest-server.sh`
calls the gate itself rather than trusting anyone to remember.

Then hand the user, in one message:

- **how to get in** — Minecraft Java 1.21.11 → Multiplayer → Direct Connect →
  `localhost:25565`. That wording is for the message you send them and nowhere
  else: the storybook's own connect line is step 14's, and it names no game
  version;
- **what to look for, item by item.** Not "have a look". Name the scale
  question, the route, each silhouette you are unsure of, and — per item — every
  finding still open from an earlier round that they must **not** test (see
  *Playtest rounds*, rule 2). Anything the staging gate reported red goes in this
  list by class;
- **how to tell you they are done.**

**Then end your turn and wait.** Do not run the ladder, do not start step 12,
and do not write `walk-record.json`. When they report back, their words are the
finding — record them, and take the server down:
`$DELVEWRIGHT_ENGINE/tools/playtest-server.sh down --name <name>` — which also frees
the 25565 mutex. `--name` defaults to `dw-playtest`; pass the one you brought up.

The staging gate is not optional here and not skippable by going around it:
`owner-play.yaml` is the only file that publishes 25565, and it refuses to start
the server without a token the gate minted for *that exact build tree*.

**A red gate is not a defect count.** It is the list of defect classes a
playtester is not protected from, drawn from every finding ever reported on any
campaign — so a campaign that contains none of the objects a row is about shows
as `UNBOUND`, and that is a fact about the ledger, not about your delve. Read
the list, put it in what you hand the user item by item, and never backfill a
weak check to turn a row green. To go in anyway on a build you know is red:

```sh
python3 "$DELVEWRIGHT_ENGINE/tools/staging-gate.py" --campaign <dir> --build <out> \
    --stage-anyway "<why this session needs a red build>" --acknowledge-red <N>
```

It prints every class being overridden, records the reason, and the server
announces it at boot — so anything hit from those classes in that session is the
override, not a new finding.

For a site-plan campaign **this walk is the campaign's first real gate**: scale,
pacing, route legibility, and the silhouette from the declared `views[]`. Say so
when you hand it over — the user is not being asked to admire it, they are the
gate. A finding edits the graph or the plan and regenerates — there is no hand edit to
lose, because there was never a hand edit to make.

`docker compose … down -v` is the compose path's teardown.
