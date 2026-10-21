# Step 9 — detail

## Contents

- [What a detail plan is](#what-a-detail-plan-is)
- [Each place has its own author](#each-place-has-its-own-author)
- [The procedure, per place](#the-procedure-per-place)
- [The program's contract](#the-programs-contract)
- [After a plan or graph edit](#after-a-plan-or-graph-edit)
- [What `GENERATION.md` records](#what-generationmd-records)

## What a detail plan is

Required before hand-over, for **every** place: the staging gate refuses a
build with no detail plan, and one whose derivation still stands a
stand-in in any place — a stand-in never ships. Places are detailed one at a
time. Until a place is detailed the derivation masses it with a stand-in —
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
writes `minecraft:structure_void` there, and a piece that paints a void (air
included) is `DW0987`, one that writes the ring's fixed ground `DW0990`.

## Each place has its own author

**You do not write a program.** Each place's program is written by a **new
agent** that is handed only that place's own necessary information — what the
whole hands it, and that place's rows of the campaign's design — and a summary
of each adjacent box: what it is, its floors, its facade and material, its
roofline, and the seam joining the two. The agent writes one document, checks
it with the one verb that judges a place, and reports. You confirm the verdict,
record the attempt, and launch the next place's agent. A refusal is repaired by
a **fresh** agent, with the refusal added to its handout; you never edit a
program yourself, and you never read one.

The handout is a file a tool writes from the campaign on every invocation. You
type nothing into it: to tell a place's author more, edit a document the
handout quotes — the node's `note` in `layout-graph.json`, the seam's `form` in
`site-plan.json` — and run the tool again. One agent runs at a time: the check
writes the plan row and the piece and judges the whole as that run would leave
it, so two agents checking at once race on both and judge a whole the other is
changing. The order is the critical path from `entry`, then every other place
in the order `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
<campaign-dir> --all` prints them, scenery last — a spine
place no program can answer is found before the rest of the map is spent on it.

A place filled by a piece the library already has, or by one admitted through
`delvec prefab` instead of written as a program, is bound by you without an
agent: *Reference: when the prefab library has no piece you need* and
*Reference: the shipped library* say how.

## The procedure, per place

1. **Write the handout.** Mandatory, and the only input to the agent:

   ```sh
   "$DELVEWRIGHT_PYTHON" "$DELVEWRIGHT_SKILL/scripts/handout.py" <campaign-dir> <place> \
       --engine "$DELVEWRIGHT_ENGINE" --prefabs "$DELVEWRIGHT_PREFABS"
   ```

   It runs `delvec allocation` itself, writes
   `<campaign-dir>/.out/handouts/<place stem>/handout-<n>.md` — `n` is the
   attempt, counted from the files already there — and prints two lines: the
   handout's path, and the one-line prompt you launch the agent with. Where the
   place already has a program it runs `delvec detail` on it: a program that
   passes is **exit 3** and no handout (nothing is owed), a refused one puts the
   verdict into the handout verbatim as the refusal the next attempt repairs.
   A place the plan allocates no box to is exit 1, naming every place it does;
   a directory that is not a campaign, an engine tree or a prefab directory
   that is not there is exit 2. Read its stderr line: it states the handout's
   binding — design rows, neighbours, seams, voids, owed names, refusal lines.
   **Do not read the handout**: it carries the allocation, and holding it is
   how a session starts authoring.

2. **Launch the agent** with the **Agent tool**: `subagent_type`
   `general-purpose`, and as its prompt exactly the second line the script
   printed — `Read and execute <handout path>. Report, then exit.` Nothing is
   added. **Never a fork**: a fork inherits this session's whole context, every
   other place and the design included, which is the one thing the agent must
   not hold. The handout tells the agent what it may read (the handout, its
   own program, the authoring references, the grammar corpus), what it may run
   (`delvec allocation` for the same facts again; `delvec grammar expand` at
   the frame and a render to look; `delvec detail` to check), what it writes
   (`programs/<place stem>.json`, and a generator script beside it under
   `design/generator/` when the program is emitted), and what it never does:
   read another place's program, the design record, the site plan, the layout
   graph, the quest documents or another handout; edit any other document; run
   `detail --all`, `build`, a server or `git`; retype a handed number; weaken a
   check. Wait for its report.

3. **Confirm the verdict yourself**: `delvec --prefabs "$DELVEWRIGHT_PREFABS"
   detail <campaign-dir> <place>` must exit 0. It binds the handing into the
   program, expands at the frame, voids every cell the place does not own, runs
   every gate before any file is written, writes the piece into the prefab
   directory and the row into `detail-plan.json`, and builds the whole to prove
   the map still walks. The agent ran the same verb green; your run after a
   green one moves no byte, and a byte that moves is a finding. Each owed anchor
   name is re-bound to one of the piece's own anchors (`DW0845`) — that is what
   keeps the quest layer working, because those names were bound to places
   before any detail existed and detailing must never force a quest edit — and
   a way the plan did not allocate is refused in both directions (`DW0844`).

4. **On any other exit, repair with a fresh agent**: run step 1 again — the
   script captures the refusal from the program the agent left and writes
   `handout-<n+1>.md` with it — and step 2 on that handout. The refused program
   stays where it is; the new agent repairs it. **Three consecutive refusals of
   one place naming the same diagnostic code** mean the cause is upstream of
   the program — the plan's box or seam, a `form` no program can answer, or a
   capability the grammar lacks. Launch no fourth agent: say so, with the
   three verdicts, and act on the cause — a plan edit (below), or the page's
   rule for a missing capability: say what is missing and offer the closest
   authorable alternative.

5. **Record the attempt** in `GENERATION.md` (below), then the next place.

## The program's contract

What the agent's program has to meet — the handout carries it, and you read it
here only to know what a refusal is about. The program lives at
`programs/<place stem>.json` inside the campaign, by hand or emitted by a
script (*New pieces*, step 3; a script reads the handed values from the
allocation and retypes none). It declares the handed values it uses as
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
classes). A scenery place (`"reached": false`) is judged sealed: no way in, no
floor or light owed. A ground place draws its own ground down to the terrain
the whole hands, ring included where the whole has not fixed it; an aloft one
is handed none. A declared roof's `clipped` runs and a neighbour's facade cells
are voids: whatever the program paints there is replaced after expansion, so
nothing put there survives and nothing may depend on it.

The agent iterates with `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar
expand --file … --region <the frame>` and a render until it reads as the place
— the program's `handed/…` defaults are the handed values, so no `--param` is
typed — and checks with `delvec detail`, which is the only run that sees the
voids and the seams.

## After a plan or graph edit

Including one that answers a playtest finding in a later round:
`delvec --prefabs "$DELVEWRIGHT_PREFABS" detail <campaign-dir> --all`. Every
program-detailed place is re-made; one that no longer fits is refused by name,
and that place goes back through the procedure above from step 1 — the script
captures the refusal as the next attempt's. The run judges the whole it would
leave, so the order of the places decides nothing, and it writes nothing
unless every place and that whole pass.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.

## What `GENERATION.md` records

One table, **Detail**, one row per attempt, written by you after the agent's
report and your confirming run: the place; the attempt number; the handout's
path (a regenerable file under `.out/`, which the table says once above it);
the agent's token reading and its duration as the Agent tool's completion
reports them; the diagnostic codes of the refusal that attempt repaired (none
on a first authoring); the `delvec detail` exit you confirmed. No date, no
model name, no agent id.
