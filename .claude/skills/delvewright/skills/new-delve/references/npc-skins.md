# NPC skins — the face toolchain


Skip unless the design calls for a custom skin. The skin toolchain is a Python
package with dependencies, and `python3 --version` answering is not the same as
the package being importable — a missing skin is a build error, not a silent
skip. Make it a venv here so step 5 does not stop on it:

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

A cast entry says what the character is **wearing**, in a `wardrobe` block. The
palette says what colour each garment is; the wardrobe says whether the garment
is there at all and how far it reaches. Colours alone cannot put sleeves on a
character: leave the wardrobe out and you get the default costume — a
short-sleeved belted tunic, bare legs, sandals and a full beard — in whatever
colours you named, which is a Bronze-Age sailor no matter what the brief said.

```json
{
  "texture_id": "castle-guide",
  "model": "wide",
  "palette": {
    "skin": "#c89a74", "hair": "#5e4330",
    "tunic": "#2f4436", "tunic_shadow": "#1f3226", "belt": "#1b241d",
    "legwear": "#3b3f46", "legwear_shadow": "#292c31",
    "sandal": "#33251a", "eye": "#33473c"
  },
  "wardrobe": {
    "sleeves": "long", "legs": "full",
    "footwear": "boot", "facial_hair": "none"
  }
}
```

| key | values | what it does |
|---|---|---|
| `sleeves` | `bare`, `short`, `long` | `long` reaches the wrist and leaves the hand; `short` covers the upper arm only, so the forearm is bare |
| `legs` | `bare`, `short`, `full` | `full` is trousers to the ankle; `short` is a skirt over the upper thigh. Painted in `legwear`, which falls back to `tunic` |
| `footwear` | `none`, `sandal`, `shoe`, `boot`, `tall_boot` | how far up the leg it reaches — barefoot, sandal, shoe, mid-calf, knee. Painted in `sandal` |
| `facial_hair` | `none`, `moustache`, `beard` | `features.greying` streaks a beard grey |

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
contrasting one. Write the character so their look survives those limits, rather
than writing a look the toolchain has to fake.

**Look at the previews** before accepting a skin, and always set `model`
(`wide`/`slim`) — an omitted model renders slim and distorts a wide skin.
