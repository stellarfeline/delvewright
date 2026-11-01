//! **The sign at the anchor shows the text** (`DW1075`, spec-0128 §8).
//!
//! A `signs` entry gives text to a sign the **piece already placed** — the
//! division of labour a `loot` fill keeps with its container. Whether the
//! anchor's cell holds a sign, and whether each line fits the board that sign
//! draws, are questions only the assembled world answers, so they are a
//! build-tier proof over it.
//!
//! Both failures are silent in the game. `data merge block … {front_text:…}`
//! against a cell with no sign fails without output, and the shipped sign
//! stays blank; a line wider than the board is cut at its edge by the client,
//! which draws only the first wrapped line of each message.
//!
//! ## The board (vanilla, 1.21.11, read from the pinned client jar)
//!
//! - `SignText.LINES` is 4 ([`delvewright_dsl::SIGN_LINES`]).
//! - `SignBlockEntity.getMaxTextLineWidth()` is **90** font pixels, and
//!   `HangingSignBlockEntity` overrides it with **60**.
//! - `AbstractSignRenderer` draws each message as
//!   `font.split(message, maxWidth).get(0)`, so a line wider than the board is
//!   cut at the last break that fits.
//!
//! Which board a block draws on is its block entity's type, read from the one
//! block-entity table ([`crate::compiler::registry::block_entity_type`]):
//! every standing and wall sign is `minecraft:sign`, every hanging and
//! wall-hanging sign `minecraft:hanging_sign`.

use std::collections::BTreeMap;

use crate::compiler::assembled::base_id;
use crate::compiler::failure::Failure;
use crate::compiler::plan::SignPlan;
use delvewright_dsl::{DwCode, ExitTier, L10nDoc};

delvewright_dsl::dw_code! {
    /// `DW1075` (spec-0128 §8): a sign text the sign at its anchor cannot show
    /// — the cell holds no sign, a second text names the same sign, or a line,
    /// in the English or a declared translation, is wider than the board that
    /// sign draws.
    pub const DW_SIGN_CANNOT_SHOW: DwCode = DwCode::new("DW1075", ExitTier::Build);
}

/// The block entity a standing or wall sign carries.
pub const SIGN_ENTITY: &str = "minecraft:sign";
/// The block entity a hanging or wall-hanging sign carries.
pub const HANGING_SIGN_ENTITY: &str = "minecraft:hanging_sign";

/// `SignBlockEntity.getMaxTextLineWidth()` in the pinned client jar.
pub const SIGN_LINE_WIDTH: u32 = 90;
/// `HangingSignBlockEntity.getMaxTextLineWidth()` in the pinned client jar.
pub const HANGING_SIGN_LINE_WIDTH: u32 = 60;

/// The board a block state draws text on: its block entity type and line
/// width, or `None` when the block is not a sign.
pub fn board(state: &str) -> Option<(&'static str, u32)> {
    match crate::compiler::registry::block_entity_type(base_id(state))? {
        SIGN_ENTITY => Some((SIGN_ENTITY, SIGN_LINE_WIDTH)),
        HANGING_SIGN_ENTITY => Some((HANGING_SIGN_ENTITY, HANGING_SIGN_LINE_WIDTH)),
        _ => None,
    }
}

/// The width a sign line draws at, in font pixels: the line as authored, its
/// markup dropped and its bold spans widened — the measure every width check
/// of this compiler uses ([`crate::compiler::textfit`]).
pub fn line_width(text: &str) -> u32 {
    crate::compiler::textfit::default_font_width(&delvewright_dsl::textstyle::measured(text))
        + crate::compiler::textfit::bold_widening(text)
}

/// What the sign proof bound to on one build.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignBinding {
    /// Sign texts that resolved to a cell.
    pub texts: usize,
    /// The texts owed to a place's detail, each naming the text and the place.
    pub deferred: Vec<String>,
    /// Judged texts by the board standing at their cell.
    pub by_board: BTreeMap<&'static str, usize>,
    /// Lines measured, over the English and every declared translation.
    pub lines: usize,
    /// Languages measured beside the English.
    pub languages: usize,
}

impl SignBinding {
    /// One line, for stderr: the denominator, how many were judged against the
    /// world, and every deferral by name.
    #[must_use]
    pub fn line(&self) -> String {
        let judged = self.texts - self.deferred.len();
        let boards = self
            .by_board
            .iter()
            .map(|(k, n)| format!("{k}: {n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let named = if self.deferred.is_empty() {
            String::new()
        } else {
            format!(
                " ({}); a deferred sign is owed by its place's detail, and the build that details \
                 the place refuses it (DW1075) if the piece does not stand it there",
                self.deferred.join(", ")
            )
        };
        format!(
            "sign binding: {t} sign text(s), {judged} judged against the assembled world{b}, {l} \
             line(s) measured over the English and {n} declared language(s), {d} deferred to the \
             detail of a place still standing in its stand-in{named}.",
            t = self.texts,
            b = if boards.is_empty() {
                String::new()
            } else {
                format!(" ({boards})")
            },
            l = self.lines,
            n = self.languages,
            d = self.deferred.len(),
        )
    }
}

/// **Build-tier proof** (`DW1075`): every sign text names a cell holding a
/// sign no other text names, and every line of it — in the English and in
/// every declared translation — fits the board that sign draws.
///
/// A text whose cell holds no sign and lies inside a place still standing in
/// its stand-in ([`crate::compiler::blockout::Blockout::stand_in_holding`]) is
/// owed by that place's detail: it is counted and named on the binding line,
/// and judged by the build that details the place — the rule the container
/// proofs keep (`crate::compiler::loot::split_owed_by_detail`).
pub fn check_signs(
    blocks: &crate::compiler::blockstate::BlockMap,
    blockout: Option<&crate::compiler::blockout::Blockout>,
    signs: &[SignPlan],
    sidecars: &BTreeMap<String, L10nDoc>,
) -> (SignBinding, Result<(), Failure>) {
    let mut bind = SignBinding {
        texts: signs.len(),
        languages: sidecars.len(),
        ..SignBinding::default()
    };
    let mut faults: Vec<String> = Vec::new();
    let mut named: BTreeMap<[i32; 3], &str> = BTreeMap::new();
    for s in signs {
        let c = s.cell;
        let at = format!(
            "sign `{}` -> anchor `{}` at [{}, {}, {}]",
            s.id, s.anchor, c[0], c[1], c[2]
        );
        let found = blocks
            .get(&c)
            .map(|b| b.as_str())
            .unwrap_or("minecraft:air");
        let Some((entity, width)) = board(found) else {
            if let Some(place) = blockout.and_then(|b| b.stand_in_holding(c)) {
                bind.deferred.push(format!("sign `{}` in `{place}`", s.id));
                continue;
            }
            faults.push(format!(
                "  {at} holds `{found}`, which is not a sign: a sign text writes the words of a \
                 sign the piece placed and never places one"
            ));
            continue;
        };
        *bind.by_board.entry(entity).or_insert(0) += 1;
        if let Some(prev) = named.insert(c, s.id.as_str()) {
            faults.push(format!(
                "  {at} names the sign sign `{prev}` already writes: a sign shows one text, so \
                 the later `data merge` would overwrite the earlier one — write both into ONE entry"
            ));
        }
        for (side, lines) in [("front", &s.front), ("back", &s.back)] {
            for (i, line) in lines.iter().enumerate() {
                let (key, english) = match delvewright_dsl::l10n::untag(line) {
                    Some((k, e)) => (Some(k), e),
                    None => (None, line.as_str()),
                };
                let mut renditions: Vec<(String, &str)> = vec![("English".to_string(), english)];
                if let Some(k) = key {
                    for (lang, doc) in sidecars {
                        if let Some(t) = doc.content.get(k) {
                            renditions.push((format!("`{lang}` (`{k}`)"), t.as_str()));
                        }
                    }
                }
                for (which, text) in renditions {
                    bind.lines += 1;
                    let w = line_width(text);
                    if w > width {
                        faults.push(format!(
                            "  {at}: {side} line {} in {which} is {w} font px wide, over the \
                             {width} px a `{entity}` board draws — the client cuts it at the \
                             edge: `{text}`",
                            i + 1
                        ));
                    }
                }
            }
        }
    }
    if faults.is_empty() {
        return (bind, Ok(()));
    }
    let result = Err(Failure {
        code: DW_SIGN_CANNOT_SHOW,
        message: format!(
            "{} sign text fault(s): the sign at the anchor cannot show the text.\n{}\n  A sign's \
             board is {SIGN_LINE_WIDTH} font px wide on a standing or wall sign and \
             {HANGING_SIGN_LINE_WIDTH} on a hanging one (Minecraft 1.21.11); a line wider than \
             its board is cut, so shorten the line, move words to the next line (four to a \
             side), or stand a wider sign. A missing sign is the piece's to place: point the \
             entry at the anchor the piece declares at its sign's cell, or have the piece stand \
             a sign there — do NOT place one with a `set-block` effect.",
            faults.len(),
            faults.join("\n"),
        ),
    });
    (bind, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::{BlockMap, BlockState};

    fn plan(id: &str, cell: [i32; 3], front: &[&str]) -> SignPlan {
        SignPlan {
            id: id.to_string(),
            anchor: format!("anchor/{id}"),
            cell,
            front: front.iter().map(|s| (*s).to_string()).collect(),
            back: Vec::new(),
        }
    }

    fn world(cells: &[([i32; 3], &str)]) -> BlockMap {
        let mut m = BlockMap::new();
        for (c, s) in cells {
            m.insert(*c, BlockState::new(s));
        }
        m
    }

    /// Every sign of every wood is a board, standing and wall at 90 px,
    /// hanging and wall-hanging at 60; nothing else is.
    #[test]
    fn every_sign_draws_on_its_board_and_nothing_else_does() {
        assert_eq!(
            board("minecraft:oak_sign[rotation=0,waterlogged=false]"),
            Some((SIGN_ENTITY, 90))
        );
        assert_eq!(
            board("minecraft:spruce_wall_sign[facing=south,waterlogged=false]"),
            Some((SIGN_ENTITY, 90))
        );
        assert_eq!(
            board("minecraft:cherry_hanging_sign[attached=false,rotation=4,waterlogged=false]"),
            Some((HANGING_SIGN_ENTITY, 60))
        );
        assert_eq!(
            board("minecraft:bamboo_wall_hanging_sign[facing=east,waterlogged=false]"),
            Some((HANGING_SIGN_ENTITY, 60))
        );
        assert_eq!(board("minecraft:chest[facing=north,type=single]"), None);
        assert_eq!(board("minecraft:air"), None);
        let table = include_str!("../../data/block-entities-1.21.11.tsv");
        let signs = table
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('\t'))
            .filter(|(b, _)| board(b).is_some())
            .count();
        assert_eq!(
            signs, 48,
            "24 woods × standing/wall/hanging/wall-hanging, less none"
        );
    }

    /// A text on a sign that fits is judged and passes; on a cell holding no
    /// sign it is `DW1075`; two texts on one sign are `DW1075`.
    #[test]
    fn a_text_needs_a_sign_of_its_own() {
        let w = world(&[
            (
                [0, 1, 0],
                "minecraft:oak_wall_sign[facing=south,waterlogged=false]",
            ),
            ([2, 1, 0], "minecraft:stone"),
        ]);
        let none = BTreeMap::new();
        let (b, r) = check_signs(&w, None, &[plan("a", [0, 1, 0], &["Bakery"])], &none);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(
            (b.texts, b.lines, b.by_board.get(SIGN_ENTITY)),
            (1, 1, Some(&1))
        );
        let (_, r) = check_signs(&w, None, &[plan("b", [2, 1, 0], &["Bakery"])], &none);
        let e = r.unwrap_err();
        assert_eq!(e.code, "DW1075");
        assert!(
            e.message.contains("holds `minecraft:stone`"),
            "{}",
            e.message
        );
        let (_, r) = check_signs(
            &w,
            None,
            &[
                plan("a", [0, 1, 0], &["One"]),
                plan("c", [0, 1, 0], &["Two"]),
            ],
            &none,
        );
        assert!(r.unwrap_err().message.contains("sign `a` already writes"));
    }

    /// A line wider than the board refuses, on a hanging sign at a width a
    /// standing sign takes; a translation is measured beside its English.
    #[test]
    fn a_line_wider_than_its_board_is_refused_in_every_language() {
        // "Apothecary" is 58 px; "The Apothecary" is 80 px.
        assert_eq!(line_width("Apothecary"), 58);
        assert_eq!(line_width("The Apothecary"), 80);
        let w = world(&[
            (
                [0, 1, 0],
                "minecraft:oak_sign[rotation=0,waterlogged=false]",
            ),
            (
                [1, 1, 0],
                "minecraft:oak_hanging_sign[attached=false,rotation=0,waterlogged=false]",
            ),
        ]);
        let none = BTreeMap::new();
        let (_, r) = check_signs(
            &w,
            None,
            &[plan("s", [0, 1, 0], &["The Apothecary"])],
            &none,
        );
        assert!(r.is_ok());
        let (_, r) = check_signs(
            &w,
            None,
            &[plan("h", [1, 1, 0], &["The Apothecary"])],
            &none,
        );
        let e = r.unwrap_err();
        assert!(
            e.message.contains("80 font px wide, over the 60 px"),
            "{}",
            e.message
        );
        let tagged = delvewright_dsl::l10n::tag("sign.h.front.0", "Apothecary");
        let doc: L10nDoc = serde_json::from_value(serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "t",
            "kind": "l10n",
            "lang": "zh-cn",
            "content": {"sign.h.front.0": "药剂师药剂师药剂师药剂师"}
        }))
        .unwrap();
        let sidecars = BTreeMap::from([("zh-cn".to_string(), doc)]);
        let (b, r) = check_signs(&w, None, &[plan("h", [1, 1, 0], &[&tagged])], &sidecars);
        assert_eq!((b.lines, b.languages), (2, 1));
        let e = r.unwrap_err();
        assert!(
            e.message.contains("`zh-cn` (`sign.h.front.0`)"),
            "{}",
            e.message
        );
        assert!(!e.message.contains("in English"), "{}", e.message);
    }
}
