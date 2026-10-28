//! **A step that says it opens something opens it** (spec-0124 §6.2,
//! `DW1070`).
//!
//! A `happening` is a chronicle statement: what a node does to the story. Two
//! of its verbs are about the world — `opens` and `seals` — and the world has
//! the verbs that do it. A `happening` that says *opens* is held to the effects
//! of the node that carries it: something those effects do opens a gate or a
//! way, and the `subject`, when it names an anchor, is the gate opened. `seals`
//! likewise, to `close-gate`.
//!
//! The node's own effects are: an objective's completion effects (its quest's
//! `on_objective_complete` bundle, nested lists included); a dialogue option's
//! effects, which open nothing the world has; and a story-node effect itself,
//! with the lists nested in it. Read off the documents alone, so it is refused
//! at validation, where the statement is written.

use delvewright_dsl::{
    Campaign, Diagnostic, DwCode, ExitTier, Happening, HappeningVerb, QuestEffect, Verb,
};

delvewright_dsl::dw_code! {
    /// `DW1070`: a `happening` says the node opens (or seals) a thing, and the
    /// node's own effects open (or seal) nothing it names (spec-0124 §6.2).
    pub const DW_OPENS_NOTHING: DwCode = DwCode::new("DW1070", ExitTier::Build);
}

/// What `DW1070` read, zeroes included.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HappeningBinding {
    /// Happenings on objectives, dialogue options and story-node effects.
    pub happenings: usize,
    /// Of those, the ones whose verb is `opens` or `seals` — the judged ones.
    pub judged: usize,
    /// Judged happenings refused.
    pub refused: usize,
}

impl HappeningBinding {
    /// The binding line `delvec validate` prints.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "happening binding ({}): {} opens/seals happening(s) among {}; {} refused",
            DW_OPENS_NOTHING.id(),
            self.judged,
            self.happenings,
            self.refused
        )
    }
}

/// What one effect list does to the world's ways, deep: every gate anchor an
/// `open-gate` names, every way an `open-way` names, every gate a
/// `close-gate` names.
#[derive(Default)]
struct Doing {
    opened_gates: Vec<String>,
    opened_ways: Vec<String>,
    closed_gates: Vec<String>,
}

impl Doing {
    fn of(effects: &[&QuestEffect]) -> Doing {
        let mut d = Doing::default();
        let mut stack: Vec<&QuestEffect> = effects.to_vec();
        while let Some(e) = stack.pop() {
            match &e.verb {
                Verb::OpenGate { anchor } => d.opened_gates.push(anchor.as_str().to_string()),
                Verb::CloseGate { anchor } => d.closed_gates.push(anchor.as_str().to_string()),
                Verb::OpenWay { piece, way } => {
                    d.opened_ways
                        .push(format!("`{}` way `{way}`", piece.as_str()));
                }
                _ => {}
            }
            for list in e.nested_effect_lists() {
                stack.extend(list.iter());
            }
        }
        d
    }

    fn words(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        parts.extend(
            self.opened_gates
                .iter()
                .map(|a| format!("`open-gate` `{a}`")),
        );
        parts.extend(self.opened_ways.iter().map(|w| format!("`open-way` {w}")));
        parts.extend(
            self.closed_gates
                .iter()
                .map(|a| format!("`close-gate` `{a}`")),
        );
        if parts.is_empty() {
            "no opening or sealing effect".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// The refusal for one judged happening, or `None` when its node does what it
/// says.
fn judge(node: &str, h: &Happening, doing: &Doing) -> Option<String> {
    let anchor_subject = h.subject.as_deref().filter(|s| s.starts_with("anchor/"));
    let (verb, done, gates): (&str, bool, &[String]) = match h.verb {
        HappeningVerb::Opens => (
            "opens",
            !doing.opened_gates.is_empty() || !doing.opened_ways.is_empty(),
            &doing.opened_gates,
        ),
        HappeningVerb::Seals => ("seals", !doing.closed_gates.is_empty(), &doing.closed_gates),
        _ => return None,
    };
    let names = anchor_subject.is_none_or(|s| gates.iter().any(|g| g == s));
    if done && names {
        return None;
    }
    let what = if done {
        format!(
            "its subject `{}` is not the thing it {verb}",
            anchor_subject.unwrap_or_default()
        )
    } else {
        format!("it {verb} nothing")
    };
    let opener = if verb == "opens" {
        "`open-gate` (or an `open-way`)"
    } else {
        "`close-gate`"
    };
    Some(format!(
        "{node} carries a `happening` that says it {verb}{}, and {what}: the node carries {}. A \
         step that says it {verb} a thing does it (spec-0124 §6.2). Either add the {opener} on \
         the thing it names to the node's own effects, or say what the node does with another \
         verb",
        anchor_subject
            .map(|s| format!(" `{s}`"))
            .unwrap_or_default(),
        doing.words(),
    ))
}

/// `DW1070` over every objective, dialogue option and story-node effect.
#[must_use]
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, HappeningBinding) {
    let mut bind = HappeningBinding::default();
    let mut d = Vec::new();
    let mut verdict = |bind: &mut HappeningBinding,
                       stage: &str,
                       path: String,
                       node: String,
                       h: &Happening,
                       doing: &Doing| {
        bind.happenings += 1;
        if !matches!(h.verb, HappeningVerb::Opens | HappeningVerb::Seals) {
            return;
        }
        bind.judged += 1;
        if let Some(msg) = judge(&node, h, doing) {
            bind.refused += 1;
            d.push(Diagnostic::error(DW_OPENS_NOTHING, stage, path, msg));
        }
    };
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Some(h) = o.happening() else { continue };
            let effects: Vec<&QuestEffect> = q
                .on_objective_complete
                .get(o.id())
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            verdict(
                &mut bind,
                "quests",
                format!("/content/quests/{qi}/objectives/{oi}/happening"),
                format!("objective `{}`", o.id()),
                h,
                &Doing::of(&effects),
            );
        }
    }
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (ni, n) in tree.nodes.iter().enumerate() {
            for (pi, opt) in n.options.iter().enumerate() {
                let Some(h) = opt.happening.as_ref() else {
                    continue;
                };
                verdict(
                    &mut bind,
                    "dialogue",
                    format!("/content/dialogues/{ti}/nodes/{ni}/options/{pi}/happening"),
                    format!("`{}`'s option \"{}\"", tree.npc.as_str(), opt.label),
                    h,
                    &Doing::default(),
                );
            }
        }
    }
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some(h) = eff.happening.as_ref() else {
            return;
        };
        verdict(
            &mut bind,
            "quests",
            format!("{path}/happening"),
            format!("the effect at `{path}`"),
            h,
            &Doing::of(&[eff]),
        );
    });
    (d, bind)
}
