//! `DW0810` — runtime-watch coverage of per-object bodies.
//!
//! The check is judged over a finished tree, so these fixtures ARE finished
//! trees: a handful of emitted paths and their bytes. That is deliberate. A
//! test that drove a real campaign would only ever exercise the mechanics that
//! campaign happens to declare, and the whole point of `watch` is that it names
//! no mechanic at all.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_compiler::watch::{self, DW_UNWATCHED_SIBLING};

const NS: &str = "g";

fn ids(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

/// A tree: campaign function names, then (template name, body) pairs.
fn tree(functions: &[&str], templates: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for f in functions {
        out.insert(
            format!("datapack/data/{NS}/function/{f}.mcfunction"),
            b"# body\n".to_vec(),
        );
    }
    for (name, body) in templates {
        out.insert(
            format!("packtest-datapack/data/{NS}/test/{name}.mcfunction"),
            body.as_bytes().to_vec(),
        );
    }
    out
}

/// The reported defect, reduced to its bones: three declared gates, one body
/// each, and a suite that drives only the first. The two it skips are named,
/// and the one it skips LAST is the one the level put the crush on.
#[test]
fn a_family_the_suite_only_partly_drives_is_dw0810() {
    let out = tree(
        &[
            "tgate_open_side_door",
            "tgate_open_mid_door",
            "tgate_open_crush_door",
        ],
        &[("souls_timed_gate", "function g:tgate_open_side_door\n")],
    );
    let (binding, findings) =
        watch::check_tree(NS, &out, &ids(&["side_door", "mid_door", "crush_door"]));

    assert_eq!(binding.multi_object_families, 1);
    assert_eq!(binding.watched_objects, 1);
    assert_eq!(binding.unwatched_objects, 2);
    assert_eq!(
        findings.iter().map(|u| &u.function).collect::<Vec<_>>(),
        ["tgate_open_crush_door", "tgate_open_mid_door"],
        "both skipped siblings are named, not just the next one"
    );
    for u in &findings {
        assert_eq!(u.family, "tgate_open_");
        assert_eq!(
            u.watched_siblings,
            ["side_door"],
            "the finding carries the sibling that IS driven — the proof this is a \
             family the suite claims to watch"
        );
    }

    let d = watch::finding(&binding, &findings).expect("DW0810 fires");
    assert_eq!(d.code, DW_UNWATCHED_SIBLING);
    assert!(
        d.message.contains("tgate_open_crush_door") && d.message.contains("tgate_open_mid_door"),
        "the message names every unwatched sibling: {}",
        d.message
    );
}

/// The fixed emitter: a template per gate, so nothing is left behind.
#[test]
fn a_family_the_suite_drives_whole_is_silent() {
    let out = tree(
        &[
            "tgate_open_side_door",
            "tgate_open_mid_door",
            "tgate_open_crush_door",
        ],
        &[
            (
                "souls_timed_gate_side_door",
                "function g:tgate_open_side_door\n",
            ),
            (
                "souls_timed_gate_mid_door",
                "function g:tgate_open_mid_door\n",
            ),
            (
                "souls_timed_gate_crush_door",
                "function g:tgate_open_crush_door\n",
            ),
        ],
    );
    let (binding, findings) =
        watch::check_tree(NS, &out, &ids(&["side_door", "mid_door", "crush_door"]));
    assert_eq!(binding.watched_objects, 3);
    assert_eq!(binding.unwatched_objects, 0);
    assert!(findings.is_empty());
    assert!(watch::finding(&binding, &findings).is_none());
}

/// A sub-body is not a sibling. `lethal_east_pit_kill` shares every character of
/// `lethal_east_pit`'s prefix, but only the latter ends in a declared id — so the
/// former is the object's own machinery, not a second object. Without this the
/// rule reports sixteen families on the gallery where eight are real.
#[test]
fn a_sub_body_is_not_a_sibling() {
    let out = tree(
        &["lethal_east_pit", "lethal_east_pit_kill", "lethal_west_pit"],
        &[(
            "lethal",
            "function g:lethal_east_pit\nfunction g:lethal_west_pit\n",
        )],
    );
    let (binding, findings) = watch::check_tree(NS, &out, &ids(&["east_pit", "west_pit"]));
    assert_eq!(binding.unwatched_objects, 0);
    assert!(
        findings.is_empty(),
        "`_kill` is the object's own machinery, not an undriven sibling: {findings:?}"
    );
}

/// A family NOTHING drives is counted, never diagnosed. It is a different and
/// far broader question — most emitted functions have no template and never
/// will — and folding it in here would bury the finding this exists to surface.
/// The limit is reported rather than silent, which is the whole difference
/// between a drawn scope and an unbound gate.
#[test]
fn a_family_nothing_drives_is_counted_not_diagnosed() {
    let out = tree(&["bark_a", "bark_b"], &[("t", "# drives nothing\n")]);
    let (binding, findings) = watch::check_tree(NS, &out, &ids(&["a", "b"]));
    assert_eq!(binding.multi_object_families, 1);
    assert_eq!(binding.unwatched_families, 1);
    assert_eq!(binding.unwatched_objects, 0);
    assert!(findings.is_empty());
    assert!(
        binding.to_json(&findings)["unwatched_families"] == 1,
        "and it travels with the build, so the limit is never silent"
    );
}

/// Where two declared ids both match, the longest wins — otherwise
/// `tgate_open_side_door` would be filed under `door` and land in a family with
/// every other id ending in `door`.
#[test]
fn the_longest_declared_id_wins() {
    let out = tree(
        &["tgate_open_side_door", "tgate_open_back_door"],
        &[("t", "function g:tgate_open_side_door\n")],
    );
    let (_, findings) = watch::check_tree(NS, &out, &ids(&["door", "side_door", "back_door"]));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].id, "back_door");
    assert_eq!(findings[0].family, "tgate_open_");
}

/// Mentioning a body is not driving it. A template that names a function in a
/// comment has proven nothing about it, and a watch definition that accepted
/// that would be satisfied by exactly the thing it exists to catch.
#[test]
fn a_mention_is_not_a_proof() {
    let out = tree(
        &["unleash_a", "unleash_b"],
        &[(
            "t",
            "function g:unleash_a\n# unleash_b is covered elsewhere, honest\n",
        )],
    );
    let (_, findings) = watch::check_tree(NS, &out, &ids(&["a", "b"]));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].function, "unleash_b");
}

/// The id authority is a generic walk over the authored documents: every `id`
/// at any depth, in any stage, with no list of mechanics anywhere. A stage or a
/// mechanic added later is covered without touching `watch`.
#[test]
fn declared_ids_are_walked_generically_from_the_authored_documents() {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "quests.json".to_string(),
        br#"{"content":{"timed_gates":[{"id":"timed-gate/side-door"}],
             "nested":{"deep":[{"id":"actor/rafter.spider"}]}}}"#
            .to_vec(),
    );
    inputs.insert(
        "not-json.txt".to_string(),
        b"this is not a stage document".to_vec(),
    );
    let ids = watch::declared_ids(&inputs);
    assert!(
        ids.contains("side_door"),
        "namespace stripped, dashes safed"
    );
    assert!(
        ids.contains("rafter_spider"),
        "found at depth, dots safed too: {ids:?}"
    );
}

/// Determinism (ADR-0006): the finding list and the message are a function of
/// the tree alone.
#[test]
fn the_verdict_is_deterministic() {
    let out = tree(
        &["talk_a", "talk_b", "talk_c"],
        &[("t", "function g:talk_a\n")],
    );
    let want = watch::check_tree(NS, &out, &ids(&["a", "b", "c"]));
    for _ in 0..8 {
        assert_eq!(watch::check_tree(NS, &out, &ids(&["a", "b", "c"])), want);
    }
}

// --------------------------------------------------- DW0811, the refusal --
//
// `DW0810` above cannot refuse, and the reason is worth stating where the tests
// are: nothing in a finished tree separates *the emitter meant to prove every
// member and skipped some* from *the suite drives one exemplar by design*. Both
// are a family with a watched member and an unwatched one, and eight families on
// the gallery are honestly the second. The distinction lives in the emitter, so
// the emitter registers a `Claim` over the plan's own authored list — and THAT
// is a proof obligation the defect cannot discharge.

fn claim(families: &'static [&'static str], declared: &[&str]) -> watch::Claim {
    watch::Claim {
        mechanic: "timed-gate",
        families,
        declared: declared.iter().map(|s| (*s).to_string()).collect(),
    }
}

/// The motivating defect, as the refusal sees it: the emitter walked
/// `first()`, so it WROTE three bodies and drove one. `declared` comes from the
/// plan and does not shrink when the walk stops early, which is exactly why the
/// claim can refuse where the byte-read warning can only report.
#[test]
fn a_claim_the_suite_only_partly_discharges_is_dw0811() {
    let out = tree(
        &[
            "tgate_open_side_door",
            "tgate_open_mid_door",
            "tgate_open_inner_door",
        ],
        &[("souls_timed_gate", "function g:tgate_open_side_door\n")],
    );
    let claims = [claim(
        &["tgate_open_"],
        &["side_door", "mid_door", "inner_door"],
    )];
    let (binding, breaches) = watch::check_claims(NS, &out, &claims);

    assert_eq!(binding.claims, 1);
    assert_eq!(binding.declared_objects, 3);
    assert_eq!(binding.bodies_judged, 3);
    assert_eq!(binding.bodies_watched, 1);
    assert_eq!(
        breaches.iter().map(|b| &b.function).collect::<Vec<_>>(),
        ["tgate_open_inner_door", "tgate_open_mid_door"],
        "every undriven body is named, not just the next one"
    );

    let d = watch::claim_finding(&binding, &breaches).expect("DW0811 fires");
    assert_eq!(d.code, watch::DW_CLAIM_NOT_DISCHARGED);
    assert_eq!(
        d.severity,
        delvewright_dsl::Severity::Error,
        "refusal tier, not a warning"
    );
    assert!(
        d.message.contains("tgate_open_inner_door") && d.message.contains("tgate_open_mid_door"),
        "the message names every undischarged member: {}",
        d.message
    );
}

/// The repaired emitter discharges its claim, and the binding is stated rather
/// than assumed: seven bodies judged, seven driven.
#[test]
fn a_claim_the_suite_discharges_whole_is_silent() {
    let out = tree(
        &[
            "tgate_open_a",
            "tgate_open_b",
            "tgate_close_a",
            "tgate_close_b",
        ],
        &[
            (
                "souls_timed_gate_a",
                "function g:tgate_open_a\nfunction g:tgate_close_a\n",
            ),
            (
                "souls_timed_gate_b",
                "function g:tgate_open_b\nfunction g:tgate_close_b\n",
            ),
        ],
    );
    let claims = [claim(&["tgate_open_", "tgate_close_"], &["a", "b"])];
    let (binding, breaches) = watch::check_claims(NS, &out, &claims);
    assert_eq!(binding.bodies_judged, 4);
    assert_eq!(binding.bodies_watched, 4);
    assert!(breaches.is_empty());
    assert!(watch::claim_finding(&binding, &breaches).is_none());
}

/// A family emitted for a SUBSET of the declared list by design is not a
/// breach. `tgate_disarm_<id>` exists only for the gates that declare a disarm,
/// so the rule is over the body that was WRITTEN — written for a declared
/// object, therefore driven — and never over one that was never meant to exist.
/// Without this the refusal would red every campaign with an optional
/// affordance, which is how a correct rule gets weakened to get green.
#[test]
fn a_body_that_was_never_written_is_not_a_breach() {
    let out = tree(
        &["tgate_open_a", "tgate_open_b", "tgate_disarm_b"],
        &[(
            "t",
            "function g:tgate_open_a\nfunction g:tgate_open_b\nfunction g:tgate_disarm_b\n",
        )],
    );
    let claims = [claim(&["tgate_open_", "tgate_disarm_"], &["a", "b"])];
    let (binding, breaches) = watch::check_claims(NS, &out, &claims);
    assert_eq!(
        binding.bodies_judged, 3,
        "`tgate_disarm_a` was never emitted, so there is nothing to drive"
    );
    assert!(breaches.is_empty());
}

/// A campaign that declares none of the mechanic binds to nothing, and says so.
/// This is the one green that must never read as a pass on its own — the ledger
/// carries `bodies_judged: 0` so a zero binding is visible rather than absorbed.
#[test]
fn a_claim_over_an_empty_declaration_binds_to_nothing_and_states_it() {
    let out = tree(&["setup"], &[("t", "function g:setup\n")]);
    let claims = [claim(&["tgate_open_"], &[])];
    let (binding, breaches) = watch::check_claims(NS, &out, &claims);
    assert_eq!(binding.claims, 1);
    assert_eq!(binding.declared_objects, 0);
    assert_eq!(binding.bodies_judged, 0);
    assert!(breaches.is_empty());
    let ledger = binding.to_json(&breaches);
    assert_eq!(ledger["bodies_judged"], 0);
    assert_eq!(
        ledger["examined"], 0,
        "and it is spelled the way `check-gallery-coverage.py` already reds on, so a claim \
         that stops binding on the gallery is a red rather than a number nobody reads"
    );
}

/// Mentioning a body still is not driving it — the refusal reads the same
/// `function <ns>:<name>` invocation the warning does, from the same helper, so
/// the two gates can never disagree about what the tree contains.
#[test]
fn a_mention_does_not_discharge_a_claim() {
    let out = tree(
        &["tgate_open_a", "tgate_open_b"],
        &[(
            "t",
            "function g:tgate_open_a\n# tgate_open_b is covered elsewhere, honest\n",
        )],
    );
    let claims = [claim(&["tgate_open_"], &["a", "b"])];
    let (_, breaches) = watch::check_claims(NS, &out, &claims);
    assert_eq!(breaches.len(), 1);
    assert_eq!(breaches[0].function, "tgate_open_b");
}

/// Determinism (ADR-0006): the breach list is a function of the tree alone.
#[test]
fn the_claim_verdict_is_deterministic() {
    let out = tree(
        &["tgate_open_a", "tgate_open_b", "tgate_open_c"],
        &[("t", "function g:tgate_open_a\n")],
    );
    let claims = [claim(&["tgate_open_"], &["a", "b", "c"])];
    let want = watch::check_claims(NS, &out, &claims);
    for _ in 0..8 {
        assert_eq!(watch::check_claims(NS, &out, &claims), want);
    }
}

/// **The interaction that belonged to neither branch.** A second mechanic
/// emitting into the same suite must fall into exactly one of three states, and
/// only two of them are correct: the claim covers its bodies, the claim
/// excludes them, or the claim silently ignores them. `open-way` (dsl 0.12) is
/// the worked case, and it is the second state — it lowers to one `fill` per
/// box of the piece's exported way, INSIDE the beat's own effect bundle, so it
/// writes no `<family>_<id>` function at all and there is nothing for a claim to
/// discharge. What its gallery element brings with it is an OBJECTIVE, whose
/// bodies land in families the suite already drives one exemplar of.
///
/// So the tree here is the union shape: a claimed family driven whole, a second
/// feature's body under no per-object family (`beat_`, one member — outside the
/// rule entirely), and an objective family the suite drives one of two members
/// of. The claim must stay bound at exactly its own bodies and report no
/// breach, and the sibling the claim does not reach must be named by the byte
/// read instead of vanishing between the two gates.
#[test]
fn a_coexisting_mechanic_is_covered_or_excluded_but_never_ignored() {
    let out = tree(
        &[
            // The claimed mechanic: every declared gate's body.
            "tgate_open_side_door",
            "tgate_open_mid_door",
            "tgate_open_inner_door",
            // `open-way`'s shape: the beat body carrying the way's `fill`. One
            // member, so it forms no per-object family and no claim reaches it.
            "beat_repair_the_stair",
            // The objective its gallery element declares, beside a sibling the
            // suite already drives.
            "activate_o_press_the_case",
            "activate_o_climb_the_loft",
        ],
        &[
            (
                "souls_timed_gate_side_door",
                "function g:tgate_open_side_door\n",
            ),
            (
                "souls_timed_gate_mid_door",
                "function g:tgate_open_mid_door\n",
            ),
            (
                "souls_timed_gate_inner_door",
                "function g:tgate_open_inner_door\n",
            ),
            ("verb_press", "function g:activate_o_press_the_case\n"),
        ],
    );

    // The claim's own binding does not move because a second mechanic emitted
    // beside it: three bodies written for three declared gates, three driven.
    let claims = [claim(
        &["tgate_open_", "tgate_close_", "tgate_disarm_"],
        &["side_door", "mid_door", "inner_door"],
    )];
    let (binding, breaches) = watch::check_claims(NS, &out, &claims);
    assert_eq!(binding.declared_objects, 3);
    assert_eq!(
        binding.bodies_judged, 3,
        "only the bodies that were written"
    );
    assert_eq!(binding.bodies_watched, 3);
    assert!(
        breaches.is_empty(),
        "a coexisting mechanic must not manufacture a breach: {breaches:?}"
    );
    assert!(watch::claim_finding(&binding, &breaches).is_none());

    // And what the claim does not reach, the byte read does. `beat_` is a
    // one-member family and outside the rule; the objective sibling is not.
    let all = ids(&[
        "side_door",
        "mid_door",
        "inner_door",
        "repair_the_stair",
        "press_the_case",
        "climb_the_loft",
    ]);
    let (wb, findings) = watch::check_tree(NS, &out, &all);
    assert_eq!(
        wb.multi_object_families, 2,
        "the gate family and the objective family; the beat body is not one"
    );
    assert_eq!(
        findings.iter().map(|u| &u.function).collect::<Vec<_>>(),
        ["activate_o_climb_the_loft"],
        "the sibling no claim reaches is named by DW0810, not lost between the two gates"
    );
    assert_eq!(findings[0].watched_siblings, ["press_the_case"]);
    assert!(watch::finding(&wb, &findings).is_some());
}
