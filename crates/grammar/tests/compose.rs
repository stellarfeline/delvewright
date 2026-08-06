//! The include seam: copying one program into another must be a **rename and
//! nothing else**.
//!
//! Everything a zone program claims rests on that. If an include changed what a
//! piece builds, every gate the vocabulary earned in its own fixture would have
//! to be re-earned from scratch inside every zone that used it; because it does
//! not, a zone's gates are about the *composition* and only about that.

use std::collections::BTreeSet;

use delvewright_grammar::compose::{ComposeError, entry, include};
use delvewright_grammar::ir::{Node, Program};
use delvewright_grammar::library::{cliff_path, store_room, watch_bay};
use delvewright_grammar::{Box3, ExpandOptions, expand};

/// The claim in its strongest form: a program included under a prefix and
/// called over the whole region builds exactly what the original builds there —
/// the same bytes and the same anchors.
///
/// Binding: three vocabulary programs, one of which (`store_room`) places its
/// tell by a *recursion* and one of which (`cliff_path`) draws its variants from
/// the seed, over four seeds each.
#[test]
fn an_included_program_expands_to_exactly_what_it_did_alone() {
    for (source, region) in [
        (watch_bay(), Box3::at_origin([7, 7, 24])),
        (store_room(), Box3::at_origin([7, 5, 14])),
        (cliff_path(), Box3::at_origin([3, 6, 30])),
    ] {
        let host = Program::new("host", "host").rule("host", Node::call(&entry("in", &source)));
        let composed = include(host, &source, "in").unwrap();
        composed
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", source.name));
        for seed in 0..4u64 {
            let alone = expand(&source, region, &ExpandOptions::seeded(seed)).unwrap();
            let inside = expand(&composed, region, &ExpandOptions::seeded(seed)).unwrap();
            assert_eq!(
                alone.model.canonical_bytes(),
                inside.model.canonical_bytes(),
                "{} seed {seed}: including the program changed what it builds",
                source.name
            );
            assert_eq!(
                alone.anchors.keys().collect::<Vec<_>>(),
                inside.anchors.keys().collect::<Vec<_>>(),
                "{} seed {seed}: including the program renamed its anchors",
                source.name
            );
            for (name, before) in &alone.anchors {
                let after = &inside.anchors[name];
                assert_eq!(
                    (before.pos, before.facing),
                    (after.pos, after.facing),
                    "{} seed {seed}: including the program moved {name}",
                    source.name
                );
                // The one thing that *does* change is the provenance: an anchor
                // records the rule that declared it, and that rule now answers
                // to its qualified name. A composed prefab's anchors say which
                // included piece they came from, which is the right answer.
                assert_eq!(
                    after.declared_by,
                    format!("in/{}", before.declared_by),
                    "{} seed {seed}: {name}'s declaring rule",
                    source.name
                );
            }
        }
    }
}

/// Nothing of the source keeps its old name, and everything it referred to
/// still resolves — including a rule's calls to *itself*, which is how
/// `store_room` keeps its "exactly one tell" invariant.
#[test]
fn every_name_is_qualified_and_no_reference_is_left_behind() {
    let source = store_room();
    let composed = include(Program::new("host", "in/stores"), &source, "in").unwrap();

    for name in source.rules.keys() {
        assert!(
            composed.rules.contains_key(&format!("in/{name}")),
            "the rule {name:?} did not arrive"
        );
        assert!(
            !composed.rules.contains_key(name),
            "the rule {name:?} arrived unqualified"
        );
    }
    for role in source.palette.keys() {
        assert!(composed.palette.contains_key(&format!("in/{role}")));
        assert!(!composed.palette.contains_key(role));
    }

    // The self-call is the one a naive rewrite misses: `line_before_tell` is
    // recursive, and a body still naming the bare symbol would be an
    // `UnknownRule` here rather than a model with two tells in it.
    let json = serde_json::to_string(&composed).unwrap();
    assert!(json.contains("in/line_before_tell"));
    assert!(!json.contains("\"line_before_tell\""));
    composed.validate().expect("every reference resolves");
}

/// An anchor is the campaign's name for a place, not the program's name for a
/// rule, so it is the one thing an include leaves alone. (What that costs is in
/// `tests/zones.rs`: two copies of one piece collide, loudly.)
#[test]
fn an_include_does_not_rename_anchors() {
    let composed = include(Program::new("host", "in/gate_passage"), &watch_bay(), "in").unwrap();
    let out = expand(
        &composed,
        Box3::at_origin([7, 7, 24]),
        &ExpandOptions::seeded(1),
    )
    .unwrap();
    let names: BTreeSet<&str> = out.anchors.keys().map(String::as_str).collect();
    assert!(names.contains("anchor/watch"), "{names:?}");
    assert!(names.contains("anchor/gate"), "{names:?}");
    assert!(
        names.iter().all(|n| !n.contains("in/")),
        "an include qualified an anchor name: {names:?}"
    );
}

/// A parameter of an included piece is still a control, under its qualified
/// name — and an unqualified `set_param` is refused, so a knob that quietly
/// stopped reaching the geometry cannot pass for one that was never turned.
#[test]
fn an_included_parameter_is_still_a_control() {
    let source = watch_bay();
    let host = Program::new("host", "host").rule("host", Node::call(&entry("in", &source)));
    let mut composed = include(host, &source, "in").unwrap();
    let region = Box3::at_origin([7, 7, 24]);
    let plain = expand(&composed, region, &ExpandOptions::seeded(1)).unwrap();

    assert!(composed.set_param("obstruct", 1).is_err());
    composed.set_param("in/obstruct", 1).unwrap();
    let knocked = expand(&composed, region, &ExpandOptions::seeded(1)).unwrap();
    assert_ne!(
        plain.model.canonical_bytes(),
        knocked.model.canonical_bytes(),
        "the qualified parameter did not reach the geometry"
    );
}

/// A composition that would redefine a name is refused, not merged: silently
/// letting the second definition win is how a zone would end up expanding a
/// piece nobody wrote.
#[test]
fn a_name_that_would_be_redefined_is_refused() {
    let bay = watch_bay();
    let once = include(Program::new("host", "in/gate_passage"), &bay, "in").unwrap();
    match include(once, &bay, "in") {
        Err(ComposeError::Clash { kind, name }) => {
            assert_eq!(kind, "parameter");
            assert!(name.starts_with("in/"), "{name}");
        }
        Err(other) => panic!("wrong error: {other}"),
        Ok(_) => panic!("including the same piece under the same prefix was allowed"),
    }
}

/// A prefix that could not separate the two halves of a qualified name is
/// refused before anything is copied.
#[test]
fn an_unusable_prefix_is_refused() {
    let bay = watch_bay();
    for prefix in ["", "in/ner"] {
        match include(Program::new("host", "host"), &bay, prefix) {
            Err(ComposeError::BadPrefix { prefix: got }) => assert_eq!(got, prefix),
            other => panic!("prefix {prefix:?}: {other:?}"),
        }
    }
}
