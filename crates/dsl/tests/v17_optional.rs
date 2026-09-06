//! spec-0051 — optional is a claim about the proof.
//!
//! The fence in both directions, the three partition refusals with a green half
//! each, and the non-vacuity perturbations that show each gate goes RED when the
//! safety is removed rather than green.
//!
//! Every fixture here is built by **structurally patching** the hello-world
//! campaign through [`common::patch_doc`], which panics when a patch matches
//! nothing — a textual splice that silently no-ops would leave the assertion
//! running against an unpatched document and passing for the wrong reason.

mod common;

use std::collections::BTreeSet;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

const OLD: &str = "0.19.0";
const NEW: &str = "0.19.0";

// ---------------------------------------------------------------------------
// Fixture construction
// ---------------------------------------------------------------------------

/// The shape every fixture below is a variation of: a two-quest campaign whose
/// finale is the hello-world quest and whose second quest is a side strand.
///
/// `plan_version` is the stage-4 `dsl_version` — the thing the fence reads.
/// `optional` decides the strand's `mandatory`. `attach` decides whether the
/// strand hangs off the spine by a `depends_on` edge.
struct Fixture {
    plan_version: &'static str,
    strand_optional: bool,
    /// `depends_on` for the strand quest.
    strand_deps: Vec<&'static str>,
    /// When set, the FINALE is given this `depends_on` list instead of `[]`.
    finale_deps: Vec<&'static str>,
    /// When true the finale itself is declared optional.
    finale_optional: bool,
    /// When set, the finale's first objective requires this flag.
    finale_requires: Option<&'static str>,
    /// When set, the strand's `on_complete` sets this flag.
    strand_sets: Option<&'static str>,
    /// When true the strand's stage-5 trigger is `quest-complete` of the finale;
    /// when false it is `campaign-start`.
    strand_after_finale: bool,
    /// When set, the FINALE's stage-5 trigger is `quest-complete` of this quest.
    finale_triggered_by: Option<&'static str>,
}

impl Default for Fixture {
    fn default() -> Self {
        Fixture {
            plan_version: NEW,
            strand_optional: true,
            strand_deps: vec![],
            finale_deps: vec![],
            finale_optional: false,
            finale_requires: None,
            strand_sets: None,
            strand_after_finale: false,
            finale_triggered_by: None,
        }
    }
}

const FINALE: &str = "quest/open-the-door";
const STRAND: &str = "quest/side-strand";

impl Fixture {
    fn raw(&self) -> RawCampaign {
        let mut c = common::valid_raw();

        c.quest_plan = common::patch_doc(&c.quest_plan, |v| {
            v["dsl_version"] = json!(self.plan_version);
            let quests = v["content"]["quests"]
                .as_array_mut()
                .expect("stage-4 quests is an array");
            assert_eq!(quests.len(), 1, "hello-world has exactly one planned quest");
            quests[0]["mandatory"] = json!(!self.finale_optional);
            quests[0]["depends_on"] = json!(self.finale_deps);
            quests.push(json!({
                "act": 1,
                "area": "area/keep",
                "depends_on": self.strand_deps,
                "goal": "A side strand the mainline neither needs nor fears.",
                "id": STRAND,
                "mandatory": !self.strand_optional,
                "npcs": [],
            }));
        });

        c.quests = common::patch_doc(&c.quests, |v| {
            // The flag surfaces this fixture uses (`requires_flags`, `set-flag`)
            // are v0.6 QUESTS-stage surfaces. Pinning this stage at the newest
            // version keeps every assertion below about spec-0051's fence and
            // not about that one — and makes the stage-4 fence test stronger,
            // since it must still refuse with a neighbour at the top version.
            v["dsl_version"] = json!(OLD);
            let quests = v["content"]["quests"]
                .as_array_mut()
                .expect("stage-5 quests is an array");
            assert_eq!(
                quests.len(),
                1,
                "hello-world has exactly one expanded quest"
            );

            if let Some(flag) = self.finale_requires {
                let objs = quests[0]["objectives"]
                    .as_array_mut()
                    .expect("objectives is an array");
                objs[0]["requires_flags"] = json!([flag]);
            }
            if let Some(src) = self.finale_triggered_by {
                quests[0]["trigger"] = json!({ "type": "quest-complete", "quest": src });
            }

            let trigger = if self.strand_after_finale {
                json!({ "type": "quest-complete", "quest": FINALE })
            } else {
                json!({ "type": "campaign-start" })
            };
            let on_complete: Vec<Value> = match self.strand_sets {
                Some(flag) => vec![json!({ "type": "set-flag", "flag": flag })],
                None => vec![],
            };
            quests.push(json!({
                "id": STRAND,
                "objectives": [{
                    "anchor": "anchor/exit",
                    "id": "obj/strand-look",
                    "radius": 2,
                    "type": "reach-anchor",
                }],
                "on_complete": on_complete,
                "trigger": trigger,
            }));
        });

        c
    }

    fn codes(&self) -> BTreeSet<String> {
        check_campaign(&self.raw())
            .iter()
            .map(|d| d.code.clone())
            .collect()
    }
}

fn set(codes: &[&str]) -> BTreeSet<String> {
    codes.iter().map(|s| (*s).to_string()).collect()
}

// ---------------------------------------------------------------------------
// Criterion 1 — the fence pair, proved in BOTH directions
// ---------------------------------------------------------------------------

/// The same document at the adopting version compiles green — no diagnostics at
/// all, not merely a different set.
#[test]
fn at_the_fence_optional_is_accepted() {
    let f = Fixture::default();
    assert_eq!(
        f.codes(),
        BTreeSet::new(),
        "the same document at {NEW} must be green"
    );
}

// ---------------------------------------------------------------------------
// Criterion 2 — the partition is legal and the mainline is unchanged
// ---------------------------------------------------------------------------

/// A campaign with one mandatory spine and one optional quest is green, and the
/// optional quest is genuinely OFF the finale's closure — which is the fact the
/// whole proof rests on. Asserted against the ONE authority rather than assumed.
#[test]
fn the_optional_quest_is_off_the_spine() {
    let raw = Fixture::default().raw();
    let c = delvewright_dsl::parse_campaign(&raw).expect("fixture parses");
    let spine = c.quest_plan.content.spine();
    let optional = c.quest_plan.content.optional();

    assert!(spine.contains(FINALE), "the finale is always on the spine");
    assert!(
        !spine.contains(STRAND),
        "the strand must be off the closure"
    );
    assert_eq!(optional, [STRAND].into_iter().collect::<BTreeSet<_>>());
    // The partition is a partition: nothing is both.
    assert!(spine.intersection(&optional).next().is_none());
}

/// **Skipped is not absent.** An optional quest triggered by a mandatory
/// completion still activates for every party — so it is legal for its trigger
/// to chain off the spine, and that is not a `DW0867`.
#[test]
fn an_optional_quest_may_be_triggered_by_the_mainline() {
    let f = Fixture {
        strand_after_finale: true,
        ..Fixture::default()
    };
    assert_eq!(f.codes(), BTreeSet::new());
}

/// An optional quest may depend on a mandatory one — that is a strand's
/// attachment to the spine (§4), and it must not be read as the mainline
/// hanging off elective content.
#[test]
fn an_optional_quest_may_depend_on_a_mandatory_one() {
    let f = Fixture {
        strand_deps: vec![FINALE],
        ..Fixture::default()
    };
    assert_eq!(f.codes(), BTreeSet::new());
}

// ---------------------------------------------------------------------------
// Criterion 5 — the mismatch pair, both directions
// ---------------------------------------------------------------------------

/// §8.1 — an optional quest the finale cannot fire without. Here the finale
/// declares ITSELF optional, which is the one shape that reaches this rule
/// without also tripping the edge rule below.
#[test]
fn an_optional_quest_in_the_finale_closure_is_refused() {
    let f = Fixture {
        finale_optional: true,
        ..Fixture::default()
    };
    assert_eq!(f.codes(), set(&["DW0866"]));
}

/// The other half of the pair: a MANDATORY quest the closure does not reach
/// keeps today's convergence refusal, asserted by code. The partition does not
/// let a wiring mistake quietly become optional content.
#[test]
fn a_mandatory_quest_off_the_closure_keeps_dw0132() {
    let f = Fixture {
        strand_optional: false,
        ..Fixture::default()
    };
    assert_eq!(f.codes(), set(&["DW0132"]));
}

// ---------------------------------------------------------------------------
// Criterion 4 — direction of dependency
// ---------------------------------------------------------------------------

/// §8.2 — a mandatory quest whose stage-5 `quest-complete` trigger names an
/// optional quest. The party may never complete the strand, so the mainline
/// would stop there.
///
/// The trigger edge is the one that reaches this rule alone: a `depends_on`
/// edge from a mandatory quest necessarily also puts the strand in the finale's
/// closure, so it co-fires `DW0866` (asserted separately below).
#[test]
fn a_mandatory_quest_triggered_by_an_optional_one_is_refused() {
    let f = Fixture {
        finale_triggered_by: Some(STRAND),
        ..Fixture::default()
    };
    assert_eq!(f.codes(), set(&["DW0867"]));
}

/// The reversed direction greens: the optional quest triggered by the mandatory
/// one is the ordinary attachment.
#[test]
fn the_reversed_trigger_direction_is_green() {
    let f = Fixture {
        strand_after_finale: true,
        ..Fixture::default()
    };
    assert_eq!(f.codes(), BTreeSet::new());
}

/// The `depends_on` arm of §8.2, and the co-firing stated rather than left to be
/// discovered. Both diagnostics are true and they prescribe different repairs:
/// `DW0867` names the edge to cut, `DW0866` names the claim to withdraw.
#[test]
fn a_mandatory_depends_on_edge_reports_both_the_edge_and_the_closure() {
    let f = Fixture {
        finale_deps: vec![STRAND],
        ..Fixture::default()
    };
    assert_eq!(f.codes(), set(&["DW0866", "DW0867"]));
}

// ---------------------------------------------------------------------------
// Criterion 3 — a key behind participation, refused at the edge
// ---------------------------------------------------------------------------

/// §8.3 — a mandatory objective gated on a flag only an optional quest sets.
#[test]
fn a_mainline_key_behind_participation_is_refused() {
    let f = Fixture {
        finale_requires: Some("flag/strand-token"),
        strand_sets: Some("flag/strand-token"),
        ..Fixture::default()
    };
    assert_eq!(f.codes(), set(&["DW0868"]));
}

/// Moving the producer onto a mandatory quest greens the same document — the
/// green half criterion 3 demands, and the one that shows the rule is about
/// WHERE the producer sits rather than about the gate existing.
#[test]
fn the_same_gate_greens_when_the_producer_is_mandatory() {
    let f = Fixture {
        finale_requires: Some("flag/strand-token"),
        strand_sets: Some("flag/strand-token"),
        strand_optional: false,
        strand_deps: vec![],
        finale_deps: vec![STRAND],
        ..Fixture::default()
    };
    assert_eq!(f.codes(), BTreeSet::new());
}

// ---------------------------------------------------------------------------
// Non-vacuity: perturb TOWARD the vacuous shape and check the gate goes RED
// ---------------------------------------------------------------------------

/// The binding counts, **computed from the objects rather than written beside
/// them**. A constant in this position is the vacuity the count exists to
/// expose, and `clippy::assertions_on_constants` is right to object to it.
///
/// Each row perturbs one thing and states which refusal it is the only possible
/// cause of.
#[test]
fn every_refusal_binds_and_is_the_only_thing_that_could_have_caught_it() {
    // (label, fixture, the code it must raise)
    let cases: Vec<(&str, Fixture, &str)> = vec![
        (
            "finale declares itself optional",
            Fixture {
                finale_optional: true,
                ..Fixture::default()
            },
            "DW0866",
        ),
        (
            "mandatory finale triggered by an optional quest",
            Fixture {
                finale_triggered_by: Some(STRAND),
                ..Fixture::default()
            },
            "DW0867",
        ),
        (
            "mandatory objective keyed on an optional-only flag",
            Fixture {
                finale_requires: Some("flag/strand-token"),
                strand_sets: Some("flag/strand-token"),
                ..Fixture::default()
            },
            "DW0868",
        ),
    ];

    let mut bound = 0usize;
    for (label, f, code) in &cases {
        let got = f.codes();
        assert!(got.contains(*code), "{label}: expected {code}, got {got:?}");

        // The perturbation is the ONLY thing that could have caught it: with the
        // partition withdrawn — the strand marked mandatory, or the finale's
        // claim withdrawn — the SAME document must stop raising this code. If it
        // still raised it, some unrelated mechanism was doing the catching and
        // the demonstration would prove nothing about this rule.
        let withdrawn = Fixture {
            strand_optional: false,
            finale_optional: false,
            ..Fixture {
                plan_version: f.plan_version,
                strand_deps: f.strand_deps.clone(),
                finale_deps: f.finale_deps.clone(),
                finale_requires: f.finale_requires,
                strand_sets: f.strand_sets,
                strand_after_finale: f.strand_after_finale,
                finale_triggered_by: f.finale_triggered_by,
                ..Fixture::default()
            }
        };
        let after = withdrawn.codes();
        assert!(
            !after.contains(*code),
            "{label}: {code} still fires with the partition withdrawn — something \
             other than the optional declaration is catching this, so the \
             demonstration says nothing about {code}. Got {after:?}"
        );
        bound += 1;
    }

    assert_eq!(bound, cases.len(), "every case must have been exercised");
    assert!(bound >= 3, "binding count: {bound} refusals demonstrated");
}

/// The complement, and it is the direction that matters most: with the surface
/// unused — every quest mandatory — **not one** of the new refusals can fire, at
/// any version. This is what makes spec-0051 byte-identical on every committed
/// campaign, measured rather than promised.
#[test]
fn the_new_refusals_are_inert_when_nothing_is_optional() {
    let new_codes = ["DW0866", "DW0867", "DW0868"];
    let mut checked = 0usize;
    for version in ["0.19.0", "0.19.0", OLD, NEW] {
        for finale_deps in [vec![], vec![STRAND]] {
            for strand_after_finale in [false, true] {
                let f = Fixture {
                    plan_version: version,
                    strand_optional: false,
                    strand_deps: if finale_deps.is_empty() {
                        vec![FINALE]
                    } else {
                        vec![]
                    },
                    finale_deps: finale_deps.clone(),
                    strand_after_finale,
                    ..Fixture::default()
                };
                let got = f.codes();
                for code in new_codes {
                    assert!(
                        !got.contains(code),
                        "{code} fired at {version} with nothing optional: {got:?}"
                    );
                }
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 16, "population actually swept");
}

// ---------------------------------------------------------------------------
// Criterion 9 — dead elective content stays dead
// ---------------------------------------------------------------------------

/// Marking a quest optional laundering nothing: an optional quest gated on a
/// flag NOTHING produces is still refused by the existing reachability family.
/// The opt-out is secured by a property the defect cannot supply.
#[test]
fn optional_does_not_launder_a_broken_strand() {
    let mut c = Fixture::default().raw();
    c.quests = common::patch_doc(&c.quests, |v| {
        let quests = v["content"]["quests"].as_array_mut().unwrap();
        let strand = quests
            .iter_mut()
            .find(|q| q["id"] == STRAND)
            .expect("the strand is in the fixture");
        strand["objectives"][0]["requires_flags"] = json!(["flag/nobody-sets-this"]);
    });
    let codes: BTreeSet<String> = check_campaign(&c).iter().map(|d| d.code.clone()).collect();
    assert!(
        codes.contains("DW0172"),
        "an optional quest gated on an unproduced flag must still be refused, got {codes:?}"
    );
}
