//! The act lexicon (spec-0104 §5): per language and per objective kind, the
//! verbs that name the act vanilla completes the kind on.
//!
//! The data is `data/act-verbs.json`; this module is its one reader. A string
//! is read for **membership in a closed set**, never for meaning: its visible
//! text ([`crate::l10n::plain`]) is scanned left to right and, at each
//! position, the longest entry of any kind that matches there is the verb read
//! — so `pick up` is read as one collect verb and never also as anything inside
//! it, and `找到` is one reach verb and never also `找`. A `words` language
//! matches an entry only on whole-word boundaries, case-insensitively; a
//! `substring` language matches it anywhere.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

const DATA: &str = include_str!("../data/act-verbs.json");

/// The five objective kinds, in the order the lexicon lists them.
pub const KINDS: [&str; 5] = ["talk-to", "interact", "reach-anchor", "kill", "collect"];

#[derive(Deserialize)]
struct Doc {
    languages: BTreeMap<String, LangDoc>,
}

#[derive(Deserialize)]
struct LangDoc {
    #[serde(rename = "match")]
    how: String,
    kinds: BTreeMap<String, Vec<String>>,
}

/// How a language's entries are matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Match {
    /// A whole word or phrase, case-insensitively.
    Words,
    /// A substring.
    Substring,
}

/// One language's table.
#[derive(Clone, Debug)]
pub struct Table {
    /// How its entries are matched.
    pub how: Match,
    /// Kind → its verbs, in file order.
    pub kinds: BTreeMap<String, Vec<String>>,
}

/// Every language's table, parsed once.
pub fn tables() -> &'static BTreeMap<String, Table> {
    static T: OnceLock<BTreeMap<String, Table>> = OnceLock::new();
    T.get_or_init(|| {
        let doc: Doc = serde_json::from_str(DATA).expect("data/act-verbs.json parses");
        doc.languages
            .into_iter()
            .map(|(lang, d)| {
                let how = match d.how.as_str() {
                    "words" => Match::Words,
                    "substring" => Match::Substring,
                    other => panic!("data/act-verbs.json: unknown match `{other}`"),
                };
                (
                    lang,
                    Table {
                        how,
                        kinds: d.kinds,
                    },
                )
            })
            .collect()
    })
}

/// The table for `lang` (`en` for the English document), when one exists.
pub fn table(lang: &str) -> Option<&'static Table> {
    tables().get(lang)
}

/// One verb read in a string: the entry and the kind it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// The kind the verb names the act of.
    pub kind: String,
    /// The entry as the lexicon spells it.
    pub verb: String,
}

impl Table {
    /// Every verb `text` contains, in order, longest entry first at each
    /// position.
    pub fn verbs_in(&self, text: &str) -> Vec<Found> {
        let entries: Vec<(&str, &str)> = self
            .kinds
            .iter()
            .flat_map(|(k, vs)| vs.iter().map(move |v| (k.as_str(), v.as_str())))
            .collect();
        match self.how {
            Match::Substring => {
                let chars: Vec<char> = text.chars().collect();
                let mut out = Vec::new();
                let mut i = 0;
                while i < chars.len() {
                    let rest: String = chars[i..].iter().collect();
                    let best = entries
                        .iter()
                        .filter(|(_, v)| rest.starts_with(*v))
                        .max_by_key(|(_, v)| v.chars().count());
                    match best {
                        Some((k, v)) => {
                            out.push(Found {
                                kind: (*k).to_string(),
                                verb: (*v).to_string(),
                            });
                            i += v.chars().count();
                        }
                        None => i += 1,
                    }
                }
                out
            }
            Match::Words => {
                let lower = text.to_lowercase();
                let words: Vec<&str> = lower
                    .split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
                    .filter(|w| !w.is_empty())
                    .collect();
                let mut out = Vec::new();
                let mut i = 0;
                while i < words.len() {
                    let best = entries
                        .iter()
                        .filter_map(|(k, v)| {
                            let phrase: Vec<&str> = v.split_whitespace().collect();
                            let n = phrase.len();
                            (i + n <= words.len() && words[i..i + n] == phrase[..])
                                .then_some((*k, *v, n))
                        })
                        .max_by_key(|(_, _, n)| *n);
                    match best {
                        Some((k, v, n)) => {
                            out.push(Found {
                                kind: k.to_string(),
                                verb: v.to_string(),
                            });
                            i += n;
                        }
                        None => i += 1,
                    }
                }
                out
            }
        }
    }

    /// The verbs of `kind`, joined for a message.
    pub fn listed(&self, kind: &str) -> String {
        self.kinds
            .get(kind)
            .map(|v| v.join(", "))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One verb belongs to one kind in a language: every language's five sets
    /// are pairwise disjoint, and every language lists all five kinds.
    #[test]
    fn every_language_holds_its_five_sets_disjoint() {
        for (lang, t) in tables() {
            let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
            for k in KINDS {
                let verbs = t.kinds.get(k).unwrap_or_else(|| panic!("{lang}: {k}"));
                assert!(!verbs.is_empty(), "{lang}: {k} is empty");
                for v in verbs {
                    if let Some(other) = seen.insert(v.as_str(), k) {
                        panic!("{lang}: `{v}` is in both `{other}` and `{k}`");
                    }
                }
            }
            assert_eq!(t.kinds.len(), KINDS.len(), "{lang}: an unknown kind");
        }
    }

    /// Each row of spec-0104 §5's seed is in the file.
    #[test]
    fn the_seed_is_in_the_file() {
        let en = table("en").unwrap();
        for (k, v) in [
            ("talk-to", "report"),
            ("interact", "knock"),
            ("reach-anchor", "get to"),
            ("kill", "cut down"),
            ("collect", "steal"),
        ] {
            assert!(en.kinds[k].iter().any(|x| x == v), "{k}: {v}");
        }
        let zh = table("zh-cn").unwrap();
        for (k, v) in [
            ("talk-to", "找"),
            ("interact", "读"),
            ("reach-anchor", "抵达"),
            ("kill", "除掉"),
            ("collect", "夺"),
        ] {
            assert!(zh.kinds[k].iter().any(|x| x == v), "{k}: {v}");
        }
    }

    #[test]
    fn the_longest_entry_at_a_position_is_the_one_read() {
        let en = table("en").unwrap();
        let f = en.verbs_in("Pick up the key and get to the mill");
        assert_eq!(
            f.iter().map(|x| x.verb.as_str()).collect::<Vec<_>>(),
            ["pick up", "get to"]
        );
        assert!(en.verbs_in("Settle down").is_empty(), "whole words only");
        assert_eq!(en.verbs_in("TALK to Mara")[0].kind, "talk-to");
        let zh = table("zh-cn").unwrap();
        let f = zh.verbs_in("找到磨坊");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, "reach-anchor");
        assert_eq!(zh.verbs_in("找磨坊主")[0].kind, "talk-to");
    }
}
