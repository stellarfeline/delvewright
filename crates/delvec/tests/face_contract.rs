//! **Two pieces that were each approved alone, and do not assemble**
//! (ADR-0020 §3, spec-0036 §2.8; `DW0780`).
//!
//! The failure this exists for is not that a piece is wrong. Both pieces here
//! pass every prefab gate there is, individually — they are the same prefab. It
//! is the pair that is wrong: one declares a way out on the face they share, and
//! the piece on the other side of it does not answer.
//!
//! The artifacts are real. The prefabs are exported by `crates/grammar` from the
//! corpus program that declares a contract, loaded back through the engine's own
//! `PrefabRegistry`, and placed the way a campaign places them.

use delvewright_compiler::faces;
use delvewright_compiler::plan::{AreaPlacement, PiecePlacement, PlacedTemplate};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::solver::Rotation;
use delvewright_grammar::library::spatial_contract::spatial_contract;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

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
        }],
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
