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
//! the same pair `delve-admit` hands it for a piece nobody generated, so the
//! fixtures are written in that vocabulary and not in the one door's. The
//! corpus example goes through the other door end to end in `tests/contract.rs`
//! and through the CLI in `tests/cli.rs`.

use std::collections::BTreeMap;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::contract::{check, exterior_faces};
use delvewright_grammar::geom::Box3;
use delvewright_grammar::model::VoxelModel;
use delvewright_schem::prefab::{
    ContractBar, ContractEdge, ContractNoBody, ContractSpace, ContractVolume, Region,
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
    }
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
fn gate<'a>(
    report: &'a delvewright_grammar::ContractReport,
    id: &str,
) -> &'a delvewright_grammar::Gate {
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

/// **A roofed room cannot be downgraded out of closure** (spec-0036 §2.3) — the
/// adversary's second move, and the one an envelope keyword alone would buy.
#[test]
fn a_roofed_space_declared_open_is_refused_and_a_sky_open_one_is_not() {
    let (b, mut c) = hall();
    c.spaces.get_mut("hall").unwrap().envelope = "open".to_string();
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-closure");
    assert!(!g.passed(), "{}", g.detail);
    assert!(
        g.detail.contains("blocks overhead"),
        "the roof is what refuses it: {}",
        g.detail
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
    for id in [
        "contract-closure",
        "contract-edge-proof",
        "contract-reachability",
    ] {
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
            >= 3,
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
#[test]
fn the_same_grid_and_contract_give_the_same_verdict_every_time() {
    let (b, c) = hall();
    let a = check(&b.model, &c, &no_anchors());
    let d = check(&b.model, &c, &no_anchors());
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
// The refusal names the route
// ---------------------------------------------------------------------------

/// The one-floor clause of a `contract-well-formed` detail, on its own.
///
/// The detail is every complaint joined with " · ", so asserting over the whole
/// string cannot tell "the climb sentence was added to the one-floor refusal"
/// from "the word `stair` appears somewhere else in this report". Scoping to the
/// clause is what makes the NEGATIVE half below mean anything.
fn one_floor_clause(detail: &str) -> String {
    detail
        .split(" · ")
        .find(|part| part.contains("is ONE floor"))
        .unwrap_or_else(|| panic!("no one-floor refusal in this detail: {detail}"))
        .to_string()
}

/// **A climb refused as a space is told where treads live** (spec-0036 §1a; the
/// diagnostic-reachability finding of trial-0001's contract round).
///
/// The invariant is right and stays: a space is one floor, which is what makes
/// an edge's rise measurable. What is wrong is that the refusal is *routeless
/// from where the author is standing*. An author who models a climb as one
/// space — which is what a recursive rule hands them, one region name over every
/// tread — is told which invariant they broke and nothing about where the treads
/// actually belong. The engine does know: it refuses a via-less stair with "a
/// stair's treads belong to the edge". That sentence only ever fires for an
/// author who has ALREADY declared a stair edge, so it is unreachable from the
/// space-shaped mistake, and the space-shaped mistake is the one people make.
/// Three zones were reported undeclarable and a design round was dispatched for
/// a capability that already existed.
///
/// So: when the offending span is a **contiguous climb** — every level reachable
/// from the one below — the one-floor refusal names the construct that does hold
/// it, by the identifiers an author types: a `stair` edge and its `via`.
///
/// **What would make this test vacuous**, in the order the modes bite:
///
/// 1. *Asserting only the added sentence.* An unconditional append to every
///    one-floor refusal would pass, and the message would then be wrong on every
///    multi-level space that is not a climb. The second half of this test is the
///    whole point: a space whose standable levels have a GAP is not a climb, and
///    it must NOT be sent down the stair route. Delete that half and the test
///    stops measuring.
/// 2. *Asserting only the route and not the invariant.* Replacing the one-floor
///    refusal with an explanation would pass. `is ONE floor` is asserted on both
///    fixtures so the invariant cannot be traded for the message.
/// 3. *A zero binding.* `contract-well-formed` binds to spaces + out-of-walk
///    regions + edges; over an empty contract it examines nothing and could go
///    red for reasons unrelated to floors. `bound` is asserted on both fixtures.
/// 4. *A red from somewhere else in the detail.* `one_floor_clause` scopes every
///    assertion to the one-floor complaint, so neither half can be satisfied by
///    another gate's or another clause's wording.
/// 5. *Matching prose instead of the surface.* The route is asserted by the two
///    identifiers an author must write — the `stair` class and the `via` field —
///    not by a sentence, so the wording stays free and the test stays about
///    whether the author is told what to type.
#[test]
fn a_climb_refused_as_one_space_is_told_that_treads_belong_to_a_stairs_via() {
    // The climb. `stair_piece`'s blocks — foot at y1, three treads at y1/y2/y3,
    // head at y4 — declared the wrong way: one union name over the whole thing.
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
    assert!(g.bound > 0, "the gate examined nothing: {:?}", g);

    let clause = one_floor_clause(&g.detail);
    assert!(
        clause.contains("is ONE floor"),
        "the invariant is still what refuses this, and it is not traded for the \
         explanation: {clause}"
    );
    assert!(
        clause.contains("stair"),
        "a contiguous climb's refusal names the edge class that holds treads, so an \
         author who modelled it as a space can reach the construct that works: {clause}"
    );
    assert!(
        clause.contains("via"),
        "and names the transit volume the treads become — the half the via-less-stair \
         refusal already states, which is unreachable from here: {clause}"
    );

    // The other side. A space that spans levels WITHOUT being a climb — a
    // shelf three courses up, standable at y3, with nothing standable at y2 to
    // reach it from — is refused for the same reason and must NOT be routed to a
    // stair. Nothing climbs here, so a `via` full of treads is not the answer,
    // and a refusal that says it is has been appended rather than computed.
    let (mut b, c) = hall();
    b.stone([4, 2, 4], [4, 2, 4]);
    let report = check(&b.model, &c, &no_anchors());
    let g = gate(&report, "contract-well-formed");
    assert!(
        !g.passed(),
        "the shelf is a second level and is refused as one: {}",
        g.detail
    );
    assert!(g.bound > 0, "the gate examined nothing: {:?}", g);

    let clause = one_floor_clause(&g.detail);
    assert!(
        clause.contains("is ONE floor"),
        "the same invariant refuses it: {clause}"
    );
    assert!(
        !clause.contains("stair"),
        "a gapped span is not a climb, so the stair route does not apply to it — a \
         refusal that names it here was appended to every one-floor red rather than \
         computed from this one: {clause}"
    );
}
