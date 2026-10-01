# `delve-skin` — NPC skin toolchain (spec-0009)

Given a **cast-sheet entry** (character brief + palette + `wide`/`slim` model),
compose an **original 64×64 Minecraft player skin** deterministically and render
headless multi-angle previews for human review.

Skins are **original artwork composed pixel-by-pixel** from the brief
(ADR-0013) — never downloaded from skin sites (those are unlicensed user
uploads). There is no scavenging track for skins (spec-0009).

## Pipeline

```
cast sheet ──▶ compose (skinpy-extended part/face addressing) ──▶ 64×64 PNG
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
```

## Cast sheet

`{"campaign": "...", "skins": [ <entry>, ... ]}` (or a bare list). Each entry:

| field | required | meaning |
|---|---|---|
| `texture_id` | yes | kebab id; PNG basename and resource-pack texture segment |
| `model` | **yes** | `wide` or `slim`. **Never omit** — an omitted model renders slim, distorting a wide skin (spec-0009). |
| `palette` | yes | `#rrggbb` colours — see below (a missing key derives a shade) |
| `wardrobe` | no | how the character is dressed — see below (an absent block dresses them in the defaults) |
| `seed` | no | integer; defaults to a stable SHA-256 of `texture_id` |
| `style_brief` | no | prose description → catalog card `description` |
| `role`, `features`, `hidden_layers` | no | catalog tags / passthrough metadata |

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
| `collar` | `open` (default), `closed` | `open` leaves the V of bare skin a tunic or an unbuttoned shirt has at the throat; `closed` takes it away, which is the only way to get a jacket that fastens — the V is painted from `skin` itself, so no palette key can reach it |
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
their bytes are stable on one machine only. Nothing commits or ships them.

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
- Only the **base layer** is authored (no hat/jacket overlay); `skinpy-extended`
  addresses the base layer only. So **nothing can stand proud of the body**: an
  open coat, a hood, a hat with a brim, a cloak, a beard that juts and hair with
  volume all need the overlay layer or model geometry, and are refused rather
  than approximated into a paint job that reads as none of them.
- **Hair is paint on the skull.** A bun, a braid, a ponytail, a fringe that
  falls, a parting and any silhouette that is not the cube do not exist. `long`
  is hair-coloured paint down the sides of the head and across the top of the
  torso back: it reads at playing distance, and it is not the same thing as hair.
- **A figure cannot be made to read as a woman.** The three things that do it on
  a player model are the `slim` arm geometry (unsupported here), a hair
  silhouette off the cube (the overlay layer), and face detail finer than the
  8×8 the head gives. What is left is hair length, which on a cube is
  androgynous. Write the character so the writing carries it.
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
