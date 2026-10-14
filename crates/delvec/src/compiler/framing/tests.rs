//! spec-0110 criteria 5, 6 and 8, over synthetic worlds: the frame, the
//! sight lines, and the sent / drawn bounds.

use super::*;
use std::collections::BTreeSet;

/// A lens pose facing +Z, level.
fn pose_at(pos: [f64; 3]) -> ([f64; 3], f64, f64) {
    (pos, 0.0, 0.0)
}

const LENS: [f64; 3] = [0.5, 0.5, 0.5];

fn none(_: [i32; 3]) -> bool {
    false
}

/// A point `dz` ahead of `LENS` whose normalised device y is `ndc_y` at a
/// vertical field of view of 70°.
fn at_ndc_y(dz: f64, ndc_y: f64) -> [f64; 3] {
    let ty = (PROOF_FOV_DEG.to_radians() / 2.0).tan();
    [LENS[0], LENS[1] + ndc_y * dz * ty, LENS[2] + dz]
}

// --- criterion 5: the frame ------------------------------------------------

#[test]
fn a_reference_at_y_095_is_out_of_frame_and_at_090_is_in() {
    let out = at_ndc_y(10.0, 0.95);
    let b = CellBox::cell(cell_of(out));
    let j = frame_and_sight(&[pose_at(LENS)], &[out], &[b], &none, &none);
    assert_eq!(j.in_frame, vec![false]);
    let msg = j.first_failure.expect("out of frame is a refusal");
    assert!(msg.contains("at tick 0"), "{msg}");
    assert!(msg.contains("out of frame"), "{msg}");

    let inside = at_ndc_y(10.0, 0.90);
    let b = CellBox::cell(cell_of(inside));
    let j = frame_and_sight(&[pose_at(LENS)], &[inside], &[b], &none, &none);
    assert_eq!(j.in_frame, vec![true]);
    assert_eq!(j.first_failure, None);
}

#[test]
fn a_lens_in_water_narrows_the_judged_angle_to_60_degrees() {
    // ndc y 0.8 at 70° is tan(35°)·0.8 / tan(30°) ≈ 0.97 at 60°.
    let p = at_ndc_y(10.0, 0.8);
    let b = CellBox::cell(cell_of(p));
    let dry = frame_and_sight(&[pose_at(LENS)], &[p], &[b], &none, &none);
    assert_eq!(dry.first_failure, None, "passes at 70°");
    let water = |c: [i32; 3]| c == [0, 0, 0];
    let wet = frame_and_sight(&[pose_at(LENS)], &[p], &[b], &water, &none);
    assert_eq!(wet.in_frame, vec![false]);
    assert!(wet.first_failure.unwrap().contains("60° vertical"));
}

// --- criterion 6: sight -----------------------------------------------------

/// A 3 × 3 × 1 box ten blocks ahead.
fn wide_box() -> CellBox {
    CellBox {
        min: [-1, -1, 10],
        max: [1, 1, 10],
    }
}

#[test]
fn a_pillar_on_the_centre_line_leaves_eight_lines_clear() {
    let solid: BTreeSet<[i32; 3]> = [[0, 0, 5]].into_iter().collect();
    let blocks = |c: [i32; 3]| solid.contains(&c);
    let b = wide_box();
    let j = frame_and_sight(&[pose_at(LENS)], &[b.centre()], &[b], &none, &blocks);
    assert_eq!(j.lines_clear, vec![8]);
    assert_eq!(j.first_failure, None);
}

#[test]
fn a_wall_across_all_nine_lines_is_occluded_naming_the_cell() {
    let solid: BTreeSet<[i32; 3]> = (-4..=4)
        .flat_map(|x| (-4..=4).map(move |y| [x, y, 5]))
        .collect();
    let blocks = |c: [i32; 3]| solid.contains(&c);
    let b = wide_box();
    let j = frame_and_sight(&[pose_at(LENS)], &[b.centre()], &[b], &none, &blocks);
    assert_eq!(j.lines_clear, vec![0]);
    let msg = j.first_failure.expect("a wall is a refusal");
    assert!(msg.contains("occluded"), "{msg}");
    assert!(
        msg.contains("[0, 0, 5]"),
        "names the centre line's cell: {msg}"
    );
}

#[test]
fn a_bell_whose_own_cell_is_the_only_block_is_not_hidden_by_itself() {
    let bell = [0, 0, 10];
    let blocks = |c: [i32; 3]| c == bell;
    let b = CellBox::cell(bell);
    let j = frame_and_sight(&[pose_at(LENS)], &[b.centre()], &[b], &none, &blocks);
    assert_eq!(j.lines_clear, vec![9]);
    assert_eq!(j.first_failure, None);
}

#[test]
fn a_place_framed_from_outside_is_not_hidden_by_its_own_roof() {
    // A 5 × 3 × 5 place whose roof course is solid; the lens stands above and
    // in front of it, looking down, so every line crosses the roof.
    let place = CellBox {
        min: [-2, -6, 6],
        max: [2, -4, 10],
    };
    let roof: BTreeSet<[i32; 3]> = (-2..=2)
        .flat_map(|x| (6..=10).map(move |z| [x, -4, z]))
        .collect();
    let blocks = |c: [i32; 3]| roof.contains(&c);
    let pose = (LENS, 0.0, 40.0);
    let j = frame_and_sight(&[pose], &[place.centre()], &[place], &none, &blocks);
    assert!(j.lines_clear[0] >= 1, "{:?}", j.lines_clear);
    assert_eq!(j.first_failure, None, "{:?}", j.first_failure);
}

#[test]
fn sight_points_stay_inside_a_one_cell_box_and_are_distinct() {
    let b = CellBox::cell([4, 70, -3]);
    let ps = b.sight_points();
    assert_eq!(ps[0], b.centre());
    for p in ps {
        for (a, v) in p.iter().enumerate() {
            assert!(*v > b.min[a] as f64 && *v < b.max[a] as f64 + 1.0);
        }
    }
    for p in &ps[1..] {
        assert_ne!(*p, ps[0]);
    }
}

// --- criterion 8: sent ------------------------------------------------------

fn display() -> EntityBody {
    body_of("minecraft:item_display").expect("pinned table")
}

#[test]
fn an_assembly_root_161_blocks_off_is_not_sent_and_at_159_it_is() {
    let lens = [0.0, 70.0, 0.0];
    let id = "minecraft:item_display";
    let r = sent_and_drawn(7, lens, [161.0, 70.0, 0.0], id, display(), 320.0, false);
    let msg = r.expect_err("161 > 160");
    assert!(msg.contains("at tick 7"), "{msg}");
    assert!(msg.contains("161"), "{msg}");
    assert!(msg.contains("160"), "{msg}");
    assert!(sent_and_drawn(7, lens, [159.0, 70.0, 0.0], id, display(), 320.0, false).is_ok());
    // ... and its view_range is derived to cover the 159 blocks it is framed
    // from: 159 / 32 = 4.97.
    let (range, won) = derive_range(None, Some(159.0 / 32.0), 0.0);
    assert_eq!((range, won), (4.97, "shots"));
    assert!(range * 32.0 >= 159.0);
}

#[test]
fn a_mannequin_100_blocks_off_is_sent_and_refused_as_not_drawn() {
    let m = body_of("minecraft:mannequin").expect("pinned table");
    assert_eq!(m.tracking_blocks().min(320.0), 320.0, "512 capped at 320");
    let msg = sent_and_drawn(
        0,
        [0.0, 70.0, 0.0],
        [100.0, 70.0, 0.0],
        "minecraft:mannequin",
        m,
        320.0,
        true,
    )
    .expect_err("beyond the cull radius");
    assert!(msg.contains("cull radius of 64"), "{msg}");
    assert!(!msg.contains("tracking range"), "{msg}");
}
