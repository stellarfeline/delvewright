# Web narrative — structures, endings and guidance for a delve

For the agent planning a delve's quest plan (step 3) and expanding it (step 5): which non-linear structures established games use, how their endings read the whole run, which of them fit a 2–3 hour delve, how a level guides without highlights, and what the engine can express and prove for each today.

Every claim is marked **cited** (with its source, listed at the end) or **authored** (this page's judgement, with its reason). Every source below is used for its ideas only and paraphrased; none is quoted or ported (ADR-0013, `docs/ACKNOWLEDGEMENTS.md`).

## Contents

- [1. What goes wrong in a chain](#1-what-goes-wrong-in-a-chain)
- [2. Structures](#2-structures)
- [3. Endings that read the run](#3-endings-that-read-the-run)
- [4. Fit for a 2–3 hour delve](#4-fit-for-a-23-hour-delve)
- [5. Guidance without highlights](#5-guidance-without-highlights)
- [6. Engine gaps](#6-engine-gaps)
- [Sources](#sources)

## 1. What goes wrong in a chain

A delve played as a chain shows four defects together. **Authored**, from a playtest of a linear campaign:

- every step needs the one before it, so there is one story and one order;
- a gate the story relies on does not hold, because the body has a move the gate does not stop (swimming round a door);
- the ending is decided by the last dialogue choice alone, so nothing the party did earlier counts;
- with markers and announcements hidden, nothing in the level says where to go.

Sections 2–3 answer the first and third, section 5 the fourth, and section 6 names what the engine must prove for the second.

## 2. Structures

Ashwell's catalogue of choice-based structures is the reference vocabulary; he notes the patterns overlap and a real work mixes several (**cited**: Ashwell). Each entry: how it works, who uses it, its authoring cost, its known failure.

**S1 — Gauntlet (the chain).** One central thread; side branches die, backtrack or rejoin at once. Easy to author, and most players see most content; players notice the constraint (**cited**: Ashwell). This is the shape section 1 describes.

**S2 — Branch and bottleneck (delayed branching).** Branches rejoin at events every version shares; earlier choices persist as state and change what later scenes do. Playthroughs start alike and diverge as consequences accumulate. Its cost is state tracking, and it needs enough length for change to accumulate (**cited**: Ashwell). Choice of Games builds this way: an early choice moves a numeric score instead of forking the plot, and a later scene tests the score — a chapter-one temperament decides a later duel. Checking specific past choices directly is the alternative they use sparingly, because tracking many individual decisions becomes unwritable (**cited**: Fabulich). Failure: merge too aggressively and the game becomes only its scores (**cited**: Fabulich).

**S3 — Hub and spoke / open map / parallel threads that converge.** Several branches leave a central node and return to it; travel between major places is reversible, so the geography is static and roamable. Suited to exploration and a consistent world; the pace slows and the story is less directed, and it needs state tracking (**cited**: Ashwell, quest, open map and spoke-and-hub patterns). Dungeon design shows the same shape in space: a lock-and-key graph in which several keys are live at once gives the player a choice of order, while a dungeon that only ever offers one locked door is a chain however large its map (**cited**: Brown, *Boss Keys*, via Boris the Brave and Zelda Universe). Failure: a scene written assuming an order breaks when the player arrives by another; inkle's open-world *Sorcery!* was scripted with "defensive logic" so the story makes sense whatever order its persistent consequences arrive in (**cited**: Ingold, GDC session description).

**S4 — Gated by knowledge, not doors.** Progress is what the player has learned, not what they hold; *Outer Wilds* was built to drive exploration without assigned missions, through diegetic, player-determined progression (**cited**: Beachum, GDC session descriptions). In a game with no runtime language model, the game can only recognise knowledge as a fact it recorded when the player witnessed or heard something; what the player merely inferred is invisible to it (**authored**: spec-0001 forbids runtime interpretation). Failure: the player knows the answer and the game does not, so the gate refuses a player who is right (**authored**).

**S5 — Quality-based narrative / storylets / floating modules.** Small storylets unlock on numeric qualities; there is no tree and no central plot, and the player plays whichever storylets are open (**cited**: Short, *Beyond Branching*; Ashwell, floating modules; Failbetter's *Fallen London*). Cost: content is uninteresting until a large number of storylets exist, every quality is bookkept by hand, and a large pool needs ranking (**cited**: Short). Without a lot of content it collapses into a linear system (**cited**: Ashwell).

**S6 — Salience-based selection.** From a pool, the most specific item that fits the moment is chosen; *Left 4 Dead*'s and *Firewatch*'s dialogue systems are the examples. Failures: an unauthored sequence (a salience match can produce a badly timed outcome) and hard testing (**cited**: Short).

**S7 — Many routes to one goal (immersive sim).** One objective, several physical approaches. *Deus Ex* decided midway through development that each path should support at least combat, stealth and hacking; *Human Revolution*'s level designers stated a "multi-path, multi-solution" rule (**cited**: Spector postmortem via GameBanshee; PlayStation Blog). Cost: geometry is built several times over for one beat (**authored**). Failure: a route the designer did not intend is still a route — a body move the gate does not model opens a path nobody built (**authored**, from section 1).

**S8 — Sorting hat.** Heavy early branching sorts the player onto one of several mostly linear branches. The author effectively writes several games (**cited**: Ashwell).

**S9 — Time cave.** Heavy branching, many endings, little merging; most players miss most content (**cited**: Ashwell).

## 3. Endings that read the run

**How established games do it** (all **cited**):

- **A tally with a threshold.** *The Witcher 3* decides Ciri's fate from five scenes spread across the game, each recording a positive or negative reaction; three positives give the better endings (Shacknews; Prima Games). *Dishonored*'s chaos is a hidden running value fed chiefly by kills and by some side objectives; the overall value going into the final mission sets the final level and the ending (Dishonored Wiki).
- **An ending composed of per-thread outcomes.** *Fallout: New Vegas* closes on a sequence of slides: the main faction outcome, then one slide per faction, companion and many side quests, each chosen by what the player did in that thread (secondary coverage of Sawyer and Avellone interviews).
- **Seed early, read late.** An early choice is stored as a score and tested by a later scene (Fabulich). The seed is made long before it is read; the reading is the payoff.

**How many inputs.** One aggregate fed by every action (*Dishonored*); five discrete scenes (*The Witcher 3*); dozens of independent per-thread slides (*New Vegas*) (**cited**, as above).

**The failure "only the last choice matters".** *Mass Effect 3*'s original ending came down to a final three-way choice, and critics' dominant complaint was that it ignored the decisions of the whole series (**cited**: Den of Geek; TrueAchievements). Its accumulated input, military strength, existed but mostly gated variants of the ending and was tied to multiplayer readiness, which drew its own criticism (**cited**: RPG Site; Game Informer). Telltale's "will remember that" acknowledgements are the related failure: choices are noticed on screen and then converge to the same outcome (**cited**: Den of Geek; Game Informer). The literature's remedy is the delayed branch: the ending reads state the whole run wrote (**cited**: Fabulich; Ashwell, branch and bottleneck).

**For a delve** (**authored**, from the cited practice scaled to the delve's size):

- the ending reads at least three inputs written in different threads, by deeds as well as by dialogue;
- if a final choice exists, which options it offers is decided by those inputs, so the last choice reads the run rather than replacing it;
- every seed is acknowledged where it is made, by a change the party can see, so the ending's reading of it is recognisable;
- the epilogue states the outcome per thread, the *New Vegas* form, so each input is visibly read.

## 4. Fit for a 2–3 hour delve

The delve is 2–3 hours for 1–4 players, all dialogue pre-written (spec-0001), in adventure mode, with a party that may split. Every verdict is **authored**, from the cited costs above.

| Structure | Verdict | Reason |
|---|---|---|
| S1 gauntlet | Spine segments only | A whole delve as a chain is section 1. |
| S2 branch and bottleneck | Fits, with one or two bottlenecks | It needs length for change to accumulate (Ashwell); two hours carry a few consequences, not many. |
| S3 hub with parallel spokes | Fits best | A box-garden is an open map; cost grows linearly in spokes, not exponentially; a split party can take spokes in parallel. Two to four spokes. |
| S4 knowledge gates | Fits, sparingly | Recognition must be a recorded fact from a witnessed event; `docs/reference/game-writing.md` L1 forbids a note or plaque whose only job is to be read so a gate opens, so the knowledge comes from people and events. |
| S5 storylets | Not as structure; fits as NPC variants | The bootstrap volume (Short) exceeds a delve; per-state dialogue variants on one NPC are the local form. |
| S6 salience | Not as structure | It sequences without an authored arc (Short). |
| S7 many routes | Fits, per gate | Two or three routes to a key place, each one built and proven; the cost is geometry per beat. |
| S8 sorting hat | Does not fit | Several delves inside one budget. |
| S9 time cave | Does not fit | Most players miss most content in a work meant to be played once. |

## 5. Guidance without highlights

Where information comes from and how text states it is `docs/reference/game-writing.md` §8 L1 and §1 F1; this section adds only the spatial practice.

- **Legibility.** Lynch's five elements — paths people move along, edges that bound areas, districts with their own character, nodes where routes meet, landmarks seen from afar — are the level designer's map-reading vocabulary (**cited**: Lynch, via Totten's second edition).
- **Landmarks that draw ("weenies").** A landmark visible from far pulls the player toward it; it is hidden and revealed again from new angles as the player advances, because one always in view stops working (**cited**: Rogers, GDC 2009, via Doctorow's notes).
- **Breadcrumbs.** A trail of supplies leads forward, one way, because a collected pickup disappears behind the player (**cited**: secondary analysis of *Half-Life 2*). Light, shadow and props such as lanterns lead instead of markers (**cited**: same coverage of Rogers).
- **Gaze.** Players look where they travel; a climb makes them look up, which is where *Lost Coast* puts its first enemies (**cited**: Walker's commentary, via a secondary guide).
- **In a hub** (**authored**): each spoke's mouth is visible from the hub and distinct as a district, so the open threads are seen, not listed; a spoke that is done looks done.

## 6. Engine gaps

Read against the DSL and compiler as spec-0108 left them (story worlds, the ending reading, a cutscene's `then`): `crates/dsl/src/quest_plan.rs`, `crates/dsl/src/quest/mod.rs`, `crates/delvec/src/compiler/flow.rs` and `flow/story.rs`, `crates/delvec/src/compiler/branch.rs`, `crates/delvec/src/compiler/ending.rs`, `docs/reference/compiler.md` (stage 4, stage 5, nav), `docs/reference/dsl/quest_plan.md`, `docs/reference/dsl/state.md`, `docs/reference/delvec/compiler/branch.md`, `docs/reference/delvec/compiler/flow.md`, spec-0025, spec-0038, spec-0051. Each gap is a capability; none is a design.

What the proofs assume, in one place:

- **The flow replay** (`DW0204`, `DW0879`) walks the exported critical path, which is the participation-minimal order (no optional quest), and each enumerated world's own whole path (everything that completes in that world).
- **A world**, for the reachability codes (`DW0201`–`DW0203`, `DW0879`), is one selection per *dialogue choice group* — a node whose options set conflicting flags (`flow.rs` `ChoiceGroup`, `MAX_WORLDS` 512) — with every elective act performed. **A story world**, for the branch and ending proofs, is a selection **and a participation**: which optional quests and which ambient acts (a trigger the mainline does not press, a trap, a disarm, a purchase, a credited kill) are performed, once or not at all. Per selection the story worlds are the *all* world, the *none* world and one witness per value class the endings' gates distinguish (`compiler.md`, *Story worlds*), so a flag only a deed writes separates two story worlds.
- **The branch proofs** (`DW0480`–`DW0485`) quantify over declared `branch_points` and the story worlds; `DW0480` refuses an undeclared fork where a flag is set in some story world and not another, deed-made forks included. `DW0485` alone searches every play order a branch admits.
- **The ending proofs** (`DW1029`, `DW1030`) read every gate of each ending's final scene, date its writers by quest and kind (said, did, finished) and judge each ending against the campaign's `endings_read` and against the story worlds that reach it.
- **Nav** routes the legs of each path with gate seals applied by causal ancestry (`compiler.md`, "Runtime region solidity"): a seal fired on a parallel thread does not seal a leg of another thread. Water is impassable in every route proof (spec-0038 §5); swimming is modelled only by `DW0921`, which asks whether a body can leave, never whether it can pass.
- **The bot** plays the exported path and one path per story world that has one (`validation/branch-runs.sh`): one order each, performing the world's entry deeds, and reads the aftermath of the ending it reaches.

| Capability | What exists | What is missing | Proof affected |
|---|---|---|---|
| Several open threads at once | `depends_on` is a DAG; siblings activate together; optional quests (spec-0051) attach strands to the spine; a quest's runtime arming is one `quest-complete{quest}` trigger (`quest/mod.rs` `Trigger`). | A join — a quest armed when *all* of several threads complete — has no trigger; it is spelled as an objective gated on each thread's flag or on a counter. Geometry and the bot never walk an order other than the exported one, so an order-dependent block (thread B seals a passage thread A needs, if B is done first) is unproven. | Nav `DW0311` legs; the critical-path bot. |
| Endings that read the whole run | `campaign-complete` takes a full `when` gate; an ending is an id; every gate of its final scene is an input, its writers dated and kinded (`validation/endings.json`); stage 4's `endings_read` declares how many seeded inputs (and seeded deeds) every ending reads (`DW1029`); the story worlds include a witness per value class the endings distinguish, found by running candidate participations, and every ending must be reached by one (`DW1030`); a cutscene's `then` changes the world after the camera returns, and the aftermath is proven by a PackTest per (ending, world) and read by the bot. | A tally that needs part of one optional quest (objective-level participation) is reported unrealizable; a datum a forced act, a stake, a loop or a reaction bundle moves stays undatable and an ending read only on it is refused as unproven; a world whose witness needs a trap, a disarm, a shortcut, a purchase or a credited kill exports no path. | `DW1029`; `DW1030`; ending PackTests; world runs. |
| A fork made by an action | Any effect may `set-flag`; a flag only elective acts write is set in a selection's *all* world and not its *none* world, so `DW0480` reaches it and a two-branch point whose second branch pins nothing set declares it; the world that realizes each side exports a path that performs (or skips) the act. | An act the mainline itself performs (a debt, a way-opening trigger, a counted press, a link, a mandatory kill) is forced and cannot be skipped by any world; an act with no harness step kind is proven statically and skipped by name at runtime. | `DW0480`; story worlds; world runs. |
| Gates keyed to knowledge or state | Every gate consumer takes `requires_flags`, `forbids_flags`, `requires_state`; an `open-gate` effect or a trigger can be gated; flags come from talk, interact and approach beats. | No gap found: knowledge is a recorded flag, proven by producibility and replay. | — |
| A gate that holds | Route-existence proofs: a route *exists* under each gate state. | A proof that a shut gate *separates*: that its far side is unreachable from its near side while it is shut, under every move a body has, swimming and jumping included. | Completability's ordering claim; the bypass in section 1. |
| Many routes to one goal | Geometry may hold several routes; nav finds one. | Declared alternative routes, each proven walkable under its own gate state; today a second route is neither required nor checked. | Nav `DW0311`; the bot. |
| Storylet-style variants | Cast placements and dialogue options select per flag and state; `DW0483` and the cast ladder prove one selection per branch. | None needed at the fitting use (section 4). | — |

## Sources

- Ashwell, S. K. *Standard Patterns in Choice-Based Games*. These Heterogeneous Tasks; mirrored at ifwiki.org/Standard_Patterns_in_Choice-Based_Games.
- Beachum, A. & Verneau, L. *Curiosity-Driven Exploration: The Design of Outer Wilds*, GDC 2020; Beachum, K. *Sparking Curiosity-Driven Exploration Through Narrative in Outer Wilds*, GDC 2021 (session descriptions, gdcvault.com).
- Boris the Brave. *Lock and Key Dungeons*. boristhebrave.com; Zelda Universe, *Explore Zelda dungeon design in Mark Brown's YouTube series Boss Keys* — both on Brown, *Boss Keys* (Game Maker's Toolkit).
- Den of Geek. *The Walking Dead: The False Promises of Telltale's Masterpiece*; *Mass Effect: The First Trilogy's Ending and the Futility of the Hero*.
- Dishonored Wiki (Fandom, CC BY-SA). *Chaos*.
- Doctorow, C. Notes on Scott Rogers, *Everything I Learned About Level Design I Learned from Disneyland*, GDC 2009 (craphound.com; Boing Boing).
- Fabulich, D. *By the Numbers: How to Write a Long Interactive Novel That Doesn't Suck*. Choice of Games blog.
- Game Informer. *Why Your Choices Don't Matter in Telltale Games*; *Mass Effect 3 Extended Cut: The Good, Bad, and Ugly*.
- GameBanshee. *The Deleted Scenes of Deus Ex* (excerpt of Spector's Deus Ex postmortem lecture).
- Ingold, J. *Narrative Sorcery: Coherent Storytelling in an Open World*, GDC 2017 (session description).
- Lynch, K. *The Image of the City* (1960), as organised in Totten, C. *An Architectural Approach to Level Design*, 2nd ed. (CRC Press, 2019).
- PlayStation Blog. *Deus Ex: Human Revolution's Level Design is Multi-Path, Multi-Solution*.
- Prima Games. *The Witcher 3 Ending – The Fate of Ciri*; Shacknews. *Ciri's fate and Best ending – The Witcher 3*.
- Quarter to Three, *What happened to the ending of Fallout: New Vegas?*; Prima Games, *Why Fallout: NV comes to a proper close* (Sawyer and Avellone on the ending slides).
- RPG Site. *Mass Effect 3 Endings guide*; TrueAchievements, *Second Take: Mass Effect 3*.
- Short, E. *Beyond Branching: Quality-Based, Salience-Based, and Waypoint Narrative Structures*. emshort.blog.
- Secondary analyses of *Half-Life 2* and *Lost Coast* developer commentary (gangles.ca; gamertagmythras.com).
