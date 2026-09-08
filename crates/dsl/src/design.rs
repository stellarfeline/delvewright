//! **The design record** — the machine half of an approved look (spec-0061).
//!
//! A reference image is approved by a human looking at it, and nothing in this
//! toolchain reads a picture. What a machine can hold is the **token written
//! beside the picture at the moment of approving**: the sky it was drawn under.
//! `design.json` is that record — one row per approved image, each row naming
//! the file by its stem and stating the [`WorldTime`] and [`WorldWeather`] the
//! picture shows.
//!
//! The rows are compared with the skies the built world can actually be in
//! (`delvec validate`, `DW0890`), so a delve whose art was approved at night and
//! whose `world.json` says noon is refused before anything is placed and long
//! before a frame is rendered.
//!
//! # What this document is not
//!
//! It is not prose, and it is not the sidecar `tools/refimg.py` writes. The
//! sidecar records what was *asked for*, before any approval exists; this
//! records what came back and was *said yes to*. It is not player-visible, so
//! nothing in it is l10n-inventoried.
//!
//! # The vocabulary is the world's own
//!
//! [`Reference::time`] and [`Reference::weather`] are the same two enums
//! `world.json` declares. One authority for what an hour is: a time state added
//! to the world is an hour a row can state, with no second table to update.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ids::is_kebab;
use crate::stages::{WorldTime, WorldWeather};

/// The directory an approved image of **one scene** lives in, relative to
/// `design/`.
pub const CONCEPT_DIR: &str = "concept";

/// The directory an approved view of the **whole map** lives in, relative to
/// `design/`.
pub const REFERENCE_DIR: &str = "reference";

/// The two directories a [`Reference::name`] may name, in the order a
/// diagnostic lists them.
pub const REFERENCE_DIRS: [&str; 2] = [CONCEPT_DIR, REFERENCE_DIR];

/// **The image extensions this engine counts as an approved image**, and the
/// one place the set is written down.
///
/// It is a set of *image* extensions on purpose. `design/` also carries the
/// re-issue sidecars `tools/refimg.py` writes, and those are neither counted
/// as approved images nor refused for having no row — a file that is not an
/// image is simply not this record's subject.
pub const IMAGE_EXTENSIONS: [&str; 4] = ["jpeg", "jpg", "png", "webp"];

/// True if `ext` (lowercased, no dot) is one of [`IMAGE_EXTENSIONS`].
#[must_use]
pub fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&ext)
}

/// The design stage's payload: one row per approved reference image.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DesignContent {
    /// The approved reference images, one row each.
    ///
    /// **At least one** (`minItems: 1`): a record of nothing is not a record,
    /// and a campaign that has approved no design writes no `design.json` at
    /// all rather than an empty one. The difference matters at staging, where
    /// zero approved images is the state that is refused.
    #[schemars(length(min = 1))]
    pub references: Vec<Reference>,
}

/// One approved reference image, and the sky it was drawn under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// The image's path **stem** relative to `design/`, under `concept/` (one
    /// scene) or `reference/` (a view of the whole map) — e.g.
    /// `concept/shore-far`. The extension is deliberately absent: the name
    /// resolves against the directory, and a stem two files answer to is a
    /// candidate rather than a match, which `DW0890` refuses.
    pub name: String,
    /// One sentence saying what the picture shows. Agent-facing: it is the
    /// creator's own note to the next reader of the record, is never put on a
    /// player's screen, and is not l10n-inventoried.
    pub shows: String,
    /// The time of day the picture was drawn under — the creator's reading of
    /// the approved image, and the only creative judgement on this surface.
    pub time: WorldTime,
    /// The weather the picture was drawn under.
    pub weather: WorldWeather,
}

impl Reference {
    /// True if [`Reference::name`] is `concept/<kebab>` or `reference/<kebab>`.
    ///
    /// The same shape every id in this DSL takes, for the same reason: a name
    /// that resolves to a path needs one spelling, or the record and the
    /// directory can disagree about which file a row is about while both look
    /// right.
    #[must_use]
    pub fn is_valid_name(&self) -> bool {
        REFERENCE_DIRS.iter().any(|dir| {
            self.name
                .strip_prefix(dir)
                .and_then(|r| r.strip_prefix('/'))
                .is_some_and(is_kebab)
        })
    }

    /// The form a [`Reference::name`] has to take, for the refusal that rejected
    /// one.
    #[must_use]
    pub fn name_form() -> String {
        REFERENCE_DIRS
            .iter()
            .map(|d| format!("`{d}/<kebab>`"))
            .collect::<Vec<_>>()
            .join(" or ")
    }

    /// Which of [`REFERENCE_DIRS`] this row's name is under, when it is
    /// well-formed.
    #[must_use]
    pub fn directory(&self) -> Option<&'static str> {
        REFERENCE_DIRS
            .iter()
            .copied()
            .find(|dir| self.name.starts_with(&format!("{dir}/")))
    }
}

/// **The design record's document-level refusals** — spec-0061 §6 shape d.
///
/// These are ordinary schema and referential findings and they carry the codes
/// every other document's equivalents carry; they are deliberately **not** a
/// shape of `DW0890`, which is about the disagreement between the record and
/// the world. A reader who meets one of these is being told the document itself
/// cannot be read, not that the sky is wrong.
///
/// Empty for a campaign with no `design.json`, which is every campaign that has
/// approved no design yet.
pub fn check(c: &crate::envelope::Campaign, d: &mut Vec<crate::diagnostic::Diagnostic>) {
    use crate::diagnostic::{Diagnostic, codes};
    let Some(env) = &c.design else {
        return;
    };
    let rows = &env.content.references;
    // A record of nothing is not a record. The exported schema says `minItems:
    // 1` and serde does not enforce it, so the rule is stated here as well —
    // and it is the schema tier's refusal, because that is what it is.
    if rows.is_empty() {
        d.push(Diagnostic::error(
            codes::SCHEMA,
            crate::envelope::Stage::Design.name(),
            "/content/references",
            format!(
                "`references` is empty. The design record's schema requires at least one row \
                 (`minItems: 1`): a record of nothing is not a record, and it is not how a \
                 campaign says it has approved no design — that campaign ships no `design.json` \
                 at all. Either AUTHOR the row for an approved image, naming it and the sky it \
                 was drawn under (`delvec schema --stage {stage}`), or DELETE `design.json`.",
                stage = crate::envelope::Stage::Design.name(),
            ),
        ));
    }
    let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        if !r.is_valid_name() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                crate::envelope::Stage::Design.name(),
                format!("/content/references/{i}/name"),
                format!(
                    "reference name `{name}` is not a path this record can resolve. A name is the \
                     image's stem relative to `design/`, under one of the two directories an \
                     approved image lives in: {form}. `{concept}/` holds a picture of one scene \
                     and `{reference}/` a view of the whole map; nothing else under `design/` is \
                     an approved image. Write the name without its extension — the row resolves \
                     against the directory, so it names the picture rather than one encoding of \
                     it.",
                    name = r.name,
                    form = Reference::name_form(),
                    concept = CONCEPT_DIR,
                    reference = REFERENCE_DIR,
                ),
            ));
        }
        if let Some(first) = seen.get(r.name.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                crate::envelope::Stage::Design.name(),
                format!("/content/references/{i}/name"),
                format!(
                    "reference name `{name}` is already used by row {first}. One approved image \
                     has one row: two rows for one picture are two answers to the question this \
                     record exists to answer — which sky was that image approved under. DELETE \
                     the duplicate row, or CORRECT its `name` to the image it is really about.",
                    name = r.name,
                ),
            ));
        } else {
            seen.insert(r.name.as_str(), i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str) -> Reference {
        Reference {
            name: name.to_string(),
            shows: "a thing".into(),
            time: WorldTime::Night,
            weather: WorldWeather::Clear,
        }
    }

    #[test]
    fn a_name_is_one_of_two_directories_and_a_kebab_stem() {
        assert!(row("concept/shore-far").is_valid_name());
        assert!(row("reference/whole-map-1").is_valid_name());
        assert!(!row("shore-far").is_valid_name());
        assert!(!row("sketches/shore-far").is_valid_name());
        assert!(!row("concept/Shore_Far").is_valid_name());
        assert!(!row("concept/shore/far").is_valid_name());
        assert!(!row("concept/").is_valid_name());
    }

    #[test]
    fn the_directory_is_read_off_the_name() {
        assert_eq!(row("concept/a").directory(), Some("concept"));
        assert_eq!(row("reference/a").directory(), Some("reference"));
        assert_eq!(row("nope/a").directory(), None);
    }

    /// The extension set is one constant, and the sidecars `design/` also
    /// carries are not in it.
    #[test]
    fn only_image_extensions_are_images() {
        for ext in IMAGE_EXTENSIONS {
            assert!(is_image_extension(ext));
        }
        assert!(!is_image_extension("json"));
        assert!(!is_image_extension("md"));
        assert!(!is_image_extension("PNG"), "the caller lowercases");
    }
}
