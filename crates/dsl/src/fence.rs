//! **The obligation fence**: a campaign is processed according to its DECLARED
//! `dsl_version`, so a campaign that compiled before keeps its behaviour
//! unchanged (owner ruling, 2026-08-10).
//!
//! ## What this is for
//!
//! Per-stage `dsl_version` fences have always guarded **new surface** — "you may
//! not write this field below version X" (`DW0141`). Nothing guarded **new
//! obligations** — "you are now required to have X". The asymmetry cost the
//! project a staging round (task #51): `dsl::l10n::each_string` was widened onto
//! an actor's own `name` with no version gate, `DW0180` compares key SETS and had
//! no version gate either, and the obligation therefore reached every campaign at
//! every declared version instantly. `nobodys-cave-island` (0.6.0/0.8.0) went red
//! mid-staging with nothing in its own documents changed, and was unblocked with
//! a three-string patch rather than an adoption — precisely the anti-pattern
//! CLAUDE.md's version-adoption discipline exists to prevent.
//!
//! ## The mechanism, in two halves
//!
//! 1. **Declaration is compulsory.** A [`Diagnostic`] can only be built from a
//!    [`DwCode`](crate::DwCode), and a `DwCode` can only be built by naming which
//!    of the two kinds of rule it is ([`Binds::EveryVersion`] /
//!    [`Binds::Since`]). There is no constructor for "did not say", so "forgot to
//!    fence" is not a category of mistake an author can make.
//! 2. **The fence is the only exit.** [`Fenced`] is the type `delvec` prints and
//!    derives an exit code from, and its only campaign-aware constructor is
//!    [`Fenced::apply`], which drops every `Since(n)` diagnostic raised against a
//!    stage that declares less than `n`. A check therefore cannot reach a verdict
//!    against a campaign that never opted into it, however the check itself is
//!    written.
//!
//! ## Two granularities, because obligations have two shapes
//!
//! An obligation reaches an old campaign in one of two ways, and only the first
//! is a per-code question:
//!
//! * **A new check.** A new `DW` code with no fence. The mechanism above is
//!   exactly this case: the code declares [`Binds::Since`] and the fence carries
//!   it.
//! * **An existing check's binding widens.** Same code, more objects examined —
//!   task #51's actual shape, which added no code at all. A per-code fence cannot
//!   see it, because the code was always allowed to fire. Such a check has to
//!   version its own **binding**, at whatever granularity that binding has;
//!   [`crate::l10n::required_inventory`] is the worked instance (an l10n key
//!   declares the version at which it entered the inventory, and a sidecar is
//!   never asked for a key from above its campaign's version).
//!
//! Both halves answer to the same rule, stated once in [`Binds`]: *could this go
//! from green to red on a campaign whose own documents did not change?*
//!
//! ## What the fence deliberately does NOT promise
//!
//! Not byte-identical emission forever. A released delve reproduces through its
//! pinned engine (`versions.toml` + the OCI image), per CLAUDE.md and ADR-0010;
//! this is about **verdicts and behaviour at a declared version**, which is what
//! the ruling asked for.

use std::collections::BTreeMap;

use crate::diagnostic::{Binds, Diagnostic, Severity};
use crate::envelope::{Campaign, minor_ordinal};

/// A diagnostic list that has passed the obligation fence — the only shape
/// `delvec` reports or derives an exit code from.
///
/// Constructing one is the fence: there is no `From<Vec<Diagnostic>>`, no public
/// field and no way to hand a raw list to the printer. The two constructors
/// correspond to the only two situations that exist —
/// [`apply`](Fenced::apply) when a parsed campaign is in hand, and
/// [`structural`](Fenced::structural) when there is not yet one to read a version
/// off, which is why that constructor refuses anything version-scoped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fenced {
    reported: Vec<Diagnostic>,
    grandfathered: Vec<Diagnostic>,
}

impl Fenced {
    /// Fence `diags` against `c`'s declared per-stage `dsl_version`s.
    ///
    /// A [`Binds::Since`] diagnostic is **grandfathered** — kept out of the
    /// report and out of the exit code — when the stage it names declares less
    /// than the version at which its rule started binding. Everything else is
    /// reported exactly as raised: this pass never rewrites a diagnostic, never
    /// changes a severity, and never adds one.
    pub fn apply(c: &Campaign, diags: Vec<Diagnostic>) -> Fenced {
        let versions = stage_versions(c);
        let mut reported = Vec::new();
        let mut grandfathered = Vec::new();
        for d in diags {
            match d.binds {
                Binds::EveryVersion => reported.push(d),
                Binds::Since(need) => {
                    if stage_ordinal(&versions, &d.stage) >= need {
                        reported.push(d);
                    } else {
                        grandfathered.push(d);
                    }
                }
            }
        }
        Fenced {
            reported,
            grandfathered,
        }
    }

    /// Fence a list raised **before any campaign exists** — a stage document that
    /// did not parse, so there is no declared `dsl_version` to read.
    ///
    /// It cannot grandfather, so it refuses to carry anything that would need to
    /// be: a [`Binds::Since`] diagnostic here is dropped rather than reported,
    /// because reporting it would be exactly the unfenced obligation this module
    /// exists to make impossible. In practice the set is `DW0100` alone (a schema
    /// failure is [`Binds::EveryVersion`] by nature), which
    /// `structural_path_raises_only_everyversion_codes` pins.
    pub fn structural(diags: Vec<Diagnostic>) -> Fenced {
        let (reported, grandfathered): (Vec<_>, Vec<_>) = diags
            .into_iter()
            .partition(|d| d.binds == Binds::EveryVersion);
        Fenced {
            reported,
            grandfathered,
        }
    }

    /// The diagnostics that survived the fence — what a user sees.
    pub fn reported(&self) -> &[Diagnostic] {
        &self.reported
    }

    /// The diagnostics the fence withheld, and why they were withheld: the
    /// **binding count** of the fence on this run, so a round summary can state a
    /// number instead of asserting a property (CLAUDE.md: a green gate that binds
    /// to nothing is vacuous, not a pass).
    pub fn grandfathered(&self) -> &[Diagnostic] {
        &self.grandfathered
    }

    /// True if any reported diagnostic is a hard rejection.
    pub fn has_error(&self) -> bool {
        self.reported.iter().any(|d| d.severity == Severity::Error)
    }
}

impl std::ops::Deref for Fenced {
    type Target = [Diagnostic];

    fn deref(&self) -> &[Diagnostic] {
        &self.reported
    }
}

/// Each stage document's declared `dsl_version`, by the stage name a diagnostic
/// carries in its `stage` field.
fn stage_versions(c: &Campaign) -> BTreeMap<&'static str, u32> {
    let mut m = BTreeMap::new();
    m.insert("world", minor_ordinal(&c.world.dsl_version));
    m.insert("npcs", minor_ordinal(&c.npcs.dsl_version));
    m.insert("classes", minor_ordinal(&c.classes.dsl_version));
    m.insert("quest-plan", minor_ordinal(&c.quest_plan.dsl_version));
    m.insert("quests", minor_ordinal(&c.quests.dsl_version));
    m.insert("dialogue", minor_ordinal(&c.dialogue.dsl_version));
    if let Some(we) = &c.world_edits {
        m.insert("world-edits", minor_ordinal(&we.dsl_version));
    }
    m
}

/// The version a diagnostic's `stage` is judged at.
///
/// A stage that names a document reads that document's own declaration. Anything
/// else — `l10n`, `prefabs`, `build`, the empty stage, a stage-7 diagnostic on a
/// campaign that ships no stage 7 — is judged at the **minimum** across the
/// campaign's stage documents. Those diagnostics are about artifacts that carry
/// no `dsl_version` of their own but are derived from the whole campaign (an
/// l10n sidecar covers every stage's strings), and a campaign is only as adopted
/// as its least-adopted stage: taking the minimum is the reading that
/// grandfathers, and grandfathering is the direction that is safe to be wrong in.
fn stage_ordinal(versions: &BTreeMap<&'static str, u32>, stage: &str) -> u32 {
    versions
        .get(stage)
        .copied()
        .unwrap_or_else(|| versions.values().copied().min().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::DwCode;

    #[test]
    fn structural_refuses_to_carry_a_version_scoped_obligation() {
        // Built inline rather than as `const`s: a `const … : DwCode = …"DWxxxx"`
        // is how `tools/check-dw-codes.py` learns that a symbol DECLARES a code,
        // and a test fixture declaring `DW0100` would read as a second rule
        // wearing the same number. (It said so, which is the check working.)
        let every = DwCode::every_version("DW0100");
        let since8 = DwCode::since("DW0481", 8);
        let f = Fenced::structural(vec![
            Diagnostic::error(every, "quests", "", "malformed"),
            Diagnostic::error(since8, "quests", "", "requires a happening"),
        ]);
        assert_eq!(f.reported().len(), 1);
        assert_eq!(f.reported()[0].code, "DW0100");
        assert_eq!(f.grandfathered().len(), 1);
        assert!(f.has_error());
    }

    /// The claim [`Fenced::structural`]'s doc makes, checked: the only code the
    /// pre-parse path can raise is the schema failure, and it is
    /// [`Binds::EveryVersion`] — so `structural`'s refusal to carry a
    /// version-scoped rule discards nothing real.
    #[test]
    fn structural_path_raises_only_everyversion_codes() {
        let garbage = "not json".to_string();
        let diags = crate::envelope::parse_campaign(&crate::envelope::RawCampaign {
            world: garbage.clone(),
            npcs: garbage.clone(),
            classes: garbage.clone(),
            quest_plan: garbage.clone(),
            quests: garbage.clone(),
            dialogue: garbage.clone(),
            world_edits: Some(garbage),
        })
        .expect_err("unparseable stages cannot produce a campaign");
        assert_eq!(diags.len(), 7, "one schema failure per stage document");
        assert!(diags.iter().all(|d| d.binds == Binds::EveryVersion));
        let f = Fenced::structural(diags.clone());
        assert_eq!(f.reported().len(), diags.len());
        assert!(f.grandfathered().is_empty());
    }

    /// The unmapped-stage rule: an `l10n` diagnostic is judged at the campaign's
    /// least-adopted stage, so a sidecar obligation cannot bind through the one
    /// stage that happens to have been raised.
    #[test]
    fn an_unmapped_stage_takes_the_minimum() {
        let mut versions = BTreeMap::new();
        versions.insert("world", 6u32);
        versions.insert("quests", 10u32);
        assert_eq!(stage_ordinal(&versions, "quests"), 10);
        assert_eq!(stage_ordinal(&versions, "l10n"), 6);
        assert_eq!(stage_ordinal(&versions, ""), 6);
    }
}
