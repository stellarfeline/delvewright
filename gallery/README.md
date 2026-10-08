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
| `npcs.json` | four speaking parts — a quest-giver, a gatekeeper, a counter, a drill officer; the quest-giver is a mannequin that hides its cape layer, and the standard bearer in `quests.json` hides the other six, so every overlay layer a mannequin can hide is written once (spec-0097) |
| `classes.json` | two kits, one carrying a flask (what a bonfire rest refills) |
| `quest-plan.json` | three quests and the branch point the fork opens |
| `quests.json` | the bulk: objectives, effects, waves, actors (a barded and saddled horse among them), traps, triggers, a shop, a shortcut, a stake, a timed gate, three killing volumes — one of them the pit under the terrace annex, live from the beat that clears the lid over it (spec-0088) — and six named datums of which one — the tokens — stands on the sidebar (`display: sidebar`, spec-0076); the hearth's two buttons, like every shop offer and some dialogue options, hover a tooltip saying what pressing them does (spec-0078) |
| `dialogue.json` | one tree per NPC; the Curator's carries the fork, and the Marshal's carries the two scenes it leads to — a pair of nodes no option leads to, reached only because the quest's `cast` ledger opens one of them per branch |
| `world-edits.json` | the annex's piece-verb batches, the shards stamped by `fragment`, the barrier course, and the batches that dress the floor, lay the hearth, open the vault and rough the lane |
| `geometry-brief.json` | four numbers out of the hall's own brief, the kind a site plan is later held to |
| `layout-graph.json` | the same hall stated as six places and twelve connections, before any coordinate — three barred doors through the wall because the hall really has three, a stair and a drop that close a loop, a sightline to the loft, and one place deliberately off the mandatory spine |
| `overlays/site-plan/` | the same places given geometry, and then a whole map DERIVED from it: a region, a box each stating only its extent and plane, ONE pinned corner, a seam per connection stating which face of its `a` box it sits on and where along it (the compiler derives every other corner and every sill — spec-0059; `delvec validate` prints the corners), the rock and the sky the whole owns, and eight comparisons holding all of it to its own written brief. It carries its own world, cast, quest layer and translations, because a campaign has ONE placement authority and the primary's is `areas[]` — so at this point of the campaign nothing describes a block, and everything a body meets is derived: the floors it walks, the doors it is stopped by, the stair it climbs, the anchors the quests bind to |
| `overlays/valley-site/` | a SITE placed: one area bound to one prefab on `horizon: valley`, which is the other way a campaign states how big its map is — the map IS that piece, so its declared region is the extent the surround rings. `gallery-bank` is what a site is: three courses of island mass, a bank of grass out to the box's own edge, a walled court on top. It is seated by its WALK PLANE, so the bank's top course is the valley's gap floor and a body walks off the piece onto ground the horizon built; and it carries both halves of `DW0885` at once, the courses under the bank buried by earth and the parapet above it answered by four `shown_faces`. It carries its own cast and quest layer, because its one area is not the primary's two |
| `l10n/zh-cn.json` | the second language, so the sidecar surface is real rather than declared |
| `render-plan.json` | the view set the gallery declares, so a shot that vanishes is a red |
| `area/annex` (in `world.json`) | a three-tile chain assembled from `pool/gallery-annex` — what binds the piece verbs |
| `overlays/` | parameter points — settings that take one value per world |
| `probes/` | documents the engine **refuses**, each naming the diagnostic |
| `baseline/` | the committed emission index, the expected-warnings ledger, and the review delta — which names the commit it was measured from, so it can be recomputed rather than believed |

The hall itself is generated, not committed: one 31 × 8 × 31 stone room split by
a barred wall, with three doors in it and a mezzanine in the far half whose
stair is missing its treads. Beside it the generator emits a small
**annex tileset** — four 7 × 6 × 7 boxes and a pool — plus a 3-cube **shard**
that exists only to be stamped by `fragment`. The annex is deliberately plain:
its job is the ASSEMBLY, and a tileset with interesting rooms would make the
placement harder to read without binding one more unit. Each tile carries one
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
| `two-placement-authorities` | `DW0839` | `validate` | carrying `areas[]` and a site plan at once |
| `aquatic-locomotion` | `DW0455` | `validate` | declaring a body that swims |
| `peaceful-difficulty` | `DW0468` | `validate` | setting the world to peaceful |
| `sound-at-actor` | `DW0335` | `validate` | playing a sound from an actor's position |
| `a-piece-the-library-does-not-hold` | `DW0856` | `validate` | binding the hall to a piece whose name is one letter wrong |
| `a-gate-two-areas-provide` | `DW0857` | `validate` | binding the annex to the hall's own piece, so both areas provide one gate anchor |
| `a-question-nobody-answers` | `DW0858` | `validate` | asking the party to talk to somebody the same quest declares silent |
| `a-walk-of-a-different-whole` | `DW0841` | `validate` | detailing against a walk record of some other map |
| `detail-without-a-walk` | `DW0841` | `validate` | detailing a place before the whole has been walked |
| `a-piece-that-is-not-its-frame` | `DW0843` | `validate` | seating a piece that does not fill the box the map gave it |
| `a-horizon-with-no-map` | `DW0855` | `validate` | declaring a horizon with nothing for it to ring |
| `a-clock-nobody-explained` | `DW0860` | `validate` | arming a stealth clock that bites before its own instruction can be read |
| `a-barrel-in-a-row-of-barrels` | `DW0861` | `validate` | adopting a container the piece placed without naming what is in it |
| `a-prompt-nobody-sees` | `DW0862` | `validate` | writing a hint on an objective with no title to carry it |
| `a-fight-nobody-points-at` | `DW0863` | `validate` | requiring a fight and saying nothing about where it happens |
| `a-bar-in-a-colour-the-game-lacks` | `DW0911` | `validate` | drawing a boss bar in `crimson`, a colour the pinned game's `bossbar` command does not list |
| `a-bar-over-a-body-nothing-can-hurt` | `DW0909` | `validate` | hanging a health bar over the usher, a body nothing unleashes and nothing can hurt |
| `a-bar-with-nothing-to-draw` | `DW0910` | `validate` | removing the muster's bar `title`, when the wave fields two kinds of body and no one name is the fight's |
| `a-body-two-areas-answer-for` | `DW0884` | `validate` | casting the curator at `anchor/lectern` when the annex is bound to the hall's piece and both areas answer to that name |
| `a-face-nothing-stands-in-front-of` | `DW0886` | `validate` | placing the open-topped yard as its own area under `horizon: void`, with sides nothing buries and nothing declares shown |
| `a-floor-carved-down-to-the-sea` | `DW0344` | `build` | a world edit cutting a three-by-three hole in the hall's plinth course down to the sea's own plane, under `horizon: ocean` |
| `a-gate-the-party-walks-back-through-after-it-is-sealed` | `DW0485` | `validate` | giving the last mainline beat the main gate as its subject, three beats after that gate is sealed |
| `a-strand-walked-after-the-gate-it-crosses-is-sealed` | `DW0485` | `validate` | giving the optional reliquary's first beat the main gate as its subject: the exported path walks it before the seal, and a player may walk it after |
| `a-key-that-charges-more-than-it-asks` | `DW0901` | `validate` | charging five tokens at the counter that gates on four |
| `a-lane-beside-a-killing-volume` | `DW0891` | `build` | setting the west pit on the near hall's floor beside the chest, so it kills the walked ring around it |
| `a-number-this-engine-does-not-implement` | `DW0102` | `validate` | declaring a `dsl_version` other than the one this engine accepts |
| `a-picture-nobody-looks-at` | `DW0900` | `build` | re-aiming the only camera that answers `concept/morning-quay` at a picture two other cameras already answer |
| `a-picture-nobody-recorded` | `DW0890` | `validate` | deleting the night row from `design.json` while its image stays under `design/concept/` |
| `a-piece-with-no-walk-plane` | `DW0886` | `validate` | seating the shard, which declares no walk plane, on an `ocean` horizon |
| `a-place-cut-below-the-sea` | `DW0886` | `validate` | standing a site-plan place two courses under the sea plane, under `horizon: ocean` |
| `a-pool-of-two-walk-planes` | `DW0886` | `validate` | seating a pool whose members declare two different walk planes on the base that derives an origin from it |
| `a-program-asking-for-a-seam-the-whole-does-not-hand` | `DW0882` | `detail` | declaring a `handed/` parameter for a seam the allocation does not hand |
| `a-program-that-marks-no-place-to-stand` | `DW0845` | `detail` | removing the program's one `mark`, so nothing answers the anchor the quests bound to the place |
| `a-program-whose-arch-misses-its-seam` | `DW0844` | `detail` | carving the annex arch one cell along the wall from the seam the plan handed it |
| `a-reach-that-completes-from-the-floor-below` | `DW0881` | `build` | widening the loft's completion radius to 2, so it completes from the hall floor three courses below |
| `a-record-that-says-nobody-walked` | `DW0841` | `validate` | detailing against a fresh walk record whose verdict says nobody walked |
| `a-rim-one-radius-out-of-reach` | `DW0850` | `build` | narrowing the well's completion radius to 2, so no walked cell on the rim reaches it |
| `a-rocket-under-a-roof` | `DW0899` | `build` | firing a rocket at `anchor/exit`, under the hall's stone ceiling |
| `a-row-with-no-picture` | `DW0890` | `validate` | pointing a `design.json` row at a stem no file under `design/concept/` answers |
| `a-signal-the-floor-does-not-carry` | `DW0891` | `build` | declaring the east strip `shown_by` a cactus that stands in none of its cells |
| `a-sky-no-picture-shows` | `DW0890` | `validate` | moving the midnight row to `night`, leaving an hour the world reaches that no approved picture shows |
| `a-sky-that-restates-its-picture` | `DW0721` | `build` | stating on `hall-exterior` the `{"sun": "high"}`+`clear` sky of the very row it answers, in place of the sun just risen it states |
| `a-moon-named-under-the-noon-sun` | `DW0931` | `validate` | naming the moon's phase on the counter's `{"sun": "high"}` cut, where the moon stands at the nadir |
| `a-night-whose-moon-nobody-named` | `DW0931` | `validate` | deleting the phase from the world's new moon just risen, which the party sees from the first tick |
| `a-sky-with-two-bodies` | `DW0931` | `validate` | naming both the moon just risen and the sun setting as the world's one time |
| `a-phase-that-restates-the-world` | `DW0931` | `validate` | copying the world's `new-moon` onto the Curator's sunrise cut, which already keeps the world's moon |
| `a-ninth-phase` | `DW0100` | `validate` | naming the world's moon `blood-moon`, a phase the pinned game does not have |
| `a-walk-plane-the-void-still-owes` | `DW0886` | `validate` | seating the shard, which declares no walk plane, on a `void` horizon |
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

A probe may ship a whole document, and several do: `site-plan.json`,
`detail-plan.json`, `walk-record.json` and a `programs/` directory are documents
the primary cannot carry at all — `DW0839` refuses a campaign holding both `areas[]` and a site plan — so
there is nothing for them to be a copy of. A file that shadows a primary
document is refused. A probe may instead declare its edit against an
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
  -- gallery-prefabs --skins gallery/skins --design gallery/design --textures gallery/textures
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

## The annex, and what a socket is worth looking at

`area/annex` is a three-tile chain assembled from `pool/gallery-annex`. It is
what binds the piece verbs — `insert-piece`, `swap-piece`, `remove-piece`,
`reseed-piece`, `rewire-socket` and `fragment` — none of which any campaign or
fixture in this repository had ever written.

Three facts about it are worth stating, because each was a place a camera came
back with nothing:

**An unmated socket is a wall, not a hole.** `solver::seal_layout` fills every
unmated connector's opening with `minecraft:stone_bricks` and clears it to air
only when the socket mates, so the 3 × 3 opening a tile carves exists in the
world exactly when something is on the other side of it. A tile can therefore
carry both an anchor and a spare socket: reachable floor beside a sealed socket
borders a wall. That is what lets every tile declare the anchor its views frame
while the chain still keeps the spare socket `insert-piece` hangs a tile off.

**`rewire-socket open` is only writable where the far side is not the void.**
The verb clears an *unmated* socket's seal — a mated one is already an open
passage and is refused as a no-op — so the cell one step past the opening lies
outside every placed tile. In a world with nothing outside, that is a bottomless
column and `DW0322` refuses it, correctly: a way out onto nothing is a fall
nobody survives. Nor can the campaign lay ground there, because every massing
batch precedes every detailing batch (`DW0162`), so the opened socket already
exists when the first batch's invariants are re-proved. The one unmated socket
whose far side is *not* the void is the one a `rewire-socket sealed` just
severed, because the partner's own plane stays walled — which is what
`batch/annex-seal-a-way` and `batch/annex-open-a-way` do, in that order: a
doorway bricked up on the far side and open on the near one.

**The chain is what binds the piece-mating check.** A socket is one of the two
places a prefab document says what a side of it is — the other is
`spatial_contract.faces`, which the hall carries and no annex tile does — and
`DW0780` reads both, so the chain's three tiles, mated end to end with four
socket faces meeting across two seams, are examined rather than passed over. The
build's binding line states it on every run, and the count is a fraction of the
placement rather than of the declarations, so a world whose pieces stop touching
reads as a zero rather than as a silence. What no campaign document here can
reach is a `DW0780` refusal itself: areas stand `AREA_SPACING` apart and the
solver computes every seated position, so an authored document has no surface on
which to move a piece off its seam. The refusal is a self-check over the solver's
own layout, and its perturbations live in `crates/delvec/tests/face_contract.rs`
— a mated piece detached by one block, and a pair that touches and declares
nothing across the plane it touches on.

**A tile carved from one material renders as one material.** Each tile wears a
`stone_bricks` panel around its socket openings and a `stone_bricks` floor under
its stone walls, because a seam camera aimed down a corridor of nothing but
`minecraft:stone` came back a rectangle of ONE distinct colour — which the render
arm reports, correctly, as a frame that shows no scene at all. The panel also
means a sealed socket reads as a bricked-up doorway rather than as a patch of the
wrong wall, the seal material being the same brick.

**The pool repeats a variant, and says so.** With two connector variants and two
filler slots the draw may seat one of them twice, which makes every anchor that
prefab declares ambiguous — `DW0498`, advisory, in the expected-warnings ledger.
The alternative it names is more distinct variants, and that is a choice about
the pool rather than about the seed.

**And the annex says what it does NOT guarantee.** `area/annex` seats three
pieces of a four-member pool, and the layout is only ever obliged to seat one of
them: the `entry` member, at the area origin. Everything else arrives because
the campaign required an anchor that piece carries, or because the filler draw
picked it. So the annex guarantees exactly `anchor/annex-threshold` — one of the
four names its pieces declare between them — and `trigger/stand-in-the-second-bay`
deliberately hangs on `anchor/annex-second-bay`, which is not one of them.
`DW0889`, advisory, in the expected-warnings ledger, on every `validate`,
`analyze` and `build`.

Read the pair together, because they are about the same anchor and say different
things. `DW0498` needs the settled draw: `gallery-annex-cell-b` is seated twice,
so the name has two carriers and resolution takes the first. `DW0889` needs no
draw at all: nothing obliges the layout to seat that connector, so the name might
not have been in the world to begin with. The trigger is green on both counts —
the draw does seat it — which is exactly why neither code refuses. What they buy
is that a creator reading this campaign learns the constraint before spending a
build, and `delvec prefab anchors --pool pool/gallery-annex` states the same set
with no campaign at all.

**The seam cameras stand in open air.** `DW0724` refuses any render-plan
camera whose eye cell is occupied, over every shot kind the plan holds — seam and
interior as well as player-POV. A seam eye stands four blocks along the seal's
axis, one cell under the ceiling, on the tile's centre column, so a lantern hung
there would be a camera inside a block; the generator also asserts those cells
are clear on every run (`ANNEX_SEAM_EYE_CELLS`).

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

## The ferry, and what a link is

In the far hall's west corner is a **sealed cabin**: three walls and a roof,
no door, no gap. The beat `obj/cross-the-strait` stands inside it, and the only
way in is the ferry. The party boards the deck
(`obj/board-the-ferry`, which sets `flag/boarded`) and pulls the tiller:

```json
{ "id": "trigger/ferry-tiller", "at": "anchor/ferry-tiller", "on": { "on": "use" },
  "once": false, "requires_flags": ["flag/boarded"],
  "effects": [{ "type": "sequence", "steps": [
    { "at_ticks": 0,  "effects": [{ "type": "cutscene", "seconds": 1, "path": [ … ] }] },
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

The cutscene plays first and the teleport fires one tick after it ends: a
cutscene's end puts every player back where it started, so a carry under the
open bracket is undone. The layout graph draws the crossing as a `carry` edge
from `node/exit` (the deck is its station) to `node/ferry-cabin` (the landing
is its station), gated on the same flag, and nothing else joins the two.

**The landing completes the beat.** The ferry sets the party down inside
`obj/cross-the-strait`'s completion volume, so the server completes it on
arrival, during the tiller's step. `critical-path.json` keeps the reach step and
marks it `completed_on_landing`: the bot walks nothing for it and asserts the
marker already came.

**And a second link takes the party back out.** A sealed room nothing leaves
would end every walk there — the bot's death loop starts where the path ends,
and no lethal volume is reachable from inside the cabin. So the ferryman's chest
carries `trigger/cabin-tiller`, `once: false`, whose teleport moves whoever
stands in the cabin onto the exit; `edge/ferry-back` draws it, one way, from the
cabin to `node/exit` (the exit anchor is its station). `obj/reach-the-end` is the
last beat and comes after the crossing, and the return lands inside its volume
too, so the final step of the path is a reach a landing completes — the shape
whose campaign marker is due at the carrying step. `validation/teleport-gate.json`
reads two links and one gather.

Four probes show what the engine refuses about it:
`a-way-onward-that-fires-once` (the teleport on the cabin beat's own completion
— a gather, `DW0311` naming it and prescribing the link), `a-lever-outside-its-own-boat`
(the deck shrunk so no cell inside it reaches the tiller, `DW0932`),
`a-crossing-the-cutscene-undoes` (the teleport at tick 0, `DW0933`) and
`a-carry-the-graph-never-drew` (the edge removed, `DW0934`).

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

The build publishes `validation/loop-gate.json`: one loop, its 15 eyes, a span of
325 cells closed by geometry, 144 visible cells compared as blocks and as light at
two skies over 10 configurations, and one exercise step. The eight probes above
whose names start with a landing, a lamp, a light, a slab, a crossing, a release,
a figure and a hall are each this gallery plus one edit, and each is refused by
the rule that edit breaks.

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
names. The wall is the guarantee; the generator checks the sides, not a radius.

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

## Why the job gates

The gallery job is a required status check. `tools/ci/check-required-contexts.py`
holds the manifest and `ci.yml` in lockstep, and it reads the coverage count out
of `gallery/baseline/header.json` and the render findings out of
`gallery/render-plan.json` — both committed by the tools that measure them, so
the condition is evaluated rather than recited.
