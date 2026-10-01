# spec-0078: Every dialog button has a tooltip — one optional hover box on every button the player sees

- **Status**: Proposed
- **Ground**: written against engine `c9e87fae`, read only — `emit_dialogs` and `build_node_dialog` in `crates/delvec/src/compiler/emit.rs`; `Verb::Bonfire`, `BonfireLabels`, `DialogueOption`, `ShopOffer`, `Class` in `crates/dsl/src/stages.rs`; `effect_strings` and the key table in `crates/dsl/src/l10n.rs`; `CheckpointPlan` in `crates/delvec/src/compiler/plan.rs`; `crates/delvec/tests/v08_option_tooltip.rs`; `crates/dsl/tests/l10n_surface.rs`; the gallery's bonfire (`gallery/quests.json`) and sidecars (`gallery/l10n/`, `gallery/overlays/ocean-horizon/l10n/`).
- **What it is for**: a button's caption is a few words on a fixed 150-GUI-px face. What pressing it does often needs a sentence. The engine already lets a dialogue option and a shop offer carry that sentence in a hover tooltip; the bonfire's two buttons cannot. The capability belongs to the object class *a dialog button the player sees*, not to the verb that first needed it (CLAUDE.md, *This is a general engine* — **cited**), so every button gets it, in one shape, from one emitter.
- **Numbers**: no ADR, no DW code. **`dsl_version` moves one minor step over the base's** (ADR-0024 Decision 4 — **cited**): §3 adds two fields to the `bonfire` effect. The value is typed in `crates/dsl/Cargo.toml` and nowhere else.
- **Non-goals**: a tooltip on the default button of a `minecraft:notice` (the engine authors no button there, §1); a `width` surface on any button; a width budget on a tooltip; any change to a label, its `DW0331` budget, or a button's action.

## 1. The buttons the engine presents

**Cited** (the tree at `c9e87fae`): the engine emits player-facing dialog buttons at four sites, all `minecraft:multi_action` action buttons:

| Dialog | Button | Tooltip at the base |
|---|---|---|
| `class_select` | one per class | always: the class's required `blurb` |
| `bonfire_<i>` | *rest and save*, *save only* | none |
| `shop_<i>` | one per offer | optional `offers[].tooltip` |
| `<npc>_<node>[__m<mask>]` | one per visible option | optional `options[].tooltip` |

A node with no visible option is a `minecraft:notice` with no `action`: vanilla draws its own default button and the engine states none, so there is nothing to carry a tooltip. No other site writes a dialog. The four sites wrote their buttons with three separate bits of code, two of which attached a tooltip in two different ways.

## 2. The codec

**Cited**: vanilla 1.21.11's dialog action button is `ActionButton(CommonButtonData, Optional<DialogAction>)`, and `CommonButtonData` is `label` + optional `tooltip` + `width` (default 150), read off the pinned client jar and recorded on `build_node_dialog`. Every site in §1 is that button, so `tooltip` is accepted on all of them. The class button has shipped a tooltip since the first engine, and every release boots it on the pinned server.

## 3. The shape

**Authored.**

1. **One emitter.** `emit::dialog_button(label, tooltip, command)` builds every button in §1: the label component, then `tr(tooltip)` under `tooltip` only when one is stated, then a `minecraft:run_command` action running `command`. No site builds a button any other way.
2. **The bonfire.** The `bonfire` effect gains `rest_tooltip?` and `save_tooltip?`, flat, beside its flat `rest_label?` / `save_label?`. Flat rather than a nested `{label, tooltip}` per button because the bonfire already holds both buttons' captions as flat siblings on the effect; a nested object would rename the two existing fields for no capability. A dialogue option's `tooltip` is the sibling of its `label` on the object that is the button; on the bonfire the effect holds the buttons, so `rest_tooltip` is the sibling of `rest_label`.
3. **Keys.** `fx.….rest_tooltip` / `fx.….save_tooltip`, beside `fx.….rest_label` / `.save_label`, inventoried only when stated — the same "button key, then `tooltip`" convention as `dlg.<n>.<node>.opt.<i>.tooltip` and `shop.<s>.offer.<i>.tooltip`.
4. **The class button keeps `blurb`.** It is the class button's tooltip and stays required: the class screen is the one dialog where every button must explain itself. No second `tooltip` field is added beside it (a second field for one slot is the defect, CLAUDE.md — **cited**).
5. **Absent means absent.** A button with no stated tooltip emits no `tooltip` key, so a campaign that states none builds byte-identically to the base (ADR-0006 — **cited**).
6. **No width check.** A tooltip wraps at 170 px in its own hover box, so `DW0331` does not reach it (**cited**, the option tooltip's existing rule).

## Acceptance criteria

Each criterion is checked against the tree at `c9e87fae`: criterion 4 holds there and must keep holding; none of the others is satisfied there.

1. **Surface.** `delvec schema --stage all` exports `rest_tooltip` and `save_tooltip` on the `bonfire` effect as optional strings; `crates/dsl/Cargo.toml` is one minor step over the base's and `tools/lib/version_sites.py verify` is green.
2. **One emitter.** `crates/delvec/src/compiler/emit.rs` builds every dialog button through `dialog_button`. `crates/delvec/tests/button_tooltips.rs::every_emitted_button_has_the_one_shape` reads every dialog of the `souls-bonfire` fixture build and asserts each button holds only `label`, an optional `tooltip` and a `/trigger` `run_command` action, with a non-zero button count printed.
3. **Bonfire emission.** `button_tooltips.rs`: a stated `rest_tooltip` / `save_tooltip` lands on its own button as `{"translate": "delve.<ns>.fx.….rest_tooltip", "fallback": …}` (resp. `save_tooltip`), the action unchanged; one stated without the other leaves the other button with no `tooltip`. Perturbation: changing the stated text moves `bonfire_0.json` and no other dialog file; stating a tooltip moves `bonfire_0.json` and no other datapack file.
4. **Existing tooltips unchanged.** `crates/delvec/tests/v08_option_tooltip.rs` (option tooltip present, absent, class blurb) and the shop tooltip tests stay green.
5. **Inventory.** `crates/dsl/src/l10n.rs` inventories `fx.….rest_tooltip` / `.save_tooltip` when stated and its key table names them; `crates/dsl/tests/l10n_surface.rs` classifies both fields `Inventoried`.
6. **Gallery** (spec-0039). `gallery/quests.json`'s bonfire states both tooltips; `gallery/l10n/zh-cn.json` and `gallery/overlays/ocean-horizon/l10n/zh-cn.json` translate both keys; `check-gallery-coverage.py` reports `QuestEffect::bonfire.rest_tooltip` and `.save_tooltip` bound and 0 units in neither state. The regenerated `gallery/baseline/` moves `bonfire_0.json` only in points whose bonfire states a tooltip, and the commit body attributes every moved row.
7. **Docs, same change.** `docs/reference/compiler.md`: the `bonfire` row names both fields; a *Dialog buttons* table lists the four sites and their tooltip sources; the l10n key list names both keys. `.claude/skills/delvewright/skills/new-delve/references/quest-capabilities.md` says every dialog button takes the same optional tooltip; `check-skill-page.py` green. `docs/demo-levels.md` gains a row for this spec; `docs/specs/README.md` gains this spec's row.
