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
