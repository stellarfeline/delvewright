
# Reference: when something goes red

The symptoms most likely to stop you, and what they actually mean.

## Contents

- [Validate and build](#validate-and-build)
- [Run and stage](#run-and-stage)
- [Toolchain reds](#toolchain-reds)

## Validate and build

**`DW0874`: "this campaign directory is missing N of the 6 stage documents"
(exit 1).** Not a fault — `delvec validate` is whole-campaign and needs all six
required documents on disk. The refusal names every one that is absent, the
whole set of six, and what a stub is; stub the ones you have not written yet.
Step 1 has the names and the one document whose stub is not empty.

**`internal error: cannot read campaign dir: …` (exit 10) instead.** That is a
different finding and it is not an authoring state: either the path is not a
campaign directory at all, or a document is there and cannot be opened. Check
the path first.

**Step 3 ends red and nothing you do clears it.** Expected — read *Where step 3
ends*, which gives the rule: a refusal whose message names something only
`quests.json` or `dialogue.json` can supply is the state of a campaign whose
plan is written and whose quests are not, and it clears at step 5. It is a list
of codes, not two, and the list is longer than it looks — `DW0172`, `DW0482`
and `DW0197` are the same absence seen from the branch, the fork and the cast.
Writing empty stage-5 quests to get past them makes it worse, and `DW0150` says
so.

**`DW0991`: "the only route … climbs a climbable the world does not keep".** A
ladder or vine on the route has nothing to hang on — usually a ladder facing
INTO its wall. The message names each rung and the face it lacks. Turn the
ladder to face away from the wall (`facing` is the way a body climbing it has
its back), or give it a sturdy block behind; never move the objective.

**"shut gate(s) do not separate their two sides".** A body gets
round a shut gate — a gated connection's bar, a gate station, a prefab gate — by walking,
falling, jumping or swimming, and reaches the far side before the story opens
it. The message names the gate, the quest step it is shut at, the cell the way
round starts from and ends at, its moves by kind, where it leaves the walked
ground and where it is back on it, and the gates it passes (a clocked door
counts as open). Close the way round in blocks: wall the water off where the
body goes in (down to the bed, not a course over the surface — a body does not
dive, but it swims across the top), raise the edge it climbs, jumps or drops
over, or close the second way the plan left open. Never fence walkable-looking
ground with an invisible barrier or mark it unwalkable, and never move the
gate's objective to dodge it. If the party is meant to get round the gate —
the door closes a loop — declare the connection `shortcut`, or, where only one
side may open it, name that side in its `gating.opens_from`.

**A gated connection's seam "is a front".** A `gating` builds a way shut with
a bar in its opening, and a `contact` has no opening. Give the seam a standard
`opening`, or remove the `gating`.

**A gated connection "nothing in this campaign opens".** Nothing names its
`anchor/seam-<edge>`. Write the `open-gate` at the beat whose completion the
gating names (or a `shortcut`, or a `timed-gate`), or remove the `gating`.

**`DW1002`: a block "the server drops", "not kept where it is written", or
"not kept in some configuration the story can reach".** The server will remove
a block the build writes: it pops off as an item, or falls. The message names
the block, its cell, the write (function and line of the shipped datapack, and
the command) or the piece holding it, and what its neighbour must give. Repair
it where the block is authored, never by moving the write later:

- *falls* — gravel, sand or concrete powder over air when its write has run.
  Give it a block under it **in the same piece or edit that writes it**; ground
  written by a later step is not under it yet. The street surface of a piece
  carries its own bed.
- *needs the block … to give …* — a torch, lantern, lever, button, flower or
  carpet with nothing to hang or stand on. Hang it on a block no write removes,
  or state the face it is on (`stone_button[face=floor]` stands on the block
  under it; the bare `stone_button` hangs on the wall north of it).
- *may hold … left by …* — a runtime write (a gate, `clear-region`, a collapse,
  a trap's or a branch's `set-block`) can leave its support missing. Move the
  block off that cell's neighbour, or have the write itself lay what holds it.

The build stops before a server boots; `check-written-world.py`'s item-entity
count is the server's own answer if one is booted.

**"The party walks up to its back" (build, exit 3).** A body the party
talks to faces away from the way the route delivers them, and its watch does
not turn it to them — a class watch turns only to that class, and a short
`within` may not reach the approach. Its home anchor's `facing`, or the turn a
walk left it on, is more than 90° off the party's approach. Declare the `watch`
the message prints on that npc, so it turns to the party as they arrive; or
stand it where the party comes at it from the front. Never move the objective
or reroll to change the approach.

**"… of the N cell(s) a player can talk to it from lie behind it" (build,
exit 3).** An NPC that declares `"watch": "none"` is judged from every side a
player can talk to it from, not only the route's. When one facing faces every
one of those cells the message names it: declare the anchor's facing that way.
When none does, the body stands in the open: give it an `offset` (and its
cast entry the same mark) that puts a wall or a counter at its back. If the
scene does not really need the body to stay still, remove `"watch": "none"`
and it watches the party.

**"The party can see one person twice" (validate, exit 1).** Two declarations
carry one name and some story state has both in the world — usually a character
re-declared per scene and spawned without the previous body leaving. Despawn the
leaving body in the same effect list that spawns the next one (one list is one
tick, so the swap is never seen); a despawn in a later `sequence` step or
`on_arrive` leaves both standing meanwhile. If they really are different people,
give each its own `name`.

**`DW0891` says to seat the volume "on an anchor at the pit's bottom" and the
place is detailed.** A quest names only the node's owed names, so declare a
station on the node (`kind: point`), bind it in `detail-plan.json` to the
piece's anchor on the pit's floor, and put the volume's `region.anchor` on it.
That anchor may stand in the piece's `no_body` region. `DW0845` refuses it only
when something else (an NPC, an objective, a wave) names the same station, and
it names that use: give the volume a station of its own.

**`DW0438`: a `collect` "adopts a holder that is not there" — the anchor
"holds `minecraft:air`, not a holder".** The anchor the collect's `holder` names
is not standing on a container, an item frame or an armour stand — an anchor
named `anchor/chest` is not evidence there is one. The message lists every
anchor in the assembled world whose cell holds a container nothing else fills:
point `holder` at one of those. When it lists none, the holder is furniture a
piece must carry — detail the place with a piece that places one. Dropping
`holder` is not the fix: the compiler then conjures its own chest beside the
anchor. Read *Reference: what a quest can do* → *Items, containers and loot*.

## Run and stage

**A bot step times out with "objective … did not complete".** Read the rest of
that line before touching the campaign. The bot reports the server's own answer
to the `/trigger` it sent: *the server ANSWERED …* means the trigger reached the
delve and a datapack guard consumed it — a re-used world whose scoreboard already
carries the objective does exactly this, so run `fresh-volumes.sh --project <id>`
and re-run before believing the content is at fault. *The server never
answered …* means the command never got there and the failure is the harness's,
not the delve's.

**`failed to read dockerfile: open Dockerfile.delve`.** The build tree is
outside the engine's `validation/`, on the one path that resolves the
Dockerfile against the tree: a bare `docker compose` on the `play` or
`playtest` profile with no `DELVE_DOCKERFILE` exported. `packtest-run.sh`,
`bot-run.sh` and `branch-runs.sh` pass an absolute path and never raise it. Build into
`$DELVEWRIGHT_ENGINE/validation/delve-output`, or copy the tree there — see step 8.

**`refimg: no delvewright.local.toml` (exit 2).** Init I7, path B — and it
is the state **every** fresh engine checkout is in, not a file that went
missing. The name is gitignored in the engine, so nothing ever shipped one and
nobody deleted it: writing it is the step, and it is owed again for every new
checkout. The file belongs at
`"$DELVEWRIGHT_ENGINE/delvewright.local.toml"`, not here — the tool resolves it
against its own checkout and takes no `--config`. It names
the file, the section, and where the template is. If the campaign already has an
approved `design/` directory you are on path A and do not need this at all.

**The staging gate refuses with a long UNBOUND list.** Not a defect count — see
step 13. Read it item by item into the round summary; override deliberately if
the session needs a red build.

**`delvec metrics` "prints 223 lines of JSON, not a table".** It prints the
table as JSON on stdout and its human summary and binding counts on stderr.
Redirect them separately: `delvec metrics > table.json`.

## Toolchain reds

**A red that came from the toolchain rather than the campaign.** Stop content
work and report it, with evidence. Never hand-edit compiler output, never
restructure the campaign to dodge it, never weaken a check or reroll a seed to
get green. Escalating is success.
