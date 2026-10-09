//! **The spatial contract's obligations, each shown from both sides**
//! (spec-0036 §2, ADR-0020).
//!
//! Every test here is a pair: a building whose blocks disagree with what its
//! author declared, and the same building one course apart where they agree. A
//! gate that only ever goes green on the fixture that was written for it proves
//! nothing about a gate, and a gate that only ever goes red proves nothing about
//! a building.
//!
//! The models are built cell by cell rather than derived from grammar programs.
//! That is deliberate: the checker's input is (block grid, resolved contract),
//! the same pair `delvec prefab` hands it for a piece nobody generated, so the
//! fixtures are written in that vocabulary and not in the one door's. The
//! corpus example goes through the other door end to end in `tests/contract.rs`
//! and through the CLI in `tests/cli.rs`.

use std::collections::{BTreeMap, BTreeSet};

use delvec::grammar::block::BlockState;
use delvec::grammar::contract::{check, exterior_faces};
use delvec::grammar::geom::Box3;
use delvec::grammar::model::VoxelModel;
use delvec::schem::prefab::{
    ContractBar, ContractEdge, ContractNoBody, ContractSpace, ContractVolume, ContractWay, Region,
    SpatialContract,
};

// ---------------------------------------------------------------------------
// Fixture vocabulary
// ---------------------------------------------------------------------------

/// A block grid to write buildings into.
struct Build {
    model: VoxelModel,
}

impl Build {
    fn new(size: [u32; 3]) -> Build {
        Build {
            model: VoxelModel::new(Box3::at_origin(size)),
        }
    }

    /// Fill an inclusive range with stone.
    fn stone(&mut self, from: [i32; 3], to: [i32; 3]) -> &mut Build {
        self.paint(from, to, "minecraft:stone_bricks")
    }

    /// Fill an inclusive range with a named block.
    fn paint(&mut self, from: [i32; 3], to: [i32; 3], name: &str) -> &mut Build {
        let block = BlockState::simple(name);
        for x in from[0]..=to[0] {
            for y in from[1]..=to[1] {
                for z in from[2]..=to[2] {
                    if self.model.get([x, y, z]).is_some() {
                        self.model.set([x, y, z], &block).unwrap();
                    }
                }
            }
        }
        self
    }

    /// Cut an inclusive range back to air.
    fn air(&mut self, from: [i32; 3], to: [i32; 3]) -> &mut Build {
        self.paint(from, to, "minecraft:air")
    }

    /// A solid box with a hollow inside it: walls, floor and ceiling one course
    /// thick, `from`/`to` inclusive over the WHOLE box.
    fn room(&mut self, from: [i32; 3], to: [i32; 3]) -> &mut Build {
        self.stone(from, to);
        self.air(
            [from[0] + 1, from[1] + 1, from[2] + 1],
            [to[0] - 1, to[1] - 1, to[2] - 1],
        );
        self
    }
}

fn region(from: [i32; 3], to: [i32; 3]) -> Region {
    Region { from, to }
}

fn space(envelope: &str, boxes: Vec<Region>) -> ContractSpace {
    ContractSpace {
        envelope: envelope.to_string(),
        boxes,
    }
}

fn edge(a: &str, b: &str, class: &str) -> ContractEdge {
    ContractEdge {
        a: a.to_string(),
        b: b.to_string(),
        class: class.to_string(),
        rise: if class == "vision" { None } else { Some(0) },
        via: None,
        bar: None,
        way: None,
    }
}

/// Declare an edge contingent: the region, its sign and the block it is made
/// of (spec-0042 §2.1).
fn with_way(
    mut e: ContractEdge,
    name: &str,
    opens: &str,
    boxes: Vec<Region>,
    block: &str,
) -> ContractEdge {
    e.way = Some(ContractWay {
        opens: opens.to_string(),
        region: name.to_string(),
        boxes,
        // A hand-built piece has no palette to have named a role in, and the
        // checker reads the block either way.
        role: None,
        block: block.to_string(),
    });
    e
}

fn with_via(mut e: ContractEdge, name: &str, boxes: Vec<Region>) -> ContractEdge {
    e.via = Some(ContractVolume {
        region: name.to_string(),
        boxes,
    });
    e
}

fn with_rise(mut e: ContractEdge, rise: i64) -> ContractEdge {
    e.rise = Some(rise);
    e
}

fn with_bar(mut e: ContractEdge, name: &str, boxes: Vec<Region>, block: &str) -> ContractEdge {
    e.bar = Some(ContractBar {
        region: name.to_string(),
        boxes,
        block: block.to_string(),
    });
    e
}

fn no_body(reason: &str, boxes: Vec<Region>) -> ContractNoBody {
    ContractNoBody {
        reason: reason.to_string(),
        boxes,
    }
}

fn contract(entry: &str) -> SpatialContract {
    SpatialContract {
        entry: entry.to_string(),
        spaces: BTreeMap::new(),
        no_body: BTreeMap::new(),
        edges: Vec::new(),
        faces: Vec::new(),
        no_body_majority_ack: None,
    }
}

/// The verdict of one gate, by id, or a panic naming every gate there was.
fn gate<'a>(report: &'a delvec::grammar::ContractReport, id: &str) -> &'a delvec::grammar::Gate {
    report
        .gates
        .iter()
        .find(|g| g.id == id)
        .unwrap_or_else(|| panic!("no gate {id:?} in {:?}", report.gates))
}

fn no_anchors() -> BTreeMap<String, [i32; 3]> {
    BTreeMap::new()
}

// ---------------------------------------------------------------------------
// The building every envelope test is one course apart from
// ---------------------------------------------------------------------------

/// A hall 11x6x9 with a doorway in its west wall, and the contract that
/// describes it honestly.
///
/// The doorway is a claimed opening, because that is the only thing that excuses
/// a hole in an `enclosed` envelope: an edge on its own would be a demand the
/// defect can meet — an eleven-course missing wall declares an exterior edge as
/// easily as a door does.
fn hall() -> (Build, SpatialContract) {
    let mut b = Build::new([11, 6, 9]);
    b.room([0, 0, 0], [10, 5, 8]);
    // The door: two courses of the west wall, cut through.
    b.air([0, 1, 4], [0, 2, 4]);
    let mut c = contract("hall");
    c.spaces.insert(
        "hall".to_string(),
        space("enclosed", vec![region([1, 1, 1], [9, 4, 7])]),
    );
    c.edges.push(with_via(
        edge("hall", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    (b, c)
}

/// **Closure, both ways.** The motivating defect of trial-0001 at fixture scale:
/// a wall that is simply not there. The contract does not change; the blocks do.
#[test]
fn a_missing_wall_course_reds_closure_and_the_same_hall_with_it_is_green() {
    let (b, c) = hall();
    let green = check(&b.model, &c, &no_anchors());
    let g = gate(&green, "contract-closure");
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 254, "boundary cells examined");

    // Strip the top course of the south wall — the shape trial-0001 shipped
    // twice, at eleven courses and both flanks.
    let (mut broken, c) = hall();
    broken.air([1, 4, 0], [9, 4, 0]);
    let red = check(&broken.model, &c, &no_anchors());
    let g = gate(&red, "contract-closure");
    assert!(!g.passed(), "{}", g.detail);
    assert_eq!(
        g.bound, 254,
        "the same boundary is examined either way — a red is a disagreement, not a smaller gate"
    );
    assert!(
        g.detail.contains("9 of its boundary cell(s)"),
        "{}",
        g.detail
    );
    assert!(g.detail.contains("[1,4,0]"), "{}", g.detail);
}

/// **The declaration is checked against the bytes, never read out of them.**
///
/// The identical block grid, judged under two different contracts, gives two
/// different verdicts. Nothing here could be true if spaces were inferred: an
/// inferred contract agrees with its own building by construction.
#[test]
fn the_same_bytes_pass_under_one_contract_and_fail_under_another() {
    let (b, honest) = hall();
    assert!(check(&b.model, &honest, &no_anchors()).is_pass());

    // The same hall, declared as two abutting rooms with nothing between them
    // and no edge joining them. The blocks did not move; the claim did.
    let mut split = contract("west");
    split.spaces.insert(
        "west".to_string(),
        space("enclosed", vec![region([1, 1, 1], [4, 4, 7])]),
    );
    split.spaces.insert(
        "east".to_string(),
        space("enclosed", vec![region([5, 1, 1], [9, 4, 7])]),
    );
    split.edges.push(with_via(
        edge("west", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    let report = check(&b.model, &split, &no_anchors());
    let g = gate(&report, "contract-reachability");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("space east"),
        "the room with no declared way into it is named: {}",
        g.detail
    );
}

/// **An edge is a checked claim, not decoration** (spec-0036 §2.5).
///
/// Two rooms, a real doorway between them, and the walk is confined to the
/// declared graph — so deleting the edge reds reachability even though a body
/// can plainly walk it. The physical reading was rejected precisely because
/// under it this test cannot exist.
#[test]
fn deleting_an_edge_reds_reachability_though_the_blocks_did_not_move() {
    let mut b = Build::new([15, 6, 9]);
    b.room([0, 0, 0], [7, 5, 8]);
    b.room([7, 0, 0], [14, 5, 8]);
    b.air([7, 1, 4], [7, 2, 4]); // the doorway through the shared wall
    b.air([0, 1, 4], [0, 2, 4]); // the way in

    let mut c = contract("west");
    c.spaces.insert(
        "west".to_string(),
        space("enclosed", vec![region([1, 1, 1], [6, 4, 7])]),
    );
    c.spaces.insert(
        "east".to_string(),
        space("enclosed", vec![region([8, 1, 1], [13, 4, 7])]),
    );
    c.edges.push(with_via(
        edge("west", "exterior", "walk"),
        "front-door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    let inner = with_via(
        edge("west", "east", "walk"),
        "inner-door",
        vec![region([7, 1, 4], [7, 2, 4])],
    );
    c.edges.push(inner.clone());

    let green = check(&b.model, &c, &no_anchors());
    assert!(green.is_pass(), "{:#?}", green.gates);
    assert!(gate(&green, "contract-reachability").bound > 0);

    c.edges.retain(|e| e.class != "walk" || e.b == "exterior");
    let red = check(&b.model, &c, &no_anchors());
    let g = gate(&red, "contract-reachability");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("space east"), "{}", g.detail);
}

/// **The level relation, and both numbers in the message** (spec-0036 §2.4).
///
/// Three of the four recorded bell-zone drifts move a landing by one course and
/// stay green on every topology obligation, because a walk steps ±1 either way.
/// `rise` is the whole reason they red.
#[test]
fn a_stair_whose_declared_rise_is_one_off_reds_with_both_numbers_named() {
    let (mut b, mut c) = stair_piece();
    // Honest first.
    let green = check(&b.model, &c, &no_anchors());
    assert!(green.is_pass(), "{:#?}", green.gates);

    // One course off, and nothing else changes.
    for e in &mut c.edges {
        if e.class == "stair" {
            e.rise = Some(2);
        }
    }
    let red = check(&b.model, &c, &no_anchors());
    let g = gate(&red, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("declares rise 2"), "{}", g.detail);
    assert!(g.detail.contains("measure 3"), "{}", g.detail);
    let _ = &mut b;
}

/// A foot, a head three courses up, and the flight between them.
fn stair_piece() -> (Build, SpatialContract) {
    let mut b = Build::new([7, 9, 13]);
    b.stone([0, 0, 0], [6, 8, 12]);
    b.air([1, 1, 1], [5, 4, 4]); // foot, floor at y0 -> standable y1
    b.air([1, 4, 8], [5, 7, 11]); // head, floor at y3 -> standable y4
    // Treads: three steps up through z 5..7.
    b.air([1, 1, 5], [5, 4, 5]);
    b.air([1, 2, 6], [5, 5, 6]);
    b.air([1, 3, 7], [5, 6, 7]);
    b.paint([1, 1, 6], [5, 1, 6], "minecraft:stone_bricks");
    b.paint([1, 1, 7], [5, 2, 7], "minecraft:stone_bricks");
    b.air([0, 1, 2], [0, 2, 2]); // the way in

    let mut c = contract("foot");
    c.spaces.insert(
        "foot".to_string(),
        space("enclosed", vec![region([1, 1, 1], [5, 4, 4])]),
    );
    c.spaces.insert(
        "head".to_string(),
        space("enclosed", vec![region([1, 4, 8], [5, 7, 11])]),
    );
    c.edges.push(with_via(
        edge("foot", "exterior", "walk"),
        "front-door",
        vec![region([0, 1, 2], [0, 2, 2])],
    ));
    c.edges.push(with_via(
        with_rise(edge("foot", "head", "stair"), 3),
        "flight",
        vec![region([1, 1, 5], [5, 6, 7])],
    ));
    (b, c)
}

/// **A `via` off the boundary its endpoints share is refused** (spec-0036 §1a,
/// AC11) — the five-boxes-over-the-breaches cheat.
#[test]
fn an_opening_claimed_away_from_the_shared_boundary_is_refused() {
    let (b, mut c) = hall();
    // Move the door's claimed cells into the middle of the hall, where they
    // excuse nothing and touch nothing.
    for e in &mut c.edges {
        e.via = Some(ContractVolume {
            region: "door".to_string(),
            boxes: vec![region([5, 2, 4], [5, 2, 4])],
        });
    }
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("inside space \"hall\""), "{}", g.detail);
}

/// **One floor per space** (spec-0036 §1a): merging a stair's two ends into one
/// space is refused, because a merge with no internal edge has nothing to carry
/// a `rise` and hides the seam the level check exists to find.
#[test]
fn merging_a_stairs_two_ends_into_one_space_is_refused() {
    let (b, _) = stair_piece();
    let mut c = contract("flight");
    c.spaces.insert(
        "flight".to_string(),
        space(
            "enclosed",
            vec![
                region([1, 1, 1], [5, 4, 4]),
                region([1, 1, 5], [5, 6, 7]),
                region([1, 4, 8], [5, 7, 11]),
            ],
        ),
    );
    c.edges.push(with_via(
        edge("flight", "exterior", "walk"),
        "front-door",
        vec![region([0, 1, 2], [0, 2, 2])],
    ));
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("is ONE floor"), "{}", g.detail);
}

/// **A covered space declared `open` is taken as declared** (spec-0098 §14,
/// a ruling: enclosed spaces are the author's declaration, and the
/// check confirms intent rather than restricting it). The hall under its roof,
/// declared `open`, is a covered space — a pavilion — and closure examines
/// nothing of it; the covered cells are stated in the enumeration. The same
/// hall declared `enclosed` is examined (the perturbation: the declaration is
/// what binds the gate).
#[test]
fn a_covered_space_declared_open_is_taken_as_declared() {
    let (b, mut c) = hall();
    let enclosed = check(&b.model, &c, &no_anchors());
    assert!(
        gate(&enclosed, "contract-closure").bound > 0,
        "declared enclosed, the hall is examined"
    );
    c.spaces.get_mut("hall").unwrap().envelope = "open".to_string();
    let report = check(&b.model, &c, &no_anchors());
    assert!(
        !report
            .gates
            .iter()
            .any(|g| g.id == "contract-closure" && !g.passed()),
        "{:#?}",
        report.gates
    );
    assert!(
        report
            .enumeration
            .iter()
            .any(|e| e.contains("blocks overhead") && e.contains("a covered space")),
        "the covered cells are stated: {:?}",
        report.enumeration
    );

    // The same claim over a yard with sky above it is fine.
    let mut yard = Build::new([11, 6, 9]);
    yard.stone([0, 0, 0], [10, 0, 8]);
    yard.stone([0, 1, 0], [0, 4, 8]);
    yard.stone([10, 1, 0], [10, 4, 8]);
    yard.stone([0, 1, 0], [10, 4, 0]);
    yard.stone([0, 1, 8], [10, 4, 8]);
    let mut open = contract("yard");
    open.spaces.insert(
        "yard".to_string(),
        space("open_top", vec![region([1, 1, 1], [9, 4, 7])]),
    );
    yard.air([0, 1, 4], [0, 2, 4]);
    open.edges.push(with_via(
        edge("yard", "exterior", "walk"),
        "gate",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    let report = check(&yard.model, &open, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);
    assert!(
        report.enumeration.iter().any(|e| e.contains("`open_top`")),
        "an open envelope is enumerated by name: {:?}",
        report.enumeration
    );
}

// ---------------------------------------------------------------------------
// §2.6 — the out-of-walk kinds, and the script that defeated the first draft
// ---------------------------------------------------------------------------

/// A hall with a walled recess beside it and a gallery stranded over it.
fn hall_with_recess_and_gallery() -> (Build, SpatialContract) {
    let mut b = Build::new([15, 12, 9]);
    b.room([0, 0, 0], [10, 5, 8]);
    b.air([0, 1, 4], [0, 2, 4]);
    // A recess: floor with air over it, walled on every side. Nothing reaches
    // it and nothing ever will — which is what makes it decoration.
    b.stone([11, 0, 0], [14, 5, 8]);
    b.air([12, 1, 1], [13, 2, 7]);
    // A gallery: an upper storey over the hall, walled and roofed like the hall
    // below it, with nothing leading up. This one is a DEFECT, not decoration —
    // and being interior is what keeps it out of every kind.
    b.room([0, 5, 0], [10, 11, 8]);
    let mut c = contract("hall");
    c.spaces.insert(
        "hall".to_string(),
        space("enclosed", vec![region([1, 1, 1], [9, 4, 7])]),
    );
    c.edges.push(with_via(
        edge("hall", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    (b, c)
}

/// **`sealed` demands its own closure, and stranding cannot supply it.**
///
/// The walled recess classifies; the stranded gallery does not, and it is the
/// same declaration in both cases. This is the 90-line adversary in miniature:
/// a script that reads the checker's red list and declares every unreached cell
/// `no_body` buys nothing, because what `sealed` asks for is exactly the fact
/// stranding cannot provide.
#[test]
fn a_walled_recess_seals_and_a_stranded_gallery_declared_the_same_way_reds() {
    let (b, mut c) = hall_with_recess_and_gallery();
    c.no_body.insert(
        "recess".to_string(),
        no_body(
            "a walled void behind the north aisle",
            vec![region([12, 1, 1], [13, 2, 7])],
        ),
    );
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.detail.contains("sealed"), "{}", g.detail);
    assert!(
        report
            .enumeration
            .iter()
            .any(|e| e.contains("no_body \"recess\": sealed")),
        "{:?}",
        report.enumeration
    );

    // The adversary: sweep the gallery into the same escape hatch.
    let (b, mut c) = hall_with_recess_and_gallery();
    c.no_body.insert(
        "gallery".to_string(),
        no_body("nobody goes up there", vec![region([1, 6, 1], [9, 6, 7])]),
    );
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("qualifies for NOTHING"), "{}", g.detail);
    assert!(
        g.detail.contains("its own boundary is not closed"),
        "the red says which demand it failed: {}",
        g.detail
    );
}

/// **`posted` is proved per cell**, so one decoy anchor over a stranded shelf is
/// not a blanket.
#[test]
fn one_anchor_covers_the_cells_within_reach_of_it_and_no_others() {
    let (b, mut c) = hall_with_recess_and_gallery();
    c.no_body.insert(
        "gallery".to_string(),
        no_body(
            "perches for the watchers",
            vec![region([1, 6, 1], [9, 6, 7])],
        ),
    );
    let mut one = BTreeMap::new();
    one.insert("anchor/watcher".to_string(), [5, 6, 4]);
    let report = check(&b.model, &c, &one);
    let g = gate(&report, "contract-no-body");
    assert!(
        !g.passed(),
        "one anchor cannot post a 63-cell gallery: {}",
        g.detail
    );

    // An anchor on every cell within reach of one — the shape `rafter_hall`'s
    // alternation was missing — and it classifies.
    let mut every = BTreeMap::new();
    let mut n = 0;
    for x in [2, 5, 8] {
        for z in [2, 5] {
            n += 1;
            every.insert(format!("anchor/watcher-{n}"), [x, 6, z]);
        }
    }
    let mut narrow = c.clone();
    narrow.no_body.insert(
        "gallery".to_string(),
        no_body(
            "perches for the watchers",
            vec![region([1, 6, 1], [9, 6, 6])],
        ),
    );
    let report = check(&b.model, &narrow, &every);
    let g = gate(&report, "contract-no-body");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.detail.contains("posted"), "{}", g.detail);
}

/// **`facade` demands exterior air, which an interior stranding cannot supply**
/// (spec-0036 §2.6, AC14).
#[test]
fn a_wall_head_is_facade_and_the_same_declaration_inside_the_hall_is_not() {
    let mut b = Build::new([11, 10, 9]);
    b.room([0, 0, 0], [10, 5, 8]);
    b.air([0, 1, 4], [0, 2, 4]);
    // A parapet course standing proud of the roof: standable, open to the sky,
    // and ordinary stonework nobody was ever meant to be on.
    b.stone([0, 6, 0], [0, 6, 8]);
    let mut c = contract("hall");
    c.spaces.insert(
        "hall".to_string(),
        space("enclosed", vec![region([1, 1, 1], [9, 4, 7])]),
    );
    c.edges.push(with_via(
        edge("hall", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    c.no_body.insert(
        "wall-head".to_string(),
        no_body("the parapet course", vec![region([0, 7, 0], [0, 7, 8])]),
    );
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.detail.contains("facade"), "{}", g.detail);
    // ...and it manufactures no closure or reachability red: the signal the
    // three-kind taxonomy diluted is what this kind restores.
    assert!(gate(&report, "contract-closure").passed());
    assert!(gate(&report, "contract-reachability").passed());

    // The adversary direction: the same words over interior cells. A region
    // nested in a space can never be `facade`, and an enclosed interior can
    // never be reached by the air outside.
    let (b2, mut c2) = hall_with_recess_and_gallery();
    c2.no_body.insert(
        "gallery".to_string(),
        no_body(
            "exterior dressing, honestly",
            vec![region([1, 6, 1], [9, 6, 7])],
        ),
    );
    let report = check(&b2.model, &c2, &no_anchors());
    assert!(!gate(&report, "contract-no-body").passed());
}

// ---------------------------------------------------------------------------
// spec-0047 — the kind belongs to the cell
// ---------------------------------------------------------------------------

/// **The pier**: a hall with a doorway, and free-standing ruined masonry east
/// of it.
///
/// The deck stands in exterior air; one cell stands inside a void the pier's
/// own blocks close on every side. The two facts are true of the same piece of
/// scenery, and no rule can carve a box around the second, so the region an
/// author writes holds both.
fn pier() -> Build {
    let mut b = Build::new([25, 8, 9]);
    // The hall, roofed at the very top of the model so nothing stands on its
    // leads — the piece under test is the pier, not a second facade.
    b.room([0, 0, 0], [10, 7, 8]);
    b.air([0, 1, 4], [0, 2, 4]);
    // The masonry: standing free, its deck two courses under the model's roof.
    b.stone([14, 0, 2], [24, 4, 6]);
    // The void the weighted ruin mix sealed. Two cells of air; the lower one is
    // standable, and the pier's own blocks are on all six sides of both.
    b.air([19, 2, 4], [19, 3, 4]);
    b
}

/// The pier's deck: 55 cells standable in exterior air.
fn deck() -> Region {
    region([14, 5, 2], [24, 5, 6])
}

/// The void inside the masonry: two cells, the lower one standable.
fn void() -> Region {
    region([19, 2, 4], [19, 3, 4])
}

/// The hall's half of the contract, which never changes across the pair.
fn pier_hall() -> SpatialContract {
    let mut c = contract("hall");
    c.spaces.insert(
        "hall".to_string(),
        space("enclosed", vec![region([1, 1, 1], [9, 6, 7])]),
    );
    c.edges.push(with_via(
        edge("hall", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    c
}

/// Every gate's id and verdict, in order — what "the same verdict" means when
/// two contracts are compared.
fn states(report: &delvec::grammar::ContractReport) -> Vec<(&'static str, bool)> {
    report.gates.iter().map(|g| (g.id, g.passed())).collect()
}

/// **The kind is a fact about the blocks, so it is decided at the cell.**
///
/// One region over the whole pier. The deck is exterior dressing and the void
/// is walled masonry, and both are true at once — which under a region-keyed
/// verdict meant the region qualified for NOTHING and the piece could not
/// declare its contract at all. Nothing about the demands changed to make this
/// green: each is asked of the cell it was always a fact about.
#[test]
fn one_region_over_a_deck_and_a_walled_void_earns_both_kinds_cell_by_cell() {
    let b = pier();
    let mut c = pier_hall();
    c.no_body.insert(
        "arcade".to_string(),
        no_body("the ruined west arcade", vec![deck(), void()]),
    );
    let report = check(&b.model, &c, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);

    let g = gate(&report, "contract-no-body");
    assert_eq!(
        g.bound, 1,
        "one region declared, and the region is still the unit"
    );
    assert!(
        g.detail.ends_with("55 cell(s) facade, 1 cell(s) sealed"),
        "{}",
        g.detail
    );
    assert!(
        report.enumeration.iter().any(|e| e
            == "no_body \"arcade\": mixed — 1 standable cell(s) sealed, 55 standable \
                          cell(s) facade, the kind computed per cell"),
        "the breakdown a reviewer reads is per region and by cell count: {:?}",
        report.enumeration
    );

    // Neither kind bound to nothing, which is what would make the pair vacuous:
    // a void with no standable cell, or a deck with none, and one of the two
    // demands would never have been asked.
    let majority = gate(&report, "contract-no-body-majority");
    assert!(
        majority.detail.contains("56 of 120 standable cell(s)"),
        "{}",
        majority.detail
    );
}

/// **Box-drawing is dead as a lever, in both directions.**
///
/// The identical bytes under the one region an author can write and under the
/// split no rule can author: same gate states, same per-kind cell counts, same
/// covered population. A split buys nothing and a merge hides nothing.
#[test]
fn the_same_cells_earn_the_same_kinds_split_or_merged() {
    let b = pier();

    let mut merged = pier_hall();
    merged.no_body.insert(
        "arcade".to_string(),
        no_body("the ruined west arcade", vec![deck(), void()]),
    );

    let mut split = pier_hall();
    split.no_body.insert(
        "arcade-deck".to_string(),
        no_body("the arcade's deck", vec![deck()]),
    );
    split.no_body.insert(
        "arcade-void".to_string(),
        no_body("the void inside the arcade's masonry", vec![void()]),
    );

    let one = check(&b.model, &merged, &no_anchors());
    let two = check(&b.model, &split, &no_anchors());
    assert_eq!(
        states(&one),
        states(&two),
        "the verdict moved with the boxes"
    );
    assert!(one.is_pass() && two.is_pass(), "{:#?}", states(&one));

    let tail = "55 cell(s) facade, 1 cell(s) sealed";
    assert!(gate(&one, "contract-no-body").detail.ends_with(tail));
    assert!(gate(&two, "contract-no-body").detail.ends_with(tail));

    // The invariance is only an invariance if the two contracts cover the same
    // cells; two populations compared would say nothing about box-drawing.
    assert_eq!(
        gate(&one, "contract-no-body-majority").detail,
        gate(&two, "contract-no-body-majority").detail,
        "the two contracts cover different standable cells"
    );
}

/// **`sealed` is bought with the piece's own blocks, never with the edge of the
/// world** (spec-0047 §1.4, AC4).
///
/// The mislabelling hatch the re-binding retires: a blanket region declared out
/// to the model's outer layer — sky included — had its "closure" supplied by the
/// end of the model, so 55 flood-reached deck cells classified `sealed` and
/// escaped the `facade` cell-share a reviewer reads. Per cell they cannot: a
/// component holding an outer-layer cell is closed by nothing this piece built.
/// The gate stays green, which is the point — this is a strengthening of what
/// the reviewer is shown, not of what passes.
#[test]
fn a_blanket_out_to_the_models_edge_classifies_facade_and_not_sealed() {
    let b = pier();
    let mut c = pier_hall();
    c.no_body.insert(
        "surround".to_string(),
        no_body(
            "everything east of the hall, out to the edge of the model",
            vec![region([11, 0, 0], [24, 7, 8])],
        ),
    );
    let report = check(&b.model, &c, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);

    // The clause under test fires: the blanket really does reach the outer
    // layer. Without that this fixture would witness an ordinary walled void.
    let blanket = cells_of(&region([11, 0, 0], [24, 7, 8]));
    assert!(
        blanket
            .iter()
            .any(|c| c[0] == 24 || c[1] == 0 || c[1] == 7 || c[2] == 0 || c[2] == 8),
        "the blanket must touch the model's outer layer"
    );
    assert_eq!(blanket.len(), 1008, "the blanket's size, stated");

    let g = gate(&report, "contract-no-body");
    assert!(
        g.detail.ends_with("55 cell(s) facade, 1 cell(s) sealed"),
        "the 55 flood-reached cells are `facade`, and only the walled void is \
         `sealed`: {}",
        g.detail
    );
    assert!(
        report
            .enumeration
            .iter()
            .any(|e| e.contains("no_body \"surround\": mixed")
                && e.contains("55 standable cell(s) facade")),
        "the facade cell share is complete — no flood-reached cell hides under \
         `sealed`: {:?}",
        report.enumeration
    );
}

/// Every cell an inclusive range covers — the test's own arithmetic, so a size
/// claim in an assertion is measured rather than asserted.
fn cells_of(r: &Region) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for x in r.from[0]..=r.to[0] {
        for y in r.from[1]..=r.to[1] {
            for z in r.from[2]..=r.to[2] {
                out.insert([x, y, z]);
            }
        }
    }
    out
}

/// **Stranding cannot buy `sealed` at cell granularity either** (AC3).
///
/// The gallery's cells are unreachable, which is the fact that made them a
/// finding — and `sealed` asks for a different one: that the air the cell stands
/// in is closed by this piece's blocks and lies wholly inside the declared
/// out-of-walk cells. The gallery's air runs up into five more courses nobody
/// declared, so every cell of it is refused, and the red says so per cell rather
/// than per region.
#[test]
fn the_stranded_gallery_is_refused_cell_by_cell_and_the_red_names_them() {
    let (b, mut c) = hall_with_recess_and_gallery();
    c.no_body.insert(
        "gallery".to_string(),
        no_body("nobody goes up there", vec![region([1, 6, 1], [9, 6, 7])]),
    );
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail
            .contains("qualifies for NOTHING on 63 of its 63 standable cell(s)"),
        "the red counts the cells, not the regions: {}",
        g.detail
    );
    assert!(
        g.detail.contains(
            "the air it stands in runs on into cells the contract does not declare out of walk"
        ),
        "the `sealed` clause names the component that escapes: {}",
        g.detail
    );
    assert!(
        g.detail.contains("the region declares no anchor at all"),
        "{}",
        g.detail
    );
    assert!(
        g.detail
            .contains("the air outside the piece does not reach it"),
        "{}",
        g.detail
    );
    assert!(
        g.detail.contains("[1,6,1]"),
        "the red sends someone to a place: {}",
        g.detail
    );

    // The component holding the gallery's air is NOT wholly inside the declared
    // cells — which is what makes this a stranding rather than a walled void,
    // and what would make the test vacuous if it were.
    let mut walled = c.clone();
    walled.no_body.insert(
        "gallery".to_string(),
        no_body(
            "the whole upper hollow",
            vec![region([1, 6, 1], [9, 10, 7])],
        ),
    );
    let report = check(&b.model, &walled, &no_anchors());
    assert!(
        gate(&report, "contract-no-body").passed(),
        "declaring the WHOLE component is what `sealed` asks for, and a stranding \
         is exactly the case that cannot: {}",
        gate(&report, "contract-no-body").detail
    );
}

// ---------------------------------------------------------------------------
// §2.4 — the other three classes
// ---------------------------------------------------------------------------

/// **A drop is one-way, and a bar bars.** Both classes shown failing on the
/// building that does not do what the class says.
#[test]
fn a_walkable_slope_declared_a_drop_and_a_bar_that_bars_nothing_both_red() {
    // A ledge and a floor below it, with no way back up.
    let mut b = Build::new([9, 10, 13]);
    b.stone([0, 0, 0], [8, 9, 12]);
    b.air([1, 5, 1], [7, 8, 4]); // ledge, floor at y4 -> standable y5
    b.air([1, 1, 5], [7, 4, 11]); // pit, floor at y0 -> standable y1
    b.air([1, 5, 5], [7, 8, 11]); // the shaft over the pit
    b.air([0, 5, 2], [0, 6, 2]); // the way in

    let mut c = contract("ledge");
    c.spaces.insert(
        "ledge".to_string(),
        space("enclosed", vec![region([1, 5, 1], [7, 8, 4])]),
    );
    c.spaces.insert(
        "pit".to_string(),
        space("enclosed", vec![region([1, 1, 5], [7, 4, 11])]),
    );
    c.edges.push(with_via(
        edge("ledge", "exterior", "walk"),
        "door",
        vec![region([0, 5, 2], [0, 6, 2])],
    ));
    c.edges.push(with_via(
        with_rise(edge("ledge", "pit", "drop"), -4),
        "shaft",
        vec![region([1, 5, 5], [7, 8, 11])],
    ));
    let report = check(&b.model, &c, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);

    // Declare the same fall the other way round: `pit -> ledge` is not a drop,
    // and the gate says so with the direction named.
    let mut wrong = c.clone();
    for e in &mut wrong.edges {
        if e.class == "drop" {
            std::mem::swap(&mut e.a, &mut e.b);
            e.rise = Some(4);
        }
    }
    let report = check(&b.model, &wrong, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("a drop falls"), "{}", g.detail);
    let g = gate(&report, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("nothing falls from pit to ledge"),
        "{}",
        g.detail
    );
}

/// **`barred`, both halves**: the bar stops a body while it stands, and taking
/// it away is what lets one through.
#[test]
fn a_bar_that_bars_is_green_and_an_open_doorway_called_barred_is_not() {
    let mut b = Build::new([15, 6, 9]);
    b.room([0, 0, 0], [7, 5, 8]);
    b.room([7, 0, 0], [14, 5, 8]);
    b.air([7, 1, 4], [7, 2, 4]);
    b.paint([7, 1, 4], [7, 2, 4], "minecraft:iron_bars");
    b.air([0, 1, 4], [0, 2, 4]);

    let mut c = contract("west");
    c.spaces.insert(
        "west".to_string(),
        space("enclosed", vec![region([1, 1, 1], [6, 4, 7])]),
    );
    c.spaces.insert(
        "east".to_string(),
        space("enclosed", vec![region([8, 1, 1], [13, 4, 7])]),
    );
    c.edges.push(with_via(
        edge("west", "exterior", "walk"),
        "front-door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    c.edges.push(with_bar(
        edge("west", "east", "barred"),
        "gate",
        vec![region([7, 1, 4], [7, 2, 4])],
        "minecraft:iron_bars",
    ));
    let report = check(&b.model, &c, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);
    assert!(
        report
            .enumeration
            .iter()
            .any(|e| e.contains("opened bars") && e.contains("gate")),
        "the bar the walk had to open is named: {:?}",
        report.enumeration
    );

    // Take the iron away and the same declaration is a lie.
    let mut open = b;
    open.air([7, 1, 4], [7, 2, 4]);
    let report = check(&open.model, &c, &no_anchors());
    let g = gate(&report, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("does not bar anything"), "{}", g.detail);
}

// ---------------------------------------------------------------------------
// §2.2, §2.7, §2.8, §2.9
// ---------------------------------------------------------------------------

/// **Coverage**: floor the contract does not account for is floor nobody decided
/// about.
#[test]
fn a_standable_cell_in_nothing_reds_coverage() {
    let (mut b, c) = hall();
    // A cupboard hollowed out of the east wall: standable, and in no declared
    // anything.
    b.air([10, 1, 3], [10, 2, 5]);
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-coverage");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("are in NOTHING"), "{}", g.detail);
    assert!(g.bound > 0);
}

/// **Anchors resolve to contract elements**, and the element is recorded.
#[test]
fn an_anchor_outside_every_declared_element_reds_and_one_inside_names_its_element() {
    let (b, c) = hall();
    let mut anchors = BTreeMap::new();
    anchors.insert("anchor/altar".to_string(), [5, 1, 4]);
    let report = check(&b.model, &c, &anchors);
    let g = gate(&report, "contract-anchors");
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 1);
    assert!(g.detail.contains("1 in a space"), "{}", g.detail);

    let mut adrift = BTreeMap::new();
    adrift.insert("anchor/altar".to_string(), [10, 5, 0]);
    let report = check(&b.model, &c, &adrift);
    let g = gate(&report, "contract-anchors");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("land in nothing"), "{}", g.detail);
}

/// **The face contract counts doors, not standable cells** (spec-0036 §2.8).
///
/// The hall has one declared way out and 63 cells of standable floor, some of
/// them against a region face. The old approach heuristic counted the latter.
#[test]
fn the_exterior_face_contract_binds_to_declared_ways_out() {
    let (b, c) = hall();
    let faces = exterior_faces(&b.model, &c);
    assert_eq!(faces.len(), 1, "{faces:?}");
    assert_eq!(faces[0].dir.as_str(), "west");
    assert_eq!(faces[0].class, "walk");
    assert_eq!(faces[0].cells.len(), 2, "a door two courses high");

    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-exterior-faces");
    assert!(g.passed(), "{}", g.detail);
    assert_eq!(g.bound, 1, "one declared way out, not 47 standable cells");
}

/// **A declared via is a face on the plane it lies in** — a way cut at a
/// piece's corner has cells on two outer planes and is one door, not two.
///
/// The motivating shape is a four-wide passage whose cells run from `z = 0`:
/// read as "every plane a cell touches", its corner column exported a second,
/// north-facing way the site plan never allocated, and assembly refused the
/// piece for a connection it discovered. A via that is one column at a corner
/// still lies in two planes and exports both.
#[test]
fn a_via_at_a_corner_is_one_face_on_the_plane_it_lies_in() {
    // A 4x4x4 frame: floor and ceiling courses, air between; the room is the
    // three columns east of the way, the way is the west plane's z 0..2.
    let mut b = Build::new([4, 4, 4]);
    b.stone([0, 0, 0], [3, 0, 3]).stone([0, 3, 0], [3, 3, 3]);
    let mut c = contract("room");
    c.spaces.insert(
        "room".to_string(),
        space("enclosed", vec![region([1, 1, 0], [3, 2, 3])]),
    );
    c.edges.push(with_via(
        edge("room", "exterior", "walk"),
        "way",
        vec![region([0, 1, 0], [0, 2, 2])],
    ));
    let faces = exterior_faces(&b.model, &c);
    assert_eq!(faces.len(), 1, "{faces:?}");
    assert_eq!(faces[0].dir.as_str(), "west");
    assert_eq!(faces[0].cells.len(), 6, "three columns, two courses");

    // The one column at the corner lies in both planes, and is two faces.
    let mut column = contract("room");
    column.spaces.insert(
        "room".to_string(),
        space("enclosed", vec![region([1, 1, 0], [3, 2, 3])]),
    );
    column.edges.push(with_via(
        edge("room", "exterior", "walk"),
        "way",
        vec![region([0, 1, 0], [0, 2, 0])],
    ));
    let faces = exterior_faces(&b.model, &column);
    let dirs: Vec<&str> = faces.iter().map(|f| f.dir.as_str()).collect();
    assert_eq!(dirs, ["west", "north"], "{faces:?}");
}

/// **Two openings meeting at the room's corner are both openings** (spec-0098
/// §6b). A street with a way along its whole west side and another across its
/// north end, each answered in the piece's first layer as a place that owns
/// neither plane answers it: the two openings share the corner column, whose
/// cells touch the street only through a cell of the other opening.
/// `contract-well-formed` passes and each way exports exactly its declared
/// cells. The perturbations: drop the north way, and the west way's corner
/// column touches nothing and reds; thicken the west way outward by a course,
/// and its outer course — reached only through its own cells — reds. Vacuous
/// if the corner cells touched the street: asserted they do not.
#[test]
fn two_openings_at_a_corner_open_through_each_other() {
    let mut b = Build::new([10, 5, 10]);
    b.stone([0, 0, 0], [9, 0, 9]);
    let street = |west: i32| {
        let mut c = contract("street");
        c.spaces.insert(
            "street".to_string(),
            space("open", vec![region([west, 1, 1], [9, 3, 9])]),
        );
        c
    };
    let west_way = |c: &mut SpatialContract, x: [i32; 2]| {
        c.edges.push(with_via(
            edge("street", "exterior", "walk"),
            "west-way",
            vec![region([x[0], 1, 0], [x[1], 3, 9])],
        ));
    };
    let north_way = |c: &mut SpatialContract| {
        c.edges.push(with_via(
            edge("street", "exterior", "walk"),
            "north-way",
            vec![region([0, 1, 0], [9, 3, 0])],
        ));
    };
    let mut c = street(1);
    west_way(&mut c, [0, 0]);
    north_way(&mut c);
    // The corner column is in both ways and beside no cell of the street.
    let corner = [0, 1, 0];
    let room: BTreeSet<[i32; 3]> = (1..=9)
        .flat_map(|x| (1..=3).flat_map(move |y| (1..=9).map(move |z| [x, y, z])))
        .collect();
    assert!(
        [[1, 0, 0], [0, 0, 1], [0, 1, 0]]
            .iter()
            .all(|d| !room.contains(&[corner[0] + d[0], corner[1] + d[1], corner[2] + d[2]]))
    );
    let report = check(&b.model, &c, &no_anchors());
    let wf = gate(&report, "contract-well-formed");
    assert!(wf.passed(), "{}", wf.detail);
    let faces = exterior_faces(&b.model, &c);
    let dirs: Vec<&str> = faces.iter().map(|f| f.dir.as_str()).collect();
    assert_eq!(dirs, ["west", "north"], "{faces:?}");
    assert!(faces.iter().all(|f| f.cells.len() == 30), "{faces:?}");

    // Without the north way, the corner column touches nothing.
    let mut alone = street(1);
    west_way(&mut alone, [0, 0]);
    let report = check(&b.model, &alone, &no_anchors());
    let wf = gate(&report, "contract-well-formed");
    assert!(!wf.passed(), "{}", wf.detail);
    assert!(
        wf.detail.contains("3 of its opening's cells touch neither"),
        "{}",
        wf.detail
    );

    // A west way two courses thick: its outer course reaches the street only
    // through its own cells.
    let mut thick = street(2);
    west_way(&mut thick, [0, 1]);
    north_way(&mut thick);
    let report = check(&b.model, &thick, &no_anchors());
    let wf = gate(&report, "contract-well-formed");
    assert!(!wf.passed(), "{}", wf.detail);
    assert!(wf.detail.contains("touch neither"), "{}", wf.detail);
}

/// **A stair's face is its via's footprint on the plane, treads included;
/// a barred edge's face is its bar** (spec-0098). A cellar entered through a
/// hole the floor above cuts: the stair's top step stands in the answering
/// layer under the hole, and the face it exports covers the whole hole, step
/// and all — the crossing the neighbour meets. A `walk` edge over the same
/// cells exports only the passable ones (the perturbation: the class is what
/// admits the tread). A barred edge with no via faces out through its bar.
#[test]
fn a_stair_faces_out_with_its_treads_and_a_bar_through_its_gate() {
    // 5x4x3: floor, a room, the top layer y 3 is the answering layer, with the
    // stair's top step at x 3 under the hole x 1..3.
    let mut b = Build::new([5, 4, 3]);
    b.stone([0, 0, 0], [4, 0, 2]).stone([0, 3, 0], [4, 3, 2]);
    b.air([1, 3, 0], [2, 3, 2]);
    b.stone([3, 1, 0], [3, 2, 2]);
    let mut c = contract("cellar");
    c.spaces.insert(
        "cellar".to_string(),
        space("open", vec![region([0, 1, 0], [4, 2, 2])]),
    );
    c.edges.push(with_via(
        edge("cellar", "exterior", "stair"),
        "hole",
        vec![region([1, 3, 0], [3, 3, 2])],
    ));
    let faces = exterior_faces(&b.model, &c);
    assert_eq!(faces.len(), 1, "{faces:?}");
    assert_eq!(faces[0].dir.as_str(), "up");
    assert_eq!(
        faces[0].cells.len(),
        9,
        "the whole hole, step included: {faces:?}"
    );
    c.edges[0].class = "walk".to_string();
    let faces = exterior_faces(&b.model, &c);
    assert_eq!(
        faces[0].cells.len(),
        6,
        "a walk faces out through air only: {faces:?}"
    );

    // A gate in a wall: the bar is the face.
    let mut g = Build::new([3, 3, 3]);
    g.stone([0, 0, 0], [2, 2, 2]).air([1, 1, 1], [1, 1, 1]);
    g.stone([1, 1, 0], [1, 1, 0]);
    let mut c = contract("cell");
    c.spaces.insert(
        "cell".to_string(),
        space("open", vec![region([1, 1, 1], [1, 1, 1])]),
    );
    c.edges.push(with_bar(
        edge("cell", "exterior", "barred"),
        "gate",
        vec![region([1, 1, 0], [1, 1, 0])],
        "minecraft:iron_bars",
    ));
    let faces = exterior_faces(&g.model, &c);
    assert_eq!(faces.len(), 1, "{faces:?}");
    assert_eq!(faces[0].dir.as_str(), "north");
    assert_eq!(faces[0].class, "barred");
}

/// **A zero binding is red on the three obligations that carry the weight**
/// (spec-0036 §2.9).
#[test]
fn a_contract_with_nothing_to_examine_reds_rather_than_passing_quietly() {
    let mut b = Build::new([5, 5, 5]);
    b.stone([0, 0, 0], [4, 4, 4]);
    let mut c = contract("nowhere");
    c.spaces.insert(
        "nowhere".to_string(),
        space("open", vec![region([1, 1, 1], [3, 3, 3])]),
    );
    c.spaces.insert(
        "elsewhere".to_string(),
        space("open", vec![region([1, 1, 4], [3, 3, 4])]),
    );
    c.edges.push(edge("nowhere", "exterior", "walk"));
    let report = check(&b.model, &c, &no_anchors());
    // Both spaces are `open`: the author declared no enclosed space, so
    // closure has nothing to confirm and is withheld with that zero stated
    // (spec-0098 §14: enclosed spaces are the author's declaration).
    assert!(
        !report.gates.iter().any(|g| g.id == "contract-closure"),
        "closure printed a verdict over an all-open contract: {:?}",
        report.gates
    );
    for id in ["contract-edge-proof", "contract-reachability"] {
        let g = gate(&report, id);
        assert_eq!(g.bound, 0, "{id}");
        assert!(
            !g.passed(),
            "{id} bound to nothing and called itself a pass"
        );
    }
    assert!(
        report
            .findings
            .iter()
            .filter(|f| f.contains("ZERO objects"))
            .count()
            >= 2,
        "{:?}",
        report.findings
    );
}

/// **The acknowledgement cannot buy a `posted` majority.**
///
/// The hatch as spec-0036 §2.9 writes it demands a string, and a string is a
/// property the failure it excuses supplies for free. It is narrowed to what the
/// author cannot write: it silences a majority made of `sealed` and `facade`
/// cells, whose demands are facts about the blocks, and never one made of
/// `posted` cells, which is the kind an author secures by placing something.
#[test]
fn an_acknowledgement_silences_a_facade_majority_and_never_a_posted_one() {
    // A tower: one cell of chamber inside, and a whole roof deck outside.
    let mut b = Build::new([9, 12, 9]);
    b.stone([0, 0, 0], [8, 8, 8]);
    b.air([4, 1, 4], [4, 4, 4]);
    b.air([0, 1, 4], [3, 2, 4]);

    let mut c = contract("chamber");
    c.spaces.insert(
        "chamber".to_string(),
        space(
            "enclosed",
            vec![region([1, 1, 4], [3, 2, 4]), region([4, 1, 4], [4, 4, 4])],
        ),
    );
    c.edges.push(with_via(
        edge("chamber", "exterior", "walk"),
        "door",
        vec![region([0, 1, 4], [0, 2, 4])],
    ));
    c.no_body.insert(
        "roof".to_string(),
        no_body("the leads", vec![region([0, 9, 0], [8, 9, 8])]),
    );
    let report = check(&b.model, &c, &no_anchors());
    assert!(gate(&report, "contract-coverage").passed());
    let g = gate(&report, "contract-no-body-majority");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("not play space"), "{}", g.detail);

    c.no_body_majority_ack = Some("a tower is mostly roof".to_string());
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body-majority");
    assert!(g.passed(), "{}", g.detail);
    assert!(gate(&report, "contract-no-body").detail.contains("facade"));

    // The same acknowledgement over a majority the author placed themselves.
    // An anchor on every cell makes the region classify `posted` — the kind
    // whose demand the author supplies — and the string buys nothing.
    let mut anchors = BTreeMap::new();
    for x in 0..=8 {
        for z in 0..=8 {
            anchors.insert(format!("anchor/perch-{x}-{z}"), [x, 9, z]);
        }
    }
    let report = check(&b.model, &c, &anchors);
    assert!(
        gate(&report, "contract-no-body").detail.contains("posted"),
        "{}",
        gate(&report, "contract-no-body").detail
    );
    let g = gate(&report, "contract-no-body-majority");
    assert!(
        !g.passed(),
        "an acknowledgement must not buy a `posted` majority: {}",
        g.detail
    );
    assert!(
        g.detail.contains("cannot buy a `posted` majority"),
        "{}",
        g.detail
    );
}

/// **The verdict is a function of the two inputs and nothing else** (ADR-0006).
///
/// Over the pier as well as the hall, because the pier's region is MIXED and the
/// per-cell partition is the part of the verdict that could carry an iteration
/// order: a hall with no out-of-walk floor at all would compare a partition that
/// was never computed.
#[test]
fn the_same_grid_and_contract_give_the_same_verdict_every_time() {
    let (b, c) = hall();
    let a = check(&b.model, &c, &no_anchors());
    let d = check(&b.model, &c, &no_anchors());
    assert_eq!(a, d);
    assert_eq!(a.enumeration, d.enumeration);

    let b = pier();
    let mut c = pier_hall();
    c.no_body.insert(
        "arcade".to_string(),
        no_body("the ruined west arcade", vec![deck(), void()]),
    );
    let a = check(&b.model, &c, &no_anchors());
    let d = check(&b.model, &c, &no_anchors());
    assert!(
        a.enumeration.iter().any(|e| e.contains("mixed")),
        "the pair must carry a mixed region: {:?}",
        a.enumeration
    );
    assert_eq!(a, d);
    assert_eq!(a.enumeration, d.enumeration);
}

/// **A `no_body` region with no standable cell decided its kind over an empty
/// set**, which is the vacuity rule one object down from a gate.
#[test]
fn an_out_of_walk_region_that_holds_no_standable_cell_is_red() {
    let (b, mut c) = hall();
    c.no_body.insert(
        "buttress".to_string(),
        no_body("solid stonework", vec![region([0, 0, 0], [0, 0, 0])]),
    );
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("no standable cell at all"),
        "{}",
        g.detail
    );
}

// ---------------------------------------------------------------------------
// A binding of zero, from both sides
// ---------------------------------------------------------------------------

/// **The honest empty.** A piece where every standable cell is play space
/// declares no out-of-walk region, and there is then nothing for §2.6 to
/// classify. The only way to hand that gate an object would be to declare a
/// region that is not there — the vacuity the gate exists to catch, one rung
/// out — so the gate is WITHHELD rather than printed green, and says why.
///
/// The pair below is what makes this a claim rather than a licence: the same
/// empty declaration reds the moment the floor stops being accounted for.
#[test]
fn an_empty_out_of_walk_declaration_is_withheld_when_every_cell_is_play_space() {
    let (b, c) = hall();
    assert!(
        c.no_body.is_empty(),
        "the fixture declares no out-of-walk floor"
    );
    let report = check(&b.model, &c, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);
    assert!(
        !report.gates.iter().any(|g| g.id == "contract-no-body"),
        "a gate over nothing claimed a verdict instead of being withheld: {:#?}",
        report.gates
    );
    // Three gates are withheld over this hall — the out-of-walk one under test,
    // plus `contract-edge-proof` (one space) and `contract-anchors` (the
    // fixture names no place). Each says which it is, so pick the one meant.
    let why = report
        .enumeration
        .iter()
        .find(|e| e.contains("contract-no-body") && e.contains("is not emitted"))
        .unwrap_or_else(|| panic!("withheld silently: {:#?}", report.enumeration));
    assert!(
        why.contains("standable cell(s) lie in a declared space"),
        "the justification must name the computed fact that discharges it, not merely assert \
         that nothing was declared: {why}"
    );
}

/// **The vacuous empty, one edit away.** The identical building and the
/// identical empty `no_body`, with the declared space pulled back off part of
/// the floor. Nothing is out of walk and nothing accounts for those cells
/// either, so the emptiness is no longer a claim about the piece — and the gate
/// reds instead of being withheld.
///
/// This is the property the withholding is secured by, shown failing: the
/// excuse is a computed fact about the blocks, and an author cannot reach it by
/// declaring less. Deleting an out-of-walk region does not delete its cells;
/// they have to land somewhere that owes MORE proof, not less.
#[test]
fn the_same_empty_declaration_reds_when_floor_is_left_unaccounted_for() {
    let (b, mut c) = hall();
    // The hall's own space, pulled back off the last courses of its floor.
    c.spaces.insert(
        "hall".to_string(),
        space("enclosed", vec![region([1, 1, 1], [9, 4, 4])]),
    );
    assert!(c.no_body.is_empty(), "still declaring no out-of-walk floor");
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-no-body");
    assert_eq!(g.bound, 0, "the population is empty in BOTH fixtures");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("examined ZERO objects"),
        "a zero binding must say so in the gate a reader sees, not only in a finding: {}",
        g.detail
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.contains("contract-no-body") && f.contains("examined ZERO objects")),
        "{:#?}",
        report.findings
    );
}

// ---------------------------------------------------------------------------
// spec-0042 — a way that content opens
// ---------------------------------------------------------------------------

/// The block a laid tread is made of, and the block the treads of
/// [`stair_piece`] are actually built from.
const TREAD: &str = "minecraft:stone_bricks";

/// The tread cells of [`stair_piece`]'s flight: the two courses of stone the
/// climb is standing on, and nothing else.
fn tread_cells() -> Vec<Region> {
    vec![region([1, 1, 6], [5, 1, 6]), region([1, 1, 7], [5, 2, 7])]
}

/// **The broken flight**: [`stair_piece`] with its treads taken out and nothing
/// else changed.
///
/// The contract is the repaired twin's, byte for byte — the claim did not move;
/// the blocks did. That is what makes the pair decidable in both directions.
fn broken_flight() -> (Build, SpatialContract) {
    let (mut b, c) = stair_piece();
    for r in tread_cells() {
        b.air(r.from, r.to);
    }
    (b, c)
}

/// Declare the broken flight's `stair` edge contingent on a laid way over
/// exactly the cells that are missing.
fn with_laid_flight(mut c: SpatialContract) -> SpatialContract {
    for e in &mut c.edges {
        if e.class == "stair" {
            *e = with_way(e.clone(), "broken-flight", "laid", tread_cells(), TREAD);
        }
    }
    c
}

/// **AC2, the twins.** The same declaration over two buildings one delta apart:
/// treads missing reds, treads missing *plus a declared way* is green, and the
/// green verdict NAMES the seam.
///
/// A bare green would not be worth having here — it cannot tell *reachable*
/// from *reachable eventually*, which is the whole ambiguity the surface exists
/// to remove — so the seam line is asserted, and so is a non-zero binding on
/// each of the three proof parts: well-formed (the confinement), edge proof
/// (closed on the bytes, open on the copy) and reachability (the union walk).
#[test]
fn a_broken_flight_reds_undeclared_and_greens_as_a_laid_way_with_the_seam_named() {
    // Red half: the current behaviour, kept as the red fixture.
    let (b, c) = broken_flight();
    let red = check(&b.model, &c, &no_anchors());
    let g = gate(&red, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("does not connect its two ends"),
        "{}",
        g.detail
    );
    let g = gate(&red, "contract-reachability");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("space head"), "{}", g.detail);

    // Green half: the identical bytes, with the break declared.
    let declared = with_laid_flight(c);
    let green = check(&b.model, &declared, &no_anchors());
    assert!(green.is_pass(), "{:#?}", green.gates);

    // The seam, named. This is the assertion the AC turns on.
    assert!(
        green.enumeration.iter().any(|e| {
            e.contains("space head") && e.contains("reached only once") && e.contains("is laid")
        }),
        "the storeys above the break are named with what opens them: {:#?}",
        green.enumeration
    );
    // …and the way itself is enumerated, with what it binds to.
    assert!(
        green
            .enumeration
            .iter()
            .any(|e| e.contains("way \"broken-flight\": laid over 15 cell(s)")),
        "{:#?}",
        green.enumeration
    );

    // Non-zero binding on all three proof parts.
    for id in [
        "contract-well-formed",
        "contract-edge-proof",
        "contract-reachability",
    ] {
        let g = gate(&green, id);
        assert!(g.passed(), "{}: {}", id, g.detail);
        assert!(g.bound > 0, "{id} bound {} — a green over nothing", g.bound);
    }
    // The union walk counts BOTH the cells that exist only once the way is laid
    // and the ones that stop existing then, so its binding is strictly larger
    // than the as-built walk's. Stated as a number: a silent change to either
    // end of the union shows up here rather than being absorbed.
    let shut = gate(&red, "contract-reachability").bound;
    let union = gate(&green, "contract-reachability").bound;
    assert_eq!(shut, 56, "the as-built walk's targets");
    assert_eq!(
        union, 66,
        "the union over the opening chain: the ten cells a body stands on only \
         once the flight is laid, on top of the fifty-six that are floor as built"
    );
}

/// **AC3, the closed direction has teeth.** The *repaired* twin's bytes with the
/// same laid way declared over them is red: the two ends already connect, so the
/// way opens nothing and the beat it claims is not real.
///
/// This is the assertion that would be vacuous if the closed proof ran on the
/// opened copy — there it passes over every building there is.
#[test]
fn a_way_over_treads_that_are_already_there_is_red() {
    let (b, c) = stair_piece();
    let declared = with_laid_flight(c);
    let report = check(&b.model, &declared, &no_anchors());
    let g = gate(&report, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("edge foot--stair--head")
            && g.detail.contains("does not open anything")
            && g.detail.contains("\"broken-flight\""),
        "the red names the edge and the way: {}",
        g.detail
    );
    // The same bytes also fail the `laid` confinement, and for its own reason.
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("is what is NOT there yet"),
        "{}",
        g.detail
    );
}

/// **AC4, the open direction has teeth, both halves.**
///
/// First: a delta that is applied and still does not connect — half the treads
/// laid — is the mirror of `barred`'s second half, and reds.
///
/// Second: `rise` one course off reds naming both numbers, on a way-carrying
/// edge. The measurement is over the two ends' resolved boxes, which is a fact
/// about the declaration rather than about either model, so it cannot be the
/// vacuous reading the AC warns about — on the as-built model a laid stair has
/// no climb to measure at all, and this number is 3 either way.
#[test]
fn a_way_whose_delta_still_does_not_connect_is_red_and_so_is_a_rise_one_course_off() {
    // Half the break laid: the upper course is still missing.
    let (b, c) = broken_flight();
    let mut half = c.clone();
    for e in &mut half.edges {
        if e.class == "stair" {
            *e = with_way(
                e.clone(),
                "broken-flight",
                "laid",
                vec![region([1, 1, 6], [5, 1, 6])],
                TREAD,
            );
        }
    }
    let report = check(&b.model, &half, &no_anchors());
    let g = gate(&report, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail
            .contains("with the way \"broken-flight\" laid the two ends still do not connect"),
        "{}",
        g.detail
    );

    // The whole break laid, and the declared rise one course off.
    let mut off = with_laid_flight(c);
    for e in &mut off.edges {
        if e.class == "stair" {
            e.rise = Some(2);
        }
    }
    let report = check(&b.model, &off, &no_anchors());
    let g = gate(&report, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("declares rise 2"), "{}", g.detail);
    assert!(g.detail.contains("measure 3"), "{}", g.detail);
}

/// **AC5, confinement.** A way region is refused wherever the defect could put
/// it: inside a room, on another edge's opening, or reaching outside its own
/// edge's transit volume. An unconfined delta is "content will build something
/// here" over any cells at all, which is the oldest shape §0 refuses.
#[test]
fn a_way_region_outside_its_own_edges_volume_is_refused_three_ways() {
    let (b, c) = broken_flight();

    // Into the room below.
    let mut in_space = c.clone();
    for e in &mut in_space.edges {
        if e.class == "stair" {
            *e = with_way(
                e.clone(),
                "broken-flight",
                "laid",
                vec![region([1, 1, 3], [5, 1, 3])],
                TREAD,
            );
        }
    }
    let report = check(&b.model, &in_space, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("claims 5 cell(s) of space \"foot\""),
        "{}",
        g.detail
    );

    // Onto another edge's opening — the front door.
    let mut on_via = c.clone();
    for e in &mut on_via.edges {
        if e.class == "stair" {
            *e = with_way(
                e.clone(),
                "broken-flight",
                "laid",
                vec![region([0, 1, 2], [0, 2, 2])],
                TREAD,
            );
        }
    }
    let report = check(&b.model, &on_via, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail
            .contains("shares 2 cell(s) with edge foot--walk--exterior"),
        "{}",
        g.detail
    );

    // Outside the flight altogether, in air the piece does not claim.
    let mut adrift = c.clone();
    for e in &mut adrift.edges {
        if e.class == "stair" {
            *e = with_way(
                e.clone(),
                "broken-flight",
                "laid",
                vec![region([1, 1, 6], [5, 1, 8])],
                TREAD,
            );
        }
    }
    let report = check(&b.model, &adrift, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail
            .contains("lie outside this edge's own transit volume"),
        "{}",
        g.detail
    );
}

/// **AC6, one prover — the statement the old surface could not make.** A
/// portcullis across a climb: a `stair` carrying a **cleared** way. `barred`'s
/// walk shape can express a grate in a doorway and nothing else; the same
/// mechanism keyed to the object rather than to the verb states this, and the
/// SAME connectivity prover decides it.
#[test]
fn a_portcullis_over_a_climb_is_a_cleared_way_on_a_stair() {
    let (mut b, c) = stair_piece();
    // The grate: the air above the first tread, filled with iron.
    b.paint([1, 2, 6], [5, 5, 6], "minecraft:iron_bars");

    let mut declared = c.clone();
    for e in &mut declared.edges {
        if e.class == "stair" {
            *e = with_way(
                e.clone(),
                "portcullis",
                "cleared",
                vec![region([1, 2, 6], [5, 5, 6])],
                "minecraft:iron_bars",
            );
        }
    }
    let report = check(&b.model, &declared, &no_anchors());
    assert!(report.is_pass(), "{:#?}", report.gates);
    assert!(
        report.enumeration.iter().any(|e| {
            e.contains("space head") && e.contains("reached only once") && e.contains("is cleared")
        }),
        "{:#?}",
        report.enumeration
    );

    // Undeclared, the same iron is simply a climb that does not climb.
    let bare = check(&b.model, &c, &no_anchors());
    let g = gate(&bare, "contract-edge-proof");
    assert!(!g.passed(), "{}", g.detail);
}

/// **AC6, the other half: one edge, one contingency.** `barred` IS a walk
/// carrying a cleared way, so declaring both on one edge is a double
/// declaration and is refused by name.
#[test]
fn a_barred_edge_that_also_declares_a_way_is_refused() {
    let (b, mut c) = hall();
    c.spaces.insert(
        "cell".to_string(),
        space("enclosed", vec![region([1, 1, 1], [3, 4, 7])]),
    );
    let mut both = with_bar(
        edge("hall", "cell", "barred"),
        "gate",
        vec![region([4, 1, 4], [4, 2, 4])],
        "minecraft:iron_bars",
    );
    both = with_way(
        both,
        "second-thoughts",
        "cleared",
        vec![region([4, 1, 4], [4, 2, 4])],
        "minecraft:iron_bars",
    );
    c.edges.push(both);
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("declares its contingency twice"),
        "{}",
        g.detail
    );
}

/// **A sightline and an exterior edge cannot be contingent.** A `vision` edge
/// claims no traversal, so it has nothing to be contingent about; an
/// `exterior` endpoint has no cells, so there is no far end for an opening to
/// reach and a seam-crossing way is the face contract's business.
#[test]
fn a_way_on_a_sightline_or_an_exterior_edge_is_refused() {
    let (b, c) = broken_flight();

    let mut outward = c.clone();
    for e in &mut outward.edges {
        if e.b == "exterior" {
            *e = with_way(
                e.clone(),
                "drawbridge",
                "laid",
                vec![region([0, 1, 2], [0, 2, 2])],
                TREAD,
            );
        }
    }
    let report = check(&b.model, &outward, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(g.detail.contains("cannot carry a way"), "{}", g.detail);

    let mut seen = c.clone();
    seen.edges.push(with_way(
        with_via(
            edge("foot", "head", "vision"),
            "squint",
            vec![region([1, 4, 7], [1, 4, 7])],
        ),
        "shutter",
        "cleared",
        vec![region([1, 4, 7], [1, 4, 7])],
        TREAD,
    ));
    let report = check(&b.model, &seen, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("nothing to be contingent about"),
        "{}",
        g.detail
    );
}

/// **An anchor inside a way region names the way**, exactly as one inside a bar
/// region names the bar — so a campaign can bind content to the place the way
/// opens rather than to the volume that contains it.
#[test]
fn an_anchor_in_a_way_region_resolves_to_the_way() {
    let (b, c) = broken_flight();
    let declared = with_laid_flight(c);
    let mut anchors = BTreeMap::new();
    anchors.insert("anchor/planks".to_string(), [3, 1, 6]);
    let report = check(&b.model, &declared, &anchors);
    let g = gate(&report, "contract-anchors");
    assert!(g.passed(), "{}", g.detail);
    assert!(g.detail.contains("1 in a way"), "{}", g.detail);
}

/// **An all-open piece states its zero instead of being refused** (spec-0098
/// §6b, criterion 15). A street: a floor and nothing around it, one `open`
/// space. `contract-closure` examines nothing — there is no envelope to close —
/// and says so in the enumeration at a passing verdict; one `enclosed` space in
/// the same contract, unclosed, still reds. Vacuous if the piece had an
/// enclosing space: the contract's envelope set is asserted to be `{open}`.
#[test]
fn an_all_open_piece_states_its_zero_and_passes() {
    let mut street = Build::new([14, 5, 8]);
    street.stone([0, 0, 0], [13, 0, 7]);
    let mut c = contract("street");
    c.spaces.insert(
        "street".to_string(),
        space("open", vec![region([1, 1, 1], [12, 3, 6])]),
    );
    let envelopes: BTreeSet<&str> = c.spaces.values().map(|s| s.envelope.as_str()).collect();
    assert_eq!(envelopes, ["open"].into_iter().collect());
    let report = check(&street.model, &c, &no_anchors());
    assert!(
        !report
            .gates
            .iter()
            .any(|g| g.id == "contract-closure" && !g.passed()),
        "{:#?}",
        report.gates
    );
    assert!(
        report
            .enumeration
            .iter()
            .any(|e| e.contains("contract-closure")
                && e.contains(
                    "0 of 1 space(s) declare an envelope closure examines; every space is open"
                )),
        "the zero is stated: {:?}",
        report.enumeration
    );
    // The same street with its space declared `enclosed`: nothing closes it,
    // and closure refuses.
    c.spaces.get_mut("street").unwrap().envelope = "enclosed".to_string();
    let report = check(&street.model, &c, &no_anchors());
    assert!(!gate(&report, "contract-closure").passed());
}

/// **A sealed scenery piece owes nothing a body would** (spec-0098 §14). A
/// beacon built to be seen and never entered: a stone block around a hollow one
/// course tall, so no cell of it is stood in. Judged as scenery
/// (`check_sealed`), the zero standable cells are stated with their count and
/// pass; the very same piece judged as a place a body reaches still reds on
/// the zero — a reached place with nowhere to stand is the defect the
/// reachability gate exists for.
#[test]
fn a_sealed_scenery_piece_states_its_zero_standable_cells() {
    let mut b = Build::new([5, 5, 5]);
    b.stone([0, 0, 0], [4, 4, 4]);
    b.air([1, 2, 1], [3, 2, 3]);
    let mut c = contract("room");
    c.spaces.insert(
        "room".to_string(),
        space("enclosed", vec![region([1, 2, 1], [3, 2, 3])]),
    );
    assert!(
        delvec::grammar::nav::standable_cells(&b.model).is_empty(),
        "the beacon holds no cell a body stands in — without that this proves nothing"
    );

    let sealed = delvec::grammar::contract::check_sealed(&b.model, &c, &no_anchors(), true);
    let red: Vec<String> = sealed
        .gates
        .iter()
        .filter(|g| g.failed())
        .map(|g| format!("{}: {}", g.id, g.detail))
        .collect();
    assert!(red.is_empty(), "scenery owes no standable cell: {red:?}");
    assert!(
        sealed
            .enumeration
            .iter()
            .any(|l| l.contains("contract-reachability") && l.contains("0 standable cell(s)")),
        "the zero is stated with its count: {:?}",
        sealed.enumeration
    );

    let reached = check(&b.model, &c, &no_anchors());
    assert!(
        gate(&reached, "contract-reachability").failed(),
        "a place a body reaches, with nowhere to stand, is refused"
    );
}

// ---------------------------------------------------------------------------
// A climb inside one piece: two spaces and the ladder between them
// ---------------------------------------------------------------------------

/// A two-level room 7x9x5: a lower floor at y 1, a mezzanine floor course at
/// y 4 whose stood-on cells are at y 5, a door in the east wall at the lower
/// level, and — when `ladder` names a facing — a ladder against the west wall
/// from the lower floor up through a one-cell hole in the mezzanine, at x 1,
/// z 2, y 1..=4. With no ladder the hole is still cut.
fn two_level(ladder: Option<&str>) -> Build {
    let mut b = Build::new([7, 9, 5]);
    b.room([0, 0, 0], [6, 8, 4]);
    b.stone([1, 4, 1], [5, 4, 3]);
    b.air([1, 4, 2], [1, 4, 2]);
    b.air([6, 1, 2], [6, 2, 2]);
    if let Some(facing) = ladder {
        let block: BlockState = format!("minecraft:ladder[facing={facing}]")
            .parse()
            .expect("a ladder state");
        for y in 1..=4 {
            b.model.set([1, y, 2], &block).unwrap();
        }
    }
    b
}

/// The honest contract of [`two_level`]: the two floors are two spaces (a
/// space is one floor), the ladder column is the climb's own volume, and the
/// way between them is a `climb` rising 4.
fn two_level_contract() -> SpatialContract {
    let mut c = contract("lower");
    c.spaces.insert(
        "lower".to_string(),
        space(
            "enclosed",
            vec![
                region([1, 1, 1], [5, 3, 1]),
                region([1, 1, 3], [5, 3, 3]),
                region([2, 1, 2], [5, 3, 2]),
            ],
        ),
    );
    c.spaces.insert(
        "upper".to_string(),
        space("enclosed", vec![region([1, 5, 1], [5, 7, 3])]),
    );
    c.edges.push(with_via(
        edge("lower", "exterior", "walk"),
        "door",
        vec![region([6, 1, 2], [6, 2, 2])],
    ));
    c.edges.push(with_rise(
        with_via(
            edge("lower", "upper", "climb"),
            "ladder",
            vec![region([1, 1, 2], [1, 4, 2])],
        ),
        4,
    ));
    c
}

/// **A two-level interior is two spaces and a declared climb** (spec-0099,
/// spec-0098 §14). The ladder hangs on the west wall: every gate holds — the
/// climb is proved both ways over the body's own climb moves, and the upper
/// floor is reached from the entry through it.
///
/// Each perturbation is one the climb proof alone could catch: the same room
/// with no ladder (the hole is still cut) and with the ladder turned to face
/// the wall it should hang on — kept only where the mezzanine's floor course
/// stands behind its top rung — both red the edge proof and leave the upper
/// floor unreached; deleting the climb edge
/// with the ladder hanging reds reachability, so the edge is a checked claim
/// and not decoration; and folding the two floors into one space is refused
/// as more than one floor.
#[test]
fn a_ladder_between_two_floors_is_a_climb_edge_proved_both_ways() {
    let c = two_level_contract();
    let report = check(&two_level(Some("east")).model, &c, &no_anchors());
    let red: Vec<String> = report
        .gates
        .iter()
        .filter(|g| g.failed())
        .map(|g| format!("{}: {}", g.id, g.detail))
        .collect();
    assert!(red.is_empty(), "the laddered room holds: {red:?}");
    let proof = gate(&report, "contract-edge-proof");
    assert_eq!(
        proof.bound, 1,
        "the one interior edge is the climb: {}",
        proof.detail
    );
    let reach = gate(&report, "contract-reachability");
    assert!(reach.passed(), "{}", reach.detail);

    for (what, b) in [
        ("no ladder", two_level(None)),
        ("a ladder with nothing behind it", two_level(Some("west"))),
    ] {
        let report = check(&b.model, &c, &no_anchors());
        let proof = gate(&report, "contract-edge-proof");
        assert!(proof.failed(), "{what}: {}", proof.detail);
        let reach = gate(&report, "contract-reachability");
        assert!(
            reach.failed() && reach.detail.contains("space upper"),
            "{what}: the upper floor is unreached: {}",
            reach.detail
        );
    }

    let bare = check(&two_level(None).model, &c, &no_anchors());
    let proof = gate(&bare, "contract-edge-proof");
    assert!(
        proof.detail.contains("no climbable a body can hold on in"),
        "with no ladder the red says what a climb is: {}",
        proof.detail
    );

    let mut severed = c.clone();
    severed.edges.retain(|e| e.class != "climb");
    let report = check(&two_level(Some("east")).model, &severed, &no_anchors());
    let reach = gate(&report, "contract-reachability");
    assert!(
        reach.failed() && reach.detail.contains("space upper"),
        "the ladder still hangs, and without the declared edge the upper floor is not reached: {}",
        reach.detail
    );

    let mut merged = contract("room");
    merged.spaces.insert(
        "room".to_string(),
        space(
            "enclosed",
            vec![
                region([1, 1, 1], [5, 3, 1]),
                region([1, 1, 3], [5, 3, 3]),
                region([2, 1, 2], [5, 3, 2]),
                region([1, 5, 1], [5, 7, 3]),
            ],
        ),
    );
    merged.edges.push(c.edges[0].clone());
    merged.edges[0].a = "room".to_string();
    let report = check(&two_level(Some("east")).model, &merged, &no_anchors());
    let wf = gate(&report, "contract-well-formed");
    assert!(
        wf.failed() && wf.detail.contains("ONE floor"),
        "two floors folded into one space are refused: {}",
        wf.detail
    );
}

/// **A climb declares its volume and its rise**, as a stair does: the ladder
/// belongs to the edge, and the level relation is measured.
#[test]
fn a_climb_without_its_volume_or_its_rise_is_refused() {
    let b = two_level(Some("east"));
    let mut no_via = two_level_contract();
    no_via.edges[1].via = None;
    let wf = gate(
        &check(&b.model, &no_via, &no_anchors()),
        "contract-well-formed",
    )
    .clone();
    assert!(
        wf.failed() && wf.detail.contains("requires a `via`"),
        "{}",
        wf.detail
    );
    let mut no_rise = two_level_contract();
    no_rise.edges[1].rise = None;
    let wf = gate(
        &check(&b.model, &no_rise, &no_anchors()),
        "contract-well-formed",
    )
    .clone();
    assert!(
        wf.failed() && wf.detail.contains("requires a declared `rise`"),
        "{}",
        wf.detail
    );
    let mut off_by_one = two_level_contract();
    off_by_one.edges[1].rise = Some(3);
    let report = check(&b.model, &off_by_one, &no_anchors());
    let proof = gate(&report, "contract-edge-proof");
    assert!(
        proof.failed()
            && proof
                .detail
                .contains("declares rise 3 but the resolved boxes measure 4"),
        "{}",
        proof.detail
    );
}
