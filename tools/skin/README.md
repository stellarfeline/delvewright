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
| `palette` | yes | `#rrggbb` colours: `skin`, `hair`, `beard`, `tunic`, `belt`, `sandal`, `eye`, … (missing keys derive a shade) |
| `seed` | no | integer; defaults to a stable SHA-256 of `texture_id` |
| `style_brief` | no | prose description → catalog card `description` |
| `role`, `features`, `hidden_layers` | no | catalog tags / passthrough metadata |

## Determinism (ADR-0006)

Same cast entry → **byte-identical PNG**. All randomness flows through one
seeded `numpy` generator; Python's salted builtin `hash` is never used. The
double-build gate is covered in `tests/test_skin.py`.

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
  rather than silently emit a distorted texture. Both `nobodys-cave` sailors are
  `wide`.
- Only the **base layer** is authored (no hat/jacket overlay); `skinpy-extended`
  addresses the base layer only.

## Attribution

[`skinpy-extended`](https://github.com/Bonenk/skinpy-extended) — MIT — see
`docs/ACKNOWLEDGEMENTS.md`.
