//! The spatial contract's **declaration** surface: what a program may say about
//! where a body goes, and the three properties that have to hold before anything
//! is ever built on top of it.
//!
//! Nothing here judges geometry. A claim is recorded, resolved and exported; no
//! obligation reads it yet. What is proved here is that the surface can carry a
//! contract without changing anything else, which is the property that is
//! expensive to discover is false later:
//!
//! 1. **Transparency.** Declaring spaces moves no blocks — asserted over the
//!    whole library by wrapping every rule of every program, not on one
//!    hand-picked example.
//! 2. **Determinism.** Two expansions in two processes write the same `.nbt` and
//!    the same metadata, contract included (ADR-0006).
//! 3. **Resolution.** What lands in metadata is the contract *as resolved for
//!    that expansion*, so re-parameterising a program re-parameterises its
//!    contract instead of leaving a stale description of a box that no longer
//!    exists.
//!
//! Plus the fence: an older document keeps compiling to the same bytes and
//! cannot write the new surface.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_grammar::export::{export_prefab, export_zone};
use delvewright_grammar::ir::{
    Alternative, Bar, Contract, EXTERIOR, EdgeClass, Envelope, Mark, MarkAt, Node, Opens, Program,
    ProgramError, Reorient, Rounding, Size, Split, Way,
};
use delvewright_grammar::library::{self, spatial_contract::spatial_contract};
use delvewright_grammar::version::{LATEST_PROGRAM_VERSION, WAY_SINCE};
use delvewright_grammar::{Axis, Box3, ExpandOptions, Expansion, expand};

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

/// The region `spatial-contract` is documented at.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

fn run(program: &Program, region: Box3, seed: u64) -> Expansion {
    expand(program, region, &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

// ---------------------------------------------------------------------------
// 1. Transparency, over the whole corpus
// ---------------------------------------------------------------------------

/// A region every library program expands in, with the id it belongs to.
///
/// Kept here rather than shared with `tests/library.rs` because that table
/// covers the buildings and the staging vocabulary and deliberately not the
/// idiom index; the claim being made below is about **every** program an author
/// can reach, so the table has to be total, and a partial one borrowed from
/// elsewhere would look total.
const CORPUS: &[(&str, [u32; 3])] = &[
    ("ambush-door", [11, 5, 13]),
    ("bait-stand", [9, 8, 14]),
    ("boulder-stair", [9, 6, 27]),
    ("broken-grate", [3, 5, 14]),
    ("castle", [41, 14, 25]),
    ("causeway", [7, 10, 9]),
    ("church", [15, 16, 30]),
    ("cliff-path", [3, 6, 30]),
    ("decorated-room", [11, 4, 11]),
    ("disarm-stand", [9, 7, 16]),
    ("drop-shaft", [4, 8, 6]),
    ("dumbwaiter", [6, 8, 8]),
    ("elite-ground", [19, 5, 25]),
    ("far-side-bar", [5, 5, 7]),
    ("hearth-ward", [8, 6, 14]),
    ("idiom-arguments", [15, 7, 15]),
    ("idiom-composition-arcade", [3, 14, 20]),
    ("idiom-erosion", [9, 5, 3]),
    ("idiom-erosion-graded", [9, 13, 3]),
    ("idiom-light", [5, 6, 13]),
    ("idiom-mirror", [15, 11, 2]),
    ("idiom-priority", [13, 6, 2]),
    ("idiom-repetition", [3, 5, 17]),
    ("idiom-shape", [15, 9, 3]),
    ("idiom-skip", [7, 5, 5]),
    ("idiom-surface-detail", [9, 12, 9]),
    ("lift-shaft", [5, 16, 7]),
    ("negated-guard", [5, 4, 12]),
    ("rafter-hall", [13, 6, 25]),
    ("spatial-contract", [11, 6, 15]),
    ("stair-flight", [5, 14, 22]),
    ("store-room", [7, 5, 14]),
    ("tee-passage", [5, 5, 12]),
    ("temple", [13, 14, 21]),
    ("threshold-motif", [9, 6, 13]),
    ("watch-bay", [7, 7, 24]),
];

/// The table above is **total**, in both directions.
///
/// Without this, a program added to the library would simply not be covered by
/// the transparency claim below, and the claim would read as if it were —
/// exactly the unbound vacuity mode that makes a green mean nothing.
#[test]
fn the_transparency_corpus_covers_every_library_program() {
    let listed: BTreeSet<&str> = CORPUS.iter().map(|(id, _)| *id).collect();
    let registered: BTreeSet<&str> = library::PROGRAMS.iter().map(|p| p.id).collect();
    assert_eq!(listed, registered);
    assert_eq!(listed.len(), CORPUS.len(), "the table repeats an id");
}

/// Wrap every alternative of every rule in a claim, and classify the region.
///
/// A mechanical transformation rather than a hand-written pair: the point is
/// that declarations are inert *wherever* they are written, and a hand-placed
/// one proves it only where it was placed.
fn claim_everything(mut program: Program) -> Program {
    const PROBE: &str = "probe";
    let rules = std::mem::take(&mut program.rules);
    program.rules = rules
        .into_iter()
        .map(|(symbol, alts)| {
            let wrapped = alts
                .into_iter()
                .map(|alt| Alternative {
                    weight: alt.weight,
                    when: alt.when,
                    body: Node::Claim {
                        region: PROBE.to_string(),
                        body: Box::new(alt.body),
                    },
                })
                .collect();
            (symbol, wrapped)
        })
        .collect();
    let contract = program
        .contract
        .take()
        .unwrap_or_else(|| Contract::new(PROBE))
        .space(PROBE, Envelope::Open);
    program.contract(contract)
}

/// Strip every claim, leaving the body it wrapped.
fn strip_claims(mut program: Program) -> Program {
    fn walk(node: Node) -> Node {
        match node {
            Node::Claim { body, .. } => walk(*body),
            Node::Mark { mark, body } => Node::Mark {
                mark,
                body: Box::new(walk(*body)),
            },
            Node::Reorient { orient, body } => Node::Reorient {
                orient,
                body: Box::new(walk(*body)),
            },
            Node::Bind {
                params,
                palette,
                body,
            } => Node::Bind {
                params,
                palette,
                body: Box::new(walk(*body)),
            },
            Node::Split(mut split) => {
                split.children = split.children.into_iter().map(walk).collect();
                Node::Split(split)
            }
            // Exhaustive on purpose. A catch-all here is how a new wrapper
            // variant keeps the claims underneath it: the strip would return the
            // node whole, the "stripped" program would still declare regions its
            // contract no longer classifies, and the comparison this exists for
            // would be against a program that was never stripped.
            node @ (Node::Void | Node::Skip | Node::Fill { .. } | Node::Call { .. }) => node,
        }
    }
    let rules = std::mem::take(&mut program.rules);
    program.rules = rules
        .into_iter()
        .map(|(symbol, alts)| {
            let stripped = alts
                .into_iter()
                .map(|alt| Alternative {
                    weight: alt.weight,
                    when: alt.when,
                    body: walk(alt.body),
                })
                .collect();
            (symbol, stripped)
        })
        .collect();
    program.contract = None;
    program
}

/// **Declaring spaces moves no blocks, anywhere in the library.**
///
/// Every program, every rule of it wrapped in a claim, at three seeds: same
/// blocks, same anchors. This is `mark`'s transparency assertion one construct
/// over, and it is made over the corpus rather than over an example because a
/// wrapper that was inert in the one place it was tested is the failure this
/// exists to rule out.
#[test]
fn a_claim_moves_no_block_in_any_library_program() {
    let mut checked = 0;
    for (id, size) in CORPUS {
        let program = library::by_id(id).unwrap_or_else(|| panic!("{id} is not in the library"));
        let claimed = claim_everything(program.clone());
        claimed
            .validate()
            .unwrap_or_else(|e| panic!("{id}: the wrapped program is invalid: {e}"));
        for seed in [0u64, 1, 7] {
            let plain = run(&program, Box3::at_origin(*size), seed);
            let wrapped = run(&claimed, Box3::at_origin(*size), seed);
            assert_eq!(
                plain.model.canonical_bytes(),
                wrapped.model.canonical_bytes(),
                "{id} at seed {seed}: a claim moved a block"
            );
            assert_eq!(plain.anchors, wrapped.anchors, "{id} at seed {seed}");
            checked += 1;
        }
        // The wrapper is inert, not absent: it did claim something.
        let wrapped = run(&claimed, Box3::at_origin(*size), 1);
        let probe = &wrapped.contract.as_ref().unwrap().spaces["probe"];
        assert!(
            probe.region.cells() > 0,
            "{id}: the probe region bound to nothing, so the run above proved nothing"
        );
    }
    assert_eq!(checked, CORPUS.len() * 3);
}

/// The other direction, on the one program that declares a contract of its own:
/// removing its declarations leaves the same building.
#[test]
fn stripping_a_programs_own_claims_leaves_the_same_blocks() {
    let program = spatial_contract();
    let bare = strip_claims(program.clone());
    for seed in [0u64, 1, 7] {
        assert_eq!(
            run(&program, PIECE, seed).model.canonical_bytes(),
            run(&bare, PIECE, seed).model.canonical_bytes(),
            "seed {seed}"
        );
    }
    assert!(run(&bare, PIECE, 1).contract.is_none());
    assert!(run(&program, PIECE, 1).contract.is_some());
}

// ---------------------------------------------------------------------------
// 2. Determinism
// ---------------------------------------------------------------------------

fn scratch(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("dw-grammar-contract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cli_expand(file: &Path, out: &Path) {
    let status = Command::new(GRAMMAR)
        .args(["expand", "--file"])
        .arg(file)
        .args(["--region", "11x6x15", "--seed", "3", "--id", "piece", "-o"])
        .arg(out)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}{}",
        String::from_utf8_lossy(&status.stdout),
        String::from_utf8_lossy(&status.stderr)
    );
}

/// **Two processes, the same bytes** — `.nbt` and metadata, contract included.
///
/// In separate processes rather than twice in one, because the failure this
/// guards against is a hash-order or address-order dependency, and one process
/// warmed by the first run can hide it.
#[test]
fn two_processes_write_the_same_nbt_and_the_same_contract() {
    let dir = scratch("determinism");
    let file = dir.join("piece.json");
    std::fs::write(
        &file,
        serde_json::to_vec_pretty(&spatial_contract()).unwrap(),
    )
    .unwrap();

    let a = dir.join("a");
    let b = dir.join("b");
    cli_expand(&file, &a);
    cli_expand(&file, &b);

    let nbt_a = std::fs::read(a.join("piece.nbt")).unwrap();
    let nbt_b = std::fs::read(b.join("piece.nbt")).unwrap();
    assert_eq!(nbt_a, nbt_b, "the structure bytes moved between processes");

    let json_a = std::fs::read_to_string(a.join("piece.json")).unwrap();
    let json_b = std::fs::read_to_string(b.join("piece.json")).unwrap();
    assert_eq!(json_a, json_b, "the metadata bytes moved between processes");
    let value: serde_json::Value = serde_json::from_str(&json_a).unwrap();
    assert!(
        value.get("spatial_contract").is_some(),
        "the metadata carried no contract, so the comparison above proved nothing:\n{json_a}"
    );
}

/// In-process, the resolved contract itself is stable — the map, the box order
/// and the provenance lists.
#[test]
fn the_resolved_contract_is_stable_across_expansions() {
    let program = spatial_contract();
    for seed in [0u64, 1, 7] {
        assert_eq!(
            run(&program, PIECE, seed).contract,
            run(&program, PIECE, seed).contract
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Resolution — the contract in the metadata is this expansion's
// ---------------------------------------------------------------------------

/// **The metadata carries the resolved contract, and re-parameterising the
/// program re-parameterises it.**
///
/// This is the property the whole scope-bound design exists for. A contract
/// written as literal boxes would describe the region it was authored against
/// and silently mis-describe every other one, and `expand` and an audit of the
/// bytes would then disagree about the same piece.
#[test]
fn the_exported_contract_is_the_one_this_expansion_resolved() {
    let program = spatial_contract();
    // Six tall, not five: the corbel shelf needs headroom over it, and an
    // out-of-walk region with no standable cell in it is a region whose kind
    // would be decided over an empty set — which `export_prefab` refuses rather
    // than freezing (spec-0036 §2.6). A program's minimum region is the one
    // where its CONTRACT holds, not the one where its rules merely expand.
    let small = Box3::at_origin([9, 6, 11]);

    let a = export_prefab(&program, PIECE, &ExpandOptions::seeded(1), "piece").unwrap();
    let b = export_prefab(&program, small, &ExpandOptions::seeded(1), "piece").unwrap();

    let ca = a.metadata.spatial_contract.as_ref().unwrap();
    let cb = b.metadata.spatial_contract.as_ref().unwrap();

    assert_eq!(ca.entry, "near");
    assert_eq!(cb.entry, "near");
    // Same names, same envelopes, same graph.
    assert_eq!(
        ca.spaces.keys().collect::<Vec<_>>(),
        cb.spaces.keys().collect::<Vec<_>>()
    );
    assert_eq!(ca.edges.len(), 3);
    assert_eq!(cb.edges.len(), 3);
    // Different boxes, because the boxes are this expansion's.
    assert_ne!(
        ca.spaces["near"].boxes, cb.spaces["near"].boxes,
        "the contract did not follow the region"
    );

    // And they are the boxes that are actually there: the near hollow of an
    // 11x6x15 piece is x 1..9, y 1..4, z 0..6 — the shell is one course on X
    // and Y, the Z faces are open, and the partition is the middle course.
    let near = &ca.spaces["near"].boxes;
    assert_eq!(near.len(), 1);
    assert_eq!(near[0].from, [1, 1, 0]);
    assert_eq!(near[0].to, [9, 4, 6]);

    // The bar region resolved to the doorway's own cells, and carries the block
    // its role is bound to — the campaign binding and the machine claim are one
    // declaration.
    let barred = ca.edges.iter().find(|e| e.class == "barred").unwrap();
    let bar = barred.bar.as_ref().unwrap();
    assert_eq!(bar.region, "gate");
    // The FULL state, properties included: the contract records what the role
    // resolved to, and a bar whose connections were dropped on the way into the
    // metadata would describe a grille while the bytes hold isolated posts.
    assert_eq!(
        bar.block,
        "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]"
    );
    assert_eq!(bar.boxes.len(), 1);
    assert_eq!(bar.boxes[0].from, [4, 1, 7]);
    assert_eq!(bar.boxes[0].to, [6, 3, 7]);

    // The out-of-walk region nests inside the space, which is the one overlap a
    // contract licenses, and it is recorded with its stated reason.
    let shelf = &ca.no_body["shelf"];
    assert!(shelf.reason.contains("watcher"));
    // One box per perch: the ledge is described by the boxes it is actually
    // built from, and claims of one name union. `posted` is proved per cell, so
    // the run that carries the anchors is the run that carries the claims.
    assert_eq!(shelf.boxes.len(), 7);
    assert_eq!(shelf.boxes[0].from, [1, 3, 0]);
    assert_eq!(shelf.boxes[6].to, [1, 4, 6]);
}

/// A zone past the structure cap carries the same block on its manifest, in
/// zone coordinates — a tile boundary is not part of the building.
#[test]
fn a_tiled_zone_carries_its_contract_on_the_manifest() {
    let program = spatial_contract();
    let big = Box3::at_origin([11, 6, 61]);
    let export = export_zone(&program, big, &ExpandOptions::seeded(1), "piece").unwrap();
    let json = export.metadata_json();
    let value: serde_json::Value = serde_json::from_str(json).unwrap();
    assert!(value.get("structure_set").is_some(), "not a tiled export");
    assert!(value.get("spatial_contract").is_some());
    let near = &value["spatial_contract"]["spaces"]["near"]["boxes"][0];
    // Zone coordinates: the near hollow still starts at the zone origin's
    // corner, not at a tile's.
    assert_eq!(near["from"], serde_json::json!([1, 1, 0]));
}

/// The metadata document round-trips the block, and a document without one is
/// still read and written unchanged — absent means legacy, not empty.
#[test]
fn the_metadata_block_round_trips_and_its_absence_is_preserved() {
    use delvewright_schem::prefab::PrefabMeta;

    let export = export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "piece",
    )
    .unwrap();
    let parsed = PrefabMeta::from_json(&export.metadata_json).unwrap();
    assert_eq!(parsed, export.metadata);
    assert_eq!(parsed.to_json(), export.metadata_json);

    let legacy = export.metadata_json.clone();
    let stripped: serde_json::Value = {
        let mut v: serde_json::Value = serde_json::from_str(&legacy).unwrap();
        v.as_object_mut().unwrap().remove("spatial_contract");
        v
    };
    let text = serde_json::to_string_pretty(&stripped).unwrap() + "\n";
    let back = PrefabMeta::from_json(&text).unwrap();
    assert!(back.spatial_contract.is_none());
    // Structurally, not by substring: the provenance sentence names the program,
    // which is called `spatial_contract`.
    let rewritten: serde_json::Value = serde_json::from_str(&back.to_json()).unwrap();
    assert!(rewritten.get("spatial_contract").is_none());
}

// ---------------------------------------------------------------------------
// 4. The version fence
// ---------------------------------------------------------------------------

/// An older document keeps compiling, and to the same bytes.
///
/// Every program in the corpus that writes no fenced construct, declared at
/// `1.0.0` — the surface the format had before any fence existed: identical
/// blocks. That is the promise every fence makes, and it is made over the corpus
/// rather than over one program.
///
/// Which programs those are is asked of `validate` rather than enumerated here.
/// An enumeration is a list, and a list is the thing the fourth fence does not
/// get added to — at which point its programs are silently compared against a
/// version that refuses them, or worse, silently dropped from the comparison.
#[test]
fn a_program_at_the_older_version_compiles_to_identical_bytes() {
    let mut compared = 0usize;
    let mut fenced = 0usize;
    for (id, size) in CORPUS {
        let program = library::by_id(id).unwrap();
        let old = program.clone().at_version("1.0.0");
        match old.validate() {
            Err(ProgramError::FencedConstruct { .. }) => {
                fenced += 1;
                continue; // it writes fenced surface; the fence refuses it below.
            }
            Err(e) => panic!("{id} at 1.0.0: {e}"),
            Ok(()) => {}
        }
        for seed in [0u64, 1] {
            assert_eq!(
                run(&program, Box3::at_origin(*size), seed)
                    .model
                    .canonical_bytes(),
                run(&old, Box3::at_origin(*size), seed)
                    .model
                    .canonical_bytes(),
                "{id} at seed {seed}"
            );
        }
        compared += 1;
    }
    println!(
        "older-version identity   bound {compared:3}  program(s) compared at 1.0.0, \
         {fenced} refused for writing fenced surface"
    );
    assert!(
        compared > 0,
        "binding count 0: every corpus program writes fenced surface, so this compared \
         nothing and the identity promise is unproven"
    );
    assert!(
        fenced > 0,
        "binding count 0: no corpus program writes fenced surface, so the skip arm is \
         untaken and this test cannot tell a fence from its absence"
    );
}

/// The fence, in the direction that matters: the new surface cannot be written
/// under the old version, and the refusal names the construct and both versions.
#[test]
fn the_new_surface_is_refused_at_the_older_version() {
    let claimed = spatial_contract().at_version("1.0.0");
    match claimed.validate() {
        Err(ProgramError::FencedConstruct {
            construct,
            since,
            declared,
            ..
        }) => {
            assert!(construct.contains("claim"));
            assert_eq!(since, "1.2.0");
            assert_eq!(declared, "1.0.0");
        }
        other => panic!("{other:?}"),
    }

    // A contract block with no claim anywhere is fenced too.
    let contract_only = Program::new("blockless", "all")
        .rule("all", Node::Void)
        .contract(Contract::new("nowhere"))
        .at_version("1.0.0");
    assert!(matches!(
        contract_only.validate(),
        Err(ProgramError::FencedConstruct { .. })
    ));
}

/// **The `way` fence, both directions** (spec-0042 AC1).
///
/// `way` is `1.7.0`'s surface. A document declaring `1.5.0` and writing one is
/// refused by the FENCE — not by serde, which is why this program is built in
/// Rust and validated rather than parsed from JSON: a serde refusal would prove
/// only that the field is unknown to some older engine, and the thing asserted
/// here is that THIS engine refuses a document claiming a compatibility it does
/// not have.
///
/// The other direction is the promise the fence exists to keep: the same
/// program with no `way` on it compiles to identical blocks at `1.5.0` and at
/// `1.7.0`, so raising the number is a deliberate act and never a side effect.
#[test]
fn a_way_is_refused_below_its_version_and_a_document_without_one_is_unchanged() {
    let contingent = spatial_contract_with_a_laid_way();
    assert!(contingent.validate().is_ok(), "{:?}", contingent.validate());
    match contingent.at_version("1.5.0").validate() {
        Err(ProgramError::FencedConstruct {
            construct,
            since,
            declared,
            written_by,
        }) => {
            assert!(construct.contains("way"), "{construct}");
            assert_eq!(since, WAY_SINCE);
            assert_eq!(since, "1.7.0");
            assert_eq!(declared, "1.5.0");
            assert!(written_by.contains("planks"), "{written_by}");
        }
        other => panic!("expected a fenced-construct refusal, got {other:?}"),
    }

    // The other direction: no `way`, and the version number changes nothing.
    let at_latest = run(
        &spatial_contract().at_version(LATEST_PROGRAM_VERSION),
        PIECE,
        1,
    );
    let at_old = run(&spatial_contract().at_version("1.5.0"), PIECE, 1);
    assert_eq!(
        at_latest.model.canonical_bytes(),
        at_old.model.canonical_bytes(),
        "a document that writes no way builds the same piece at either version"
    );
    assert!(
        !at_latest.model.canonical_bytes().is_empty(),
        "binding count 0: the two versions were compared over an empty building"
    );
}

/// Two rooms and a level way between them whose deck is laid later.
///
/// Deliberately not a geometrically honest piece — nothing here expands it —
/// because what is under test is the fence, and the fence runs at `validate`,
/// which reads the document and never a block.
fn spatial_contract_with_a_laid_way() -> Program {
    let nest = |names: &[&str]| {
        names
            .iter()
            .rev()
            .fold(Node::Void, |body, region| Node::Claim {
                region: (*region).to_string(),
                body: Box::new(body),
            })
    };
    Program::new("mended-span", "all")
        .role(
            "plank",
            delvewright_grammar::BlockState::simple("oak_planks"),
        )
        .rule("all", nest(&["near", "far", "span", "planks"]))
        .contract(
            Contract::new("near")
                .space("near", Envelope::Open)
                .space("far", Envelope::Open)
                .edge(
                    EXTERIOR,
                    "near",
                    EdgeClass::Walk {
                        rise: 0,
                        via: None,
                        way: None,
                    },
                )
                .edge(
                    "near",
                    "far",
                    EdgeClass::Walk {
                        rise: 0,
                        via: Some("span".to_string()),
                        way: Some(Way {
                            opens: Opens::Laid,
                            region: "planks".to_string(),
                            block: "plank".to_string(),
                        }),
                    },
                ),
        )
}

/// **`barred` + `way` is unwritable, not caught afterwards** (spec-0042 §2.2).
///
/// `barred` IS a walk carrying a cleared way over its bar's region, so an edge
/// that declares both is declaring its contingency twice. `way` is a field of
/// the three traversal classes and of no other, so the document form refuses it
/// by name at parse — the same property that keeps a `bar` off a walk and a
/// `rise` off a sightline.
///
/// The other door — a hand-built piece's resolved metadata, where `class` is a
/// string and every field is present — has no such type, and refuses the pair
/// at `contract-well-formed` instead (`tests/contract_check.rs`). Two doors,
/// one rule.
#[test]
fn a_way_on_a_barred_or_vision_edge_is_not_a_document_this_engine_reads() {
    let cases = [
        (
            "a way on a barred edge",
            r#"{"version":"1.7.0","name":"x","start":"all","rules":{},
                "contract":{"entry":"a","spaces":{"a":{"envelope":"open"}},
                "edges":[{"a":"a","b":"a","class":"barred",
                          "bar":{"region":"g","block":"r"},
                          "way":{"opens":"cleared","region":"g","block":"r"}}]}}"#,
        ),
        (
            "a way on a sightline",
            r#"{"version":"1.7.0","name":"x","start":"all","rules":{},
                "contract":{"entry":"a","spaces":{"a":{"envelope":"open"}},
                "edges":[{"a":"a","b":"a","class":"vision","via":"v",
                          "way":{"opens":"laid","region":"g","block":"r"}}]}}"#,
        ),
    ];
    for (what, json) in cases {
        let err = serde_json::from_str::<Program>(json)
            .expect_err(&format!("{what} must be refused, not dropped"));
        assert!(err.to_string().contains("unknown field"), "{what}: {err}");
    }
    // And the sign is a closed set: a third direction is not a thing to invent.
    let err = serde_json::from_str::<Program>(
        r#"{"version":"1.7.0","name":"x","start":"all","rules":{},
            "contract":{"entry":"a","spaces":{"a":{"envelope":"open"}},
            "edges":[{"a":"a","b":"a","class":"walk",
                      "way":{"opens":"widened","region":"g","block":"r"}}]}}"#,
    )
    .expect_err("an invented sign must be refused");
    assert!(err.to_string().contains("unknown variant"), "{err}");
}

/// **A way is one material, and so is a bar — one refusal for both.**
///
/// A region whose presence decides whether an edge is crossable cannot be
/// "mostly" anything: half a laid deck is not a deck, and a gate that is mostly
/// a bar is not a state anything can be in. The refusal names which of the two
/// spellings the author wrote, and there is exactly one of it — a second
/// bespoke refusal for the second spelling would be the defect rather than the
/// fix.
#[test]
fn a_way_built_from_a_weighted_mix_is_refused_like_a_bar() {
    let nest = |names: &[&str]| {
        names
            .iter()
            .rev()
            .fold(Node::Void, |body, region| Node::Claim {
                region: (*region).to_string(),
                body: Box::new(body),
            })
    };
    let program = Program::new("mended-span", "all")
        .role_mix(
            "rubble",
            vec![
                delvewright_grammar::ir::WeightedBlock {
                    weight: 1,
                    block: delvewright_grammar::BlockState::simple("stone"),
                },
                delvewright_grammar::ir::WeightedBlock {
                    weight: 1,
                    block: delvewright_grammar::BlockState::simple("air"),
                },
            ],
        )
        .rule("all", nest(&["near", "far", "span", "planks"]))
        .contract(
            Contract::new("near")
                .space("near", Envelope::Open)
                .space("far", Envelope::Open)
                .edge(
                    EXTERIOR,
                    "near",
                    EdgeClass::Walk {
                        rise: 0,
                        via: None,
                        way: None,
                    },
                )
                .edge(
                    "near",
                    "far",
                    EdgeClass::Walk {
                        rise: 0,
                        via: Some("span".to_string()),
                        way: Some(Way {
                            opens: Opens::Laid,
                            region: "planks".to_string(),
                            block: "rubble".to_string(),
                        }),
                    },
                ),
        );
    match program.validate() {
        Err(ProgramError::ContingentBlockIsAMix { what, role, region }) => {
            assert_eq!(what, "way");
            assert_eq!(role, "rubble");
            assert_eq!(region, "planks");
        }
        other => panic!("{other:?}"),
    }
}

/// A version this engine does not know is refused outright, rather than parsed
/// for the parts that look familiar.
#[test]
fn an_unknown_version_is_refused() {
    let future = spatial_contract().at_version("2.0.0");
    match future.validate() {
        Err(ProgramError::UnsupportedVersion { version }) => assert_eq!(version, "2.0.0"),
        other => panic!("{other:?}"),
    }
    // And it is refused before anything else is looked at, so the message is
    // about the version rather than about whatever the newer surface confused.
    let broken = Program::new("broken", "missing").at_version("0.1.0");
    assert!(matches!(
        broken.validate(),
        Err(ProgramError::UnsupportedVersion { .. })
    ));
}

// ---------------------------------------------------------------------------
// 5. Reference integrity — every name resolves, in both directions
// ---------------------------------------------------------------------------

fn shell(contract: Contract, body: Node) -> Program {
    Program::new("refs", "all")
        .role("stone", delvewright_grammar::BlockState::simple("stone"))
        .rule("all", body)
        .contract(contract)
}

#[test]
fn a_claim_the_contract_does_not_classify_is_refused() {
    let program = shell(
        Contract::new("room").space("room", Envelope::Open),
        Node::Claim {
            region: "attic".to_string(),
            body: Box::new(Node::Claim {
                region: "room".to_string(),
                body: Box::new(Node::Void),
            }),
        },
    );
    match program.validate() {
        Err(ProgramError::UnclassifiedRegion { region, .. }) => assert_eq!(region, "attic"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_contract_region_no_rule_claims_is_refused() {
    let program = shell(
        Contract::new("room")
            .space("room", Envelope::Open)
            .no_body("ledge", "nothing claims it"),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    match program.validate() {
        Err(ProgramError::UnclaimedRegion { region, .. }) => assert_eq!(region, "ledge"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_edge_endpoint_that_is_not_a_space_is_refused() {
    let program = shell(
        Contract::new("room").space("room", Envelope::Open).edge(
            "room",
            "cellar",
            EdgeClass::Walk {
                rise: 0,
                via: None,
                way: None,
            },
        ),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    match program.validate() {
        Err(ProgramError::UnknownSpace { space, .. }) => assert_eq!(space, "cellar"),
        other => panic!("{other:?}"),
    }
    // `exterior` is the one endpoint that is not a space and is legal.
    let ok = shell(
        Contract::new("room").space("room", Envelope::Open).edge(
            EXTERIOR,
            "room",
            EdgeClass::Walk {
                rise: 0,
                via: None,
                way: None,
            },
        ),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    ok.validate().unwrap();
}

#[test]
fn entry_must_name_a_declared_space() {
    let program = shell(
        Contract::new("porch").space("room", Envelope::Open),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    match program.validate() {
        Err(ProgramError::UnknownSpace {
            space,
            referenced_by,
        }) => {
            assert_eq!(space, "porch");
            assert!(referenced_by.contains("entry"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn one_region_cannot_be_two_things() {
    let both = shell(
        Contract::new("room")
            .space("room", Envelope::Open)
            .no_body("room", "and also a space"),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    assert!(matches!(
        both.validate(),
        Err(ProgramError::RegionClassifiedTwice { .. })
    ));

    // A space that is also an edge's own volume would let a transit volume
    // answer a space's obligations, so it is refused at the same place.
    let via = shell(
        Contract::new("room").space("room", Envelope::Open).edge(
            "room",
            EXTERIOR,
            EdgeClass::Stair {
                rise: 3,
                via: "room".to_string(),
                way: None,
            },
        ),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    assert!(matches!(
        via.validate(),
        Err(ProgramError::RegionClassifiedTwice { .. })
    ));
}

#[test]
fn a_bar_names_a_bound_single_block_role() {
    let mix = Program::new("bar", "all")
        .role_mix(
            "rubble",
            vec![
                delvewright_grammar::ir::WeightedBlock {
                    weight: 1,
                    block: delvewright_grammar::BlockState::simple("stone"),
                },
                delvewright_grammar::ir::WeightedBlock {
                    weight: 1,
                    block: delvewright_grammar::BlockState::simple("air"),
                },
            ],
        )
        .rule(
            "all",
            Node::Claim {
                region: "room".to_string(),
                body: Box::new(Node::Claim {
                    region: "gate".to_string(),
                    body: Box::new(Node::Void),
                }),
            },
        )
        .contract(Contract::new("room").space("room", Envelope::Open).edge(
            "room",
            EXTERIOR,
            EdgeClass::Barred {
                rise: 0,
                bar: Bar {
                    region: "gate".to_string(),
                    block: "rubble".to_string(),
                },
                via: None,
            },
        ));
    match mix.validate() {
        Err(ProgramError::ContingentBlockIsAMix { what, role, region }) => {
            assert_eq!(what, "bar");
            assert_eq!(role, "rubble");
            assert_eq!(region, "gate");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_region_may_not_be_called_exterior() {
    let program = shell(
        Contract::new("room").space("room", Envelope::Open),
        Node::Claim {
            region: EXTERIOR.to_string(),
            body: Box::new(Node::Claim {
                region: "room".to_string(),
                body: Box::new(Node::Void),
            }),
        },
    );
    match program.validate() {
        Err(ProgramError::BadRegionName { region, .. }) => assert_eq!(region, EXTERIOR),
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 6. What resolution does with the boxes
// ---------------------------------------------------------------------------

/// Several claims of one name union, and the record is the *set* of boxes: same
/// boxes claimed in a different order resolve identically.
#[test]
fn claims_of_one_name_union_in_a_canonical_order() {
    use delvewright_grammar::geom::Axis;
    use delvewright_grammar::ir::{Size, Split};

    fn two_thirds(first: &str, second: &str) -> Program {
        Program::new("union", "all")
            .rule(
                "all",
                Node::Split(Split {
                    axis: Axis::Z,
                    sizes: vec![Size::abs(2), Size::abs(2)],
                    rounding: Default::default(),
                    repeat: false,
                    orient: Default::default(),
                    children: vec![
                        Node::Claim {
                            region: first.to_string(),
                            body: Box::new(Node::Void),
                        },
                        Node::Claim {
                            region: second.to_string(),
                            body: Box::new(Node::Void),
                        },
                    ],
                }),
            )
            .contract(Contract::new("room").space("room", Envelope::Open))
    }

    let out = run(&two_thirds("room", "room"), Box3::at_origin([2, 2, 4]), 1);
    let room = &out.contract.as_ref().unwrap().spaces["room"].region;
    assert_eq!(room.boxes.len(), 2, "two claims, two boxes");
    assert_eq!(room.boxes[0].origin, [0, 0, 0]);
    assert_eq!(
        room.boxes[1].origin,
        [0, 0, 2],
        "sorted, not derivation order"
    );
    assert_eq!(room.cells(), 16);
}

/// A claim on a scope with no cells contributes nothing — the same thing `fill`
/// and `void` do there — and a space nothing claimed resolves to no boxes rather
/// than disappearing, so the zero is visible to whatever reads the contract.
#[test]
fn an_empty_scope_and_an_unreached_rule_leave_an_honest_zero() {
    use delvewright_grammar::geom::Axis;
    use delvewright_grammar::ir::{Size, Split};

    let program = Program::new("thin", "all")
        .rule(
            "all",
            Node::Split(Split {
                axis: Axis::Z,
                // Two absolute pieces that consume the whole axis leave the
                // relative one with nothing: a legal zero-volume scope.
                sizes: vec![Size::abs(2), Size::rel(1)],
                rounding: Default::default(),
                repeat: false,
                orient: Default::default(),
                children: vec![
                    Node::Claim {
                        region: "room".to_string(),
                        body: Box::new(Node::Void),
                    },
                    Node::Claim {
                        region: "sliver".to_string(),
                        body: Box::new(Node::Void),
                    },
                ],
            }),
        )
        .contract(
            Contract::new("room")
                .space("room", Envelope::Open)
                .no_body("sliver", "a zero-volume leftover")
                // `never` is claimed only by a rule nothing calls.
                .no_body("never", "claimed by an uncalled rule"),
        )
        .rule(
            "uncalled",
            Node::Claim {
                region: "never".to_string(),
                body: Box::new(Node::Void),
            },
        );

    let out = run(&program, Box3::at_origin([2, 2, 2]), 1);
    let contract = out.contract.as_ref().unwrap();
    assert_eq!(contract.spaces["room"].region.cells(), 8);
    assert_eq!(
        contract.no_body["sliver"].region.boxes,
        vec![],
        "a zero-volume scope contributes no box"
    );
    assert_eq!(contract.no_body["never"].region.cells(), 0);
    assert!(
        contract.no_body["never"].region.declared_by.is_empty(),
        "nothing ran, so nothing declared it"
    );
}

// ---------------------------------------------------------------------------
// 7. Composition
// ---------------------------------------------------------------------------

/// A region name is the *composing program's* vocabulary, so `include` prefixes
/// it — the opposite of an anchor, which is the campaign's name for a place.
///
/// Left unqualified, a piece included twice would union its two rooms into one
/// region and the zone would describe a room that is not there.
#[test]
fn including_a_claiming_piece_qualifies_its_regions_and_says_so() {
    let zone = Program::new("zone", "plan").rule("plan", Node::call("west/all"));
    let piece = shell(
        Contract::new("room").space("room", Envelope::Open),
        Node::Claim {
            region: "room".to_string(),
            body: Box::new(Node::Void),
        },
    );
    let composed = delvewright_grammar::include(zone, &piece, "west").unwrap();
    assert_eq!(
        composed.claimed_regions().keys().collect::<Vec<_>>(),
        vec!["west/room"]
    );

    // The zone did not inherit the piece's contract, and it is told so by name
    // rather than silently building a piece with no contract at all.
    match composed.validate() {
        Err(ProgramError::UnclassifiedRegion { region, .. }) => assert_eq!(region, "west/room"),
        other => panic!("{other:?}"),
    }

    // Classifying it in the zone's own contract is what makes it legal, and the
    // zone decides what the composed space is.
    let owned =
        composed.contract(Contract::new("west/room").space("west/room", Envelope::Enclosed));
    owned.validate().unwrap();
    let out = run(&owned, Box3::at_origin([4, 4, 4]), 1);
    assert_eq!(out.contract.unwrap().spaces["west/room"].region.cells(), 64);
}

// ---------------------------------------------------------------------------
// 8. Export, over a contract that carries a way (spec-0042 §2.3, AC6/AC7)
// ---------------------------------------------------------------------------

/// **The corpus piece, with its barred door respelled as what it means.**
///
/// `barred { rise, bar, via }` IS `walk { rise, via } + way { cleared,
/// bar.region, bar.block }` (spec-0042 §2.2), so this is the same building
/// declared the other way round: same rules, same roles, same blocks, one edge
/// spelled differently. That is what makes it usable as the export fixture —
/// every assertion below about a way is made against a piece whose block bytes
/// are known to be the barred piece's, so nothing that moves can be blamed on
/// the geometry.
///
/// It also carries an anchor **inside the way region**, which the barred
/// original does not: the doorway scope's local `(1, 2, 0)` is world `[5, 2, 7]`
/// and the gate resolves to `[4,1,7]..[6,3,7]`. A mark writes no block
/// (`tests/contract.rs` §1), so it costs the byte comparison nothing.
fn spatial_contract_as_a_cleared_way() -> Program {
    let mut program = spatial_contract();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { rise, bar, .. } = &edge.class {
            edge.class = EdgeClass::Walk {
                rise: *rise,
                // The bar's own region is the transit volume: the cells that
                // decide the crossing belong to the edge, which is exactly what
                // `barred` always meant and never had to say.
                via: Some(bar.region.clone()),
                way: Some(Way {
                    opens: Opens::Cleared,
                    region: bar.region.clone(),
                    block: bar.block.clone(),
                }),
            };
        }
    }
    for alt in program
        .rules
        .get_mut("doorway")
        .expect("the corpus builds its doorway in a rule of that name")
    {
        let body = std::mem::replace(&mut alt.body, Node::Void);
        alt.body = Node::Mark {
            mark: Mark::new("gate-watch", MarkAt::offset(1, 2, 0)),
            body: Box::new(body),
        };
    }
    program
}

/// **The exported way carries name, sign, role, resolved block and cells, and an
/// anchor inside it resolves to `way:<name>`** (spec-0042 §2.3, AC7).
///
/// The five things a reader needs and the compiler later emits from, asserted on
/// the metadata document rather than on the resolved contract: the document is
/// what crosses into `delvec` and into `delve-admit`, and a field that resolves
/// correctly and is dropped on the way out is the defect this exists to catch.
#[test]
fn the_exported_metadata_carries_the_way_and_the_anchor_inside_it_names_it() {
    let export = export_prefab(
        &spatial_contract_as_a_cleared_way(),
        PIECE,
        &ExpandOptions::seeded(1),
        "piece",
    )
    .expect("the respelled piece exports green");
    let contract = export.metadata.spatial_contract.as_ref().unwrap();

    let walk = contract
        .edges
        .iter()
        .find(|e| e.a == "near" && e.b == "far")
        .expect("the door edge");
    assert_eq!(walk.class, "walk", "the door is a walk now, not a `barred`");
    assert!(
        walk.bar.is_none(),
        "a walk carries no bar — the two spellings are alternatives, not layers"
    );
    let way = walk.way.as_ref().expect("and it carries its way");
    assert_eq!(way.region, "gate", "name");
    assert_eq!(way.opens, "cleared", "sign");
    assert_eq!(
        way.role.as_deref(),
        Some("bar"),
        "the palette role, in the author's words"
    );
    // The FULL state, properties included — the same demand the bar's block
    // carries: a grille whose connections were dropped on the way into the
    // metadata describes a row of posts, and an `open-way` emitting it would
    // build one.
    assert_eq!(
        way.block,
        "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]"
    );
    assert_eq!(way.boxes.len(), 1, "cells");
    assert_eq!(way.boxes[0].from, [4, 1, 7]);
    assert_eq!(way.boxes[0].to, [6, 3, 7]);
    // The same cells the barred spelling resolved its bar to — the respelling
    // moved a name, not a box.
    let barred = export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "piece",
    )
    .unwrap()
    .metadata
    .spatial_contract
    .unwrap();
    let bar = barred
        .edges
        .iter()
        .find(|e| e.class == "barred")
        .and_then(|e| e.bar.as_ref())
        .expect("the original bar");
    assert_eq!(way.boxes, bar.boxes);
    assert_eq!(way.block, bar.block);

    // An anchor inside a way region names the way, as one inside a bar region
    // names the bar — read off the metadata document a campaign binds against.
    let anchor = export
        .metadata
        .anchors
        .get("anchor/gate-watch")
        .expect("the mark reached the metadata");
    assert_eq!(anchor.pos, Some([5, 2, 7]));
    assert_eq!(anchor.resolves_to.as_deref(), Some("way:gate"));
}

/// **Declaring a way moves no block bytes** (spec-0042 AC7).
///
/// The transparency claim `claim` itself owes, one construct over: a contract is
/// a statement about a building and never a part of it, so respelling an edge as
/// a way — and marking a cell inside it — has to leave the model bit for bit
/// where it was, at every seed.
#[test]
fn declaring_a_way_moves_no_block() {
    for seed in [0u64, 1, 7] {
        let plain = run(&spatial_contract(), PIECE, seed);
        let contingent = run(&spatial_contract_as_a_cleared_way(), PIECE, seed);
        assert_eq!(
            plain.model.canonical_bytes(),
            contingent.model.canonical_bytes(),
            "seed {seed}: declaring a way moved a block"
        );
        assert!(
            !plain.model.canonical_bytes().is_empty(),
            "seed {seed}: the two models were compared over an empty building"
        );
        // …and the way is really declared, so the comparison above is between a
        // contract that carries one and a contract that does not.
        let way = contingent
            .contract
            .as_ref()
            .unwrap()
            .edges
            .iter()
            .find_map(|e| e.way.as_ref())
            .expect("the respelled edge resolved a way");
        assert!(way.boxes.iter().map(|b| b.volume()).sum::<u64>() > 0);
    }
}

/// **A piece that declares no way exports the bytes it always did** (spec-0042
/// §2.3, AC6).
///
/// `way` is an optional key that is written only when an edge carries one, and
/// `barred` keeps writing `bar`: the normalisation into one prover is the
/// checker's business and stops at the checker. Swept over the WHOLE corpus
/// rather than over the one program somebody remembered, because a key that was
/// absent in the place it was looked at is the failure this rules out — and the
/// population is asserted non-empty, because the corpus declares exactly one
/// contract today and a sweep that found none would pass identically.
///
/// The absence is only worth something beside the presence, so the last half
/// exports a way-carrying piece through the same serialiser: the key appears
/// exactly when an edge declares one, rather than never appearing at all.
#[test]
fn no_corpus_piece_writes_a_way_and_every_bar_still_writes_bar() {
    let mut pieces = 0usize;
    let mut bars = 0usize;
    for (id, size) in CORPUS {
        let program = library::by_id(id).unwrap_or_else(|| panic!("{id} is not in the library"));
        let expansion = run(&program, Box3::at_origin(*size), 1);
        let Some(contract) = delvewright_grammar::export::contract_metadata(&expansion) else {
            continue;
        };
        pieces += 1;
        for edge in &contract.edges {
            assert!(
                edge.way.is_none(),
                "{id}: an edge that declares no way exported one"
            );
            if edge.bar.is_some() {
                assert_eq!(edge.class, "barred", "{id}");
                bars += 1;
            }
        }
        let json = serde_json::to_string(&contract).unwrap();
        assert!(
            !json.contains("\"way\""),
            "{id}: the serialised contract carries a `way` key: {json}"
        );
    }
    assert!(
        pieces > 0,
        "no corpus program declares a contract, so this examined nothing"
    );
    assert!(
        bars > 0,
        "no corpus program declares a bar, so `bar stays bar` was proved over zero bars"
    );

    // The other half: the same serialiser writes the key when there is one to
    // write, so the sweep above is an absence and not a blind spot.
    let contingent = serde_json::to_string(
        &export_prefab(
            &spatial_contract_as_a_cleared_way(),
            PIECE,
            &ExpandOptions::seeded(1),
            "piece",
        )
        .unwrap()
        .metadata
        .spatial_contract
        .unwrap(),
    )
    .unwrap();
    assert!(contingent.contains("\"way\""), "{contingent}");
    // Structurally, not by substring: this corpus's palette role is itself
    // called `bar`, so a way's own `role` puts the word in the document. What
    // must be absent is the KEY on the edge.
    let value: serde_json::Value = serde_json::from_str(&contingent).unwrap();
    for edge in value["edges"].as_array().unwrap() {
        assert!(
            edge.get("bar").is_none(),
            "a walk carrying a way writes no bar: {edge}"
        );
    }
}

/// **Two processes, the same bytes — over a contract that carries a way.**
///
/// The double-expand suite's own vacuity clause (spec-0042 AC7): determinism
/// asserted only over contracts *without* ways proves nothing about the surface
/// this spec adds. Same shape as the sibling above — two processes, not two
/// runs in one — and the metadata is asserted to actually carry the way before
/// the comparison is believed.
#[test]
fn two_processes_write_the_same_contract_when_it_carries_a_way() {
    let dir = scratch("determinism-way");
    let file = dir.join("piece.json");
    std::fs::write(
        &file,
        serde_json::to_vec_pretty(&spatial_contract_as_a_cleared_way()).unwrap(),
    )
    .unwrap();

    let a = dir.join("a");
    let b = dir.join("b");
    cli_expand(&file, &a);
    cli_expand(&file, &b);

    let nbt_a = std::fs::read(a.join("piece.nbt")).unwrap();
    let nbt_b = std::fs::read(b.join("piece.nbt")).unwrap();
    assert_eq!(nbt_a, nbt_b, "the structure bytes moved between processes");

    let json_a = std::fs::read_to_string(a.join("piece.json")).unwrap();
    let json_b = std::fs::read_to_string(b.join("piece.json")).unwrap();
    assert_eq!(json_a, json_b, "the metadata bytes moved between processes");

    let value: serde_json::Value = serde_json::from_str(&json_a).unwrap();
    let way = &value["spatial_contract"]["edges"][1]["way"];
    assert!(
        way.is_object(),
        "the metadata carried no way, so the comparison above proved nothing about ways:\n{json_a}"
    );
    assert_eq!(way["region"], serde_json::json!("gate"));
    assert_eq!(way["opens"], serde_json::json!("cleared"));
    assert_eq!(
        value["anchors"]["anchor/gate-watch"]["resolves_to"],
        serde_json::json!("way:gate")
    );
}

/// **The other sign, on the same building**: the doorway's threshold is missing
/// and content lays it.
///
/// The corpus piece with its partition's floor course claimed as `deck` and
/// left empty, the passage over it open, and the door declared a `walk` whose
/// way is `laid`. As built the two rooms are severed — a body cannot stand on
/// a threshold that is not there — and laying the deck in the `floor` role
/// connects them.
///
/// It exists because the export writes the sign it was handed, and a corpus in
/// which every exported way is `cleared` would let an export that wrote the
/// word "cleared" pass every assertion here.
fn spatial_contract_as_a_laid_deck() -> Program {
    broken_threshold(true)
}

/// The same broken threshold with nothing declared about it: the deck cells are
/// air in both, and only this one has no `way` and no `deck` claim.
///
/// The red twin of the fixture above, kept beside it so the pair is one delta
/// apart in the DECLARATION and zero deltas apart in the blocks.
fn broken_threshold_undeclared() -> Program {
    broken_threshold(false)
}

fn broken_threshold(declared: bool) -> Program {
    let mut program = spatial_contract();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { rise, .. } = &edge.class {
            edge.class = EdgeClass::Walk {
                rise: *rise,
                // The whole doorway column is the transit volume; the threshold
                // course inside it is what opening lays.
                via: Some("gate".to_string()),
                way: declared.then(|| Way {
                    opens: Opens::Laid,
                    region: "deck".to_string(),
                    block: "floor".to_string(),
                }),
            };
        }
    }
    let threshold = if declared {
        Node::Claim {
            region: "deck".to_string(),
            body: Box::new(Node::Void),
        }
    } else {
        Node::Void
    };
    program.rule(
        "doorway",
        Node::Claim {
            region: "gate".to_string(),
            body: Box::new(Node::Split(Split {
                axis: Axis::Y,
                sizes: vec![Size::abs(1), Size::abs(3), Size::rel(1)],
                rounding: Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![threshold, Node::Void, Node::fill("shell")],
            })),
        },
    )
}

/// **A `laid` way exports its own sign, role, block and cells** (spec-0042
/// §2.3, AC7).
///
/// The sibling of the `cleared` assertion above, and the reason it is not
/// enough on its own: an export that hardcoded either word would satisfy one of
/// the two and neither would notice.
#[test]
fn a_laid_way_exports_the_sign_and_the_block_it_will_be_filled_with() {
    let export = export_prefab(
        &spatial_contract_as_a_laid_deck(),
        PIECE,
        &ExpandOptions::seeded(1),
        "piece",
    )
    .expect("the broken threshold exports green with its way declared");
    let contract = export.metadata.spatial_contract.as_ref().unwrap();
    let way = contract
        .edges
        .iter()
        .find_map(|e| e.way.as_ref())
        .expect("the door carries a way");
    assert_eq!(way.opens, "laid");
    assert_eq!(way.region, "deck");
    assert_eq!(way.role.as_deref(), Some("floor"));
    // What the cells BECOME when it is opened, resolved from the role — the
    // one authority an `open-way` reads.
    assert_eq!(way.block, "minecraft:stone");
    assert_eq!(way.boxes.len(), 1);
    assert_eq!(way.boxes[0].from, [4, 0, 7]);
    assert_eq!(way.boxes[0].to, [6, 0, 7]);

    // The bytes are severed as shipped: the cells the way will lay are air, and
    // that is what makes the closed half of the proof above a real one. Asserted
    // on the exported model rather than trusted from the declaration.
    let expansion = run(&spatial_contract_as_a_laid_deck(), PIECE, 1);
    for x in 4..=6 {
        assert!(
            expansion.model.get([x, 0, 7]).map(|b| b.is_air()) == Some(true),
            "the threshold at [{x},0,7] is not empty as built"
        );
    }

    // **And the way is load-bearing.** Take the declaration off the same bytes
    // and the piece is refused: the far room is unreachable and the walk does
    // not connect. A green over a piece that would have been green anyway says
    // nothing about the surface being exported.
    let undeclared = broken_threshold_undeclared();
    assert_eq!(
        run(&undeclared, PIECE, 1).model.canonical_bytes(),
        expansion.model.canonical_bytes(),
        "the two twins must be one DECLARATION apart and zero blocks apart"
    );
    match export_prefab(&undeclared, PIECE, &ExpandOptions::seeded(1), "piece") {
        Err(delvewright_grammar::ExportError::Contract { gates }) => {
            let joined = gates.join(" · ");
            assert!(joined.contains("contract-edge-proof"), "{joined}");
            assert!(joined.contains("contract-reachability"), "{joined}");
        }
        other => panic!("the undeclared twin must be refused, got {other:?}"),
    }
}
