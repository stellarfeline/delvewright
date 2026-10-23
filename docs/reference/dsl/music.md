# `delvewright_dsl::music`

The reference page for `crates/dsl/src/music.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1031–DW1032 — a scene states its music (`dsl::music` / `compiler::music`; spec-0109; error)

This module's rows of a section whose prose is on the [`delvec::compiler::music` page](../delvec/compiler/music.md#dw1031dw1032--a-scene-states-its-music-dslmusic--compilermusic-spec-0109-error).

| Code | Meaning |
|------|---------|
| `DW1031` | **A scene that does not state its music, or states one no delve body can hear.** An atmosphere whose `attributes` lack `audio/background_music`; `horizon` absent, in its retired string form (`"void"`, `"ocean"`), or without `attributes.audio/background_music`; a `creative` entry in any `audio/background_music` (the pinned client plays it only in creative mode, and no delve body is ever in it); a `horizon.attributes` id outside `audio/` (the ground's look is the base's). Validation tier (exit 1), `dsl::validate`, at the world document. The remedy names the field and both spellings: `{}` for silence, or `{default: {sound, min_delay, max_delay, replace_current_music}}` for a track. |
