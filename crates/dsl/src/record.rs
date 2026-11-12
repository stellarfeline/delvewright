//! **What the journal keeps of a finished task** (spec-0137): an objective's
//! `record` — what the task was — and the `record-outcome` effects that write
//! the way it ended.
//!
//! One enumeration of the outcome sites lives here ([`outcome_sites`]): every
//! `record-outcome`, in the order of the one campaign effect walk
//! ([`crate::for_each_effect_root`], each list's effects in order, a nested
//! list right after the effect that holds it), then every dialogue option's,
//! in document order. A site's **ordinal** is its 1-based position among the
//! sites naming the same objective in that order. Emission, the build's
//! ledger, the PackTest templates and the reviewer's table all ask this
//! function, so no two of them can number a site differently.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::{Campaign, DialogueEffect, EffectRootOwner, Objective, ObjectiveId, QuestEffect};

/// Where one `record-outcome` sits.
#[derive(Clone, Debug)]
pub enum OutcomeAt<'a> {
    /// In an effect bundle: the root it hangs off and the effect itself.
    Effect {
        /// The root's owner, as the one enumeration yields it.
        owner: EffectRootOwner<'a>,
        /// The root list's JSON pointer (`/content/…`).
        root: String,
        /// The effect.
        effect: &'a QuestEffect,
    },
    /// On a dialogue option.
    Option {
        /// Index of the dialogue tree.
        tree: usize,
        /// Index of the node in that tree.
        node: usize,
        /// Index of the option on that node.
        option: usize,
        /// Index of the effect in the option's list.
        index: usize,
        /// The effect.
        effect: &'a DialogueEffect,
    },
}

/// One `record-outcome`.
#[derive(Clone, Debug)]
pub struct OutcomeSite<'a> {
    /// The objective it names.
    pub objective: &'a ObjectiveId,
    /// Its line, as the document holds it.
    pub line: &'a str,
    /// The stage document it lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// Its JSON pointer within that document.
    pub path: String,
    /// Its ordinal among the sites naming the same objective, from 1.
    pub ordinal: usize,
    /// Where it sits.
    pub at: OutcomeAt<'a>,
}

impl OutcomeSite<'_> {
    /// Whether this site is the quest effect `e` (by identity).
    pub fn is_effect(&self, e: &QuestEffect) -> bool {
        matches!(&self.at, OutcomeAt::Effect { effect, .. } if std::ptr::eq(*effect, e))
    }

    /// Whether this site is the dialogue effect `e` (by identity).
    pub fn is_option_effect(&self, e: &DialogueEffect) -> bool {
        matches!(&self.at, OutcomeAt::Option { effect, .. } if std::ptr::eq(*effect, e))
    }

    /// Where the site is, as a diagnostic names it: `stage` and pointer.
    pub fn place(&self) -> String {
        format!("{}#{}", self.stage, self.path)
    }
}

/// Every `record-outcome` of the campaign, in walk order, each with its
/// ordinal (spec-0137 §4).
pub fn outcome_sites(c: &Campaign) -> Vec<OutcomeSite<'_>> {
    fn deep<'a>(
        eff: &'a QuestEffect,
        path: String,
        owner: EffectRootOwner<'a>,
        root: &str,
        stage: &'static str,
        out: &mut Vec<OutcomeSite<'a>>,
    ) {
        if let Some((objective, line)) = eff.record_outcome() {
            out.push(OutcomeSite {
                objective,
                line,
                stage,
                path: path.clone(),
                ordinal: 0,
                at: OutcomeAt::Effect {
                    owner,
                    root: root.to_string(),
                    effect: eff,
                },
            });
        }
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                deep(inner, format!("{path}/{pseg}/{j}"), owner, root, stage, out);
            }
        }
    }
    let mut out: Vec<OutcomeSite<'_>> = Vec::new();
    crate::for_each_effect_root(c, &mut |site, list| {
        for (i, eff) in list.iter().enumerate() {
            deep(
                eff,
                format!("{}/{i}", site.path),
                site.owner,
                &site.path,
                site.stage,
                &mut out,
            );
        }
    });
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (ni, node) in tree.nodes.iter().enumerate() {
            for (oi, opt) in node.options.iter().enumerate() {
                for (ei, de) in opt.effects.iter().enumerate() {
                    if let Some((objective, line)) = de.record_outcome() {
                        out.push(OutcomeSite {
                            objective,
                            line,
                            stage: "dialogue",
                            path: format!(
                                "/content/dialogues/{ti}/nodes/{ni}/options/{oi}/effects/{ei}"
                            ),
                            ordinal: 0,
                            at: OutcomeAt::Option {
                                tree: ti,
                                node: ni,
                                option: oi,
                                index: ei,
                                effect: de,
                            },
                        });
                    }
                }
            }
        }
    }
    let mut next: BTreeMap<&str, usize> = BTreeMap::new();
    for s in &mut out {
        let n = next.entry(s.objective.as_str()).or_insert(0);
        *n += 1;
        s.ordinal = *n;
    }
    out
}

/// **Is this objective recorded** (spec-0137 §3): it carries `record`, or a
/// `record-outcome` names it. `named` is the set of objective ids some site
/// names ([`named_objectives`]).
pub fn is_recorded(o: &Objective, named: &BTreeSet<String>) -> bool {
    o.record().is_some() || named.contains(o.id().as_str())
}

/// The objective ids some `record-outcome` names.
pub fn named_objectives(c: &Campaign) -> BTreeSet<String> {
    outcome_sites(c)
        .iter()
        .map(|s| s.objective.as_str().to_string())
        .collect()
}

/// The surface checks of the journal's records (spec-0137 §9): a
/// `record-outcome` names a declared objective (`DW0112`); a `record` and a
/// `line` are not empty (`DW0100`). `audience`, `in` and a cadence on a
/// `record-outcome` are refused by the envelope's own rules (`DW0942`,
/// `DW1102`), which read the verb's class.
pub(crate) fn record_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let declared: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter().map(|o| o.id().as_str()))
        .collect();
    for s in outcome_sites(c) {
        if !declared.contains(s.objective.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                s.stage,
                format!("{}/objective", s.path),
                format!(
                    "`record-outcome` names `{}`, which no quest declares as an objective. Name \
                     the objective whose way this line writes.",
                    s.objective
                ),
            ));
        }
        if s.line.trim().is_empty() {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                s.stage,
                format!("{}/line", s.path),
                "`record-outcome` has an empty `line`: write what the party did, or remove the \
                 effect."
                    .to_string(),
            ));
        }
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if o.record().is_some_and(|r| r.trim().is_empty()) {
                d.push(Diagnostic::error(
                    codes::SCHEMA,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/record"),
                    format!(
                        "objective `{}` has an empty `record`: write what the task was, or \
                         remove the field.",
                        o.id()
                    ),
                ));
            }
        }
    }
}
