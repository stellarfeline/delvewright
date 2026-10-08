# `delvewright_dsl::assembly`

The reference page for `crates/dsl/src/assembly.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0935–DW0938 — a fixed thing that can be hit and hits back (`dsl::validate` + `compiler::assembly`; error)

This module's rows of a section whose prose is on the [`delvec::compiler::assembly` page](../delvec/compiler/assembly.md#dw0935dw0938--a-fixed-thing-that-can-be-hit-and-hits-back-dslvalidate--compilerassembly-error).

| Code | Meaning |
|---|---|
| `DW0935` | **An assembly's rig cannot be emitted as declared** (spec-0082 §5.1, §5.5, §3.2). Validation tier (exit 1), `dsl::assembly::assembly_checks` over `AnchorRegistry::rig` (the compiler's `PrefabRegistry` reads `<library>/rigs/*/rig.json`). The library holds no `rigs/<name>/rig.json`; the file does not parse as a rig document; or it breaks a structural rule `dsl::rig::check` states — no part, a part's block not a block state of the pinned registry (the `DW0193` rule), no clip, a clip with no frame, a frame that is not one transform per part, `ticks_per_frame` outside `1..=20`, a non-finite number, a scale of 0 on an axis, a zero-length quaternion, a clip name that is not kebab-case — each naming its field; or an `initial`, a strike step's `windup`/`strike`, or a `play-clip` (at any depth of any root) names a clip the rig lacks, the message listing the rig's clips; or a strike step's `ticks_per_frame` lies outside `1..=20`, naming the field. `delvec rig describe` refuses the same rig with the same code. Prescription: regenerate the rig with its generator, or name a clip the rig declares. |

### DW0968–DW0970 — a strike locks where the player stands (`dsl::validate` + `compiler::assembly`; error)

| Code | Meaning |
|---|---|
| `DW0969` | **A locked blow is declared where the lock derives it** (spec-0094 §5.2). Validation tier (exit 1), `dsl::assembly::lock_shape_checks`. In a step with a `lock`: a top-level `damage-players` with an `in` (the blow's area is the cells the chosen pose comes down on — a written box is a second, fixed answer), a `damage-players` nested at any depth inside another effect's list (it cannot be moved with the lock), and the `lock` itself when the pattern declares `aim` (two rules choosing one turn), each naming the field. Prescription: drop the `in`, lift the `damage-players` to the top of `on_land` (a `when` on it is kept), or drop `aim` or `lock`. |
| `DW0970` | **An `arm-strikes` names an assembly that never strikes** (spec-0094 §3.3). Validation tier (exit 1), `dsl::assembly::assembly_checks`, at every depth of every effect root: the named assembly declares no `strikes`, so there is no pattern to re-arm and the beat does nothing. Prescription: give the assembly a `strikes` pattern, or drop the effect. |
