# `delvec::compiler::view::world`

The reference page for `crates/delvec/src/compiler/view/world.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW044x — command-driven trap payloads (spec-0022)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../nav.md#dw044x--command-driven-trap-payloads-spec-0022).

| Code | Meaning |
|------|---------|
| `DW0955` | `tools/ci/check-written-world.py` | **The world the engine writes is not the world the pinned server builds** (spec-0089 §5.4; declared as `compiler::view::world::DW_CAMERA_STEP_WORLD`, raised by the gate, exit 1). The written load world (`delvec written-world <campaign> -o <dir>`, from the campaign and `--prefabs` alone with no camera record; the same world `delvec cameras` writes as `<out>/worlds/at-load`) and a `validation/world-save.sh` save of the same build are read through the one reader, `tools/lib/anvil.py`, and compared cell by cell inside `render-plan.json`'s `layout_aabb`; a differing cell reds unless it falls in a named class, each a measured count printed with its cells, never an allowlist of cells — **gravity** (a gravity block on either side: the server settled it), **fluid** (water or lava on either side: the server flowed it), **random-tick** (grass block or mycelium on one side and dirt on the other: `SpreadingSnowyDirtBlock.randomTick` turned it — to dirt under a cover light does not pass, or onto lit dirt beside it — and which cells have turned by the save is the tick's draw), **re-derived** (the same block, differing only in a property the server re-derives on a block update: a fence, wall, bar or pane's sides and `up`, a stair's `shape`, leaves' `distance`), **clock** (a cell inside a gate region a clock owns, `clocked` in `validation/gate-seal.json`: the clock's phase at the instant the save was copied). Every run prints the box, the non-air counts of both worlds, the cells compared and each class's count with its cells; a non-zero class is a finding about the model, reported, not refused. It runs in tier 2 over the gallery's primary, after its PackTest pass boots its world once (`world-save.sh --seed`, the job's one verified fetch), and in the gallery bot job over the site-plan point, whose pieces carry `structure_void`. `--record <file>` writes the verdict named by the build's manifest sha256, and `tools/creator/staging-gate.py --written-world` admits a build only on a passing record of it. Remedy: the model is wrong at the cells named — fix the model, never the comparison. |
