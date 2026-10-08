//! The cast ledger: where each body is, and what it says, at each point of the story.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{AnchorId, DialogueId, FlagId, Mark, StateCompare};

#[cfg(doc)]
use crate::Quest;

/// One NPC's entry in a quest's [`cast`](Quest::cast) ledger.
///
/// Three authored shapes, tried in order (untagged): the bare keyword `"dead"` /
/// `"offstage"`, a single flat [`CastPlacement`], or a **list** of placements —
/// per-branch casts, each gated by the flags that select its branch (spec-0020
/// proof 4: where the effect history is branch-dependent, a single flat
/// declaration cannot hold on every reachable branch, and `DW0462` says so).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastEntry {
    /// The bare keyword form: `"npc/antiphos": "dead"`. Shorthand for a
    /// placement whose `at` is that keyword and which carries no `doing` or
    /// `dialogue` — a character who is not in the world has no business and
    /// answers no right-click.
    Absent(CastAbsence),
    /// A single placement that must hold on every reachable branch.
    Placement(CastPlacement),
    /// Per-branch placements (spec-0020 proof 4). Each carries the
    /// `requires_flags`/`forbids_flags` that select its branch.
    Branches(Vec<CastPlacement>),
}

impl CastEntry {
    /// Every placement this entry declares, in declared order. The bare-keyword
    /// form yields none — there is no scene to check.
    pub fn placements(&self) -> Vec<&CastPlacement> {
        match self {
            CastEntry::Absent(_) => Vec::new(),
            CastEntry::Placement(p) => vec![p],
            CastEntry::Branches(ps) => ps.iter().collect(),
        }
    }

    /// Every placement this entry declares, mutably (the l10n traversal).
    pub fn placements_mut(&mut self) -> Vec<&mut CastPlacement> {
        match self {
            CastEntry::Absent(_) => Vec::new(),
            CastEntry::Placement(p) => vec![p],
            CastEntry::Branches(ps) => ps.iter_mut().collect(),
        }
    }

    /// The bare-keyword absence this entry declares, if it is that form.
    pub fn absence(&self) -> Option<CastAbsence> {
        match self {
            CastEntry::Absent(a) => Some(*a),
            _ => None,
        }
    }

    /// True if this entry declares the NPC out of the world entirely — the bare
    /// keyword, or every placement's `at` being a keyword.
    pub fn is_absent(&self) -> bool {
        match self {
            CastEntry::Absent(_) => true,
            CastEntry::Placement(p) => p.at.absence().is_some(),
            CastEntry::Branches(ps) => {
                !ps.is_empty() && ps.iter().all(|p| p.at.absence().is_some())
            }
        }
    }
}

/// A declared absence: the NPC is deliberately not in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CastAbsence {
    /// Removed from the story for good (must match a `despawn-npc` with no later
    /// `spawn-npc`).
    Dead,
    /// Not in the world for this quest, but may return (must match a
    /// `despawn-npc`).
    Offstage,
}

impl CastAbsence {
    /// The authored keyword (`dead` / `offstage`).
    pub fn token(self) -> &'static str {
        match self {
            CastAbsence::Dead => "dead",
            CastAbsence::Offstage => "offstage",
        }
    }
}

/// Where a cast entry puts an NPC: a prefab anchor, a mark, or a declared
/// absence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastPlace {
    /// `"dead"` or `"offstage"` — explicitly not in the world.
    Absent(CastAbsence),
    /// The anchor the NPC stands on for this quest's duration. Must equal the
    /// position the effect history actually produces (`DW0461`): declaring an
    /// anchor does not teleport anybody.
    Anchor(AnchorId),
    /// The mark the NPC stands on for this quest's duration (spec-0066): the
    /// spelling for a body that stands at an offset from its anchor. `DW0461`
    /// compares anchor and offset both.
    Mark(Mark),
}

impl CastPlace {
    /// The anchor this place names, if it names one.
    pub fn anchor(&self) -> Option<&AnchorId> {
        match self {
            CastPlace::Anchor(a) => Some(a),
            CastPlace::Mark(m) => Some(&m.anchor),
            CastPlace::Absent(_) => None,
        }
    }

    /// The mark this place names, if it names one: a bare anchor is the mark at
    /// a zero offset.
    pub fn mark(&self) -> Option<Mark> {
        match self {
            CastPlace::Anchor(a) => Some(Mark::at(a.clone())),
            CastPlace::Mark(m) => Some(m.clone()),
            CastPlace::Absent(_) => None,
        }
    }

    /// The declared absence, if this place is one.
    pub fn absence(&self) -> Option<CastAbsence> {
        match self {
            CastPlace::Absent(a) => Some(*a),
            CastPlace::Anchor(_) | CastPlace::Mark(_) => None,
        }
    }

    /// The authored token, for diagnostics.
    pub fn token(&self) -> String {
        match self {
            CastPlace::Absent(a) => a.token().to_string(),
            CastPlace::Anchor(a) => a.as_str().to_string(),
            CastPlace::Mark(m) => m.display(),
        }
    }
}

/// One declared scene for one NPC in one quest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CastPlacement {
    /// Where the NPC is: an anchor, or `"offstage"` / `"dead"`.
    pub at: CastPlace,
    /// What the character is *doing* — free prose, never machine-checked.
    ///
    /// This field is the forcing function, which is the whole reason it is
    /// required (`DW0463`) despite being unverifiable: you cannot fill it in
    /// without deciding the character's business in this beat, and stage 6
    /// receives it as the context the NPC's lines are written against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doing: Option<String>,
    /// What right-click offers during this quest. Required for an on-stage
    /// placement (`DW0463`) — including the explicit `"none"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue: Option<CastDialogue>,
    /// Branch gate (per-branch casts): this placement describes the world only
    /// once every listed flag is set. Mirrors an option's `requires_flags`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Negative branch gate: this placement describes the world only while no
    /// listed flag is set. Mirrors an option's `forbids_flags`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms (DSL v0.10, spec-0031): every listed comparison must
    /// hold for this gate to be open. The third field of the one gate, carried by
    /// every gate consumer — never by the verb that first wanted it. Default
    /// empty, so a pre-0.10 campaign is byte-identical.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
}

/// What an NPC's right-click offers for a quest's duration.
///
/// Untagged, tried in order: the keywords `"none"` / `"unchanged"`, a
/// `{"barks": […]}` pool, then a dialogue root id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CastDialogue {
    /// A keyword: `"none"` or `"unchanged"`.
    Keyword(CastDialogueKeyword),
    /// A bark pool: right-click yields one inconsequential in-character line —
    /// no tree, no options, no consequences.
    Barks(CastBarks),
    /// A dialogue root id: right-click opens this node of the NPC's stage-6
    /// tree. Must be a node of *that* NPC's tree (`DW0464`).
    Root(DialogueId),
}

/// The keyword forms of [`CastDialogue`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CastDialogueKeyword {
    /// Genuinely no reaction: the right-click is recorded and consumed, and
    /// nothing opens. Legal, but a last resort — if a body is clickable, the
    /// world should answer; prefer a bark.
    None,
    /// Carry forward whatever this NPC's dialogue was at its **previous**
    /// appearance in the quest-DAG ordering.
    ///
    /// The point is that carrying dialogue forward is a *conscious, declared
    /// act*. It is never an implicit default — an omitted `dialogue` is
    /// `DW0463`, not a carry-forward — and writing `"unchanged"` states the
    /// intent without re-spelling a root id that then drifts out of sync. It
    /// resolves transitively (`unchanged` → `unchanged` → a root), and using it
    /// at an NPC's first appearance is `DW0466`: there is nothing to carry.
    ///
    /// Emission is a **no-op** — no root swap is emitted for that NPC at that
    /// quest — which is what makes the sugar cheap and byte-stable.
    Unchanged,
}

/// A bark pool: inconsequential in-character lines, cycled deterministically.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CastBarks {
    /// The lines, in cycle order. At least one (`DW0464`). Player-visible, so
    /// they enter the l10n inventory like any narrate text.
    pub barks: Vec<String>,
}
