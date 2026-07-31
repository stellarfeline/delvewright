# prefabs/cave-generator — "Mediterranean cave/shore" tileset generator

Deterministic generator for the cave/shore prefab tileset (the *prefab-ceiling
probe*: can self-created prefabs reach showcase quality via a render-critique
loop?). A **sibling** of `prefabs/generator` (the stone-keep gen): its own
`[workspace]`, outside `crates/`, so it never enters the shipped `delvec` binary
and the keep `.nbt` output stays byte-identical (ADR-0006).

```sh
# from repo root — writes campaigns/prefabs/cave-*.nbt and cave-*.json
cargo run --manifest-path prefabs/cave-generator/Cargo.toml -- campaigns/prefabs/
```

Byte-identical on every run (double-run hash-checked). Three deterministic design
layers, all seeded from a per-piece PRNG + value noise (no wall clock, no unseeded
RNG, no hash-order iteration):

1. **Palette recipes** — weighted multi-block per surface role (wall, floor,
   ceiling, boulder), sampled through a **spatially-coherent value-noise field**
   so blocks cluster into patches (real rock strata / moss) instead of
   salt-and-pepper "tiled noise". A per-cell **edge-distance / height weathering
   bias** (algorithm A1) concentrates mossy + cracked variants near floors and
   walls — a wear gradient, not uniform speckle.
2. **Module grammar + organic shaping** — irregular 2-thick wall lining, a hearth
   (campfire + stone ring), timber sheep pens (fences/gate/hay), a boulder-sealed
   cave mouth. A **4-5-rule cellular automaton** (A8) perturbs the inner wall face
   into alcoves/bumps, and a **silhouette-roughening** pass (A5) erodes the roofline
   crown, chamfers the corners, and divots the outer faces — breaking the box
   outline while leaving every socket byte-identical (roughening only removes shell
   rock, never crosses the AABB, and skips doorway columns).
3. **Detailing / aging** — hanging dripstone stalactites, floor rubble mounds,
   glow-lichen / vine patches, and (shore) a **graded** sand → wet-gravel →
   sand-bottom shallow → stone-bottom sea that laps in from the open front (depth
   grows from the seabed dropping, so there is no vertical wall of water), with an
   irregular inlet tide line, rock scatter, driftwood and seagrass.

Techniques A1/A5/A8 are re-implemented from published description (ideas-only; no
third-party code ingested) — see `docs/ACKNOWLEDGEMENTS.md`.

Sockets reuse keep-socket geometry under the `cave:socket` vocabulary; the
compiler solver reads socket geometry only, so `pool/cave-shore` is a structural
drop-in for `pool/stone-keep`. Lighting is **derived** — a static flood-fill
block-light estimate over walkable floor cells sets `measured_min_light` and the
profile is classified from it (`lit` ≥7 / `dim` 3–6). Firelight pockets are
declared honestly, not hidden. See `../cave-tileset.md` for the piece list and
the render-critique round notes.
