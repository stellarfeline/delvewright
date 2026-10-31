# A town — laying out a settlement, and drawing its places

## Contents

- [The checklist](#the-checklist)
- [The helpers that draw it](#the-helpers-that-draw-it)
- [What stays yours](#what-stays-yours)
- [When a plan moves under a drawn place](#when-a-plan-moves-under-a-drawn-place)

A town is a site plan like any other map (*The map is a site plan* on the
page): many boxes, the seams that join them, one piece per place. Use this
page at step 2, when the plan lays out the settlement, and hand its checklist
to every place's author at step 9. Each line's provenance — a ruling, a cited
source, or a rule reasoned here — and its sources are
`$DELVEWRIGHT_ENGINE/docs/reference/place-craft.md` §10, line for line.

## The checklist

Go through it at step 2 and again before you call the map done.

1. **Disguise the island, dress its edge.** The playable side faces the sea and the far side returns the player; the region reaches as far past the walk region as a player at its edge can see (`seen_region(walk, view_distance)`), the land runs on past the walk region and falls to the sea in headlands and coves, and a summit castle stands on its mountain in steps of rock, not on a flat table.
2. **Give every street a box and build it.** Paving with a gutter down the middle, worn patches, steps with landings, and on the verge lamps, benches, barrels, carts and flowers — nothing in the clear width, nothing in front of a doorway.
3. **No line where a box ends.** Walls come from buildings and yards, never from plot edges; paving runs to the edge; no edge column stands over air; no eave hangs past the play space.
4. **Fronts stand on the street.** A lot that fronts its street meets it only at its doorway span; the rest of its edge up to the street's air is the building's front or a yard wall; the trade's sign hangs on the street's bracket and the campaign letters it.
5. **Set buildings back; aim windows at open space.** Keep buildings off the plot's and the map's edges; a window looks onto a yard, a street or the sea.
6. **Several rooms inside.** A first room (shop, reception), the work room behind it through a door; outlines, porches and open fronts are free.
7. **Life outside.** Bushes, pots, small ornaments, yards.
8. **References per trade.** Each building from community builds of its own theme, listed in its node's `references[]`.
9. **One style, the community's scale.** One palette family and one scale across the town.
10. **Interior frames from a corner.** Standing eye height, across the room, field of view about 90°.
11. **Look at the whole map four ways.** From the air, from the summit, from the sea, at eye height down the main street.
12. **Count the places without a program.** `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation <campaign> --all` less `<campaign>/programs/*.json`, each named; the map is done at zero.

## The helpers that draw it

Every helper below is in `$DELVEWRIGHT_SKILL/scripts/town.py` and is imported
from the starter like `Model` and `run` (never copied):

    from starter import Model, run, stretch, street_seam, street_front

Each takes what you state — seed, edge, roles, what stands where — and reads
nothing from a place's name; what the plan hands is never stated: a way
place's run axis, clear width and clear columns come from `place.way` (the
allocation's `way`), and a value you state that differs is refused. No
default thing a helper stands is a container the holder census counts
(`DW1018`): a barrel stands only where you name one. A helper that stands a
lamp, a bracket or an awning on the frame's top course declares `up` shown
itself. `flight` and `ramp` tell the starter they are the way up to the
opening above (`Model.climbs_to`), so no ladder stands in their steps. A
cart (`handcart`) is two upside-down stairs back to back, resting on its own
ends; nothing a helper stands shows half a course of air under itself, which
the build reports as `DW1076` wherever a drawing does. `--help`-level detail
is each function's docstring.

| need | helpers |
|---|---|
| a main street's stretch, its verge's life, a market | `stretch(m, place, seed=…, roles=…, fronts=…, verge=…)`, `verge_life(m, frame, things=…, every=…, at=[(kind, row, "low"\|"high"), …])` (returns its placements), `market` |
| a side street, a flight of steps, a climbing road, a verge rail | `lane(m, place, edge=…)`, `flight(m, place, roles, lamp_side=…)`, `ramp(m, place)`, `rail(m, place, axis, edge, seed, block, lamp_at=…)` |
| paving, a square's paving and paths | `pave_way`, `square` |
| lamps, a front's sign on the street's bracket | `lamp`, `bracket`, `doorway_verge`, `seam_cols`, `way_frame`, `hsh` |
| a lot's street edge | `street_seam(place, face)`, `street_front(m, edge, lo, hi, doorway, air, roles, axis=…)`, `Model.shift(dx, dz)` to stand a building drawn at its own coordinates on the edge |
| a lot's other edges | `clip_beyond`, `clip_unwalled_edge`, `close_edge_gaps` |
| small buildings and furnishing | `post_shed`, `loft`, `ring`, `window`, `flower_bed`, `handcart` |
| boats and quays | `hull`, `hull_on_stocks`, `mast`, `pier`, `slipway`, `water_step` |
| the plan's side | `front_contact(at, meets, first_row, width, height)` for a front's seam, `sign_station(street, lot, text)` for the street's station and the `signs` entry, `seen_reach`, `seen_region` |

## What stays yours

The palette and its roles, which trade gets which front and which sign wood,
what stands on a verge and in a square, the coastline's shape, the castle's
bank: all design, written in the campaign's own generator and plan script.

## When a plan moves under a drawn place

Every program the starter writes carries the order its cuts were emitted in as
the guard of its start rule. When a plan edit moves a handed value past a
literal the program was cut against, `detail` refuses with `no alternative of
rule "place" applies … required (param:handed/… ) > N`. Re-run that place's
generator against the new allocation; nothing in the program is edited by hand.
