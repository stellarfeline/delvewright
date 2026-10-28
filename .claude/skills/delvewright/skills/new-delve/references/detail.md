# Step 9 — detail

## Contents

- [What a detail plan is](#what-a-detail-plan-is)
- [Each place has its own author](#each-place-has-its-own-author)
- [The loop](#the-loop)
- [The program's contract](#the-programs-contract)
- [After a plan or graph edit](#after-a-plan-or-graph-edit)
- [What `GENERATION.md` records](#what-generationmd-records)

## What a detail plan is

Required before hand-over, for **every** place: the staging gate refuses a
build with no detail plan, and one whose derivation still stands a
stand-in in any place — a stand-in never ships. Places are authored
concurrently and committed one at a time. Until a place is detailed the derivation masses it with a stand-in —
walkable, legible, made of concrete; detailing replaces that place's stand-in
with a real building — so the map builds, walks and renders at every point
between none detailed and all of them, and the build records every place still
massed in `validation/blockout.json`.

`detail-plan.json` has two fields and **there is no coordinate in it and no way
to write one** — no region, no extent, no datum, no seam, no offset. A `place`
and a `piece` is all a row can say:

```json
{
  "palette": { "role/wall": "minecraft:stone_bricks" },
  "details": [
    { "place": "node/near-hall",
      "piece": "prefab/near-hall",
      "anchors": { "anchor/node-near-hall": "hearth" } }
  ]
}
```

**Nobody writes that row**: one verb writes it, from the place's program. Where
the piece goes is computed from the site plan: the **frame** is the
place's claim — the ground under its plot, its floor course, its play space,
its one-cell ring and, roofed, its lid and roof zone — as a bounding box. The
piece must be **exactly** that shape — undersize is refused the same way
oversize is (`DW0843`), because a smaller building means a smaller box, which is
a site-plan edit. Every cell of the frame the place does not own — a
neighbour's wall, the ring's fixed ground, nobody's — is a **void**: the verb
writes `minecraft:structure_void` there after expansion, so whatever a program
paints at a void is replaced and never reaches the piece; a piece already in
the library that paints a void (air included) is `DW0987`, one that writes the
ring's fixed ground `DW0990`.

## Each place has its own author

**You do not write a program.** Each place's program is written by a **new
agent** that is handed only that place's own necessary information — what the
whole hands it, and that place's rows of the campaign's design — and a summary
of each adjacent box: what it is, its floors, its facade and material, its
roofline, and the seam joining the two. The agent writes one document, checks
it with `detail --check`, and reports. You **commit** what it left, with
`delvec detail`, record the attempt, and keep the wave full. A refusal is
repaired by a **fresh** agent, with the refusal added to its handout; you never
edit a program yourself, and you never read one.

The handout is a file a tool writes from the campaign on every invocation. You
type nothing into it: to tell a place's author more, edit a document the
handout quotes — the node's `note` in `layout-graph.json`, the seam's `form` in
`site-plan.json` — and run the tool again.

**Authors run concurrently, commits run one at a time.** The check and the
commit are two runs of one verb: `delvec --prefabs "$DELVEWRIGHT_PREFABS" detail --check <campaign-dir> <place>`
binds, expands, voids, runs every gate of the place and the bindings check over
the whole **as committed on disk**, and writes nothing; `delvec --prefabs "$DELVEWRIGHT_PREFABS" detail
<campaign-dir> <place>` runs the same judge, then writes the piece, its report
and the plan row — each through a temporary file renamed into place, so a check
running at that instant reads the old file or the new one, never a torn one —
and then runs the battery over the committed whole. Up to **N** authors are in
flight at once, N the line `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
<campaign-dir> --all` prints on stderr, `concurrency: N = min(P, max(1, C /
2))` — P the places with no passing program, C the cores the machine offers.
You never type N; the handout's header quotes the line. The order is the
critical path from `entry`, then every other place in the order `allocation
--all` prints them, scenery last — a spine place no program can answer is found
before the rest of the map is spent on it.

A place filled by a piece the library already has, or by one admitted through
`delvec prefab` instead of written as a program, is bound by you without an
agent: *Reference: when the prefab library has no piece you need* and
*Reference: the shipped library* say how.

## The loop

0. **Gather the references, before the first handout.** For every place with
   a `clearance` ceiling — a building — find at least two pictures or pages of
   human builds of that kind of building (a forge, a general store, a
   weaver's), read each for where the working object stands, how the rooms
   divide and what the front shows, and record each as a row of the node's
   `references[]` in `layout-graph.json`: `{url, shows, terms}`, the URL
   `https://`, `shows` one sentence of what to take from it, `terms` what the
   source states (`"none stated"` where it states nothing). This is yours and
   not the place's author's: the rows are a design document's, and an author
   holds nothing but its handout. A reference is looked at, never copied — an
   unlicensed or non-commercial source is ideas only. Then fetch them:

   ```sh
   "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/references.py" <campaign-dir> --all
   ```

   It writes each picture to `.out/references/<place stem>/<n>.<ext>`, records a
   page as a page and a failure as failed, writes `index.json` beside them, and
   prints `references: R row(s); F image(s) fetched, P page(s), E failed` per
   place. The handout names each fetched file, and the author reads it with its
   file tool. Nothing under `.out/` is committed.

1. **Write the handouts of the first wave**, at most N, in the order above.
   Mandatory, and the only input to each agent:

   ```sh
   "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/handout.py" <campaign-dir> <place> \
       --engine "$DELVEWRIGHT_ENGINE" --prefabs "$DELVEWRIGHT_PREFABS"
   ```

   It runs `delvec allocation` itself, writes
   `<campaign-dir>/.out/handouts/<place stem>/handout-<n>.md` — `n` is the
   attempt, counted from the files already there — and prints two lines: the
   handout's path, and the one-line prompt you launch the agent with. Beside the
   handout it writes `allocation.json`, the place's allocation, and
   `starter.py`, the starter generator: run on that allocation it writes an
   empty-but-correct program — floor, ring wall, lid, every seam answered,
   every owed name marked, sconces — that passes `detail --check` as it
   stands, with the places to build marked inside it. The handout makes it the
   author's step 1, so an author starts from a passing program and never from
   research: nothing outside the handout's reading list — the engine's
   source, its gallery, another campaign — is needed or read. Where the
   place already has a program it runs `detail --check` on it: a program
   that passes is **exit 3** and no handout (commit it, step 3), a refused one
   puts the verdict into the handout verbatim as the refusal the next attempt
   repairs. A place the plan allocates no box to is exit 1, naming every place
   it does; a directory that is not a campaign, an engine tree or a prefab
   directory that is not there, or a `delvec` that states no concurrency line,
   is exit 2. Read its stderr: the handout's binding — design rows, neighbours,
   seams, voids, owed names, refusal lines, open sides, references — and the
   concurrency line. The
   handout says which neighbours are detailed **as of the handout**, and that
   the author's check reads the whole as it stands when the check runs.
   **Do not read the handout**: it carries the allocation, and holding it is
   how a session starts authoring.

2. **Launch each agent** with the **Agent tool**: `subagent_type`
   `general-purpose`, and as its prompt exactly the second line the script
   printed — `Read and execute <handout path>. Report, then exit.` Nothing is
   added. **Never a fork**: a fork inherits this session's whole context, every
   other place and the design included, which is the one thing the agent must
   not hold. The handout tells the agent what it may read (the handout, its
   own program, the starter, the authoring references, the grammar corpus),
   that it starts by copying the starter to `design/generator/<place stem>.py`,
   running it and checking it green before it builds, what it may run
   (`delvec allocation` for the same facts again; `delvec grammar expand` at
   the frame and a render to look; `detail --check` to check), what it
   writes (`programs/<place stem>.json`, and a generator script beside it under
   `design/generator/` when the program is emitted), and what it never does:
   read another place's program, the design record, the site plan, the layout
   graph, the quest documents or another handout; edit any other document; run
   `detail` without `--check`, `detail --all`, `build`, a server or
   `git`; retype a handed number; weaken a check. It returns its report with
   the exit of its last check.

3. **On each return, in return order, commit** — serially, never while another
   commit runs: `delvec --prefabs "$DELVEWRIGHT_PREFABS" detail <campaign-dir>
   <place>`. The place's own gates and the bindings check run before anything
   is written; a place that passes them is **written**, and the battery then
   builds the whole that now stands on disk, so a red from the battery leaves
   the piece and its row in place and names the place. Each owed anchor name is
   re-bound to one of the piece's own anchors (`DW0845`) — that is what keeps
   the quest layer working, because those names were bound to places before any
   detail existed and detailing must never force a quest edit — and a way the
   plan did not allocate is refused in both directions (`DW0844`). **Exit 0**:
   record the attempt, write the next unstarted place's handout and launch its
   agent, keeping at most N in flight.

4. **On any other exit, hand it back**: the author's check passed against a
   whole a neighbour has since moved, or passed and the battery over the
   committed whole refuses. Capture every commit's output to a file
   (`… > .out/commit-<stem>.txt 2>&1`). A refusal before the write is what
   `detail --check` gives too: run step 1 for that place again and the script
   embeds the check's verdict. **A battery refusal is one `--check` cannot
   see** — the piece is already written, its own gates passed — so hand the
   commit's output to the script: step 1 with `--from .out/commit-<stem>.txt`
   embeds it verbatim. The battery names the place whose piece it judges;
   where it names a cell, the place whose frame holds that cell is the one
   handed back. Either way the script writes `handout-<n+1>.md` and you
   launch a fresh agent on it. While a battery refusal stands, every later
   commit's battery repeats it: commit the others as they return — each is
   written once its own gates pass — and read each commit's refusals for the
   ones that name it. The
   refused program stays where it is; the new agent repairs it. **Three
   consecutive refusals of one place naming the same diagnostic code** mean the
   cause is upstream of the program — the plan's box or seam, a `form` no
   program can answer, or a capability the grammar lacks. Launch no fourth
   agent for it: act on the cause — a plan edit (below), or the page's rule for
   a missing capability: author the closest alternative, report what the player
   will see, and record the missing capability with `debug-note.py` (`--kind
   limit --step 9/<place>`). A **spine** place refused three times with one
   code stops new launches: authors already in flight return and are committed
   or handed back, and the cause is acted on before anything else is launched.

5. **Record every attempt** in `GENERATION.md` (below), and every refusal you
   hand back with `debug-note.py` (`--kind refusal --step 9/<place> --from
   <the commit's captured output>`).

## The program's contract

What the agent's program has to meet — the handout carries it, and you read it
here only to know what a refusal is about. The program lives at
`programs/<place stem>.json` inside the campaign, by hand or emitted by a
script (*New pieces*, step 3; a script reads the handed values from the
allocation and retypes none). The shape a generator takes is the engine's own
worked example, `$DELVEWRIGHT_ENGINE/gallery/overlays/site-plan/design/generator/annex.py`
(`$DELVEWRIGHT_ENGINE/docs/reference/grammar.md` §2c, *A place from its
allocation*): it reads the saved output of `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
<campaign> <place>`, runs no binary, names no path, and writes the program. It declares the handed values it uses as
parameters under the `handed/` prefix, with the allocation's values as their
defaults (`handed/datum-y`; `handed/ground/{min-y,max-y,bottom-y}`;
`handed/roof/…` on a declared roof;
`handed/seam/<edge stem>/{x0,y0,z0,x1,y1,z1,rise}`); a name the whole does not
hand is `DW0882`, which lists every name it does. It answers each seam with an
opening at the handed cells — `cells` is an inclusive box written as its two
opposite corner cells, and every cell between them is open — and a contract
edge to `exterior` whose `via` is that opening; a seam whose plane this place
does not own is answered at the place's own first layer beside it, because the
ring on that side is not the place's. It answers each owed name with a `mark`
of that stem (`anchor/node-annex` → `node-annex`) — and a gated seam whose bar
this place owns (the allocation hands its cells as the seam's `bar`, and the
block it is built of as `bar_block` — iron bars in a wall, a closed iron
trapdoor through a floor, which holds the body standing on it) with a `barred`
contract edge whose `bar` is over exactly those cells, filled with a palette
role of that block, its region named by the same stem (`anchor/seam-west-door`
→ `"region": "seam-west-door"`), which exports as the gate anchor that name
binds to (`DW0845`). It declares a spatial contract (`DW0843`). **The enclosed
spaces are the program's to declare**, and the closure gate confirms exactly
those: a street, a pavilion or a covered market declares none and passes. A
two-level interior is two spaces (a space is one floor) and a `climb` contract
edge between them, at program version `1.10.0`, proved over the body's climb
moves (`$DELVEWRIGHT_ENGINE/docs/reference/grammar.md`, the contract's edge
classes). **Several rooms on one floor are spaces joined by `walk` edges
through their doorways**, and a doorway between rooms holds a wooden door
block, which the walk opens by hand; a door the story shuts is iron, and a
`barred` edge. **An open side** (the allocation's `sides`) stands no wall: its
ring cells stay open from the walk plane up and the floor course runs across
them; a frontage a body crosses is answered with an `exterior` edge whose `via`
is those cells, and a counter a body does not cross stands at the play space's
edge, inside. **Your open side stands no wall; build the frame the wall would
have been one cell inside the ring, and stand what you open there in it.** A
scenery place (`"reached": false`) is judged sealed: no way in, no
floor or light owed. A ground place draws its own ground down to the terrain
the whole hands, ring included where the whole has not fixed it; an aloft one
is handed none. A declared roof's `clipped` runs and a neighbour's facade cells
are voids: whatever the program paints there is replaced after expansion, so
nothing put there survives and nothing may depend on it.

**Every door and gate closes an opening.** Shut, nothing passes through it; open, the place behind it is reached through it — the engine asks the body's reach, from either side. A door stands in a wall with wall joined to both sides of it at both courses and a floor a body can stand on in front and behind; a gate stands in its fence line. A door that closes nothing — standing free on a floor, beside a counter, against a wall with nothing behind — is not placed, or wall is joined to both sides of it; a room may have as many doors as it needs, each judged on its own. A door kept as a decoration (a loft door high on a facade, a bench top laid flat) is declared `decorative` on the piece's contract with what it is dressed as, and the engine then proves no player can touch it: from nowhere a body can stand is any of its cells within the game's hand reach (`4.5` blocks from the eye), or it is a door no hand opens (iron, with no control). A wooden door within reach that closes nothing is refused whatever it is declared; raise it out of reach, or make it close a way. A hatch is a trapdoor. A door-shut seam's door is the bar of its `barred` edge: the palette role is the door's lower half under its upper, the two halves filling the bar's two courses.

**A place reads as what it is when four things hold at once**: the working
object stands in its working position, not stored; the rooms follow the work —
the room a visitor enters is not the room the work is done in, and a door
between them says so, so a building of a trade has at least two rooms; the
front says what the place is before the door is opened — a sign of its own
object over the way, the goods shown at a window or a counter, or the workroom
open to the way; and every room has a use a player could name, with nothing as
filler. **The test**: stand in the door and name what the place is with no sign
read. The review does exactly that, blind, from a frame the build draws on each
threshold.

**A building is built in one style**, four lines read with the four above
(`$DELVEWRIGHT_ENGINE/docs/reference/place-craft.md` §7 carries the sources):

- **one module** — every part measured in the unit the body gives it: a course
  (a counter, a sill, a step), a body of two courses (a door, a window's clear
  height, the gap under a beam), a storey; parts that share a wall meet at its
  lines.
- **openings follow construction** — an opening is a hole in a wall with jambs
  and a head; what fills it follows how the wall is built, and the leaf is
  chosen last: a walled front, a front of posts and a beam, a timber-framed
  front, a masonry front each take a different counter and door.
- **lines run through** — the heads of a front's openings stand at one course,
  or in a hierarchy you mean; a beam, a fascia or an eave terminates the front
  over every opening; solid stands upon solid; one structural material and one
  infill, and every block kind has a job.
- **a landmark reads by its silhouette from the approach** — what the thing
  does is visible in its shape at eye height from the cell where a body enters,
  and its parts obey the first three lines.

No door, gate or counter is named per trade: the construction names it.

**Outside the door**: a street's verge — the columns of a way place outside its
declared clear width — is the street's to dress, and so is a yard cut as its
own open-sided place: shrubs, potted flowers, a hedge on a log, a bench, a
trough; dress for what stands opposite, which the neighbour summary names.
Nothing stands in a way's clear width.

**A wide way keeps a wide approach**: the approach to a stair or a door is as
wide as the way, at least three; its landing is as deep as the way is wide;
a hearth, a newel or a pillar stands three to five cells from a stair, a newel
in the next column over. **A seam at grade is walked at the sill**: both
places draw their ground to the sill at the crossing, the side the plan hands
the ring row to paves it, and no cell of a street's or a flight's corridor lies
below the walk plane with higher cells before and after it — a trench a body
walking the road falls into.

The agent iterates with `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar
expand --file … --region <the frame>` and a render until it reads as the place
— the program's `handed/…` defaults are the handed values, so no `--param` is
typed — and checks with `detail --check`, which is the only run that
sees the voids, the seams and the whole.

## After a plan or graph edit

Including one that answers a playtest finding in a later round:
`delvec --prefabs "$DELVEWRIGHT_PREFABS" detail <campaign-dir> --all`. Every
program-detailed place is re-made; one that no longer fits is refused by name,
and that place goes back through the loop above from step 1 — the script
captures the refusal as the next attempt's. The run judges the whole it would
leave, so the order of the places decides nothing, and it writes nothing
unless every place and that whole pass.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.

## What `GENERATION.md` records

One table, **Detail**, one row per attempt, written by you after the agent's
report and your commit: the place; the attempt number; the handout's path (a
regenerable file under `.out/`, which the table says once above it); the
agent's token reading and its duration as the Agent tool's completion reports
them; the diagnostic codes of the refusal that attempt repaired (none on a
first authoring); the exit of **your commit**. One line above the table states
the wave bound as `allocation --all` printed it. No date, no model name, no
agent id.
