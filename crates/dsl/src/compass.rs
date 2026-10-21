//! **A direction in player-facing text names what the player sees, never a
//! compass point** (`DW1049`).
//!
//! A player in Minecraft has no compass on screen and does not know where north
//! is, so "the last house on the north side" cannot be acted on. The rule reads
//! every player-facing line the l10n inventory holds ([`crate::l10n::inventory`],
//! the one traversal that defines the object class) and refuses a compass point
//! used as a direction; the prescription is wording relative to the player's own
//! body or to a landmark they can see ("turn around: the first door on your
//! right").
//!
//! ## What is a compass word ([`compass_word`])
//!
//! The visible text ([`crate::l10n::plain`]) is split into **words**: maximal runs
//! of alphabetic characters. A word is a compass word when, lower-cased, it is one
//! of the four cardinals or the four intercardinals written as one word
//! (`northeast`, …), bare or with one of the suffixes `ern`, `ernmost`, `erly`,
//! `ward`, `wards` ([`SUFFIXES`]). A hyphenated or spaced intercardinal
//! (`north-east`) is two words, each a cardinal. A word that merely contains a
//! cardinal (`Westmarch`, `Northmen`, `eastgate`) is a different word and is not
//! judged. An abbreviation is judged only where it is unambiguous: the two- and
//! three-letter upper-case intercardinals (`NE`, `NNE`, …; [`ABBREVIATIONS`]),
//! matched case-sensitively; a single letter (`N`, `E`) is never judged.
//!
//! ## A declared name is not a direction ([`declared_names`])
//!
//! A string whose [`TextKind`] is [`TextKind::Name`], [`TextKind::ItemName`] or
//! [`TextKind::Title`] is a **name the campaign declares** — an area, an NPC, an
//! actor, a class, a counter, an item, a health bar, a shop, the world — and is
//! never judged. Inside every other line, a compass word is exempt when it lies
//! inside a run of words that equals the words of a declared name, compared
//! ignoring case, with the name's leading article (`the`, `a`, `an`) dropped: an
//! area named "The North Gate" makes "past the north gate" a name. The rule needs
//! no judgement: whether a word is exempt follows from the document alone.
//!
//! ## English only
//!
//! The rule reads the English inventory and no sidecar. Every other language is
//! transcreated from the English line (CLAUDE.md Conventions) and held to that
//! line by its provenance (`DW0187`/`DW0188`), so the direction a line gives is
//! decided in the English; a word list per language would have to be cited from
//! a source per language, and none is held.

use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::l10n::{TextKind, inventory, key_kind, plain};

crate::dw_code! {
    /// **A compass point used as a direction in player-facing text.** A
    /// player-visible line ([`crate::l10n::inventory`], every kind but a
    /// declared name) carries a cardinal or intercardinal direction word
    /// ([`compass_word`]) that is not inside a name the campaign declares
    /// ([`declared_names`]). The player has no compass on screen and cannot act
    /// on it; the message names the line's key and the word, and prescribes
    /// wording relative to the player's body or to a landmark they can see.
    pub const COMPASS_DIRECTION: DwCode = DwCode::new("DW1049", ExitTier::Build);
}

/// The four cardinal points; an intercardinal is a northern or southern one
/// followed by an eastern or western one.
pub const CARDINALS: [&str; 4] = ["north", "south", "east", "west"];

/// The suffixes a compass point takes as a direction word, beside the bare form.
pub const SUFFIXES: [&str; 5] = ["ern", "ernmost", "erly", "ward", "wards"];

/// The upper-case abbreviations judged, case-sensitively: the two- and
/// three-letter intercardinals. A single letter is never judged.
pub const ABBREVIATIONS: [&str; 12] = [
    "NE", "NW", "SE", "SW", "NNE", "ENE", "ESE", "SSE", "SSW", "WSW", "WNW", "NNW",
];

/// The kinds of player-facing string that are names the campaign declares:
/// never judged themselves, and each one exempts its own words wherever a line
/// quotes it.
pub const NAME_KINDS: [TextKind; 3] = [TextKind::Name, TextKind::ItemName, TextKind::Title];

/// Whether `word` (one maximal run of alphabetic characters) is a compass word.
pub fn compass_word(word: &str) -> bool {
    if ABBREVIATIONS.contains(&word) {
        return true;
    }
    let w = word.to_lowercase();
    let points = CARDINALS.iter().map(|c| c.to_string()).chain(
        ["north", "south"]
            .iter()
            .flat_map(|a| ["east", "west"].iter().map(move |b| format!("{a}{b}"))),
    );
    for p in points {
        if let Some(rest) = w.strip_prefix(p.as_str())
            && (rest.is_empty() || SUFFIXES.contains(&rest))
        {
            return true;
        }
    }
    false
}

/// The words of `text`: maximal runs of alphabetic characters, in order.
pub fn words(text: &str) -> Vec<&str> {
    text.split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty())
        .collect()
}

/// The words of every name the campaign declares (a [`NAME_KINDS`] string),
/// lower-cased, the leading article dropped, deduplicated and in a fixed order.
pub fn declared_names(inv: &BTreeMap<String, String>) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = inv
        .iter()
        .filter(|(k, _)| is_name(k))
        .map(|(_, v)| {
            let mut w: Vec<String> = words(&plain(v)).iter().map(|w| w.to_lowercase()).collect();
            if w.len() > 1 && ["the", "a", "an"].contains(&w[0].as_str()) {
                w.remove(0);
            }
            w
        })
        .filter(|w| !w.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn is_name(key: &str) -> bool {
    key_kind(key).is_some_and(|k| NAME_KINDS.contains(&k))
}

/// The compass words of `text` that no declared name covers, in order.
pub fn directions(text: &str, names: &[Vec<String>]) -> Vec<String> {
    let ws = words(text);
    let lower: Vec<String> = ws.iter().map(|w| w.to_lowercase()).collect();
    let mut covered = vec![false; ws.len()];
    for name in names {
        if name.len() > lower.len() {
            continue;
        }
        for start in 0..=lower.len() - name.len() {
            if lower[start..start + name.len()] == name[..] {
                covered[start..start + name.len()].fill(true);
            }
        }
    }
    ws.iter()
        .zip(covered)
        .filter(|(w, c)| !c && compass_word(w))
        .map(|(w, _)| (*w).to_string())
        .collect()
}

/// `DW1049` over every judged line of `c`'s English inventory.
pub fn validate_compass_directions(c: &Campaign) -> Vec<Diagnostic> {
    let inv = inventory(c);
    let names = declared_names(&inv);
    let mut d = Vec::new();
    for (key, text) in &inv {
        if is_name(key) {
            continue;
        }
        let found = directions(&plain(text), &names);
        if found.is_empty() {
            continue;
        }
        let kind = key_kind(key)
            .and_then(|k| serde_json::to_value(k).ok())
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "line".to_string());
        d.push(Diagnostic::error(
            COMPASS_DIRECTION,
            "l10n",
            format!("#/{key}"),
            format!(
                "player-visible {kind} `{key}` gives a compass direction: {}. A player has no \
                 compass on screen and cannot tell where north is. Say where it is from the \
                 player's own body or from a landmark they can see (\"turn around: the first \
                 door on your right\", \"past the well, the door with the lantern\"). A word \
                 inside a name the campaign declares (an area, NPC, actor, class, item or title) \
                 is a name, not a direction. Text: `{text}`",
                found
                    .iter()
                    .map(|w| format!("`{w}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }
    d
}

/// What `DW1049` examined over one campaign.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CompassBinding {
    /// Player-facing lines in the English inventory.
    pub lines: usize,
    /// Of those, the lines judged (every kind but a declared name).
    pub judged: usize,
    /// Declared names whose words exempt a compass word they cover.
    pub names: usize,
    /// Compass words a declared name covered, across the judged lines.
    pub exempt: usize,
    /// Compass words refused.
    pub refused: usize,
}

impl CompassBinding {
    /// Count what the rule binds over `c`.
    pub fn of(c: &Campaign) -> Self {
        let inv = inventory(c);
        let names = declared_names(&inv);
        let mut b = CompassBinding {
            lines: inv.len(),
            names: names.len(),
            ..Default::default()
        };
        for (key, text) in &inv {
            if is_name(key) {
                continue;
            }
            b.judged += 1;
            let text = plain(text);
            let all = words(&text).into_iter().filter(|w| compass_word(w)).count();
            let refused = directions(&text, &names).len();
            b.refused += refused;
            b.exempt += all - refused;
        }
        b
    }

    /// The one-line binding statement `delvec validate` prints.
    pub fn line(&self) -> String {
        format!(
            "compass binding (DW1049): {} of {} player-facing line(s) judged; {} declared \
             name(s); {} compass word(s) inside a declared name, {} refused",
            self.judged, self.lines, self.names, self.exempt, self.refused
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_list() {
        for w in [
            "north",
            "North",
            "SOUTH",
            "eastern",
            "westward",
            "westwards",
            "northerly",
            "southernmost",
            "northeast",
            "Southwestern",
            "NE",
            "SSW",
        ] {
            assert!(compass_word(w), "`{w}` is a compass word");
        }
        for w in [
            "Westmarch",
            "Northmen",
            "eastgate",
            "N",
            "E",
            "ne",
            "Ne",
            "easter",
            "northing",
            "westerns",
        ] {
            assert!(!compass_word(w), "`{w}` is not a compass word");
        }
    }

    #[test]
    fn a_declared_name_covers_its_words() {
        let names = vec![
            vec!["north".to_string(), "gate".to_string()],
            vec!["westmarch".to_string()],
        ];
        assert!(directions("Wait by the North Gate.", &names).is_empty());
        assert!(directions("wait by the north gate", &names).is_empty());
        assert_eq!(
            directions("The north wall, past the North Gate.", &names),
            vec!["north".to_string()]
        );
        assert_eq!(
            directions("Go north-east.", &names),
            vec!["north".to_string(), "east".to_string()]
        );
        assert!(directions("Turn around: the first door on your right.", &names).is_empty());
    }
}
