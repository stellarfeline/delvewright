//! spec-0083 — **the route proof takes a teleport as a link.**
//!
//! `tests/fixtures/ferry` is the primary: one area, two places with a solid wall
//! between them and no walk across, a near hull the party boards
//! (`obj/board`), and one `use` trigger, `once: false`, whose `teleport` carries
//! whoever stands in the hull onto the far landing. Every probe below is that
//! primary plus one declared edit, built through the real `delvec build` entry
//! point, so the validation funnel (`DW0933`, `DW0934`) and the build
//! (`DW0311`, `DW0932`) are each reached the way an author reaches them.
//!
//! The piece is synthesised here ([`ferry_prefabs`]) rather than drawn from the
//! content library: the property under test is "two places no walk joins", and
//! a piece that holds it by construction is the only one whose geometry a
//! reader can check from this file.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// The synthesised strait: `21 x 6 x 9`, two sealed rooms side by side with a
/// solid wall at `x = 8` and nothing — no door, no gap — between them.
const SIZE: [i32; 3] = [21, 6, 9];

/// Every non-air cell of the strait.
fn strait_cells() -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut cells = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = x == 0 || x == sx - 1 || y == 0 || y == sy - 1 || z == 0 || z == sz - 1;
                let wall = x == 8;
                if y == sy - 1 && z % 3 == 1 && x % 3 == 1 && x != 8 {
                    cells.push(([x, y, z], "minecraft:glowstone"));
                } else if shell || wall {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
            }
        }
    }
    cells
}

/// The anchors, piece-local. The near hull's volume is `anchor/boat ± [1, 1, 1]`
/// (local x 2..4); the tiller at local x 7 stands outside it and is within a
/// strike (`STRIKE_REACH`, 3.0) of the hull's east column (x 4, eye 2.5 from the
/// tiller's box) and of no other — so the volume shrunk to its middle column
/// holds no stand cell. The far landing is nine blocks east of the stand cell,
/// past the harness's observability floor (`2 x TRANSPORT_NEAR`). `far-air`
/// hangs one block over the east floor, and `far-deck` is the cell under it.
fn strait_anchors() -> Value {
    json!({
        "spawn": { "pos": [1, 1, 1], "facing": "south", "role": "entry" },
        "anchor/boat": { "pos": [3, 1, 4] },
        "anchor/tiller": { "pos": [7, 1, 4] },
        "anchor/far-landing": { "pos": [13, 1, 4] },
        "anchor/far-shore": { "pos": [18, 1, 6] },
        "anchor/far-tiller": { "pos": [9, 1, 4] },
        "anchor/far-air": { "pos": [15, 2, 4] },
        "anchor/far-deck": { "pos": [15, 1, 4] },
    })
}

/// A private prefab library: the pinned one, with every piece declaring its own
/// outside, plus the strait.
fn ferry_prefabs() -> PathBuf {
    let dir = common::shown_prefabs_dir("ferry");
    common::write_single_prefab(
        &dir,
        "ferry-strait",
        SIZE,
        &strait_cells(),
        strait_anchors(),
    );
    common::declare_shown_faces(&dir, "ferry-strait");
    dir
}

/// A private copy of the primary with `quests.json` edited by `patch`.
fn campaign(who: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let src = common::compiler_fixtures_dir().join("ferry");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("teleport-link-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    common::patch_file(&dir.join("quests.json"), patch);
    dir
}

/// What one `delvec build` run said.
struct Run {
    status: i32,
    stderr: String,
    out: PathBuf,
}

impl Run {
    fn green(&self) -> &Self {
        assert_eq!(self.status, 0, "expected a green build:\n{}", self.stderr);
        self
    }

    fn refused(&self, code: &str) -> String {
        assert_ne!(self.status, 0, "expected {code}, got a green build");
        let line = self
            .stderr
            .lines()
            .find(|l| l.starts_with(code) && l.contains("[error]"))
            .unwrap_or_else(|| panic!("expected {code}:\n{}", self.stderr));
        line.to_string()
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(self.out.join(rel))
                .unwrap_or_else(|e| panic!("{rel}: {e}\n{}", self.stderr)),
        )
        .unwrap()
    }
}

fn build(dir: &Path) -> Run {
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(&out)
        .arg("--prefabs")
        .arg(ferry_prefabs())
        .output()
        .expect("delvec runs");
    Run {
        status: o.status.code().unwrap_or(-1),
        // Validation refusals print on stdout and build refusals on stderr;
        // a reader of the run reads both.
        stderr: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    }
}

fn trigger_mut(q: &mut Value) -> &mut Value {
    &mut q["content"]["triggers"][0]
}

/// The trigger step that carries, from `critical-path.json`.
pub(crate) fn carrying_step(path: &Value) -> Option<Value> {
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "trigger" && s.get("transport").is_some())
        .cloned()
}

#[test]
fn a_link_is_a_leg_and_the_path_takes_it() {
    let run = build(&campaign("primary", |_| {}));
    run.green();
    let path = run.json("critical-path.json");
    let step = carrying_step(&path).expect("a trigger step carries the party");
    assert_eq!(step["trigger"], "trigger/tiller");
    // The world origin of the one area is the area's own; the `to` cell and the
    // stand cell are absolute, so read the anchor resolution off the step the
    // compiler wrote and check it against the piece's own geometry.
    let to = step["transport"].as_array().unwrap().clone();
    let stand = step["stand"].as_array().unwrap().clone();
    let pos = step["pos"].as_array().unwrap().clone();
    let v = |a: &Vec<Value>, i: usize| a[i].as_i64().unwrap();
    // The tiller is at local x=7 and the hull's volume spans local x 2..4: the
    // stand cell is the volume's east column, three west of the tiller.
    assert_eq!(
        v(&pos, 0) - v(&stand, 0),
        3,
        "stand {stand:?}, tiller {pos:?}"
    );
    // The far landing is at local x=13: six east of the tiller.
    assert_eq!(v(&to, 0) - v(&pos, 0), 6);
    assert!(
        run.stderr.contains("1 carried by a link"),
        "the binding line counts the carried leg:\n{}",
        run.stderr
    );
    let gate = run.json("validation/teleport-gate.json");
    assert_eq!(gate["teleports"]["links"], 1);
    assert_eq!(gate["teleports"]["gathers"], 0);
    assert_eq!(gate["legs_carried"], 1);
}

#[test]
fn a_way_onward_that_fires_once_is_refused_with_the_link_remedy() {
    let run = build(&campaign("once", |q| {
        trigger_mut(q).as_object_mut().unwrap().remove("once");
    }));
    let line = run.refused("DW0311");
    assert!(line.contains("the `teleport` at"), "{line}");
    assert!(line.contains("\"once\": false"), "{line}");
    for word in ["doorway", "gap", "fence"] {
        assert!(!line.contains(word), "`{word}` in {line}");
    }
}

#[test]
fn a_teleport_in_the_far_objectives_bundle_is_a_gather_and_refused_the_same_way() {
    let run = build(&campaign("gather", |q| {
        let tp = q["content"]["triggers"][0]["effects"][0].clone();
        q["content"]["triggers"] = json!([]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/far-shore"] = json!([tp]);
    }));
    let line = run.refused("DW0311");
    assert!(line.contains("the `teleport` at"), "{line}");
    assert!(line.contains("\"once\": false"), "{line}");
    for word in ["doorway", "gap", "fence"] {
        assert!(!line.contains(word), "`{word}` in {line}");
    }
}

// ---------------------------------------------------------------------------
// Criterion 2 — the gate holds the link shut.
// ---------------------------------------------------------------------------

#[test]
fn a_flag_no_earlier_step_sets_holds_the_link_shut() {
    // `flag/landed` is set by the far shore's own completion: at the leg into
    // the far shore it is not held, so the link does not exist for that leg.
    let run = build(&campaign("shut-flag", |q| {
        trigger_mut(q)["requires_flags"] = json!(["flag/landed"]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/far-shore"] = json!([{
            "type": "set-flag", "flag": "flag/landed",
            "happening": {"verb": "gains", "subject": "anchor/far-shore",
                          "text": "The far shore is reached."}
        }]);
    }));
    let line = run.refused("DW0311");
    assert!(line.contains("`trigger/tiller`"), "{line}");
    assert!(line.contains("shut at this step"), "{line}");
}

#[test]
fn a_flag_the_step_before_sets_opens_the_link() {
    // The primary: `obj/board`'s completion sets `flag/boarded`, which the
    // tiller requires.
    let q: Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("ferry/quests.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        q["content"]["triggers"][0]["requires_flags"],
        json!(["flag/boarded"])
    );
    build(&campaign("open-flag", |_| {})).green();
}

#[test]
fn a_state_term_the_paths_writes_leave_false_holds_the_link_shut() {
    // `state/tide` starts at 0 and nothing on the path writes it, so the
    // teleport's `when` — read through the Flow replay's data — is false at
    // the leg into the far shore.
    let run = build(&campaign("shut-state", |q| {
        q["content"]["state"] = json!([{
            "id": "state/tide", "initial": 0, "scope": "party",
            "note": "Whether the strait is in flood; only reaching the far shore raises it."
        }]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/far-shore"] = json!([{
            "type": "set-state", "state": "state/tide", "value": 1
        }]);
        trigger_mut(q)["effects"][0]["when"] =
            json!({"requires_state": [{"state": "state/tide", "op": "equals", "value": 1}]});
    }));
    let line = run.refused("DW0311");
    assert!(line.contains("`trigger/tiller`"), "{line}");
    assert!(line.contains("shut at this step"), "{line}");
}

// ---------------------------------------------------------------------------
// Criterion 3 — the four faults of DW0932.
// ---------------------------------------------------------------------------

#[test]
fn a_volume_with_no_stand_cell_is_dw0932() {
    // The hull shrunk to its middle column: local x 3, an eye 3.5 from the
    // tiller's box — past a strike.
    let run = build(&campaign("no-stand", |q| {
        trigger_mut(q)["effects"][0]["from"]["extent"] = json!([0, 1, 1]);
    }));
    let line = run.refused("DW0932");
    assert!(line.contains("no stand cell"), "{line}");
}

#[test]
fn a_to_inside_its_own_volume_is_dw0932() {
    let run = build(&campaign("to-inside", |q| {
        trigger_mut(q)["effects"][0]["to"] = json!({"anchor": "anchor/boat"});
    }));
    let line = run.refused("DW0932");
    assert!(line.contains("`to` inside `from`"), "{line}");
}

/// The tiller's bundle as a `sequence`: an optional floor laid under
/// `far-air` at `deck_tick`, and the teleport onto `far-air` at tick 2.
fn over_air(q: &mut Value, deck_tick: Option<u32>) {
    let mut steps = vec![];
    if let Some(t) = deck_tick {
        steps.push(json!({"at_ticks": t, "effects": [{
            "type": "fill-region", "block": "minecraft:stone",
            "region": {"anchor": "anchor/far-deck", "extent": [0, 0, 0]}
        }]}));
    }
    steps.push(json!({"at_ticks": 2, "effects": [{
        "type": "teleport",
        "from": {"anchor": "anchor/boat", "extent": [1, 1, 1]},
        "to": {"anchor": "anchor/far-air"}
    }]}));
    trigger_mut(q)["effects"] = json!([{"type": "sequence", "steps": steps}]);
}

#[test]
fn a_to_over_air_at_the_teleports_tick_is_dw0932() {
    let run = build(&campaign("to-air", |q| over_air(q, None)));
    let line = run.refused("DW0932");
    assert!(line.contains("`to` not standable"), "{line}");
    // A floor laid AFTER the teleport is no floor at its tick.
    let run = build(&campaign("to-air-late", |q| over_air(q, Some(3))));
    let line = run.refused("DW0932");
    assert!(line.contains("`to` not standable"), "{line}");
}

#[test]
fn a_to_over_a_floor_the_same_root_lays_earlier_is_green() {
    build(&campaign("to-deck", |q| over_air(q, Some(0)))).green();
}

#[test]
fn a_link_between_two_areas_is_dw0932() {
    let dir = campaign("two-areas", |q| {
        trigger_mut(q)["effects"][0]["to"] = json!({"anchor": "anchor/keeper-stand"});
    });
    common::patch_file(&dir.join("world.json"), |w| {
        w["content"]["areas"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room"}));
    });
    let run = build(&dir);
    let line = run.refused("DW0932");
    assert!(line.contains("different areas"), "{line}");
}

// ---------------------------------------------------------------------------
// Criterion 4 — the cutscene order.
// ---------------------------------------------------------------------------

/// A one-second cutscene in the west room.
fn cutscene() -> Value {
    json!({
        "type": "cutscene", "seconds": 1,
        "path": [
            {"anchor": "anchor/boat", "offset": [0, 2, 1]},
            {"anchor": "anchor/boat", "offset": [2, 2, 1]}
        ]
    })
}

fn teleport() -> Value {
    json!({
        "type": "teleport",
        "from": {"anchor": "anchor/boat", "extent": [1, 1, 1]},
        "to": {"anchor": "anchor/far-landing"}
    })
}

fn cutscene_then_teleport(q: &mut Value, tick: u32) {
    trigger_mut(q)["effects"] = json!([{"type": "sequence", "steps": [
        {"at_ticks": 0, "effects": [cutscene()]},
        {"at_ticks": tick, "effects": [teleport()]}
    ]}]);
}

/// The tick `cs_end` runs at, read off the emitted driver: the `matches N..`
/// of the line that calls `cs_end_<bare>`.
fn emitted_cs_end_tick(out: &Path) -> u32 {
    let dir = out.join("datapack/data/ferry/function");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with("cs_tick_") {
            continue;
        }
        for line in std::fs::read_to_string(&p).unwrap().lines() {
            if line.contains("run function ferry:cs_end_") && line.starts_with("execute if") {
                let n = line
                    .split("matches ")
                    .nth(1)
                    .unwrap()
                    .split("..")
                    .next()
                    .unwrap();
                return n.parse().unwrap();
            }
        }
    }
    panic!("no cs_tick driver emitted under {}", dir.display());
}

#[test]
fn a_teleport_under_its_roots_cutscene_is_dw0933_at_the_emitted_tick() {
    // Green one tick past `cs_end`, and the emission says which tick that is.
    let green = build(&campaign("cs-after", |q| cutscene_then_teleport(q, 22)));
    green.green();
    let end = emitted_cs_end_tick(&green.out);
    assert_eq!(end, 21, "a one-second shot ends at tick 20 + 1");
    // At the tick `cs_end` runs, refused, naming that tick.
    let run = build(&campaign("cs-at", |q| cutscene_then_teleport(q, end)));
    let line = run.refused("DW0933");
    assert!(line.contains(&format!("runs at tick {end}")), "{line}");
    assert!(
        line.contains(&format!("tick {} or later", end + 1)),
        "{line}"
    );
    // In one flat bundle with the cutscene, refused too.
    let run = build(&campaign("cs-flat", |q| {
        trigger_mut(q)["effects"] = json!([cutscene(), teleport()]);
    }));
    let line = run.refused("DW0933");
    assert!(line.contains("one flat bundle"), "{line}");
}

// ---------------------------------------------------------------------------
// Criterion 11 — the binding line and the ledger.
// ---------------------------------------------------------------------------

#[test]
fn the_binding_line_partitions_every_leg() {
    let run = build(&campaign("binding", |_| {}));
    run.green();
    let line = run
        .stderr
        .lines()
        .find(|l| l.starts_with("DW0311 binding:"))
        .expect("the binding line is printed");
    let nums: Vec<usize> = line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse().unwrap())
        .collect();
    // [0311, legs, walked, crossings, carried, links, live, taken, gathers]
    let (legs, walked, crossings, carried) = (nums[1], nums[2], nums[3], nums[4]);
    assert_eq!(walked + crossings + carried, legs, "{line}");
    // Visited positions: the entry, the hull, the stand cell, the far landing,
    // the far shore — five, so four legs.
    let path = run.json("critical-path.json");
    let positioned = path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("pos").is_some())
        .count();
    let carried_steps = path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("stand").is_some())
        .count();
    // Each carrying trigger stands for two positions (its stand and its `to`).
    assert_eq!(legs, 1 + positioned + carried_steps - 1, "{line}");
    let gate = run.json("validation/teleport-gate.json");
    let t = &gate["teleports"];
    assert_eq!(
        t["links"].as_u64().unwrap() + t["gathers"].as_u64().unwrap(),
        t["declared"].as_u64().unwrap()
    );
}
