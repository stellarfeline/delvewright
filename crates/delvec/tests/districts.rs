//! **A delve authored in parallel by district** (spec-0103) — the engine half.
//!
//! Every test here starts from `fixtures/districts`: the blockout fixture's
//! seven places partitioned into two districts (`district/front` — the landing,
//! the hall, the loft; `district/back` — past the barred door), one spine quest
//! of the whole, one optional quest per district written in its fragment, a
//! fragment NPC with its dialogue tree, and a station a fragment declares. It
//! validates and builds green from stand-ins, so every refusal below is
//! produced by perturbing a green campaign by one edit.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn fixture() -> PathBuf {
    common::repo_root().join("crates/delvec/tests/fixtures/districts")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("districts-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fresh copy of the fixture, and an empty prefab directory beside it.
fn campaign(tag: &str) -> (PathBuf, PathBuf) {
    let root = scratch(tag);
    let dir = root.join("campaign");
    common::copy_dir_all(&fixture(), &dir);
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    (dir, prefabs)
}

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn json_diags(o: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn validate(dir: &Path, prefabs: &Path, extra: &[&str]) -> (i32, Vec<Value>, String) {
    let mut args = vec![
        "--json",
        "--prefabs",
        prefabs.to_str().unwrap(),
        "validate",
        dir.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    let o = run(&args);
    (o.status.code().unwrap_or(-1), json_diags(&o), text(&o))
}

fn errors_with(diags: &[Value], code: &str) -> Vec<Value> {
    diags
        .iter()
        .filter(|d| d["code"] == code && d["severity"] == "error")
        .cloned()
        .collect()
}

fn write_json(path: &Path, v: &Value) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The fragment envelope the fixture's documents carry.
fn envelope(dir: &Path, stage: &str, content: Value) -> Value {
    let world = read_json(&dir.join("world.json"));
    json!({
        "dsl_version": world["dsl_version"],
        "campaign_id": world["campaign_id"],
        "stage": stage,
        "content": content,
    })
}

// ---------------------------------------------------------------------------
// The fixture is green, and says what it holds
// ---------------------------------------------------------------------------

#[test]
fn the_fixture_validates_and_states_its_partition_and_fragments() {
    let (dir, prefabs) = campaign("green");
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    let errors: Vec<&Value> = diags.iter().filter(|d| d["severity"] == "error").collect();
    assert_eq!(code, 0, "{t}");
    assert!(errors.is_empty(), "{errors:?}");
    assert!(
        t.contains("district binding: 2 district(s) over 7 place(s)."),
        "{t}"
    );
    assert!(
        t.contains(
            "district fragments: 5 fragment file(s) of 2 declared district(s), 5 object(s) located"
        ),
        "{t}"
    );
    // The whole's quest names the station a fragment declares, and the sleeper
    // a fragment declares: the whole may name anything.
    let quests = read_json(&dir.join("quests.json"));
    assert_eq!(
        quests["content"]["quests"][0]["cast"]["npc/sleeper"]["at"],
        "anchor/loft-window"
    );
}

#[test]
fn a_graph_with_no_districts_says_so_and_raises_nothing() {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let prefabs = scratch("zero").join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 0, "{t}");
    assert!(t.contains("0 district(s): the whole"), "{t}");
    assert!(
        !diags
            .iter()
            .any(|d| ["DW1003", "DW1004", "DW1005"].contains(&d["code"].as_str().unwrap_or(""))),
        "{diags:?}"
    );
    let o = run(&["district", "list", dir.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(
        text(&o).contains("0 district(s): the whole"),
        "{}",
        text(&o)
    );
}

// ---------------------------------------------------------------------------
// Criterion 1: the surface
// ---------------------------------------------------------------------------

fn schema(stage: &str) -> Value {
    let o = run(&["schema", "--stage", stage]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    serde_json::from_slice(&o.stdout).unwrap()
}

fn definition<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc.get("definitions")
        .or_else(|| doc.get("$defs"))
        .and_then(|d| d.get(name))
        .unwrap_or_else(|| panic!("no definition `{name}`"))
}

#[test]
fn the_exported_surface_is_exactly_the_specs() {
    let g = schema("layout-graph");
    let d = definition(&g, "District");
    let mut props: Vec<&str> = d["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    props.sort_unstable();
    assert_eq!(props, ["id", "nodes", "note"]);
    let mut required: Vec<&str> = d["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    required.sort_unstable();
    assert_eq!(required, ["id", "nodes"]);
    let content = definition(&g, "LayoutGraphContent");
    assert!(content["properties"].get("districts").is_some());
    assert!(
        !content["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "districts")
    );

    let p = schema("quest-plan");
    let q = definition(&p, "PlannedQuest");
    assert!(q["properties"].get("district").is_some());
    assert!(
        !q["required"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "district")
    );
}

// ---------------------------------------------------------------------------
// Criterion 2: the partition
// ---------------------------------------------------------------------------

fn partition_refusal(tag: &str, edit: impl FnOnce(&mut Value), needle: &str) {
    let (dir, prefabs) = campaign(tag);
    common::patch_file(&dir.join("layout-graph.json"), edit);
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    let hits = errors_with(&diags, "DW1003");
    assert!(
        hits.iter()
            .any(|d| d["message"].as_str().unwrap().contains(needle)),
        "no DW1003 saying `{needle}`: {hits:?}\n{t}"
    );
    assert!(
        hits.iter().all(|d| d["message"]
            .as_str()
            .unwrap()
            .contains("The partition is a graph edit")),
        "every shape names its remedy: {hits:?}"
    );
}

#[test]
fn a_place_in_no_district_is_refused_listing_every_one() {
    partition_refusal(
        "p-none",
        |g| {
            g["content"]["districts"][1]["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|n| n != "node/tunnel" && n != "node/undercroft");
        },
        "2 of the graph's 7 place(s) stand in no district: `node/undercroft`, `node/tunnel`",
    );
}

#[test]
fn a_place_in_two_districts_is_refused() {
    partition_refusal(
        "p-two",
        |g| {
            g["content"]["districts"][0]["nodes"]
                .as_array_mut()
                .unwrap()
                .push(json!("node/cell"));
        },
        "place `node/cell` is in two districts, `district/front` and `district/back`",
    );
}

#[test]
fn a_district_naming_an_undeclared_place_is_refused() {
    partition_refusal(
        "p-undeclared",
        |g| {
            g["content"]["districts"][0]["nodes"]
                .as_array_mut()
                .unwrap()
                .push(json!("node/nowhere"));
        },
        "names `node/nowhere`, which the graph does not declare",
    );
}

#[test]
fn an_empty_district_is_refused() {
    partition_refusal(
        "p-empty",
        |g| {
            g["content"]["districts"]
                .as_array_mut()
                .unwrap()
                .push(json!({"id": "district/empty", "nodes": []}));
        },
        "district `district/empty` holds no place",
    );
}

#[test]
fn a_disconnected_district_is_refused() {
    partition_refusal(
        "p-apart",
        |g| {
            g["content"]["districts"][1]["nodes"]
                .as_array_mut()
                .unwrap()
                .retain(|n| n != "node/exit");
            g["content"]["districts"][0]["nodes"]
                .as_array_mut()
                .unwrap()
                .push(json!("node/exit"));
        },
        "district `district/front` is not one connected set",
    );
}

#[test]
fn a_fragment_directory_naming_no_declared_district_is_refused() {
    let (dir, prefabs) = campaign("p-dir");
    common::copy_dir_all(&dir.join("districts/back"), &dir.join("districts/nowhere"));
    std::fs::remove_dir_all(dir.join("districts/back")).unwrap();
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    let hits = errors_with(&diags, "DW1003");
    assert!(
        hits.iter().any(|d| d["message"]
            .as_str()
            .unwrap()
            .contains("the directory `districts/nowhere/` names `district/nowhere`")),
        "{hits:?}"
    );
    assert_eq!(hits[0]["stage"], "districts/nowhere");
}

#[test]
fn a_quest_naming_an_undeclared_district_is_dw0112() {
    let (dir, prefabs) = campaign("plan-district");
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        p["content"]["quests"][1]["district"] = json!("district/nobody");
    });
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    assert!(
        errors_with(&diags, "DW0112")
            .iter()
            .any(|d| d["path"] == "/content/quests/1/district"),
        "{t}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 3: fragments — the union, the inputs, and what a fragment may say
// ---------------------------------------------------------------------------

#[test]
fn fragments_union_after_the_whole_in_sorted_district_order() {
    let loaded = delvec::compiler::load::load_campaign_dir(&fixture()).unwrap();
    for f in [
        "districts/back/quests.json",
        "districts/front/quests.json",
        "districts/front/npcs.json",
        "districts/front/dialogue.json",
        "districts/front/layout-graph.json",
    ] {
        assert!(
            loaded.inputs.contains_key(f),
            "`{f}` is not a manifest input"
        );
    }
    let c = delvec::compiler::load::parse_loaded(&loaded).unwrap();
    let ids: Vec<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "quest/walk-the-whole",
            "quest/see-the-tunnel",
            "quest/look-in-the-loft"
        ],
        "the whole's own first, then `district/back`, then `district/front`"
    );
    let loft = c
        .layout_graph
        .as_ref()
        .unwrap()
        .content
        .nodes
        .iter()
        .find(|n| n.id.0 == "node/loft")
        .unwrap();
    assert_eq!(loft.stations.len(), 1, "the fragment's station is unioned");
    assert_eq!(c.npcs.content.npcs.len(), 2);
}

fn build(dir: &Path, prefabs: &Path, out: &Path) -> Output {
    let o = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    o
}

#[test]
fn a_fragment_byte_moves_the_manifest() {
    let (dir, prefabs) = campaign("manifest");
    let root = dir.parent().unwrap().to_path_buf();
    build(&dir, &prefabs, &root.join("one"));
    let path = dir.join("districts/back/quests.json");
    let t = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        t.replace("stands in the tunnel", "stands in the tunnel, alone"),
    )
    .unwrap();
    build(&dir, &prefabs, &root.join("two"));
    let m1 = read_json(&root.join("one/manifest.json"));
    let m2 = read_json(&root.join("two/manifest.json"));
    let key = "districts/back/quests.json";
    assert!(m1["inputs"][key].is_string(), "{}", m1["inputs"]);
    assert_ne!(m1["inputs"][key], m2["inputs"][key]);
}

fn whole_owns(tag: &str, setup: impl FnOnce(&Path), needle: &str) {
    let (dir, prefabs) = campaign(tag);
    setup(&dir);
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    let hits = errors_with(&diags, "DW1005");
    assert!(
        hits.iter()
            .any(|d| d["message"].as_str().unwrap().contains(needle)),
        "no DW1005 saying `{needle}`: {hits:?}\n{t}"
    );
}

#[test]
fn state_in_a_fragment_is_the_wholes() {
    whole_owns(
        "own-state",
        |dir| {
            common::patch_file(&dir.join("districts/front/quests.json"), |q| {
                q["content"]["state"] = json!([]);
            });
        },
        "carries `state`, which the whole owns",
    );
}

#[test]
fn guidance_in_a_fragment_is_the_wholes() {
    whole_owns(
        "own-guidance",
        |dir| {
            common::patch_file(&dir.join("districts/front/quests.json"), |q| {
                q["content"]["guidance"] = json!({"markers": "hidden"});
            });
        },
        "carries `guidance`, which the whole owns",
    );
}

#[test]
fn a_place_field_in_a_layout_graph_fragment_is_the_wholes() {
    whole_owns(
        "own-intent",
        |dir| {
            common::patch_file(&dir.join("districts/front/layout-graph.json"), |g| {
                g["content"]["nodes"][0]["intent"] = json!("vista");
            });
        },
        "carries `intent`, which the whole owns",
    );
}

#[test]
fn a_palette_in_a_fragment_is_the_wholes() {
    whole_owns(
        "own-palette",
        |dir| {
            let doc = envelope(
                dir,
                "detail-plan",
                json!({"palette": {"wall": "minecraft:stone"}, "details": []}),
            );
            write_json(&dir.join("districts/front/detail-plan.json"), &doc);
        },
        "carries `palette`, which the whole owns",
    );
}

#[test]
fn a_scene_row_in_a_design_fragment_is_the_wholes() {
    whole_owns(
        "own-design",
        |dir| {
            let doc = envelope(
                dir,
                "design",
                json!({"references": [{"name": "concept/hall", "shows": "the hall", "time": "noon", "weather": "clear"}]}),
            );
            write_json(&dir.join("districts/front/design.json"), &doc);
        },
        "those rows are the design gate's",
    );
}

#[test]
fn a_stage_a_district_does_not_fragment_is_the_wholes() {
    whole_owns(
        "own-classes",
        |dir| {
            std::fs::copy(
                dir.join("classes.json"),
                dir.join("districts/front/classes.json"),
            )
            .unwrap();
        },
        "is a `classes` document, and a district fragments only",
    );
}

fn outside(tag: &str, setup: impl FnOnce(&Path), needle: &str) {
    let (dir, prefabs) = campaign(tag);
    setup(&dir);
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    let hits = errors_with(&diags, "DW1004");
    assert!(
        hits.iter()
            .any(|d| d["message"].as_str().unwrap().contains(needle)),
        "no DW1004 saying `{needle}`: {hits:?}\n{t}"
    );
    assert!(
        hits.iter()
            .all(|d| d["message"].as_str().unwrap().starts_with("[district/")),
        "a fragment's refusal is routed to its district: {hits:?}"
    );
}

#[test]
fn an_npc_anchored_in_the_sibling_is_refused() {
    outside(
        "out-npc",
        |dir| {
            common::patch_file(&dir.join("districts/front/npcs.json"), |n| {
                n["content"]["npcs"][0]["anchor"] = json!("anchor/node-tunnel");
            });
        },
        "names `anchor/node-tunnel`, which stands in `district/back`, not in `district/front`",
    );
}

#[test]
fn a_quest_objective_in_the_sibling_is_refused() {
    outside(
        "out-quest",
        |dir| {
            common::patch_file(&dir.join("districts/front/quests.json"), |q| {
                q["content"]["quests"][0]["objectives"][0]["anchor"] = json!("anchor/node-cell");
            });
        },
        "names `anchor/node-cell`, which stands in `district/back`",
    );
}

#[test]
fn a_quest_the_plan_gives_another_district_is_refused() {
    outside(
        "out-plan",
        |dir| {
            common::patch_file(&dir.join("quest-plan.json"), |p| {
                p["content"]["quests"][1]["district"] = json!("district/back");
            });
        },
        "expands `quest/look-in-the-loft`, which the quest plan assigns to `district/back`",
    );
}

#[test]
fn a_trigger_in_the_sibling_is_refused() {
    outside(
        "out-trigger",
        |dir| {
            common::patch_file(&dir.join("districts/front/quests.json"), |q| {
                q["content"]["triggers"] = json!([{
                    "at": "anchor/node-tunnel",
                    "effects": [{"style": "chat", "text": "A draught.", "type": "narrate"}],
                    "id": "trigger/draught",
                    "on": {"on": "approach", "range": 2},
                }]);
            });
        },
        "names `anchor/node-tunnel`, which stands in `district/back`",
    );
}

#[test]
fn a_dialogue_tree_another_districts_quest_acts_on_is_refused() {
    outside(
        "out-tree",
        |dir| {
            // The keeper's tree moved into `district/front`: the whole's
            // quest talks to the keeper, so the tree is the whole's.
            let mut whole = read_json(&dir.join("dialogue.json"));
            let tree = whole["content"]["dialogues"]
                .as_array_mut()
                .unwrap()
                .remove(0);
            write_json(&dir.join("dialogue.json"), &whole);
            common::patch_file(&dir.join("districts/front/dialogue.json"), |d| {
                d["content"]["dialogues"].as_array_mut().unwrap().push(tree);
            });
        },
        "carries the dialogue tree of `npc/keeper`, and quests outside `district/front` name that NPC",
    );
}

#[test]
fn a_station_on_a_place_of_the_sibling_is_refused() {
    outside(
        "out-station",
        |dir| {
            common::patch_file(&dir.join("districts/front/layout-graph.json"), |g| {
                g["content"]["nodes"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"id": "node/cell", "stations": [{"anchor": "anchor/cell-bench", "kind": "point"}]}));
            });
        },
        "declares stations on `node/cell`, a place of `district/back`",
    );
}

#[test]
fn a_detail_row_of_a_place_of_the_sibling_is_refused() {
    outside(
        "out-detail",
        |dir| {
            let doc = envelope(
                dir,
                "detail-plan",
                json!({"details": [{"place": "node/cell", "piece": "prefab/blockout-cell", "anchors": {}}]}),
            );
            write_json(&dir.join("districts/front/detail-plan.json"), &doc);
        },
        "carries the detail row of `node/cell`, a place of `district/back`",
    );
}

#[test]
fn a_design_row_of_a_place_of_the_sibling_is_refused() {
    outside(
        "out-design",
        |dir| {
            let doc = envelope(
                dir,
                "design",
                json!({"references": [{"name": "places/cell/from-the-door", "shows": "the cell", "time": "noon", "weather": "clear"}]}),
            );
            write_json(&dir.join("districts/front/design.json"), &doc);
        },
        "records `places/cell/from-the-door`, an image of `node/cell`, which is a place of `district/back`",
    );
}

#[test]
fn a_quest_waiting_on_the_siblings_quest_is_refused() {
    outside(
        "sib-quest",
        |dir| {
            common::patch_file(&dir.join("districts/front/quests.json"), |q| {
                q["content"]["quests"][0]["trigger"] =
                    json!({"type": "quest-complete", "quest": "quest/see-the-tunnel"});
            });
        },
        "names `quest/see-the-tunnel`, which `district/back` declares — a reference into a sibling",
    );
}

#[test]
fn a_dialogue_next_into_the_siblings_tree_is_refused() {
    outside(
        "sib-next",
        |dir| {
            // `district/back` gains a tree of its own, and the sleeper's
            // option leads into it.
            let n = envelope(dir, "npcs", {
                let mut s =
                    read_json(&dir.join("districts/front/npcs.json"))["content"]["npcs"][0].clone();
                s["id"] = json!("npc/digger");
                s["name"] = json!("The Digger");
                s["anchor"] = json!("anchor/node-tunnel");
                json!({"npcs": [s]})
            });
            write_json(&dir.join("districts/back/npcs.json"), &n);
            let d = envelope(
                dir,
                "dialogue",
                json!({"dialogues": [{"npc": "npc/digger", "root": "dlg/digger-hello", "nodes": [{"id": "dlg/digger-hello", "text": "Still digging.", "options": []}]}]}),
            );
            write_json(&dir.join("districts/back/dialogue.json"), &d);
            common::patch_file(&dir.join("districts/front/dialogue.json"), |d| {
                d["content"]["dialogues"][0]["nodes"][0]["options"] =
                    json!([{"label": "Who digs?", "next": "dlg/digger-hello"}]);
            });
        },
        "names `dlg/digger-hello`, which `district/back` declares",
    );
}

#[test]
fn a_flag_two_districts_write_is_refused() {
    outside(
        "sib-flag",
        |dir| {
            for x in ["front", "back"] {
                common::patch_file(&dir.join(format!("districts/{x}/quests.json")), |q| {
                    q["content"]["quests"][0]["on_complete"] =
                        json!([{"type": "set-flag", "flag": "flag/both-seen"}]);
                });
            }
        },
        "writes or reads `flag/both-seen`, which `district/",
    );
}

// ---------------------------------------------------------------------------
// Criterion 4: the same objects in fragments or inlined build the same world
// ---------------------------------------------------------------------------

/// The fixture with every fragment inlined into the whole's documents, by the
/// rule the loader states: the whole's own first, then each district in sorted
/// id order — written here independently of the loader.
fn inline(dir: &Path) {
    let root = dir.join("districts");
    let mut districts: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
        .collect();
    districts.sort();
    for x in &districts {
        let mut files: Vec<String> = std::fs::read_dir(root.join(x))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_str().unwrap().to_string())
            .collect();
        files.sort();
        for f in files {
            let frag = read_json(&root.join(x).join(&f));
            let mut whole = read_json(&dir.join(&f));
            if f == "layout-graph.json" {
                for n in frag["content"]["nodes"].as_array().unwrap() {
                    let node = whole["content"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|w| w["id"] == n["id"])
                        .unwrap();
                    let stations = node
                        .as_object_mut()
                        .unwrap()
                        .entry("stations")
                        .or_insert(json!([]));
                    stations
                        .as_array_mut()
                        .unwrap()
                        .extend(n["stations"].as_array().unwrap().iter().cloned());
                }
            } else {
                for (k, v) in frag["content"].as_object().unwrap() {
                    let list = whole["content"]
                        .as_object_mut()
                        .unwrap()
                        .entry(k.clone())
                        .or_insert(json!([]));
                    list.as_array_mut()
                        .unwrap()
                        .extend(v.as_array().unwrap().iter().cloned());
                }
            }
            write_json(&dir.join(&f), &whole);
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}

/// Every file under `dir`, relative, with its bytes.
fn tree(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    fn walk(base: &Path, at: &Path, out: &mut std::collections::BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    walk(dir, dir, &mut out);
    out
}

#[test]
fn fragments_and_their_inlining_build_the_same_datapack_and_world() {
    let (dir, prefabs) = campaign("determinism");
    let root = dir.parent().unwrap().to_path_buf();
    let inlined = root.join("inlined");
    common::copy_dir_all(&dir, &inlined);
    inline(&inlined);
    build(&dir, &prefabs, &root.join("from-fragments"));
    build(&inlined, &prefabs, &root.join("from-whole"));
    let a = tree(&root.join("from-fragments"));
    let b = tree(&root.join("from-whole"));
    let differ: Vec<&String> = a
        .keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .collect();
    // The manifest records the inputs (the fragment files are inputs of one
    // build and not of the other), and the district ledger records who wrote
    // each object: those two, and nothing else, may differ.
    assert_eq!(
        differ,
        ["manifest.json", "validation/districts.json"],
        "the same objects built different bytes"
    );
    let datapack = a.keys().filter(|k| k.starts_with("datapack/")).count();
    let world = a.keys().filter(|k| k.starts_with("server/")).count();
    assert!(
        datapack > 0 && world > 0,
        "{datapack} datapack and {world} world file(s) compared"
    );
}

// ---------------------------------------------------------------------------
// Criterion 5: `--district`
// ---------------------------------------------------------------------------

#[test]
fn a_workers_tree_reports_the_siblings_unexpanded_quests_under_one_line() {
    let (dir, prefabs) = campaign("scope");
    std::fs::remove_file(dir.join("districts/back/quests.json")).unwrap();
    let (code, diags, t) = validate(&dir, &prefabs, &["--district", "district/front"]);
    assert_eq!(code, 0, "{t}");
    let line = diags
        .iter()
        .find(|d| d["code"] == "DW1006")
        .unwrap_or_else(|| panic!("no DW1006: {t}"));
    assert!(
        line["message"]
            .as_str()
            .unwrap()
            .starts_with("1 planned quest(s) of 1 other district(s) are not expanded in this tree"),
        "{line}"
    );
    for code in [
        "DW0150", "DW0818", "DW0172", "DW0112", "DW0482", "DW0197", "DW0816", "DW0817",
    ] {
        assert!(
            errors_with(&diags, code).is_empty(),
            "{code} still refuses: {t}"
        );
    }
    let (code, diags, t) = validate(&dir, &prefabs, &[]);
    assert_eq!(code, 1, "{t}");
    assert!(!errors_with(&diags, "DW0150").is_empty(), "{t}");
}

#[test]
fn a_district_flag_naming_no_district_is_refused_with_the_declared_set() {
    let (dir, prefabs) = campaign("nobody");
    let (code, diags, t) = validate(&dir, &prefabs, &["--district", "district/nobody"]);
    assert_eq!(code, 1, "{t}");
    let d = errors_with(&diags, "DW1009");
    assert_eq!(d.len(), 1, "{t}");
    assert!(
        d[0]["message"]
            .as_str()
            .unwrap()
            .contains("it declares `district/front`, `district/back`"),
        "{d:?}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 6: `district list`
// ---------------------------------------------------------------------------

#[test]
fn district_list_prints_a_row_per_district_and_its_binding() {
    let o = run(&["--json", "district", "list", fixture().to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let t = text(&o);
    let rows: Vec<Value> = {
        let s = String::from_utf8_lossy(&o.stdout);
        let end = s.find("\n]").map_or(s.len(), |i| i + 2);
        serde_json::from_str(&s[..end]).unwrap()
    };
    assert_eq!(rows.len(), 2);
    let front = &rows[0];
    assert_eq!(front["district"], "district/front");
    assert_eq!(front["reached"], 3);
    assert_eq!(front["stations"], 1);
    assert_eq!(front["boundary_seams"], 1);
    assert_eq!(front["planned_quests"], 1);
    assert_eq!(front["expanded_quests"], 1);
    assert_eq!(front["path_steps"], 2);
    assert_eq!(rows[1]["district"], "district/back");
    assert_eq!(rows[1]["reached"], 4);
    assert!(
        t.contains("district binding: 2 district(s) over 7 place(s)"),
        "{t}"
    );
    assert!(!t.contains("\"code\":\"DW1007\""), "{t}");
}

#[test]
fn a_one_place_district_with_nothing_in_it_is_advised_and_passes() {
    let (dir, _) = campaign("small");
    common::patch_file(&dir.join("layout-graph.json"), |g| {
        let d = g["content"]["districts"].as_array_mut().unwrap();
        d[1]["nodes"]
            .as_array_mut()
            .unwrap()
            .retain(|n| n != "node/tunnel" && n != "node/undercroft");
        d.push(json!({"id": "district/cellars", "nodes": ["node/undercroft"], "note": "one place on purpose"}));
        d.push(json!({"id": "district/tunnel", "nodes": ["node/tunnel"]}));
    });
    // The tunnel quest's beat stands in the tunnel, so only the undercroft
    // is a district nobody acts in.
    common::patch_file(&dir.join("quest-plan.json"), |p| {
        p["content"]["quests"][2]["district"] = json!("district/tunnel");
    });
    std::fs::rename(dir.join("districts/back"), dir.join("districts/tunnel")).unwrap();
    let o = run(&["district", "list", dir.to_str().unwrap()]);
    let t = text(&o);
    assert_eq!(o.status.code(), Some(0), "{t}");
    assert!(t.contains("DW1007"), "{t}");
    assert!(
        t.contains("district `district/cellars` is below the lower bound"),
        "{t}"
    );
    assert!(!t.contains("district `district/tunnel` is below"), "{t}");
}

// ---------------------------------------------------------------------------
// Criterion 8: writes
// ---------------------------------------------------------------------------

/// The fixture inside a repository of its own, at `<repo>/campaigns/c`.
fn in_repo(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = scratch(tag);
    let repo = root.join("repo");
    let dir = repo.join("campaigns/c");
    common::copy_dir_all(&fixture(), &dir);
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let prefabs = repo.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    (repo, dir, prefabs)
}

fn writes(dir: &Path, prefabs: &Path, district: &str, paths: &str) -> Output {
    let root = dir.parent().unwrap().parent().unwrap().to_path_buf();
    let list = root.join(format!("paths-{}.txt", district.replace('/', "-")));
    std::fs::write(&list, paths).unwrap();
    run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "district",
        "writes",
        dir.to_str().unwrap(),
        district,
        "--paths",
        list.to_str().unwrap(),
    ])
}

#[test]
fn a_write_outside_the_set_is_refused_naming_its_owner() {
    let (_, dir, prefabs) = in_repo("writes");
    let inside = "campaigns/c/districts/front/quests.json\n\
                  campaigns/c/programs/loft.json\n\
                  campaigns/c/design/places/hall/from-the-door.png\n\
                  prefabs/blockout-loft.nbt\n\
                  prefabs/blockout-loft.x0y0z1.nbt\n\
                  prefabs/blockout-loft.json\n\
                  prefabs/blockout-loft.report.json\n";
    let o = writes(&dir, &prefabs, "district/front", inside);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(
        text(&o).contains("7 path(s) read, 7 inside"),
        "{}",
        text(&o)
    );

    let o = writes(
        &dir,
        &prefabs,
        "district/front",
        "campaigns/c/districts/back/quests.json\ncampaigns/c/quests.json\nprefabs/blockout-cell.nbt\n",
    );
    let t = text(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    assert!(t.contains("DW1008"), "{t}");
    assert!(
        t.contains("`campaigns/c/districts/back/quests.json` is outside `district/front`'s write set; it belongs to `district/back`"),
        "{t}"
    );
    assert!(
        t.contains("`campaigns/c/quests.json` is outside `district/front`'s write set; it belongs to the whole"),
        "{t}"
    );
    assert!(
        t.contains("`prefabs/blockout-cell.nbt` is outside `district/front`'s write set; it belongs to `district/back`"),
        "{t}"
    );
}

#[test]
fn the_rule_is_one_subcommand_called_from_the_hook_and_the_integration() {
    let repo = common::repo_root();
    let mut callers: Vec<String> = Vec::new();
    for dir in ["tools", ".claude"] {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["grep", "--untracked", "-l", "district writes", "--", dir])
            .output()
            .unwrap();
        callers.extend(
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.ends_with(".sh") || l.ends_with(".py"))
                .map(str::to_string),
        );
    }
    callers.sort();
    assert_eq!(
        callers,
        [
            "tools/creator/district-hook.sh",
            "tools/creator/district-integrate.sh"
        ],
        "the write-set rule is called from exactly the hook and the integration"
    );
}

// ---------------------------------------------------------------------------
// Criterion 10: routing
// ---------------------------------------------------------------------------

#[test]
fn the_build_writes_the_district_ledger() {
    let (dir, prefabs) = campaign("ledger");
    let out = dir.parent().unwrap().join("out");
    build(&dir, &prefabs, &out);
    let l = read_json(&out.join("validation/districts.json"));
    for class in [
        "places",
        "stations",
        "anchors",
        "quests",
        "objectives",
        "npcs",
    ] {
        assert!(
            l[class].as_object().is_some_and(|o| !o.is_empty()),
            "no `{class}` rows: {l}"
        );
    }
    assert_eq!(l["places"]["node/loft"], "district/front");
    assert_eq!(l["stations"]["anchor/loft-window"], "district/front");
    assert_eq!(l["anchors"]["anchor/seam-hall-cell"], "whole");
    assert_eq!(l["quests"]["quest/walk-the-whole"], "whole");
    assert_eq!(l["quests"]["quest/see-the-tunnel"], "district/back");
    assert_eq!(l["objectives"]["obj/see-the-loft"], "district/front");
    assert_eq!(l["npcs"]["npc/sleeper"], "district/front");
    let legs = l["legs"].as_array().unwrap();
    assert_eq!(legs.len(), 3);
    assert_eq!(legs[1]["from_district"], "district/front");
    assert_eq!(legs[1]["to_district"], "district/back");
}

#[test]
fn a_districts_refusal_carries_its_district_and_the_wholes_does_not() {
    let (dir, prefabs) = campaign("route");
    // A district quest refused by an ordinary rule — an objective anchor the
    // campaign does not have — relocated into its fragment; and the whole's
    // quest refused by the same rule.
    common::patch_file(&dir.join("districts/back/quests.json"), |q| {
        q["content"]["quests"][0]["objectives"][0]["anchor"] = json!("anchor/nowhere");
    });
    common::patch_file(&dir.join("quests.json"), |q| {
        q["content"]["quests"][0]["objectives"][1]["anchor"] = json!("anchor/nowhere-else");
    });
    let o = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "validate",
        dir.to_str().unwrap(),
    ]);
    let t = text(&o);
    assert_eq!(o.status.code(), Some(1), "{t}");
    let lines: Vec<&str> = t.lines().filter(|l| l.contains(" [error] ")).collect();
    let district: Vec<&&str> = lines
        .iter()
        .filter(|l| l.contains("districts/back/quests /content/quests/0/"))
        .collect();
    assert!(!district.is_empty(), "{t}");
    assert!(
        district.iter().all(|l| l.contains(": [district/back] ")),
        "{district:?}"
    );
    let whole: Vec<&&str> = lines
        .iter()
        .filter(|l| l.contains(" quests /content/quests/0/"))
        .collect();
    assert!(!whole.is_empty(), "{t}");
    assert!(whole.iter().all(|l| !l.contains("[district/")), "{whole:?}");
}
