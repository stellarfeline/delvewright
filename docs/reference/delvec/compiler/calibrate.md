# `delvec::compiler::calibrate`

The reference page for `crates/delvec/src/compiler/calibrate.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW039x — shot calibration (`delvec calibrate`; spec-0019)

`calibrate` is the only subcommand that reads no campaign and builds no world —
just a harvested `rehearsal-report.json` plus the build's
`creator-datapack/layout.json`. Its codes therefore carry their own exit
mapping, stated per row rather than by the DW03xx section default.

| Code | Meaning |
|------|---------|
| `DW0390` | A harvested shot proposal names a cell with **no declared anchor within the 16-block snap radius**, so it cannot be written back into the DSL at all — the DSL has no free-floating world coordinates (spec-0019 §5). Reported per offending cell with the nearest anchor and its distance; the whole shot is left un-patched (a half-snapped dolly would fly a path nobody authored), while every other shot of the same session still is. **Exit 3**, and the patch file is still written. Prescription: declare an anchor near that cell in the prefab's metadata and re-mark the shot, or move the shot to an anchored spot — do NOT widen the radius and do NOT write a raw coordinate into the stage document. |
| `DW0391` | The rehearsal report and the `--layout` manifest name **different campaigns**: the proposals would snap onto another delve's anchors and silently relocate every camera. Refused before any snapping. **Exit 1**. Prescription: point `--layout` at the `creator-datapack/layout.json` of the build that session actually played — do NOT reuse an older build's manifest. |
| `DW0392` | The rehearsal report is unreadable, is not a rehearsal report, or carries a schema `version` this `delvec` does not understand (likewise for an unreadable layout manifest). **Exit 1**. Prescription: re-run `delvec harvest` over the session log — the report is a machine artifact and is never hand-written or hand-edited. |
