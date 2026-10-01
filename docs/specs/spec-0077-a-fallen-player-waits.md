# spec-0077: A fallen player waits — a declared respawn wait in a party

- **Status**: Proposed
- **Ground**: written against engine `b09fbe5f` (`fix/party-wipe-reseat`: the party-wipe latch `#wipe`, `dw_wiped`, `party_wipe_tick`, `cp_respawn_check`, `cp_respawn_fire`, `cp_on_respawn_<i>`, `cp_reset_<i>` in `crates/delvec/src/compiler/emit.rs`), the cutscene observation state (`CUTSCENE_TAG`, `SNEAK_HELD_PREDICATE` and the two-camera `spectate` bounce in the same file), `WorldContent` in `crates/dsl/src/stages.rs`, and the pinned command tree `crates/delvec/data/commands-1.21.11.json`.
- **What it is for**: in a party, a player who falls is out of the fight for a while, so the rest of the party fights short-handed and a wipe — everyone down at once — is a state the party can still avoid by holding on. Today a fallen player is back at the fire the moment they click *Respawn*, which makes a wipe almost unreachable while two players trade deaths.
- **Research**: §2 is this spec's research record, measured on the pinned server. Each rule below is marked **cited** (a measurement, the record, or a constitution rule requires it) or **authored** (this spec chooses).
- **Numbers**: no ADR. **No DW code** is consumed by this text (§7 names the one refusal it needs and leaves the number to allocation). **`dsl_version` moves by one minor step** over the base it lands on (ADR-0024: §3 adds a field to the world stage); the number is allocated, not chosen here.
- **Non-goals**: holding the death screen (§2 shows vanilla has no way to); a wait in a party of one unless the creator asks for it (§3); a free-roaming spectator (§4); any change to what a wipe re-seats (spec-0016 §1, multiplayer addendum).

## 1. The ruling this serves

**Cited** (spec-0016 §1, multiplayer addendum): a respawn re-seats the scene only after a party wipe. This spec makes a wipe something the party can feel coming: a fallen player waits a declared number of seconds before rejoining, and while they wait they count as down.

## 2. Research record — what the pinned server offers

Instrument: `harness/probe/respawn-wait.ts`, two mineflayer 4.37.1 clients against the shipped delve image (vanilla 1.21.11, `force-gamemode=true`), gallery `primary.en` built by `delvec` at `e95b789d`. Every reading below is the reply the server gave.

| | Question | Reading | Verdict |
|---|---|---|---|
| M1 | Can the server hold the death screen? | `kill rd-b` → `Health 0.0f`; the client sends respawn; 150 ms later `Health 20.0f`. | **No.** Respawn is a client command the server honours at once; nothing a datapack can set delays it. The pinned command tree has 118 gamerule nodes and none delays a respawn. |
| M2 | What does `immediate_respawn` do? | Set `true`, `kill rd-b`, a client that does not act: 3 s later still `Health 0.0f`. | It tells the **client** to skip the death screen. It shortens the wait to zero; it cannot lengthen it. |
| M3 | Is there a vanilla clock for "time since this player came back"? | `minecraft.custom:minecraft.time_since_death`: `0` twice on the death screen 2 s apart; after respawn, in spectator, `6` then `48` 2 s later. | **Yes**, a stat: 0 while dead, counting from the respawn, spectator included. |
| M4 | Can a spectator's view be bound to a teammate? | `spectate rd-a rd-b`, `rd-a` moved to z=2.5: `rd-b` at z=2.5. After `rd-b` sneaks, `rd-a` moved to z=9.5: `rd-b` stays at z=2.5. | **Yes**, and sneaking releases it, as the engine's cutscene already measured and handles. |
| M5 | A waiting player disconnects. | The stat holds at `120` across 3 s offline and resumes (`144`). The player rejoins with `playerGameType 2` (adventure): `force-gamemode=true` resets the mode on join. | The clock pauses offline. The mode is lost on rejoin, so the waiting state has to be held by something that survives a relog (a tag) and re-applied. |

Found while measuring: until a client sends `player_loaded`, the server holds the player invulnerable (`kill` answers `Killed <name>` and nothing dies). The vanilla client sends it after joining and after every respawn. It does not affect the design, but every bot proof of this spec sends it.

**The judgement against CLAUDE.md's no-hacks rule.** Vanilla has no "respawn after N seconds" primitive, and the death screen cannot be held (M1, M2). It does have every part of the established alternative. Spectator mode is vanilla's own state for a player who is present and not playing; a hardcore death ends in it. `spectate` binds the view, a stat or a scoreboard counter times the wait, and `gamemode` ends it. The one part that acts against the player's input is putting the camera back after they sneak off a teammate, and the engine already ships it: the cutscene bounce re-attaches a spectator each tick, except while the `sneak_held` input predicate reads the key as held. A respawn wait built from these parts uses only first-class primitives the engine already emits. **Verdict: buildable.** The cost is engine-wide, not a hack: every engine rule that reads a player's position or health must skip a waiting player (§5).

## 3. Declared on the world — `world.respawn_wait`

**Authored.**

```json
"respawn_wait": { "seconds": 10, "alone": false }
```

- `seconds` — how long a fallen player waits after clicking *Respawn*, `1..=120`.
- `alone` — whether a player who dies with nobody else present also waits. Default `false`: a party of one never waits. **Cited** (the owner's ruling: disabled alone by default, configured by the creator in a party).
- Absent — no wait, and emission is byte-identical to the base.

It is a world-level declaration because it is a rule of the delve, not of any fire or checkpoint: the same wait applies wherever the player falls. **Cited** (CLAUDE.md, *This is a general engine*: the creator states the number; a primitive does not choose it).

## 4. The waiting state

**Authored, on cited parts.** A player waits when they come back from a death (the respawn edge `cp_respawn_check` already detects) and either another player is present or `alone` is true. While waiting the player:

1. is in spectator mode and carries the observation tag (§5);
2. has their view bound to a living teammate in play, re-attached by the cutscene bounce, which leaves a player alone while they hold sneak (**cited**: the shipped bounce and its `sneak_held` predicate). With no teammate in play to watch, they are not waiting: see 4.
3. sees the seconds left on the action bar, as engine chrome localised through the chrome tables (`delvewright.ui.respawn.wait`, one `%s`);
4. is released when `seconds × 20` ticks of their own wait clock have run, **or at once when no player is left in play**. The second case is a party wipe (spec-0016 §1): every waiting body is released, and the first one back re-seats the scene exactly as the first respawn after a wipe does today;
5. on release: mode back to the delve's mode, the tag removed, seated on the active checkpoint cell (`cp_seat_<i>`), and the fire's per-player respawn half run (flask refill), as for any respawn.

The wait clock is a `dummy` objective the tick increments for each waiting player. The stat measured in M3 would also work, but a dummy counter has no second meaning, and it pauses offline because the tick only reaches online players (**authored**). A waiting player who disconnects keeps the tag, rejoins in adventure (M5), and the tick puts them back in spectator; their clock resumes where it stopped.

**For the wipe, waiting counts as down.** The party-wipe detector counts "in play" as alive and not waiting. Without that rule, a party of two where one waits and one dies would never wipe.

## 5. A waiting player is out of play everywhere

**Cited** (`CUTSCENE_TAG`'s staging invariant: a player who is watching is not asked anything and is not harmed). The waiting state is the same object class as a cutscene viewer: a player who is watching, not playing. So it uses **one** observation tag, not a second one (CLAUDE.md: a capability belongs to the object class; a second bespoke field is the defect). Every rule that today excludes `dw_cutscene` excludes the waiting player by the same tag, and every positional or health selector over players that does not yet exclude it is brought under it. A spectator flying into a proximity trigger must not fire an ambush, complete a `reach` objective, die in a lethal volume or be judged by the stealth judge.

Measured on the base gallery build: 110 positional player selectors (`@a[...]` with `x=`/`dx=` or `distance=`) across 15 functions. 98 already exclude `dw_cutscene`. The other 12 are 6 `@a[distance=…]`, 3 box selectors without the tag, and 3 `distance` selectors guarded on health. Each is either brought under the tag or recorded with the reason it may see a watcher.

## 6. What the bot proves

**Authored.** A two-body probe, of the shape `harness/probe/party-wipe.ts` already takes:

- one of two dies and clicks *Respawn*: they are a spectator, tagged, `#wipe` stays 0, and the wave is not re-seated;
- the waiting body moves into a declared proximity trigger's box (by `tp`): the trigger does not fire;
- after `seconds`: they are back in adventure on the checkpoint cell, untagged, with the flask refilled;
- one waits and the other dies: both are released, the wave re-seats once, and the second one back does not re-seat it again;
- `alone: false`, one player present: a death does not wait.

## 7. Refusal

`seconds` outside `1..=120`, or a `respawn_wait` in a campaign that declares no checkpoint or bonfire to come back to. **Authored.** One DW code, allocated at implementation.

## 8. Decisions for the owner

1. **A party of one never waits unless the creator says `alone: true`.** (The owner's ruling, restated as the surface.)
2. **A waiting player watches a teammate, not the map.** The free spectator view would let a waiting player fly ahead through walls and spoil every first-encounter kill the party has not reached yet. The cost: the view is a teammate's, and sneaking frees it while the key is held, exactly as in a cutscene.
3. **A wipe ends every wait at once.** A party in which nobody is left in play is wiped, and the scene re-seats on the first return.
4. **The wait counts from the *Respawn* click, not from the death.** M3: the death screen cannot be timed by vanilla's own clock, and a player who leaves the death screen open is already waiting.
5. **The countdown is engine chrome** (`Back in %s`), localised like the other engine strings, not authored per campaign.
6. **The range is 1–120 seconds.**

## Acceptance criteria

1. `delvec schema --stage world` exports `respawn_wait` with `seconds` (integer) and `alone` (boolean, default `false`); a campaign without it builds byte-identically to the base (gallery baseline: no emitted path moves for a domain point that does not declare it).
2. A gallery point declares `respawn_wait`, and perturbing `seconds` moves an emitted byte (the coverage gate's acceptance rule).
3. `cargo test`: the respawn edge of a campaign declaring `respawn_wait` puts the respawning player in spectator, tags them with the observation tag, and starts their clock; the release line compares the clock against `seconds × 20`; the party-wipe detector counts a tagged player as down; with `alone: false` the wait is gated on a second present player.
4. `cargo test`: every positional player selector in the gallery build either excludes the observation tag or is named in a committed allowlist with its reason; the count of each is printed and asserted non-zero.
5. The two-body probe of §6 passes 100% of its assertions on a live server with the gallery build, and fails at least the "does not fire" and "both released" assertions on a build with the wait removed.
6. `docs/reference/compiler.md` carries the `respawn_wait` row; the skill's pitfalls page says what a party feels and when to declare it.
7. The refusal of §7 has a test for each shape.
