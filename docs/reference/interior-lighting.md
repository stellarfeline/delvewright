# Interior lighting

> **UNFINISHED — this round was stopped by the planner mid-flight.** Sections 1–3 are complete and
> sourced. Sections 4–7 (the emitter table, map-maker practice, the mob-spawning floor, the
> provenance table) are placeholders: that research line was cut off before it reported. Do not
> read the absence of section 4 as a claim that emitter mechanics do not matter — it is a gap.
> `docs/notes/interior-lighting-measurements.md` holds the measurements behind section 1.

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
the function is right. The other columns are what a designer needs and the engine does not model.

<!-- EMITTER-TABLE -->

## 5. What established map-makers do

<!-- PRACTICE -->

## 6. The mob-spawning floor

<!-- SPAWNING -->

## 7. Rule provenance

<!-- PROVENANCE -->
