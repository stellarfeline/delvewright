# Step 13 — detail


Optional. A blockout is walkable and legible and made of concrete; detailing
replaces one place's massing with a real building. **Detail one place at a
time** — every unbound box is still massed, so the map builds, walks and renders
at every point between none detailed and all of them.

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
2. **`delvec --prefabs "$DELVEWRIGHT_PREFABS" allocation <campaign-dir> <place>`** — the frame's extents, the
   datum, every seam with the face class it must be answered by, and the owed
   anchor names. Build the piece against that and nothing else. It is an input
   to nothing; ask again whenever you want it.
3. **Build the piece** — a grammar program's export or a piece admitted through
   `delvec prefab`; the engine consumes the object, never the tool that made it.
   It must carry a spatial contract (`DW0843`), answer every seam, and open no
   way the plan did not allocate (`DW0844`, both directions). See *Reference:
   when the prefab library has no piece you need*.
4. **Bind it**, re-binding each owed anchor name to one of the piece's own
   anchors (`DW0845`). That is what keeps the quest layer working: those names
   were bound to places before any detail existed, and detailing must never
   force a quest edit.

Only when `details[]` binds every node does a declared vista stop being an
advisory and become a refusal (`DW0821`) — by then there is nothing left to
carve.
