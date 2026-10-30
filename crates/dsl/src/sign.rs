//! Sign text: what a sign the piece placed says.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, SignId};

/// The lines one side of a sign holds in the pinned game: `SignText.LINES`,
/// read from the 1.21.11 client jar (`docs/reference/compiler.md`, `signs[]`).
pub const SIGN_LINES: usize = 4;

/// A stage-5 sign text (spec-0128 §8): the words a sign the **prefab already
/// placed** shows — a standing, wall, hanging or wall-hanging sign of any wood.
///
/// The sign is hardware the piece authored, exactly as a container is for a
/// `loot` fill: this declaration gives an already-placed sign its text. The
/// compiler never places the sign; if the anchor's cell does not hold one, or
/// a line is wider than that sign draws, the build refuses (`DW1075`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sign {
    /// Unique sign id (`sign/<kebab>`).
    pub id: SignId,
    /// The anchor whose cell holds the sign.
    pub anchor: AnchorId,
    /// The lines on the sign's front, top to bottom: one to four. An empty
    /// string is a blank line. Player-facing: each non-empty line is
    /// inventoried for translation as `sign.<id>.front.<i>`.
    pub front: Vec<String>,
    /// The lines on the sign's back, top to bottom: up to four. Absent, the
    /// back stays as the piece left it (blank). Inventoried as
    /// `sign.<id>.back.<i>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub back: Vec<String>,
}

impl Sign {
    /// Both sides with their key token, front first.
    pub fn sides(&self) -> [(&'static str, &[String]); 2] {
        [("front", &self.front), ("back", &self.back)]
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

/// Stage-5 `signs` declarations: id syntax and uniqueness (`DW0110`/`DW0111`),
/// anchor resolution (`DW0142`), and the line count a side holds — one to four
/// on the front, at most four on the back — restated at the schema tier
/// (`DW0100`), the schema stating the bound and serde enforcing none.
///
/// Whether the anchor's cell holds a sign, and whether each line fits the sign
/// standing there, need the assembled world: they are the build-tier proof
/// (`DW1075`).
pub(crate) fn sign_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let signs = &c.quests.content.signs;
    if signs.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, s) in signs.iter().enumerate() {
        let path = format!("/content/signs/{i}");
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("{path}/id"),
                format!(
                    "malformed sign id `{}` — sign ids are lowercase kebab-case with the `sign/` \
                     prefix (e.g. `sign/bakery`)",
                    s.id
                ),
            ));
        }
        if !seen.insert(s.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("{path}/id"),
                format!("duplicate sign id `{}`", s.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            s.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a sign",
            "quests",
            format!("{path}/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(s.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("{path}/anchor"),
                format!(
                    "sign anchor `{}` is not provided by any prefab bound in this campaign — {}",
                    s.anchor,
                    providers.anchor_remedy(
                        "use the anchor the piece declares at its sign's cell (anchor names come \
                         from prefab metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        if s.front.is_empty() {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                "quests",
                format!("{path}/front"),
                format!(
                    "sign `{}` declares no front line: a sign text says something on its front — \
                     give it one to {SIGN_LINES} lines, or delete the entry and leave the sign blank",
                    s.id
                ),
            ));
        }
        for (side, lines) in s.sides() {
            if lines.len() > SIGN_LINES {
                d.push(Diagnostic::error(
                    codes::SCHEMA,
                    "quests",
                    format!("{path}/{side}"),
                    format!(
                        "sign `{}` declares {} lines on its {side}; a side of a sign holds \
                         {SIGN_LINES} in Minecraft 1.21.11 and the rest would never show — cut \
                         the text to {SIGN_LINES} lines",
                        s.id,
                        lines.len()
                    ),
                ));
            }
        }
    }
}
