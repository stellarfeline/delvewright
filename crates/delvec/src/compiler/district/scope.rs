//! **`delvec validate --district <x>`** (spec-0103 §7.5): whose tree this is.
//!
//! A worker's tree holds the whole campaign, because every proof reads the
//! whole; it also holds every sibling's planned quests unexpanded, which
//! `DW0150` would name as the planned-but-unexpanded state it is. Under
//! `--district <x>`, planned quests whose `district` is neither `x` nor absent
//! are counted into one informational line, `DW1006`, and the refusals that
//! stand only on their absence are reported beneath it as advisories rather
//! than as the worker's. Everything else is judged as it always is.
//!
//! **The set, by rule** — the codes `references/story.md` enumerates as the
//! unwritten mission's, each on the arm that stands only on the absence:
//!
//! * `DW0150` when every quest it names is a sibling's;
//! * `DW0818` when it names a sibling's quest;
//! * `DW0172`, `DW0482`, `DW0197` — a flag, an ending, a summons stage 5 has
//!   not written yet — while any sibling's quest is unexpanded;
//! * `DW0112` on a branch's `leads_to`, and on a reference naming a sibling's
//!   unexpanded quest (the whole's spine waiting on a district's quest — the
//!   same absence, on the generic code's path);
//! * `DW0816` and `DW0817` on the arm that carries the unwritten-mission
//!   caveat (their message points at `DW0150`).
//!
//! Without the flag, `validate` judges the whole, which is the planner's run
//! after a wave is integrated.

use std::collections::BTreeSet;

use delvewright_dsl::{Campaign, Diagnostic, Severity};

use super::{DW_NO_SUCH_DISTRICT, DW_SIBLINGS_UNEXPANDED, Partition};

/// The planned quests of districts other than `district` that stage 5 does
/// not expand in this tree, and how many districts they belong to.
#[must_use]
pub fn siblings_unexpanded(c: &Campaign, district: &str) -> (BTreeSet<String>, usize) {
    let expanded: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let mut quests = BTreeSet::new();
    let mut districts = BTreeSet::new();
    for q in &c.quest_plan.content.quests {
        let Some(d) = q.district.as_ref().map(|d| d.as_str()) else {
            continue;
        };
        if d != district && !expanded.contains(q.id.as_str()) {
            quests.insert(q.id.as_str().to_string());
            districts.insert(d.to_string());
        }
    }
    let n = districts.len();
    (quests, n)
}

/// `DW1009` when `district` is not a district the graph declares.
#[must_use]
pub fn unknown(c: &Campaign, district: &str) -> Option<Diagnostic> {
    let p = Partition::of(c);
    if p.ids.iter().any(|d| d == district) {
        return None;
    }
    Some(Diagnostic::error(
        DW_NO_SUCH_DISTRICT,
        "layout-graph",
        "/content/districts",
        format!(
            "`--district {district}` names no district this campaign declares (it declares {}). \
             Name a district of `layout-graph.json` `districts[]` — the packet's `tools` line \
             carries the right one — or drop the flag to judge the whole.",
            super::ticked(&p.ids)
        ),
    ))
}

fn names_any(d: &Diagnostic, ids: &BTreeSet<String>) -> bool {
    ids.iter().any(|q| {
        d.message.contains(&format!("`{q}`"))
            || d.message.contains(q.as_str())
            || d.path.contains(q.as_str())
    })
}

/// Every planned quest id a diagnostic names.
fn planned_named<'a>(c: &'a Campaign, d: &Diagnostic) -> Vec<&'a str> {
    c.quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .filter(|q| d.message.contains(q))
        .collect()
}

/// Whether `d` stands only on the absence of the siblings' quests.
fn on_absence(c: &Campaign, d: &Diagnostic, siblings: &BTreeSet<String>) -> bool {
    if siblings.is_empty() || d.severity != Severity::Error {
        return false;
    }
    match d.code.as_str() {
        "DW0150" => {
            let named = planned_named(c, d);
            !named.is_empty() && named.iter().all(|q| siblings.contains(*q))
        }
        "DW0818" => names_any(d, siblings),
        "DW0172" | "DW0482" | "DW0197" => true,
        "DW0112" => d.path.contains("leads_to") || names_any(d, siblings),
        "DW0816" | "DW0817" => d.message.contains("DW0150"),
        _ => false,
    }
}

/// **Apply the scope**: demote the refusals that stand only on the siblings'
/// absence to advisories, and add the one `DW1006` line that counts them.
pub fn apply(c: &Campaign, district: &str, diags: &mut Vec<Diagnostic>) {
    let (siblings, n_districts) = siblings_unexpanded(c, district);
    let mut demoted: Vec<String> = Vec::new();
    for d in diags.iter_mut() {
        if on_absence(c, d, &siblings) {
            d.severity = Severity::Warning;
            d.message = format!(
                "(reported under DW1006 — this stands on the absence of other districts' \
                 quests in this tree, and the whole's validate after integration judges it) {}",
                d.message
            );
            demoted.push(d.code.clone());
        }
    }
    let mut by_code: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for c in &demoted {
        *by_code.entry(c.as_str()).or_default() += 1;
    }
    diags.push(Diagnostic::warning(
        DW_SIBLINGS_UNEXPANDED,
        "quest-plan",
        "/content/quests",
        format!(
            "{n} planned quest(s) of {n_districts} other district(s) are not expanded in this tree \
             ({list}); validated as `{district}`'s tree, {k} diagnostic(s) standing only on that \
             absence are reported as advisories{codes}. They are the siblings' to write; the \
             whole's `delvec validate` after the wave is integrated judges them.",
            n = siblings.len(),
            list = super::ticked(&siblings.iter().cloned().collect::<Vec<_>>()),
            k = demoted.len(),
            codes = if by_code.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    by_code
                        .iter()
                        .map(|(c, n)| format!("{c} ×{n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        ),
    ));
}
