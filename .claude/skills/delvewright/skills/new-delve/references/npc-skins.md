# NPC skins — the face toolchain


**Every named character gets a face**, so this page is the ordinary path and not
an exception: the default body is a player model wearing that character's own
skin, and a re-dressed villager is what you use when the character IS a
villager. Skip this page only for a cast that is genuinely villagers.

The skin toolchain is a Python package with dependencies, and `python3
--version` answering is not the same as the package being importable — a missing
skin is a build error, not a silent skip. Make it a venv here so step 5 does not
stop on it:

```sh
python3 -m venv .venv-skin
.venv-skin/bin/pip install -r "$DELVEWRIGHT_ENGINE/tools/skin/requirements.txt"
PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin --help
```

The last line answering is the confirmation. Only the dependencies are
installed; the package itself is reached on `PYTHONPATH`, which leaves no build
artifacts in either repository. Use
`PYTHONPATH="$DELVEWRIGHT_ENGINE/tools/skin" .venv-skin/bin/python -m delve_skin …`
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
| `hair` | `bald`, `crop`, `short`, `jaw`, `long` | how far hair comes down the sides of the head. Past the ear it frames the face and takes a cut line in `hair_shadow`; `long` also falls onto the shoulders. **A clean-shaven character left at `short` reads as a short-back-and-sides man** — set this whenever they are not one |
| `facial_hair` | `none`, `moustache`, `beard` | painted in `beard` |
| `collar` | `open`, `closed` | `closed` is the only way to fasten a garment at the throat — the open V is painted from `skin`, so no colour can close it |
| `greying` | `none`, `hair`, `beard`, `both` | streaks `hair_grey` / `beard_grey` through what it names. Keep those two colours **close together**: a wide gap reads as lichen on a rock, not as a greying head |

A misspelled key, value or palette colour **exits by name** rather than
composing the default and saying nothing, so a refusal here is the tool telling
you the character was about to be dressed wrong. `--help` on any subcommand
prints the whole surface — every entry field, every palette colour, every
wardrobe value, with the rows each one paints.

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
