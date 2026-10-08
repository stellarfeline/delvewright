# `delvec::compiler::atmosphere`

The reference page for `crates/delvec/src/compiler/atmosphere.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0928–DW0930 — a place's own sky (`compiler::atmosphere` + `compiler::horizon`; error; exit 1 / exit 3)

| Code | Meaning |
|------|---------|
| `DW0928` | **An attribute line the pinned game does not accept here** (spec-0080 §6.1), validation tier, at the world document. Its shapes: an id the pinned registry does not hold (the nearest ids by edit distance are named); an id the overworld day cycle overrides (`sun_angle`, `moon_angle`, `star_angle`, `sunrise_sunset_color`, `moon_phase`), with what the party would see instead and that the vanilla remedy is world-wide; a `gameplay/` id, with the reason (`monsters_burn` stacks by `or`, so a biome can never stop daylight burning; `sky_light_level` is read by three proofs); a value not in the id's shape — a number for a colour, `#rrggbb` where `#aarrggbb` is read, a particle list entry without `probability`, an unknown modifier; a value outside the range the codec rejects outside of (`AttributeRange` on the attribute, `Codec.floatRange` / `ExtraCodecs.NON_NEGATIVE_INT` on a record field — read from the jar into the vendored table; where the codec bounds nothing, nothing is bounded); a sound id the pinned `sound_event` registry lacks (the one `DW0326` reads); a particle type the pinned `particle_type` registry lacks, or one that takes options; and a tint colour that is not `#rrggbb`. A biome that fails its codec stops the world from opening, which is why this refuses at the document. |
| `DW0929` | **A paint that reaches cells it may not** (§6.2). At the document: a `set-atmosphere` naming neither or both of `region` / `place`. At the build (exit 3, before any model): two carried places whose painted 4-cells meet under different atmospheres (a shared cell would belong to whichever `fillbiome` ran last — an order, not a declaration); and a repaint volume with a column outside the map's extent — the rectangle `plan::surround_rect` reads when the campaign states one, else the placed pieces' footprints, which are the columns world setup force-loads — or a y outside the build height. `fillbiome` into a chunk nothing loads is a silent no-op. |
| `DW0930` | **An atmosphere declared against itself or against nothing** (§6.3), validation tier: one no area or site-plan box carries and no `set-atmosphere` paints (it would ship as a biome no cell is painted with); a `climate` whose temperature contradicts `precipitation` (snow at or above 0.15, rain below it); a duplicate `id`. |

A dangling `atmosphere/<id>` (on an area, a box or a repaint) or an unknown `place` is the ordinary `DW0112`. Every move each message names is taken in `crates/delvec/tests/remedy_reachability.rs`.
