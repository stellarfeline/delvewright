# Step 9 — detail


Required before hand-over, for **every** place: the staging gate refuses a
site-plan build with no detail plan, and one whose derivation still stands a
stand-in in any place — a stand-in never ships. Places are detailed one at a
time. Until a place is detailed the derivation masses it with a stand-in —
walkable, legible, made of concrete; detailing replaces that place's stand-in
with a real building — so the map builds, walks and renders at every point
between none detailed and all of them, and the build records every place still
massed in `validation/blockout.json`.

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
Where the piece goes is computed from the site plan: the **frame** is the
place's claim — the ground under its plot, its floor course, its play space,
its one-cell ring and, roofed, its lid and roof zone — as a bounding box. The
piece must be **exactly** that shape — undersize is refused the same way
oversize is (`DW0843`), because a smaller building means a smaller box, which is
a site-plan edit. Every cell of the frame the place does not own — a
neighbour's wall, the ring's fixed ground, nobody's — is a **void**: the verb
writes `minecraft:structure_void` there, and a piece that paints a void (air
included) is `DW0987`, one that writes the ring's fixed ground `DW0990`.

1. **Read the allocation**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
   <campaign-dir> <place>`. It is what the whole hands the place — the frame's
   extents, the datum, the ground it stands on (or none, aloft), the roof zone,
   its neighbours, every seam with its opening — `cells` is an inclusive box
   written as its two opposite corner cells, and every cell between them is in
   the opening and must be left open — and its `form` and the face class that
   answers it, the owed anchor names, every void with its owner, the palette. **Read it; type nothing from it
   anywhere.** It is an input to nothing; ask again whenever you want it.
2. **Write the program** at `programs/<place stem>.json` inside the campaign.
   Declare the handed values you use as parameters under the `handed/` prefix,
   with the allocation's values as their defaults (`handed/datum-y`;
   `handed/ground/{min-y,max-y,bottom-y}`; `handed/roof/…` on a declared roof;
   `handed/seam/<edge stem>/{x0,y0,z0,x1,y1,z1,rise}`); answer each seam with
   an opening at the handed cells and a contract edge to `exterior` whose `via`
   is that opening; answer each owed name with a `mark` of that stem
   (`anchor/node-annex` → `node-annex`) — and a `barred` seam whose plane this
   place owns with the contract edge's own `bar` over exactly the seam's
   cells, its region named by the same stem (`anchor/seam-west-door` →
   `"region": "seam-west-door"`), which exports as the gate anchor that name
   binds to (`DW0845`); declare a spatial contract (`DW0843`). **The enclosed spaces are
   yours to declare**, and the closure gate confirms exactly those: a street, a
   pavilion or a covered market declares none and passes. A two-level interior
   is two spaces (a space is one floor) and a `climb` contract edge between
   them, at program version `1.10.0`, proved over the body's climb moves
   (`$DELVEWRIGHT_ENGINE/docs/reference/grammar.md`, the contract's edge
   classes). A scenery place (`"reached": false`) is judged sealed: no way in,
   no floor or light owed.
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
4. **After any plan or graph edit** — including one that answers a playtest
   finding in a later round: `delvec --prefabs "$DELVEWRIGHT_PREFABS"
   detail <campaign-dir> --all`. Every program-detailed place is re-made; one
   that no longer fits is refused by name.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.
