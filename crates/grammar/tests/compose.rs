//! The include seam: copying one program into another must be a **rename and
//! nothing else**.
//!
//! Everything a zone program claims rests on that. If an include changed what a
//! piece builds, every gate the vocabulary earned in its own fixture would have
//! to be re-earned from scratch inside every zone that used it; because it does
//! not, a zone's gates are about the *composition* and only about that.

use std::collections::BTreeSet;

use delvewright_grammar::compose::{AnchorRenames, ComposeError, entry, include, include_renaming};
use delvewright_grammar::ir::{Node, Program};
use delvewright_grammar::library::{cliff_path, far_side_bar, store_room, watch_bay};
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
/// rule, so the **prefix** never touches it. Moving one is an explicit decision
/// the caller writes down (`include_renaming`, below); an include that was not
/// asked leaves every contract exactly where it was.
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

// ---------------------------------------------------------------------------
// The rename at the include site
// ---------------------------------------------------------------------------

/// The old three-argument [`include`] is the new call with an empty map, and
/// this is the assertion that keeps it that way: same bytes, same anchor names,
/// same positions. Every zone program written before renaming existed is
/// therefore untouched by it.
///
/// Binding: three vocabulary programs × four seeds, compared byte for byte.
#[test]
fn an_empty_rename_map_is_the_old_include_exactly() {
    for (source, region) in [
        (watch_bay(), Box3::at_origin([7, 7, 24])),
        (store_room(), Box3::at_origin([7, 5, 14])),
        (cliff_path(), Box3::at_origin([3, 6, 30])),
    ] {
        let host = Program::new("host", "host").rule("host", Node::call(&entry("in", &source)));
        let plain = include(host.clone(), &source, "in").unwrap();
        let empty = include_renaming(host, &source, "in", &AnchorRenames::new()).unwrap();
        assert_eq!(plain, empty, "{}", source.name);
        for seed in 0..4u64 {
            let a = expand(&plain, region, &ExpandOptions::seeded(seed)).unwrap();
            let b = expand(&empty, region, &ExpandOptions::seeded(seed)).unwrap();
            assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
            assert_eq!(a.anchors, b.anchors);
        }
    }
}

/// A rename moves the named stem and nothing else — and it moves the *stem*, so
/// an indexed mark keeps its numbering under the new name. `cliff_path` is the
/// fixture because it declares two stems, one of them indexed: renaming
/// `niche-watch` must leave `niche-<i>` alone, which a substring-minded
/// implementation would get wrong.
///
/// Binding: 6 anchors (3 recesses, 3 watch cells), of which 3 move.
#[test]
fn a_rename_moves_one_stem_and_leaves_the_rest_alone() {
    let source = cliff_path();
    let region = Box3::at_origin([3, 6, 30]);
    let host = Program::new("host", "host").rule("host", Node::call(&entry("in", &source)));
    let renames = AnchorRenames::from([("niche-watch", "road-watch")]);
    let composed = include_renaming(host, &source, "in", &renames).unwrap();
    composed.validate().expect("every reference resolves");

    let alone = expand(&source, region, &ExpandOptions::seeded(4)).unwrap();
    let out = expand(&composed, region, &ExpandOptions::seeded(4)).unwrap();
    assert_eq!(alone.anchors.len(), 6, "the fixture's anchors");
    assert_eq!(out.anchors.len(), alone.anchors.len(), "no anchor was lost");
    assert_eq!(
        alone.model.canonical_bytes(),
        out.model.canonical_bytes(),
        "a rename moved a block"
    );

    let names: BTreeSet<&str> = out.anchors.keys().map(String::as_str).collect();
    for i in 1..=3 {
        let moved = format!("anchor/road-watch-{i}");
        let stayed = format!("anchor/niche-{i}");
        assert!(names.contains(moved.as_str()), "{moved} missing: {names:?}");
        assert!(
            names.contains(stayed.as_str()),
            "{stayed} missing: {names:?}"
        );
        assert!(
            !names.contains(format!("anchor/niche-watch-{i}").as_str()),
            "the old name survived: {names:?}"
        );
        // The renamed anchor is the *same place*, numbered the same way.
        assert_eq!(
            out.anchors[&moved].pos,
            alone.anchors[&format!("anchor/niche-watch-{i}")].pos
        );
    }
}

/// A rename that matches nothing is refused, because the one thing worse than
/// no rename is a rename that looks like one. Without this, a misspelled stem
/// leaves the collision it was written to fix exactly where it was, and the
/// composition fails several steps later naming a rule the caller never wrote.
///
/// Binding: 4 refusals — a stem the source does not declare, a target that is
/// not a kebab stem, a target the destination already carries, and a target
/// another anchor of the same source is keeping.
#[test]
fn a_rename_that_would_do_nothing_or_collide_is_refused() {
    let bay = watch_bay();

    // A stem `watch_bay` does not declare.
    let host = Program::new("host", "host");
    match include_renaming(host, &bay, "in", &AnchorRenames::from([("wath", "look")])) {
        Err(ComposeError::UnknownAnchor {
            anchor,
            program,
            declares,
        }) => {
            assert_eq!(anchor, "wath");
            assert_eq!(program, "watch_bay");
            assert_eq!(
                declares,
                ["gate", "watch"],
                "the message lists the real ones"
            );
        }
        other => panic!("{other:?}"),
    }

    // A target the DSL could never name.
    let host = Program::new("host", "host");
    match include_renaming(
        host,
        &bay,
        "in",
        &AnchorRenames::from([("watch", "Look_Out")]),
    ) {
        Err(ComposeError::BadAnchorName { anchor, target }) => {
            assert_eq!((anchor.as_str(), target.as_str()), ("watch", "Look_Out"));
        }
        other => panic!("{other:?}"),
    }

    // A target the destination already carries, from a piece included earlier.
    let once = include(Program::new("host", "host"), &bay, "first").unwrap();
    match include_renaming(
        once,
        &far_side_bar(),
        "second",
        &AnchorRenames::from([("unlock", "watch")]),
    ) {
        Err(ComposeError::AnchorTaken { target, holder, .. }) => {
            assert_eq!(target, "watch");
            assert!(holder.contains("host"), "{holder}");
        }
        other => panic!("{other:?}"),
    }

    // A target this same source is keeping for another of its own anchors.
    let host = Program::new("host", "host");
    match include_renaming(host, &bay, "in", &AnchorRenames::from([("watch", "gate")])) {
        Err(ComposeError::AnchorTaken { target, holder, .. }) => {
            assert_eq!(target, "gate");
            assert!(holder.contains("watch_bay"), "{holder}");
        }
        other => panic!("{other:?}"),
    }
}
