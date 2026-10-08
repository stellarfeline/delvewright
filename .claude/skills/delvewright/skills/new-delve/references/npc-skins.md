# NPC skins — the face toolchain

- [Dress the character; do not hope the palette does it](#dress-the-character-do-not-hope-the-palette-does-it)
- [The face is the composer's, not yours](#the-face-is-the-composers-not-yours)
- [What the model cannot wear](#what-the-model-cannot-wear)
- [Replacing a vanilla texture](#replacing-a-vanilla-texture)

**Every named character gets a face**, so this page is the ordinary path and not
an exception: the default body is a player model wearing that character's own
skin, and a re-dressed villager is what you use when the character IS a
villager. Skip this page only for a cast that is genuinely villagers.

The skin toolchain is a Python package with dependencies, and Python
answering at I1 is not the same as the package being importable — a missing
skin is a build error, not a silent skip. Make it a venv here so step 5 does not
stop on it:

```sh
"$DELVEWRIGHT_PYTHON" -m venv .venv-skin
.venv-skin/bin/pip install -r "$DELVEWRIGHT_ENGINE/tools/creator/skin/requirements.txt"
PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/creator/skin" .venv-skin/bin/python -m delve_skin --help
```

The last line answering is the confirmation. Only the dependencies are
installed; the package itself is reached on `PYTHONPATH`, which leaves no build
artifacts in either repository. Use
`PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/creator/skin" .venv-skin/bin/python -m delve_skin …`
wherever this page says `python -m delve_skin`.

## Dress the character; do not hope the palette does it

A cast entry says what the character is **wearing and how they are groomed**, in
a `wardrobe` block. The palette says what colour each thing is; the wardrobe says
whether it is there at all and how far it reaches. Colours alone cannot put
sleeves on a character, or hair past their ears, or close a jacket at the throat.
Leave the wardrobe out and you get the default — a short-sleeved belted tunic
open at the neck, bare legs, sandals, a short back and sides and a full beard —
in whatever colours you named, which is a Bronze-Age sailor whatever the brief
said.

```json
{
  "texture_id": "castle-steward",
  "model": "wide",
  "palette": {
    "skin": "#d0a17c", "skin_shadow": "#a67a58", "eye": "#4b5a51",
    "hair": "#7d7669", "hair_grey": "#a9a29a", "hair_shadow": "#564f42",
    "tunic": "#39495e", "tunic_shadow": "#2a3646", "belt": "#212a3a",
    "legwear": "#3a3b41", "legwear_shadow": "#2a2b2f", "sandal": "#2c2521"
  },
  "wardrobe": {
    "sleeves": "long", "legs": "full", "footwear": "shoe",
    "hair": "jaw", "facial_hair": "none", "collar": "closed", "greying": "hair"
  }
}
```

| key | values | what it does |
|---|---|---|
| `sleeves` | `bare`, `short`, `long` | `long` reaches the wrist and leaves the hand; `short` covers the upper arm only, so the forearm is bare |
| `legs` | `bare`, `short`, `full` | `full` is trousers to the ankle; `short` is a skirt over the upper thigh. Painted in `legwear`, which falls back to `tunic` |
| `footwear` | `none`, `sandal`, `shoe`, `boot`, `tall_boot` | how far up the leg it reaches — barefoot, sandal, shoe, mid-calf, knee. Painted in `sandal` |
| `hair` | `bald`, `crop`, `short`, `jaw`, `long` | how far hair comes down the sides of the head. Past the ear it frames the face and takes a cut line in `hair_shadow`; `long` also falls onto the shoulders. **`short` is a short back and sides** — set this whenever the character is not one |
| `facial_hair` | `none`, `moustache`, `beard` | painted in `beard`. `moustache` is the lip row, `beard` the mouth and chin rows plus the sides of the jaw. **`none` is a face, not a blank** — the composer models the mouth, chin and jaw for everyone, so pick grooming for the character and nothing else |
| `collar` | `open`, `closed` | `closed` is the only way to fasten a garment at the throat — the open V is painted from `skin`, so no colour can close it |
| `greying` | `none`, `hair`, `beard`, `both` | streaks `hair_grey` / `beard_grey` through what it names. Keep those two colours **close together**: a wide gap reads as lichen on a rock, not as a greying head |

A misspelled key, value or palette colour **exits by name** rather than
composing the default and saying nothing, so a refusal here is the tool telling
you the character was about to be dressed wrong. `--help` on any subcommand
prints the whole surface — every entry field, every palette colour, every
wardrobe value, with the rows each one paints.

## The face is the composer's, not yours

You choose colours and grooming. **The composer paints the face**, on the rows a
face is on: chin, mouth, lip, eyes, eyebrows, forehead, the shadow under the
fringe, the fringe. Those rows are a measurement of the nine default player
skins the pinned client ships, not a style — `docs/reference/face-craft.md` in
the engine repository has the counts and their denominators.

Two consequences for what you write. **A clean-shaven character is not a blank
one**: everyone gets a mouth, a chin and a jaw that narrows toward it, so
`facial_hair` is a fact about the character and never a fix for a head that
looks wrong. And **there is no nose and no palette key for one** — at 8×8 a nose
is two dark pixels immediately over the mouth and the pair merges into a muzzle,
which is why no clean-shaven default skin has one either.

## What the model cannot wear

Only the base layer is authored, so **nothing stands proud of the body**: a coat
that hangs open, a hood, a hat with a brim, a cloak, a beard that juts, hair with
volume. Do not ask for these and do not approximate them — a hood painted flat on
a head reads as a badly-shaped haircut. A limb is 4 px around, so a cuff, a lapel
or a buckle narrower than a pixel does not exist; a garment cannot cross a body
part, so a sleeve and a torso hem are independent and no skirt hangs past the
hips; and sleeves take the torso garment's colour, there being no key for a
contrasting one.

**Hair is paint on the skull** — no bun, braid, ponytail, parting or silhouette
off the cube. And on this model **a figure cannot be made to read as a woman**:
what does it on a player skin is the `slim` arm geometry (which this composer
refuses), a hair silhouette off the cube, and face detail finer than 8×8. Hair
length is the only lever left and on a cube it is androgynous. So write a woman
in the **dialogue and the brief**, dress the figure well, and do not expect the
skin to carry the reading on its own — and never quietly recast a character
because the texture will not say it.

**Look at the previews** before accepting a skin, and always set `model`
(`wide`/`slim`) — an omitted model renders slim and distorts a wide skin.

## Replacing a vanilla texture

When the design wants something vanilla draws to look different for the whole
delve — the moon at the phase the story states, the drowned on the shore, the
stone of a hall — the campaign replaces that texture through the pack it already
ships. Every step is mandatory once a design calls for one:

1. **Name the texture from the census.** `world.json` `content.textures[]` takes
   one row per texture: `id` (a kebab token), `replaces` (a `minecraft:` path as
   vanilla's models spell it — `minecraft:environment/celestial/moon/full_moon`,
   `minecraft:entity/zombie/drowned` — never with `textures/` or `.png`), and
   `license`. A path the pinned client does not ship is `DW0939`, and the
   refusal prints the nearest real paths; read them, do not guess a spelling.
2. **Draw the image, from nothing.** `textures/<id>.png` in the campaign
   directory, at vanilla's size or a whole multiple of it (`DW0940` names the
   size if it is wrong). A recoloured vanilla texture is not original and may
   not ship; draw it new, or take one under an allowlisted licence. An
   animation is a strip of whole frames with `textures/<id>.png.mcmeta` beside
   it holding vanilla's `animation` object.
3. **Record the licence.** `{"spdx": "original", "source": "original"}` for
   your own drawing; anything else needs `url`, and CC BY also `attribution`
   (`DW0741`).
4. **Look at it.** `delvec --prefabs "$DELVEWRIGHT_PREFABS" textures campaigns/<id>` writes one sheet per row to
   `review/textures/` — vanilla beside yours. That sheet is the only place a
   mob's skin or the moon can be seen before the hand-over: no render this engine
   makes draws either. A block texture also shows in the build's own pack:
   `delvec --prefabs "$DELVEWRIGHT_PREFABS" viewer --pack <out>/resourcepack.zip …` and Chunky through
   `validation/chunky.sh --pack <out>/resourcepack.zip …`.
5. **Say it in the hand-over.** Each row is one item of step 13's report, in the
   player's words — *the moon tonight*, *the drowned on the shore*. A server
   never reads a resource pack, so only a person looking can confirm it.

The pack is **served by the server, never installed** into the player's own
folder: it applies while they are connected and is gone when they leave.
**A player who declines the pack sees vanilla's textures** (and reads English),
exactly as they would if the delve replaced nothing — so nothing a player must
see to finish the delve may live only in a replaced texture. If the design
genuinely needs every player to accept it, `world.json` `content.require_resource_pack:
true` makes the server disconnect anyone who declines; say so to the user before
choosing it.
