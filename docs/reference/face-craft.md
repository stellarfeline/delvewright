# A face at 8x8

Where the features of a human face go on the front of a Minecraft head, and why. Agent-facing.
The consumer is `tools/creator/skin/delve_skin/compose.py`, which holds these rows as the
`FACE_*` constants; this page is where they came from.

Every rule is marked **[cited]** with its source or **[authored]** — reasoned here from a cited
rule, with no source of its own.

Coordinates are the composer's: `y = 0` is the chin, `y` increases upward, `x = 0` is the
observer's left. Most external sources count `y` downward from the top of the face; `y_down =
7 - y`.

## 1. The rows

| y | what is on it |
|---:|---|
| 7 | hair — the fringe |
| 6 | the shadow under the fringe. Skin on a bald head |
| 5 | forehead |
| 4 | eyebrows, at x=1,2 and x=5,6 |
| 3 | eyes: pupil at x=2 and x=5, a skin highlight outboard at x=1 and x=6 |
| 2 | the lip. Bare on a clean-shaven face; a moustache, or a beard's top row |
| 1 | the mouth: two pixels at x=3,4 |
| 0 | the chin, stepping down to its corners |

**A face is the lower five rows and the hair takes the upper three.** **[cited]** A face painted
on the upper rows instead leaves four rows of unmodelled fill below it — half a head of flat
skin, which reads at playing distance as an enormous jaw, and which a full beard hides by
occupying exactly those rows. That is the failure this page exists to prevent, and it shipped.

## 2. The measurement

Two readings, sharing no instrument, agreeing row for row.

**The pinned client's own default skins.** **[cited]** The nine wide default player textures in
`assets/minecraft/textures/entity/player/wide/` of the 1.21.11 client jar (sha256
`1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd`), read at the head-front
region UV (8,8)–(15,15):

| what | count | denominator |
|---|---:|---|
| eye row at y=3 | 9 | 9 |
| a two-pixel mouth at x=3,4 on y=1 | 9 | 9 |
| chin row darker at its outer columns than at its centre | 7 | 9 |
| …among the clean-shaven (the two misses are the two whose chin row is beard) | 7 | 7 |
| a nose pixel anywhere, among the clean-shaven | **0** | 5 |
| bottom row of BOTH side faces of the head darker than the row above it | 6 | 9 |
| rows of hair over the centre of the brow | 0–4, median 2 | 9 |

**Mojang's hosted skin templates plus the published skinning tutorials.** **[cited]** An
independent reading of `assets.mojang.com/SkinTemplates/{steve,alex}.png` and the bearded Steve
variant, cross-checked against `textures.minecraft.net` and against tutorials at
[BlockSkinLab](https://www.blockskinlab.com/blog/how-to-add-eyes-to-minecraft-skin/),
[PlanetMinecraft](https://www.planetminecraft.com/blog/the-science-of-skinning-head-and-face-design/),
[DinowCookie](https://www.planetminecraft.com/blog/dinow-s-tutorials-part-5-human-anatomy-tips-and-tricks-for-shading-humans-skins/)
and [Envato Tuts+](https://design.tutsplus.com/tutorials/how-to-create-a-minecraft-skin-in-adobe-illustrator--cms-30831).
It reached the same rows: hair, hair, hairline, brow, eyes, lip, mouth, chin. Three of those
sources independently state a **2×1 mouth**, and DinowCookie states "darkening around the chin
with shading to make the head look a bit more round" and "shading underneath the hairline,
especially when the character has a fringe".

**Do not copy a vanilla pixel.** The default skins are Mojang's assets. Where a feature is and
how dark it is relative to the skin around it are facts about the game and are what this page
records; the hex values are not, and no committed file here holds one (ADR-0013).

## 3. The rules

**There is no nose.** **[cited]** No clean-shaven default skin has one (0 of 5). The dark pair
below the eyes on the bearded ones is the moustache. At this size a nose is two dark pixels
immediately above the mouth, and the pair merges with it into a muzzle — the general form of
"two adjacent dark pixels often merge into a visor or nose", BlockSkinLab.

**The mouth is two pixels, centred, one row, never black.** **[cited]** Vanilla's sit at
0.56–0.65 of the cheek's luminance; the tutorials say "1x2 mouth in a dark shade" and "mouths
don't need to be black lines".

**The chin row steps down toward its corners, and so do the sides of the head.** **[cited]** In
vanilla the chin row's outer pair is the darkest non-feature pixel on the face (0.55 of cheek)
and the pair inboard of it is 0.65. A jaw that narrows only on the front face is a mask, so the
taper carries onto the left and right faces of the head, which is most of what a player walking
past sees.

**The face is never ringed.** **[cited]** Darkening every edge of a form is pillow shading —
"what happens when trying to shade an object with no clear light source and making a generic
shadow around the outlines" ([Saint11](https://saint11.art/pixel_art_articles/article4/)).
Vanilla darkens the bottom and leaves the top of the face alone, and so does the composer: the
taper is the chin row only.

**A band across the whole face is a headband.** **[authored]** The eyebrows are two pixels each,
over the columns each eye occupies; the one full-width row is the shadow under the fringe, and a
bald head does not get it because there is no fringe to cast it.

**A second skin shade is half a step further along the first, not a fixed offset.** **[authored,
from the measurement]** `palette.deepen` takes the step the palette itself declares — `skin` to
`skin_shadow` — and goes half as far again. Against the 0.67–0.72 shadows the palettes in hand
declare, that lands at 0.51–0.58, matching vanilla's mouth and chin corners; a fixed offset
darkens a dark skin to black long before it has moved a pale one, and a whole further step lands
at 0.35–0.44, darker than anything vanilla puts on a face.

## 4. What is still open

**Light direction.** **[authored]** Vanilla grades the two side columns of the face
asymmetrically — one side is lit, the other is not. The composer's face is symmetric. Adopting a
light direction means fixing which texture column renders on the viewer's left, which no source
consulted here states; settle it with a render before encoding it.

**Eyes have no whites.** **[cited that vanilla has them]** Every default skin gives each eye two
pixels: a true-white sclera outboard and a pupil inboard. The composer paints a lightened skin
pixel where the sclera goes, so the eye reads at the right size with no palette key for white.
Whether a white sclera is wanted is a look decision, not a defect.
