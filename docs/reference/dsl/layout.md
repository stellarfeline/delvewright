# `delvewright_dsl::layout`

The reference page for `crates/dsl/src/layout.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0869` | **A station takes a name the engine derives** (spec-0052 §7.1). Validation tier (exit 1), `dsl::layout`. A layout-graph node's `stations[]` entry whose `anchor` begins `anchor/node-`, `anchor/seam-` or `anchor/unlock-`, or equals `spawn`. **The prefix is the rule, not the collision**: `anchor/seam-vestry-door` is refused whether or not the graph has such an edge today, so adding the edge later cannot turn a legal graph into two claims on one name. `spawn` is reserved by its exact name because it is one name with no family. Prescription: name the station something of the author's own — the quest layer references a declared name exactly as it references a derived one. |
| `DW0870` | **Two stations claim one name** (spec-0052 §7.2). Validation tier (exit 1), `dsl::layout`. Two `stations[]` entries anywhere in the graph — one node or two — declaring the same `anchor`. **The scope of uniqueness is the AREA**, unchanged from the standing rule that every anchor reference resolves in an area, and a site-plan campaign has exactly one, so the campaign's whole vocabulary (synthesized ∪ declared) shares it. Piece anchor names stay piece-scoped and never enter this scope, which is why two pieces may both declare `anchor/door` and collide with nothing. The message names both nodes. Prescription: rename one, or declare it on one place only — a quest in either place may name a station of the other. |
| `DW0871` | **A reference demands a shape the station is not** (spec-0052 §7.3). Validation tier (exit 1), `dsl::validate`. Judged at the reference site **from the declaration**, with zero pieces bound, so the answer does not wait for a piece to arrive; when one does, `DW0842` demands the same shape of the piece anchor it binds to, so the two readings cannot drift. The demand travels with the reference in `QuestEffect::anchor_refs`, the ONE authority on the anchor-bearing effect surface, so a new anchor-bearing variant cannot be added without stating what it does with the anchor. **Gate-demanding sites**: `open-gate`, `close-gate`, a `shortcut`'s `gate`, a `timed-gate`'s `gate`. **Point-demanding sites**: every objective anchor, an NPC or actor station, a `move-actor`/`move-npc` destination, a trigger's `at`, a trap's `at` and disarm, a timed gate's disarm, a shop counter, a loot chest, a lane waypoint, every camera field, and **the centre of every anchor-centred volume** — a `lethal_volumes[]` region, `damage-players`'s `in`, a `volley` kill zone, `collapse`, `begin-stealth`, `fill-region`, `clear-region` — all of which are a `StealthZone`, resolved from a point plus an extent rather than from a region anchor. Returns nothing for a prefab campaign by construction: `AnchorRegistry` answers names only, so a piece's shape stays the compiler's to discover at placement. Prescription: change the station's `kind` in the layout graph, or name a station that is already the demanded shape. |

### DW0814–DW0822 — the layout graph (`dsl::layout`; error + advisory)

Stage-3 of the map pipeline: the campaign's space checked as an object of its
own, **cheaply, before geometry exists to make it expensive**.

**One tier, and it is validation (exit 1)** — referential wellformedness,
agreement with the mission, and reachability alike. `dsl::layout::check` is
called from `validate_campaign_with` whenever either document is present, and it
is the only caller of `dsl::layout::reachability`, so there is one battery in one
place, no second copy of any rule, and no step anyone has to remember. `delvec
analyze` and `delvec build` both validate first, so a graph fault still cannot
reach a built world.

**What decides a tier here is when a check can fire, not what kind of question it
asks.** `DW0816`/`DW0817`/`DW0819` are reachability questions like
`DW0202`–`DW0204`, but raised from `compiler::analyze::analyze_campaign` they
would fire at no step they exist for: the analysis pass runs only on a campaign
that already validates, and a campaign at the graph step carries `DW0150` by
construction (the plan is written and stage 5 is not), so the graph would go
unchecked until stage 5 was written — past the design gate. The battery is a
function of the campaign documents — it reads no plan, no prefab and no block —
so it runs at validation.

**The proofs read the mission, so they say so while the mission is unwritten.**
`Grants::of` derives a flag grant from stage-5 quest effects and stage-6 dialogue,
so a campaign between the plan and the quests has no `set-flag` anywhere and every
flag-gated way in its graph is shut to the closure. A **quest**-gated way is not
affected: a quest is credited once every one of its beats sits at a reached place,
and beats are the graph's own document. So a reachability refusal computed while
stage 5 declares no quests carries a caveat naming that state and pointing at
`DW0150` — attached only where the mission's absence could be the cause, which is
a graph that gates on a flag at all, and worded as a caveat rather than a
dismissal, because a place can be unreached for reasons the mission has nothing to
do with and those are findings now. `DW0818`'s clause is the same predicate and a
different consequence: there the absent mission is the whole of the fault, so it
says the refusal clears; here it says which of two readings the author must
choose between.

| Code | Meaning |
|------|---------|
| `DW0814` | **The graph is not a graph.** A duplicate node or edge id, an end naming no declared place, an edge with both ends in one place (at every class — a self-loop states nothing a place does not already state, and would later be a seam with no face to sit on), a `critical_path` step or a `beats[]` entry naming no place, an `entry` or `goal` that is not a node, a place with an empty `intent`. Referential wellformedness, refused before any semantic check runs, because every check below reads node ids and a dangling one would make each of them answer about a place that is not there. Malformed ids are the ordinary `DW0110` and duplicates in the brief the ordinary `DW0111`: an id is an id. Validation tier (exit 1). |
| `DW0816` | **A node the closure never reaches — or scenery it does.** Under the monotone closure (§2), a place unreachable from `entry` respecting gating and one-way direction. The message names the place, **the nearest place a body can stand** — so the missing link is visible rather than searched for — and how many of the graph's places are reachable at all. Validation tier (exit 1). While stage 5 declares no quests and the graph gates on a flag, the message carries the unwritten-mission caveat described above. **Binding: places examined, stated.** **Both ways** (spec-0098 §14): a node declared `reached: false` — scenery, built to be seen and never entered — is refused when the closure DOES reach it, naming the entry; a vision edge is how a place is seen. |
| `DW0817` | **The critical path does not hold.** Four faults under one code, because they are one claim: it does not run `entry` → `goal`; a step names two places no connection joins, or joins them the other way; a step crosses a connection **not open yet at that point in the walk**, judged stepwise against what the beats bound to places already visited have granted (quest-legal order, not merely eventual satisfiability); or it never visits a place where a beat of the **mandatory quest spine** happens, so a body walking it would reach the goal without doing the mission. The spine is the finale and everything its `depends_on` chain demands. Validation tier (exit 1). The unwritten-mission caveat rides the **not-open-yet** fault alone, and only when that connection waits on a flag: the other three are judgements about the graph by itself, which an absent mission has nothing to say about. **Binding: path steps checked and spine beats required, both stated in the binding line**, and a zero on the second is reported there as a finding — a critical path over an unbound graph is a route through nothing. The count is stated rather than raised as a diagnostic on purpose: a line saying *this bound to nothing* is not a fault, and the binding line is where a count belongs. |
| `DW0818` | **The graph names quest-side state that does not exist, or a beat has no place.** Four shapes of one referential rule between the graph and the quest documents: a `beats[]` entry naming a quest or objective the mission does not declare; a `gating` naming a flag no producer sets or a quest that does not exist; a `barred` connection whose `gating` is **empty**, which is passable from world load and therefore not barred; and the reverse direction — an objective in the quest documents bound to no node, or to two. The flag half reads `dsl::validate::produced_flags`, the one producer inventory, so the answer here is the same answer `DW0172` gives. **The reverse direction is the ordering tooth between the mission and the space** (spec-0049 §7): a graph may be authored at any point without touching the quests, and it may not coexist with a mission it ignores. Validation tier (exit 1). **Where stage 5 declares no quests at all**, every name this rule borrows from the mission is absent at once — every beat and every quest-gated way — so each refusal carries a clause saying so and pointing at `DW0150`, which is the one diagnostic that names that state. The clause is attached to both borrowing sites rather than to the beat alone: they are the same borrowing, and a clause on one of them would be a binding narrower than the rule. |
| `DW0819` | **A one-way edge strands.** For every one-way traversal edge `u → v`, some path from `v` back to the critical path must exist over edges passable under the obtained set with which `u` was **first** reached. A body can only be at `v` having been at `u` holding at most that much; if it cannot rejoin the spine, the drop is a softlock. **Marked judgement**: the set at `u` is the maximal one available at that round, and a player may arrive holding less — the residual is covered over bytes by the branch-aware battery, and a walked blockout demonstrating a strand this called green is the evidence that moves it to a gate-state lattice. Validation tier (exit 1). Carries the unwritten-mission caveat on the same terms `DW0816` does. **Binding: one-way edges examined, stated.** |
| `DW0820` | **A shortcut closes no loop.** An edge marked `shortcut` must lie on a cycle: its ends stay connected with it removed. **Direction-blind and gating-blind** — the loop a shortcut closes is spatial, and a long way round that is gated or one-way is still the long way round. A shortcut that closes nothing is a corridor wearing a shortcut's name, and the graph is where that claim is cheap to refuse. Validation tier (exit 1). **Binding: shortcut edges examined, stated.** |
| `DW0822` | **The pacing measurement.** Per critical-path leg, the box's LONG horizontal extent read off the site plan — the size the author declared by drawing the box — summed and multiplied by the pacing coefficient into a projected route-minutes figure. **Warning, exit 0, with no threshold anywhere**: the coefficient is uncalibrated until the first walked blockout and the first full playtest, and a threshold on a number that uncertain would be defending nothing. It is printed so the projection and the measurement taken over the built world can be set side by side, which is how the coefficient gets calibrated at all. **Binding: places crossed and steps measured, both in the message.** Where the campaign carries no site plan yet, a leg has no geometry and the line says so: the leg is stated **unprojected** in its own binding rather than given an invented number, which is the ordinary state of a graph authored before its embedding and not a fault. **The message is one line**: the figure and the world's own `target_minutes` beside it, so the reader has something to read it against, with the reasoning above left here rather than reprinted every run. Subject: the campaign — it measures THIS graph. |
