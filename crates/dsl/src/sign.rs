//! Signs: the words a sign the world holds says (spec-0115).
//!
//! A sign block enters a delve's world by a piece's template, a `prop` or a
//! `set-block` / `fill-region`; what it says is its block entity's
//! `front_text` / `back_text`, which no block string carries. The words are
//! the campaign's, declared against the anchor where the sign stands
//! ([`Sign`]), and a beat rewrites them with `letter-sign`
//! ([`crate::Verb::LetterSign`]).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::AnchorProviders;
use crate::{AnchorId, SignId, Verb};

/// A stage-5 **sign** (spec-0115): the words on the sign block an anchor's
/// cell holds, or comes to hold, on either face.
///
/// The engine writes `is_waxed: 1b` on every sign it letters, and there is no
/// field for it: in adventure mode no player can open, dye or glow a sign
/// either way, and the flag states the text final under every other mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sign {
    /// Unique sign id (`sign/<kebab>`).
    pub id: SignId,
    /// The anchor whose cell holds, or comes to hold, the sign block: a point
    /// anchor, or a region resolving to its `from` cell. The cell is proven to
    /// hold a sign in a configuration the party can be in (`DW0431`); one row
    /// per anchor (`DW0435`).
    pub anchor: AnchorId,
    /// What the front face says.
    pub front: SignFace,
    /// What the back face says. Absent, the back is blank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<SignFace>,
}

/// One face of a sign: up to four lines, a dye colour, glowing or not.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignFace {
    /// One to four lines, top to bottom. `""` leaves a line blank; a face
    /// shorter than four lines is blank below. Each line is a player-facing
    /// string: it is translated, may carry styled spans, and must fit the
    /// face (`DW1048`).
    #[schemars(length(min = 1, max = 4))]
    pub lines: Vec<String>,
    /// The dye the text is drawn in. Absent = `black`, the game's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Dye>,
    /// Whether the text glows. Absent = `false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub glowing: bool,
}

impl SignFace {
    /// The dye this face is drawn in.
    pub fn dye(&self) -> Dye {
        self.color.unwrap_or(Dye::Black)
    }
}

/// The sixteen dye colours a sign's text can take, in the game's own
/// `DyeColor` order. Not the chat palette: `dark_purple`, `light_purple` and
/// `dark_gray` are not dyes.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Dye {
    /// `white`.
    White,
    /// `orange`.
    Orange,
    /// `magenta`.
    Magenta,
    /// `light_blue`.
    LightBlue,
    /// `yellow`.
    Yellow,
    /// `lime`.
    Lime,
    /// `pink`.
    Pink,
    /// `gray`.
    Gray,
    /// `light_gray`.
    LightGray,
    /// `cyan`.
    Cyan,
    /// `purple`.
    Purple,
    /// `blue`.
    Blue,
    /// `brown`.
    Brown,
    /// `green`.
    Green,
    /// `red`.
    Red,
    /// `black`.
    Black,
}

impl Dye {
    /// The name the game's `DyeColor` codec reads.
    pub fn token(self) -> &'static str {
        match self {
            Dye::White => "white",
            Dye::Orange => "orange",
            Dye::Magenta => "magenta",
            Dye::LightBlue => "light_blue",
            Dye::Yellow => "yellow",
            Dye::Lime => "lime",
            Dye::Pink => "pink",
            Dye::Gray => "gray",
            Dye::LightGray => "light_gray",
            Dye::Cyan => "cyan",
            Dye::Purple => "purple",
            Dye::Blue => "blue",
            Dye::Brown => "brown",
            Dye::Green => "green",
            Dye::Red => "red",
            Dye::Black => "black",
        }
    }
}

/// Which face of a sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignSide {
    /// `front_text`.
    Front,
    /// `back_text`.
    Back,
}

impl SignSide {
    /// The key segment and the word: `front` / `back`.
    pub fn token(self) -> &'static str {
        match self {
            SignSide::Front => "front",
            SignSide::Back => "back",
        }
    }
}

/// **The width of a sign's face is the block's kind** (spec-0115 §2.1): a
/// standing or wall sign makes a `SignBlockEntity` whose line is 90 font
/// pixels; a ceiling or wall hanging sign makes a `HangingSignBlockEntity`
/// whose line is 60.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SignKind {
    /// `<wood>_sign` and `<wood>_wall_sign`: 90 font pixels.
    Standing,
    /// `<wood>_hanging_sign` and `<wood>_wall_hanging_sign`: 60 font pixels.
    Hanging,
}

impl SignKind {
    /// The kind of sign `block` is (a namespaced id, with or without a
    /// blockstate suffix), or `None` when it is no sign: read from the name as
    /// the game's own block classes split them.
    pub fn of(block: &str) -> Option<SignKind> {
        let id = block.split(['[', '{']).next().unwrap_or(block);
        let id = id.rsplit(':').next().unwrap_or(id);
        if !id.ends_with("_sign") {
            return None;
        }
        Some(if id.contains("hanging") {
            SignKind::Hanging
        } else {
            SignKind::Standing
        })
    }

    /// The face's line width, in font pixels (`getMaxTextLineWidth`).
    pub fn budget(self) -> u32 {
        match self {
            SignKind::Standing => 90,
            SignKind::Hanging => 60,
        }
    }

    /// The face in words, for a message.
    pub fn word(self) -> &'static str {
        match self {
            SignKind::Standing => "standing or wall sign",
            SignKind::Hanging => "hanging sign",
        }
    }

    /// The budget stated in characters for a translator: typical Latin
    /// letters are 6 font pixels, Han glyphs 9.
    pub fn in_characters(self) -> String {
        let b = self.budget();
        format!(
            "about {b} font pixels: {} Latin or {} Han characters",
            b / 6,
            b / 9
        )
    }
}

/// The block a campaign document writes at a cell, wherever it is stated
/// there: a trigger's or an `interact`'s `prop`, a `set-block`, or a
/// `fill-region` centred on the anchor. Used to read the kind of sign a
/// [`Sign`] row's cell will hold when the document says so; a template's
/// sign is the build's to read.
pub fn document_sign_block<'a>(c: &'a Campaign, anchor: &str) -> Option<&'a str> {
    let mut found: Option<&'a str> = None;
    for t in &c.quests.content.triggers {
        if t.at_anchor() == Some(anchor)
            && let Some(p) = &t.prop
            && SignKind::of(&p.block).is_some()
        {
            found.get_or_insert(p.block.as_str());
        }
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if let crate::Objective::Interact {
                anchor: a,
                prop: Some(p),
                ..
            } = o
                && a.as_str() == anchor
                && SignKind::of(&p.block).is_some()
            {
                found.get_or_insert(p.block.as_str());
            }
        }
    }
    crate::for_each_campaign_effect(c, &mut |_path, _site, e| match &e.verb {
        Verb::SetBlock { anchor: a, block } if a.as_str() == anchor => {
            if SignKind::of(block).is_some() {
                found.get_or_insert(block.as_str());
            }
        }
        Verb::FillRegion { region, block } if region.anchor.as_str() == anchor => {
            if SignKind::of(block).is_some() {
                found.get_or_insert(block.as_str());
            }
        }
        _ => {}
    });
    found
}

/// One sign line a campaign states, wherever it is stated: a [`Sign`] row's
/// face or a `letter-sign`'s. The one enumeration the inventory, the fit
/// check and the binding line read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignLine {
    /// The sign the line is on.
    pub sign: String,
    /// The face.
    pub side: SignSide,
    /// The line's index on its face.
    pub index: usize,
    /// The l10n key (`sign.<s>.front.<i>`, `fx.….front.<j>`).
    pub key: String,
    /// The stage document and the JSON pointer of the line.
    pub stage: &'static str,
    /// The JSON pointer.
    pub path: String,
    /// The line as authored.
    pub text: String,
    /// The face's other lines, in order, for a translator.
    pub face: Vec<String>,
}

/// Every face a campaign states for a sign: `(sign, side, face, keybase,
/// stage, pointer)`, declarations first, then each `letter-sign` in effect
/// order.
pub fn stated_faces(
    c: &Campaign,
) -> Vec<(String, SignSide, &SignFace, String, &'static str, String)> {
    let mut out = Vec::new();
    for (i, s) in c.quests.content.signs.iter().enumerate() {
        let sl = crate::l10n::local_id(s.id.as_str()).to_string();
        out.push((
            s.id.as_str().to_string(),
            SignSide::Front,
            &s.front,
            format!("sign.{sl}.front"),
            "quests",
            format!("/content/signs/{i}/front"),
        ));
        if let Some(b) = &s.back {
            out.push((
                s.id.as_str().to_string(),
                SignSide::Back,
                b,
                format!("sign.{sl}.back"),
                "quests",
                format!("/content/signs/{i}/back"),
            ));
        }
    }
    crate::l10n::each_effect_site(c, &mut |stage, path, keybase, eff| {
        if let Verb::LetterSign { sign, front, back } = &eff.verb {
            for (side, face) in [(SignSide::Front, front), (SignSide::Back, back)] {
                if let Some(f) = face {
                    out.push((
                        sign.as_str().to_string(),
                        side,
                        f,
                        format!("{keybase}.{}", side.token()),
                        stage,
                        format!("{path}/{}", side.token()),
                    ));
                }
            }
        }
    });
    out
}

/// Every non-blank sign line the campaign states, with its key ([`SignLine`]).
/// A blank line is not a row.
pub fn sign_lines(c: &Campaign) -> Vec<SignLine> {
    let mut out = Vec::new();
    for (sign, side, face, keybase, stage, path) in stated_faces(c) {
        for (i, line) in face.lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            out.push(SignLine {
                sign: sign.clone(),
                side,
                index: i,
                key: format!("{keybase}.{i}"),
                stage,
                path: format!("{path}/lines/{i}"),
                text: line.clone(),
                face: face.lines.clone(),
            });
        }
    }
    out
}

/// Stage-5 `signs` and every `letter-sign` (spec-0115): the id's syntax and
/// uniqueness (`DW0110` / `DW0111`), the anchor's resolution (`DW0142`), one
/// row per anchor (`DW0435`), a `letter-sign` naming a declared sign
/// (`DW0112`) and stating a face (`DW0100`).
///
/// Whether the anchor's cell really holds a sign is the build's proof
/// (`DW0431`), and whether a line fits its face is `DW1048`'s.
pub(crate) fn sign_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let signs = &c.quests.content.signs;
    let declared: BTreeSet<&str> = signs.iter().map(|s| s.id.as_str()).collect();
    crate::for_each_campaign_effect(c, &mut |path, site, e| {
        let Verb::LetterSign { sign, front, back } = &e.verb else {
            return;
        };
        let stage = if matches!(site, crate::EffectSite::DialogueRespawn { .. }) {
            "dialogue"
        } else {
            "quests"
        };
        if !declared.contains(sign.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                format!("{path}/sign"),
                format!(
                    "`letter-sign` names sign `{sign}`, which no `signs[]` row declares. A beat \
                     re-letters a sign the campaign declared at its anchor: declare it in \
                     `signs[]` (its `anchor`, and what it says first), or name one of: {}",
                    if declared.is_empty() {
                        "(this campaign declares no sign)".to_string()
                    } else {
                        declared.iter().copied().collect::<Vec<_>>().join(", ")
                    }
                ),
            ));
        }
        if front.is_none() && back.is_none() {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                path.to_string(),
                format!(
                    "`letter-sign` on `{sign}` states neither `front` nor `back`, so it says \
                     nothing. State the face (or both faces) the beat rewrites, whole"
                ),
            ));
        }
    });
    if signs.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    let mut seen_anchor: BTreeMap<&str, &str> = BTreeMap::new();
    for (i, s) in signs.iter().enumerate() {
        let at = |tail: &str| format!("/content/signs/{i}{tail}");
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                at("/id"),
                format!(
                    "malformed sign id `{}` — sign ids must be lowercase kebab-case with the \
                     `sign/` prefix (e.g. `sign/the-label`)",
                    s.id
                ),
            ));
        }
        if !seen_id.insert(s.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                at("/id"),
                format!("duplicate sign id `{}`", s.id),
            ));
        }
        if !providers.resolvable(s.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                at("/anchor"),
                format!(
                    "sign `{}` names anchor `{}`, which no prefab bound in this campaign \
                     provides — {}",
                    s.id,
                    s.anchor,
                    providers.anchor_remedy(
                        "use the anchor the piece marks where its sign stands (anchor names \
                         come from prefab metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        if let Some(prev) = seen_anchor.insert(s.anchor.as_str(), s.id.as_str()) {
            d.push(Diagnostic::error(
                crate::loot::LOOT_DUPLICATE_ANCHOR,
                "quests",
                at("/anchor"),
                format!(
                    "sign `{prev}` and sign `{}` both letter anchor `{}`. One cell holds one \
                     sign, and the later write would replace the earlier one's words whole. \
                     Declare ONE row for the sign; a change of words at a beat is a \
                     `letter-sign`",
                    s.id, s.anchor
                ),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The budget is the block's kind, over every sign block of the pinned
    /// registry: 48 sign blocks, twelve woods by four shapes, the two hanging
    /// shapes 60 and the other two 90.
    #[test]
    fn every_pinned_sign_block_has_its_face_width() {
        let blocks: Vec<&str> = crate::blocks::BlockRegistry::v1_21_11()
            .ids()
            .filter(|b| SignKind::of(b).is_some())
            .collect();
        assert_eq!(blocks.len(), 48, "{blocks:?}");
        let mut hanging = 0;
        for b in &blocks {
            let k = SignKind::of(b).unwrap();
            if b.ends_with("_hanging_sign") {
                assert_eq!(k.budget(), 60, "{b}");
                hanging += 1;
            } else {
                assert!(b.ends_with("_sign"), "{b}");
                assert_eq!(k.budget(), 90, "{b}");
            }
        }
        assert_eq!(hanging, 24);
        assert_eq!(
            SignKind::of("minecraft:oak_sign[rotation=8]"),
            Some(SignKind::Standing)
        );
        assert_eq!(SignKind::of("minecraft:stone"), None);
        assert_eq!(SignKind::of("minecraft:design"), None);
    }

    /// The sixteen dye names are the game's, in its order.
    #[test]
    fn the_dyes_are_the_games_sixteen() {
        let all = [
            Dye::White,
            Dye::Orange,
            Dye::Magenta,
            Dye::LightBlue,
            Dye::Yellow,
            Dye::Lime,
            Dye::Pink,
            Dye::Gray,
            Dye::LightGray,
            Dye::Cyan,
            Dye::Purple,
            Dye::Blue,
            Dye::Brown,
            Dye::Green,
            Dye::Red,
            Dye::Black,
        ];
        let names: Vec<&str> = all.iter().map(|d| d.token()).collect();
        for (d, n) in all.iter().zip(&names) {
            assert_eq!(serde_json::to_value(d).unwrap(), serde_json::json!(n));
        }
        assert!(serde_json::from_value::<Dye>(serde_json::json!("dark_purple")).is_err());
    }
}
