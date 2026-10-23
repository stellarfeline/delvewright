//! spec-0110 criterion 9: a body is drawn. An assembly is made drawable by
//! the engine — `asm_summon_<s>` carries `view_range` on the root and every
//! part, `max(1.0, seen_from/32, d_max/32)` to two decimals, the ledger
//! naming the term that won — and an NPC or actor is judged at the proof
//! frame against its type's cull radius.

mod common;

use common::framing_fixture as ff;
use delvec::compiler::framing::{
    ShotLedger, ShotRecord, SubjectRecord, derive_range, entity_body, sent_and_drawn,
};
use serde_json::json;

/// Every `view_range:` the limb's summon carries, and how many summons.
fn ranges(out: &delvec::compiler::emit::BuildOutput) -> (Vec<String>, usize) {
    let f = ff::function(out, "asm_summon_limb");
    let summons = f.lines().filter(|l| l.starts_with("summon ")).count();
    let rs = f
        .lines()
        .filter_map(|l| l.split("view_range:").nth(1))
        .map(|r| r.split('f').next().unwrap().to_string())
        .collect();
    (rs, summons)
}

#[test]
fn the_root_and_every_part_carry_the_derived_view_range() {
    let q = ff::quests_with_cutscene(ff::cutscene(vec![ff::shot(
        [-3, 2, 0],
        json!({"assembly": "assembly/limb"}),
    )]));
    let out = ff::green(&q);
    let (rs, summons) = ranges(&out);
    // The root, three parts and the hitbox interaction: the displays carry it.
    assert_eq!(rs.len(), 4, "root + 3 parts: {rs:?}");
    assert!(summons >= 4);
    assert!(rs.iter().all(|r| r == "1.00"), "{rs:?}");
    let a = &ff::ledger(&out)["assemblies"][0];
    assert_eq!(a["won"], "floor");
    assert_eq!(a["view_range"], 1.0);
    let shots = a["terms"]["shots"].as_f64().unwrap();
    assert!(shots > 0.0 && shots < 1.0, "{a}");
}

#[test]
fn seen_from_raises_the_range_and_the_ledger_says_so() {
    let mut q = ff::quests_with_cutscene(ff::cutscene(vec![ff::shot(
        [-3, 2, 0],
        json!({"assembly": "assembly/limb"}),
    )]));
    q["content"]["assemblies"][0]["seen_from"] = json!(100);
    let out = ff::green(&q);
    let (rs, _) = ranges(&out);
    // 100 / 32 = 3.125, rounded up to 3.13.
    assert!(rs.iter().all(|r| r == "3.13"), "{rs:?}");
    assert_eq!(ff::ledger(&out)["assemblies"][0]["won"], "seen_from");
    assert_eq!(
        derive_range(Some(100.0 / 32.0), None, 0.0),
        (3.13, "seen_from")
    );
}

#[test]
fn an_assembly_no_shot_frames_carries_the_floor_from_its_place() {
    let out = common::assembly_fixture::build_fixture();
    let (rs, _) = ranges(&out);
    assert!(rs.len() == 4 && rs.iter().all(|r| r == "1.00"), "{rs:?}");
    assert!(
        !out.contains_key("validation/shot-subjects.json"),
        "no cutscene, no ledger"
    );
}

#[test]
fn a_villager_70_blocks_off_is_not_drawn_and_60_off_is() {
    let v = entity_body("minecraft:villager").unwrap();
    assert!((v.cull_radius() - 67.2).abs() < 1e-9);
    let lens = [0.0, 70.0, 0.0];
    let m = sent_and_drawn(
        3,
        lens,
        [70.0, 70.0, 0.0],
        "minecraft:villager",
        v,
        160.0,
        true,
    )
    .expect_err("70 > 67.2");
    assert!(m.contains("cull radius of 67.2"), "{m}");
    assert!(
        sent_and_drawn(
            3,
            lens,
            [60.0, 70.0, 0.0],
            "minecraft:villager",
            v,
            160.0,
            true
        )
        .is_ok()
    );
    // The binding line states the slack: below entity distance 60 / 67.2 the
    // villager vanishes.
    let ledger = ShotLedger {
        cutscenes: vec![delvec::compiler::framing::CutsceneRecord {
            path: "/content/x".into(),
            digest: "00000000".into(),
            start_lens: [0.0, 70.0, 0.0],
            shots: vec![ShotRecord {
                index: 0,
                start_tick: 0,
                ticks: 0,
                kind: "actor",
                subjects: vec![SubjectRecord {
                    field: "subject",
                    kind: "actor",
                    canon: "c:actor/x@0,0,0".into(),
                    reference: [60.0, 71.0, 0.0],
                    in_frame: vec![true],
                    lines_clear: vec![9],
                    presence: Some("placed by setup".into()),
                    farthest_horizontal: Some(60.0),
                    farthest: Some(60.0),
                    tracking_range: Some(160.0),
                    cull_radius: Some(v.cull_radius()),
                    entity: Some("minecraft:villager".into()),
                    body_at: Some([60.0, 70.0, 0.0]),
                }],
            }],
        }],
        assembly_reach: Default::default(),
    };
    let line = ledger.line();
    assert!(line.contains("hidden below entity distance 0.90"), "{line}");
    assert!(line.contains("farthest 60.0 blocks"), "{line}");
}
