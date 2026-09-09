
# Reference: when something goes red

The symptoms most likely to stop you, and what they actually mean.

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

**`DW0311`: "the player cannot walk from [x] to [y] … no collision-free path",
and the two points are hundreds of blocks apart.** They are in different areas,
and the crossing between them was never emitted. The message's own suggestions —
a wedged doorway seam, a void gap, a fence ring, a `close-gate` to reopen — are
about the same-area case and will not apply. Read step 2A: **the destination
area's piece must declare an entry point**, and only 5 of 36 shipped prefabs do.
Bind that area to one of them, or make a piece that has one.

**The build is green, PackTest is green, and the bot fails its FIRST step with
"No path to the goal!"** The party's first objective is in an area they did not spawn in,
and no crossing carries them there — the delve is not completable and nothing
before the bot said so. Read step 2A rule 1: put a beat in the spawn area first,
or move the spawn. Confirm by reading `critical-path.json` at the build output's
root: the first `reach`/`talk-to` step should be at coordinates inside the spawn
area, and a step that crosses areas carries a `transport` key.

**`DW0438`: a `collect` container "holds `minecraft:air`, not a container".**
The anchor you pointed at is not standing on a chest or barrel — an anchor named
`anchor/chest` is not evidence there is one, and two shipped pieces declare
exactly that over air. Read *Reference: what a quest can do* → *Items,
containers and loot*: use `dropped_by`, or drop the `container` field and let the
compiler place one, or make a piece that carries the container. Do not point the
field at a different anchor and hope.

**`DW0857`: an anchor "is provided by 2 of this campaign's areas".** You bound
one prefab to two areas. The remedy available to you is **a different piece for
one of them** — not renaming an anchor, which belongs to the shared library and
would change it for every campaign.

**A bot step times out with "objective … did not complete".** Read the rest of
that line before touching the campaign. The bot reports the server's own answer
to the `/trigger` it sent: *the server ANSWERED …* means the trigger reached the
delve and a datapack guard consumed it — a re-used world whose scoreboard already
carries the objective does exactly this, so run `fresh-volumes.sh --project <id>`
and re-run before believing the content is at fault. *The server never
answered …* means the command never got there and the failure is the harness's,
not the delve's.

**`failed to read dockerfile: open Dockerfile.delve`.** The build tree is
outside the engine's `validation/`, on one of the paths that resolves the
Dockerfile against the tree: `branch-runs.sh`, or a bare `docker compose` on the
`play` or `playtest` profile. `packtest-run.sh` and `bot-run.sh` pass an
absolute path and never raise it. Build into
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
step 9. Read it item by item into the round summary; override deliberately if
the session needs a red build.

**`delvec metrics` "prints 341 lines of JSON, not a table".** It prints the
table as JSON on stdout and its human summary and binding counts on stderr.
Redirect them separately: `delvec metrics > table.json`.

**A red that came from the toolchain rather than the campaign.** Stop content
work and report it, with evidence. Never hand-edit compiler output, never
restructure the campaign to dodge it, never weaken a check or reroll a seed to
get green. Escalating is success.
