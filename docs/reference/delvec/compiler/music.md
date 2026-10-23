# `delvec::compiler::music`

The reference page for `crates/delvec/src/compiler/music.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1031–DW1032 — a scene states its music (`dsl::music` / `compiler::music`; spec-0109; error)

The pinned client chooses music from one thing, the biome under the camera, and a biome that declares none inherits the overworld dimension's own calm overworld music. Every biome a camera can stand in — each atmosphere and the ground (`horizon`) — therefore states a track or silence (`DW1031`, on the [`delvewright_dsl::music` page](../../dsl/music.md#dw1031dw1032--a-scene-states-its-music-dslmusic--compilermusic-spec-0109-error)), and a cutscene may declare what it hears so the build confirms its camera never leaves that music (`DW1032`). A sound id is `DW0326`, an attribute value outside its pinned range `DW0928`, a dangling atmosphere reference (`hears` included) `DW0112`, an unknown category or enum word `DW0100`: no second code for a rule that exists.

| Code | Meaning |
|------|---------|
| `DW1032` | **A cutscene that hears what it did not declare.** A cutscene with `hears` (an `atmosphere/<id>`, or `null` for the ground) whose camera passes a 4-cell wearing another biome in the configuration the forced route reaches it in — the biome map after every `set-atmosphere` the route fires before it, in route order; the first tick's map for a cutscene the route never reaches. The cells judged are each shot's waypoints and every 4-cell the straight segments between them cross, for a `path` shot and for the dolly a `shot_style` expands to, read at each 4-cell's 2-mod-4 sample. Build tier (exit 3), `compiler::music::measure`. The message names the cutscene's path, the shot, the cell, the biome it wears, the one declared and the configuration. Prescription: the shot's path (keep it inside the declared place), the place's atmosphere (carry the declared one on what the camera crosses), or a repaint before the beat (`set-atmosphere`). No advisory for a cutscene without `hears`: it has declared nothing, and the music ledger reports what its camera hears without judging it. |

Every move each message names is taken in `crates/delvec/tests/remedy_reachability.rs`.
