# What a delve means for two, three and four players

For whoever adds or reviews an engine surface. A creator never writes how many will play, so every surface must be right for one to four players as emitted. This page says how that is judged and what holds today.

## The three questions

A party of one cannot ask them, so a single bot never answers them.

- **Who is credited?** Progress is the party's (`#party`): any member's act advances everyone, and the acting player (`@s`) is the member whose act fired the beat. A count the party owes is summed over its members.
- **Who is told, and who is given?** A telling is for the members present. A thing that lasts is for every member, and a member who was away is handed theirs on return.
- **Who is moved?** Only the bodies a surface names. A member who enters after the party has moved on enters where the party respawns. A crossing into another area moves the whole party, and a member who missed it is put where the party is.

"Away" is disconnected on the tick of a beat, joining for the first time after it, or still choosing a class. No rule tells the three apart.

## How a surface is judged

1. **Its row.** `tools/ci/party-semantics.json` gives every surface unit of `delvec schema --stage all` a class and, where it has a meaning of its own, who is credited, told and moved, with the evidence. `tools/ci/check-party-semantics.py` refuses a unit with no row and prints the table (`--index`). The classes are `party`, `each`, `actor`, `present`, `first-come`, `declared`, `parameter` and `no-runtime`.
2. **Its bytes.** `compiler::absent` reads every shipped function on every build. A lasting write onto the players present outside the root `tick`, and a pick of one player, each stand at a named site or refuse the build (`DW1126`, `DW1127`; [`delvec/compiler/absent.md`](delvec/compiler/absent.md)).
3. **Its run.** The generated PackTests that bring a second member: `party_join_<obj>`, `party_carrier_<obj>`, `party_collect`, `party_grant_<i>`, `party_class_arrives`, `party_limit_bar_<obj>`, `party_teleport_<key>`, `party_cross_<area>`, `board_<b>_players`, `vehicle_<id>_party`.
4. **Its party run.** `DELVEWRIGHT_PARTY=1` (`harness/src/party.ts`) walks the critical path with three more members on the server — one idling at the entry, one joining late, one leaving and returning — and the `party` stage asserts, read on the server, where each stands, what each was handed and what each was told. At a step where the party's crossing count (`#party dw.xc`) rises, the idler is asserted on a seat by the step's landing; a member who returns, respawns or joins after a crossing is asserted where the party is. The `gallery party bot` CI job runs it on the gallery.

A row is read from the emitted commands, never from the DSL's prose: the question is what an `@a`, an `@s` and a `#party` in the function do with four players online and one of them late.

## What holds, by surface

| Surface | With several players |
|---|---|
| objectives, quests, flags, campaign end | one state on `#party`; any member's act completes; told to everyone present |
| `collect` | counts what the party holds between all inventories |
| `interact` with `requires_item` | the presser's own main hand |
| `after: [a, b]` | a division of labour, proven with `min_players` agents |
| dialogue | each player's own screen, reading party state |
| class and kit | each player's own; a class taken once a checkpoint is armed arrives on it |
| `give-item` | `carrier: all` reaches every member, one who was away included; `carrier: one` the acting player |
| triggers | once for the party by default; `audience: presser` once per pressing player |
| cutscene | every present player watches from, and returns to, their own place; a member who leaves inside it is repaired on return |
| `teleport` | every body in the declared volume; with `from: "party"`, every member present, each on a seat of their own; a member away is not owed it |
| crossing into another area | every member present, each on a seat of their own around the entry point, whoever completed the objective; a member who was away, dead or still choosing a class is put where the party is once back: on the checkpoint armed since, else on the entry point |
| checkpoint, bonfire | the party's respawn point; every respawn is seated on it |
| death, respawn wait, stake, stealth | each player's own |
| shop | each player buys for themself |
| state | `scope` says party or player; a player's datum is seeded on their first tick |
| waves, vehicles | one set in the world; a `per_player` count measures the players present |
| race | every present player races on their own count; the absent are not waited for |
| board | one sidebar; each body is placed once, in the order it acts |
| loot | one fill in the world: first come |
| limit | one clock for the party; its bar is restated to every member present once a second |

## What does not hold yet

- `close-gate` does not gather the party: a member on the far side stays there until the story opens it.
- A `carrier: one` item is out of the world while its holder is disconnected.
