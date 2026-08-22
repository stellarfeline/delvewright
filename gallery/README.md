# The gallery

One campaign, holding at least one instance of every content-visible surface the
Delvewright DSL declares. It is built on every pull request, and it is never
played, released or staged.

It exists so that a new authoring surface meets something built to receive it. A
surface no campaign exercises is a surface nothing has ever compiled end to end,
and that is not a hypothetical: of the DSL's **941 declared surface units**, the
whole authored corpus — four campaigns and twenty-eight fixtures — writes 527.
The gallery binds **every one of them**: 935 written, 6 proven refused by a
probe the engine really rejects, and none left over.

Four of those turned out not to work the first time anything reached them: an
ambush and a named mob drop could not compile at all, a generated flag-gate test
could not pass, and a one-waypoint lane emitted a march test asserting an index
it had no way to reach. A fifth is open: the build's render plan and the one
`delvec snapshot` derives disagree whenever a world-edit blocks the route, because
one is computed after the edits and the other before them.

## Reading it

Every element is named for what it is, not for what it binds. Walk it in the
order a player would:

| Where | What it holds |
| --- | --- |
| `world.json` | the hall, its lighting and mitigation, the boundary, the declared languages |
| `npcs.json` | four speaking parts — a quest-giver, a gatekeeper, a counter, a drill officer |
| `classes.json` | two kits, one carrying a flask (what a bonfire rest refills) |
| `quest-plan.json` | two quests and the branch point the fork opens |
| `quests.json` | the bulk: objectives, effects, waves, actors, traps, triggers, a shop, a shortcut, a stake, a timed gate, two killing volumes |
| `dialogue.json` | one tree per NPC; the Curator's carries the fork |
| `world-edits.json` | four batches that dress the floor, lay the hearth, thin the vault and rough the lane |
| `geometry-brief.json` | four numbers out of the hall's own brief, the kind a site plan is later held to |
| `layout-graph.json` | the same hall stated as six places and twelve connections, before any coordinate — three barred doors through the wall because the hall really has three, a stair and a drop that close a loop, a sightline to the loft, and one place deliberately off the mandatory spine |
| `overlays/site-plan/` | those same six places given geometry, and then a whole map DERIVED from it: a region, a box each, a seam per connection on a face the two boxes share, the rock and the sky the whole owns, and eight comparisons holding all of it to its own written brief. It carries its own world, cast, quest layer and translations, because a campaign has ONE placement authority and the primary's is `areas[]` — so at this point of the campaign nothing describes a block, and everything a body meets is derived: the floors it walks, the doors it is stopped by, the stair it climbs, the anchors the quests bind to |
| `l10n/zh-cn.json` | the second language, so the sidecar surface is real rather than declared |
| `render-plan.json` | the view set the gallery declares, so a shot that vanishes is a red |
| `area/annex` (in `world.json`) | a three-tile chain assembled from `pool/gallery-annex` — what binds the piece verbs |
| `overlays/` | parameter points — settings that take one value per world |
| `probes/` | documents the engine **refuses**, each naming the diagnostic |
| `baseline/` | the committed emission index and the expected-warnings ledger |

The hall itself is generated, not committed: one 31 × 8 × 31 stone room split by
a barred wall, with three doors in it and a mezzanine in the far half whose
stair is missing its treads. Beside it the generator emits a small
**annex tileset** — four 7 × 6 × 7 boxes and a pool — plus a 3-cube **shard**
that exists only to be stamped by `fragment`. The annex is deliberately plain:
its job is the ASSEMBLY, and a tileset with interesting rooms would make the
placement harder to read without binding one more unit. Each tile carries one
anchor at the middle of its floor, which is what gives a camera pointed into the
annex something the campaign declares to frame. Its anchors are named for their role —
`anchor/hearth` is where you come back to life, `anchor/muster` is where a wave
forms up — and the generator prints that role into the piece's metadata, so the
piece explains itself without the campaign in hand.

## What the engine refuses

`probes/` is the half worth reading if you want to know what this engine checks.
Each probe is a committed document that a creator might reasonably write and that
`delvec validate` says no to, with the diagnostic it says no with. The last of
them names no surface at all, and that is the point of it: both halves of what it
writes are perfectly legal, and what the engine refuses is holding them at once.

| Probe | Code | What it tries |
| --- | --- | --- |
| `reserved-npc-roles` | `DW0141` | giving an NPC the `vendor` or `boss` role |
| `two-placement-authorities` | `DW0839` | carrying `areas[]` and a site plan at once |
| `aquatic-locomotion` | `DW0455` | declaring a body that swims |
| `peaceful-difficulty` | `DW0468` | setting the world to peaceful |
| `sound-at-actor` | `DW0335` | playing a sound from an actor's position |
| `a-piece-the-library-does-not-hold` | `DW0856` | binding the hall to a piece whose name is one letter wrong |
| `a-gate-two-areas-provide` | `DW0857` | binding the annex to the hall's own piece, so both areas provide one gate anchor |

Run any of them yourself:

```
delvec validate gallery/probes/peaceful-difficulty --prefabs <generated-prefabs>
```

They are not documentation of the refusals. They **are** the refusals: the
coverage gate runs each one and fails if the compiler ever starts accepting it,
because an exemption whose proof stopped holding is no longer an exemption.

## Building it

The piece and the mannequin skins are generated by the engine's own generator, so
the whole thing builds from this repository alone — no content checkout:

```
mkdir -p gallery-prefabs
cargo run --release --manifest-path prefabs/gallery-generator/Cargo.toml \
  -- gallery-prefabs --skins gallery/skins
cargo build --release -p delvec --bin delvec
target/release/delvec build gallery -o gallery-out --prefabs gallery-prefabs
```

Then the three gates:

```
python3 tools/check-gallery-coverage.py --prefabs gallery-prefabs \
  --build-out gallery-out --index gallery-coverage.md
python3 tools/check-gallery-render.py --prefabs gallery-prefabs \
  --build-out gallery-out --frames gallery-frames
python3 tools/gallery-baseline.py --prefabs gallery-prefabs
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

## What writing it turned up

Three engine defects, each invisible for the same reason: the surface had never
been written by anything, so nothing had ever compiled it. Each is attributed by
a differential, because that is the only honest way to say "this one thing is
responsible".

**An `ambushes[]` entry cannot compile at `dsl_version` 0.8.0 or above.**
`Ambush::to_trigger` desugars an ambush into a trigger whose effects are the
telegraph, then a `spawn-actor` and an `unleash-actor` per actor — constructing
both with `happening: None`. Since 0.8.0 `DW0481` requires a `happening` on every
beat, and `Ambush` carries no field an author could supply one through. The
surface is declared, schema-valid, and refused at validation on effects the author
never wrote and cannot reach. *Attribution:* removing the gallery's one ambush
takes the `DW0481` count from 2 to 0 with nothing else changed, and `to_trigger`
visibly hard-codes both `None`s. *State:* not fixed, and deliberately **not
exempted** — the five `Ambush.*` units are reported unaccounted, because a
capability gap is not a fence, and dressing one as an exemption is how a gate
stops being able to see it.

**A mob drop's `name` ships as an untranslatable literal.** `ItemDrop.name` is
lowered into `loot_table/dw_drop/*.json` as a raw `"text"` still carrying the
l10n marker sigil, and `DW0185` refuses the build. It is unconditional — it does
not need a second declared language — so every campaign that ever wanted to name
what a mob leaves behind would have hit it. *Attribution:* the diagnostic still
fires with the sidecar removed, and goes to zero with the drop names removed and
the sidecar restored. *State:* not fixed; the drops stay and only the name they
cannot carry is dropped. The fix is one call site — lower it through `emit::tr`
like every other player-visible string.

**A `traversal` declaration may be unbindable by construction.** `DW0454`
correctly refuses a `locomotion` that restates what the entity id already implies.
The consequence is that `Locomotion::ground` cannot be written on a ground mob at
all: binding one needs a body whose derived class differs *and* a route that makes
the claim non-inert. That is a question for the spec, which assumes any legal
combination is expressible by some overlay — these four may need geometry (a
climb, a wall to fly over) rather than a declaration. The hall's one vertical
route is the mezzanine's broken flight, and no body walks it — the climb is a
player's, so it settles nothing about a declared locomotion.

None of the three was found by reading code. Each was found by trying to write the
surface down.

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
The gallery hangs nothing on those anchors, which is the branch the diagnostic
sanctions; the alternative it names is more distinct variants, and that is a
choice about the pool rather than about the seed.

**One camera derivation is unguarded, and this tileset guards itself against it.**
`DW0724` refuses a **player-POV** camera whose eye cell is occupied — "fix the
camera derivation" — and nothing checks the same thing for the interior and seam
cameras `render_plan` derives beside them. A seam eye stands four blocks along
the seal's axis, one cell under the ceiling, on the tile's centre column; a
lantern hung there is a camera inside a block. The generator asserts those cells
are clear on every run (`ANNEX_SEAM_EYE_CELLS`), which is strictly weaker than the
diagnostic would be, because it can only speak for these four tiles.

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

## A finding still open

**The build's render plan and `snapshot`'s disagree when an edit blocks the
route.** One is computed after world-edits and the other before them, so solid
geometry on the critical path makes the build's legs longer than the ones
`snapshot` can resolve, and declared views become unproducible by name. Seen
twice: first with four shards stamped across the far hall, then with a
full-width barrier line. Both times the instance fix was to move the geometry off
the route; the divergence itself is untouched.

## Why the job gates

The gallery job is a required status check. `tools/check-required-contexts.py`
holds the manifest and `ci.yml` in lockstep, and it reads the coverage count out
of `gallery/baseline/header.json` and the render findings out of
`gallery/render-plan.json` — both committed by the tools that measure them, so
the condition is evaluated rather than recited.
