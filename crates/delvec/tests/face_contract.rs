//! **Two pieces that were each approved alone, and do not assemble**
//! (ADR-0020 §3, spec-0036 §2.8; `DW0780`).
//!
//! The failure this exists for is not that a piece is wrong. Both pieces here
//! pass every prefab gate there is, individually — they are the same prefab. It
//! is the pair that is wrong: one declares a way out on the face they share, and
//! the piece on the other side of it does not answer.
//!
//! The artifacts are real. The prefabs are exported by `crates/delvec/src/grammar` from the
//! corpus program that declares a contract, loaded back through the engine's own
//! `PrefabRegistry`, and placed the way a campaign places them.

use delvec::compiler::faces;
use delvec::compiler::plan::{AreaPlacement, PiecePlacement, PlacedTemplate};
use delvec::compiler::registry::PrefabRegistry;
use delvec::compiler::solver::Rotation;
use delvec::grammar::library::spatial_contract::spatial_contract;
use delvec::grammar::{Box3, ExpandOptions, export_prefab};

/// The region the corpus program is documented at.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

/// Export the contract-carrying corpus program into a throwaway prefab library.
fn library(tag: &str) -> (std::path::PathBuf, PrefabRegistry) {
    let dir = std::env::temp_dir().join(format!("dw-face-contract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let exported = export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "twin-room",
    )
    .expect("the corpus program exports");
    exported.write_to_dir(&dir).unwrap();
    let registry =
        PrefabRegistry::load_dir(&dir).expect("the engine loads a contract-carrying prefab");
    (dir, registry)
}

fn placed(area: &str, pos: [i32; 3], rotation: Rotation) -> AreaPlacement {
    AreaPlacement {
        area_id: area.to_string(),
        pieces: vec![PiecePlacement {
            prefab_id: "prefab/twin-room".to_string(),
            templates: vec![PlacedTemplate {
                structure_id: "twin-room".to_string(),
                structure_file: "twin-room.nbt".to_string(),
                pos,
                size: [11, 6, 15],
            }],
            pos,
            size: [11, 6, 15],
            rotation,
            mated: Vec::new(),
        }],
        seals: Vec::new(),
        mass: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// The library as it actually is: pieces with SOCKETS and no spatial contract
// ---------------------------------------------------------------------------
//
// Every hand-built piece in the content library predates spec-0036, so the
// contract-carrying prefab above is the exception and this is the rule. It is
// also what the drill campaign is made of, and what the check reported `0 with a
// spatial contract` about while passing.

/// A socket-only piece: a 7 × 6 × 7 box with a 3 × 3 jigsaw socket on its north
/// and south faces, and no `spatial_contract` at all.
///
/// Written as the metadata document a generator emits rather than through a
/// grammar program, because *that document* is the artifact under test: what is
/// being asserted is that the compiler judges a piece which declares its sides
/// the only way the library has ever declared them.
fn socket_library(tag: &str) -> (std::path::PathBuf, PrefabRegistry) {
    let dir = std::env::temp_dir().join(format!("dw-face-socket-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = serde_json::json!({
        "prefab_id": "prefab/cell",
        "structure": {
            "file": "cell.nbt",
            "id": "cell",
            "size": [7, 6, 7],
            "data_version": 4671,
            "generator": "face_contract test"
        },
        "anchors": {},
        "connectors": [
            {
                "name": "kit:socket", "target": "kit:socket",
                "local_pos": [3, 1, 0], "facing": "north",
                "opening": [3, 3], "joint": "aligned"
            },
            {
                "name": "kit:socket", "target": "kit:socket",
                "local_pos": [3, 1, 6], "facing": "south",
                "opening": [3, 3], "joint": "aligned"
            }
        ],
        "license": {
            "source": "original", "spdx": "GPL-3.0-or-later",
            "note": "test fixture", "provenance": "written by this test"
        }
    });
    std::fs::write(
        dir.join("cell.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
    let registry = PrefabRegistry::load_dir(&dir).expect("the engine loads a socket-only prefab");
    assert!(
        registry
            .get("prefab/cell")
            .expect("it loaded")
            .spatial_contract
            .is_none(),
        "the fixture is the library as it is: sockets and no contract"
    );
    (dir, registry)
}

/// One socket-only piece, placed, with the layout's mated flags stated.
fn socket_piece(pos: [i32; 3], mated: [bool; 2]) -> PiecePlacement {
    PiecePlacement {
        prefab_id: "prefab/cell".to_string(),
        templates: vec![PlacedTemplate {
            structure_id: "cell".to_string(),
            structure_file: "cell.nbt".to_string(),
            pos,
            size: [7, 6, 7],
        }],
        pos,
        size: [7, 6, 7],
        rotation: Rotation::None,
        mated: mated.to_vec(),
    }
}

/// The chain a pool area produces: pieces in one area, mated end to end.
fn socket_area(pieces: Vec<PiecePlacement>) -> AreaPlacement {
    AreaPlacement {
        area_id: "area/annex".to_string(),
        pieces,
        seals: Vec::new(),
        mass: Vec::new(),
    }
}

/// **The engine reads a contract-carrying prefab at all.**
///
/// The seam that would otherwise fail months later in a campaign build: the
/// grammar exporter writes the contract block, and the compiler's metadata
/// reader refuses fields it does not know.
#[test]
fn a_prefab_carrying_a_spatial_contract_loads_and_exports_its_faces() {
    let (_dir, registry) = library("load");
    let meta = registry.get("prefab/twin-room").expect("it loaded");
    let contract = meta
        .spatial_contract
        .as_ref()
        .expect("the contract block survived the round trip");
    assert_eq!(contract.faces.len(), 2, "one way in at each end");
    let sides: Vec<&str> = contract.faces.iter().map(|f| f.dir.as_str()).collect();
    assert_eq!(sides, vec!["north", "south"]);
    assert!(contract.faces.iter().all(|f| f.class == "walk"));
}

/// **The green.** Two copies back to back: the south face of one meets the north
/// face of the other, same opening, same class.
#[test]
fn two_pieces_whose_faces_answer_each_other_assemble() {
    let (_dir, registry) = library("green");
    let areas = vec![
        placed("area/first", [0, 0, 0], Rotation::None),
        placed("area/second", [0, 0, 15], Rotation::None),
    ];
    let binding = faces::check(&areas, &registry).expect("the pieces fit");
    assert!(
        binding.bound > 0,
        "the mating check examined ZERO abutting faces, so it proved nothing"
    );
    assert_eq!(binding.contracted, 2);
    assert!(binding.finding(2, false).is_none());
}

/// **The red.** The same two pieces, one of them a block to the side. Every
/// prefab gate still passes on each piece; the assembly is a doorway into a
/// wall, and the refusal names both pieces, both faces and what is wrong.
#[test]
fn a_way_out_the_neighbour_does_not_answer_is_refused_naming_both_pieces() {
    let (_dir, registry) = library("red");
    let areas = vec![
        placed("area/first", [0, 0, 0], Rotation::None),
        placed("area/second", [1, 0, 15], Rotation::None),
    ];
    let err = faces::check(&areas, &registry).expect_err("a door into a wall must be refused");
    assert_eq!(err.failure.code.id(), "DW0780");
    // Both pieces, by the name a reviewer would look them up under.
    assert!(
        err.failure.message.contains("area/first"),
        "{}",
        err.failure.message
    );
    assert!(
        err.failure.message.contains("area/second"),
        "{}",
        err.failure.message
    );
    assert!(
        err.failure.message.contains("prefab/twin-room"),
        "{}",
        err.failure.message
    );
    // Both faces, with where they are.
    assert!(
        err.failure.message.contains("south walk"),
        "{}",
        err.failure.message
    );
    assert!(
        err.failure.message.contains("north walk"),
        "{}",
        err.failure.message
    );
    // And the incompatibility itself, said as a thing to do something about.
    assert!(
        err.failure.message.contains("does not answer"),
        "{}",
        err.failure.message
    );
}

/// **A rotated piece turns its face contract with it.** Reading the declared
/// side without the placement's rotation would mate a north door to a north
/// door, which is two doors into the same wall.
#[test]
fn a_rotation_turns_the_declared_side_and_the_mating_follows() {
    let (_dir, registry) = library("rotated");
    // 180° about the origin pivot puts the piece behind and to the left of its
    // position; the two ends swap which way they point.
    let areas = vec![
        placed("area/first", [0, 0, 0], Rotation::None),
        placed("area/second", [10, 0, 29], Rotation::Cw180),
    ];
    let binding = faces::check(&areas, &registry).expect("a turned piece still mates");
    assert!(binding.bound > 0, "the check examined nothing");
}

/// **A face onto the outside is not a finding.** A box garden has an outside,
/// and a front door is meant to face it — but a world in which nothing abuts
/// anything says so, rather than reporting a pass.
#[test]
fn a_lone_piece_binds_the_mating_check_to_zero_and_says_so() {
    let (_dir, registry) = library("lone");
    let areas = vec![placed("area/only", [0, 0, 0], Rotation::None)];
    let binding = faces::check(&areas, &registry).expect("a lone piece cannot mis-mate");
    assert_eq!(binding.bound, 0);
    let finding = binding.finding(1, false).expect("a zero binding is stated");
    assert_eq!(finding.code, "DW0781");
    assert!(
        finding.message.contains("ZERO abutting faces"),
        "{}",
        finding.message
    );
}

/// **A world whose ways are ALLOCATED is never asked to mate** (spec-0050 §3).
///
/// The pair below is the red above with one thing changed — the same two pieces,
/// the same one-block offset, the same doorway into the same wall — placed in the
/// one area a site plan has. That is the whole difference, and it is the whole
/// point. A site plan cuts its ways at stage 4 on faces two boxes already share
/// and proves them over the built bytes with `DW0836`/`DW0838`; nothing in that
/// world ever claimed to mate with anything, so a claim it never made cannot be
/// a claim it fails to answer.
///
/// Two things reach this, and the second is why the predicate is the AREA rather
/// than "the registry has never heard of this piece". A derived blockout box has
/// no metadata, and the party plane a detail piece's face opens onto lies inside
/// that box's shell — a narrower predicate covers that one. Two DETAIL pieces
/// stacked vertically mate through the horizontal party plane DIRECTLY, and this
/// check demands their classes be equal where spec-0050 §3's table requires
/// `drop` leaving against `walk` landing, and `stair` in the hosting box against
/// `walk` in the other. That pair would have satisfied `DW0844` and then
/// hard-failed here.
#[test]
fn pieces_in_the_site_area_are_allocated_rather_than_mated() {
    let (_dir, registry) = library("allocated");
    let areas = vec![
        placed(delvewright_dsl::SITE_AREA, [0, 0, 0], Rotation::None),
        placed(delvewright_dsl::SITE_AREA, [1, 0, 15], Rotation::None),
    ];
    let binding = faces::check(&areas, &registry)
        .expect("an allocated world is not judged by the mating check");
    assert_eq!(
        binding.bound, 0,
        "and it examined nothing, rather than examining and passing"
    );
    assert_eq!(
        binding.contracted, 2,
        "the pieces still declare their contracts — the count is honest about what \
         is there, and only the QUESTION moved"
    );
    let finding = binding
        .finding(2, true)
        .expect("a zero binding is stated whether or not it is a fault");
    assert_eq!(finding.code, "DW0781");
    assert!(
        finding.message.contains("SITE PLAN") && finding.message.contains("DW0836"),
        "and the reader is told where the question went — one line, with the essay in \
         `compiler.md`'s `DW0781` row: {}",
        finding.message
    );

    // The suspicion is the AREA and nothing else: the identical pair in ordinary
    // areas is still refused, so the repair did not weaken the check.
    let areas = vec![
        placed("area/first", [0, 0, 0], Rotation::None),
        placed("area/second", [1, 0, 15], Rotation::None),
    ];
    let err = faces::check(&areas, &registry)
        .expect_err("the same pair outside a site plan is still a door into a wall");
    assert_eq!(err.failure.code.id(), "DW0780");
}

// ---------------------------------------------------------------------------
// The library the engine actually has to judge
// ---------------------------------------------------------------------------

/// **A world of socket-only pieces is examined, not excused.**
///
/// This is the drill campaign's shape and the gallery annex's shape: pieces that
/// declare no `spatial_contract` and are mated end to end by the jigsaw. The
/// check reported `0 with a spatial contract, 0 of those with a face, and 0
/// face(s) declared in all` on exactly this and passed — while the sockets that
/// placed the pieces sat unread in the same document.
#[test]
fn a_chain_of_socket_only_pieces_is_examined_by_its_sockets() {
    let (_dir, registry) = socket_library("green");
    let areas = vec![socket_area(vec![
        socket_piece([0, 0, 0], [false, true]),
        socket_piece([0, 0, 7], [true, false]),
    ])];
    let binding = faces::check(&areas, &registry).expect("a mated chain assembles");
    assert_eq!(binding.contracted, 0, "not one of them declares a contract");
    assert_eq!(binding.socketed, 2, "and both are judged by their sockets");
    assert_eq!(binding.pairs, 1, "the two boxes touch");
    assert_eq!(binding.judged, 1, "and a declared face crosses the plane");
    assert_eq!(binding.bound, 2, "one socket from each side of the seam");
    assert!(
        binding.finding(2, false).is_none(),
        "a bound check raises no zero-binding advisory"
    );
    let line = binding.line(2);
    assert!(
        line.contains("2 of 2 placed piece(s) touch in 1 pair(s), 1 of which"),
        "the line states the placement's own fraction: {line}"
    );
}

/// **The perturbation toward the vacuous shape: detach one piece by a block.**
///
/// Nothing about either piece changes and no author writes anything — the second
/// piece simply stands one block further away. The pair stops touching, so a
/// check quantified over pairs that touch has nothing left to say about it; what
/// still speaks is the LAYOUT'S OWN claim that the socket is mated, which is the
/// claim `seal_layout` cleared the doorway to air on. The world would ship an
/// open hole with nothing behind it.
#[test]
fn detaching_a_mated_piece_by_one_block_is_named() {
    let (_dir, registry) = socket_library("detached");
    let areas = vec![socket_area(vec![
        socket_piece([0, 0, 0], [false, true]),
        socket_piece([0, 0, 8], [true, false]),
    ])];
    let err = faces::check(&areas, &registry)
        .expect_err("a mated socket with nothing beyond it must be refused");
    assert_eq!(err.failure.code.id(), "DW0780");
    assert!(
        err.failure.message.contains("MATED"),
        "{}",
        err.failure.message
    );
    assert!(
        err.failure.message.contains("jigsaw socket `kit:socket`"),
        "the socket is named where a reviewer would look it up: {}",
        err.failure.message
    );
    assert!(
        err.failure.message.contains("z 7"),
        "and the plane it looks at: {}",
        err.failure.message
    );

    // The suspicion is the DETACHMENT and nothing else: the same two pieces,
    // seated where the solver mated them, assemble.
    let areas = vec![socket_area(vec![
        socket_piece([0, 0, 0], [false, true]),
        socket_piece([0, 0, 7], [true, false]),
    ])];
    faces::check(&areas, &registry).expect("the undisturbed chain is fine");
}

/// **Two pieces that touch and say nothing about the side they share is a
/// refusal, not a zero.**
///
/// The pieces here declare their ways on north and south and are placed side by
/// side on x, so the plane they actually meet across is one neither document has
/// an opinion about. That is the one thing this check exists for that it cannot
/// judge — so it says so, rather than counting zero examined faces and passing.
#[test]
fn a_pair_that_touches_and_declares_nothing_across_the_shared_plane_is_refused() {
    let (_dir, registry) = socket_library("silent");
    let areas = vec![socket_area(vec![
        socket_piece([0, 0, 0], [false, false]),
        socket_piece([7, 0, 0], [false, false]),
    ])];
    let err = faces::check(&areas, &registry)
        .expect_err("a pair nothing can judge is not a pair that passed");
    assert_eq!(err.failure.code.id(), "DW0780");
    assert!(
        err.failure
            .message
            .contains("NEITHER piece declares anything"),
        "{}",
        err.failure.message
    );
    // The plane, so a reviewer can go and look at it.
    assert!(
        err.failure.message.contains("x 6..6"),
        "{}",
        err.failure.message
    );
    // And both roads out, named: a piece says what its sides are in one of two
    // documented places, and the message names both rather than only the newer.
    assert!(
        err.failure.message.contains("spatial_contract")
            && err.failure.message.contains("connectors"),
        "{}",
        err.failure.message
    );
}

/// **A world whose pieces never touch is the one honest zero**, and it is stated
/// with the denominator that makes it readable: not *zero faces declared*, which
/// is a fact about documents, but *zero pairs*, which is a fact about the world
/// that was built.
#[test]
fn pieces_that_never_touch_report_zero_pairs_rather_than_zero_declarations() {
    let (_dir, registry) = socket_library("apart");
    let areas = vec![socket_area(vec![
        socket_piece([0, 0, 0], [false, false]),
        socket_piece([0, 0, 40], [false, false]),
    ])];
    let binding = faces::check(&areas, &registry).expect("two pieces far apart cannot mis-mate");
    assert_eq!(binding.pairs, 0);
    assert_eq!(binding.bound, 0);
    assert_eq!(
        binding.declared, 4,
        "the sockets are still read — the count is honest about what is there"
    );
    let finding = binding.finding(2, false).expect("a zero binding is stated");
    assert_eq!(finding.code, "DW0781");
    assert!(
        finding.message.contains("0 abutting pair(s)"),
        "and it says WHY the zero is a zero: {}",
        finding.message
    );
}
