//! Composing one program **document** into another: the surface a creator has.
//!
//! `tests/compose.rs` pins what a composition *means*, over the Rust API. This
//! file pins that the document surface reaches exactly that meaning and adds
//! nothing of its own — the same program, the same bytes, the same anchors, the
//! same refusals — plus the things only a file can be wrong about (a path, a
//! cycle, a prefix used twice), and the **bound** on the seam promise, which is
//! a real limit and is demonstrated rather than assumed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_grammar::compose::{self, ComposeError, entry};
use delvewright_grammar::document::{self, DocumentError};
use delvewright_grammar::geom::Axis;
use delvewright_grammar::ir::{
    Alternative, Expr, Include, Node, Program, ProgramError, Reorient, Rounding, Size, Split,
};
use delvewright_grammar::library::{cliff_path, store_room, watch_bay};
use delvewright_grammar::version::{INCLUDE_SINCE, LOCAL_FRAME_SINCE};
use delvewright_grammar::{Box3, ExpandOptions, expand};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-doc-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, program: &Program) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, serde_json::to_vec_pretty(program).unwrap()).unwrap();
    path
}

/// Write raw JSON, for the documents a `Program` value cannot express — an
/// absolute path, a `\` separator, a version below the fence.
fn write_raw(dir: &Path, name: &str, json: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, json).unwrap();
    path
}

fn split(axis: Axis, sizes: Vec<Size>, children: Vec<Node>) -> Node {
    Node::Split(Split {
        axis,
        sizes,
        rounding: Rounding::Truncate,
        repeat: false,
        orient: Reorient::KEEP,
        children,
    })
}

/// A host that calls one composed piece over its whole box and draws nothing of
/// its own before it — the shape the seam promise is stated for.
fn host_calling(prefix: &str, source: &Program) -> Program {
    Program::new("host", "host")
        .rule("host", Node::call(&entry(prefix, source)))
        .including("piece.json", prefix)
}

/// The two-zone map every test below builds on: two documents, two boxes, one
/// derivation.
fn two_zone_map() -> Program {
    Program::new("two-zone-map", "map")
        .param("head", 4)
        .including("watch-bay.json", "z0")
        .including("store-room.json", "z1")
        .rule(
            "map",
            split(
                Axis::Z,
                vec![Size::abs(24), Size::abs(14)],
                vec![
                    // The whole-map datum, pushed into the piece with the
                    // general mechanism that already exists for it — a `bind`
                    // around the call. The include site has no binding surface
                    // of its own, on purpose.
                    Node::call(&entry("z0", &watch_bay()))
                        .with_params([("z0/head", Expr::param("head"))]),
                    Node::call(&entry("z1", &store_room())),
                ],
            ),
        )
}

// ---------------------------------------------------------------------------
// The claim: the document surface reaches the Rust composition exactly
// ---------------------------------------------------------------------------

/// **spec-0040 AC7, first half**: "document-level include reproduces the
/// Rust-composed bytes for the three programs the existing seam test pins, from
/// JSON alone".
///
/// Those three are `watch_bay`, `store_room` and `cliff_path` — the ones
/// `tests/compose.rs` pins, one of which places its tell by a recursion and one
/// of which draws its variants from the seed. Composed from JSON alone they
/// produce the program the Rust call produces, and therefore the same bytes and
/// the same anchors.
///
/// Binding: three source programs × four seeds, each compared on three
/// independent things (the resolved `Program` value, the expanded model's
/// canonical bytes, the anchor names).
#[test]
fn a_document_include_reproduces_the_rust_composition() {
    for (source, region) in [
        (watch_bay(), Box3::at_origin([7, 7, 24])),
        (store_room(), Box3::at_origin([7, 5, 14])),
        (cliff_path(), Box3::at_origin([3, 6, 30])),
    ] {
        let dir = scratch(&format!("reproduces-{}", source.name));
        write(&dir, "piece.json", &source);
        let map = write(&dir, "map.json", &host_calling("in", &source));

        let loaded = document::load(&map).unwrap_or_else(|e| panic!("{}: {e}", source.name));
        assert_eq!(loaded.includes(), 1, "{}: binding count", source.name);
        assert_eq!(loaded.compositions[0].prefix, "in");
        assert_eq!(loaded.compositions[0].name, source.name);
        assert_eq!(loaded.compositions[0].depth, 0);

        // The resolved document IS the Rust composition — the same value, not a
        // value that merely builds the same thing. Anything the document
        // surface quietly added would show up here rather than being averaged
        // out by an expansion.
        let rust = compose::include(
            Program::new("host", "host").rule("host", Node::call(&entry("in", &source))),
            &source,
            "in",
        )
        .unwrap();
        assert_eq!(
            loaded.program, rust,
            "{}: the document composition is not the Rust composition",
            source.name
        );
        loaded.program.validate().unwrap();
        assert!(
            loaded.program.include.is_empty(),
            "resolution must consume the include list"
        );

        for seed in 0..4u64 {
            let alone = expand(&source, region, &ExpandOptions::seeded(seed)).unwrap();
            let inside = expand(&loaded.program, region, &ExpandOptions::seeded(seed)).unwrap();
            assert_eq!(
                alone.model.canonical_bytes(),
                inside.model.canonical_bytes(),
                "{} seed {seed}: composing from JSON changed what the piece builds",
                source.name
            );
            assert_eq!(
                alone.anchors.keys().collect::<Vec<_>>(),
                inside.anchors.keys().collect::<Vec<_>>(),
                "{} seed {seed}: composing from JSON renamed its anchors",
                source.name
            );
        }
    }
}

/// A map is not one piece under a host: it is **two** pieces in boxes the map
/// allocated, with a whole-map datum pushed into one of them.
///
/// The shape spec-0040 §4 requires of "the whole owes a part: the datums, bound
/// once" — one map-level `param`, stated in exactly one place, pushed down by
/// `bind`. Its worked example is a single `water_y` bound into every wet zone;
/// this is that arrangement at two zones, which is the smallest size at which
/// "one place" is a claim about anything.
#[test]
fn two_documents_compose_into_one_map_and_the_map_binds_a_datum() {
    let dir = scratch("two-zone-map");
    write(&dir, "watch-bay.json", &watch_bay());
    write(&dir, "store-room.json", &store_room());
    let path = write(&dir, "map.json", &two_zone_map());

    let loaded = document::load(&path).unwrap();
    assert_eq!(loaded.includes(), 2, "binding count");
    loaded.program.validate().unwrap();

    // Both vocabularies arrived under their own prefixes, and nothing arrived
    // bare: a bare name would mean one piece could silently rebind the other's.
    assert!(loaded.program.params.contains_key("z0/head"));
    assert!(
        loaded
            .program
            .params
            .keys()
            .all(|k| k == "head" || k.starts_with("z0/")),
        "an unprefixed name arrived: {:?}",
        loaded.program.params.keys().collect::<Vec<_>>()
    );
    for prefix in ["z0/", "z1/"] {
        assert!(
            loaded.program.rules.keys().any(|k| k.starts_with(prefix)),
            "{prefix} brought no rules: {:?}",
            loaded.program.rules.keys().collect::<Vec<_>>()
        );
    }
    assert!(
        loaded
            .program
            .rules
            .keys()
            .all(|k| k == "map" || k.starts_with("z0/") || k.starts_with("z1/")),
        "an unprefixed rule arrived: {:?}",
        loaded.program.rules.keys().collect::<Vec<_>>()
    );

    let out = expand(
        &loaded.program,
        Box3::at_origin([7, 7, 38]),
        &ExpandOptions::seeded(1),
    )
    .expect("the map expands over the box its splits allocate");
    assert!(!out.model.canonical_bytes().is_empty());

    // Both pieces' anchors are in the composition, under the names their own
    // programs gave them — the prefix never touches an anchor.
    let names: Vec<&str> = out.anchors.keys().map(String::as_str).collect();
    assert!(names.contains(&"anchor/watch"), "{names:?}");
    assert!(names.contains(&"anchor/tell"), "{names:?}");
}

/// Determinism (ADR-0006) for a composed document: the same documents and the
/// same seed produce byte-identical output, across two independent loads.
///
/// Two loads and not one, because resolution is the new step: a loader that
/// iterated a `HashMap` or read a directory would produce the same program from
/// one call and a different one from the next.
#[test]
fn a_composed_document_double_expands_byte_identically() {
    let dir = scratch("determinism");
    write(&dir, "watch-bay.json", &watch_bay());
    write(&dir, "store-room.json", &store_room());
    let path = write(&dir, "map.json", &two_zone_map());

    let first = document::load(&path).unwrap();
    let second = document::load(&path).unwrap();
    assert_eq!(
        serde_json::to_vec(&first.program).unwrap(),
        serde_json::to_vec(&second.program).unwrap(),
        "two loads of one document resolved to different programs"
    );
    assert_eq!(first.compositions, second.compositions);

    for seed in [0u64, 1, 7] {
        let a = expand(
            &first.program,
            Box3::at_origin([7, 7, 38]),
            &ExpandOptions::seeded(seed),
        )
        .unwrap();
        let b = expand(
            &second.program,
            Box3::at_origin([7, 7, 38]),
            &ExpandOptions::seeded(seed),
        )
        .unwrap();
        assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    }
}

/// The include list's ORDER is the author's and does not reach the composed
/// program: every name a composition carries lands in a `BTreeMap`.
#[test]
fn the_include_order_does_not_reach_the_composed_program() {
    let dir = scratch("order");
    write(&dir, "watch-bay.json", &watch_bay());
    write(&dir, "store-room.json", &store_room());
    let forward = write(&dir, "forward.json", &two_zone_map());

    let mut swapped = two_zone_map();
    swapped.include.reverse();
    let reverse = write(&dir, "reverse.json", &swapped);

    assert_eq!(
        document::load(&forward).unwrap().program,
        document::load(&reverse).unwrap().program
    );
}

// ---------------------------------------------------------------------------
// The seam's bound — demonstrated, never assumed
// ---------------------------------------------------------------------------

/// **The bound on the byte-identity promise.** A composed piece keeps its
/// standalone bytes only when nothing drew from the stream before it.
///
/// spec-0040 §1.4 established this by probe — the seeded stream is one
/// sequential splitmix64 consumed in traversal order, so two programs identical
/// except that an earlier sibling draws produce different bytes inside the same
/// called piece — and §5 draws the consequence the reference text has to carry:
/// texture is composition-relative, so the review that certifies a composed
/// piece's appearance is the composed one.
///
/// A sibling that draws first shifts every draw the piece makes. This asserts
/// **both** halves against the same source program: the host that draws nothing
/// reproduces the piece exactly, and the host that draws one coin — a coin whose
/// two outcomes are geometrically identical, so nothing but the stream position
/// changed — does not.
///
/// Written this way because the reference text's byte-identity claim is bounded,
/// and a test that assumed the unbounded form would be asserting something the
/// engine does not promise.
#[test]
fn the_seam_is_byte_identical_only_when_nothing_drew_first() {
    let source = cliff_path();
    let region = Box3::at_origin([3, 6, 30]);
    let dir = scratch("seam-bound");
    write(&dir, "piece.json", &source);

    let quiet = write(&dir, "quiet.json", &host_calling("in", &source));
    let quiet = document::load(&quiet).unwrap().program;

    // A host that draws exactly one coin first. Both outcomes are `void`, so the
    // coin contributes no block either way and the ONLY difference between the
    // two hosts is that the stream advanced by one before the piece was reached.
    let loud = Program::new("host", "host")
        .including("piece.json", "in")
        .rule_alts(
            "coin",
            vec![Alternative::new(Node::Void), Alternative::new(Node::Void)],
        )
        .rule(
            "host",
            split(
                Axis::Y,
                vec![Size::abs(1), Size::rel(1)],
                vec![Node::call("coin"), Node::call(&entry("in", &source))],
            ),
        );
    let loud = write(&dir, "loud.json", &loud);
    let loud = document::load(&loud).unwrap().program;

    let mut shifted = 0;
    for seed in 0..4u64 {
        let alone = expand(&source, region, &ExpandOptions::seeded(seed))
            .unwrap()
            .model
            .canonical_bytes();
        let quiet_bytes = expand(&quiet, region, &ExpandOptions::seeded(seed))
            .unwrap()
            .model
            .canonical_bytes();
        assert_eq!(
            alone, quiet_bytes,
            "seed {seed}: the seam does not hold even with nothing drawn before it"
        );
        // The coin takes the top course, so the piece gets a box of exactly the
        // region it was measured alone over.
        let loud_bytes = expand(
            &loud,
            Box3::at_origin([3, 7, 30]),
            &ExpandOptions::seeded(seed),
        )
        .unwrap()
        .model
        .canonical_bytes();
        if loud_bytes != quiet_bytes {
            shifted += 1;
        }
    }
    println!("seam bound: {shifted} of 4 seeds re-textured once one coin was drawn first");
    assert!(
        shifted > 0,
        "binding count 0: no seed showed the shift, so this fixture is not demonstrating the \
         bound it was written for — the piece may have stopped drawing from the stream"
    );
}

// ---------------------------------------------------------------------------
// The version fence
// ---------------------------------------------------------------------------

/// **spec-0040 AC7, second half**: "a loader below the fenced `Program` version
/// refuses the surface by name".
///
/// By name means the construct, the version that introduced it and the version
/// the document declares — never accepted with the include quietly dropped,
/// which is the silent-wrongness ADR-0018 §7 built the fence against.
#[test]
fn a_document_below_the_fence_refuses_the_include_by_name() {
    let dir = scratch("fence");
    write(&dir, "piece.json", &store_room());
    let below = write_raw(
        &dir,
        "map.json",
        &format!(
            r#"{{
  "version": "{LOCAL_FRAME_SINCE}",
  "name": "map",
  "start": "map",
  "include": [{{ "program": "piece.json", "prefix": "in" }}],
  "rules": {{ "map": [{{ "body": {{ "op": "call", "symbol": "in/stores" }} }}] }}
}}"#
        ),
    );
    let err = document::load(&below).unwrap_err();
    match &err {
        DocumentError::Program { path, detail } => {
            assert!(path.ends_with("map.json"), "{}", path.display());
            match &**detail {
                ProgramError::FencedConstruct {
                    construct,
                    since,
                    declared,
                    ..
                } => {
                    assert_eq!(*construct, "an `include` list");
                    assert_eq!(*since, INCLUDE_SINCE);
                    assert_eq!(declared, LOCAL_FRAME_SINCE);
                }
                other => panic!("expected the include fence, got {other:?}"),
            }
        }
        other => panic!("expected the fence at the document, got {other}"),
    }
    let text = err.to_string();
    assert!(text.contains("include"), "{text}");
    assert!(text.contains(INCLUDE_SINCE), "{text}");
}

/// The fence reaches a document at any depth: a map that composes a zone that is
/// itself below the fence is refused with the *zone's* file named.
#[test]
fn the_fence_reaches_a_composed_document_at_depth() {
    let dir = scratch("fence-deep");
    write(&dir, "piece.json", &store_room());
    write_raw(
        &dir,
        "inner.json",
        &format!(
            r#"{{
  "version": "{LOCAL_FRAME_SINCE}",
  "name": "inner",
  "start": "inner",
  "include": [{{ "program": "piece.json", "prefix": "deep" }}],
  "rules": {{ "inner": [{{ "body": {{ "op": "call", "symbol": "deep/stores" }} }}] }}
}}"#
        ),
    );
    let outer = write(
        &dir,
        "outer.json",
        &Program::new("outer", "outer")
            .including("inner.json", "in")
            .rule("outer", Node::call("in/inner")),
    );
    let err = document::load(&outer).unwrap_err();
    match &err {
        DocumentError::Program { path, detail } => {
            assert!(path.ends_with("inner.json"), "{}", path.display());
            assert!(matches!(**detail, ProgramError::FencedConstruct { .. }));
        }
        other => panic!("expected the inner document's fence, got {other}"),
    }
    assert!(err.to_string().contains(INCLUDE_SINCE), "{err}");
}

/// A program whose includes were never resolved is refused rather than expanded.
///
/// This is the guard on the OTHER path: `serde_json::from_slice::<Program>` is
/// one line and reads a document without resolving it. Left unguarded, the
/// composed vocabulary would simply be absent, and the failure would be an
/// unknown rule named after a prefix — blaming the map for the loader that was
/// skipped.
#[test]
fn an_unresolved_document_never_expands() {
    let json = serde_json::to_vec(
        &Program::new("map", "map")
            .including("piece.json", "in")
            .rule("map", Node::call("in/stores")),
    )
    .unwrap();
    let program: Program = serde_json::from_slice(&json).unwrap();
    let err = expand(
        &program,
        Box3::at_origin([7, 5, 14]),
        &ExpandOptions::seeded(0),
    )
    .unwrap_err();
    let text = err.to_string();
    assert!(text.contains("include"), "{text}");
    assert!(text.contains("document::load"), "{text}");
    assert!(matches!(
        program.validate(),
        Err(ProgramError::UnresolvedInclude { count: 1, .. })
    ));
}

/// A composed document's own declared version stays honest: a piece that says
/// `1.0.0` and writes a construct from later is refused where it is written,
/// with its own file named, rather than laundered by a composition into a
/// document whose version does allow it.
#[test]
fn composition_cannot_launder_a_dishonest_version() {
    let dir = scratch("honest-version");
    // A piece that writes a `bind` (introduced at 1.3.0) and declares 1.0.0.
    // Alone it is refused; composed into a 1.5.0 map, the composition's own
    // version would allow the construct, so only the piece's own check can
    // notice.
    let piece = Program::new("piece", "piece")
        .at_version("1.0.0")
        .param("h", 2)
        .role("stone", "minecraft:stone".parse().unwrap())
        .rule(
            "piece",
            Node::call("floor").with_params([("h", Expr::int(1))]),
        )
        .rule("floor", Node::fill("stone"));
    write(&dir, "piece.json", &piece);
    let map = write(
        &dir,
        "map.json",
        &Program::new("map", "map")
            .including("piece.json", "in")
            .rule("map", Node::call("in/piece")),
    );
    match document::load(&map).unwrap_err() {
        DocumentError::Program { path, detail } => {
            assert!(path.ends_with("piece.json"), "{}", path.display());
            assert!(
                matches!(*detail, ProgramError::FencedConstruct { .. }),
                "{detail}"
            );
        }
        other => panic!("expected the piece's own fence, got {other}"),
    }
}

// ---------------------------------------------------------------------------
// What only a file can be wrong about
// ---------------------------------------------------------------------------

/// Every refusal that belongs to the document surface, each demonstrated on the
/// document that provokes it.
///
/// One table rather than eight tests, because the property is the surface's: a
/// document that cannot be resolved says which document, which prefix, and why.
#[test]
fn the_document_surface_refuses_what_only_a_file_can_be_wrong_about() {
    let dir = scratch("refusals");
    write(&dir, "piece.json", &store_room());

    let doc = |name: &str, program: &str| {
        write_raw(
            &dir,
            name,
            &format!(
                r#"{{
  "version": "{INCLUDE_SINCE}",
  "name": "map",
  "start": "map",
  "include": [{{ "program": {program}, "prefix": "in" }}],
  "rules": {{ "map": [{{ "body": {{ "op": "call", "symbol": "in/stores" }} }}] }}
}}"#
            ),
        )
    };

    let abs = doc("abs.json", "\"/etc/piece.json\"");
    match document::load(&abs).unwrap_err() {
        DocumentError::UnusablePath { why, .. } => assert!(why.contains("absolute"), "{why}"),
        other => panic!("{other}"),
    }
    let back = doc("back.json", r#""pieces\\piece.json""#);
    match document::load(&back).unwrap_err() {
        DocumentError::UnusablePath { why, .. } => assert!(why.contains('\\'), "{why}"),
        other => panic!("{other}"),
    }
    let empty = doc("empty.json", "\"\"");
    assert!(matches!(
        document::load(&empty).unwrap_err(),
        DocumentError::UnusablePath { .. }
    ));
    // Missing — the plain case, and the one a creator meets most.
    let missing = doc("missing.json", "\"no-such-piece.json\"");
    match document::load(&missing).unwrap_err() {
        DocumentError::Read { path, asked_by, .. } => {
            assert!(path.ends_with("no-such-piece.json"));
            assert!(asked_by.contains("missing.json"), "{asked_by}");
            assert!(asked_by.contains("in"), "{asked_by}");
        }
        other => panic!("{other}"),
    }
    // Not a program document at all.
    write_raw(&dir, "prose.json", "{\"hello\": 1}");
    let prose = doc("prose-map.json", "\"prose.json\"");
    assert!(matches!(
        document::load(&prose).unwrap_err(),
        DocumentError::Parse { .. }
    ));

    // One prefix, twice.
    let twice = write(
        &dir,
        "twice.json",
        &Program::new("map", "map")
            .including("piece.json", "in")
            .including("piece.json", "in")
            .rule("map", Node::call("in/stores")),
    );
    match document::load(&twice).unwrap_err() {
        DocumentError::PrefixTwice { prefix, .. } => assert_eq!(prefix, "in"),
        other => panic!("{other}"),
    }

    // A prefix `compose` itself cannot use.
    let slash = write(
        &dir,
        "slash.json",
        &Program::new("map", "map")
            .including("piece.json", "a/b")
            .rule("map", Node::call("a/b/stores")),
    );
    match document::load(&slash).unwrap_err() {
        DocumentError::Compose { detail, .. } => {
            assert!(
                matches!(*detail, ComposeError::BadPrefix { .. }),
                "{detail}"
            )
        }
        other => panic!("{other}"),
    }

    // A cycle, direct and indirect.
    let a = write(
        &dir,
        "a.json",
        &Program::new("a", "a")
            .including("a.json", "self")
            .rule("a", Node::call("self/a")),
    );
    match document::load(&a).unwrap_err() {
        DocumentError::Cycle { chain } => assert_eq!(chain.len(), 2, "{chain:?}"),
        other => panic!("{other}"),
    }
    write(
        &dir,
        "b.json",
        &Program::new("b", "b")
            .including("c.json", "c")
            .rule("b", Node::call("c/c")),
    );
    write(
        &dir,
        "c.json",
        &Program::new("c", "c")
            .including("b.json", "b")
            .rule("c", Node::call("b/b")),
    );
    match document::load(&dir.join("b.json")).unwrap_err() {
        DocumentError::Cycle { chain } => assert_eq!(chain.len(), 3, "{chain:?}"),
        other => panic!("{other}"),
    }

    // A clash is `compose`'s refusal, reported with the two documents named.
    let clash = write(
        &dir,
        "clash.json",
        &Program::new("map", "map")
            .including("piece.json", "in")
            .rule("in/stores", Node::Void)
            .rule("map", Node::call("in/stores")),
    );
    match document::load(&clash).unwrap_err() {
        DocumentError::Compose { detail, prefix, .. } => {
            assert_eq!(prefix, "in");
            assert!(matches!(*detail, ComposeError::Clash { .. }));
        }
        other => panic!("{other}"),
    }
}

/// The anchor rename is the document form of `include_renaming`, with the same
/// refusals — including the typo guard, which is the one a document surface
/// would otherwise lose.
///
/// spec-0040 §4 puts the rename here deliberately, as something a part owes the
/// whole: renameable anchor stems, "so two parts declaring one stem are the
/// include site's explicit decision, never a silent union".
#[test]
fn the_document_renames_anchors_and_refuses_a_rename_that_matches_nothing() {
    let dir = scratch("renames");
    write(&dir, "piece.json", &store_room());

    let mut renamed = Program::new("map", "map")
        .including("piece.json", "in")
        .rule("map", Node::call(&entry("in", &store_room())));
    renamed.include[0].rename_anchors =
        BTreeMap::from([("tell".to_string(), "map-tell".to_string())]);
    let path = write(&dir, "map.json", &renamed);

    let out = expand(
        &document::load(&path).unwrap().program,
        Box3::at_origin([7, 5, 14]),
        &ExpandOptions::seeded(1),
    )
    .unwrap();
    let names: Vec<&str> = out.anchors.keys().map(String::as_str).collect();
    assert!(names.contains(&"anchor/map-tell"), "{names:?}");
    assert!(!names.contains(&"anchor/tell"), "{names:?}");

    // …and a rename of a stem the piece never declares is refused, rather than
    // leaving the name it was written to move exactly where it was.
    let mut typo = renamed.clone();
    typo.include[0].rename_anchors = BTreeMap::from([("tel".to_string(), "map-tell".to_string())]);
    let path = write(&dir, "typo.json", &typo);
    match document::load(&path).unwrap_err() {
        DocumentError::Compose { detail, .. } => {
            assert!(
                matches!(*detail, ComposeError::UnknownAnchor { .. }),
                "{detail}"
            );
        }
        other => panic!("{other}"),
    }
}

/// An include composes a document that itself composes one: the depth a map
/// actually has, and the binding count that reports every level of it.
#[test]
fn composition_nests_and_the_binding_count_reports_every_depth() {
    let dir = scratch("nested");
    write(&dir, "piece.json", &store_room());
    write(
        &dir,
        "zone.json",
        &Program::new("zone", "zone")
            .including("piece.json", "p")
            .rule("zone", Node::call(&entry("p", &store_room()))),
    );
    let map = write(
        &dir,
        "map.json",
        &Program::new("map", "map")
            .including("zone.json", "z")
            .rule("map", Node::call("z/zone")),
    );
    let loaded = document::load(&map).unwrap();
    assert_eq!(loaded.includes(), 2, "{:?}", loaded.compositions);
    // Innermost first, and each carries the depth it sits at.
    assert_eq!(loaded.compositions[0].prefix, "p");
    assert_eq!(loaded.compositions[0].depth, 1);
    assert_eq!(loaded.compositions[1].prefix, "z");
    assert_eq!(loaded.compositions[1].depth, 0);
    loaded.program.validate().unwrap();
    // The piece's rules arrived under BOTH prefixes, in that order.
    assert!(loaded.program.rules.keys().any(|k| k.starts_with("z/p/")));
}

/// Two spellings of one path are one document, so `./a.json` cannot slip past
/// the cycle check that `a.json` is caught by.
#[test]
fn a_path_spelled_two_ways_is_one_document() {
    let dir = scratch("spelling");
    write(
        &dir,
        "a.json",
        &Program::new("a", "a")
            .including("./a.json", "self")
            .rule("a", Node::call("self/a")),
    );
    assert!(matches!(
        document::load(&dir.join("a.json")).unwrap_err(),
        DocumentError::Cycle { .. }
    ));
    assert_eq!(
        document::normalised_path(Path::new("x/./y/../z.json")),
        PathBuf::from("x/z.json")
    );
}

/// The `Include` type round-trips through JSON, and an unknown key in it is
/// refused rather than dropped — the closed-schema rule, at the new type.
#[test]
fn an_include_is_a_closed_schema() {
    let include = Include {
        program: "piece.json".to_string(),
        prefix: "in".to_string(),
        rename_anchors: BTreeMap::from([("a".to_string(), "b".to_string())]),
    };
    let json = serde_json::to_string(&include).unwrap();
    assert_eq!(serde_json::from_str::<Include>(&json).unwrap(), include);
    let err = serde_json::from_str::<Include>(
        r#"{"program":"p.json","prefix":"in","bind_params":{"a":1}}"#,
    )
    .unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{err}");
}
