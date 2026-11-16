# `delvec::compiler::absent`

The reference page for `crates/delvec/src/compiler/absent.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1126, DW1127 — a shape that assumes one player who is always there (`compiler::absent`; spec-0141; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1126` | **Engine self-check: a lasting write onto the players present, with nothing for a member who was absent** (spec-0141 §4). On every build, a shipped command outside the root `tick` whose final command is one of `absent::LASTING` (`give`, `clear`, `item`, `tag`, `spawnpoint`, `gamemode`, `effect`, `attribute`, `advancement`, `scoreboard players`, `team`, `tp`/`teleport`, `ride`, `spectate`, `kill`, `damage`, `xp`/`experience`, `recipe`, `bossbar`) aimed at `@a` (or at `@s` under an `execute … as @a` of the same line) and that stands at no `absent::SERVED` site. Build-tier (exit 3), `compiler::absent::check`, read off the shipped bytes after emission. A DEFECT IN DELVEC, never the campaign's: the message says so and asks for a report; the repair is the emitter serving the absent member — issuing the write as a grant, restating it on a refresh, or repairing it at the member's return — and the site named in `absent::SERVED` with that rule. |
| `DW1127` | **Engine self-check: a pick of one player among several, with no rule** (spec-0141 §4). On every build, a shipped command holding `@p`, `@r`, a player-typed `@n`, or an `@a` / player-typed `@e` with a `limit=`, that stands at no `absent::PICKS` site. A selector is read only outside every bracket, brace and quoted string, so `@p` in an authored line is text. Build-tier (exit 3), `compiler::absent::check`. A DEFECT IN DELVEC: the repair is the emitter addressing every player, the acting player (`@s`) or the party holder, or the site named in `absent::PICKS` with which player is picked and why. |

**Binding.** Every build prints `absent-member census (spec-0141): W lasting write(s) onto every present player over F shipped function(s), S served at a named site, U unserved (DW1126); P pick(s) of one player, N named, M unnamed (DW1127); R line(s) restated on the tick and D dispatch(es) as every player not judged.` and writes `validation/absent-census.json` (`functions_read`, `lasting_writes_onto_every_present_player`, `served[{site,count,rule}]`, `unserved`, `picks_of_one_player`, `named[{site,count,rule}]`, `unnamed`, `restated_on_tick`, `dispatched_as_every_player`).

**What is not judged.** A line of the root `tick` (restated every tick, so a member who arrives is reached on their first); a line that tells the present what happened (`tellraw`, `title`, `playsound`, `stopsound`, `particle`, `dialog`); a write narrowed to a box or a distance (the bodies standing in a place on that tick); the body of a function dispatched `as @a` (counted as `dispatched_as_every_player`); the creator overlay.
