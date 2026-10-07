# Step 9 — detail


Optional. A blockout is walkable and legible and made of concrete; detailing
replaces one place's massing with a real building, one place at a time — every
unbound box is still massed, so the map builds, walks and renders at every point
between none detailed and all of them.

`detail-plan.json` has two fields and **there is no coordinate in it and no way
to write one** — no region, no extent, no datum, no seam, no offset. A `place`
and a `piece` is all a row can say:

```json
{
  "palette": { "role/wall": "minecraft:stone_bricks" },
  "details": [
    { "place": "node/near-hall",
      "piece": "prefab/near-hall",
      "anchors": { "anchor/node-near-hall": "hearth" } }
  ]
}
```

**You do not write that row**: one verb writes it, from the program you wrote.
Where the piece goes is computed from the site plan's own box: the play space
plus the one floor course under it. The piece must be **exactly** that shape —
undersize is refused the same way oversize is (`DW0843`), because the box is the
footprint and a smaller building means a smaller box, which is a site-plan edit.

**Nothing here waits on a walk.** The user walks the detailed world at step 13,
so detail comes first and needs no walk record. A walk record written before
detail would name the blockout, which is not the build that ships.

1. **Read the allocation**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
   <campaign-dir> <place>`. It is what the whole hands the place — the frame's
   extents, the datum, every seam with its cells and the face class that answers
   it, the owed anchor names, the palette. **Read it; type nothing from it
   anywhere.** It is an input to nothing; ask again whenever you want it.
2. **Write the program** at `programs/<place stem>.json` inside the campaign.
   Declare the handed values you use as parameters under the `handed/` prefix,
   with the allocation's values as their defaults (`handed/datum-y`;
   `handed/seam/<edge stem>/x0` … `rise`); answer each seam with an opening at
   the handed cells and a contract edge to `exterior` whose `via` is that
   opening; answer each owed name with a `mark` of that stem
   (`anchor/node-annex` → `node-annex`); declare a spatial contract (`DW0843`).
   Iterate with `delvec --prefabs "$DELVEWRIGHT_PREFABS" grammar expand --file …
   --region <the frame>` and a render until it reads as the place. A piece the
   library already has, or one admitted through `delvec prefab` instead of
   written as a program, is *Reference: when the prefab library has no piece you
   need*.
3. **Run one verb**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" detail
   <campaign-dir> <place>`. It binds the handing, expands, runs every gate,
   writes the piece into the prefab directory and the row into
   `detail-plan.json`, and builds the whole to prove the map still walks. Read
   the verdict. A refusal names what to change in the program; **there is no
   number to edit anywhere else.** Each owed anchor name is re-bound to one of
   the piece's own anchors (`DW0845`) — that is what keeps the quest layer
   working, because those names were bound to places before any detail existed
   and detailing must never force a quest edit — and a way the plan did not
   allocate is refused in both directions (`DW0844`).
4. **After any plan or graph edit** — including one that answers a walk
   finding at step 13: `delvec --prefabs "$DELVEWRIGHT_PREFABS"
   detail <campaign-dir> --all`. Every program-detailed place is re-made; one
   that no longer fits is refused by name.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.
