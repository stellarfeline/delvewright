# Press answers — what a refused press says and does

What established practice, the pinned game and this engine's own record say about a right-click that cannot act: a thing the player presses before its step is open, after it is spent, or with the wrong thing in hand. Each line is **cited** (a source named under *Sources*, a line of the tree, or a reading of pinned data) or **authored** (the synthesis this page draws). The research answers one question — how a refused press is answered so a player reads it — and stops.

## 1. Every action gets a reaction

- **Cited** (Nielsen Norman Group, *10 Usability Heuristics*, #1 *Visibility of system status*): "The design should always keep users informed about what is going on, through appropriate feedback within a reasonable amount of time." "When users know the current system status, they learn the outcome of their prior interactions and determine next steps."
- **Cited** (the same, #9 *Help users recognize, diagnose, and recover from errors*): "Error messages should be expressed in plain language (no error codes), precisely indicate the problem, and constructively suggest a solution."
- **Cited** (Norman, *The Design of Everyday Things*, as summarised by secondary readers — ideas only): feedback is information returned about what action was done and what came of it; it closes the gulf of evaluation, the difficulty of working out what happened after an action; it may be sound, touch or speech as well as sight.
- **Cited** (`game-writing.md` F2, citing the Nielsen Norman *Error-Message Guidelines*): a refusal says what is wrong and what fixes it; one line of flavour may follow the statement, never replace it.

**Authored.** A press that does nothing and says nothing is the gulf of evaluation left open: the player cannot tell a shut step from a broken one, and presses again. Silence is the defect, in every state the thing can be pressed in — before its step, after it, and with the wrong thing in hand.

## 2. Two channels, not one

- **Cited** (Game Accessibility Guidelines, *Ensure no essential information is conveyed by sounds alone*, basic, hearing; *Ensure no essential information (especially instructions) is conveyed by text alone, reinforce with visuals and/or speech*, intermediate, cognitive).
- **Cited** (Game Accessibility Guidelines, *Give a clear indication that interactive elements are interactive*, intermediate): the thing must read as pressable before the press; spec-0104's highlight is this engine's answer to that half.

**Authored.** A refused press is told in words and reinforced by a sound: the sound says *you were heard*, the words say *why nothing happened*. Neither alone satisfies both guidelines.

## 3. What the pinned game does itself

- **Cited** (Minecraft Wiki, *Data component format*, `minecraft:lock`): a container carrying `lock` opens only for a player holding the matching item in the main hand; a player without it who interacts with the container is answered — the game "will play a sound and display a message in the actionbar".
- **Cited** (`crates/delvec/data/sounds-1.21.11.json`, the pinned sound registry): `minecraft:block.chest.locked` is the sound event vanilla pairs with that message; `minecraft:block.lever.click`, `minecraft:entity.villager.no` and `minecraft:ui.button.click` are in the same registry.
- **Cited** (`crates/delvec/data/commands-1.21.11.json`, the pinned command tree): `title <targets> actionbar <component>` is the one command that writes the action bar; `tellraw <targets> <component>` writes chat; `playsound <sound> …` plays a registered sound event. **Cited** (Minecraft Wiki, *Commands/title*): the action bar is "the space just above the player's hotbar" and a new `actionbar` "changes the current action bar title".
- **Cited** (`game-writing.md` F3, FromSoftware as quoted there): the system message for a shut door is flat — "Door does not open from this side" — and says the state, not the lore.

**Authored.** Vanilla's own convention for a refused use — one short actionbar line and one sound, to the player who pressed, every time they press — is the convention a Minecraft player already knows. The action bar is the right strip for a reply: it does not interrupt, it does not stack, and the next reply overwrites it, so a player who presses five times reads one line and no scrollback fills with it; chat is where a line is kept, and a refusal is not kept.

## 4. What this engine already holds

- **Cited** (`crates/dsl/src/quest/verb.rs`, `NarrateStyle::Actionbar`): "This is the channel a *reply* uses: it does not interrupt, it does not stack, and it is overwritten by the next one. Every reply the compiler itself writes has always used it." `narrate` carries `style: actionbar` and an optional `sound`.
- **Cited** (`docs/reference/compiler.md`, *The answer is not a mechanism — it is an ordinary trigger*): a press answer is an `EnvTrigger { on: use, audience: presser, once: false, effects: [narrate { style: actionbar, text }] }`, authored by the campaign or synthesized by the compiler for a sealed body from an authored `sealed_hint`; it is dispatched by a `player_interacted_with_entity` advancement whose reward revokes itself, so the object answers every press; it gates through `trigger_poll_guards`, the one authority the tick clauses read.
- **Cited** (`crates/dsl/src/trigger.rs`, `DW0429`): a sealed body with no press answer is refused; "The compiler deliberately does **not** fill that silence. A baked default is the compiler making a design statement … on the author's behalf"; "One rule for the whole pressable class".
- **Cited** (`crates/dsl/src/l10n.rs`, `TextKind::Refusal`): "A message shown when an action fails: says what is wrong and what fixes it" — the class `missing_item_hint`, `sealed_hint` and `world.boundary.message` already belong to.

**Authored — the synthesis.** A refused press is answered on the action bar, to the presser, on every press, with a `Refusal` line the author wrote and a sound the engine plays; the wording is never baked by the engine, and a pressable that would be silent in a state a player can press it in is refused at build, as a sealed body is. The engine supplies the body, the dispatch, the state and the sound; the campaign supplies the words. Spec-0117 is where this binds.

## Sources

- Nielsen Norman Group. *10 Usability Heuristics for User Interface Design*, heuristics 1 and 9. nngroup.com/articles/ten-usability-heuristics/.
- Nielsen Norman Group (Neusesser, Sunwall). *Error-Message Guidelines* (as already entered in `game-writing.md`).
- Norman, D. *The Design of Everyday Things* (revised edition, 2013), the feedback principle and the gulf of evaluation, read through secondary summaries; ideas only (ADR-0013).
- Game Accessibility Guidelines. *Ensure no essential information is conveyed by sounds alone*; *Ensure no essential information (especially instructions) is conveyed by text alone*; *Give a clear indication that interactive elements are interactive*. gameaccessibilityguidelines.com/full-list/.
- Minecraft Wiki. *Data component format* (`minecraft:lock`); *Commands/title* (`actionbar`). minecraft.wiki.
- `crates/delvec/data/sounds-1.21.11.json` and `crates/delvec/data/commands-1.21.11.json` (the pinned game's registries, provenance in `crates/delvec/data/PROVENANCE.md`).
- `docs/reference/game-writing.md` F2 and F3, with the sources they list.
