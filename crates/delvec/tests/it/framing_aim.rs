//! spec-0110 criterion 3: the aim follows the subject unless told otherwise.
//! An unstyled shot with a `subject` and no `look_at` emits every keyframe's
//! yaw/pitch toward the subject's reference point; with `look_at` the aim is
//! `look_at`'s; a `"landscape"` keeps the travel aim the unstyled shot always
//! had, byte for byte.

mod common;

use common::framing_fixture as ff;
use serde_json::json;

/// The exit's cell, `[5, 65, 8]`, plus `offset`, at the block centre.
fn exit_point(offset: [i32; 3]) -> [f64; 3] {
    [
        5.5 + offset[0] as f64,
        65.5 + offset[1] as f64,
        8.5 + offset[2] as f64,
    ]
}

fn dolly(extra: serde_json::Value) -> serde_json::Value {
    let mut shot = json!({"seconds": 2, "path": [
        {"anchor": "anchor/exit", "offset": [-3, 2, -1]},
        {"anchor": "anchor/exit", "offset": [-3, 2, 1]}
    ]});
    for (k, v) in extra.as_object().unwrap() {
        shot[k] = v.clone();
    }
    shot
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() <= 0.01 && (a.1 - b.1).abs() <= 0.01
}

#[test]
fn an_unstyled_shot_aims_at_its_subject_at_every_keyframe() {
    let out = ff::green(&ff::quests_with_cutscene(ff::cutscene(vec![dolly(
        json!({"subject": {"anchor": "anchor/exit"}}),
    )])));
    let kf = ff::keyframes(&out);
    assert!(kf.len() > 2, "a dolly has keyframes: {kf:?}");
    let reference = exit_point([0, 0, 0]);
    for (t, pos, yaw, pitch) in kf {
        assert!(
            close((yaw, pitch), ff::aim(pos, reference)),
            "tick {t}: aim ({yaw}, {pitch}) is not toward the subject from {pos:?}"
        );
    }
}

#[test]
fn look_at_overrides_the_aim() {
    let out = ff::green(&ff::quests_with_cutscene(ff::cutscene(vec![dolly(
        json!({
            "subject": {"anchor": "anchor/exit"},
            "look_at": {"anchor": "anchor/exit", "offset": [0, 1, 1]}
        }),
    )])));
    let target = exit_point([0, 1, 1]);
    for (t, pos, yaw, pitch) in ff::keyframes(&out) {
        assert!(
            close((yaw, pitch), ff::aim(pos, target)),
            "tick {t}: aim ({yaw}, {pitch}) is not toward look_at from {pos:?}"
        );
    }
}

#[test]
fn a_landscape_keeps_the_travel_aim_byte_for_byte() {
    let out = ff::green(&ff::quests_with_cutscene(ff::cutscene(vec![dolly(
        json!({"subject": "landscape"}),
    )])));
    // The travel aim is the pre-subject keyframe plan: `plan_shot` with no
    // subject over the same waypoints and ticks.
    let travel = delvec::compiler::camera::plan_shot(
        &[exit_point([-3, 2, -1]), exit_point([-3, 2, 1])],
        None,
        40,
    );
    let kf = ff::keyframes(&out);
    assert_eq!(kf.len(), travel.frames.len());
    for ((t, pos, yaw, pitch), f) in kf.iter().zip(&travel.frames) {
        assert_eq!(*t, f.tick);
        assert_eq!(*pos, f.pos);
        assert_eq!((*yaw, *pitch), (f.yaw, f.pitch), "tick {t}");
    }
}

/// The build refuses a shot whose aim leaves its subject out of the frame:
/// `look_at` lifted twenty blocks over the exit stares at the ceiling.
#[test]
fn a_look_at_that_leaves_the_subject_out_of_frame_is_dw1033() {
    let q = ff::quests_with_cutscene(ff::cutscene(vec![dolly(json!({
        "subject": {"anchor": "anchor/exit"},
        "look_at": {"anchor": "anchor/exit", "offset": [0, 20, 0]}
    }))]));
    let m = ff::refusal(&q, "DW1033");
    assert!(m.contains("out of frame") && m.contains("at tick 0"), "{m}");
}
