//! **One seat, every reader** — the agreement test for the respawn seat
//! (`nav::World::rest_seat`, `docs/reference/compiler.md`, *Respawn seat*).
//!
//! A checkpoint's rest object (its anchor) and its seat (the cell a respawn
//! puts a body on) are two cells. Every consumer of "where does a respawn put
//! the party" is asked the same generated cases here and must give one answer:
//! the emitted re-seat teleport, the `spawnpoint` and `dw:cp` mirror the beat
//! writes, the generated PackTest's read-back, the combat plan's governing
//! checkpoint the die-retry stage asserts, and the respawn safe zone's ledger
//! (`DW0478`). The rest object keeps the cell the bot rests at.
//!
//! The cases, on the `die-retry` fixture (one `set-checkpoint` on
//! `anchor/keeper-stand`, one mandatory wave):
//! * the anchor on open floor — the seat is the anchor;
//! * a lit campfire laid in the floor under the anchor (a world edit), so a
//!   body on the anchor stands with its feet in the fire — the seat is beside
//!   it (the keeper is posted on the anchor, so the fire cannot stand in the
//!   anchor's own cell; that shape is the gallery's hearth);
//! * the same, with the cell that seat stood on walled — the seat moves to
//!   another side, and every reader moves with it.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::Value;

const NS: &str = "die-retry";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// A world-edits document laying `block` at each anchor-relative offset of
/// `anchor/keeper-stand`, one batch.
fn edits(laid: &[([i32; 3], &str)]) -> String {
    let mut ops = Vec::new();
    for (i, (offset, block)) in laid.iter().enumerate() {
        let name = format!("region/laid-{i}");
        ops.push(serde_json::json!({
            "name": name,
            "shape": {
                "frame": {"anchor": "anchor/keeper-stand", "kind": "anchor-relative"},
                "kind": "box",
                "min": offset,
                "max": offset,
            },
            "verb": "select",
        }));
        ops.push(serde_json::json!({
            "recipe": {"blocks": [{"block": block, "weight": 1.0}]},
            "region": name,
            "verb": "fill",
        }));
    }
    let doc = serde_json::json!({
        "campaign_id": NS,
        "content": {"batches": [{
            "area": "area/keep",
            "edits": ops,
            "id": "batch/the-fire",
            "note": "A campfire under the checkpoint's cell: the rest object stops being a seat.",
        }]},
        "dsl_version": common::VERSION_TOKEN,
        "stage": "world-edits",
    });
    common::at_dsl_version(&doc.to_string())
}

fn campaign(world_edits: Option<String>) -> Campaign {
    let dir = fixture_dir();
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("campaign parses")
}

/// The plan (for the rest object's cell) and the build.
fn build(c: &Campaign) -> (Plan<'_>, BuildOutput) {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out = emit::build(
        &plan,
        &common::campaign_inputs(&fixture_dir()),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"));
    (plan, out)
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("{path} is emitted")),
    )
    .unwrap()
}

fn json(out: &BuildOutput, path: &str) -> Value {
    serde_json::from_str(text(out, path)).unwrap()
}

fn ints(s: &str) -> Vec<f64> {
    s.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter(|t| !t.is_empty())
        .map(|t| t.parse().unwrap())
        .collect()
}

fn cell(v: &[f64]) -> [i32; 3] {
    [v[0].floor() as i32, v[1].floor() as i32, v[2].floor() as i32]
}

/// Every reader's answer, named.
fn readings(out: &BuildOutput) -> Vec<(&'static str, [i32; 3])> {
    let mut r = Vec::new();
    let seat_fn = text(out, &format!("datapack/data/{NS}/function/cp_seat_0.mcfunction"));
    let tp = seat_fn.trim().strip_prefix("tp @s ").expect("a re-seat teleport");
    r.push(("the re-seat teleport (cp_seat_0)", cell(&ints(tp))));
    // The beat that sets the checkpoint writes both lines in one function
    // (the setup seeds `dw:cp` with the entry spawn, which is not this seat).
    let mut spawn = None;
    let mut mirror = None;
    for (path, bytes) in out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        let Some(sp) = body.lines().find_map(|l| l.strip_prefix("spawnpoint @a ")) else {
            continue;
        };
        assert!(spawn.is_none(), "one function sets the checkpoint");
        spawn = Some(cell(&ints(sp)));
        mirror = body
            .lines()
            .find_map(|l| l.strip_prefix("data modify storage dw:cp pos set value "))
            .map(|m| cell(&ints(m)));
    }
    r.push(("the beat's spawnpoint", spawn.expect("a spawnpoint line")));
    r.push(("the beat's dw:cp mirror", mirror.expect("a dw:cp line")));
    // The generated PackTest reads dw:cp back per axis against the seat.
    let pt = text(
        out,
        &format!("packtest-datapack/data/{NS}/test/v06_checkpoint_respawn.mcfunction"),
    );
    let wants: Vec<f64> = pt
        .lines()
        .filter(|l| l.starts_with("assert score #") && l.contains("_cpr dw.sys matches"))
        .map(|l| ints(l.rsplit("matches").next().unwrap())[0])
        .collect();
    assert_eq!(wants.len(), 3, "three axes asserted: {pt}");
    r.push(("the PackTest read-back", cell(&wants)));
    let combat = json(out, "validation/combat-plan.json");
    let governing: Vec<[i32; 3]> = combat["encounters"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e.get("checkpoint"))
        .map(|c| {
            let v: Vec<f64> = c.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
            cell(&v)
        })
        .collect();
    assert_eq!(governing.len(), 1, "one encounter, governed by the checkpoint");
    r.push(("the combat plan's governing checkpoint", governing[0]));
    let safety = json(out, "validation/respawn-safety.json");
    let rest = safety["rest_points"]
        .as_array()
        .unwrap_or_else(|| panic!("rest points listed: {safety}"))
        .iter()
        .find(|p| p["anchor"] == "anchor/keeper-stand")
        .unwrap_or_else(|| panic!("the checkpoint is a rest point: {safety}"));
    let v: Vec<f64> = rest["pos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect();
    r.push(("the respawn safe zone (DW0478)", cell(&v)));
    // The death plan's seats, when the build writes one (a campaign with a
    // killing volume or a stake): the bot's "respawned at the governing seat".
    if let Some(bytes) = out.get("validation/death-plan.json") {
        let plan: Value = serde_json::from_slice(bytes).unwrap();
        for s in plan["placement"]["seats"].as_array().unwrap() {
            if s["cp"] == 0 {
                let v: Vec<f64> = s["cell"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| x.as_f64().unwrap())
                    .collect();
                r.push(("the death plan's seat", cell(&v)));
            }
        }
    }
    r
}

/// One case: every reader agrees on one seat, and that seat is returned.
fn agreed(out: &BuildOutput) -> [i32; 3] {
    let r = readings(out);
    let seat = r[0].1;
    for (who, at) in &r {
        assert_eq!(*at, seat, "{who} reads {at:?}, the re-seat reads {seat:?}: {r:?}");
    }
    seat
}

#[test]
fn every_reader_of_a_respawn_reads_one_seat() {
    // Open floor: the anchor is its own seat.
    let plain = campaign(None);
    let (plan, out) = build(&plain);
    let anchor = plan.checkpoints[0].at;
    assert_eq!(agreed(&out), anchor, "a safe anchor is its own seat");

    // A campfire under the anchor: the seat is beside the rest object.
    let fire = campaign(Some(edits(&[([0, -1, 0], "minecraft:campfire[lit=true]")])));
    let (plan, out) = build(&fire);
    assert_eq!(plan.checkpoints[0].at, anchor, "the rest object does not move");
    let beside = agreed(&out);
    assert_ne!(beside, anchor, "never over the campfire");
    let ring = (0..3).map(|i| (beside[i] - anchor[i]).abs()).max().unwrap();
    assert_eq!(ring, 1, "the seat is next to the rest object: {beside:?} by {anchor:?}");

    // Wall the cell that seat stood on: the seat moves to another side, and
    // every reader moves with it.
    let off = [beside[0] - anchor[0], beside[1] - anchor[1], beside[2] - anchor[2]];
    let walled = campaign(Some(edits(&[
        ([0, -1, 0], "minecraft:campfire[lit=true]"),
        (off, "minecraft:stone_bricks"),
        ([off[0], off[1] + 1, off[2]], "minecraft:stone_bricks"),
    ])));
    let (_, out) = build(&walled);
    let moved = agreed(&out);
    assert_ne!(moved, beside, "the seat leaves the walled cell");
    assert_ne!(moved, anchor, "and never lands on the fire");
}
