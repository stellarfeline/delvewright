# Tier-2 / methodology capability probe — artifact index

Sandbox only. The engine repo was used **read-only** (`delve-schem`, `delve-render`,
and `delvewright-render` as a path dependency); nothing in it was modified.

## Pipeline

```
programs/<name>.js                 build program (block/box/line/rng, no block dumps)
  -> minebench/scripts/dw-run-build.ts       MineBench (MIT) voxel.exec runtime +
                                             validator + Sponge .schem exporter
  -> builds/<name>.schem                     Sponge v3
  -> delve-schem convert --split 512         -> builds/<name>.nbt  (DataVersion 4671)
  -> delve-render piece                      4 corner isos + top-down
  -> dw-shot                                 front elevation / side / hero angles
```

Single recorded seed for every build: **121111**. Grid 256³, palette `advanced`
(80 block ids). Driver: `pipeline.py`. Conditions: `programs/CONDITIONS.md`.

## Builds

| # | Target | A (baseline) | B (methodology-primed) |
|---|---|---|---|
| 1 | Temple of Heaven, Hall of Prayer | `A1-tiantan-baseline` | `B1-tiantan-methodology` |
| 2 | Greek Doric peripteral temple | `A2-greek-baseline` | `B2-greek-methodology` |
| 3 | Colossal ruined stone bridge | `A3-bridge-baseline` | `B3-bridge-methodology` |

Each has `builds/<name>.{call.json,schem,nbt,expanded.json,stats.json}` and
`renders/<name>/<name>-{ext-ne,ext-se,ext-sw,ext-nw,top,front,side,hero}.png`.

## Contact sheets (target x condition, A left / B right)

- `contact-sheets/sheet-1-perspective.png`   — 3/4 perspective, yaw 115 / pitch 14
- `contact-sheets/sheet-2-front-elevation.png` — yaw 0 / pitch 0
- `contact-sheets/sheet-3-isometric.png`     — yaw 45 / pitch 30

## Machine metrics

`metrics.py` -> `metrics.json` / `metrics.txt`: silhouette fill ratio and perimeter
complexity, the squint (low-pass) value spread, palette histogram with top-1 /
top-3 / accent shares, and exposed-faces-per-block as a matchbox proxy.

## Preserved failure evidence (do not delete — this is the finding)

- `renders/B1-tiantan-methodology-round1/` + `programs/_B1-round1-splatter.js.txt`
  — gradient rule applied with 7/4-block noise cells: read as splatter.
- `renders/B2-greek-methodology-round1/` + `programs/_B2-round1-6030-10-violation.js.txt`
  — weathering ramp produced ~33/33/33 while the file's own self-check claimed 60/30/10.
- `renders/A3-bridge-baseline-round1/` + `programs/_A3-round1-terrain-tray.js.txt`
  — chasm read as a rectangular stone tray; round 2 walls occluded the whole bridge.
