# The gallery

One campaign, holding at least one instance of every content-visible surface the
Delvewright DSL declares. It is built on every pull request, and it is never
played, released or staged.

It exists so that a new authoring surface meets something built to receive it. A
surface no campaign exercises is a surface nothing has ever compiled end to end,
and that is not a hypothetical: the rest of the authored corpus — the campaigns
and the fixtures together — writes only a part of what the DSL declares. The
gallery binds **all of it**: every unit written, or proven refused by a probe
the engine really rejects, and none left over.

**The coverage numbers are not written on this page.** The unit set is enumerated from the
compiler's own `schema --stage all` export, which moves whenever the DSL does,
so a count typed here would be a measurement with nothing behind it. The gate
states it, on every run, and that line is the count:

```
gallery coverage: <N> unit(s) enumerated, <N> bound, <N> refusal-proven, 0 in NEITHER state.
```

The last figure is the one that matters: anything in neither state is a
declared surface nothing has ever compiled, and the gate fails on it.

Surfaces do fail when something first reaches them, which is the whole reason
this campaign exists: an ambush and a named mob drop could not compile at all, a
generated flag-gate test could not pass, and a one-waypoint lane emitted a march
test asserting an index it had no way to reach — each found here, by binding it.
Two are still open: the build's render plan and the one `delvec snapshot`
derives disagree whenever a world-edit blocks the route, and nothing refuses an
objective gated on a state an earlier forced beat clears.

## Reading it

Every element is named for what it is, not for what it binds. Walk it in the
order a player would:

| Where | What it holds |
| --- | --- |
| `world.json` | the hall, its lighting and mitigation, the boundary, the declared languages, and its sky stated in a designer's words: a new moon just risen (`{"moon": "just-risen", "phase": "new-moon"}`, day 4 — every build's `clock:` lines say where each stated time puts the sun and the moon); and two atmospheres (spec-0080): `atmosphere/frost-hall`, which sets every one of the twenty attributes an atmosphere admits and is carried by the hall from the first tick, and `atmosphere/still-air`, which the vantage trigger paints over the east bay and the counter paints back off the whole hall; and and the one vanilla texture the delve replaces — stone bricks, the annex tiles' walls and the hall's tread courses, drawn in one flat colour from `textures/hall-stone.png` so a render shows which block the pack changed — with the pack declared required, and the drowned's outer layer, painted from `textures/drowned-wrap.png` on the boxes that model really builds — the base positions, grown — which is where a mob's second layer reads (spec-0097); and the seconds a fallen player waits before rejoining the party |
| `npcs.json` | five speaking parts — a quest-giver, a gatekeeper, a counter, a drill officer and a copyist at the wall desk who never looks up; the quest-giver is a mannequin that hides its cape layer, and the standard bearer in `quests.json` hides the other six, so every overlay layer a mannequin can hide is written once (spec-0097) |
| `classes.json` | two kits, one carrying a flask (what a bonfire rest refills) |
| `quest-plan.json` | three quests and the branch point the fork opens |
| `quests.json` | the bulk: objectives, effects, waves, actors (a barded and saddled horse among them), traps, triggers, a shop, a shortcut, a stake, a timed gate, three killing volumes — one of them the pit under the terrace annex, live from the beat that clears the lid over it (spec-0088) — and six named datums of which one — the tokens — stands on the sidebar (`display: sidebar`, spec-0076); the hearth's two buttons, like every shop offer and some dialogue options, hover a tooltip saying what pressing them does (spec-0078); and the guidance surface (spec-0093): every objective kind states its `announcement`, the two reach objectives at the end of the hall and over the well are quiet (`announcement: hidden`, the first unmarked too), the ferry deck states both `shown`, and the valley overlay's whole campaign is quiet by `guidance` with its far corner opting its marker back in; the ferry's tiller and the cabin's are levers (`triggers[].prop`), the block the party uses and vanilla reports, so no hitbox stands there; the label under the case is a sign, a visible thing vanilla reports no use of, so its hit area is fitted over the sign; the stone beside the hearth is a stone slab, the thing the `strike` hits, and since the slab's sides lie on its cell's faces its hit area stands one hundredth of a block past them, so a swing at the slab reaches the hit area and not the slab (`DW1013` judges every hit area the gallery places); and the counter's third bark carries two styled spans, one obfuscated and one coloured, which its zh-cn row places in the Chinese order (spec-0096) |
| `dialogue.json` | one tree per NPC; the Curator's carries the fork, and the Marshal's carries the two scenes it leads to — a pair of nodes no option leads to, reached only because the quest's `cast` ledger opens one of them per branch; the Curator says what the Long Hall is before the Marshal sends the party to it, the one told name a later line uses (`DW0982` binds to it) |
| `world-edits.json` | the barrier course, and the batches that dress the floor, lay the hearth, open the vault and rough the lane |
| `geometry-brief.json` | the hall's own numbers and the site's span and depth, which the site plan is held to |
| `layout-graph.json` | three places — the hall, the annex wing and the long hall — and the three `carry` edges the levers realise; the hall's own rooms, doors, stair and drop are its piece's, stated by its spatial contract |
| `site-plan.json`, `detail-plan.json`, `programs/` | the primary's map (spec-0106 §9): the hall bound by `--piece` to `prefab/gallery-hall` and owning its ground, lit by `boxes[].lighting` and mitigated by `boxes[].mitigation`; the annex wing detailed from its program; the long hall bound by `--piece` to a piece whose contract declares zero exterior edges; no seam |
| `overlays/site-plan/` | the same places given geometry, and then a whole map DERIVED from it: a region, a box each stating only its extent and plane, ONE pinned corner, a seam per connection stating which face of its `a` box it sits on, where along it and what the crossing is (the compiler derives every other corner and every sill — spec-0059; `delvec validate` prints the corners), an `open` fill over a generated heightmap with the rock and the sky the whole owns, a declared roof over the annex, whose program draws its own walls and gable while the yard stands as a faced plinth over the falling terrain (spec-0098), a plank gantry hung over the yard (`base: {"aloft": 0}`) that owns no ground, climbed from the yard on a ladder through its deck and walked to a small lookout hung at its far end, and eight comparisons holding all of it to its own written brief. It carries its own world, cast, quest layer and translations, so at this point of the campaign nothing describes a block, and everything a body meets is derived: the floors it walks, the doors it is stopped by, the stair it climbs, the anchors the quests bind to. Inside one place, too: the causeway's program raises a lookout on a stone stand and a crow's nest over it, reached by two ladders stacked one on the other — the first tops out on the foot of the second — which its spatial contract declares as three spaces and two `climb` edges: two floors of one place are two spaces, each ladder is proved both ways on the body's climb moves, and the critical path climbs the stack to the crow's nest (`obj/climb-the-crow`), so the exported route carries two one-column climbs each way. Its factor, whom the route brings the party to from behind the annex stand's facing, declares a watch of the nearest player within 4 blocks, so it turns to the party as they come into reach to talk (`DW1025`); declaring none, it would watch within 8 by default. The curator declares one too: the party reaches the near hall's stand side-on, at the edge of what `DW1025` admits, and a detailed variant of the place puts it behind. The undercroft's ladder and stair join the near hall to the far hall round the dividing wall, so its west and centre doors are declared `shortcut`, as `east-door` is: a gate a body gets round is declared so, or the build refuses it (`DW1011`) — and the same holds in the solid overlay |
| `overlays/site-plan-solid/` | the same seven places carved in rock: the plan states `fill: solid`, so every cell no place claims is deepslate and the exit's sky is a declared clearance well, and a one-cell clearance keeps the vista's air open through the rock between the near hall and the loft. It is its own campaign (`gallery-rock`), so its places — every one detailed from `programs/` by `delvec detail`, because a stand-in never ships — are frozen as `gallery-rock-<place>` and never share a name with the open point's pieces. Beside its open sibling it shows that the kind of site is the plan's declaration (spec-0098); its factor and curator declare their watches for the same reasons as the open point's (`DW1025`) |
| `overlays/valley-site/` | a SITE placed: one place on `horizon: valley` whose piece owns its ground (`base: {"owned": 3}`, spec-0106 §3.1) — `gallery-bank`, three courses of island mass, a bank of grass out to the box's own edge, a walled court on top. The whole's flat terrain meets the piece's bottom course from below (`DW1021` is the refusal when it does not), and the allocation fixes no ring cell: the piece draws its own ground. It carries both halves of `DW0885` at once, the courses under the bank buried by earth and the parapet above it answered by four `shown_faces`, and its own cast and quest layer |
| `l10n/zh-cn.json` | the second language, so the sidecar surface is real rather than declared |
| `render-plan.json` | the view set the gallery declares, so a shot that vanishes is a red |
| `overlays/` | parameter points — settings that take one value per world |
| `probes/` | documents the engine **refuses**, each naming the diagnostic |
| `baseline/` | the expected-warnings ledger — the one baseline document committed; the emission baseline is computed in every CI run from the merge base and the head, both built from source (ADR-0034) |

The hall itself is generated, not committed: one stone room 31 × 31 across,
split by a wall with five barred openings in it — the story gate, the shortcut
door, and three doors on clocks, each opening onto a roofed closet on the near
side, so that while the story gate is shut nothing a body does gets it into the
far half (`DW1011`) — and a mezzanine in the far half whose stair is missing
its treads. When the muster is cleared the story gate comes down again, and the
same beat moves the checkpoint to where the muster stood in the far half,
where the way goes on: a party that dies after the reseal comes back on the side it is still
owed. The near half, hearth and pits included, is behind the gate from then
on, unless the party pulls the side door's lever on the far side and opens the
shortcut back.
Beside it the generator emits a small
**annex kit** — three 7 × 6 × 7 tiles the annex wing's program places by name.
The kit is deliberately plain: its job is the placement, and a tileset with
interesting rooms would make it harder to read without binding one more unit. Each tile carries one
anchor at the middle of its floor, which is what gives a camera pointed into the
annex something the campaign declares to frame. Its anchors are named for their
purpose — `anchor/hearth` is where you come back to life, `anchor/muster` is
where a wave forms up — and the generator prints a one-line `note` beside each
one, so the piece explains itself without the campaign in hand.

Four fights pay for their kills (`on_kill`, spec-0074), one of each shape the
bundle takes, and each writes a datum the hall already reads: `wave/muster`
comes back after every rest and pays `every-kill` into the killer's `tokens`;
`wave/lane`, the boss lane, is re-seated only while it stands and pays
`first-kill` into the party's `bounty` purse; `actor/hall-moth` pays
`first-kill` into the killer's `keepsake`; and `wave/edge`, which nothing seats
twice, states no `fires` and pays the killer's `relics`, with a chime gated on
the bounty — the bounty's only reader. The generated `kill_pays_*` templates run
each one on the pinned server with a PackTest dummy credited by vanilla's own
`player_killed_entity`, beside a death nobody is credited with, every compiler
removal, and a rest.

Three valves stand on the near hall's north wall, levers on the floor, and the
counter waits on them: `obj/reach-the-counter` reads `state/valve-round at-least
3`, which only `trigger/valve-first`, `trigger/valve-second` and
`trigger/valve-third` write. Each answers only in its turn, and the second and
third put the round back to 0 when pressed out of it. The document declares them
third, second, first. The plan replays every press the way the datapack runs
it, line by line, and the path presses first, second, third in front of the
counter. A gate no press sequence can open is refused (`DW0985`).

One place in the hall is climbed to, and a ladder is the only way up. The ferry
cabin's roof stands four courses over the far hall's floor, past any step or jump,
and four rungs hang on the cabin's east wall, facing east. After the party crosses
the strait and is carried back out, `obj/climb-onto-the-cabin` sends it up the
ladder onto the roof (`anchor/cabin-roof`) and `obj/reach-the-end` sends it back
down — so the route proof climbs both ways, the exported legs carry a `climbs`
entry each, and the bot drives both climbs rather than walking them (spec-0099).
Turn the rungs to face into the wall, with the open hall behind them, and the
world no longer keeps them: the build is refused `DW0991`, naming each rung and
the sturdy face it lacks (`probes/a-ladder-with-nothing-behind-it`).

The far hall has a **listening floor** (spec-0100). A sculk sensor is set into
the floor course beside the walk off the loft stair to the ferry deck, so every
footstep on that stretch clicks it; four courses south of it a shrieker stands
on the floor with summoning off, close enough to hear the click, and answers a
player's footstep with sound and particles and nothing else. At the foot of the
long gallery's stair a calibrated sensor faces north with air behind it, so its
filter stays open and it hears every vibration within sixteen blocks; it is more
than eight blocks from the shrieker, so only the plain sensor wakes it. Nothing
in either sensor's reach reads a redstone signal. Over the near hall's roof, in
the solid stone, a sculk catalyst is sealed where no body can stand within eight
blocks: the build states the nearest body cell and its distance. The waypoint
export carries the plain sensor and its shrieker on the legs that walk past them
and the calibrated sensor on the legs up and down the stair, and the bot asserts
each click and shriek and closes the run with `[sculk] darkness 0, warden 0`.
No wave body stands within the sensor's reach while a leg that carries the
shriek is walked: a mob's footstep sets the sensor off and a shrieker answers
only a player, so a body there would take the click the party is owed. The
walk to the Marshal is the one such leg before the muster is cleared, and the
muster forms up only once she has the party's answer; every later leg past the
floor comes after the muster is cleared.
A third sensor is set into the near hall's floor two cells south of the walk
along the valves. Two legs pass within its earshot without predicting it: the
walk from the emeralds to the first valve and the walk from the third valve to
the counter, each walked only on cells from which the party could finish one of
its two steps (a chest or a lever is used from 4.5 blocks, the counter is a
volume), so neither is a walk the party must make. The first leg, from the
arrival to the curator, and the walk from the arrival to the marshal pass it on
cells no step completes from, and carry it.
Four probes each refill one cell: the shrieker with summoning on (`DW0998`), the
sensor caught mid-click (`DW0999`), an iron door in the floor beside the sensor
(`DW1000`), and a catalyst set into the near hall's floor (`DW1001`).

Every block the build writes is one the server keeps (`DW1002`): the build judges
each block whose pinned `canSurvive` rule asks a neighbour for something — the
hall's hanging lanterns, the torches and lanterns the relight pass sites, the
cabin ladder, the floor levers the triggers set, the stamped shards' standing lanterns — against
the neighbours the world gives it, and prints the count on the `attached
blocks` line. The lidded pit and the west well each carry a wall torch on their west wall,
because a lantern hung from the lid would fall with the lid. Hang a lever on a glass
pane's side and the build is refused, naming the lever, its cell and the full
face the pane does not give (`probes/a-lever-hung-on-a-pane`). The build also
lays the load one write at a time, as the server runs it, and prints the
`write sequence` line: leave a block of gravel three blocks over the near
hall's floor and the build is refused, naming the gravel and the air under it
when the world build has run (`probes/gravel-left-over-air`). And it lays every
write the load does not run — the props, the gates, the collapse, the beats
nobody has to play — at once, and prints the `runtime writes` line: the case's
button stands on the pedestal (`stone_button[face=floor]`), where the bare
button would hang on a wall north of nothing. Have the plate trap's payload, or
only `branch/skipped`, clear the pedestal column and the build is refused,
naming the button and the air it may stand on: a fill clears from the bottom
up, so the server drops the button as an item
(`probes/a-trap-that-takes-the-case-button-floor`,
`probes/one-branch-takes-the-case-button-floor`).

One place in the hall is found rather than named, and it is the only one: the
cell a body arrives at. `anchor/arrival` is named like every other place and is
the entry because it declares the entry **role**; ten cells down the same floor
stands an anchor called `spawn`, which is the compatibility spelling the
compiler falls back to for a piece older than that role, and which nothing in
the campaign binds. The pair is the element: with the role declared the party
arrives at `anchor/arrival`, and taking the role away moves `setworldspawn`, the
first-join placement, the checkpoint seed, both class-apply teleports and the
first player-POV frame onto the decoy, in a campaign that still builds. A gallery
element that cannot fail when the surface it covers is removed is coverage in
name only.

## What the engine refuses

`probes/` is the half worth reading if you want to know what this engine checks.
Each probe is a committed document that a creator might reasonably write and that
the engine says no to, with the diagnostic it says no with. Most are refused by
`delvec validate`, which reads the documents; a rule about GEOMETRY has no verdict
until the anchors resolve, so those are refused by `delvec build` instead, and a
probe that ships a program is refused by `delvec detail`, where the program is
entered. The table says which. Some name no surface at all, and that is the point of them: both
halves of what they write are perfectly legal, and what the engine refuses is
holding them at once.

| Probe | Code | Refused by | What it tries |
| --- | --- | --- | --- |
| `aquatic-locomotion` | `DW0455` | `validate` | declaring a body that swims |
| `peaceful-difficulty` | `DW0468` | `validate` | setting the world to peaceful |
| `sound-at-actor` | `DW0335` | `validate` | playing a sound from an actor's position |
| `a-piece-the-library-does-not-hold` | `DW0842` | `validate` | binding the hall's place to a piece whose name is one letter wrong |
| `a-question-nobody-answers` | `DW0858` | `validate` | asking the party to talk to somebody the same quest declares silent |
| `a-piece-that-is-not-its-frame` | `DW0843` | `validate` | seating a piece that does not fill the box the map gave it |
| `a-horizon-with-no-map` | `DW0855` | `validate` | declaring a horizon that builds terrain on a campaign with no site plan |
| `a-world-still-carrying-areas` | `DW0100` | `validate` | carrying the removed `areas[]` beside the site plan |
| `a-quest-still-booked-to-an-area` | `DW0100` | `validate` | booking a planned quest to an area with the removed `area` field |
| `a-hall-with-its-lamps-undeclared` | `DW0210` | `build` | removing the hall place's `boxes[].lighting` and `mitigation`, so its dark cells are a finding naming the piece |
| `a-station-the-hall-does-not-answer` | `DW0842` | `validate` | declaring a station on the hall that its bound piece has no anchor for |
| `the-valley-lowered-under-the-bank` | `DW1021` | `validate` | lowering the valley site's ground a course under the bank, which owns its ground |
| `the-bank-on-the-ground` | `DW0990` | `validate` | declaring the bank `base: ground` over ground at its own bottom course, so the whole fixes the ring the piece draws |
| `a-clock-nobody-explained` | `DW0860` | `validate` | arming a stealth clock that bites before its own instruction can be read |
| `a-barrel-in-a-row-of-barrels` | `DW0861` | `validate` | adopting a container the piece placed without naming what is in it |
| `a-prompt-nobody-sees` | `DW0862` | `validate` | writing a hint on an objective with no title to carry it |
| `a-fight-nobody-points-at` | `DW0863` | `validate` | requiring a fight and saying nothing about where it happens |
| `a-bar-in-a-colour-the-game-lacks` | `DW0911` | `validate` | drawing a boss bar in `crimson`, a colour the pinned game's `bossbar` command does not list |
| `a-bar-over-a-body-nothing-can-hurt` | `DW0909` | `validate` | hanging a health bar over the usher, a body nothing unleashes and nothing can hurt |
| `a-bar-with-nothing-to-draw` | `DW0910` | `validate` | removing the muster's bar `title`, when the wave fields two kinds of body and no one name is the fight's |
| `a-floor-carved-down-to-the-sea` | `DW0344` | `build` | a world edit cutting a three-by-three hole in the hall's plinth course down to the sea's own plane, under `horizon: ocean` |
| `a-gate-the-party-walks-back-through-after-it-is-sealed` | `DW0485` | `validate` | giving the last mainline beat the main gate as its subject, three beats after that gate is sealed |
| `a-strand-walked-after-the-gate-it-crosses-is-sealed` | `DW0485` | `validate` | giving the optional reliquary's first beat the main gate as its subject: the exported path walks it before the seal, and a player may walk it after |
| `a-key-that-charges-more-than-it-asks` | `DW0901` | `validate` | charging five tokens at the counter that gates on four |
| `a-lane-beside-a-killing-volume` | `DW0891` | `build` | setting the west pit on the near hall's floor beside the chest, so it kills the walked ring around it |
| `a-number-this-engine-does-not-implement` | `DW0102` | `validate` | declaring a `dsl_version` other than the one this engine accepts |
| `a-picture-nobody-looks-at` | `DW0900` | `build` | re-aiming the only camera that answers `concept/morning-quay` at a picture two other cameras already answer |
| `a-picture-nobody-recorded` | `DW0890` | `validate` | deleting the night row from `design.json` while its image stays under `design/concept/` |
| `a-place-cut-below-the-sea` | `DW0886` | `validate` | standing a site-plan place two courses under the sea plane, under `horizon: ocean` |
| `a-program-asking-for-a-seam-the-whole-does-not-hand` | `DW0882` | `detail` | declaring a `handed/` parameter for a seam the allocation does not hand |
| `a-program-that-marks-no-place-to-stand` | `DW0845` | `detail` | removing the program's one `mark`, so nothing answers the anchor the quests bound to the place |
| `a-program-whose-arch-misses-its-seam` | `DW0844` | `detail` | carving the annex arch one cell along the wall from the seam the plan handed it |
| `a-reach-that-completes-from-the-floor-below` | `DW0881` | `build` | widening the loft's completion radius to 2, so it completes from the hall floor three courses below |
| `a-rim-one-radius-out-of-reach` | `DW0850` | `build` | narrowing the well's completion radius to 2, so no walked cell on the rim reaches it |
| `a-rocket-under-a-roof` | `DW0899` | `build` | firing a rocket at `anchor/exit`, under the hall's stone ceiling |
| `a-tiller-pulled-from-the-hall-floor` | `DW0932` | `build` | playing the ferry's cutscene before its carry, so a pull from the open hall floor puts the whole party down there and nobody crosses |
| `a-bolt-beside-the-arrival` | `DW0958` | `build` | striking a lightning bolt two cells from `anchor/arrival`, where every player first stands |
| `a-bolt-on-a-copper-tile` | `DW0959` | `build` | laying a waxed copper tile under the exit and striking a bolt on it, so the game would scrape copper along a random walk |
| `a-row-with-no-picture` | `DW0890` | `validate` | pointing a `design.json` row at a stem no file under `design/concept/` answers |
| `a-signal-the-floor-does-not-carry` | `DW0891` | `build` | declaring the east strip `shown_by` a cactus that stands in none of its cells |
| `a-sky-no-picture-shows` | `DW0890` | `validate` | moving the midnight row to `night`, leaving an hour the world reaches that no approved picture shows |
| `a-sky-that-restates-its-picture` | `DW0721` | `build` | stating on `hall-exterior` the `{"sun": "high"}`+`clear` sky of the very row it answers, in place of the sun just risen it states |
| `a-moon-named-under-the-noon-sun` | `DW0931` | `validate` | naming the moon's phase on the counter's `{"sun": "high"}` cut, where the moon stands at the nadir |
| `a-night-whose-moon-nobody-named` | `DW0931` | `validate` | deleting the phase from the world's new moon just risen, which the party sees from the first tick |
| `a-sky-with-two-bodies` | `DW0931` | `validate` | naming both the moon just risen and the sun setting as the world's one time |
| `a-phase-that-restates-the-world` | `DW0931` | `validate` | copying the world's `new-moon` onto the Curator's sunrise cut, which already keeps the world's moon |
| `a-ninth-phase` | `DW0100` | `validate` | naming the world's moon `blood-moon`, a phase the pinned game does not have |
| `nothing-places-the-whole` | `DW0883` | `validate` | deleting the entry box's pinned `min`, so nothing places the site plan |
| `two-faces-at-one-place` | `DW0880` | `build` | giving two recovery stakes that share a place two different marker items |
| `two-presses-on-one-cell` | `DW0878` | `build` | hanging an `interact` objective and a click trigger on one anchor |
| `a-gate-the-path-already-cleared` | `DW0879` | `validate` | clearing a counter between the beat that fills it and the gate that reads it |
| `two-bodies-on-one-mark` | `DW0896` | `build` | taking the page's offset away, so it is summoned onto the usher's own cell while the usher is still standing on it |
| `an-offset-out-of-the-room` | `DW0897` | `build` | writing the page's offset from the usher as forty cells instead of four, past the hall's east wall |
| `a-chestplate-on-a-horse` | `DW0898` | `validate` | putting a chestplate on the barded horse, which the server stores and the client never draws |
| `a-chest-that-is-not-there` | `DW0917` | `build` | hanging the trapped-chest trap on open floor, so the chest the party should open is empty air |
| `a-kill-nobody-can-be-credited-with` | `DW0913` | `validate` | paying for the kill of the usher, who is never unleashed and not `vulnerable`, so no player can ever be credited with killing him |
| `every-kill-on-a-fight-that-never-comes-back` | `DW0914` | `validate` | saying the bay's lone skeleton pays `every-kill`, when nothing ever seats it twice |
| `a-fight-that-comes-back-with-no-judgement` | `DW0915` | `validate` | removing `fires` from the muster's bundle, when every rest brings the muster back |
| `a-second-purse-on-the-one-sidebar` | `DW0919` | `validate` | asking the relics to stand on the sidebar beside the tokens, when the slot holds one objective |
| `a-party-purse-the-sidebar-cannot-draw` | `DW0919` | `validate` | moving the standing display onto the party's bounty, whose `#party` holder the sidebar hides |
| `an-archer-seated-beside-the-burning-corner` | `DW0922` | `build` | summoning the edge skeleton around the east bay, so its ring stands it in the near hall within its pursuit of the burning corner in the barrier pocket |
| `a-hatch-the-party-leaves-open` | `DW0923` | `build` | seating the muster at the hearth beside the terrace and laying a trapdoor shut over the west well: a player can open it and leave it open, and the muster then falls in |
| `a-sky-the-sun-does-not-obey` | `DW0928` | `validate` | holding the hall's sun in place: the overworld's day timeline overrides `visual/sun_angle` every tick, so the line would ship and do nothing |
| `an-attribute-the-game-never-heard-of` | `DW0928` | `validate` | spelling `visual/sky_colour`, which the pinned game does not register — the refusal names `visual/sky_color` |
| `a-colour-that-is-a-number` | `DW0928` | `validate` | writing the hall's sky colour as an integer where vanilla's data writes `#rrggbb` |
| `a-snow-at-summer-heat` | `DW0930` | `validate` | declaring snow in a climate at temperature 0.8, where vanilla rains |
| `an-atmosphere-nobody-stands-in` | `DW0930` | `validate` | declaring a third sky no place carries and no beat paints |
| `a-repaint-past-the-edge-of-the-world` | `DW0929` | `build` | widening the vantage's repaint four hundred blocks past every placed piece, into chunks nothing loads |
| `a-repaint-that-names-both-a-box-and-a-place` | `DW0929` | `validate` | giving the vantage's repaint both an anchor-centred region and a whole place |
| `a-landing-in-the-wall` | `DW0947` | `build` | moving the long gallery's landing two bays past its slab, so the view out of it runs through the end room's far wall into open air |
| `a-lamp-missing-from-one-bay` | `DW0946` | `build` | swapping the lantern in the bay the loop lands a body in for a soul lantern, so that bay is a different block and a different light from the one behind it |
| `a-light-round-the-corner` | `DW0946` | `build` | standing a lantern in the sealed cavity behind one bay's window, where no eye sees it but its light reaches one bay and not the next |
| `a-slab-too-thin-to-catch-a-fall` | `DW0945` | `build` | turning the loop on its side: a slab one cell thick in y under a drop, which a falling body passes between two polls |
| `a-crossing-that-moves-you-twice` | `DW0945` | `build` | drawing the slab seven courses thick while the landing stays one bay back, so a moved body is still in the slab and is moved again |
| `a-release-one-player-holds` | `DW0949` | `validate` | declaring the count the long gallery's release reads `player`-scoped, so one player is released and another still looped |
| `a-figure-in-the-hall` | `DW0948` | `build` | posting the hall moth in a bay of the long gallery, a body with an identity the move cannot repeat |
| `a-hall-nobody-releases` | `DW0311` | `build` | raising the count the long gallery waits for from 1 to 99, so the forced path never releases the loop and the route across its slab never opens |
| `a-floor-that-wakes-underfoot` | `DW0891` | `build` | raising the lidded pit's volume onto the lid, so the floor a body stands on before the beat is killed under it when the beat lands — the fourth shape, named in the configuration before the flip |
| `a-way-onward-through-a-pit-that-woke` | `DW0891` | `build` | moving the lidded pit's volume onto the counter the party walks to after the beat; its keep-out catches floor walked before the beat, so the visibility proof refuses it before the route proof would (`DW0510`) |
| `a-stage-with-no-term` | `DW0953` | `validate` | emptying the lidded pit's `when` to `{}` — a stage with no term |
| `a-stage-one-player-holds` | `DW0953` | `validate` | staging the lidded pit on the tokens, a datum each player holds for themselves |
| `a-wait-longer-than-two-minutes` | `DW0925` | `validate` | making a fallen player wait 121 seconds before rejoining, one past the two minutes a wait may last |
| `a-ladder-with-nothing-behind-it` | `DW0991` | `build` | turning the cabin ladder's four rungs to face into the cabin's wall, so each hangs on the open hall behind it |
| `a-shrieker-that-can-summon` | `DW0998` | `validate` | refilling the listening floor's shrieker with `can_summon=true`, a warden no wave declares |
| `a-sensor-caught-mid-click` | `DW0999` | `validate` | refilling the listening floor's sensor with `sculk_sensor_phase=active`, a click nobody made |
| `an-iron-door-beside-a-sensor` | `DW1000` | `build` | setting an iron door into the floor beside the listening floor's sensor, a door every footstep would open |
| `a-catalyst-over-the-floor` | `DW1001` | `build` | setting a sculk catalyst into the near hall's floor, under a cell the party walks |
| `a-watcher-for-a-class-nobody-plays` | `DW0996` | `validate` | setting the Warden to watch the nearest `class/pilgrim`, a class stage 3 does not declare |
| `a-watcher-nobody-can-reach` | `DW0997` | `build` | setting the destrier at the annex threshold to watch within one block, where no cell the party walks to stands |
| `a-beat-with-no-interval` | `DW0993` | `validate` | setting the slow heartbeat to beat `every: 0` ticks, the same tick again forever |
| `a-floor-as-loud-as-the-source` | `DW0993` | `validate` | setting the slow heartbeat's `floor` to 1.0, the farthest ear as loud as the source, a reach without end |
| `a-beat-one-player-hears` | `DW0993` | `validate` | gating the slow heartbeat on the tokens, a datum each player holds for themselves |
| `a-beat-heard-both-here-and-there` | `DW0929` | `validate` | giving the annex drip, heard in a `region`, a `place` as well |
| `a-beat-in-solid-rock` | `DW0994` | `build` | moving the annex drip's box into the stone canopy over the hall's east bay, where no body can stand |
| `a-beat-the-game-never-heard` | `DW0326` | `validate` | misspelling the slow heartbeat's sound `entity.warden.heartbeats`, an id the pinned game does not hold |
| `a-lever-hung-on-a-pane` | `DW1002` | `build` | hanging a lever on the north side of a glass pane under the near hall's roof: a pane's side is not the full face a wall-hung block asks for, so the server drops the lever |
| `gravel-left-over-air` | `DW1002` | `build` | writing a block of gravel three blocks over the near hall's floor: the cell under it is air when the world build has run, so the server lets it fall, and a write landing under a falling block turns it into an item |
| `a-trap-that-takes-the-case-button-floor` | `DW1002` | `build` | having the plate trap's payload clear the pedestal column the case's button stands on: a beat nobody has to play, and the fill clears the pedestal before the button, so the server drops it |
| `one-branch-takes-the-case-button-floor` | `DW1002` | `build` | having `obj/take-the-bone` clear the same column only once `flag/skipped-the-label` holds: a write only one branch makes, judged as every write is |
| `a-copyist-in-the-open` | `DW1025` | `build` | standing the copyist, who declares `watch: "none"`, one block off the north wall, so seven of the cells a player can talk to her from lie behind her — though no step of the route talks to her |
| `a-name-two-bodies-wear-at-once` | `DW1026` | `validate` | giving the factor the curator's name, so two bodies of one character stand in the hall from world init |

**A probe is the primary plus one declared edit.** It carries no copy of any
document the primary already holds; what it perturbs is written out in its own
`probe.json`, as a `patch` of JSON-pointer edits over the primary's canonical
form — so `peaceful-difficulty` is three lines:

```json
"patch": [
  { "doc": "world.json", "op": "replace", "path": "/content/difficulty", "value": "peaceful" }
]
```

The edits are `add`, `remove` and `replace`, applied in order by
`tools/ci/gallery_domain.py` while it materialises the point. Everything the probe
does not name comes from the primary on every run, which is what makes a probe
unable to drift from the campaign it perturbs. An edit whose pointer the primary
no longer has is a red naming the probe and the pointer, rather than a probe
quietly changing what it is about; `add` and `replace` are separate verbs so
that a key the primary GAINS is that same red rather than a silent overwrite.

A file that shadows a primary document is refused. A probe whose point is an
overlay says so with `"over": "<overlay>"`: the overlay is laid over the primary
first, exactly as its own point is, and the probe's edits apply to the result.
A `remove` at the empty pointer removes a whole document — the point is then the
primary without it (a campaign with no site plan). A probe may instead declare its edit against an
**overlay's** document, naming it by its path (`overlays/site-plan/site-plan.json`):
that document is brought into the point as the campaign's own and the edit is
applied to it, so the probe is the primary plus that document plus one edit and
ships no copy — a probe that both names an overlay document and ships a file of
the same name is refused.

A probe is an OVERLAY, not a campaign, so `delvec validate` pointed at a probe
directory refuses the directory (`DW0874`) rather than the document. Materialise
it over the primary first, exactly as the coverage gate does, and then run any of
them yourself:

```
python3 -c "import sys, pathlib; sys.path.insert(0, 'tools/ci'); import gallery_domain; \
  gallery_domain.materialise(pathlib.Path('probe-src'), pathlib.Path('gallery/probes/peaceful-difficulty'))"
target/release/delvec validate probe-src --prefabs gallery-prefabs
```

It exits 1, and the diagnostic it exits with is the one the probe's `probe.json`
names. For a probe the table marks `build`, run `delvec build probe-src -o out`
instead: `validate` accepts it, because what it declares is only wrong once the
anchors are resolved to cells.

They are not documentation of the refusals. They **are** the refusals: the
coverage gate runs each one and fails if the compiler ever starts accepting it,
because an exemption whose proof stopped holding is no longer an exemption.

## Building it

The piece, the mannequin skins and the texture images are generated by the engine's own generator, so
the whole thing builds from this repository alone — no content checkout:

```
mkdir -p gallery-prefabs
cargo run --release --manifest-path prefabs/gallery-generator/Cargo.toml \
  -- gallery-prefabs --skins gallery/skins --design gallery/design --textures gallery/textures \
  --terrain gallery/overlays/site-plan/terrain
cargo build --release -p delvec --bin delvec
target/release/delvec build gallery -o gallery-out --prefabs gallery-prefabs
```

Then the three gates:

```
python3 tools/ci/check-gallery-coverage.py --prefabs gallery-prefabs \
  --build-out gallery-out --index gallery-coverage.md
python3 tools/ci/check-gallery-render.py --prefabs gallery-prefabs \
  --build-out gallery-out --frames gallery-frames
python3 tools/ci/gallery-baseline.py --prefabs gallery-prefabs
```

The last one builds the domain twice — this tree, and the commit it branched
from (`merge-base(origin/main, HEAD)`) checked out under `.gallery-baseline/`
and built there from its own sources — and prints the delta between them; its
report lands in `.gallery-baseline/report/`.

`gallery-coverage.md` is the map from every declared surface to the place the
gallery writes it — and, for anything written nowhere, the word **nowhere**.
`gallery-frames/` is one picture per declared view, for eyes; no pixel is
committed or compared, because a renderer's bytes are not the same across
drivers and the manifests beside the frames are what a machine reads.

The suite the gallery generates runs on a real server as part of the `tier 2`
job — it is by far the largest any campaign here emits, because a template is
emitted per surface that has one.

## Adding to it

When a change lands something a campaign author can write, the same change adds
its element here. The coverage gate reds until it does, and it reds only then: a
change that adds no authoring surface leaves the unit set alone and cannot fire
it. The discharge is usually one field line.

Two rules keep it usable. **Keep it legible** — an element that binds a surface
and tells a reader nothing has failed half its job, so name things for their role
and put the explanation in `note`. And **keep it singular**: a second gallery is
two authorities on one question, and is refused in review. A setting that cannot
coexist with the primary becomes an overlay under `overlays/`, declaring exactly
what it reaches.

## The annex, and a library piece placed by name

`node/annex-wing` is a place whose program, `programs/annex-wing.json`, sets
four library tiles down by name with the grammar's `prefab` operation (spec-0106
§3.8): the entry tile, the bay tile twice, the cap. Which piece stands where is
the program's statement, never a seed's draw — a `prefab` reached through a
weighted choice is refused by the `prefab-placed-by-design` expansion gate — so
the annex is the gallery's one instance of a region holding library parts
beside authored work, and `delvec detail --all` writes it as **one piece**,
`prefab/gallery-annex-wing`, whose provenance row names every tile it holds.

Three facts about it are worth stating:

**A piece placed twice numbers its anchors.** The bay tile carries
`anchor/annex-first-bay`; placed twice it arrives as `anchor/annex-first-bay-1`
and `anchor/annex-first-bay-2`, and the drip and the second-bay trigger hang on
the second. The entry tile's `anchor/annex-threshold` carries `role: entry`, so
the place's `anchor/node-annex-wing` binds to it without the program marking
one.

**No seam joins it.** The annex, like the long hall, is a place the party is
carried into by a lever (`edge/to-the-annex`, a `carry`), so its contract
declares zero exterior edges and the light probe walks in from its entry anchor
(spec-0106 §3.4).

**The tiles' doorways are their own bytes.** Each tile is open on the faces where
the next one stands and declares no socket: how they join is the program's
design. A tile carved from one material renders as one material, so each wears a
`stone_bricks` panel around its doorway — a seam camera aimed down the annex sees
the doorways rather than one flat colour.

## The broken flight, and what a way costs to declare

The far half of the hall carries a **mezzanine**: a solid dais three courses
tall, with a stair up to it whose two tread courses are not there. It is the
only floor in the piece a body cannot walk onto, and a campaign puts the treads
back:

```json
{ "type": "open-way", "piece": "prefab/gallery-hall", "way": "broken-flight" }
```

That is the whole effect. **There is no region on it, no block and no
direction** — all three come from the piece's own `spatial_contract`, so the
beat and the building cannot disagree about what a way is. What the campaign
decides is *when*: this one fires when the party takes the muster's bone, and
the objective that asks them to stand on the mezzanine comes after it.

Three things are worth reading, because each is a place the claim could have
been empty instead of proved.

**The severance is proved on the bytes that ship, not asserted.** A way is an
opt-out from "reachable as built", and stranding supplies severance for free —
so the generator runs its own walk over the blocks it is about to write, twice:
once as shipped and once with the tread cells filled. The mezzanine must be
unreachable in the first and reachable in the second, or nothing is written at
all. It prints both counts (`390 stance(s) reachable shut, 406 laid`), so a
claim that stopped binding says so rather than going quiet.

**The way lives on the traversal edge, not on the effect.** `broken-flight` is
a field of the `stair` edge from `far-hall` to `loft` in the piece's contract,
confined to that edge's own transit volume. An unconfined way region would be a
licence to write anything anywhere at delve time, which is why the generator
asserts the tread cells lie inside the flight's `via` box before it exports
them.

**The three gate fields are not decoration.** The effect carries
`requires_flags`, `forbids_flags` and `requires_state`, and all three reach the
emitted command:

```
execute if score #party dw.f_muster_cleared matches 1
        unless score #party dw.f_hall_sealed matches 1
        if score #party dw.s_labels_read matches 0.. run fill 19 65 22 20 65 22 minecraft:stone_bricks
```

Each is a condition that holds wherever this beat can fire — the flag the
objective itself required, the flag the quest sets only afterwards, a count
that starts at zero and never falls. A gate that could be false here would be a
gate the completability proof credits and the delve does not honour, because
forcedness is decided at the effect's ROOT and not by its conditions.

The build publishes the ledger at `validation/ways.json`: one way staged across
one piece of four, four cells behind it, opened by a forced beat at critical-path
step 6, and twelve required elements examined against it.

**The treads are tested where every walk onto the loft steps.** Taking the bone
also sets `flag/flight-relaid`, and `trigger/test-the-treads` — an `approach`
within 2 of `anchor/flight-head`, the mezzanine's south edge where the flight
comes out — sets `flag/treads-held` and says so on the actionbar.
`obj/climb-the-loft` reads that flag, so the critical path performs the trigger,
and `DW1044` judges it: every walk from the bone to the loft passes the head of
the flight, so the build prints `1 judged …, 1 entered by every walk` and writes
the same counts to `validation/threshold-gate.json`. At range 1 the build is
refused: a walk exists that never stands where the selector is sure to catch a
body, and the message names range 2 as the least range every walk enters. The coverage gate reds if that ledger ever judges no
trigger; the refusal side is `probes/a-threshold-the-party-walks-round`.

## The barrier pocket

In the near hall's north-east corner is a pocket whose only way in is a
**full-cube course** set into a 1.5-tall wall line. A body walking there steps up
onto the course and down the far side — crossing a line the same line refuses to
let it walk through, which is exactly what `DW0453` names.

Three bodies walk it, and each declaration is *paid for* because it changes a
verdict rather than restating one:

| Body | Derived | Declared | What the declaration does |
| --- | --- | --- | --- |
| `npc/warden` | ground | `flier` | waives the advisory the crossing earns |
| `npc/marshal` | ground | `climber` | waives it likewise |
| `actor/rafter-spider` | **climber** | `ground` | **tightens** — binds a body back to the surmount rule its species would have been excused from |

The spider is the one worth reading twice. A declaration that restates the
species is refused (`DW0454`), so `ground` cannot be written on a ground mob —
but on a derived climber it is not a restatement, it is a claim the build holds
the body to. None of them carries a skin, because a skinned body is a mannequin
and a mannequin is derived ground.

The build publishes the ledger: three bodies, **three exercised**, two advisories
waived, one of each class.

**The control, and it is the half that makes the greens mean anything.** Change
the course from stone to air and it becomes an ordinary doorway: the crossing
earns no advisory, all three declarations change no verdict, and `DW0454` refuses
them by name. Reproduce it in one edit —
`gallery/world-edits.json` → `batch/lay-the-barrier` → `region/barrier-course`.

The pocket sits off the critical path on purpose. Blocking geometry on the route
makes the build's render plan and the one `delvec snapshot` derives disagree —
see below.

## The still ones

Every NPC a player can talk to watches the nearest player within eight blocks unless it declares otherwise, so `npc/curator` and `npc/factor`, which declare nothing, watch by default (the build's `watch binding:` line counts them `by default`). Three bodies declare a watch (spec-0101). `npc/warden`, the gatekeeper standing on
`anchor/warden`, a plain villager body, turns to face the nearest player within
eight blocks (`watch: {who: "nearest", within: 8}`); `actor/standard-bearer`, the
skinned mannequin at `anchor/march`, turns only to a player of the warder's class
within six (`watch: {who: {class: "class/warder"}, within: 6}`). Between them they
write both body classes, both forms of `who`, the mob and the mannequin summon,
and the yield on both walk drivers: the Warden goes over the barrier line to
`anchor/pocket` and the bearer carries the colours up to `anchor/vantage`, and
while each walks, the walk owns its facing; on the tick after it arrives the
watch has it back. `npc/marshal` watches because the party talks to him: his
muster faces south and the route brings the party to him from the north, so
without `watch: {who: "nearest", within: 4}` the party would walk up to his back
(`DW1025`; the build's `approach binding:` line counts him as drawn by a watch).

One body declares that it never turns. `npc/copyist` stands at the north wall
(`anchor/usher` and `offset: [0, 0, -4]`), bent over her page, with
`watch: "none"`: she holds the usher's south facing, and because a body that
does not turn is judged from every cell a player can talk to it from, not only
from the route — and no step of the route talks to her — the build's
`held-facing binding (DW1025):` line reads 25 cells judged over her one stance,
none behind her: the wall is at her back. Taking the declaration away makes her
watch by default, and her body gains the watch tag and a line in `watch_tick`.

The build says what it proved on its `watch binding:` line — the watchers
declared, the walked cells they were judged over, how many are drawable,
refused and unobservable — and writes
`validation/watchers.json`, the cell each generated PackTest stands its dummy on:
the walked cell in reach whose bearing differs most from the body's home facing.
`watch_npc_warden` and `watch_actor_standard_bearer` turn the body by its
`watch_tick` and see it hold that facing once the dummy is out of reach — the
bearer's first sees the dummy draw nothing until it wears the class — and then
lift body and dummy above the build limit to see one real `tick` reach the watch:
the bearer's own cell is inside `trigger/glimpse-the-marshal`'s approach, whose
cutscene takes every player out of the watch.
`watch_yield_*` sees each walk take the watch and give it back. The bot asks the
server, at every waypoint within reach, whether each body faces it, and prints
`[watch] …` with what it reached.

The probes are the two ways a watch is refused:
`probes/a-watcher-for-a-class-nobody-plays` (`DW0996`) and
`probes/a-watcher-nobody-can-reach` (`DW0997`); and a body the party talks to
is refused for facing away by `probes/a-copyist-in-the-open` (a held facing,
from every cell) (`DW1025`). The route's walk to the marshal arrives side-on,
through the main gate in the west corner, so narrowing his watch to one class
leaves him facing 90° off the approach, which `DW1025` admits; the route has
no talk beat that a narrowed watch turns its back on.

## The heartbeat, and the drip

Three sounds beat over places (spec-0102). Under the hearth, one cell below
`anchor/hearth`, a warden's heartbeat sounds over the whole near hall
(`place: node/hall`, `floor: 0.4`). `pulse/the-heart` beats every 30 ticks
from `obj/hear-the-curator`, which sets `flag/lid-fell`, until
`obj/board-the-ferry` sets `flag/boarded`; `pulse/the-heart-racing`, the same
sound from the same cell at `every: 14` and `pitch: 1.2`, beats from that flag
on — a quickening is a second pulse on one source with a disjoint gate. The
third, `pulse/the-drip`, is a dripstone drip in the annex's second bay, heard
in a `region` and ungated, so it beats from world load. Between them they
write every field: both spellings of where a pulse is heard, a gate and no
gate, a namespaced sound and a bare one, an offset mark and a plain one.

The build says what it derived on its `pulse binding:` line — the farthest
standing ear, the range and the volume each `floor` asks for, how many pulses
are live on the forced route, how many have a listening station — and writes
`validation/pulses.json`, each pulse's derived numbers and its stations. The
forced route never walks into the annex's second bay, so the drip has no
station: the build says so with the `DW0995` advisory, naming the bay's box,
and the bot reports it `not_heard: no station` — a beat the party need never
hear is a design, stated, never passed. PackTest drives each heartbeat's gate
open and sees its chain start (`pulse_the_heart`, `pulse_the_heart_racing`),
shuts it by one term and sees the chain stop (`…_shut`), and sees world setup
reach the drip (`pulse_the_drip`). The bot stands `2 · every + 10` ticks at the
slow heartbeat's listening station in the hall and at its silent station down
the long hall, and at the racing heartbeat's listening station on the ferry
deck, and prints `[pulse] …` with the beats it heard and
`pulse binding: H of N pulse(s) heard at a listening station, S silent at a
silent station`.

The probes are the ways a pulse is refused: `probes/a-beat-with-no-interval`,
`probes/a-floor-as-loud-as-the-source` and `probes/a-beat-one-player-hears`
(`DW0993`), `probes/a-beat-heard-both-here-and-there` (`DW0929`),
`probes/a-beat-in-solid-rock` (`DW0994`) and
`probes/a-beat-the-game-never-heard` (`DW0326`).

## The ferry, and what a link is

In the far hall's west corner is a **sealed cabin**: three walls and a roof,
no door, no gap. The beat `obj/cross-the-strait` stands inside it, and the only
way in is the ferry. The party boards the deck
(`obj/board-the-ferry`, which sets `flag/boarded`) and pulls the tiller:

```json
{ "id": "trigger/ferry-tiller", "at": "anchor/ferry-tiller", "on": { "on": "use" },
  "once": false, "requires_flags": ["flag/boarded"],
  "effects": [{ "type": "sequence", "steps": [
    { "at_ticks": 23, "effects": [{ "type": "cutscene", "seconds": 1, "path": [ … ] }] },
    { "at_ticks": 22, "effects": [{ "type": "teleport",
        "from": { "anchor": "anchor/ferry-deck", "extent": [1, 1, 1] },
        "to": { "anchor": "anchor/ferry-landing" } }] } ] }] }
```

A `teleport` in a trigger declared `once: false` is a **link**: a carry the
route proof takes where a walk fails, because a straggler left on the deck can
pull the tiller again and follow. The build splices the pull into the path —
`critical-path.json` carries a `trigger` step with `stand` (the deck cell the
tiller is pulled from, inside the volume) and `transport` (the landing) — and
the `DW0311 binding:` line counts the leg as carried by a link. The hall's
other teleport, on `obj/take-the-bone`'s completion, is a **gather**: whoever is
in the march's box travels, once, and no proof leans on it.

The teleport fires first and the cutscene plays at the landing one tick
later. A cutscene's end puts every player on the cell the presser stood on, so
a carry under the open bracket is undone (`a-crossing-the-cutscene-undoes`),
and a carry after it takes everyone or no one — which is refused wherever the
trigger can be pulled from outside the volume, as this tiller can from the open
hall floor (`a-tiller-pulled-from-the-hall-floor`). The deck and the landing
are both stations of the hall, so the crossing is a move inside one place and
owes the layout graph no edge.

**The landing completes the beat.** The ferry sets the party down inside
`obj/cross-the-strait`'s completion volume, so the server completes it on
arrival, during the tiller's step. `critical-path.json` keeps the reach step and
marks it `completed_on_landing`: the bot walks nothing for it and asserts the
marker already came.

**And a second link takes the party back out.** A sealed room nothing leaves
would end every walk there — the bot's death loop starts where the path ends,
and no lethal volume is reachable from inside the cabin. So the ferryman's chest
carries `trigger/cabin-tiller`, `once: false`, whose teleport moves whoever
stands in the cabin onto the exit. `obj/reach-the-end` is the
last beat and comes after the crossing, and the return lands inside its volume
too, so the final step of the path is a reach a landing completes — the shape
whose campaign marker is due at the carrying step. `validation/teleport-gate.json`
reads two links and one gather.

Five probes show what the engine refuses about it:
`a-way-onward-that-fires-once` (the teleport on the cabin beat's own completion
— a gather, `DW0311` naming it and prescribing the link), `a-lever-outside-its-own-boat`
(the deck shrunk so no cell inside it reaches the tiller, `DW0932`),
`a-tiller-pulled-from-the-hall-floor` (the cutscene moved before the carry, so the
carry takes everyone or no one and the tiller can be pulled from the open floor,
`DW0932`), `a-crossing-the-cutscene-undoes` (the teleport inside the cutscene,
`DW0933`), and `a-carry-the-graph-never-drew` refuses the hall's lever to the
annex once its `carry` edge is removed (`DW0934`).

**Who is in the shot** (spec-0095). The ferry's crossing and the first muster
cutscene leave a stand-in for every player where they stood, wearing their own
skin and gear: the party is in its own cutscenes by default, and the ferry
spells the default out (`"party": "present"`). The second muster
cutscene, the parade shot style by style, declares `"party": "absent"`: the
scene is the muster's, and its functions are `cs_muster_3_2_12687a8f_absent`
with no stand-in line. `validation/stand-in-gate.json` reads two present and
one absent (`DW0971`).

## The long gallery

On the far hall's roof, up a stair at the hall's east end, is a corridor of three
identical bays, each with two staggered baffles that close the view inside the bay: `loop/long-gallery` (spec-0086). Its slab
crosses the gallery at bay 2's mouth and moves every body that crosses it one bay
back, so the gallery goes on. It holds once `hall-open` is set and until
`hall-sealed`, and at most until `state/gallery-crossings` passes 1: the first
crossing counts 1 and lights a second lamp in every bay and the end room at once (`on_cross`, keyed by
`when`), the second counts 2 and the gate shuts, and the same plane is floor.
`obj/walk-the-long-gallery` waits at the gallery's far end, so the critical path
carries a `loop` step that crosses twice.

The build publishes `validation/loop-gate.json`. Its row for this loop holds 24
eyes (the landing's standing eyes and the catch band, where a walking body is
when the poll finds it), a span of 325 cells, 155 visible cells, all of them in
the near field and compared as blocks and as light at two skies over 10
configurations, no far differences, and one exercise step. The eight probes above
whose names start with a landing, a lamp, a light, a slab, a crossing, a release,
a figure and a hall are each this gallery plus one edit, and each is refused by
the rule that edit breaks.

## The long hall

In a place of its own, `node/long-hall` (bound by `--piece` to `prefab/gallery-long-hall`), stands one
straight hall of identical 6-block bays with no fog. A soul lantern and a pillar
pair mark every bay, and a glowstone lintel lights the exit into a room at the
far end. `loop/the-hall-that-runs-on` stands across one bay's mouth and returns
a crossing body one bay back, so the lit exit, 54 blocks past the slab, stays
where it was (spec-0090). The gallery's long gallery closes its view inside one
bay and sees no far field; this hall is where the far field binds. Past the near
field of a 6-block jump (10.6 blocks), its cells may differ from their images,
and the build measures how far the jump moves each one on screen against the
1.2852° threshold. The binding line states the count of far differences admitted
and the largest shift (0.7574°).

The route runs through it. `quest/long-hall` plays between the annex and the
hall's far half: the annex's cap lever carries the party to the long hall's
porch (`edge/to-the-long-hall`, a `carry` the lever's link realises), the porch
sets the checkpoint on arrival (`obj/arrive-at-the-porch`), the party runs the
hall until the loop has turned it back twice and reaches the lit room
(`obj/reach-the-hall-end`), the quest's completion moves the seat to the hall's
landing, and the end room's lever carries the party back to that landing
(`edge/out-of-the-long-hall`). No seam joins the long hall to anything, so its
contract declares zero exterior edges (spec-0106 §3.4). The checkpoint and stake
proofs walk a respawned body the way the party is carried — through the levers'
links and the loop's slab (`nav::reachable_carried`) — and the stake proof
partitions the map by the site plan's places.

Two probes show what the far field refuses: `an-exit-too-near-to-hide` (a wall
across the hall 36 blocks past the slab, so the hall ends in view too near for a
6-block jump to hide; `DW0947`, raised at the world-edits replay, which prints
its loop binding first) and `a-figure-down-the-long-hall` (the hall moth posted 16
blocks past the slab, past the near field but moved too far by the jump;
`DW0947`).

## The fight, and the floor it needs

The far hall carries the one mandatory encounter — the muster, billed `elite`.
The ladder reads its bodies against the declaration and then stages them away; it
grades no fight. What still has to hold is that the bodies the probe reads are
the wave's own, so three facts about where things stand are load-bearing rather
than decorative.

**A killing volume does not share a room with a fight.** A lethal volume emits
`@e[x=…,dx=…,…]`, and a selector takes any entity whose bounding box touches
the region — a spider is 1.4 blocks wide, so it is inside a box while its feet
are still 0.7 blocks outside it, and the cell immediately beside a volume is
fatal to anything standing at that cell's near edge. Distance does not answer
this, because a fight does not stay where it is staged: the ladder's own census
found a re-seated cohort spread 14.4 blocks from its anchor. So both killing
volumes live in the NEAR hall — the west corner and the north wall — and the
muster forms up beyond the dividing wall, in the far hall the quest's beat
names — the Marshal's answer, so no body of it stands while the party walks to
her. The wall is the guarantee; the generator checks the sides, not a radius.

**The pits sit where nothing walks.** Each is four blocks clear of every anchor
and of every cell the piece's own bodies are teleported through on their way to
the pocket, and neither touches a gate mouth. Both are three cells square,
because a volume one cell deep has no interior — the death loop walks a body
to a cell inside the box, and every cell inside it was also its edge.

**The muster keeps two cells of air on every side.** A wave takes the standable
cells NEAREST its anchor, and standable is judged for a body one cell wide, so
an anchor with a wall two cells off seats the 1.4-wide member of the stack
against that wall: the census then reads a mob below full health before the
fight has started, and re-seat fidelity reds on a wave nothing has touched.
Staged one cell from the mezzanine's corner, this wave came back `1 of 3` below
full on both scripted deaths; in the open middle of the hall it comes back
whole. The same two cells keep the fight out of the loft's own completion box,
which stands three courses up and one cell wide along the dais's west face —
a jump inside it finishes a later objective during this one.

**The patrol comes down the hall as the party leaves it.** A lane squad's
`follow_range` is its lane's `aggro_radius` verbatim — eight — and `DW0477`
records in writing that nothing measures `wave/lane`. Eight blocks of
perception in a hall fourteen deep covers the hall, so there is no line the
patrol can walk that the party's route stays clear of: moved to the back wall,
the crossbows stopped reaching the muster and killed the bot on its way to the
finale instead. A fight the campaign does not bill and does not require is
therefore armed by the hall's own closing beat rather than by the beat that
opens it, and the back wall is where it walks when it arrives. Both halves are
held: the arming is a beat, and the generator holds the geometry, because an
ordering is the half a later edit can undo without moving a coordinate.

The generator holds the geometry of all of it, on every run, and prints what it examined:

```
gallery-hall: muster clearance bound — 3 killing volume(s), all across the wall at z=15 from the muster: anchor/west-pit 20.62, anchor/lid-pit 17.80, anchor/east-pit 22.80 (floor 8.0 block(s))
gallery-hall: lane clearance bound — the patrol line passes `anchor/muster` at [15, 1, 29], 10.00 block(s) away (floor 10.0)
```

## Open findings

**The build's render plan and `snapshot`'s disagree when an edit blocks the
route.** One is computed after world-edits and the other before them, so solid
geometry on the critical path makes the build's legs longer than the ones
`snapshot` can resolve, and declared views become unproducible by name. The
gallery keeps its blocking geometry (the shards, the barrier course) off the
route; the divergence itself has no diagnostic.

**An objective can be gated on a state an earlier forced beat clears, and
nothing says so.** `obj/reach-the-end` requires `state/labels-read` `at-least`
1; `obj/clear-the-muster`, four objectives ahead of it on the mandatory spine,
clears that state to zero. Every static proof passes — the reachability model
walks objectives and flags, not the arithmetic of the states their `requires_state`
compares — and the delve is unfinishable: the bot stands inside the finale's
own completion box and no marker arrives.
`DW0527` sees the same write and says nothing about this, because it is a rule
about one bundle and these are two. The gallery lives with it by setting the
count again on the bone's beat, LAST in its bundle so no reader follows the
write; the general form has no diagnostic.

**Nothing refuses a wave seated inside a killing volume.** `DW0511` is the
rule that a body put somewhere by DECLARATION rather than by walking must not
be put inside a lethal volume — no route proof can see it — and
`lethal::posted_places` enumerates the entry cell, every checkpoint, every
NPC, every per-quest `cast` placement and every actor. A wave's seated cells
are not in that list, so a muster standing between two pits compiles green and
loses bodies to the world at runtime. The clearances above are the strongest
form a piece generator can carry, and they are weaker than the diagnostic
would be twice over: the generator cannot see the campaign's own `extent`
values, so its constants state that half as a premise, and it compares anchor
to anchor where the real question is hitbox to box.

## Why the jobs gate

The gallery's four jobs — pieces, coverage, views, baseline — are each a required status check. `tools/ci/check-required-contexts.py`
holds the manifest and `ci.yml` in lockstep.
