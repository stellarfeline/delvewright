# spec-0116: A direction names what the player sees — a compass point in player-facing text is refused

- **Status**: Proposed
- **Ground**: written against engine `b24550dcf` (`origin/main`), read only — `each_string`, `inventory`, `TextKind` and `key_kind` in `crates/dsl/src/l10n.rs`; `SURFACE` in `crates/dsl/tests/l10n_surface.rs`; `validate_inline_styles` in `crates/dsl/src/textstyle.rs` and its call site in `crates/delvec/src/cli/campaign.rs`; `docs/reference/game-writing.md` F1.
- **What it is for**: a hint read in a playtest — *Marra's house is the last on the north side of the street* — could not be acted on: Minecraft shows no compass, and a player does not know where north is. `game-writing.md` F1 already says a place is named by a landmark the player can see and a position relative to it or to the player, never by a compass point; nothing enforced it. This spec makes the engine refuse the compass half.
- **Numbers**: spec `0116`, one diagnostic, `DW1049`. No DSL surface changes, so no `dsl_version` step.
- **Non-goals**: judging whether a landmark a line names is built (a separate, open finding); judging a sidecar's wording; judging a line's other directions (`left`, `behind you` are what the rule asks for); a declaration surface for a name that is not one of the campaign's own names.

## 1. What is judged

**Cited** (`l10n.rs`): every player-facing string the DSL carries is visited by `each_string`, and `inventory` is its key → English map; `l10n_surface.rs` holds the classification of every string field of the stage schemas exact against those schemas — 119 fields, 36 `Inventoried`. The DSL carries no sign or book text.

**Authored.** `DW1049` reads `inventory` and judges every line whose `key_kind` is not `Name`, `ItemName` or `Title` — 22 of the 36 fields. The other 14 are the campaign's **declared names**.

## 2. The rule

**Authored**, each part chosen so it is decided by the document alone:

1. **Words.** The visible text (`l10n::plain`) splits into maximal runs of alphabetic characters.
2. **Compass word.** Lower-cased: `north`, `south`, `east`, `west`, `northeast`, `northwest`, `southeast`, `southwest`, bare or with `ern`, `ernmost`, `erly`, `ward`, `wards`. `north-east` is two words. A word containing a cardinal (`Westmarch`) is a different word. Abbreviations: `NE`, `NW`, `SE`, `SW` and the eight three-letter points, case-sensitive upper case; a single letter is never judged, because `N`, `S`, `E`, `W` are ordinary letters in prose.
3. **Proper names.** A compass word inside a run of words equal, ignoring case, to a declared name's words (leading `the`/`a`/`an` dropped) is a name. Capitalisation decides nothing: a capitalised-word heuristic cannot be stated exactly (a sentence's first word is capitalised).
4. **English only.** Every other language is transcreated from its English line (CLAUDE.md Conventions) and held to it by `DW0187`/`DW0188`; the direction a line gives is decided in the English. A per-language list would have to be cited per language, and none is held.

The message names the key, the text kind, the word and the line, and prescribes body- or landmark-relative wording (`turn around: the first door on your right`).

## Acceptance criteria

1. `crates/dsl/src/compass.rs` declares `DW1049` with `dw_code!`; `delvec validate` raises it through `dsl::compass::validate_compass_directions` and prints a `compass binding (DW1049):` line with judged, declared-name, exempt and refused counts.
2. `crates/dsl/tests/compass.rs::every_judged_line_is_refused_and_no_name_is`: a compass word planted in every inventoried string of the keep-trial fixture is refused in exactly the inventory keys whose kind is not a name, one diagnostic each, and in none of the names.
3. `compass.rs::a_declared_name_is_not_a_direction`: with an area named `The North Gate`, `by the North Gate` and `by the north gate` pass, `the north stair` is refused, `NE` is refused, `Westmarch` and `Northmen's` pass; without the declaration the same line is refused.
4. The gallery validates with zero `DW1049`, and the committed probe `gallery/probes/a-compass-point-in-a-hint/` (the primary plus one declared edit planting a compass word in a hint) is refused with `DW1049` by `check-gallery-coverage.py`.
5. `docs/reference/dsl/compass.md` carries the `DW1049` row with the field count and its denominator; `check-dw-codes.py` is green with and without `--delvec`.
6. `docs/playtest-findings.json` carries a row whose carrier is `DW1049`, with a non-zero binding over the campaign it was found on.
