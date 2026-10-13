# spec-0103: A delve is authored in parallel by district — the layout graph partitions its places into districts, one worker authors one district from a packet a tool derives, the whole owns what crosses a boundary, and a write outside the district is refused where it is entered

- **Status**: Proposed
- **Ground**: written against engine `c1c4a6384b4a3d670dc3ed4ffccb39c30d261060` (`main`), read only — `LayoutGraphContent`, `Node`, `Station` (`crates/dsl/src/layout.rs`); `PlannedQuest.area` (`crates/dsl/src/quest_plan.rs`); `DetailPlanContent` (`crates/dsl/src/detailplan.rs`); `Campaign` and `RawCampaign` (`crates/dsl/src/envelope.rs`); `load_campaign_dir` (`crates/delvec/src/compiler/load.rs`), which reads one file per stage by name; the handout `delvec allocation` emits (spec-0098 §4; `docs/reference/delvec/compiler/detail.md`); the `/new-delve` page (`.claude/skills/delvewright/skills/new-delve/SKILL.md`) and its `references/placement.md`, `detail.md`, `design-gate.md`, `map-reference.md`; the delegation record `docs/reference/skill-workflow.md` §2; ADR-0002, ADR-0012, ADR-0032. The campaign record: the content branch `campaign/stranding` at `0d1f897363fff1b76ca483519cf2dde1c66f09cd` — `campaigns/the-stranding/GENERATION.md` and `docs/stranding-creation-problems.md` (31 rows) — and the content tag `release/the-treehouse-camp/v1.0.1` (`campaigns/the-treehouse-camp/`). `grep -rIl district crates/ docs/ .claude/ tools/ gallery/` at the ground revision matches 0 files: none of the surface below exists.
- **What it is for**: a campaign of twenty or more scenes authored through `/new-delve` by more than one agent at once. The Stranding (55 places, 16 quests, 64 seams) was authored by one agent holding every place, and the record of that run (§1) is the evidence. This spec gives the engine a **district** — a named set of the layout graph's places — and gives the skill a planner that authors the whole and dispatches one worker per district, each knowing three things: the campaign's concept, its district's place in the whole, and the contract at every boundary with a neighbour. A worker neither reads nor writes another district. How many districts are in flight at once is the creator's choice.
- **Research**: §2 is the research record. Each rule below is marked **cited** (a source named in §2, a reading of the tree, a row of the campaign record, or a constitution rule) or **authored** (this spec chooses). The partitioning craft itself is the skill page `references/partitioning.md`, which carries its own cited/authored marks and two worked examples.
- **Numbers**: spec-0103. **No ADR** (§12). **Seven DW codes**, allocated by the planner: `DW1003` (the partition is ill-formed, §3.3), `DW1004` (a district's object stands outside its district, §7.3), `DW1005` (a district fragment carries a field the whole owns, §7.2), `DW1006` (informational: the quests of other districts are not expanded in this tree, §7.5), `DW1007` (advisory: a district outside the guide's size advice, §4.2), `DW1008` (a write outside the district's write set, §7.6), `DW1009` (`--district` names no declared district, §7.5). `dsl_version` 0.40.0 numbers the surface and promises nothing.
- **Non-goals**: a second agent runtime (ADR-0012 forbids one; the workers are Claude Code's own `Agent` tool, §9.3); a change to the staged DSL's stages (ADR-0002 stands: a district fragment is the same stage schema in a second file, §7.1); a change to the map route (ADR-0032 stands: a district is a set of places of the one site plan, §3); a tool that refuses a district for its size (CLAUDE.md, *a check confirms intent; it never restricts* — §4.2 advises); dividing the design gate (§10: one gate for the whole); authoring the content of The Stranding or any campaign by this route (that is a campaign round, not this spec).

## 1. The defect, and the record it rests on

**The defect.** `/new-delve` is written for one agent: thirteen steps, one actor, every document and every place that actor's. On a map of 55 places that actor wrote a geometry model in Python (`generators/mapmodel.py`, `places_town.py`, `places_flat.py`, `places_body.py` — `GENERATION.md`, *The map*) to generate every place's program rather than designing each place against its handout, met and answered 127 blockout refusals across the whole (`stranding-creation-problems.md` row 28), spent hours on per-place detail before `--all` was found (row 15), and recorded the run as broken down. A per-place piece designed from its own handout is what spec-0098 §4 built the handout for; one context holding 55 of them does not design, it generates.

**The record, classified.** Every row of `docs/stranding-creation-problems.md` (31 rows, `campaign/stranding` @ `0d1f8973`), read against the route this spec builds: would one worker per district have **prevented** it, made it **worse**, or left it **unchanged**? The reading is by what the row's symptom is caused by, not by whether a more attentive author would have avoided it — attention is the claim this route makes, and a claim is not evidence.

| Row | Symptom (short) | Class in the record | Under per-district workers | Why |
|---|---|---|---|---|
| 1 | a carry-only place cannot be detailed | capability | unchanged | engine |
| 2 | a plan edit makes every other detail row stale; `--all` refuses up front | engine | **worse** | with several trees detailing at once, a plan edit during a wave strands every worker's rows; §8.1 freezes the plan at dispatch and makes a plan edit a planner act that re-dispatches |
| 3 | `DW0891` prescribes an anchor `DW0845` refuses | engine | unchanged | engine |
| 4 | `DW0948` refuses a loop released from inside | capability | unchanged | engine |
| 5 | a world-edits fragment writes over bound frames silently | engine | unchanged | engine; the body form is the whole's object (§5) |
| 6 | an `open` fill floods nothing below sea level | engine | unchanged | engine |
| 7 | `pulse_<s>_shut` PackTest is not isolated | engine | unchanged | engine |
| 8 | `open-gate` misses re-derived iron bars | engine | unchanged | engine |
| 9 | the bot's first leg starts before placement | engine | unchanged | engine |
| 10 | `DW0955` model cells under two sea places | engine | unchanged | engine |
| 11 | the combat plan reads a late checkpoint as governing every encounter | engine | unchanged | engine; the checkpoint chain is the whole's proof (§5) |
| 12 | two atmospheres meeting at the body's mouth, `DW0929` | capability | **worse** | an atmosphere boundary falls exactly where a district boundary falls when each worker paints its own sky; §5 keeps `boxes[].atmosphere` with the planner |
| 13 | no out-of-sight proof for two skiffs | capability | unchanged | engine |
| 14 | a killing volume with no `on_death` skips the death-loop stage | doc | unchanged | a doc gap |
| 15 | per-place `detail` re-runs the whole build; hours before `--all` was found | friction / doc | unchanged | the cost is per run of a whole-campaign verb and a worker pays it as one author does; the route divides how many runs one agent makes, not what one costs |
| 16 | stations added in two places at once deadlock per-place detail | friction | unchanged | a worker with two places meets the same deadlock inside its own tree |
| 17 | one global `env.sh` names one engine tree | friction | unchanged | every worker runs the one pinned engine |
| 18 | the light probe needs a side entrance on top-entered pieces | doc | unchanged | a doc gap |
| 19 | through-floor stair patterns undocumented | doc | unchanged | a doc gap |
| 20 | branch runs took the default wall-clock budget | doc / author | unchanged | the ladder is the whole's |
| 21 | the critical path is 3030 blocks | measurement | unchanged | the critical path is the planner's design |
| 22 | scratch output under `/tmp` | author | unchanged | a rule the packet restates |
| 23 | waiting loops on `ps aux \| grep` never terminated | author | unchanged | process hygiene |
| 24 | a world save ran beside a bot run; two servers on one host | author | **worse** | several agents on one host multiply the collision; §5 and §8.2 give no worker a server — the ladder, the save and the play server are the planner's alone |
| 25 | two bodies on one mark | author | unchanged | a content mistake inside one place |
| 26 | barred seams built of iron bars | author | unchanged | row 8 |
| 27 | dressing that re-derives or falls (`DW0955`) | author | unchanged | a piece's own defect, met by whoever writes the piece |
| 28 | 127 blockout refusals, plus the route, stair, face, carry, camera and localisation codes, every one met and answered by one author across 55 places | author | **prevented** | every refusal in the row names a place, a seam or a quest object; under this route each is met by the worker whose district it names, inside a context that holds that district and not the other eight (§8.3) |
| 29 | showcase cameras are estimates, not composed views | author | unchanged | step 8b is the planner's |
| 30 | two waves seated at one anchor from one trigger | author (staging) | unchanged | a staging mistake inside one place |
| 31 | a sculk prediction repeated across consecutive legs | engine | unchanged | engine |

**Counts: prevented 1, worse 3, unchanged 27, of 31.** The three *worse* rows each name the rule in this spec that removes the worsening (plan frozen at dispatch; atmospheres the planner's; no server in a worker's hands), and each is a gallery probe or an acceptance criterion below. The one *prevented* row is the load, and the load is what this spec is for: the record carries no row for the breakdown itself, because the record is a ledger of defects met, and a context that generates instead of designing meets no diagnostic. Twenty-seven rows are engine defects, capability gaps and documentation gaps this route does not touch, and the spec says so rather than claiming them.

## 2. Research record

### 2.1 What a district is, in the literature **[cited]**

Kevin Lynch, *The Image of the City* (MIT Press, 1960), names five elements by which a person holds a city in mind — paths, edges, districts, nodes and landmarks — and defines a district as "the medium-to-large sections of the city, conceived of as having two-dimensional extent, which the observer mentally enters 'inside of,' and which are recognizable as having some common, identifying character." What makes the character is "thematic continuities which may consist of an endless variety of components: texture, space, form, detail, symbol, building type, use, activity, inhabitants, degree of maintenance, topography." An edge is "the boundaries between two phases, linear breaks in continuity"; a path is a channel of movement; a node is a strategic spot a person enters, a junction or a concentration; a landmark is an external point of reference with a clear form and figure-background contrast. Two consequences this spec takes from Lynch: a district is recognised by continuity and bounded by a break in it, so the partition is drawn where the material, the use or the ground changes; and a path or an edge is not a district, so a connector belongs to a district rather than being one. David Shaver (Naughty Dog, *Invisible Intuition*, GDC 2018) carries the landmark half into level design — a distant object seen from many vantage points, with a unique silhouette — and `docs/reference/interior-lighting.md` §3.4 already holds that citation.

### 2.2 How large levels are divided among their makers **[cited, with the gap named]**

`docs/research/how-a-large-level-is-actually-built.md` §1.4 and §1.6 hold the project's research on who owns a large world's layout. The findings this spec rests on: Skyrim's interiors were produced by eight level designers against shared kits whose standards were fixed first (Burgess and Purkeypile, GDC 2013, STRONG); the Dark Souls interview record attests per-area rough maps and per-area owners with coherence held by one director's continuous review (MEDIUM); WesterosCraft, the best-documented Minecraft build team, fixes per-region style guides and has project leads lay out plots that individual builders fill, with terraforming run first by a separate role (MEDIUM); Unreal's World Partition keeps the whole in one persistent level and hands authorial chunks to it, which the research reads as the engine's conclusion that per-file zone ownership was the wrong model (STRONG). The research found **no published example of a hard per-zone spatial refusal**; practice is soft renegotiation while the plan is cheap to move, with the whole's owner arbitrating. The gap this spec names: no source read describes a *machine* partition of a level into units for parallel authoring with an enforced ownership boundary; that shape is authored here, and §2.2's findings bound it — the whole is designed first and owns the plan (§5), per-district style is fixed before building (the packet, §6), and a part that does not fit asks the whole to move the plan rather than moving it itself (§8.1).

### 2.3 What the engine already has **[cited: the tree]**

A place is a layout-graph node with a box in the site plan; a connection is an edge with a seam; the engine derives where everything stands and hands each place its allocation — frame, ground, every seam with its `form`, the other side, the owed anchors, the palette, the voids (spec-0098 §4; `delvec allocation`). Stations are the named spots inside a place (spec-0052). The campaign has one assembly area, `area/site`; `world.areas[]` is empty on a site-plan campaign (`DW0839`); quests and NPCs name `area/site`. The quest plan assigns each quest an `area` (`PlannedQuest.area`); the layout graph binds beats to nodes and carries the authored `critical_path[]`. Every proof is over the whole's bytes, indifferent to who wrote them (ADR-0032 §3). So the engine has the places, the connections, the per-place handout and the whole's proofs; what it lacks is a name for a *set* of places and an owner for it.

### 2.4 The runtime's own facilities **[cited: ADR-0012; Claude Code]**

ADR-0012 makes Claude Code the agent runtime and forbids building another. Claude Code's `Agent` tool launches a sub-agent with a prompt; with `isolation: "worktree"` the sub-agent runs in its own git worktree of the current repository, cut from the current HEAD, kept when it holds changes; a `fork` inherits the caller's context and a fresh agent does not. The planner of this spec is the session running `/new-delve`; a worker is a fresh `Agent` with a worktree, so it holds only what its packet gives it, which is the property §1 asks for.

## 3. The unit: a district

### 3.1 Decision **[authored, on §2.1 and §2.3]**

**A district is a named, non-empty set of the layout graph's nodes, and the districts partition the nodes.** It is declared in `layout-graph.json`, beside the graph it partitions, because the partition is a design judgement about the space made before any coordinate exists (ADR-0002's ordering; the graph is where `critical_path[]` and `beats[]`, the other authored claims about the space, already live). It is not a site-plan object: a box is a place's geometry, and a district has none of its own. It is not a place: Lynch's district is entered "inside of" and has extent; a place is one scene.

**The engine's noun is `district`, not `area`.** `area` is taken three ways in the tree (`world.areas[]`, `area/site`, `PlannedQuest.area`, `npcs[].area`, `world_edits.batches[].area`), each meaning an assembly area of prefab pools or the one site; a fourth meaning for one word would be two authorities for one name. The direction this spec is written to calls the unit an area; the engine calls it a district, and the skill page says once that the two words name one thing.

### 3.2 The surface

`layout-graph.json` `content` gains one optional field:

| field | shape | meaning |
|---|---|---|
| `districts[]` | `[{id: "district/<kebab>", nodes: [NodeId, …], note?: string}]` | the partition. `id` is unique (`DW0111`'s rule for ids). `nodes` is non-empty and every member is a declared node. `note` is recorded judgement for the reviewer and the packet, keyed on by no check, exactly as `intent` and `stations[].note` are. |

`quest-plan.json` `quests[]` gains one optional field:

| field | shape | meaning |
|---|---|---|
| `district` | `"district/<kebab>"` | the district whose worker expands this quest (§5). Absent: the quest is the whole's — a spine quest the planner expands. Must name a declared district (`DW0112`, the generic unresolved-reference code, on this path). |

**Zero districts is the current route.** A layout graph with no `districts[]` is one whole authored by one agent; nothing below applies, nothing refuses, and the binding line says `0 district(s): the whole`. A campaign with districts may still be authored by one agent in series: the partition is a design record first and a dispatch unit second.

### 3.3 `DW1003` — the partition is ill-formed (validation tier) **[authored]**

Refused, each shape its own message naming the node or district: a node in no district (the message lists every unassigned node — one diagnostic, not one per node, the way `DW0874` lists documents); a node in two districts; a district naming a node the graph does not declare; an empty district; a district whose nodes are not connected to one another through edges whose both ends lie in the district (a district is entered "inside of", §2.1 — two unconnected sets declared as one are two districts); a `districts/<x>/` directory (§7.1) whose `x` is no declared district. The remedy is a graph edit, named in the message. A scenery node (`reached: false`) is a member like any other: a crown over a treehouse belongs to the tree's district and its worker draws it.

### 3.4 Who draws the partition, and when **[authored, on §2.2]**

The planner, at step 2, after `layout-graph.json`'s nodes and edges and before `site-plan.json`, applying `references/partitioning.md` (§4). It is drawn once for the whole, as the plan is: the research (§2.2) is unanimous that the whole is laid out first and the parts receive their extent from it. It is shown to the creator with the design walkthrough at step 4 (§10) and is frozen for a wave at dispatch (§8.1); changing it is a planner act between waves, re-run through `delvec district list` and the design record.

### 3.5 Connectors **[authored, on §2.1]**

A connector that is itself a place — a bridge, a long stair, a corridor (ADR-0032 §4) — belongs to exactly one district, as every node does. The rule the guide gives: it belongs to the district whose approach it is — the district at its far end from the campaign's `entry` along the critical path — because a path takes its identity from its destination (Lynch) and the player reads the bridge as the way *into* the Watch Tree, not as a piece of the camp. A connector that is a seam and no place (a doorway, a contact, a ladder on a wall) belongs to nobody: it is the boundary, and its contract (§6.3) is what both sides receive.

## 4. The partitioning guide

### 4.1 Where it lives

`.claude/skills/delvewright/skills/new-delve/references/partitioning.md`, bundled with the plugin and named from `SKILL.md` at step 2. It is the planner's craft page: the test for one district (Lynch's continuity and edge, the fiction's own names, one building or one region), the bounds on size and content with their reasons, the connector rule, how landmarks and things that cross are listed, and the two worked examples — The Stranding at 55 places and The Treehouse Camp at 11 — each a table from place to district with the boundary seams named. Every rule on it is marked cited or authored. `docs/reference/skill-workflow.md` §2 points at it.

### 4.2 `DW1007` — a district outside the guide's advice (advisory) **[authored]**

`delvec district list <campaign>` prints, per district: places (reached and scenery), seams inside it, boundary seams and boundary carries, stations, beats, planned quests, critical-path steps, and the acts those quests belong to. Where a district stands below the guide's lower bound or above its upper bound it prints `DW1007` beside the row with the bound it crossed. **It advises and never refuses**: the partition is a creative judgement, the check confirms what was declared, and a declared district of one place passes with its count stated. The bounds are the guide's and are revised there when the demo level measures them (§11.2).

## 5. What the planner owns and what a worker owns

The rule behind every row: **the whole owns what two districts would both write, and what the proofs read across the whole** (ADR-0032 §3's ownership rule, carried from cells to documents). A district owns every object located wholly inside it. A thing that crosses a boundary is the whole's.

| Stage / step | Planner (the whole) | Worker (its district) |
|---|---|---|
| Init, 1 workspace | everything | — |
| 2 placement | the map reference, `geometry-brief`, `layout-graph` (nodes, edges, `districts[]`, `entry`, `goal`, `critical_path[]`, `beats[]`), `site-plan` (boxes, seams and their `form`, `fill`, volumes, datums, identities, sightlines, views, `boxes[].atmosphere`) | nothing; the worker's places are the plan's and it may not move them (§8.1) |
| 3 story | `npcs` — the cast the design gate approves; `classes`; `quest-plan`, with `district` on each quest a district expands | — |
| 4 design gate | the whole walkthrough, one gate; `design.json` and `design/` for every scene | — |
| 5 content, wave A | its own quests (every quest with no `district`): the spine, every quest whose objects stand in two districts, every NPC who moves between districts and the quests that move them, every item given in one district and needed in another; `quests.json`'s `state` and `guidance`; `dialogue` for its own quests; then the dispatch (§9) | `districts/<x>/quests.json`: the quests the plan assigns it, with their objectives carrying the ids `beats[]` already names, and every trigger, wave, actor, assembly, lethal volume and pulse located in its places; `districts/<x>/dialogue.json` for those quests; `districts/<x>/npcs.json` for an NPC whose every anchor is in the district (a flavour sleeper, a keeper who never leaves); `districts/<x>/layout-graph.json` carrying only its own nodes' `stations[]` |
| 6 fmt, 7 analyze, 8 build, 8b cameras | the whole, on the assembled campaign; the showcase camera record | a worker runs `delvec fmt` over its tree before every commit (the formatter formats fragments as documents); it runs `validate --district <x>` (§7.5); it never runs the ladder, a world save or a play server (§1 row 24) |
| 9 detail, wave B | `detail-plan.json`'s `palette`; `detail --all` over the assembled whole after the wave; `world-edits.json` — every edit, since an edit is placed in world coordinates and the commons belongs to no place | the per-place concept image for each of its places (`design/places/<stem>/`, anchored on the map's view 1 under the style contract the packet carries), its rows in `districts/<x>/design.json`; `programs/<stem>.json` per place; `delvec detail <place>` per place, whose row lands in `districts/<x>/detail-plan.json` and whose piece lands in the repository's prefab directory (§7.4) |
| 10 ladder, 11 chronicle, 12 visual review, 13 hand-over | everything; a finding is routed to its district (§8.3) and repaired by a repair dispatch (§9.6) | a repair dispatch, on the finding's district, with the packet and the finding |
| other languages | the sidecar, after both waves (one sidecar covers the whole) | — |

**Things that cross a boundary, named:**

| what crosses | owner | how it reaches the worker |
|---|---|---|
| the critical path | planner (`critical_path[]`) | the packet marks the steps that enter and leave the district and the boundary seam each crosses |
| a story beat spanning two districts | planner: the quest is the whole's (no `district`) | each district's packet lists the whole's objectives that stand in it, so the worker builds the place the beat needs and writes no quest for it |
| an NPC who moves between districts | planner: the NPC row and every quest that moves them | listed under each boundary seam the NPC crosses, with the quest and the direction |
| an item carried across | planner: the `give-item` and the objective that needs it are in the whole's quests | listed under the boundary seam, with the quest that gives and the quest that needs |
| the sky or the fluid of a place | planner: `boxes[].atmosphere`, the plan's fluid bodies, every `set-atmosphere` naming a `region` | the packet names each place's atmosphere; a district quest may repaint a place of its own with `set-atmosphere` naming that `place`, which is located (§7.3) |
| a checkpoint | the district, for a `set-checkpoint` at its own anchor; the planner, for the one its spine sets where the critical path enters each district (the skill's authored default; a brief may say otherwise) | the packet states the checkpoint the whole sets at the entry, so the district's own respawn chain begins from a known point |
| a pulse heard across a boundary, a sightline, a vista | planner (`pulses[]` whose reach leaves the district; `sightlines[]`, `views[]`) | listed under the seam as `passes.sound` / `passes.sight` |
| a world edit | planner | — |
| the pieces' palette | planner (`detail-plan.json` `palette`) | in the packet verbatim, as the allocation already carries it |

**Checkpoints, the authored default.** The spine sets a checkpoint where the critical path enters each district. It is a skill default a creator overrides in the brief, never a mechanism: the engine proves the chain as it does today (`DW0315`), whoever set it.

## 6. The handover packet

### 6.1 Derived, never typed **[cited: CLAUDE.md; spec-0098 §4]**

`delvec district packet <campaign> <district> -o <dir>` writes the packet from the documents on every invocation. Nothing in it is hand-written; a planner that wants to say more to a worker edits a document (`districts[].note`, `nodes[].note`, `seams[].form`) and re-derives. The packet is an input to nothing: `validate`, `detail` and the battery recompute every obligation from the plan.

### 6.2 Contents

`<dir>/packet.json`, and `<dir>/PACKET.md` rendered from it (the worker's brief, in the order a worker reads it), and `<dir>/images/` holding copies of every image the packet names:

| section | derived from | carries |
|---|---|---|
| `campaign` | `world.json`, `design.json`, `design/README.md`, the map reference's sidecars | title, theme, premise, `time`, `weather`, `target_minutes`, `languages`, the `dsl` number; the approved map reference views (paths, `shows`, sky) and view 1's anchor (`id` and file) with the style-contract string from its sidecar; the design record's sentence every round is held to |
| `district` | `layout-graph.json` | `id`, `note`, every node with `intent`, `note`, `stations[]`, `reached`; every edge whose both ends are inside; `critical_path[]` restricted to the district with the entering and leaving steps marked; every `beats[]` row in the district |
| `story` | `quest-plan.json`, `quests.json` | the acts; every planned quest with `district == this` (id, goal, `depends_on`, `npcs`, `mandatory`, act), each with the objective ids `beats[]` already names for it — **the objectives the worker owes**; every quest of the whole with an objective or cast position in the district (id, goal, the objects in the district), so the worker builds what the whole's beats need and writes none of them; the cast rows of every NPC the quest plan names for its quests, verbatim from `npcs.json` |
| `concept` | `design.json`, `design/` | every approved image whose `shows` names a place of the district, with its row; the wave-B packet adds the per-place images already approved |
| `places[]` | `delvec allocation`, one per node | the allocation verbatim — frame, ground, voids, seams with `form`, owed anchors, palette — as `allocation` prints it (the packet asserts equality with a fresh `allocation` run, §13 criterion 7) |
| `neighbours[]` | `layout-graph.json`, `site-plan.json`, `quests.json`, `npcs.json`, `pulses[]`, `sightlines[]` | one entry per neighbouring district, with one **connection contract** per boundary edge (§6.3) |
| `wave` | the dispatch | `story` or `build` or `repair`, and for `repair` the finding verbatim |
| `write_set[]` | §7.6 | every path the worker may create or change |
| `tools` | the dispatch | the `--prefabs` path (the worktree's own prefab directory, §7.4), the `env.sh` line, the `--district` flag |
| `baseline` | git | the campaign branch's commit the worktree was cut from |

### 6.3 The connection contract **[authored, on the seam the engine already has]**

One per boundary edge, derived; it adds no DSL field. For a seam:

| field | derived from | meaning |
|---|---|---|
| `edge`, `class`, `form` | the graph and the plan | the connection, and the sentence both sides were handed |
| `opening` | the seam | the standard's name or `{width, height}`; for a contact, its span |
| `height` | the two boxes | the sill `y` (the higher floor) and the rise (their difference); `stair_in` where a stair hosts it |
| `landing` | the allocation | the cells on each side a body stands on first — for an opening, the cells one step inside each box across the opening's width; for a contact, the span on each side; for a climb, the foot and the head |
| `facing` | the seam's `face` | the face of `a` the crossing is on, and the direction a body travels into `b` — what an NPC placed to greet the party faces |
| `gating` | the edge | `barred` with its `gating`, `opens_from`, `one_way`, `shortcut` |
| `passes` | the documents at dispatch | `party`: whether and in which direction the critical path crosses here; `npcs[]`: each NPC whose cast positions stand on both sides, with the quests; `items[]`: each item given on one side and required on the other, with the quests; `atmosphere`: the two places' atmospheres (equal, or the planner's note on the boundary); `sound[]`: pulses whose reach crosses; `sight[]`: `vision` edges and sightlines across |

For a `carry` edge (no seam): the carried volume's extents on each side, the trigger that moves it, and `passes` as above. For a `vision` edge: the two places and the sightline's two ends.

`passes` is derived from what is written at dispatch. At wave A only the whole's quests exist, so wave A's contract lists what the spine passes; wave B's lists everything. A worker that needs a thing to pass that the contract does not list writes it in its return as a request; the planner writes it into the whole's documents and re-derives both neighbours' packets. A declaration of nothing passing is legitimate and is printed as such.

## 7. Ownership enforcement

### 7.1 Decision: district fragments of the stage documents **[authored, on CLAUDE.md]**

Of the three mechanisms the question allows — per-district documents the engine assembles, a DSL surface change, a tool that refuses a write outside the district — this spec takes the first for the documents and the third for the tree, and the second only where §3.2 states it. Several writers on one `quests.json` collide textually (two insertions in one array) and semantically (two authors of one object), and a merge of two clean hunks is not evidence of compatibility (CLAUDE.md). An object's owner is therefore a property of **where it is written**: a campaign directory may hold `districts/<x>/` for each declared district, and in it fragments of the stage documents:

| fragment | stage | may carry | the whole owns (refused in a fragment, `DW1005`) |
|---|---|---|---|
| `districts/<x>/npcs.json` | `npcs` | `npcs[]` | — |
| `districts/<x>/quests.json` | `quests` | `quests[]`, `triggers[]`, `waves[]`, `actors[]`, `assemblies[]`, `lethal_volumes[]`, `pulses[]` | `state`, `guidance` |
| `districts/<x>/dialogue.json` | `dialogue` | `dialogues[]` | — |
| `districts/<x>/layout-graph.json` | `layout-graph` | `nodes[]` carrying `id` and `stations[]` only | every other field of the graph and of a node |
| `districts/<x>/detail-plan.json` | `detail-plan` | `details[]` | `palette` |
| `districts/<x>/design.json` | `design` | the rows of images under `design/places/<stem>/` for the district's places | the map and scene rows of step 4 |

A fragment is the same schema as its stage (ADR-0002 stands: no new stage, no new document kind), with the same four-key envelope; its `stage` names the stage it fragments. The loader (`load_campaign_dir`) reads `districts/*/` in **sorted district-id order** and unions each fragment's lists into the stage document's lists after the whole's own, in file order; it hashes every fragment into the manifest's inputs as it hashes every document. Every validation that runs over a stage's list runs over the union, so an id declared twice across a fragment and the whole, or across two fragments, is the duplicate it already is (`DW0111`). A stage not in the table found under `districts/<x>/` is `DW1005` naming the stage.

**Determinism** (ADR-0006): the union order is a function of the files' names and contents, so the same documents yield the same bytes whether an object stands in the whole or in a fragment, and however many workers wrote them. Criterion 4 proves it on the gallery.

### 7.2 `DW1005` — a district fragment carries a field the whole owns (validation tier) **[authored]**

Refused per fragment, naming the field and the document the whole writes it in. The repair is to move the field.

### 7.3 `DW1004` — a district's object stands outside its district (validation tier) **[authored, on ADR-0032 §3]**

**The located rule**: every object in a district's fragment is located in that district, and every reference it makes resolves to the whole or to its own district — never to a sibling. Per object class, "located" is:

| object | located when |
|---|---|
| an NPC (`npcs[]`) | its `anchor` resolves into a place of the district |
| a quest (`quests[]`) | every `cast` position, every objective anchor and every effect's place or anchor resolves into the district; the plan's `district` for the quest equals the fragment's |
| a trigger, wave, actor, assembly, lethal volume, pulse | its anchor, mark or region resolves into the district (`place`: a place of the district; `region`: every cell inside the district's claims) |
| a dialogue tree | referenced only from quests of the same fragment |
| a station | declared on a node of the district |
| a detail row | its `place` is a node of the district |
| a design row | its image is under `design/places/<stem>/` where `<stem>` is a node of the district |

A reference from a fragment to a sibling's object (a quest `depends_on` a sibling's quest, a dialogue `next` into a sibling's tree, a `set-flag` a sibling's objective requires) is refused under the same code: the thing that joins two districts belongs to the whole, and the message names the whole's document to move it to. **The whole may name anything**: a spine quest names a district's station, waits on a district's objective, repaints a district's place. The direction of the rule is the direction of the ownership: parts receive from the whole (§2.2).

Where the engine cannot locate an object (an anchor it does not resolve), the resolution refusal that exists already (`DW0112`) stands and `DW1004` is not raised on top of it.

### 7.4 `delvec detail` writes the row where the place belongs **[authored]**

`delvec detail <place>` writes the place's `details[]` row into `districts/<x>/detail-plan.json` when the place is in district `x`, and into `detail-plan.json` when the campaign declares no districts. The place determines the file: no flag chooses it. The piece is written into the prefab directory `--prefabs` names, and **on the parallel route that directory is inside the repository the campaign lives in**, so a worker's branch carries its pieces and the merge brings them: the packet's `tools.prefabs` is the worktree's own prefab directory, and `delvec district packet` refuses a dispatch whose `--prefabs` resolves outside the campaign's repository, naming the remedy (take the library into the repository first — the skill's Init I6 states the cases).

### 7.5 `delvec validate --district <x>` **[authored]**

A worker's tree holds the whole campaign, because every proof reads the whole; it also holds every sibling's planned quests unexpanded, which `DW0150` would name as the planned-but-unexpanded state it is. `--district <x>` tells `validate` whose tree this is: planned quests whose `district` is not `x` and not absent are counted into one informational line, `DW1006`: `<n> planned quest(s) of <m> other district(s) are not expanded in this tree`, and the refusals that would stand only on their absence (`DW0150`, `DW0818`, `DW0172`, `DW0112` on a branch's `leads_to`, `DW0482`, `DW0197`, the not-open-yet arm of `DW0817` and the gated arm of `DW0816` — the set `references/story.md` already enumerates by rule) are reported under it rather than as the worker's. Everything else is judged as today. A `--district` naming no declared district is `DW1009`, listing the declared set. Without the flag, `validate` judges the whole, which is the planner's run after a wave is integrated.

### 7.6 `DW1008` — a write outside the district's write set **[authored, on CLAUDE.md]**

The documents are refused by the engine where they are read; the **tree** is refused where it is committed. A district's write set is derived from the declaration and the plan: `campaigns/<id>/districts/<x>/**`; `campaigns/<id>/programs/<stem>.json` and `campaigns/<id>/design/places/<stem>/**` for each node `<stem>` of the district; the piece files under the prefab directory that the district's `details[]` rows name. `delvec district writes <campaign> <district> [--paths <file>|-]` reads a list of repository-relative paths and refuses any path outside the set, exit 1, naming the path and its owner (the whole, or the district it belongs to). Two call sites, one rule: the dispatch installs a `pre-commit` hook in the worker's worktree that pipes `git diff --cached --name-only` into it, so the refusal is met at the commit where the write is entered; the integration (§8.2) pipes `git diff --name-only <baseline>..<head>` into the same verb before any merge. A prompt never asks a worker to stay inside its district; the hook does.

## 8. Integration

### 8.1 Two waves, and the plan frozen between **[authored, on §1 rows 2 and 16]**

Steps 5 and 9 are each one **wave**: a dispatch of every district, integrated before the next step. Two waves and not one, because `delvec detail` builds the whole to prove the map still walks, and the whole does not build while any district's quests are unexpanded; so wave A (story) finishes and is integrated, the planner runs steps 6–8b over the assembled whole, and wave B (build) begins on a campaign that validates. A repair after step 10 or 12 is a third kind of dispatch on one district (§9.6).

**The plan is frozen during a wave.** `geometry-brief.json`, `layout-graph.json` (except a fragment's stations) and `site-plan.json` are the planner's and change between waves only; a change re-derives every packet and re-dispatches every district the change touches (a seam moved touches both its districts). A worker whose place does not fit its box returns the finding; it never moves the box (ADR-0032's "a part exceeding its box is a refusal", and §2.2's "the whole's owner arbitrates").

### 8.2 Gates, per district and on the whole

**Before a worker's branch is merged** (the worker runs these in its tree and reports their lines; the planner re-runs the first three on the branch head):

1. `delvec district writes` over `git diff --name-only <baseline>..<head>` exits 0.
2. `delvec fmt --check` over the campaign exits 0.
3. `delvec validate --district <x>` carries no refusal outside `DW1006`'s set; its `DW1006` line's count equals the number of planned quests of other districts.
4. Wave B only: every node of the district has a `details[]` row in the fragment, and `delvec detail <place>` exited 0 on each, with the verdict lines in the report.
5. The worker's report names, per place, every refusal met and how it was answered, and per request to the whole (a thing to pass, a box that does not fit) the contract it concerns.

**On the assembled whole, by the planner, after every wave**: `delvec fmt --check`; `delvec validate` (no flag — no `DW1006`); after wave A, steps 6–8b; after wave B, `delvec detail --all` (every program-detailed place re-made over the assembled whole; the order decides nothing, and it writes nothing unless the whole passes), then steps 10–13. Nothing of the ladder, the world save, the renders or the play server runs anywhere but here (§1 row 24).

**The merge** is a plain local `git merge --no-ff <worker-branch>` into the campaign branch, one worker at a time, in the order the workers returned; a branch that no longer merges cleanly is a planner finding (two workers wrote one path, which the write sets forbid, so it is a defect of the write set and not of the workers). A worker's branch is never rebased, amended or force-pushed; a worker adds commits.

### 8.3 Routing a whole-campaign red to its district **[authored]**

`delvec build` writes `validation/districts.json`: the object-to-district ledger — every place, station, synthesized anchor, quest, objective, trigger, wave, actor, assembly, lethal volume, pulse, NPC and detail row, with its district or `whole`; and every leg of `validation/critical-path-waypoints.json` with the district of its start and end places. Every diagnostic the engine prints that names an object of a district prints `[district/<x>]` after the code, read from the same ledger (the whole's objects print nothing). A finding that names objects of two districts, or none, is the planner's. The harness and PackTest reports name steps, legs and objects; the skill's procedure page says to resolve each through the ledger. A finding routed to a district is a repair dispatch on that district (§9.6); a finding on the whole is the planner's repair.

### 8.4 Determinism **[cited: ADR-0006]**

Same documents, same output, whatever wrote them. The parallel route adds fragments and a union order (§7.1) and nothing else to the build's inputs; criterion 4 holds the gallery built from fragments byte-identical to the gallery with the same objects inlined, and the existing determinism gate holds two builds of either identical.

## 9. The planner's procedure in the skill

Lands as `references/parallel.md`, named from `SKILL.md` at steps 2, 5 and 9; the planner is the agent running the page. The page's spine stays thirteen steps.

### 9.1 Asking for the worker count **[authored]**

At step 2 the planner shows the partition — `delvec district list`'s table, with any `DW1007` — beside the layout graph, and asks one question: *how many districts shall be authored at once?* The question rides with the step-4 hand-over, because the page stops there anyway and a stop is where an answer arrives; a yes at the gate with no number takes the default. **The default is three.** It is authored, not measured: it is the number of whole-campaign builds a developer workstation runs beside one another without starving them, taken from this project's own dispatch practice, and the demo level (§11.2) measures it — wall-clock and CPU at two and at three workers on one host — and the number on the page is revised to what was measured. A creator may name any number from one up to the district count; one is the current route with the partition as its record.

### 9.2 Sizing districts to workers **[authored, on §2.1]**

Districts are sized by meaning (§4), never by the worker count. The worker count is a throughput: with `D` districts and `W` workers, `W` are dispatched and each return dispatches the next, in critical-path order from `entry`, so the districts the party meets first return first and the planner can validate the spine early. A worker authors one district and exits; the next district gets a fresh worker, which is the property §1 asks for. Two neighbours in flight at once need no conversation: their boundary's contract is frozen in both packets.

### 9.3 Dispatch **[cited: ADR-0012; §2.4]**

For each district: commit the campaign branch (the baseline); `delvec district packet <campaign> <district> -o .out/packets/<x>` (gitignored; the packet is derived, never committed); launch an `Agent` — a fresh agent, never a fork, so it holds nothing but the packet — with `isolation: "worktree"`, whose prompt is the packet's `PACKET.md` plus the standing rules (commit as you go; `delvec fmt` before every commit; `validate --district`; never a server, never `/tmp`, never a plan edit; report, then exit). The dispatch installs the pre-commit hook (§7.6) in the worktree and sets the worker's `--prefabs` to the worktree's prefab directory. The page carries the `Agent` call shape; it carries no model tier (`skill-workflow.md` §2 holds those, and this spec changes that record's sentence that the page carries no dispatch: it now carries this one, because the creator's session is where the workers run).

### 9.4 Reviewing a return **[authored]**

The planner reads the worker's report against the packet: the five gate lines (§8.2), the per-place renders the worker made with `grammar expand` and a render against the place's concept image, the requests to the whole. It re-runs §8.2's first three gates on the branch head, merges, and runs the whole's gates. A request to the whole is answered in the whole's documents before the next wave; a render that does not read as the place is a repair dispatch, not a merge refusal (the piece is the district's to fix, and it is already on the branch).

### 9.5 A worker that fails or runs out of context **[authored]**

The branch is the state, not the worker's memory. The planner reads the branch (`git log`, `delvec district list` for the district's census: places detailed of total, quests expanded of planned), re-derives the packet, and dispatches a fresh worker on the same district with the packet and one line: continue from the branch; what is committed stands. Never resume by amend; never dispatch a second worker on a district whose first is still running (one district, one worker at a time — the write set is per district, not per worker).

### 9.6 A repair dispatch **[authored]**

A finding routed to a district (§8.3) is a `repair` wave on that district alone: the packet plus the finding verbatim, on a worktree cut from the current campaign branch; the same gates; the same merge. The planner never repairs a district's object in the whole's documents to get green.

## 10. The creator's experience

The creator types the prompt and sees: at Init I5 the jar question, unchanged; at step 2, nothing new unless they read the partition table the planner prints; at step 4, **one design gate for the whole campaign** — every scene, near and far, under its sky, as today — with the partition table and the one-line worker-count question beneath it; between steps 4 and 13, a short summary per district as it returns (what was built, the gate lines, any request to the whole) and, if a wave reds on the whole, the finding and the district it was routed to; at step 13 the hand-over, unchanged. The design gate is never split by district, and no per-district approval is asked: a district is the planner's dispatch unit, not the creator's review unit.

## 11. The gallery, and the demo level

### 11.1 Gallery (spec-0039)

The gallery's site-plan overlay declares two districts with one boundary seam between them, one spine quest of the whole crossing it with an item, one district quest per district in a fragment, one NPC in a fragment, one station declared from a fragment, and one detail row per district written by `detail` into the fragment. Probes, each the primary plus one declared edit, refused with the code named: `DW1003` (a node in no district; a node in two; a disconnected district; an undeclared `districts/<x>/`), `DW1004` (a fragment NPC anchored in the sibling; a fragment quest `depends_on` the sibling's quest), `DW1005` (`state` in a fragment; `palette` in a fragment), `DW1009` (`--district district/nobody`). `DW1006` and `DW1007` are exercised by the coverage gate's run lines (`validate --district` over the gallery; `district list` with one district declared below the lower bound and said so).

### 11.2 Demo level: The Toll Fair

**The Toll Fair** — a fair held where a toll bridge crosses a stream below a chapel hill, 15–20 minutes, 1–4 players, no combat. Three districts by meaning: **the fairground** (a green of stalls, the ale tent, the reeve's table — open places, timber and canvas), **the toll bridge** (the toll-house on the near bank, the bridge as a connector place belonging to the far bank's district by §3.5, the far bank — stone and water), **the chapel hill** (the hill stair, the chapel — rubble stone and slate, the landmark seen from the green). The spine, the planner's: the reeve's letter carried to the priest; a toll token bought at a stall and spent at the toll-house (an item crossing); a pedlar who walks from the green to the bridge at a beat (an NPC crossing); the chapel bell as a pulse heard on the bridge (sound crossing). Authored by two workers: the fairground and the toll bridge in flight, the chapel hill dispatched on the first return; each worker's `PACKET.md` committed to the level's `design/packets/` after the run as the record of what each knew. The level's `GENERATION.md` records: the partition table and its `DW1007` lines; wall-clock and CPU for the two waves at two workers, and the same level re-run at three, on the named host (the measurement §9.1's default owes); the write-set refusal transcript (a worker's commit touching the sibling's fragment, refused by the hook); the `[district/…]` prefix on one routed finding; the determinism comparison (the level built from fragments and with the same objects inlined, hashes equal). The owner's walk judges whether the three districts read as three places of one fair, and whether the bridge reads as the way to the far bank and not as a piece of the green. Row added to `docs/demo-levels.md`.

## 12. Why no ADR

ADR-0012 stands: Claude Code is the runtime, the planner is the session, the workers are its `Agent` tool, and nothing is built to schedule, retry or manage context. ADR-0002 stands: the stages are unchanged and a fragment is a stage's own schema in a second file. ADR-0032 stands: one map route, and a district is a set of the one plan's places. ADR-0006 stands and is proved (§8.4). The decisions here are a DSL surface (`districts[]`, `district`, fragments), three subcommands, a flag, a ledger and a skill procedure — spec territory. The one record whose sentence this changes is `docs/reference/skill-workflow.md` §2 ("the skill page carries no subagent dispatch"), a reference page updated by the implementing change; it is not a decision an ADR holds. If the implementation finds that the planner needs a facility Claude Code does not offer, that is the trigger for an ADR, and this spec names it as such.

## Acceptance criteria

Each criterion is checked against the tree at `c1c4a6384b4a3d670dc3ed4ffccb39c30d261060`; **none is satisfied there** (`grep -rIl district` over `crates/ docs/ .claude/ tools/ gallery/` matches 0 files). `delvec` is the implementing tree's binary, built from its manifests.

1. **Surface.** `delvec schema --stage layout-graph` exports `districts[]` with `id`, `nodes`, `note` and nothing else; `delvec schema --stage quest-plan` exports `quests[].district` as optional; `compiler.md`'s stage rows carry both; a test holds the exported fields to exactly these.
2. **The partition.** Each shape of §3.3 is refused with `DW1003` and its remedy by a fixture under `crates/delvec/tests/`, with a `remedy_reachability.rs` row each; a graph with no `districts[]` prints `0 district(s): the whole` and raises nothing.
3. **Fragments.** `load_campaign_dir` reads `districts/*/{npcs,quests,dialogue,layout-graph,detail-plan,design}.json`, unions them in sorted district-id order after the whole's own, and hashes each into the manifest inputs (a test perturbs one fragment byte and asserts the manifest hash moves); each row of §7.1's right-hand column is refused with `DW1005` by a fixture; each row of §7.3's table is refused with `DW1004` by a fixture, and a sibling reference of each of the three kinds named is refused by a fixture; a whole's quest naming a district's station passes.
4. **Determinism.** A test builds the gallery with its district objects in fragments and again with the same objects inlined in the whole's documents and asserts the datapack and the world are byte-identical; the existing two-build gate stays green.
5. **`--district`.** `delvec validate --district <x>` on the gallery with one district's quests removed prints `DW1006` with the removed count and no refusal from §7.5's set; without the flag the same tree refuses with `DW0150`; `--district district/nobody` is `DW1009` listing the declared set.
6. **`district list`.** Prints one row per district with the counts of §4.2 and the binding line; a fixture with a one-place district prints `DW1007` naming the lower bound and exits 0; `delvec district` with no districts declared prints the zero line.
7. **The packet.** `delvec district packet` on the gallery writes `packet.json`, `PACKET.md` and `images/`; a test asserts every boundary edge appears once with the seven contract fields of §6.3 (`passes` carrying the gallery's crossing item under `items[]` and the party's direction), every `places[]` entry equals `delvec allocation` on that place byte for byte, every image path named exists under `images/`, and the write set equals §7.6's derivation; a `--prefabs` outside the repository is refused naming the remedy.
8. **Writes.** `delvec district writes` refuses a path outside the set with `DW1008` naming the owner and exits 1, and exits 0 on a path inside; `tools/creator/` carries the hook installer and the integration call, and `grep` finds the rule implemented once (one subcommand) and called from exactly those two sites.
9. **Detail.** `delvec detail <place>` on a gallery place in a district writes its row into `districts/<x>/detail-plan.json` and no row into `detail-plan.json`; on a campaign with no districts it writes to `detail-plan.json` as today; the piece lands under `--prefabs`.
10. **Routing.** `delvec build` writes `validation/districts.json` with a row per object class of §8.3 and per critical-path leg; a test perturbs a district's quest to raise a refusal and asserts the line carries `[district/<x>]`; a whole's object's refusal carries no prefix.
11. **Gallery** (spec-0039). §11.1's elements are bound and its probes refused with the codes named, with the coverage gate's counts on its own line; `gallery/baseline/` is regenerated after the merge commit exists with every moved row attributed.
12. **Skill, same change.** `SKILL.md` step 2 names the partition and `references/partitioning.md`; step 4 carries the worker-count question and the default; steps 5 and 9 name the two waves and `references/parallel.md`, which carries §9.1–§9.6 and the `Agent` call shape; `tools/ci/check-skill-page.py` is green; `docs/reference/skill-workflow.md` §2 states the planner-and-workers shape and points at both pages.
13. **Docs.** `compiler.md`: the two stage fields, the fragment table of §7.1, the seven codes in the diagnostics catalog, `validation/districts.json` under *World / build output*; `tools.md`: `delvec district list|packet|writes` and `validate --district`; `tools/ci/check-dw-codes.py` green with the seven codes; `docs/specs/README.md` carries this spec's row; `docs/demo-levels.md` carries §11.2's row.
14. **The demo level.** The Toll Fair is built through `/new-delve` with three districts and two workers, and its `GENERATION.md` carries every record §11.2 names, the measured host figures among them; `references/parallel.md`'s default is the measured number, and the guide's bounds cite the level.

### Recorded debts

Checked against the ground; each is a debt, never a pass.

- **§9.1, the default of three.** Authored, not measured, until criterion 14 measures it.
- **§4.2, the guide's bounds.** Rest on two campaigns (The Treehouse Camp completed at 11 places by one agent; The Stranding not completed at 55); the guide marks them authored and criterion 14 adds a third point.
- **§6.3, `passes.items`.** Derivable only from stage-5 effects already written, so wave A's contract lists the spine's items alone and a district's own crossing is a request to the whole; the engine has no plan-level item declaration and this spec adds none.
- **§8.3, the harness.** The bot's and PackTest's reports do not print the district; resolution goes through the ledger by hand on the procedure page until a later change teaches the readers the ledger.
- **§7.3, a region.** A `region` is located by its centre anchor, not by "every cell inside the district's claims": the region's cells need the derivation's anchor positions, which validation does not have. A region whose centre stands in the district and whose extent reaches over its boundary is not refused.

### Loosenings declared by the implementation

Each reduces what a rule above asserts, and is declared as a loosening in those words.

- **§7.3, a quest's cast positions — a loosening.** The cast ledger (spec-0020, `DW0460`) demands every NPC live during a quest in that quest's `cast`, so a district quest must state where the whole's NPCs and its siblings' stand, in places outside the district. A cast row that only records where an NPC the quest does not act on stands is therefore treated as the whole's state, not a placement: it is neither located nor a join. A row for an NPC the quest names anywhere outside its `cast` is judged as §7.3 says. Without it no district quest could be written while any NPC of the whole is live.
- **§7.5, the `--district` set — a widening of what is reported under `DW1006`.** Beyond the set §7.5 enumerates: `DW0112` on a reference naming a sibling's unexpanded quest (the whole's spine waiting on a district's quest), `DW0181` and `DW0187` on a sidecar row a sibling's unexpanded quest owns (an orphan only because the quest is not in this tree), and `DW0460` on a quest not of the district about an NPC the district's fragment declares — another quest's cast names it once the wave is integrated, and that row is not the worker's to write. The planner completes the ledger at integration (`references/parallel.md`).
