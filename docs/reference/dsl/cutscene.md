# `delvewright_dsl::cutscene`

The reference page for `crates/dsl/src/cutscene.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0199` | A `cutscene` effect's shape is invalid: it mixes the multi-shot `shots` list with the single-shot `path`/`seconds`/`subject` fields, declares neither, omits `seconds` on a single shot, gives a shot with an empty camera `path`, or gives a shot no `subject` (spec-0110), in either spelling — the message names the shot. The two spellings normalize to one shot list, so this is where the shape is policed and emission may then assume a well-formed, non-empty list. |
| `DW0348` | A `shot_style` declaration is semantically invalid (spec-0015): style params (`subject_b`/`dist`/`degrees`/`bearing`) on an unstyled shot; `subject_b` off `two-shot` (or a `two-shot` without one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside `1..=48`; `bearing` outside `-360..=360`; or a `"landscape"` subject under a style or as `subject_b` (spec-0110: a style builds its dolly from a subject's geometry, and a landscape has none). Validation-tier (exit 1), `dsl::validate`. |
| `DW0349` | A `side-track`/`low-follow` shot whose subject provably cannot move: those styles dolly *with* a moving subject, so the subject must be an npc/actor with a matching `move-npc`/`move-actor` in the same effect group or the same `sequence` timeline (an `anchor` subject can never move; reaction lists `on_arrive`/`on_caught`/`on_respawn` start a fresh scope — their firing time is statically unknowable). Validation-tier (exit 1), `dsl::validate`. Prescription: add the move alongside the cutscene, or use a static style (`locked-off`, `push-in`). |
