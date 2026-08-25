# Interior lighting

> The measurements behind §1, §2 and §4.1 are `docs/notes/interior-lighting-measurements.md`.
> §6.5 records what could not be researched, so those gaps are not read as absence of practice.

How an interior is lit in a Delvewright delve. Agent-facing. Current behaviour.

**Light is placed while the room is designed. The engine only checks it.** Where a lamp hangs
is part of what a room is — a watch post, a turning toward the cellar, a niche nobody lights any
more. When a brightness check fails the repair is to re-arrange the room or raise the density of
what is already there, never to run a pass over a lightless scene afterwards: an algorithm cannot
place light for atmosphere, only for coverage.

Every rule below is marked **[cited]** with its source or **[authored]** — derived here from the
engine's own measured behaviour, or reasoned from a cited rule without a source of its own.

## 1. What the engine checks

`DW0210` refuses a build when a reachable walkable cell in an area measures below light **3**
under the darkest reachable `(time, weather)` sky, with no `lighting` and no `mitigation`
declaration. `DW0211` is its declared-relight sibling.

The model is `crates/compiler/src/light.rs` — the **one** authority for emission, opacity and the
flood. `crates/admit`'s prefab probe and spec-0010's assembled gate both read it rather than
keeping a copy; a private second copy is what once left the prefab probe with no sky term at all
and reported daylit colonnades as pitch black.

Three facts follow, and they decide how much light a room actually needs. **[authored]** —
measured at engine `86944766` by reading `effective_sky`, `DARK_THRESHOLD` and `flood`.

**Sky light reaches exactly two cells deep at night.** `effective_sky` returns **4** for every
night hour (`dusk`, `night`, `midnight`, `dawn`), and weather is ignored at that floor. The flood
seeds a passable cell whose column above is open at that value and decrements **1 per step**. So:

| cell | light | verdict |
|---|---:|---|
| open to the sky | 4 | passes |
| one step from open sky | 3 | passes — the test is `l < 3` |
| two steps from open sky | 2 | **fails** |

An open arcade at the top of a tower still measures dark in its middle. No opening, louvre or
oculus lights a room at night. In a night delve, interior light is **placed** or it does not exist.

**The gate is a minimum, not a coverage requirement.** Light falls 1 per step, so an emitter of
level `E` holds every cell within `E - 3` steps of shortest passable path at or above the bar:

| emission | holds light 3 out to |
|---:|---:|
| 15 | 12 steps |
| 14 | 11 steps |
| 10 | 7 steps |
| 7 | 4 steps |
| 5 | 2 steps |
| 3 | 0 — lights only its own cell |

One lantern covers a 25-block-diameter sphere. **Paving a floor with a glowing block is a
misreading of this gate**, not an implementation of it — the bar is cleared by sparse fixtures
placed where a person would have hung them.

**`DW0210` names one cell of one area per build.** `measure_undeclared` returns the single
*darkest* reachable cell of an area; diagnostics sort by `(code, message)` and `emit` returns on
`diagnostics.first()`. Since every message begins ``area `area/<name>` has …``, the build reports
the alphabetically-first dark area and one cell of it. **A cell count read off the diagnostic is
an undercount**, and an area count read off it hides every other dark area. Measuring the
distribution needs a harness over the assembled world; the diagnostic does not carry it.

## 2. Where a lamp may physically go

`crates/grammar/src/nav.rs` decides passability as **air, or a `*_skull`**. Every other block is
a full solid cube to the zone-program walk proof. A lantern, torch, candle or campfire dropped
into a cell a body walks through therefore *removes that cell from the walk*, and a bed of a
non-collidable block on a floor reads as a **new floor level**.

So a lamp never occupies a cell a body stands in. It goes in one of:

| placement | how | why it is safe |
|---|---|---|
| **into the wall face** | replace one course of a masonry column with the lamp role, masonry above and below | the cell was solid and stays solid — no walk proof moves |
| **under the ceiling** | hanging lantern in the air course above head height | body needs its own cell and the one above; the ceiling course is neither |
| **in a niche** | recess cut into the wall, lamp inside it | the niche is not on the walk |
| **on a ledge, sill or shelf** | standing lantern on a solid block above floor level | the supporting block is not floor a body uses |

This constraint and the craft agree, which is the useful part: architecture puts light in sconces,
brackets, hanging chains and window recesses for its own reasons. **[authored]**, from the three
predicates read at `86944766`.

Note that the engine carries **three** different passability predicates — `grammar::nav`
(air-or-skull, strictest, governs zone programs), `admit::light::is_passable` (air plus torches,
water, vine, glow lichen, rail, light block), and `compiler::assembled::occupancy_of` (air plus
trap triggers, thin decoration, fence gates). A design that satisfies the grammar's satisfies all
three.

## 3. The craft rules

### 3.1 Motivated light — every source has an in-fiction reason to be there **[cited]**

A *fixture* is the visible plausible cause; a *motivated* light is one that has one.

> "A **light fixture** is a visible plausible source of light, like a light bulb or a fireplace. …
> A **motivated light** is a light source with a plausible fixture." — The Level Design Book,
> [Lighting](https://book.leveldesignbook.com/process/lighting)

> "Motivated lighting is the technique used to imitate or accentuate existing light sources.
> Motivated light is commonly described as light within a shot that can be justified." —
> [StudioBinder](https://www.studiobinder.com/blog/what-is-motivated-lighting-in-film/)

**The contested half, and it is worth knowing.** Steve Theodore (Half-Life, Team Fortress,
Counter-Strike) argues the opposite emphasis:

> "Many games shy away from overtly theatrical lighting, fearing that players will wonder where
> that helpful little light is coming from. **Audiences rarely notice or care** … Light for the
> effect you need, and to hell with physics." — *Game Developer*, March 2005,
> [How and where to use colored light](https://www.gamedeveloper.com/design/lighting-design-fundamentals-how-and-where-to-use-colored-light)

His refinement is the usable rule, and it is stated nowhere else found: **strong lights need a
plausible source, soft fills do not.** In Minecraft the distinction mostly collapses, because
every emitter *is* its own visible fixture — there are no invisible fills except the `light`
block. That makes motivation cheap to honour here and is why this document keeps it. **[authored]**

### 3.2 A scene needs more than one role **[cited]**

> "(1) **Key light** is the main dominant light source … (2) **Fill light** brightens darker areas
> to avoid plunging everything into shadow. (3) **Rim light** highlights edges to pop the
> foreground from the background." — The Level Design Book,
> [Three point lighting](https://book.leveldesignbook.com/process/lighting/three-point)

Three-point is a camera theory and the player owns the camera:

> "the big problem with using three point for games is that it assumes you have complete control
> over the camera… but what if the player controls the camera?" — Robert Yang,
> [GDC 2018: How To Light A Level](https://www.blog.radiator.debacle.us/2018/03/gdc-2018-how-to-light-level-slides-and.html)

The book's recommended replacement is spatial rather than framed, and its first two strategies are
directly buildable in a delve:

> "⚀ 1. Focal point — Place a lone light source to emphasize a specific point or place, to suggest
> the player approach this exact location. ⚁ 2. Focal frame — Place two similar light sources next
> to each other to frame something in the space between… Frame an entrance or exit. Torches,
> sconces." — [D6 lighting](https://book.leveldesignbook.com/process/lighting/d6-lighting)
> (strategies 4–6 on that page are headings with no body text; do not cite them as elaborated)

### 3.3 Light and dark are a navigation grammar **[cited, with a documented dissent]**

> "**EXIT HIGHLIGHTING** — … the scene is mostly plainly lit, but the intensity of the exit-area
> draws the player in… **PATH HIGHLIGHTING** — … emphasizes the correct route through the area."
> — Magnar Jenssen,
> [Functional Lighting](https://www.worldofleveldesign.com/categories/wold-members-tutorials/magnar_jenssen/functional-lighting-magnar-jenssen.php)

Darkness is the negative half, and the hierarchy is the load-bearing part:

> "The far wall is dark, maybe it leads to a backdoor or a closet. It's probably not a primary
> exit, which would be lit more prominently." … "Build a hierarchy. Big important exits should
> have more important looking lighting, while secondary spaces should have dimmer less focused
> lights." — The Level Design Book, [Lighting](https://book.leveldesignbook.com/process/lighting)

> "Players are attracted to the light. God rays draw attention and a line to the goal." — David
> Shaver (Naughty Dog), *Invisible Intuition*, GDC 2018,
> [Director's Cut deck](http://davidshaver.net/DShaver_Invisible_Intuition_DirectorsCut.pdf)

**Three qualifications, and a design that ignores them will over-claim.**

- The same book that supplies the hierarchy rule **rejects the framing**: "We urge level designers
  to reject notions of 'guiding the player'"; and calls leading-line reasoning the "shot
  composition fallacy" — "A video game level is a place, not a painting, photo, or film"
  ([wayfinding](https://book.leveldesignbook.com/process/blockout/wayfinding),
  [composition](https://book.leveldesignbook.com/process/blockout/massing/composition)). It
  concedes this is a minority position. Shaver's deck, meanwhile, teaches leading lines as one of
  its principles — **the two best sources here disagree on exactly this point.**
- The same book's wayfinding table ranks "lighting, color" at **40% certainty**, *below*
  environmental storytelling, ground composition and repetition. Lighting is a suggestion, not an
  instruction.
- Lighting-as-guidance can hide a layout defect: "It's usually better to add breadcrumbs to your
  blockmesh after early playtests because they can hide fundamental layout guidance problems." —
  Shaver.

**Consequence for this engine [authored]:** light may reinforce a route the geometry already
makes legible. It may not be the only thing that makes a route legible.

### 3.4 Landmarks orient; light is not what makes them landmarks **[cited for the first clause, authored for the second]**

> "**Landmarks (aka weenies)** • Orients players. • Distant object seen from many vantage points in
> the level. • A goal to work toward." … "They also have unique silhouettes so you don't confuse
> them." — Shaver, GDC 2018

> "Landmarks become more easily identifiable … if they have a clear form; if they contrast with
> their background; and if there is some prominence of spatial location. Figure-background
> contrast seems to be the principal factor." — Kevin Lynch, *The Image of the City*, 1960

**No source found states "light the landmark so it reads as a beacon across the space" as a
principle.** The nearest attested things are the D6 focal point and Lynch's figure–background
contrast. So: a landmark earns recognition from its silhouette, and light may make that silhouette
legible at night — but treat the beacon reading as **authored**, not as established practice. This
matches the constitution's own rule that the silhouette carries the recognition detail cannot.

### 3.5 The eye reads contrast, not absolute brightness **[cited]**

This is the best-evidenced rule in the set and the one that resolves "make it dark" against a
minimum-brightness gate.

> "**The key-fill ratio is distinct from the overall brightness of the scene.** High ratio scenes
> are typically darker overall than those with lower ratios." … "our eyes are programmed to
> recalibrate their perceptions of light and dark… **we instinctively see contrast as a proxy for
> the intensity of light in a scene.**" — Steve Theodore, *Game Developer*, April 2005,
> [Using contrast in your game](https://www.gamedeveloper.com/art/lighting-design-fundamentals-using-contrast-in-your-game)

He gives a floor worth keeping: "begin experimenting with a moderate contrast ratio, in the region
of 3:1. Go much lower than that and it becomes difficult to perceive the 3D contours of a space".

And the industry convention for darkness:

> "game developers generally follow the film industry convention ('Hollywood Darkness'): **make it
> *feel* dark, but don't actually make it dark.**" … "If we light our level 'accurately' with
> pitch-black darkness, then players can't see where to go and become frustrated." — The Level
> Design Book, [Lighting for darkness](https://book.leveldesignbook.com/process/lighting/darkness)

On a night moodboard the same page notes: "it's actually pretty bright; what makes it feel dark is
increased contrast".

**Consequence for this engine [authored]:** a design record that calls a room "dark" is asking for
a *ratio*, not for light 0. A room whose lit pool sits at 12–15 and whose corners fall to 3 reads
as darker than a room flat at 7, and only the first passes `DW0210`. The gate and the atmosphere
are not in conflict; a flat, evenly-lit room is the thing both reject.

### 3.6 Colour temperature is an authored code, not a universal one **[cited]**

> "We all know the basics of color association: red for danger, blue for peace… It can be very
> tempting to fall back on these old standbys as a shortcut to an emotion. Often, though,
> dominating color schemes can backfire." — Theodore, March 2005

His *Half-Life 2* example is the proof that the code is learned:

> "**The opening levels of Half-Life 2 use the play of color temperature the opposite way: The
> player quickly learns to associate the warm colors of sunlight and open space with danger, while
> the blues and greens of murky sewers and tunnels become indicators of safety and concealment.**"

**Consequence for this engine [authored]:** Minecraft gives exactly one temperature axis that
costs nothing — the **soul** variants (soul lantern, soul torch, soul campfire, soul fire) are
cyan-white against the orange of every ordinary flame, and they are simultaneously *dimmer* (10
against 15). Warm-versus-cool and bright-versus-dim are therefore the same decision here, which is
a real constraint on how much a delve can say with colour. Consistency across a delve is what makes
either readable.

## 4. The emitter table

Emission values **mirror** `emission()` in `crates/compiler/src/light.rs`, which is the authority
and carries a per-block wiki citation for each entry. If this table and that function disagree,
the function is right. The other columns are what a designer needs and the engine does not model;
they are read from each block's own page on `minecraft.wiki`. **[cited]**

**"Modelled"** says whether `emission()` knows the block. A block it does not know emits **0** in
the proof, so it cannot clear `DW0210` however bright it is in game — see §4.1.

| emitter | light | modelled | full cube | waterloggable | attaches to |
|---|---:|:---:|:---:|:---:|---|
| [lantern](https://minecraft.wiki/w/Lantern) | 15 | yes | no | **yes** | top face of a block, or **hung** from a block or chain above (`hanging=`) |
| [soul lantern](https://minecraft.wiki/w/Soul_Lantern) | 10 | yes | no | **yes** | as lantern |
| [torch](https://minecraft.wiki/w/Torch) | 14 | yes | no | **no** — pops off in water | top of a solid block; `wall_torch` is a **separate id** with `facing` |
| [soul torch](https://minecraft.wiki/w/Soul_Torch) | 10 | yes | no | no | as torch |
| [campfire](https://minecraft.wiki/w/Campfire) | 15 | yes (`lit`) | no | **yes** (extinguishes it) | floor. **Damages anything in its cell**; any block above it prevents that |
| [soul campfire](https://minecraft.wiki/w/Soul_Campfire) | 10 | yes (`lit`) | no | yes | as campfire, double damage |
| [glowstone](https://minecraft.wiki/w/Glowstone) | 15 | yes | **yes** | no | ordinary block |
| [sea lantern](https://minecraft.wiki/w/Sea_Lantern) | 15 | yes | **yes** | no | ordinary block |
| [shroomlight](https://minecraft.wiki/w/Shroomlight) | 15 | yes | **yes** | no | ordinary block; passes redstone |
| [froglight](https://minecraft.wiki/w/Froglight) ×3 | 15 | yes | **yes** | no | ordinary block. No plain `froglight` id — only the three prefixed ones |
| [glow lichen](https://minecraft.wiki/w/Glow_Lichen) | 7 | yes | no | **yes** | **any face** of a solid block, several at once |
| [amethyst cluster](https://minecraft.wiki/w/Amethyst_Cluster) | 5 | yes | no | yes | a solid full surface; refuses fences, chains, lanterns, panes |
| large / medium / small bud | 4 / 2 / 1 | yes | no | yes | as cluster |
| [end rod](https://minecraft.wiki/w/End_Rod) | 14 | yes | no | no | **any face of any block**, and **survives its support being removed** |
| [magma block](https://minecraft.wiki/w/Magma_Block) | 3 | yes | **yes** | no | ordinary block. **Damages anything standing on it** unless sneaking |
| [crying obsidian](https://minecraft.wiki/w/Crying_Obsidian) | 10 | yes | **yes** | no | ordinary block |
| [jack o'lantern](https://minecraft.wiki/w/Jack_o%27Lantern) | 15 | yes | **yes** | no | ordinary; **lights while submerged** |
| [lit furnace / smoker / blast furnace](https://minecraft.wiki/w/Furnace) | 13 | yes (`lit`, default false) | yes | no | directional; only while active |
| [sea pickle](https://minecraft.wiki/w/Sea_Pickle) (1–4) | 6/9/12/15 | yes | no | always | **only lights underwater** |
| [cave vines + glow berries](https://minecraft.wiki/w/Glow_Berries) | 14 | yes (`berries`) | no | no | **hangs** from a block's bottom face |
| [light block](https://minecraft.wiki/w/Light_(block)) | 0–15 | yes (`level`) | invisible | yes | **attaches to nothing**; survives neighbours breaking |
| [beacon](https://minecraft.wiki/w/Beacon) | 15 | yes | no | no | lights **without** a pyramid or beam |
| [respawn anchor](https://minecraft.wiki/w/Respawn_Anchor) | 0/3/7/11/15 | yes (`charges`, default 0) | yes | no | **explodes if used outside the Nether** |
| [brown mushroom](https://minecraft.wiki/w/Brown_Mushroom) | 1 | yes | no | no | **emits nothing in a flower pot** |
| [enchanting table](https://minecraft.wiki/w/Enchanting_Table) / [ender chest](https://minecraft.wiki/w/Ender_Chest) | 7 | yes | no | ender chest yes | ordinary |
| **[candle](https://minecraft.wiki/w/Candle) ×1–4 lit** | **3/6/9/12** | **NO — 0** | no | yes | up to four per cell, on a **solid** block; placed unlit |
| **[copper bulb](https://minecraft.wiki/w/Copper_Bulb)** | **15/12/8/4** by oxidation | **NO — 0** | yes | no | ordinary; toggles on a redstone **pulse** |
| **[copper lantern](https://minecraft.wiki/w/Copper_Lantern)** | **15** | **NO — 0** | no | yes | as lantern. Added 1.21.9 |
| **[copper torch](https://minecraft.wiki/w/Copper_Torch)** | **14** | **NO — 0** | no | yes | as torch. Added 1.21.9 |
| **[redstone lamp](https://minecraft.wiki/w/Redstone_Lamp) (lit)** | **15** | **NO — 0** | yes | no | ordinary; instant on, 0.2 s off |
| **[sculk catalyst](https://minecraft.wiki/w/Sculk_Catalyst)** | **6** | **NO — 0** | yes | no | ordinary |
| **[firefly bush](https://minecraft.wiki/w/Firefly_Bush)** | **2** | **NO — 0** | no | — | added 1.21.5 |

### 4.1 The modelling gap, and it is design-blocking **[authored]**

Thirteen emitters that exist at the pin are absent from `emission()` and therefore measure **0**:
candle, copper bulb, copper lantern, copper torch, redstone lamp, sculk catalyst, firefly bush,
sculk sensor, trial spawner, vault, nether portal, end portal frame and dragon egg. Verified by
grepping the function at `86944766`.

The direction is safe — the function documents a **never-overestimate contract**, and an absent
block emitting 0 is an underestimate, which can only make the gate stricter. But the consequence
for a designer is real and is not a safety property:

> **A room lit only by candles, copper lanterns or redstone lamps refuses to build**, however
> bright it is in the game.

Candles are the sharp case: they are the fiction-correct low, warm, domestic source, and they are
invisible to the proof. Use them as **unlit decoration**, which costs nothing, and light the room
with something the model knows.

## 5. The mob-spawning floor

**At 1.21 a hostile Overworld mob needs `block light == 0` *and* internal sky light ≤ 7.**
**[cited]**

> "In the Overworld, if the **internal sky light** level is **7 or less** (which always occurs
> inside a cave) and the **block level is 0**, all Overworld monsters can spawn."
> — [Mob spawning](https://minecraft.wiki/w/Mob_spawning)

This changed in 1.18 and the change is the important part:

> "**1.18** (Experimental Snapshot 1): Block light level now must be **0** for many hostile mobs to
> spawn." — [Light § History](https://minecraft.wiki/w/Light)

**The floor on interior light is therefore block light 1, not 8. [authored]** Any enclosed room
has sky light 0, so block light is the only gate. `DW0210`'s threshold of 3 sits **above** the
spawn floor, so **a room that satisfies the darkness gate is already spawn-proof** — the two
constraints do not compete, and atmospheric darkness at block light 1–4 is mechanically safe.

Carve-outs a delve should know: **slime-chunk slimes ignore light entirely**; **blaze and
silverfish spawners need light 12**; and mobs cannot spawn on **bottom slabs, glass, trapdoors,
leaves, ice or rails at any light level**, which is usually cheaper than lighting.

**Hazard.** Most indexable community lighting advice predates 1.18 and states the old "light 8"
rule — including [minecraft101](https://minecraft101.net/t/lighting.html), whose torch-spacing
tables are built on it. Do not mine spacing figures from undated guides.

## 6. What established map-makers do

### 6.1 Hide the source, keep the light **[cited]**

The oldest and most consistent practice in the record is that the *fixture* is often concealed
while the light stays. From the Minecraft Forum
[hidden lighting](https://www.minecraftforum.net/forums/minecraft-java-edition/discussion/193082-hidden-lighting)
and [indoor lighting](https://www.minecraftforum.net/forums/minecraft-java-edition/survival-mode/218004-indoor-lighting)
threads:

> "Place a light source of any kind in a hole, then place carpet on top." — Outkin, 2014
> "You can cover torches with half slabs or stairs on floors and ceilings." — nocturn333, 2014
> "I hide jack-o-lanterns under water." — Vincenzo, 2011

and glowstone recessed "in the wall at the base of the windows" to wash light upward through
stained glass — cove lighting, in effect. The wiki's own
[Adding beauty to constructions](https://minecraft.wiki/w/Tutorial:Adding_beauty_to_constructions)
tutorial recommends "redstone lamps or sea lanterns and glowstone **instead of torches**", plus
chandeliers and a ceiling-hung beacon.

**Caution for this engine [authored]:** carpets and slabs are exactly the thin non-collidable
decorations `grammar::nav` treats as **full solid cubes** (§2). The concealment tricks that work
in survival will move a zone program's walk proof. Concealment here means **recessing the emitter
into the wall or ceiling**, not covering it with a thin block.

### 6.2 The light block is the adventure-map tool **[cited]**

> "Maps are always cluttered with glowstone and torches. Why not have a only accessible in creative
> light source, that provides light without an eyesore." — Surfer_72, 2012,
> [invisible light source for map makers](https://www.minecraftforum.net/forums/minecraft-java-edition/suggestions/70064-invisible-light-source-for-map-makers)

Three facts decide whether a delve should use it, all from
[Light (block)](https://minecraft.wiki/w/Light_(block)):

1. **In Adventure mode it is completely invisible and inaccessible** — and a delve is Adventure
   mode. It is the only emitter with no visual presence at all.
2. It **attaches to nothing**; breaking a neighbour does not remove it.
3. **A falling block, or any block placed into its cell, destroys it** — silently.

**Use it only where an unmotivated fill is genuinely wanted [authored]**, and prefer a real
fixture everywhere else: this document's first rule is that light is part of what a room is, and
an invisible source is by construction not decoration.

### 6.3 "Gradient" means the palette, not the light — a correction **[cited]**

The brief for this research assumed builders discuss light-level gradients. They mostly do not.
In build-team vocabulary a gradient is a **block-value** gradient:

> "A gradient is simply a transition from one value and/or colour to another. In Minecraft this
> means transitioning one block to another to create a gradient effect on a surface."
> — [Conquest Reforged, Gradienting your Builds](https://www.conquestreforged.com/guides/gradients)

That page does not discuss light sources, lighting or shadow at all. The community's craft language
for "light and dark" lives in the **palette**. The clearest attributable statement of light used
for *contrast* rather than illumination is a build guide:

> "I set my light sources behind the build so it lights up the back of the build along with the
> wall behind it, making the build pop from its backdrop." — NutmegNam,
> [Building Tips From a #1 Builder](https://hypixel.net/threads/guide-building-tips-from-a-1-builder.3741027/)

### 6.4 What they avoid — and the honest state of the evidence

Torch-spam as the beginner tell is attested, but only weakly and only in old posts: builders in
2011 and 2014 say torches "don't look nice" indoors and that they avoid putting them on floors.
**Every one of those posts predates lanterns, campfires, soul variants, candles, glow lichen,
froglights and copper bulbs.** The modern lantern/candle/campfire vocabulary is real in practice
but **no substantive attributable written source for it was found**; the sources asserting it are
content farms and are deliberately not cited here.

**On paving a floor with glowing blocks, the evidence is weaker than the practice deserves.** The
one on-point community statement found — "Placing glowstone in random places in the floor is
considered a noobish approach to lighting large rooms" — is from a Planet Minecraft thread that
**returned 403 and could not be opened**. It is a search snippet and is marked here as
**unverified**, not cited.

**This engine does not need that citation, and should not lean on it. [authored]** §1 gives an
established, engine-native reason: light falls one level per step, so one lantern holds a
25-block-diameter sphere above the threshold. Paving is not a stricter reading of the gate — it is
a misreading of what the gate asks for. That argument rests on measurement, not on taste.

### 6.5 What could not be researched

Recorded so the gaps are not mistaken for absence of practice. **Reddit is entirely inaccessible**
to this toolchain (crawler blocked); **Planet Minecraft, minecraftmaps.com, wiki.ardacraft.me and
minecraft.wonderhowto.com all return 403**; the SearXNG fallback returned HTTP 429 on every
attempt and produced nothing. No named building channel's transcript and no builder interview was
obtained. Two wiki facts are contested and are **not** relied on above: whether a conduit lights
when inactive (two wiki pages disagree), and whether light passes a *closed* trapdoor at full
strength (no wiki statement found).

## 7. Rule provenance

| § | rule | provenance |
|---|---|---|
| 1 | sky reaches two cells deep at night | **authored** — measured at `86944766` |
| 1 | emitter radius `E − 3`; the gate is a minimum | **authored** — measured |
| 1 | `DW0210` names one cell of one area | **authored** — measured |
| 2 | a lamp never occupies a walkable cell | **authored** — measured, three predicates |
| 3.1 | motivated light | **cited** — Level Design Book, StudioBinder; dissent from Theodore |
| 3.1 | strong lights need a source, fills do not | **authored** — from Theodore's refinement |
| 3.2 | key / fill / rim; focal point and frame | **cited** — Level Design Book, Yang |
| 3.3 | light and dark as navigation grammar | **cited** — Jenssen, LDB, Shaver; **LDB rejects the framing**, and LDB ranks lighting at 40% certainty |
| 3.3 | light may reinforce but not carry a route | **authored** — from that dissent |
| 3.4 | landmarks orient | **cited** — Shaver, Lynch |
| 3.4 | light makes a landmark a beacon | **authored** — *no source found* |
| 3.5 | the eye reads contrast, not brightness | **cited** — Theodore 2005; LDB "Hollywood darkness" |
| 3.5 | "dark" means a ratio, not light 0 | **authored** — from 3.5 plus §1 |
| 3.6 | colour temperature is a learned code | **cited** — Theodore 2005, incl. the *Half-Life 2* inversion |
| 3.6 | soul variants are cooler *and* dimmer | **authored** — from §4 |
| 4 | emitter mechanics | **cited** — `minecraft.wiki`, per row |
| 4.1 | thirteen emitters model as 0 | **authored** — measured |
| 5 | spawn floor is block light 1 | **cited** — wiki; **authored** that it sits below the gate |
| 6.1–6.2 | concealment, the light block | **cited** — forum threads, wiki |
| 6.1 | conceal by recessing, not by covering | **authored** — from §2 |
| 6.3 | "gradient" means palette | **cited** — Conquest Reforged |
| 6.4 | do not pave | **authored** — from §1; the community citation is **unverified** |
