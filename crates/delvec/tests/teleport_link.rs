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

/// The synthesised strait: `21 x 8 x 9`, two sealed rooms side by side with a
/// solid wall at `x = 8` and nothing — no door, no gap — between them. The
/// floor is three blocks thick (y 0..2), so a body stands at y 3, and a hole can
/// be cut into it.
const SIZE: [i32; 3] = [21, 8, 9];

/// The one-cell pit [`strait_cells`] cuts when asked: two blocks deep at local
/// `(17, 1..=2, 2)`, in the east room beside the far shore and off its route. A
/// body that walks into it lands at y 1 and cannot climb the two blocks out.
const PIT: [i32; 3] = [17, 1, 2];

/// The raised walk [`strait_cells`] builds when asked, in the east room: a
/// two-block-high run at local `(16..=19, 3..=4, 6)` whose top is reached by
/// one step at `(19, 3, 5)`, at its east end. `anchor/ledge` is the run's west
/// end — a cell a body arrives on only by walking round by the step.
const LEDGE: [i32; 3] = [16, 5, 6];

/// Every non-air cell of the strait, with or without [`PIT`] and [`LEDGE`].
fn strait_cells(pit: bool, ledge: bool) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut cells = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = x == 0 || x == sx - 1 || y <= 2 || y == sy - 1 || z == 0 || z == sz - 1;
                let wall = x == 8;
                let hole = pit && x == PIT[0] && z == PIT[2] && (1..=2).contains(&y);
                if hole {
                    continue;
                }
                let pillar = ledge
                    && ((z == LEDGE[2] && (16..=19).contains(&x) && (3..=4).contains(&y))
                        || (x == 19 && y == 3 && z == 5));
                if pillar {
                    cells.push(([x, y, z], "minecraft:stone"));
                    continue;
                }
                if y == sy - 1 && z % 3 == 1 && x % 3 == 1 && x != 8 {
                    cells.push(([x, y, z], "minecraft:glowstone"));
                } else if pit && x == PIT[0] && z == PIT[2] && y == 0 {
                    // The pit's floor, lit: the light gate is not what this is about.
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
/// `pit` is [`PIT`]'s floor cell and `rescue` a lever on the floor beside it.
fn strait_anchors() -> Value {
    json!({
        "spawn": { "pos": [1, 3, 1], "facing": "south", "role": "entry" },
        "anchor/boat": { "pos": [3, 3, 4] },
        "anchor/tiller": { "pos": [7, 3, 4] },
        "anchor/far-landing": { "pos": [13, 3, 4] },
        "anchor/far-shore": { "pos": [18, 3, 6] },
        "anchor/far-tiller": { "pos": [9, 3, 4] },
        "anchor/far-air": { "pos": [15, 4, 4] },
        "anchor/far-deck": { "pos": [15, 3, 4] },
        "anchor/pit": { "pos": PIT },
        "anchor/rescue": { "pos": [17, 3, 1] },
        "anchor/ledge": { "pos": LEDGE },
    })
}

/// A private prefab library: the pinned one, with every piece declaring its own
/// outside, plus the strait (`prefab/ferry-strait`), the strait with its pit
/// cut (`prefab/ferry-strait-pit`) and the strait with its pillar raised
/// (`prefab/ferry-strait-ledge`).
fn ferry_prefabs() -> PathBuf {
    let dir = common::shown_prefabs_dir("ferry");
    for (id, pit, ledge) in [
        ("ferry-strait", false, false),
        ("ferry-strait-pit", true, false),
        ("ferry-strait-ledge", false, true),
    ] {
        common::write_single_prefab(&dir, id, SIZE, &strait_cells(pit, ledge), strait_anchors());
        common::declare_shown_faces(&dir, id);
    }
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

// ---------------------------------------------------------------------------
// Criterion 12 — the graph draws the carry.
// ---------------------------------------------------------------------------

/// The ferry's layout graph: the west shore and the east shore, the hull a
/// station of the west and the far landing a station of the east, joined by
/// one `carry` edge gated on the flag boarding sets — and by nothing else.
fn ferry_graph(edges: Value) -> Value {
    json!({
        "campaign_id": "ferry", "dsl_version": "0.35.0", "stage": "layout-graph",
        "content": {
            "nodes": [
                {"id": "node/west-shore", "intent": "jetty", "size_class": "room",
                 "stations": [{"anchor": "anchor/boat", "kind": "point"}]},
                {"id": "node/east-shore", "intent": "landing", "size_class": "room",
                 "stations": [{"anchor": "anchor/far-landing", "kind": "point"}]}
            ],
            "edges": edges,
            "entry": "node/west-shore",
            "goal": "node/east-shore",
            "critical_path": ["node/west-shore", "node/east-shore"],
            "beats": [
                {"quest": "quest/cross", "objective": "obj/board", "node": "node/west-shore"},
                {"quest": "quest/cross", "objective": "obj/far-shore", "node": "node/east-shore"}
            ]
        }
    })
}

fn carry_edge() -> Value {
    json!({"class": "carry", "id": "edge/strait", "a": "node/west-shore",
           "b": "node/east-shore", "one_way": "a-to-b",
           "gating": {"flags": ["flag/boarded"]}})
}

fn with_graph(who: &str, graph: Value, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = campaign(who, patch);
    std::fs::write(
        dir.join("layout-graph.json"),
        serde_json::to_string_pretty(&graph).unwrap(),
    )
    .unwrap();
    dir
}

#[test]
fn a_carry_edge_the_link_realises_is_green() {
    let run = build(&with_graph(
        "graph-green",
        ferry_graph(json!([carry_edge()])),
        |_| {},
    ));
    run.green();
    assert!(run.stderr.contains("1 carry)"), "{}", run.stderr);
}

#[test]
fn a_link_no_carry_edge_joins_is_dw0934_naming_both_places() {
    // The only edge left joins the shores by a walk the quests never take —
    // the link's direction has no carry.
    let run = build(&with_graph(
        "graph-no-edge",
        ferry_graph(json!([{"class": "vision", "id": "edge/across",
                            "a": "node/west-shore", "b": "node/east-shore"}])),
        |_| {},
    ));
    let line = run.refused("DW0934");
    assert!(
        line.contains("node/west-shore") && line.contains("node/east-shore"),
        "{line}"
    );
}

#[test]
fn a_carry_edge_no_link_realises_is_dw0934_naming_both_places() {
    // Both directions owed; only west → east is a link.
    let mut edge = carry_edge();
    edge.as_object_mut().unwrap().remove("one_way");
    let run = build(&with_graph(
        "graph-no-link",
        ferry_graph(json!([edge])),
        |_| {},
    ));
    let line = run.refused("DW0934");
    assert!(
        line.contains("from `node/east-shore` to `node/west-shore`"),
        "{line}"
    );
}

#[test]
fn the_schema_exports_carry_with_one_way_and_a_required_gating() {
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "all"])
        .output()
        .expect("delvec runs");
    assert!(o.status.success());
    let text = String::from_utf8_lossy(&o.stdout);
    let schema: Value = serde_json::from_str(&text).unwrap();
    // Find the `carry` variant wherever the export put the edge union.
    fn find(v: &Value) -> Option<&Value> {
        match v {
            Value::Object(m) => {
                let is_carry = m
                    .get("properties")
                    .and_then(|p| p.get("class"))
                    .and_then(|c| c.get("const").or_else(|| c.get("enum")))
                    .is_some_and(|c| c == "carry" || c == &json!(["carry"]));
                if is_carry {
                    return Some(v);
                }
                m.values().find_map(find)
            }
            Value::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    let carry = find(&schema).expect("the schema exports a `carry` edge");
    assert!(carry["properties"].get("one_way").is_some(), "{carry}");
    let required: Vec<&str> = carry["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r.as_str().unwrap())
        .collect();
    assert!(required.contains(&"gating"), "{carry}");
    assert!(!required.contains(&"one_way"), "{carry}");
}

// ---------------------------------------------------------------------------
// Criterion 5 — one enumeration.
// ---------------------------------------------------------------------------

/// Every `.rs` file under `crates/delvec/src/`, with its text.
fn sources() -> Vec<(PathBuf, String)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, String)>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                out.push((p, text));
            }
        }
    }
    let mut out = Vec::new();
    walk(&common::repo_root().join("crates/delvec/src"), &mut out);
    out
}

#[test]
fn the_seal_lifting_list_is_gone_and_one_function_marks_a_ride() {
    let all = sources();
    assert!(all.len() > 50, "the walk read {} file(s)", all.len());
    for (p, text) in &all {
        for gone in ["transit_teleports", "is_teleport_source"] {
            assert!(!text.contains(gone), "{} still names `{gone}`", p.display());
        }
    }
    // Every place a visited position is MARKED as arrived at by a ride — a
    // `transport_before:` field set to anything but `false` — is inside
    // `positions_of`, in the non-test half of `nav.rs`.
    let (_, nav) = all
        .iter()
        .find(|(p, _)| p.ends_with("compiler/nav.rs"))
        .expect("nav.rs");
    let body = nav.split("#[cfg(test)]\nmod tests").next().unwrap();
    let start = body.find("\nfn positions_of(").expect("positions_of");
    let end = start + body[start..].find("\n}\n").expect("its end");
    let mut marks = 0usize;
    for (i, _) in body.match_indices("transport_before:") {
        let rest = &body[i + "transport_before:".len()..];
        let value = rest.trim_start().split([',', '\n']).next().unwrap().trim();
        if value == "false" || value == "bool" {
            continue;
        }
        marks += 1;
        assert!(
            (start..end).contains(&i),
            "a ride is marked outside `positions_of`: `transport_before: {value}`"
        );
    }
    assert!(
        marks >= 2,
        "positions_of marks a crossing and a link ({marks} found)"
    );
    // The four readers all enumerate through it.
    for reader in [
        "pub fn check_critical_path_bound(",
        "pub fn check_branch_path(",
        "pub fn branch_path_routes(",
        "fn critical_positions(",
    ] {
        let at = body.find(reader).unwrap_or_else(|| panic!("{reader}"));
        let f = &body[at..at + body[at..].find("\n}\n").unwrap()];
        assert!(
            f.contains("positions_of(") || f.contains("critical_positions("),
            "{reader} does not read positions_of"
        );
    }
}

#[test]
fn the_waypoints_skip_the_carried_leg_and_export_its_two_walked_halves() {
    let run = build(&campaign("waypoints", |_| {}));
    run.green();
    let step = carrying_step(&run.json("critical-path.json")).unwrap();
    let (stand, to) = (step["stand"].clone(), step["transport"].clone());
    let wp = run.json("validation/critical-path-waypoints.json");
    let legs = wp["legs"].as_array().unwrap();
    assert!(
        !legs.iter().any(|l| l["from"] == stand && l["to"] == to),
        "the carried leg is exported as a walk: {legs:?}"
    );
    assert!(
        legs.iter().any(|l| l["to"] == stand),
        "no leg walks into the stand cell"
    );
    assert!(
        legs.iter().any(|l| l["from"] == to),
        "no leg walks on from the landing"
    );
    // Perturbed so the link is never taken — the far shore moved into the
    // west room, where a walk reaches it — the export walks every leg and the
    // carried pair is gone with the link.
    let dir = campaign("waypoints-walked", |q| {
        q["content"]["quests"][0]["objectives"][1]["anchor"] = json!("spawn");
    });
    let walked = build(&dir);
    walked.green();
    assert!(carrying_step(&walked.json("critical-path.json")).is_none());
    let wp = walked.json("validation/critical-path-waypoints.json");
    assert!(
        !wp["legs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l["from"] == to)
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — a way out.
// ---------------------------------------------------------------------------

/// The strait with its pit cut, and a rescue lever beside the pit whose
/// repeatable `teleport` lifts whoever stands in the pit onto the far landing,
/// gated on `gate`.
fn pit_campaign(who: &str, gate: &str) -> PathBuf {
    let gate = gate.to_string();
    let dir =
        campaign(who, move |q| {
            q["content"]["triggers"].as_array_mut().unwrap().push(json!({
            "id": "trigger/rescue", "at": "anchor/rescue", "on": {"on": "use"}, "once": false,
            "requires_flags": [gate],
            "effects": [{"type": "teleport",
                         "from": {"anchor": "anchor/pit", "extent": [0, 0, 0]},
                         "to": {"anchor": "anchor/far-landing"}}]
        }));
            q["content"]["quests"][0]["on_objective_complete"]["obj/far-shore"] = json!([{
                "type": "set-flag", "flag": "flag/landed",
                "happening": {"verb": "gains", "subject": "anchor/far-shore",
                              "text": "The far shore is reached."}
            }]);
        });
    common::patch_file(&dir.join("world.json"), |w| {
        w["content"]["areas"][0]["prefab"] = json!("prefab/ferry-strait-pit");
    });
    dir
}

#[test]
fn a_pocket_whose_only_exit_is_a_live_links_stand_cell_is_not_a_trap() {
    // Live while the far shore is next: `flag/boarded` is held from the
    // boarding on.
    let run = build(&pit_campaign("pit-live", "flag/boarded"));
    run.green();
    let line = run
        .stderr
        .lines()
        .find(|l| l.starts_with("DW0921 binding:"))
        .expect("the leave proof states its binding");
    assert!(
        line.ends_with("1 link stand cell(s) served as a way out"),
        "{line}"
    );
    // Shut in that configuration — `flag/landed` is set only once the far
    // shore is reached — the pit is a place a body gets into and not out of.
    let run = build(&pit_campaign("pit-shut", "flag/landed"));
    let line = run.refused("DW0921");
    assert!(line.contains("[17, "), "{line}");
}

// ---------------------------------------------------------------------------
// Criterion 7 — two populations.
// ---------------------------------------------------------------------------

/// A `reach` whose anchor stands on a raised walk, on the far side of the link,
/// with a radius that reaches the floor beside the walk: the party completes it
/// from the floor and never climbs round by the step. The far shore is
/// reached only by the link, so a population walked from the entry alone never
/// stood there — `DW0881` judges it because the stands-at population roots at
/// the link's `to` too.
#[test]
fn a_raised_reach_on_the_far_side_of_a_link_is_dw0881() {
    let dir = campaign("ledge", |q| {
        let far = &mut q["content"]["quests"][0]["objectives"][1];
        far["anchor"] = json!("anchor/ledge");
        far["radius"] = json!(2);
    });
    common::patch_file(&dir.join("world.json"), |w| {
        w["content"]["areas"][0]["prefab"] = json!("prefab/ferry-strait-ledge");
    });
    let run = build(&dir);
    let line = run.refused("DW0881");
    assert!(line.contains("obj/far-shore"), "{line}");
}

/// The ferry with a crossing after the far shore — a second area, the keep,
/// whose entry point the compiler carries the party to — and a trap on the far
/// landing whose payload is a gather onto the far deck.
fn populations_campaign() -> PathBuf {
    let dir = campaign("populations", |q| {
        let quests = q["content"]["quests"].as_array_mut().unwrap();
        quests[0]["on_complete"] = json!([]);
        quests.push(json!({
            "id": "quest/keep",
            "happening": {"verb": "arrives", "subject": "spawn", "text": "Beyond the strait, the keep."},
            "trigger": {"type": "quest-complete", "quest": "quest/cross"},
            "objectives": [{"id": "obj/keep", "type": "reach-anchor", "anchor": "anchor/exit",
                "radius": 1,
                "happening": {"verb": "arrives", "subject": "anchor/exit", "text": "They reach the keep's door."}}],
            "on_complete": [{"type": "campaign-complete",
                "happening": {"verb": "arrives", "subject": "anchor/exit", "text": "The journey ends."}}]
        }));
        q["content"]["traps"] = json!([{
            "id": "trap/drop", "at": "anchor/far-landing", "trigger": "pressure-plate",
            "lethality": "harmful", "reset": "rearm",
            "effect": {"dispense": {"count": 1, "item": "minecraft:arrow"}},
            "payload": [{"type": "teleport",
                         "from": {"anchor": "anchor/far-landing", "extent": [0, 0, 0]},
                         "to": {"anchor": "anchor/far-deck"}}]
        }]);
    });
    common::patch_file(&dir.join("world.json"), |w| {
        w["content"]["areas"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room"}));
    });
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        p["content"]["finale"] = json!("quest/keep");
        p["content"]["quests"].as_array_mut().unwrap().push(json!({
            "act": 1, "area": "area/keep", "depends_on": ["quest/cross"],
            "goal": "Walk to the keep's door.", "id": "quest/keep", "mandatory": true, "npcs": []
        }));
    });
    dir
}

#[test]
fn the_put_at_roots_hold_every_destination_and_the_stands_at_roots_no_gather() {
    use delvec::compiler::plan::Plan;
    use delvec::compiler::registry::PrefabRegistry;
    let dir = populations_campaign();
    let loaded = delvec::compiler::load::load_campaign_dir(&dir).unwrap();
    let campaign = delvewright_dsl::parse_campaign(&loaded.raw).expect("parses");
    let prefabs = PrefabRegistry::load_dir(&ferry_prefabs()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap_or_else(|e| panic!("{}", e.failure.message));
    assert_eq!(plan.links.len(), 1, "the tiller");
    assert_eq!(plan.gathers.len(), 1, "the trap payload");
    let crossing: Vec<[i32; 3]> = plan.transport.values().copied().collect();
    assert_eq!(crossing.len(), 1, "one crossing, into the keep");
    let entry = plan.campaign_start().map(|(_, p)| p);
    let put = delvec::compiler::lethal::put_at_roots(&plan, entry);
    let stands = delvec::compiler::lethal::stands_at_roots(&plan, entry);
    let (link_to, gather_to) = (plan.links[0].to, plan.gathers[0].to);
    assert!(
        put.contains(&link_to) && put.contains(&gather_to),
        "{put:?}"
    );
    assert!(put.contains(&crossing[0]), "{put:?}");
    assert!(
        stands.contains(&link_to) && stands.contains(&crossing[0]),
        "{stands:?}"
    );
    assert!(
        !stands.contains(&gather_to),
        "a trap's gather is no place the party certainly stands"
    );
}

// ---------------------------------------------------------------------------
// Criterion 6 — branches.
// ---------------------------------------------------------------------------

/// `branch-transport`, with the bolt branch's way past the keep's door made a
/// link instead of an `open-gate`: a lever at the entry, `once: false`, live
/// only while `flag/<gate>` is held, carrying whoever stands by the Keeper out
/// past the door onto the exit. The hold branch no longer crosses the door at
/// all — it walks out by the entry — so the door has no opener anywhere.
///
/// The hold branch's own `open-gate` is removed rather than kept because the
/// region model credits a bundle whose objective is absent from a branch's
/// path at that path's step 0, forced (`plan::firing_of`'s `unwrap_or(0)`),
/// which would open the door for the bolt branch too and leave the link
/// nothing to carry.
fn branch_link_campaign(who: &str, gate: &str) -> PathBuf {
    let src = common::compiler_fixtures_dir().join("branch-transport");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("teleport-link-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    let gate = gate.to_string();
    common::patch_file(&dir.join("quests.json"), move |q| {
        let decide = &mut q["content"]["quests"][0]["on_objective_complete"]["obj/decide"];
        decide
            .as_array_mut()
            .unwrap()
            .retain(|e| e["type"] != "open-gate");
        let hold = &mut q["content"]["quests"][3];
        assert_eq!(hold["id"], "quest/hold");
        hold["on_objective_complete"]["obj/watch"] = json!([]);
        hold["objectives"][1]["anchor"] = json!("spawn");
        q["content"]["triggers"] = json!([{
            "id": "trigger/vault", "at": "spawn", "on": {"on": "use"}, "once": false,
            "requires_flags": [gate],
            "effects": [{"type": "teleport",
                         "from": {"anchor": "anchor/keeper-stand", "extent": [1, 1, 1]},
                         "to": {"anchor": "anchor/exit"}}]
        }]);
    });
    dir
}

/// Every `branch-path-<slug>.json` a build wrote, by slug.
fn branch_paths(run: &Run) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(run.out.join("validation")).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if let Some(slug) = name
            .strip_prefix("branch-path-")
            .and_then(|s| s.strip_suffix(".json"))
        {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            out.push((slug.to_string(), v));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn a_link_live_on_one_branch_is_taken_on_that_branch_alone() {
    let run = build(&branch_link_campaign("branch-live", "flag/flee"));
    run.green();
    let paths = branch_paths(&run);
    assert_eq!(
        paths.len(),
        2,
        "two reachable branches: {:?}",
        paths.iter().map(|p| &p.0).collect::<Vec<_>>()
    );
    let carried: Vec<&str> = paths
        .iter()
        .filter(|(_, v)| carrying_step(v).is_some())
        .map(|(s, _)| s.as_str())
        .collect();
    assert_eq!(
        carried.len(),
        1,
        "exactly one branch takes the link: {carried:?}"
    );
    assert!(carried[0].contains("bolt"), "{carried:?}");
    let step = paths
        .iter()
        .find(|(s, _)| s == carried[0])
        .and_then(|(_, v)| carrying_step(v))
        .unwrap();
    assert_eq!(step["trigger"], "trigger/vault");
    assert!(step.get("stand").is_some());
    // The default path (the hold branch) walks.
    assert!(carrying_step(&run.json("critical-path.json")).is_none());
}

#[test]
fn a_branchs_link_made_shut_reds_that_branchs_proof() {
    // `flag/wait` is the hold branch's flag: on the bolt branch the lever is
    // shut, and the bolt branch's leg past the door has no way.
    let run = build(&branch_link_campaign("branch-shut", "flag/wait"));
    // The leg crosses the keep's door, so the refusal is the door's own
    // (`DW0317`, the leg family's gate counterfactual) — naming the link too.
    let line = run.refused("DW0317");
    assert!(line.contains("branch `branch/bolt`"), "{line}");
    assert!(line.contains("shut at this step"), "{line}");
}
