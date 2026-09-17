//! **What a drawing builds** — the executor's own claims (spec-0072 §5, and
//! criteria 3, 5 and 6).
//!
//! Determinism, the frame every state is read in, the derived stair shape, the
//! order the operations run in, and what each arranger arranges. Every
//! assertion is a **state string or a cell set**, not a count: a count is
//! satisfied by the wrong block.

use std::collections::{BTreeMap, BTreeSet};

use delvec::drawing::execute::{self, ExecuteOptions, Execution};
use delvec::drawing::ir::Drawing;
use delvec::grammar::Box3;
use delvec::grammar::gates;

const VERSION: &str = delvec::compiler::DSL_VERSION;

fn doc(body: &str) -> String {
    format!(r#"{{ "dsl_version": "{VERSION}", "name": "t", {body} }}"#)
}

fn run_at(json: &str, region: [u32; 3], seed: u64) -> Execution {
    let drawing: Drawing = serde_json::from_str(json).expect("the document parses");
    execute::execute(
        &drawing,
        Box3::at_origin(region),
        &ExecuteOptions::seeded(seed, "."),
    )
    .unwrap_or_else(|e| panic!("the drawing executes: {e}"))
}

fn run(json: &str, region: [u32; 3]) -> Execution {
    run_at(json, region, 0)
}

/// The block at a cell, as the vanilla string.
fn at(x: &Execution, pos: [i32; 3]) -> String {
    x.expansion
        .model
        .get(pos)
        .expect("the cell is in the region")
        .to_string()
}

fn filled(x: &Execution) -> BTreeSet<[i32; 3]> {
    x.expansion
        .model
        .region()
        .positions()
        .filter(|p| x.expansion.model.get(*p).is_some_and(|b| !b.is_air()))
        .collect()
}

// ---------------------------------------------------------------------------
// Criterion 3 — determinism
// ---------------------------------------------------------------------------

const WEATHERED: &str = r#""palette": {
        "wall": [ { "weight": 8, "block": "minecraft:stone_bricks" },
                  { "weight": 2, "block": "minecraft:cracked_stone_bricks" },
                  { "weight": 1, "block": "minecraft:mossy_stone_bricks" } ],
        "floor": "minecraft:polished_andesite" },
      "ops": [ { "op": "box", "role": "wall" },
               { "op": "box", "role": "floor", "from": [0,0,0], "to": [7,0,7] } ]"#;

/// Same drawing, same region, same seed: byte-identical, model and canonical
/// bytes alike.
#[test]
fn executing_twice_gives_a_byte_identical_model() {
    let json = doc(WEATHERED);
    let a = run(&json, [8, 4, 8]);
    let b = run(&json, [8, 4, 8]);
    assert_eq!(
        a.expansion.model.canonical_bytes(),
        b.expansion.model.canonical_bytes()
    );
    assert_eq!(a.report, b.report, "and the run report with it");
}

/// **Two seeds move the weighted cells and nothing else.** Geometry has no
/// seed, so a cell whose paint is one state is the same state at every seed,
/// and a cell whose paint is a weighted list is the one thing a seed reaches.
#[test]
fn two_seeds_agree_at_every_cell_whose_paint_is_not_a_weighted_list() {
    let json = doc(WEATHERED);
    let a = run_at(&json, [8, 4, 8], 0);
    let b = run_at(&json, [8, 4, 8], 99);
    assert_eq!(filled(&a), filled(&b), "the geometry is the same shape");

    let mut same = 0usize;
    let mut moved = 0usize;
    for pos in a.expansion.model.region().positions() {
        let (x, y) = (at(&a, pos), at(&b, pos));
        // The floor is one state, so it never moves; the wall is a list, so it
        // may.
        if x == "minecraft:polished_andesite" || x == "minecraft:air" {
            assert_eq!(x, y, "a cell whose paint is one state moved at {pos:?}");
            same += 1;
        } else if x == y {
            same += 1;
        } else {
            moved += 1;
        }
    }
    assert!(
        moved > 0 && same > 0,
        "binding count {moved} moved of {} — a seed that moved nothing would make the equality \
         above vacuous",
        same + moved
    );
}

/// **A weighted paint draws by POSITION**, so editing one operation re-textures
/// no other cell.
///
/// The drawing below paints the same weathered wall and then adds one more
/// operation over a corner. Every cell the second version did not repaint holds
/// exactly what it held before — which a stream-consuming draw could not
/// promise, because one more draw would shift every later cell's.
#[test]
fn editing_one_operation_re_textures_no_other_cell() {
    let before = run(&doc(WEATHERED), [8, 4, 8]);
    let after = run(
        &doc(r#""palette": {
                 "wall": [ { "weight": 8, "block": "minecraft:stone_bricks" },
                           { "weight": 2, "block": "minecraft:cracked_stone_bricks" },
                           { "weight": 1, "block": "minecraft:mossy_stone_bricks" } ],
                 "floor": "minecraft:polished_andesite" },
               "ops": [ { "op": "box", "role": "wall" },
                        { "op": "box", "role": "floor", "from": [0,0,0], "to": [7,0,7] },
                        { "op": "box", "role": "floor", "from": [0,3,0], "to": [1,3,1] } ]"#),
        [8, 4, 8],
    );
    let mut untouched = 0usize;
    for pos in before.expansion.model.region().positions() {
        let repainted = pos[1] == 3 && pos[0] <= 1 && pos[2] <= 1;
        if repainted {
            assert_eq!(at(&after, pos), "minecraft:polished_andesite");
            continue;
        }
        assert_eq!(
            at(&before, pos),
            at(&after, pos),
            "the added operation re-textured {pos:?}"
        );
        untouched += 1;
    }
    assert!(untouched > 200, "binding count: {untouched} cell(s)");
}

// ---------------------------------------------------------------------------
// Criterion 5 — frames
// ---------------------------------------------------------------------------

/// The four turns of a `use`, read as the state a stair lands in.
///
/// A stair written `facing=north` inside a define faces wherever the `use`
/// turns it, and a turn is a quarter-turn **clockwise seen from above**: north,
/// east, south, west as the turn goes 0, 1, 2, 3. A resolver that read the
/// permutation alone would land two of the four wrong, because every turn but
/// the identity is a transposition WITH a reversed axis.
#[test]
fn a_turned_use_lands_its_states_in_the_world_frame() {
    let mut got = Vec::new();
    for turn in 0..4 {
        let x = run(
            &doc(&format!(
                r#""palette": {{ "tread":
                     "minecraft:stone_brick_stairs[facing=north,half=bottom,waterlogged=false]" }},
                   "defines": {{ "step": {{ "roles": ["s"], "body": [
                     {{ "op": "box", "role": "s", "from": [0,0,0], "to": [0,0,0] }} ] }} }},
                   "ops": [ {{ "op": "use", "define": "step", "turn": {turn},
                              "from": [0,0,0], "to": [2,0,2],
                              "roles": {{ "s": "tread" }} }} ]"#
            )),
            [3, 1, 3],
        );
        let cell = *filled(&x).iter().next().expect("one cell");
        got.push((turn, cell, at(&x, cell)));
    }
    let facings: Vec<&str> = got
        .iter()
        .map(|(_, _, s)| {
            s.split("facing=")
                .nth(1)
                .unwrap()
                .split([',', ']'])
                .next()
                .unwrap()
        })
        .collect();
    assert_eq!(
        facings,
        ["north", "east", "south", "west"],
        "a quarter-turn clockwise from above takes north to east: {got:?}"
    );
    // And the body's own corner travels with it: local (0,0,0) is the box's
    // north-west corner unturned, and each turn moves it round the box.
    let corners: Vec<[i32; 3]> = got.iter().map(|(_, c, _)| *c).collect();
    assert_eq!(
        corners,
        [[0, 0, 0], [2, 0, 0], [2, 0, 2], [0, 0, 2]],
        "the geometry turns with the state, not against it"
    );
}

/// A `mirror` on a `use` reflects the body across the centre plane of its own
/// local box, and the states reflect with it: a door's hinge swaps.
#[test]
fn a_mirrored_use_swaps_a_handedness() {
    let door = |mirror: &str| {
        run(
            &doc(&format!(
                r#""palette": {{ "door":
                     "minecraft:oak_door[facing=north,half=lower,hinge=left,open=false,powered=false]" }},
                   "defines": {{ "leaf": {{ "roles": ["d"], "body": [
                     {{ "op": "box", "role": "d", "from": [0,0,0], "to": [0,0,0] }} ] }} }},
                   "ops": [ {{ "op": "use", "define": "leaf", {mirror}
                              "from": [0,0,0], "to": [2,0,2],
                              "roles": {{ "d": "door" }} }} ]"#
            )),
            [3, 1, 3],
        )
    };
    let plain = door("");
    let mirrored = door(r#""mirror": "x","#);
    assert!(at(&plain, [0, 0, 0]).contains("hinge=left"));
    assert!(
        at(&mirrored, [2, 0, 0]).contains("hinge=right"),
        "a mirror is exactly what swaps a hand: {}",
        at(&mirrored, [2, 0, 0])
    );
}

/// **A state whose image the frame does not determine is refused, not guessed.**
///
/// The control beside it is the same role unturned: refusing everything would
/// pass an assertion about a refusal, so the accepting half is what says the
/// rule is about the frame.
#[test]
fn a_state_with_no_image_under_a_frame_is_refused() {
    // There is no frame in a drawing that moves the vertical — a drawing has
    // gravity — so what is left with no image is a property the horizontal
    // frames cannot map: a rail's direction-composed `shape`.
    let json = doc(
        r#""palette": { "rail": "minecraft:rail[shape=ascending_north,waterlogged=false]" },
           "defines": { "run": { "roles": ["r"], "body": [
             { "op": "box", "role": "r", "from": [0,0,0], "to": [0,0,0] } ] } },
           "ops": [ { "op": "use", "define": "run", "turn": 1,
                      "from": [0,0,0], "to": [2,0,2], "roles": { "r": "rail" } } ]"#,
    );
    let drawing: Drawing = serde_json::from_str(&json).unwrap();
    let e = execute::execute(
        &drawing,
        Box3::at_origin([3, 1, 3]),
        &ExecuteOptions::seeded(0, "."),
    )
    .expect_err("a turned rail shape has no image");
    assert_eq!(e.code.id(), "DW0738");
    assert!(e.to_string().contains("ascending_north"), "{e}");

    // The control: unturned, the same role is written as authored.
    let ok = json.replace(r#""turn": 1,"#, "");
    let x = run(&ok, [3, 1, 3]);
    assert_eq!(
        at(&x, [0, 0, 0]),
        "minecraft:rail[shape=ascending_north,waterlogged=false]"
    );
}

// ---------------------------------------------------------------------------
// Criterion 6 — derived state, the stair half
// ---------------------------------------------------------------------------

/// **Two stair runs meeting at a corner export the shapes the game derives**,
/// and the drawing types none of them.
///
/// The `stair-shape` gate is the judge, and it reports the corner bound and
/// nothing mismatched — which it must, because the writer and the judge are the
/// one derivation read from two ends.
#[test]
fn two_stair_runs_meeting_at_a_corner_export_the_derived_shapes() {
    let x = run(
        &doc(r#""palette": {
                 "north": "minecraft:stone_brick_stairs[facing=north,half=bottom,waterlogged=false]",
                 "west":  "minecraft:stone_brick_stairs[facing=west,half=bottom,waterlogged=false]" },
               "ops": [
                 { "op": "box", "role": "north", "from": [0,0,0], "to": [0,0,2] },
                 { "op": "box", "role": "west",  "from": [1,0,0], "to": [2,0,0] } ]"#),
        [3, 1, 3],
    );
    assert_eq!(x.report.stairs_settled, 5, "every stair is settled");

    // The corner is the WEST-facing stair at [1,0,0]: the cell it faces holds a
    // north-facing stair of the same half, and the cell on the far side of the
    // turn holds none, so the game mitres it. West's counter-clockwise
    // direction is south and the stair in front faces north, so the hand is
    // RIGHT — the game's own naming, which the engine did not choose.
    assert_eq!(
        at(&x, [1, 0, 0]),
        "minecraft:stone_brick_stairs[facing=west,half=bottom,shape=outer_right,waterlogged=false]"
    );
    // ...and every other stair of the two runs is straight, including the one
    // that ends the north run at the corner cell: its own front is outside the
    // piece, and a cell outside reads as no stair.
    for cell in [[0, 0, 0], [0, 0, 1], [0, 0, 2], [2, 0, 0]] {
        assert!(
            at(&x, cell).contains("shape=straight"),
            "{cell:?}: {}",
            at(&x, cell)
        );
    }

    let report = gates::judge(&x.expansion, gates::Options::default());
    let gate = report
        .gates
        .iter()
        .find(|g| g.id == "stair-shape")
        .expect("the gate is emitted over a piece with stairs");
    assert!(gate.passed(), "{}", gate.detail);
    assert_eq!(gate.bound, 5, "every stair examined");
}

// ---------------------------------------------------------------------------
// The arrangers, and the order
// ---------------------------------------------------------------------------

/// A later operation overwrites an earlier one; `where` reads the cell it is
/// about to paint and no other.
#[test]
fn a_later_operation_overwrites_and_where_reads_the_cell_it_paints() {
    let x = run(
        &doc(
            r#""palette": { "rock": "minecraft:stone", "moss": "minecraft:moss_block",
                            "gold": "minecraft:gold_block" },
               "ops": [
                 { "op": "box", "role": "rock", "from": [0,0,0], "to": [3,0,0] },
                 { "op": "box", "role": "moss", "from": [2,0,0], "to": [3,0,0] },
                 { "op": "box", "role": "gold", "from": [0,0,0], "to": [3,0,0],
                   "where": ["moss"] } ]"#,
        ),
        [4, 1, 1],
    );
    assert_eq!(at(&x, [0, 0, 0]), "minecraft:stone");
    assert_eq!(at(&x, [1, 0, 0]), "minecraft:stone");
    assert_eq!(at(&x, [2, 0, 0]), "minecraft:gold_block");
    assert_eq!(at(&x, [3, 0, 0]), "minecraft:gold_block");

    // `air` is a role in `where` too, which is what makes "only where nothing
    // stands yet" a property of every solid instead of a verb of its own.
    let y = run(
        &doc(
            r#""palette": { "rock": "minecraft:stone", "gold": "minecraft:gold_block" },
               "ops": [
                 { "op": "box", "role": "rock", "from": [0,0,0], "to": [1,0,0] },
                 { "op": "box", "role": "gold", "where": ["air"] } ]"#,
        ),
        [4, 1, 1],
    );
    assert_eq!(at(&y, [0, 0, 0]), "minecraft:stone");
    assert_eq!(at(&y, [2, 0, 0]), "minecraft:gold_block");
}

/// A counted `repeat` moves the scope's origin and keeps its extents; the index
/// is a parameter the body reads.
#[test]
fn a_counted_repeat_stamps_along_its_step_and_binds_its_index() {
    let x = run(
        &doc(r#""palette": { "tread": "minecraft:polished_andesite" },
               "ops": [ { "op": "repeat", "from": [0,0,0], "to": [0,0,0],
                          "step": [1,1,0], "count": 4, "index": "k",
                          "body": [ { "op": "box", "role": "tread" } ] } ]"#),
        [4, 4, 1],
    );
    assert_eq!(
        filled(&x),
        [[0, 0, 0], [1, 1, 0], [2, 2, 0], [3, 3, 0]]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "a flight climbs the diagonal in one operation"
    );

    // The index, read as a parameter: the body's own box moves with `k`.
    let y = run(
        &doc(r#""palette": { "tread": "minecraft:polished_andesite" },
               "ops": [ { "op": "repeat", "from": [0,0,0], "to": [3,0,0],
                          "step": [0,1,0], "count": 3, "index": "k",
                          "body": [ { "op": "box", "role": "tread",
                                      "from": [{"expr":"param","name":"k"},0,0],
                                      "to": [3,0,0] } ] } ]"#),
        [4, 3, 1],
    );
    assert_eq!(
        filled(&y).iter().filter(|c| c[1] == 0).count(),
        4,
        "course 0 starts at x=0"
    );
    assert_eq!(filled(&y).iter().filter(|c| c[1] == 1).count(), 3);
    assert_eq!(filled(&y).iter().filter(|c| c[1] == 2).count(), 2);
}

/// A fitted `repeat` divides the scope's own axis, and `remainder` says where
/// the cells the items do not cover go — the difference the three words make,
/// read as the cells that hold something.
#[test]
fn a_fitted_repeat_puts_its_remainder_where_it_is_told() {
    let bays = |remainder: &str| {
        let x = run(
            &doc(&format!(
                r#""palette": {{ "pier": "minecraft:stone" }},
                   "ops": [ {{ "op": "repeat", "from": [0,0,0], "to": [9,0,0],
                              "along": "x", "stride": 4, "item": 1,
                              "remainder": "{remainder}",
                              "body": [ {{ "op": "box", "role": "pier" }} ] }} ]"#
            )),
            [10, 1, 1],
        );
        filled(&x).iter().map(|c| c[0]).collect::<Vec<i32>>()
    };
    // E = 10, item = 1, stride = 4: n = ⌊9/4⌋ + 1 = 3 items covering 9, r = 1.
    assert_eq!(bays("end"), [0, 4, 8]);
    assert_eq!(bays("start"), [1, 5, 9]);
    assert_eq!(
        bays("middle"),
        [0, 4, 8],
        "the lower-middle split: ⌊1/2⌋ = 0"
    );

    // With two cells over, `middle` splits them one and one.
    let x = run(
        &doc(r#""palette": { "pier": "minecraft:stone" },
               "ops": [ { "op": "repeat", "from": [0,0,0], "to": [10,0,0],
                          "along": "x", "stride": 4, "item": 1, "remainder": "middle",
                          "body": [ { "op": "box", "role": "pier" } ] } ]"#),
        [11, 1, 1],
    );
    assert_eq!(
        filled(&x).iter().map(|c| c[0]).collect::<Vec<i32>>(),
        [1, 5, 9]
    );
}

/// A `mirror` runs the body, then runs it again reflected across the centre
/// plane of its own box — exactly for an odd extent and for an even one.
#[test]
fn a_mirror_runs_the_body_twice_and_the_reflection_is_exact() {
    let odd = run(
        &doc(r#""palette": { "pier": "minecraft:stone" },
               "ops": [ { "op": "mirror", "axis": "x", "from": [0,0,0], "to": [6,0,0],
                          "body": [ { "op": "box", "role": "pier",
                                      "from": [1,0,0], "to": [1,0,0] } ] } ]"#),
        [7, 1, 1],
    );
    assert_eq!(
        odd.iter_x(),
        vec![1, 5],
        "a cell at c lands at n - 1 - c: 1 and 5 of 7"
    );
    let even = run(
        &doc(r#""palette": { "pier": "minecraft:stone" },
               "ops": [ { "op": "mirror", "axis": "x", "from": [0,0,0], "to": [5,0,0],
                          "body": [ { "op": "box", "role": "pier",
                                      "from": [1,0,0], "to": [1,0,0] } ] } ]"#),
        [6, 1, 1],
    );
    assert_eq!(even.iter_x(), vec![1, 4]);
}

/// A convenience for the mirror test: the `x` of every filled cell.
trait FilledX {
    fn iter_x(&self) -> Vec<i32>;
}

impl FilledX for Execution {
    fn iter_x(&self) -> Vec<i32> {
        filled(self).iter().map(|c| c[0]).collect()
    }
}

/// A `claim` names its box for the contract and runs its body inside it, so the
/// operation that claims a gate's box is the operation that paints it — and the
/// mark inside lands where `gate_anchor` reads it.
#[test]
fn a_claim_names_its_box_paints_it_and_marks_it_in_one_operation() {
    let x = run(
        &doc(
            r#""palette": { "bars": "minecraft:iron_bars[east=true,north=false,south=false,waterlogged=false,west=true]" },
               "contract": { "entry": "room",
                             "spaces": { "room": { "envelope": "open" } },
                             "edges": [ { "a": "room", "b": "exterior", "class": "barred",
                                          "bar": { "region": "portcullis", "block": "bars" } } ] },
               "ops": [
                 { "op": "claim", "region": "room", "from": [0,0,1], "to": [4,2,2], "body": [] },
                 { "op": "claim", "region": "portcullis", "from": [1,0,0], "to": [3,2,0],
                   "body": [ { "op": "box", "role": "bars" },
                             { "op": "mark", "mark": { "anchor": "gate", "at": "floor_center" } } ] } ]"#,
        ),
        [5, 3, 3],
    );
    let contract = x
        .expansion
        .contract
        .as_ref()
        .expect("the contract resolves");
    let bar = contract.edges[0].bar.as_ref().expect("a barred edge");
    assert_eq!(bar.region, "portcullis");
    assert_eq!(bar.boxes.len(), 1);
    assert_eq!(bar.boxes[0].origin, [1, 0, 0]);
    assert_eq!(bar.boxes[0].size, [3, 3, 1]);
    // The cells the claim named are the cells its body painted.
    assert_eq!(filled(&x).len(), 9);
    // And the anchor stands in the middle of the claimed box's floor.
    let anchor = &x.expansion.anchors["anchor/gate"];
    assert_eq!(
        anchor.pos,
        [2, 0, 0],
        "`floor_center` is the middle of the CLAIMED box's floor, and the claim is one cell deep"
    );
}

/// A `scope` is a box its body's coordinates are local to, so a block of
/// operations moves as one when the plan moves it.
#[test]
fn a_scope_moves_a_block_of_operations_as_one() {
    let corner = |from: &str, to: &str| {
        run(
            &doc(&format!(
                r#""palette": {{ "pier": "minecraft:stone" }},
                   "ops": [ {{ "op": "scope", "from": {from}, "to": {to}, "body": [
                     {{ "op": "box", "role": "pier", "from": [0,0,0], "to": [1,0,0] }} ] }} ]"#
            )),
            [8, 1, 8],
        )
    };
    assert_eq!(
        filled(&corner("[0,0,0]", "[3,0,3]")),
        [[0, 0, 0], [1, 0, 0]].into_iter().collect::<BTreeSet<_>>()
    );
    assert_eq!(
        filled(&corner("[4,0,4]", "[7,0,7]")),
        [[4, 0, 4], [5, 0, 4]].into_iter().collect::<BTreeSet<_>>(),
        "the same body, one `scope` field moved"
    );
}

/// The run report says what did nothing, by address — dead text in the document
/// of record, listed rather than refused.
#[test]
fn an_operation_that_paints_nothing_is_listed_with_its_address() {
    let x = run(
        &doc(r#""params": { "h": 0 },
               "palette": { "pier": "minecraft:stone" },
               "ops": [
                 { "op": "box", "role": "pier", "from": [0,0,0], "to": [1,0,0] },
                 { "op": "box", "role": "pier", "from": [0,0,0], "to": [1,0,0],
                   "when": { "cond": "cmp", "lhs": { "expr": "param", "name": "h" },
                             "op": "gt", "rhs": { "expr": "int", "value": 0 } } },
                 { "op": "box", "role": "pier", "from": [0,0,0], "to": [1,0,0],
                   "where": ["air"] } ]"#),
        [4, 1, 1],
    );
    assert_eq!(
        x.report.silent,
        vec!["/ops/1".to_string(), "/ops/2".to_string()],
        "the guarded one never ran; the `where` one ran and painted nothing"
    );
    let activity: BTreeMap<&str, (u64, u64)> = x
        .report
        .activity
        .iter()
        .map(|(k, a)| (k.as_str(), (a.instances, a.cells)))
        .collect();
    assert_eq!(activity["/ops/0"], (1, 2));
    assert_eq!(activity["/ops/1"], (0, 0));
    assert_eq!(activity["/ops/2"], (1, 0));
}

// ---------------------------------------------------------------------------
// grammar — the box-split expander, kept for what it was adopted for
// ---------------------------------------------------------------------------

/// **A `grammar` operation expands a program into its own box**, at the literal
/// seed the operation writes, and its marks join the drawing's anchors.
///
/// Its cells overwrite what was drawn where the program wrote a block, and leave
/// it where the program wrote air: spec-0072 §4.5 asks for `fill` and `void` to
/// overwrite and `skip` to leave, and an expansion cannot tell those apart — a
/// `VoxelModel` starts as air and `void` writes air, so a voided cell and a
/// skipped one are the same byte. The rule here is the one the compiler's
/// `fragment` stamp already applies, and it is stated rather than approximated.
#[test]
fn a_grammar_operation_expands_a_program_into_its_box_and_hands_over_its_marks() {
    use std::path::Path;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("drawing-grammar");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A program that fills its box and marks the middle of its floor.
    std::fs::write(
        dir.join("colonnade.json"),
        r#"{ "version": "1.9.0", "name": "colonnade", "start": "bay",
             "palette": { "shaft": "minecraft:polished_andesite" },
             "rules": { "bay": [ { "body": {
               "op": "mark",
               "mark": { "anchor": "bay", "at": "floor_center" },
               "body": { "op": "fill", "material": { "role": "shaft" } } } } ] } }"#,
    )
    .unwrap();
    let drawing: Drawing =
        serde_json::from_str(&doc(r#""palette": { "ground": "minecraft:stone" },
           "ops": [
             { "op": "box", "role": "ground", "from": [0,0,0], "to": [5,0,5] },
             { "op": "grammar", "program": "colonnade.json", "seed": 7,
               "from": [1,1,1], "to": [3,2,3] } ]"#))
        .unwrap();
    let x = execute::execute(
        &drawing,
        Box3::at_origin([6, 3, 6]),
        &ExecuteOptions::seeded(0, &dir),
    )
    .expect("the program expands into the operation's box");

    // The program's blocks, inside the operation's box and nowhere else.
    assert_eq!(at(&x, [1, 1, 1]), "minecraft:polished_andesite");
    assert_eq!(at(&x, [3, 2, 3]), "minecraft:polished_andesite");
    assert_eq!(at(&x, [0, 1, 0]), "minecraft:air");
    assert_eq!(
        at(&x, [0, 0, 0]),
        "minecraft:stone",
        "the drawing's own floor"
    );

    // Its mark joins the drawing's anchors, rebased onto the place's box: the
    // programs's box starts at y=1, and `floor_center` is that box's own floor.
    assert_eq!(x.expansion.anchors["anchor/bay"].pos, [2, 1, 2]);

    // The program is named in the run's provenance, with its own hash, so a row
    // that promises byte reproducibility names every document it read.
    assert_eq!(x.programs.len(), 1);
    assert_eq!(x.programs[0].0, "colonnade.json");
    assert!(x.programs[0].1.starts_with("sha256:"));

    // A cell the program left as air leaves what the drawing drew: the
    // operation's box reaches y=1..2 over the floor, and the floor at y=0 is
    // untouched by it either way.
    assert!(x.report.cells_painted > 0);
}

/// A `grammar` operation that names an absolute path is refused: a document
/// naming one machine's paths builds on that machine and on no other.
#[test]
fn a_grammar_operation_with_an_absolute_program_path_is_refused() {
    let drawing: Drawing = serde_json::from_str(&doc(r#""palette": {},
           "ops": [ { "op": "grammar", "program": "/etc/colonnade.json" } ]"#))
    .unwrap();
    let e = execute::execute(
        &drawing,
        Box3::at_origin([4, 4, 4]),
        &ExecuteOptions::seeded(0, "."),
    )
    .expect_err("an absolute path is refused");
    assert_eq!(e.code.id(), "DW0100");
    assert!(e.to_string().contains("relative"), "{e}");
}
