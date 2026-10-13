# Reference: authoring in parallel — one planner, one worker per district

## Contents

- [When this page applies](#when-this-page-applies)
- [Who owns what](#who-owns-what)
- [Asking for the worker count](#asking-for-the-worker-count)
- [Sizing districts to workers](#sizing-districts-to-workers)
- [A wave](#a-wave)
- [Dispatching one district](#dispatching-one-district)
- [What the worker is told](#what-the-worker-is-told)
- [Reviewing a return](#reviewing-a-return)
- [Merging, and the whole's gates](#merging-and-the-wholes-gates)
- [Wave A — the story](#wave-a--the-story)
- [Wave B — the places](#wave-b--the-places)
- [A worker that fails or runs out of context](#a-worker-that-fails-or-runs-out-of-context)
- [A repair dispatch](#a-repair-dispatch)
- [What the creator sees](#what-the-creator-sees)

Read at step 2 (once the partition is drawn), again at step 5 and at step 9.
**You are the planner**: the agent running `/new-delve`. You author the whole
and dispatch one worker per district; a worker authors one district and exits
(spec-0103). The workers are Claude Code's own `Agent` tool — nothing else is
built to schedule, retry or manage them (ADR-0012). The page's thirteen steps
do not change: steps 5 and 9 each become a **wave**, a dispatch of every
district, integrated before the next step.

## When this page applies

When `layout-graph.json` declares `districts[]` and the creator's answer at
step 4 is a worker count above one. A graph with no districts, or a count of
one, is the ordinary route: you author every district in series, and the
partition stays a design record. Everything below is in the order you do it.

## Who owns what

**The whole owns what two districts would both write, and what the proofs read
across the whole**; a district owns every object located wholly inside it. The
engine enforces both halves: a fragment carrying a field the whole owns is
`DW1005`, an object of a fragment standing outside its district or reaching into
a sibling is `DW1004`, and a committed path outside a district's write set is
`DW1008`.

| step | you (the whole) | a worker (its district) |
|---|---|---|
| Init, 1, 2 | everything: the map reference, `geometry-brief.json`, `layout-graph.json` with `districts[]`, `site-plan.json` | nothing — the plan is yours and a worker never moves a box |
| 3, 4 | `npcs.json` (the cast the gate approves), `classes.json`, `quest-plan.json` with `district` on each quest a district expands; the one design gate | nothing |
| 5, wave A | every quest with no `district`: the spine, a quest whose objects stand in two districts, every NPC who moves between districts and the quests that move them, every item given in one district and needed in another; `quests.json`'s `state` and `guidance`; dialogue for your quests | `districts/<x>/quests.json`, `dialogue.json`, `npcs.json` (an NPC whose every anchor is in the district), `layout-graph.json` (its places' `stations[]` only) |
| 6–8b | `fmt`, `analyze`, `build`, the showcase cameras — on the assembled whole | `delvec fmt` on its tree before every commit; `delvec validate --district` |
| 9, wave B | `detail-plan.json`'s `palette`; every `world-edits.json` edit; `delvec detail --all` over the assembled whole | per place: the concept image under `design/places/<stem>/` and its row in `districts/<x>/design.json`, `programs/<stem>.json`, `delvec detail <place>` (which writes the row into `districts/<x>/detail-plan.json` itself) |
| 10–13 | everything; a finding routed to a district goes back to it as a repair | a repair dispatch |

**Nothing of the ladder, the world save, the renders of the whole or the play
server runs anywhere but in your tree.** Several agents on one host multiply
every collision a single run already meets.

What crosses a boundary is yours, and the packet tells each worker what crosses
its boundaries: the critical path's crossings, an NPC placed on both sides, an
item given on one side and required on the other, the two places' atmospheres,
a pulse sounding on one side and heard on the other, a line of sight. A
district quest may receive from the whole — be triggered by a spine quest,
stand where a spine beat stands, need an item the spine gives — and never from a
sibling.

**Checkpoints, the default you write**: the spine sets a checkpoint where the
critical path enters each district, so each district's own respawn chain begins
from a known point. A brief may say otherwise; the engine proves the chain the
same way whoever wrote it.

## Asking for the worker count

At step 2, once the partition is written, run

```sh
delvec district list campaigns/<id>
```

and keep its table: one row per district — places, seams, boundary seams and
carries, stations, beats, planned quests, critical-path steps, acts — with
`DW1007` beside any district outside the partitioning guide's bounds. It
advises and refuses nothing; a district the guide would draw differently is
re-read against the guide, and kept if the design says so.

The question rides with the step-4 hand-over, because the page stops there
anyway: beneath the design walkthrough, show the table and ask one line —
*how many districts shall be authored at once?* **The default is three**: a yes
at the gate with no number takes it. It is the number of whole-campaign builds a
developer workstation runs beside one another without starving them, an
authored figure the demo level for spec-0103 measures. Any number from one up
to the district count is the creator's to name.

## Sizing districts to workers

Districts are sized by meaning, never by the worker count; the count is a
throughput. With `D` districts and `W` workers, dispatch `W`, and each return
dispatches the next, in critical-path order from `entry` — the districts the
party meets first return first, so the spine is validated early. One worker
authors one district and exits; the next district gets a fresh worker. Two
neighbours in flight at once need no conversation: the contract at their
boundary is frozen in both packets.

## A wave

1. **Freeze the plan.** `geometry-brief.json`, `layout-graph.json` (bar a
   fragment's stations) and `site-plan.json` do not change while a wave is in
   flight. A change between waves re-derives every packet and re-dispatches
   every district it touches — a seam moved touches both of its districts.
2. **Commit the campaign.** That commit is the wave's **baseline**.
3. **Dispatch** up to `W` districts, as below.
4. **On each return**: review it, run the gates on its branch, merge it, run
   the whole's gates.
5. When every district has returned and merged, the wave is integrated.

## Dispatching one district

Derive the packet — never commit it; it is derived, and `.out/` is
gitignored — from the baseline:

```sh
delvec --prefabs "$DELVEWRIGHT_PREFABS" district packet campaigns/<id> district/<x> \
    -o .out/packets/<x> --wave story
```

(`--wave build` for wave B; `--wave repair --finding <file>` for a repair.) It
writes `packet.json`, `PACKET.md` rendered from it, and `images/`. It refuses a
`--prefabs` outside the campaign's repository: on this route a worker details
into the prefab directory of its own worktree, so its pieces travel on its
branch and the merge brings them. Init I6 says which directory that is; when it
is outside the repository, take the library into the repository first.

Then launch the worker — a **fresh** agent, never a fork, so it holds nothing
but its packet — in a worktree of its own:

```text
Agent({
  description: "Author district/<x>",
  subagent_type: "general-purpose",
  isolation: "worktree",
  prompt: <the whole of .out/packets/<x>/PACKET.md>
          + the standing rules below
          + "Report, then exit."
})
```

The packet's `tools` line carries the `--prefabs` path and the `--district`
flag; the worktree is cut from the current commit, which is the baseline.

## What the worker is told

These lines go after `PACKET.md`, word for word:

- First, in your worktree, install the commit hook that holds your writes to
  your district: `"$DELVEWRIGHT_ENGINE/tools/creator/district-hook.sh"
  "$(git rev-parse --show-toplevel)" <campaign> district/<x> <prefabs>
  "$(command -v delvec)"`, with the campaign and prefab paths from the packet's
  `tools` line.
- Commit as you go. Run `delvec fmt` over the campaign before every commit.
- Judge your tree with `delvec --prefabs <prefabs> validate <campaign> --district
  district/<x>`. Its `DW1006` line counts your siblings' quests, which are not
  in your tree; the refusals it reports beneath it are theirs, not yours.
  Everything else it refuses is yours.
- Never start a server, never write under `/tmp`, never edit the plan. A place
  that does not fit its box, or a thing that must cross a boundary and is not in
  your contract, is a **request** in your report — name the contract it concerns.
- Report: per place, every refusal you met and how you answered it; the last
  lines of `validate --district`; every request to the whole. Then exit.

## Reviewing a return

Read the report against the packet: the gate lines below, the renders the worker
made of its places beside each place's concept image, and every request. A
request is answered in your documents before the next wave, and the
neighbours' packets are derived again. A render that does not read as its place
is a repair dispatch, not a merge refusal — the piece is the district's to fix,
and it is already on the branch.

The gates on the worker's branch, the first three re-run by you on its head:

1. Every path it changed is in its write set:
   `"$DELVEWRIGHT_ENGINE/tools/creator/district-integrate.sh" . campaigns/<id>
   district/<x> <baseline> <head> "$DELVEWRIGHT_PREFABS" "$(command -v delvec)"`
   exits 0.
2. `delvec fmt --check campaigns/<id>` exits 0.
3. `delvec --prefabs "$DELVEWRIGHT_PREFABS" validate campaigns/<id> --district
   district/<x>` carries no refusal; its `DW1006` count equals the planned quests
   of the other districts not yet in this tree.
4. Wave B only: every place of the district has a row in
   `districts/<x>/detail-plan.json`, and `delvec detail <place>` exited 0 on each
   — the verdict lines are in the report.
5. The report names per place every refusal met, and every request.

## Merging, and the whole's gates

Merge one worker at a time, in the order they returned:

```sh
git merge --no-ff <worker-branch>
```

A branch that no longer merges cleanly is your finding, not the workers': two
workers wrote one path, which the write sets forbid. Never rebase, amend or
force-push a worker's branch; a worker adds commits.

After each merge, on the assembled whole: `delvec fmt --check campaigns/<id>`,
and `delvec --prefabs "$DELVEWRIGHT_PREFABS" validate campaigns/<id>` with no
flag — the whole is judged, and every refusal is yours or routed.

**The cast ledger is completed at integration.** Every quest's `cast` names every
NPC live during it (spec-0020). A district's own NPC is unknown to the whole's
quests and to its siblings' until its branch is merged, so after each merge add
its row — where it stands, what it is doing — to every quest of the whole it is
live in, and to every sibling quest already merged. A row that only records where
an NPC stands is not a placement, so it never trips `DW1004`. The worker's tree
reports the whole's missing rows under its `DW1006` line.

## Wave A — the story

At step 5: write your own quests first — the spine, `state`, `guidance`, the
dialogue they need — and commit; that commit is wave A's baseline. Dispatch
every district with `--wave story`. When the wave is integrated, steps 6–8b run
on the assembled whole, exactly as on the ordinary route.

## Wave B — the places

At step 9, on a campaign that validates and builds: dispatch every district
with `--wave build`. A worker makes each of its places' concept image under
`design/places/<stem>/`, anchored on the map's view 1 under the style contract
its packet carries, with its row (and the sky it was drawn under) in
`districts/<x>/design.json`; writes `programs/<stem>.json`; and runs
`delvec detail <place>` for each place, which writes the place's row into the
district's fragment. When the wave is integrated, run
`delvec --prefabs "$DELVEWRIGHT_PREFABS" detail campaigns/<id> --all` over the
whole — every program-detailed place re-made over the assembled campaign; it
writes nothing unless the whole passes — then steps 10–13. A per-place image is
an approved picture like any other: a showcase camera answers it before the
next build (step 8b's rule).

## A worker that fails or runs out of context

The branch is the state, not the worker's memory. Read the branch (`git log`,
and `delvec district list campaigns/<id>` on it — places detailed of total,
quests expanded of planned), derive the packet again, and dispatch a fresh
worker on the same district with the packet and one line: *continue from the
branch; what is committed stands.* Never resume by amending, and never dispatch
a second worker on a district whose first is still running — one district, one
worker at a time.

## A repair dispatch

A finding after step 10 or 12 is routed to its district by its own text: every
diagnostic the engine prints that names an object of one district carries
`[district/<x>]`, and a harness or PackTest finding names a step, a leg or an
object you resolve through `validation/districts.json` in the build tree (every
object and every critical-path leg with its district). A finding on one district
is a repair dispatch on that district alone — `--wave repair --finding <file>`,
the finding verbatim, on a worktree cut from the current campaign branch, the
same gates, the same merge. A finding that names two districts, or none, is
yours. Never repair a district's object in the whole's documents to get green.

## What the creator sees

At step 4, one design gate for the whole campaign — every scene, near and far —
with the partition table and the one-line worker-count question beneath it. The
gate is never split by district and no per-district approval is asked: a
district is your dispatch unit, not the creator's review unit. Between steps 4
and 13, a short summary per district as it returns — what was built, its gate
lines, any request to the whole — and, if a wave reds on the whole, the finding
and the district it was routed to. Step 13 is unchanged.
