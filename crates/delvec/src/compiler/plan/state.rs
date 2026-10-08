//! State: the terms a gate or a state comparison compiles to.

use super::*;

/// **One term of a gate, as the scoreboard question the server actually asks.**
///
/// A gate ([`delvewright_dsl::gate::Gate`]) is three authored lists — required
/// flags, forbidden flags, numeric comparisons — and all three reduce to one
/// runtime question: *does this holder's score in this objective fall in this
/// range, or not?* A flag is `dw.f_<id>` on [`PARTY`] matched against `1`; a datum
/// is `dw.s_<id>` on its declared scope matched against the range its
/// `CompareOp` names.
///
/// The reduction lives here, once, because it has two consumers that must not be
/// allowed to disagree: the emitter, which renders each term as an `execute`
/// sub-clause, and `validation/death-plan.json`, which hands the same terms to the
/// bot tier so a conditional promise can be read as the conditional it is. A bot
/// handed an `on_death` bundle with its gates stripped asserts a forfeit the
/// campaign never promised under the state in force.
///
/// The range is a closed interval with open ends: `min` and `max` both set and
/// equal is `matches <v>`, an open `max` is `matches <v>..`, an open `min` is
/// `matches ..<v>`. [`GateTerm::negate`] is the `unless` half — a term the gate
/// requires to be FALSE.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateTerm {
    /// The scoreboard objective the term reads.
    pub objective: String,
    /// Whether [`PARTY`] holds the value (`false` = the acting player does).
    pub party: bool,
    /// Lower bound, inclusive; `None` for an open end.
    pub min: Option<i32>,
    /// Upper bound, inclusive; `None` for an open end.
    pub max: Option<i32>,
    /// Whether the gate requires the range NOT to hold.
    pub negate: bool,
}

impl GateTerm {
    /// The holder as an emitted selector: the party fake player, or `@s`.
    #[must_use]
    pub fn holder(&self) -> &'static str {
        if self.party { PARTY } else { "@s" }
    }

    /// The `matches` range, as vanilla spells it.
    #[must_use]
    pub fn range(&self) -> String {
        match (self.min, self.max) {
            (Some(a), Some(b)) if a == b => format!("{a}"),
            (Some(a), Some(b)) => format!("{a}..{b}"),
            (Some(a), None) => format!("{a}.."),
            (None, Some(b)) => format!("..{b}"),
            (None, None) => String::new(),
        }
    }

    /// This term as one `execute` sub-clause, **without** a leading space.
    ///
    /// `negate` asks for the clause that asserts the term is NOT satisfied — the
    /// "any single term failing shuts it" form a trap's arming tick needs. It
    /// composes with [`GateTerm::negate`]: flipping the keyword is the whole of
    /// negation, so the two readings can never disagree about what the range
    /// means.
    #[must_use]
    pub fn clause(&self, negate: bool) -> String {
        let kw = if self.negate == negate {
            "if"
        } else {
            "unless"
        };
        format!(
            "{kw} score {} {} matches {}",
            self.holder(),
            self.objective,
            self.range()
        )
    }
}

impl<'a> Plan<'a> {
    /// **A gate, reduced to the scoreboard terms that decide it** — flags, then
    /// forbidden flags, then the numeric comparisons, in gate field order.
    ///
    /// The one reduction ([`GateTerm`]), so the `execute` condition the emitter
    /// writes and the terms `validation/death-plan.json` hands the bot tier are
    /// the same reading of the same declaration. Empty for an ungated site, so a
    /// caller that splices it in unconditionally emits exactly what it emitted
    /// before DSL v0.10.
    pub fn gate_terms(&self, gate: delvewright_dsl::gate::Gate<'_>) -> Vec<GateTerm> {
        gate_terms_of(self.campaign, gate)
    }

    /// The numeric half of a gate, as [`GateTerm`]s ([`state_terms_of`]).
    pub fn state_terms(&self, cmps: &[delvewright_dsl::StateCompare]) -> Vec<GateTerm> {
        state_terms_of(self.campaign, cmps)
    }
}

/// **A gate, reduced to scoreboard terms** — the one reduction ([`GateTerm`])
/// every consumer takes: [`Plan::gate_terms`] for a site the plan holds, and a
/// lethal volume's [`StagedGate`] before the plan exists. Empty for an ungated
/// site.
pub fn gate_terms_of(campaign: &Campaign, gate: delvewright_dsl::gate::Gate<'_>) -> Vec<GateTerm> {
    let mut out: Vec<GateTerm> = Vec::new();
    for f in gate.requires_flags {
        out.push(GateTerm {
            objective: flag_score(f.as_str()),
            party: true,
            min: Some(1),
            max: Some(1),
            negate: false,
        });
    }
    for f in gate.forbids_flags {
        out.push(GateTerm {
            objective: flag_score(f.as_str()),
            party: true,
            min: Some(1),
            max: Some(1),
            negate: true,
        });
    }
    out.extend(state_terms_of(campaign, gate.requires_state));
    out
}

/// The numeric half of a gate, as [`GateTerm`]s.
///
/// `equals` and `not-equals` are the same one-value range under opposite
/// keywords; `at-least` and `at-most` are the half-open ranges. Who holds the
/// value is the datum's declared scope and nothing else — a `party` datum
/// lives on [`PARTY`], a `player` one on the acting player. An undeclared
/// datum (already `DW0500`) answers `party`, so a campaign that failed
/// validation still yields something well-formed rather than panicking
/// mid-build.
pub fn state_terms_of(
    campaign: &Campaign,
    cmps: &[delvewright_dsl::StateCompare],
) -> Vec<GateTerm> {
    use delvewright_dsl::{CompareOp, StateScope};
    cmps.iter()
        .map(|c| {
            let party = !matches!(
                campaign
                    .quests
                    .content
                    .state_decl(c.state.as_str())
                    .map(|s| s.scope),
                Some(StateScope::Player)
            );
            let (min, max, negate) = match c.op {
                CompareOp::Equals => (Some(c.value), Some(c.value), false),
                CompareOp::NotEquals => (Some(c.value), Some(c.value), true),
                CompareOp::AtLeast => (Some(c.value), None, false),
                CompareOp::AtMost => (None, Some(c.value), false),
            };
            GateTerm {
                objective: state_score(c.state.as_str()),
                party,
                min,
                max,
                negate,
            }
        })
        .collect()
}
