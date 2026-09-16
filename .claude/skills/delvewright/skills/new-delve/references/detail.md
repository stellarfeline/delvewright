# Step 13 — detail


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
footprint and a smaller building means a smaller box, which is a site-plan edit
and another walk.

1. **Record the walk the user did at step 9.** The `verdict` is theirs, not
   yours. Write `walk-record.json` beside the
   documents — `delvec schema --stage walk-record` is its shape. It is a
   campaign artifact rather than a stage document, so it carries no
   `dsl_version`, no `campaign_id` and no `stage`. Fill it with the three hashes
   every site-plan build prints (`site_plan_sha256`, `layout_graph_sha256`,
   `blockout_sha256`), the engine revision printed beside them, the verdict, and
   whatever the walk noted. **Copy all four out of the build output rather than
   computing them.** Nothing about detail compiles without it (`DW0841`),
   including asking for an allocation. **The first two hashes are
   the record's freshness key**: the whole a walk judges is derived from the
   plan AND the graph, so editing either one — even an edit that moves no block,
   such as which side a barred way opens from — re-opens this gate and asks for
   another walk.

   **`verdict` is one of three and you transcribe it, you never choose it.**
   `passed` — they walked it and it is fit to detail; the only value that opens
   this step. `findings` — they walked it and something must change first.
   `unwalked` — **nobody walked it**: abandoned, cut short, or a build stood up
   and taken down. Reach for `unwalked` for every one of those; it is the only
   value that does not assert a walk, and `DW0841` refuses on the field, so the
   truth stops detail by itself. Putting it in `findings[]` instead does not:
   that list is prose, and no check reads prose.
2. **Read the allocation**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation
   <campaign-dir> <place>`. It is what the whole hands the place — the frame's
   extents, the datum, every seam with its cells and the face class that answers
   it, the owed anchor names, the palette. **Read it; type nothing from it
   anywhere.** It is an input to nothing; ask again whenever you want it.
3. **Write the program** at `programs/<place stem>.json` inside the campaign.
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
4. **Run one verb**: `delvec --prefabs "$DELVEWRIGHT_PREFABS" detail
   <campaign-dir> <place>`. It binds the handing, expands, runs every gate,
   writes the piece into the prefab directory and the row into
   `detail-plan.json`, and builds the whole to prove the map still walks. Read
   the verdict. A refusal names what to change in the program; **there is no
   number to edit anywhere else.** Each owed anchor name is re-bound to one of
   the piece's own anchors (`DW0845`) — that is what keeps the quest layer
   working, because those names were bound to places before any detail existed
   and detailing must never force a quest edit — and a way the plan did not
   allocate is refused in both directions (`DW0844`).
5. **After any plan edit and re-walk**: `delvec --prefabs "$DELVEWRIGHT_PREFABS"
   detail <campaign-dir> --all`. Every program-detailed place is re-made; one
   that no longer fits is refused by name.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.
