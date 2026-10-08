# `delve-skin` — NPC skin toolchain (spec-0009)

Given a **cast-sheet entry** (character brief + palette + `wide`/`slim` model),
compose an **original 64×64 Minecraft player skin** deterministically — both
layers: the base, and the overlay shell over it — and render headless
multi-angle previews for human review. The same wardrobe dresses a mob whose
body is the player's size (a zombie, a drowned, a drowned's outer layer), drawn
to that mob's own boxes (spec-0097).

Skins are **original artwork composed pixel-by-pixel** from the brief
(ADR-0013) — never downloaded from skin sites (those are unlicensed user
uploads). There is no scavenging track for skins (spec-0009).

## Pipeline

```
cast sheet ──▶ compose (skinpy-extended part/face addressing,
                         over the model's own boxes) ──▶ PNG
                                   │
                                   ├──▶ preview: 4 iso 3/4 views (front/left/right/back)
                                   └──▶ catalog card + provenance (license: original)
```

The compiler bakes the PNG from `campaigns/<id>/skins/<texture_id>.png` into the
per-delve resource pack at `assets/delvewright/textures/npc/<id>/<texture_id>.png`
(`pack_format` 75 for 1.21.11); the mannequin profile resolves
`delvewright:npc/<id>/<texture_id>` with an **always-explicit** `model`. The
delve's own directory is the compiler's — a client merges every applied pack's
textures into one space, so two delves that both ship a `keeper` would otherwise
wear each other's faces. Nothing here writes it: a cast sheet names the skin, the
campaign names the file, and the delve that ships it decides where it lands.

## Usage

```shell
python -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt         # pinned skinpy-extended, numpy, Pillow

# compose + preview + catalog card for every entry in a cast sheet
python -m delve_skin all cast.json \
  --skins-dir   out/skins \
  --preview-dir out/previews \
  --catalog-dir out/catalog

# or one stage at a time (optionally a single --id)
python -m delve_skin build   cast.json --out-dir out/skins
python -m delve_skin preview cast.json --out-dir out/previews --id eurylochus
python -m delve_skin catalog cast.json --out-dir out/catalog

# any model's boxes, base and shell, with every face rectangle
python -m delve_skin parts drowned_outer_layer
```

## Each model's boxes

Every sheet is drawn to the boxes of the model that wears it, read from the
model-part table the compiler judges every sheet by
(`crates/delvec/data/model-parts-1.21.11.json`, measured from the pinned client
by `tools/maintenance/extract-model-parts.py`). A part has a **base** box and,
on most models, a **shell** over it: the same box grown half a pixel a side on
the head and a quarter on the torso and limbs. A player-model mannequin has a
shell on every part (`hat`, `jacket`, both sleeves, both pants). A zombie or a
husk has only the hat. A drowned's, stray's or bogged's outer layer is a sheet
of its own whose boxes sit at the **base** positions, grown — on such a model
the composer paints the shell features onto those boxes and leaves the base
empty. `python -m delve_skin parts <model>` prints any model's table, so a sheet
drawn by hand for a villager, a piglin or a skeleton is drawn to its own layout.

The composer never paints a pixel outside a model's boxes; the compiler refuses
a sheet that does.

## Cast sheet

`{"campaign": "...", "skins": [ <entry>, ... ]}` (or a bare list). Each entry:

| field | required | meaning |
|---|---|---|
| `texture_id` | yes | kebab id; PNG basename and resource-pack texture segment |
| `entity` | no | the body: `mannequin` (default), or a mob model whose head, torso and limbs are the player's size — `zombie`, `husk`, `drowned`, `drowned_outer_layer`, `stray_outer_layer`, `bogged_outer_layer` (`--help` lists them from the table). Any other model is refused by name |
| `model` | **yes**, for a mannequin | `wide` or `slim`. **Never omit** — an omitted model renders slim, distorting a wide skin (spec-0009). A mob has one model, and refuses this field. |
| `palette` | yes | `#rrggbb` colours — see below (a missing key derives a shade) |
| `wardrobe` | no | how the character is dressed — see below (an absent block dresses them in the defaults) |
| `seed` | no | integer; defaults to a stable SHA-256 of `texture_id` |
| `style_brief` | no | prose description → catalog card `description` |
| `hidden_layers` | no | a mannequin's overlay layers it does not draw — `cape`, `jacket`, `left_sleeve`, `right_sleeve`, `left_pants_leg`, `right_pants_leg`, `hat` (the model's own left and right); each at most once. Carried to the catalog card; the campaign's `skin.hidden_layers` is what the mannequin is summoned with |
| `role`, `features` | no | catalog tags / passthrough metadata |

An unknown entry field, palette key, wardrobe key or wardrobe value is **refused
by name**: a misspelled `wardrobe` would otherwise compose the default costume
and say nothing. `python -m delve_skin <cmd> --help` prints the whole surface,
enumerated from the constants the parser validates against.

### Palette

| key | paints |
|---|---|
| `skin`, `skin_shadow` | the body. `skin_shadow` is the hand, the knee, the eyebrows, the shadow under the fringe and the inner step of the jaw taper; the mouth and the chin's outer corners are half a step further along the same two colours, so a dark skin gets a shallow mouth from the same numbers |
| `hair` | the crown, the back of the head, the fringe and however far down the sides the `hair` axis says |
| `hair_shadow` | the cut line at the lower edge of hair long enough to show one |
| `hair_grey` | the grey coming into the hair, when `greying` names it |
| `beard`, `beard_grey` | facial hair, and the grey coming into it |
| `tunic`, `tunic_shadow` | the torso garment — tunic, jacket, coat, robe — and its sleeves |
| `belt` | a 2 px band low on the waist |
| `legwear`, `legwear_shadow` | the leg garment, and its knee shadow. Defaults to `tunic` / `tunic_shadow`, so a skirt cut from the same cloth needs no colour of its own |
| `sandal` | the footwear, whatever kind it is |
| `eye` | the pupils — one pixel each, with a lightened `skin` pixel outboard standing in for the white |
| `hood`, `hood_shadow` | a hood, and its rim round the face. `hood` defaults to `tunic` |

### Wardrobe

What the character wears and how they are groomed is **declared**, never inferred
from the palette. Every key is optional; the defaults are a short-sleeved belted
tunic open at the throat over bare legs, sandals, a short back and sides and a
full beard — so a sheet that names no `wardrobe` composes exactly what it
composed before this block existed.

| key | values | reaches |
|---|---|---|
| `sleeves` | `bare`, `short` (default), `long` | `bare` leaves the arm bare to the shoulder; `short` is a 5 px sleeve on the upper arm; `long` reaches the wrist, leaving the hand |
| `legs` | `bare`, `short` (default), `full` | `bare` is a bare leg; `short` is a 2 px skirt over the upper thigh; `full` is trousers to the ankle |
| `footwear` | `none`, `sandal` (default), `shoe`, `boot`, `tall_boot` | 0, 2, 3, 6 and 9 px up a 12 px leg — barefoot, sandal, shoe, mid-calf boot, knee boot |
| `hair` | `bald`, `crop`, `short` (default), `jaw`, `long` | how far hair comes down the 8 px sides of the head: none, 2, 3, 6 and 8 rows. The crown, the back of the head and the brow fringe come with every length. Past the ear it also **frames the face** down its outer columns and takes a cut line in `hair_shadow`; `long` falls across the top of the torso back as well |
| `facial_hair` | `none`, `moustache`, `beard` (default) | `moustache` is the lip row; `beard` adds the mouth and chin rows, the sides of the jaw and the chin underside. `none` is a modelled face, not a blank one — see [the face at 8×8](../../../docs/reference/face-craft.md) |
| `collar` | `open` (default), `closed`, `high` | `open` leaves the V of bare skin a tunic or an unbuttoned shirt has at the throat; `closed` takes it away, which is the only way to get a jacket that fastens — the V is painted from `skin` itself, so no palette key can reach it; `high` is `closed` with a collar ring on the torso's shell, its top two rows all the way round. A model with no torso shell refuses `high` |
| `hood` | `none` (default), `up` | `up` covers the head's shell but for the face — the brow row and the outer columns frame it, the face rows stay open — and falls onto the torso shell's top and upper back. It replaces the hair's shell; the hair painted on the skull still shows round the face |
| `greying` | `none` (default), `hair`, `beard`, `both` | streaks `hair_grey` / `beard_grey` through whatever it names. `features.greying` is the older spelling of `beard` and still means exactly that; a sheet carrying **both** is refused rather than resolved by a precedence rule |

```json
{
  "texture_id": "castle-steward",
  "model": "wide",
  "palette": {
    "tunic": "#39495e", "legwear": "#3a3b41", "sandal": "#2c2521",
    "hair": "#7d7669", "hair_grey": "#a9a29a", "hair_shadow": "#564f42"
  },
  "wardrobe": {
    "sleeves": "long", "legs": "full", "footwear": "shoe",
    "hair": "jaw", "facial_hair": "none", "collar": "closed", "greying": "hair"
  }
}
```

### The shell

After the whole base is painted, the composer paints the shell — so a sheet's
base is the base it composed before the shell existed:

- **beard** — the hat's front at the chin, mouth and lip rows, its sides at
  those rows, its whole underside; a **moustache** is the lip row. The base
  beard stays, so the face reads where the shell is clear.
- **hair** — the hat's top and back, its sides down to the `hair` length, a
  fringe lip on the front's top row, the outer columns for a length that frames
  the face; `long` also falls on the jacket's back at the shoulder rows.
- **hood** and **collar: high** — as the wardrobe table says.

The shell is a shell: it stands off the base by half a pixel on the head and a
quarter on the body and cannot grow, so a brim, a crest, a bun, a braid, a
ponytail, a cloak or a coat that hangs open have no geometry to be drawn on. A
creator who wants a flat head hides `hat` on the mannequin.

### The face

The 8×8 front of the head is painted from the chin up, not from the crown down:
chin `y=0`, mouth `y=1`, lip `y=2`, eyes `y=3`, eyebrows `y=4`, forehead `y=5`,
the fringe's shadow `y=6`, the fringe `y=7`. Every row is a measurement of the
nine default player skins the pinned client ships — the derivation, the counts
and their denominators are in
[the face at 8×8](../../../docs/reference/face-craft.md). Nothing here is a knob:
a cast entry chooses colours and grooming, and the composer paints the face.

There is no nose, because no clean-shaven default skin has one and at this size
a nose merges with the mouth below it into a muzzle. A clean-shaven character
gets a mouth, a chin and a jaw that narrows toward it — so a beard is a choice
about the character and not the only thing keeping a head from reading as a jaw.

## Determinism (ADR-0006)

Same cast entry → **the same skin PNG file, byte for byte, on any machine**.
Compare skins by **file sha256**; that is the comparison this tool promises.

- **Pixels.** All randomness flows through one seeded `numpy` generator; Python's
  salted builtin `hash` is never used. numpy is pinned beside skinpy-extended,
  because NEP 19 does not hold a `Generator` method's stream stable across
  numpy versions.
- **File.** `delve_skin/png.py` writes the PNG itself: IHDR, one IDAT, IEND, no
  ancillary chunk, filter 0 on every row, and a zlib stream of **stored**
  deflate blocks. Every byte is fixed by the PNG specification and RFCs
  1950/1951 given the pixels; no compressor is called, so the machine's zlib
  never reaches the file. A 64×64 skin is 16516 bytes.
- **Why not Pillow's encoder.** Pillow's manylinux wheels link the system
  `libz.so.1`, and its macOS and Windows wheels carry zlib-ng: the same Pillow
  12.3.0 writes one picture as different files on macOS and on Linux, so pinning
  Pillow cannot make the file portable.

`tests/fixtures/golden/` holds the composed file of every fixture entry, and the
suite compares it as bytes (and as pixels, so a red says which moved). The
compiler bakes the committed PNG into the resource pack as it is, so the
committed bytes are the shipped bytes.

**Previews are review images, not artifacts**: they are written by Pillow and
their bytes are stable on one machine only. Nothing commits or ships them. A
preview projects the base cubes with each shell's opaque pixels laid over the
face beneath — what the shell covers, seen from outside; the half-pixel
stand-off is not drawn. A mob sheet is previewed on the player's figure.

## Why not headless skinview3d for previews?

spec-0009 anticipated a "skinview3d-lineage, Node" preview renderer.
`skinview3d` is **browser-only** (three.js/WebGL); headless rendering needs a
fragile native GL stack whose output varies across GPU drivers — the opposite of
the "produced deterministically" acceptance criterion. `skinpy-extended` already
ships a **pure-Python orthographic isometric renderer** with no GPU dependency,
so previews are deterministic and CI-portable. We adopt it for both composition
and preview and **did not** add a skinview3d/WebGL dependency. (Nucleation, the
prefab renderer, cannot render player models — do not use it here.)

## Limitations

- **`slim` geometry** is validated and emitted as metadata but not yet composed:
  the wide-only `skinpy-extended` layout would distort it. A `slim` entry raises
  rather than silently emit a distorted texture.
- **Nothing stands proud of the body by more than the shell.** An open coat, a
  hat with a brim, a cloak, a beard that juts and hair with volume need model
  geometry, which no skin has, and are refused rather than approximated.
- **Hair is a lip of paint half a pixel off the skull.** A bun, a braid, a
  ponytail, a fringe that falls, a parting and any silhouette that is not the
  cube do not exist. `long` reads at playing distance, and it is not the same
  thing as hair.
- **A figure cannot be made to read as a woman.** On a player model that is the
  `slim` arm geometry (unsupported here) and face detail finer than the 8×8 the
  head gives; the shell adds depth, not a silhouette, so hair length on a cube
  stays androgynous. Write the character so the writing carries it.
- **Mobs of another shape** — villager, piglin, the skeleton family — have a
  part table (`parts`) and the compiler's refusal, not a wardrobe: their heads
  and limbs are not the player's size.
- **A limb is 4 px around and a torso 8 px.** A lapel, a cuff, a buckle or a seam
  narrower than a pixel does not exist, and a belt is the finest horizontal band
  there is at 2 px on a 12 px torso.
- **A garment cannot cross a body part.** Arms, legs and torso are separate
  boxes: a sleeve length and a torso hem are independent, there is no shoulder
  seam to align, and no skirt hangs past the hips.
- Sleeves take the **torso garment's** colour. A jacket with contrasting sleeves
  would need a palette key of its own, and has none.

## Attribution

[`skinpy-extended`](https://github.com/Bonenk/skinpy-extended) — MIT — see
`docs/ACKNOWLEDGEMENTS.md`.
