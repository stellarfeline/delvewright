# spec-0076: A currency stands on screen — the standing display of a named datum

- **Status**: Accepted
- **Ground**: written against engine `322a342b` (`integration/vesperhold-playtest-2`), read only — `StateDecl`, `StateScope` in `crates/dsl/src/stages.rs`; `state_checks` in `crates/dsl/src/validate.rs`; `codes` in `crates/dsl/src/diagnostic.rs`; the `state.<id>.name` inventory in `crates/dsl/src/l10n.rs`; `named_states`, `named_state_tick`, `emit_named_state_functions`, `state_shadow_score`, the `setup` state block and the `dw.campaign` block in `crates/delvec/src/compiler/emit.rs`; `state_score`, `PARTY` in `crates/delvec/src/compiler/plan.rs`; the pinned command tree `crates/delvec/data/commands-1.21.11.json`; `SCORE_DISPLAY_SLOTS`, `trackScore`, `clearScoreDisplay`, `assertComplete` in `harness/src/executor.ts`; `docs/reference/compiler.md` (`state[]`, `state[].name`, the `campaign-complete` emission row); the gallery (`gallery/quests.json`, its six named datums; `gallery/probes/`; `gallery/README.md`); `tools/ci/gallery_units.py`.
- **What it is for**: a player can read what they hold **without spending or earning anything**. Today a named datum announces its new balance on the action bar at the moment it changes and nowhere else; once the line fades the balance is not on screen, and a player who wants to know it has to make it change. This spec adds the other half: a declared, standing readout of one currency, on the one vanilla surface that stands.
- **Research**: §6 is this spec's research record. Every rule below is marked **cited** (the record, a vanilla behaviour the engine already emits, or a constitution rule requires it) or **authored** (this spec chooses).
- **Numbers**: no ADR (no settled decision moves). **One DW code**: `DW0919` for §7 (a standing display the sidebar cannot draw as declared — two shapes, one rule). **`dsl_version` moves by one minor step over the base's** (ADR-0024): §2 adds a field to `state[]`, so a document that carries it stops parsing under the previous number; the value is typed in `crates/dsl/Cargo.toml` and nowhere else — `tools/lib/version_sites.py` refuses a second statement of it, this page included.
- **Non-goals**: a standing display of a `party`-scoped datum (§5, §8 — recorded as the open gap); a second standing slot (`list`, `below_name`, §6); a display with no declaration (§2); any change to the action-bar announcement, to `on_kill`, to a purchase, or to `dw.campaign` (§3 answers the sidebar objection in words and leaves the objective where it is).

## 1. The finding

`vh-08`: on `vesperhold` the party could not see how much tallow it held. The campaign declares one named, `player`-scoped datum, `state/tallow`; the engine emits its announcement correctly — one tick driver keyed on `dw.s_tallow` differing from its shadow `dw.sh_tallow`, firing `st_show_tallow`, which writes `<name>: <value>` to the action bar as a live `score` component once per change from any cause. That half stays exactly as it is. What is missing is a readout the party can consult when nothing is changing.

## 2. Declared, on the datum — `state[].display`

**Authored.** A named datum may declare where it stands:

```json
{ "id": "state/tallow", "scope": "player", "name": "Tallow", "display": "sidebar", "note": "…" }
```

`display` is an enum (`StateDisplay`) with one value today, `sidebar`, naming the vanilla display slot the datum occupies. It is optional. **Absent means what the engine does today**: the datum announces itself on change and stands nowhere, and its emission is byte-for-byte what spec-0032 shipped. It is a declaration and not automatic because a primitive encodes a mechanism, never a decision about what it is for (CLAUDE.md, *This is a general engine* — **cited**): a creator whose game shows a hidden tally only when it moves, or whose economy has two named datums of which one is the purse, has to be able to say so, and "every named datum stands" would decide it for them.

`display` requires `name`: a standing display with nothing to head it would show the objective's id, which is the thing §3 forbids. A `display` on an unnamed datum is `DW0919` (§7, first shape, stated there so the rule has one home).

## 3. Where it stands — the sidebar, and the two objections answered

**Cited** (§6, the wiki on display slots): vanilla has exactly three display slots a scoreboard objective can occupy. `list` draws a number in the tab menu, which is visible only while the player holds the key; `below_name` draws under **other** players' name tags, hidden beyond about ten blocks and when they sneak, never over the viewer's own body, and not at all in singleplayer; `sidebar` draws on the right-hand side of the screen, headed by the objective's **display name**, one line per score holder, up to fifteen. Only the third stands, for every player, at every moment, in a party of one as in a party of four. So the standing display is the sidebar, and there is no second choice to weigh.

The tree records two reasons the sidebar has been kept clear, both in the `setup` block that declares `dw.campaign` and in the `campaign-complete` emission row. Both are answered here, neither is weakened:

- **"The slot would show a raw internal id."** True of `dw.campaign`, which has no player-facing name, and it stays off the sidebar. It is not true of a currency: the sidebar's heading is the objective's display name (**cited**), and a named datum has one — its `name`, already translated under `state.<id>.name`. The emission sets it (`scoreboard objectives modify <obj> displayname <component>`, **cited**, in the pinned tree), so the id `dw.s_<local>` never reaches a screen. Player lines are player names, which are the players' own.
- **"The harness's mineflayer cannot decode the pinned game's score packets."** The bot does not read the standing display, and nothing about this spec asks it to: campaign completion travels on the anchored chat marker; the `assertComplete` fallback read is keyed by objective name (`dw.campaign`), so a currency objective on the sidebar cannot feed it. The harness's die-retry stage does read ledgers from raw score packets by putting an objective in a display slot (`trackScore`), and it takes the plain `sidebar` **first**, releasing it by clearing the slot (`scoreboard objectives setdisplay <slot>` with no objective clears it — **cited**). With a currency standing there, that stage would evict the campaign's readout and then blank it. Its own comment already says the plain `sidebar` is the delve's; §4 makes that true and takes `sidebar` out of the harness pool. The bot's read is unchanged in every other respect: sixteen team-coloured sidebar slots, which no player is on a team to see.

## 4. What is emitted

**Authored**, three lines in `setup` for the one datum that declares `display: sidebar`, after every datum's objective is declared and before the economy's objectives, so the world's own init sets the slot once:

```
scoreboard objectives modify dw.s_<local> displayname <tr(name)>
scoreboard objectives modify dw.s_<local> numberformat styled {"color":"gold"}
scoreboard objectives setdisplay sidebar dw.s_<local>
```

`tr(name)` is the same `{translate, fallback}` component the action bar already uses, under the same key, so the sidebar and the announcement read the same word in every language and no new l10n key exists. `numberformat styled` is in the pinned tree (**cited**) and paints the value the gold the action bar already paints it — one currency, one colour, two places. Nothing per tick: the sidebar reads the objective's own scores, which `state_seed` seeds on each player's first tick and every state verb moves, so the display is live by construction. A campaign with no `display` emits none of the three lines.

The harness: `SCORE_DISPLAY_SLOTS` loses `"sidebar"`; its ceiling is sixteen and its comment says why the plain slot is not the harness's to take.

## 5. Whose balance, and what the party sees

**Cited**: a `player`-scoped datum is one purse per player; `on_kill` pays the credited killer's purse and a purchase debits the buying player's (spec-0074 §5, spec-0032). The sidebar draws every score holder of the objective, so with a `player` datum standing **each player sees every party member's balance, one line under each name, their own among them**. That is the right reading of a per-player purse in a party: the player reads their own line, and the party reads who can afford what, which is what a shared screen of personal purses is for. It is also the only reading the slot has — the sidebar has no per-viewer filter (**cited**, §6).

A `party`-scoped datum is held by `#party`, and **fake players whose names start with `#` do not show up in the sidebar under any circumstances** (**cited**). A party purse asking to stand would therefore stand as an empty box under a heading. The engine refuses it (§7, second shape) rather than working around it: mirroring the value onto a visible fake player is a second holder whose name could be a real player's, labelled with a word the engine would have to invent — a lower-layer hack at the compiler→DSL boundary, which the constitution excludes until vanilla provides the primitive (CLAUDE.md, *No hacks at any layer* — **cited**). The gap is recorded in §8.

## 6. Research record — what stands on a vanilla screen

- **Minecraft Wiki, *Scoreboard*, display slots.** `list`: *Displays a yellow number or some hearts … on the tab menu, where online players are shown.* `sidebar`: *Shows on the right hand side of the screen, up to 15 entities with the highest score*, under *a heading labeled with the objective's display name*. `below_name`: *Shows the score followed by the objective's display name below the player's name tag above their head. This is hidden beyond around 10 blocks and when the player is sneaking. Not visible in singleplayer.* → **Adopted**: the sidebar is the standing display (§3); the other two are declined (§8).
- **Minecraft Wiki, *Scoreboard*, hidden holders.** *Fake players with names starting with a `#` character do not show up in the sidebar under any circumstances.* → **Adopted** as the reason a `party` datum cannot stand (§5, §7).
- **Minecraft Wiki, *Commands/scoreboard*.** `scoreboard objectives add <objective> <criteria> [<displayName>]` *creates a new objective with the given internal objective name, specified criterion, and the optional display name*; `scoreboard objectives modify <objective> displayname <displayName>` *changes the display name of the scoreboard in display slots*; `numberformat styled` *sets the default number format to styled, which indicates that the score is displayed with the selected style*; `scoreboard objectives setdisplay <slot> [<objective>]` *displays score info for the objective in the given slot … if no objective is provided, this display slot is cleared.* → **Adopted** as the three emitted lines (§4) and as the reason the harness's release would blank a standing display (§3). The pinned tree `crates/delvec/data/commands-1.21.11.json` carries every one of them under `scoreboard/objectives/modify/<objective>/displayname|numberformat` and `scoreboard/objectives/setdisplay/<slot>/<objective>`, read for this spec.
- **The tree.** `harness/src/executor.ts` `SCORE_DISPLAY_SLOTS` takes `sidebar` first, *because the plain `sidebar` is the slot the delve's own campaign readout uses* — a claim the tree did not yet make true; `clearScoreDisplay` releases by `setdisplay <slot>` with no objective. `assertComplete` reads `bot.scoreboards[step.objective]` for `dw.campaign` only. → **Adopted** as §3's answer and §4's harness change.
- **Dark Souls, Hollow Knight, Elden Ring (the reference games spec-0074 §7 records).** Each keeps the player's currency count on a permanent HUD corner and animates it on change; none hides it between changes. → the finding's shape confirmed; no mechanism adopted from them, because vanilla's is the one on hand.

## 7. Refusal — `DW0919`, a standing display the sidebar cannot draw as declared

**Authored**, one code, two shapes, the `DW0520` precedent (one rule about a stake's datum, two ways to break it). Raised at `delvec validate` by `dsl::validate::state_checks`, validation tier (exit 1), path `/content/state/<i>/display`, the message naming the datum:

1. **Two datums ask for the one slot.** The sidebar holds one objective (**cited**); a second `display: sidebar` is refused naming both datums, never resolved by order — a silent "first wins" would hide a design decision the creator has to make. A `display` on a datum with no `name` is the same shape: the slot cannot draw what it is handed (there is no heading), and the prescription is the same sentence — give it a `name`, or take the `display` off.
2. **A `party` datum asks to stand.** Its holder is hidden from the sidebar (§5); the display would be an empty heading. Prescription: declare the datum `player`-scoped if each player holds their own, or leave `display` off and keep the announcement.

## 8. Declined, with the reason

- **A standing display for a `party`-scoped datum** — §5; excluded until vanilla can draw a hidden holder or the engine adopts a mechanism that is not a mirror onto an invented name. **Open capability gap**, recorded here as spec-0038 records water.
- **`list` and `below_name`** — §6; neither stands for the viewer.
- **Automatic display of every named datum** — §2; a design decision inside a primitive.
- **"First declared wins"** for two `display: sidebar` — §7; a resolution that hides a decision.
- **A `numberformat` or colour surface on the datum** — the value's colour is the engine's, already chosen for the action bar; a second knob for one number.
- **A per-viewer sidebar** (each player sees only their own line) — vanilla's sidebar has no per-viewer filter; `sidebar.team.<colour>` is per team, and putting each player on a colour team to filter the readout would repurpose teams, a hack.
- **The bot reading the sidebar** — §3; the completion channel is the marker and the die-retry ledger read is raw packets on team slots; nothing this spec adds is for the bot.

## Acceptance criteria

Each criterion is checked against the tree at `322a342b`: the surface it names exists (or is named as owed by the implementation), and none is satisfied today.

1. **Surface.** `delvec schema --stage all` exports `display` on `state[]` as `StateDisplay` with one variant, `sidebar`; `dsl_version` is one minor step over the base's, typed in `crates/dsl/Cargo.toml` only (`tools/lib/version_sites.py verify` green). A campaign that declares no `display` builds byte-identical to the previous engine (`gallery/baseline/` differs only where a display was added and `delta.json` attributes every row).
2. **Emission test** (`crates/delvec/tests/v10_economy.rs`, beside the announcement tests): a `player` datum with `display: sidebar` emits the three §4 lines in `setup`, in that order, the display name carrying `"translate":"<pack key of state.<id>.name>"` and the `setdisplay` naming the datum's own `dw.s_<local>`; no other function contains `setdisplay sidebar`; a datum without `display` emits none of the three and its `setup` is byte-identical to the base; `dw.campaign` is on no display slot. Perturbation: remove `display` and the three lines vanish; rename the datum and the objective in all three moves. Build twice, byte-equal (ADR-0006).
3. **Diagnostic.** `DW0919` has a red fixture for each of its two shapes in `crates/dsl/tests/v10_economy.rs` — two `player` datums both declaring `display: sidebar`; one `party` datum declaring it — and a third for the unnamed shape, each naming the datum in the message; the well-formed economy with one `display: sidebar` validates clean. `tools/ci/check-dw-codes.py` green; `docs/reference/compiler.md` carries the row and the section.
4. **Harness.** `SCORE_DISPLAY_SLOTS` contains no `"sidebar"`; `npm test` and `tsc` in `harness/` green.
5. **Gallery** (spec-0039). `gallery/quests.json` binds `display: sidebar` on `state/tokens` (the `player`-scoped wager the stakes forfeit, so the standing readout is the one a death moves); `StateDecl.display` and `StateDisplay::sidebar` are bound in the primary. Two probes under `gallery/probes/`, each the primary plus one declared edit: `display: sidebar` added to `state/relics` (§7.1) and moved onto `state/bounty`, the `party` purse (§7.2). `check-gallery-coverage.py` reports 0 units in neither state; `gallery/README.md` names the display and gains the two refusal rows.
6. **Docs, same change.** `docs/reference/compiler.md`: the `state[]` shape gains `display?`; a `state[].display` row beside `state[].name` states the three emitted lines, the harness slot rule and what absence means; the `campaign-complete` row's *never on the sidebar* is qualified — the sidebar is a currency's slot (this spec), and `dw.campaign` stays off it; the `DW0919` row and its section. `docs/demo-levels.md` gains the row **The Counting House (0076)**, status pending. `docs/specs/README.md` gains this spec's row.
7. **The creator's page.** `.claude/skills/delvewright/skills/new-delve/references/quest-capabilities.md`, in the bullet that says a currency is a named datum: `display: "sidebar"` makes the purse stand on the right of every player's screen, headed by its `name`, one line per player; one datum per campaign may stand; a `party` purse cannot (`DW0919`); `tools/ci/check-skill-page.py` green.
8. **Content.** `vesperhold` declares `display: sidebar` on `state/tallow`; the build exits 0 with no new diagnostic; `vh-08` in `docs/playtest-findings.json` carries the emission invariant's test as its carrier with a binding that selects `display: sidebar` by identity, and `tools/creator/staging-gate.py` reports it bound on that build with a non-zero count.
