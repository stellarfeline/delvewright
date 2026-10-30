//! Sign texts (spec-0128 §8): the words a sign the piece placed shows.

use super::*;

/// A resolved sign text: the cell of the sign and the lines of each side, as
/// the campaign reaching emission carries them (translation-tagged).
#[derive(Clone, Debug, PartialEq)]
pub struct SignPlan {
    /// Sign id (`sign/<kebab>`).
    pub id: String,
    /// The anchor named by the declaration.
    pub anchor: String,
    /// The world cell of the sign.
    pub cell: [i32; 3],
    /// The front's lines, top to bottom.
    pub front: Vec<String>,
    /// The back's lines, top to bottom; empty leaves the back as the piece
    /// left it.
    pub back: Vec<String>,
}

/// Resolve every stage-5 `signs` declaration to a world cell. An unresolvable
/// anchor is skipped here and reported by the DSL tier (`DW0142`).
pub(super) fn collect_signs(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<SignPlan> {
    campaign
        .quests
        .content
        .signs
        .iter()
        .filter_map(|s| {
            let cell = point_any(anchors, s.anchor.as_str())?;
            Some(SignPlan {
                id: s.id.as_str().to_string(),
                anchor: s.anchor.as_str().to_string(),
                cell,
                front: s.front.clone(),
                back: s.back.clone(),
            })
        })
        .collect()
}
