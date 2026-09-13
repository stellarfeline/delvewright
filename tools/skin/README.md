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
per-delve resource pack at `assets/delvewright/textures/npc/<texture_id>.png`
(`pack_format` 75 for 1.21.11); the mannequin profile resolves
`delvewright:npc/<texture_id>` with an **always-explicit** `model`.

## Usage

```shell
python -m venv .venv && . .venv/bin/activate
pip install -r requirements.txt         # pinned skinpy-extended==1.0.1 (MIT)

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
| `skin`, `skin_shadow` | the body; `skin_shadow` is the hand, the knee and the brow |
| `hair` | the hair cap, fringe and back of the head |
| `beard`, `beard_grey` | facial hair; `features.greying` streaks it with `beard_grey` |
| `tunic`, `tunic_shadow` | the torso garment — tunic, jacket, coat, robe — and its sleeves |
| `belt` | a 2 px band low on the waist |
| `legwear`, `legwear_shadow` | the leg garment, and its knee shadow. Defaults to `tunic` / `tunic_shadow`, so a skirt cut from the same cloth needs no colour of its own |
| `sandal` | the footwear, whatever kind it is |
| `eye` | the two eye pixels |

### Wardrobe

What the character wears is **declared**, never inferred from the palette. Every
key is optional; the defaults are a short-sleeved belted tunic over bare legs,
sandals and a full beard — so a sheet that names no `wardrobe` composes exactly
what it composed before this block existed.

| key | values | reaches |
|---|---|---|
| `sleeves` | `bare`, `short` (default), `long` | `bare` leaves the arm bare to the shoulder; `short` is a 5 px sleeve on the upper arm; `long` reaches the wrist, leaving the hand |
| `legs` | `bare`, `short` (default), `full` | `bare` is a bare leg; `short` is a 2 px skirt over the upper thigh; `full` is trousers to the ankle |
| `footwear` | `none`, `sandal` (default), `shoe`, `boot`, `tall_boot` | 0, 2, 3, 6 and 9 px up a 12 px leg — barefoot, sandal, shoe, mid-calf boot, knee boot |
| `facial_hair` | `none`, `moustache`, `beard` (default) | `moustache` is the single row under the nose; `beard` adds the chin, the jaw and the chin underside |

```json
{
  "texture_id": "modern-guide",
  "model": "wide",
  "palette": { "tunic": "#2f4436", "legwear": "#3b3f46", "sandal": "#33251a" },
  "wardrobe": { "sleeves": "long", "legs": "full", "footwear": "boot", "facial_hair": "none" }
}
```

## Determinism (ADR-0006)

Same cast entry → **the same 64×64 image, on any machine**. All randomness flows
through one seeded `numpy` generator; Python's salted builtin `hash` is never
used. `tests/fixtures/golden/` pins the composed pixels of every fixture sheet.

**The image is portable; the PNG file is not**, and that difference is not this
tool's to close. Pillow hands the scanlines to whatever zlib it is linked
against, and deflate output differs between zlib builds. Measured with the same
Pillow 12.3.0 and numpy 2.5.3 on both sides, varying only zlib: macOS (1.2.12)
and Linux (1.3.1) compose **identical pixels** and write **different files** at
`compress_level` 1, 6 and 9 alike. Only `compress_level=0` agreed — 16516 bytes
against 1902, so a portable serialisation exists at 8.7× the file size.

This does not move a delve's bytes: the compiler bakes the PNG a creator
**committed**, and never recomposes it. It does mean two machines regenerating
one cast sheet produce one picture in two files, so compare **pixels**, not a
file hash.

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
