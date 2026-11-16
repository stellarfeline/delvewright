# spec-0141: The party is every member, present or not — every surface states what it means for one to four players, a collect counts the party, a stack for the party is owed to a member who was away, a class taken late arrives where the party is, and the build refuses a shape that assumes one player who is always there

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `d63627cec` (`delvec 1.17.0`, `dsl 0.79.0`), read for: the party holder and the three audiences in `emit/effect.rs` (`Audience`, `root_audience`, `effect_audience`); the tick's completion drivers in `emit/functions.rs`; `class_apply_fns` in `emit/class.rs`; `emit_set_checkpoint`, `cp_seat_dispatch` and the respawn edge in `emit/checkpoint.rs`; the limit clock's `start_lines` in `emit/limit.rs`; the observer census in `compiler/observer.rs`; the party-division templates in `emit/packtest/party.rs`; and the unit enumeration in `tools/ci/gallery_units.py`.
- **What it is for**: a delve is built for a fixed group of one to four players, and every machine proof it has is one bot. A creator never writes how many will play, so each thing the DSL declares must compile to content that is right for one, two, three and four players by construction. This spec states what "right" is for every surface, repairs the three places the emission assumed one player who is always there, and adds the gates that turn the next such assumption red.
- **Numbers**: spec `0141`; `DW1126` and `DW1127`, two used (§4). `DW1128`, `DW1129` and `dsl_version` `0.82.0` were handed and are not used: nothing here changes what a creator can write.
- **Non-goals**: a new field (the meanings below are the engine's defaults and the fields that already exist); a party size above four; a proof that a design is fun for four (the playtest judges it); the countdown bar of a limit for a late member and a gate closing on a member outside it (§6, both named as open).

## 1. The model **[authored, on the settled party holder]**

Progress is a fact about the party (the `#party` holder): any member's act advances everyone. What that leaves open is the three questions a party of one cannot ask.

- **Who is credited.** The acting player (`@s`) of a party beat is the member whose act fired it. A count the party owes — items gathered — is the party's, summed over its members.
- **Who is told, and who is given.** A telling (a line, a sound, a title) is for the members present: a beat is not replayed. A thing that lasts — a stack in an inventory — is for every member, and a member who was away when it was handed out is handed theirs when they are back.
- **Who is moved.** A member is where their own body is. The engine moves a body only where a surface says so (a cutscene's bracket, a `teleport`'s volume, a respawn), and a member who enters the delve after the party has moved on enters where the party respawns.

"Away" is one state with three causes, and no rule below tells them apart: a member who was disconnected on the tick of a beat, one who joins the delve for the first time after it, and one still choosing a class.

## 2. Every surface states its party meaning **[authored]**

`tools/ci/party-semantics.json` holds, for every surface unit of `delvec schema --stage all` — the units the gallery coverage gate enumerates, by the same walk (`tools/ci/gallery_units.py`) — one of eight classes and, for a unit with a meaning of its own, who is credited, told and moved for one to four players, and the evidence that the emission does that:

| Class | Meaning |
|---|---|
| `party` | one fact for the whole party; happens once whoever acted |
| `each` | every player has their own, kept on that player |
| `actor` | belongs to the one player who acted |
| `present` | reaches the players present, or in a declared box, on the tick it happens |
| `first-come` | one physical thing in the world; whoever takes it has it |
| `declared` | a field states which of these applies; the default leaves nobody out |
| `parameter` | a value of the surface that holds it; judged with its holder |
| `no-runtime` | shapes the world or the build's proofs; addresses no player |

`tools/ci/check-party-semantics.py` refuses a unit no row reaches, a row that reaches no unit, and a row with no semantics or evidence. A surface added to the DSL is red until its party meaning is written. The row is a statement a reviewer reads, not a proof: §4 and §5 are what bind statements to bytes.

## 3. The three repairs **[authored]**

### 3.1 A collect counts the party

A `collect` of `count: n` completed when one player's inventory held `n`. Three of an item gathered by three players completed nothing. The tick's held check now sums every player's count of the item into `#hold dw.sys` and completes the objective, as the first player who carries one of them, once the sum reaches `n`. For one player the sum is that player's count, so nothing a single player meets changes. The completion advancement still reads one inventory; the tick line is what it misses.

### 3.2 A stack for the party is a grant

`give-item` with `carrier: all` (the default) under a party or scheduled audience emitted `give @a`. A member who was away never held the stack — and where a later objective needs it in a hand, had nothing to do. It now emits `function <ns>:grant_<i>`:

- `grant_<i>` gives every present player the stack and counts the issue on `#all` and on each player it reached (`dw.gr.<i>`, and the total `dw.gr`);
- the tick's one owed line runs `grant_owed` as each player who has taken a class (every player, in a campaign with no classes) whose total is not the party's; `grant_owed` hands that player one stack of each grant they are behind on, and counts it.

A grant is keyed by the stack it gives. `carrier: one`, a solo audience (`on_death`, `on_kill`, a shop offer, a presser trigger, `audience: actor`) and a give under an `in` box are not grants: they name who is given. For one player who never leaves, the stack arrives on the same tick as before. A single player who was offline when a scheduled beat handed out a stack now holds it on return, which before was lost.

### 3.3 A class taken late arrives where the party is

`class_apply_<c>` teleported its player to the campaign's entry point. A friend who joined an hour in stood at the start, behind whatever the party had shut. It now also runs the respawn's own seat dispatch (`cp_seat_<i>` under `#cp dw.sys`): once a checkpoint is armed, a player who takes a class arrives on it. Before the first checkpoint the entry point stands, so a single player's first minute is unchanged.

## 4. The census **[authored, on the observer census's shape]**

Two shapes in a shipped datapack assume one player who is always there, and `compiler::absent` reads every shipped function for both on every build:

- **a lasting write onto every present player** outside the root `tick` — a command of `absent::LASTING` aimed at `@a`, or at `@s` under an `execute … as @a` of the same line — which must stand at a site `absent::SERVED` names with what reaches a member who was away, or the build is refused (`DW1126`);
- **a pick of one player** — `@p`, `@r`, a player-typed `@n`, a player selector with `limit=` — which must stand at a site `absent::PICKS` names with which player is picked and why, or the build is refused (`DW1127`).

Both are engine self-checks: the campaign cannot cause or repair them, and the message says so. A line of the root `tick` is restated every tick and is counted, not judged; a telling is for the present by §1 and is not read; the body of a function dispatched `as @a` is counted, not read. The build prints the census's binding line and writes `validation/absent-census.json`.

## 5. The party's PackTests **[authored, on spec-0018's templates]**

A single dummy cannot tell `give @a` from `give @s`, or one player's count from the party's. Three generated templates bring a second member:

- `party_collect` — for the first `collect` of two or more: one member holds `n − 1`, a spawned member holds one, one real `tick`, the objective is done;
- `party_grant_<i>` — per grant: the grant is issued through `grant_<i>`, a member is spawned after it, the owed dispatch hands them the stack once, their count is the party's, and a second dispatch owes them nothing;
- `party_class_arrives` — for the first checkpoint whose seat is off the entry point's column: with it armed, a class apply puts the player on that seat.

The build prints `absent-member binding (spec-0141): …` with each denominator.

## 6. What it leaves open, by name **[authored]**

- **A limit's countdown bar for a late member.** The bar's players are set when the clock starts (`bossbar set … players @a`). A member who first joins after that does not see it; the clock, its expiry and its telling are the party's and are unaffected. The repair is to restate the bar's players on the clock's own once-a-second step. Named at the census site `limit countdown` as a known gap, and not repaired here because the limit clock is under change on another branch.
- **A gate closing on a member outside it.** `close-gate` fills its region once. A member on the far side stays there until the story opens it. Whether a closing gate gathers the party is a design choice the DSL has no field for; it is a proposal (a `gather` mark on the effect), not built.
- **A unique item whose holder leaves.** A `carrier: one` stack, or a `collect`'s item, is in one inventory; when that member disconnects, the item is out of the world until they return. A collect is settled on the tick the party holds its count, so the window is an `interact` that presents a `carrier: one` item. A proposal, not built.
- **A run with several bots.** The harness walks the critical path as one bot. A second bot that joins late and a third that idles would bind the census's rules to a live server. A proposal, not built.

## 7. Acceptance criteria

1. **The record covers the surface.** `python3 tools/ci/check-party-semantics.py --delvec <delvec>` exits 0 and prints the unit count it classified equal to the unit count of the export; a row removed from the record, and a unit row naming no unit, each turn it red (`tools/tests/test_check_party_semantics.py`).
2. **A collect counts the party.** `crates/delvec/tests/it/absent.rs`: the tick of a campaign with a `collect` sums every player's held count into `#hold dw.sys` and completes on the sum; the gallery's suite carries `party_collect`, whose two members hold the count between them.
3. **A party give is a grant.** `absent.rs`: a `carrier: all` give under an objective's completion emits `function <ns>:grant_<i>` and no `give @a` in the bundle; `grant_<i>` gives `@a` and counts the issue on `#all` and on `@a`; the tick carries one owed line; a `carrier: one` give and a shop offer's give stay `give @s`; a campaign with no party give emits none of it.
4. **A late class arrives at the checkpoint.** `absent.rs`: `class_apply_<c>` of a campaign with a checkpoint carries the seat dispatch after its entry teleport, and of a campaign with none does not.
5. **The census binds and reds.** `absent.rs`: over every fixture family the census reports no unserved write and no unnamed pick; a `tag @a add` planted in a completion bundle is `DW1126` naming it, and an `execute at @p` planted there is `DW1127` naming it; `@p` inside an authored line is not a pick.
6. **PackTest.** The gallery build emits `party_collect`, one `party_grant_<i>` per grant and `party_class_arrives`, and they run green in tier 2.
7. **Codes and pages.** `DW1126` and `DW1127` are asserted by a test and sit on `docs/reference/delvec/compiler/absent.md`; `check-dw-codes.py`, `check-diagnostic-messages.py`, `check-live-commands.py` and `check-json-canonical.py` are green.

## 8. Measurements this spec states

Written into the commit bodies that land the implementation: the unit count and the count per class of the record; the census's binding line over the gallery primary; the absent-member binding line over the gallery primary.
