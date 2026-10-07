//! The stake proof (`DW0525`) and the no-stranding proof (`DW0315`) on a route
//! that crosses areas.
//!
//! A crossing is one-way: the party is carried into the next area and nothing
//! carries it back. A route that enters an area and plays on needs a respawn
//! point there, set on arrival, and from then on no player can stand in the
//! area it left. `DW0525` judged every respawn seat against the cells the entry
//! spawn's walk reaches, whatever area the party was in while the seat held, so
//! the arrival seat read every cell of the first area as stranded and a route
//! could not enter a second area with a stake declared — and the cells of the
//! second area were judged against no seat at all.
//!
//! Fixture: `branch-transport`'s two areas, re-authored as one line: talk to the
//! Keeper in the keep, be carried to the landing, the landing's arrival sets the
//! checkpoint, walk to the exit. A stake is declared.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

fn campaign(tag: &str, stake: bool) -> PathBuf {
    let src = common::compiler_fixtures_dir().join("branch-transport");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("stake-across-areas-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&src, &dir);
    common::patch_file(&dir.join("quest-plan.json"), |qp| {
        qp["content"] = json!({
            "quests": [
                { "id": "quest/decide", "area": "area/keep", "act": 1, "depends_on": [],
                  "goal": "Hear the Keeper out.", "mandatory": true, "npcs": ["npc/keeper"] },
                { "id": "quest/away", "area": "area/landing", "act": 2,
                  "depends_on": ["quest/decide"], "goal": "Walk off the landing.",
                  "mandatory": true, "npcs": [] }
            ],
            "finale": "quest/away"
        });
    });
    common::patch_file(&dir.join("dialogue.json"), |d| {
        d["content"]["dialogues"][0]["nodes"] = json!([{
            "id": "dlg/greeting",
            "text": "The moor road is drowned. Go by the landing.",
            "options": [{
                "label": "We go.",
                "effects": [{ "type": "complete-objective", "objective": "obj/decide" }]
            }]
        }]);
    });
    common::patch_file(&dir.join("quests.json"), |q| {
        let mut content = json!({
            "quests": [
                { "id": "quest/decide", "trigger": { "type": "campaign-start" },
                  "objectives": [{ "type": "talk-to", "id": "obj/decide", "npc": "npc/keeper" }],
                  "on_complete": [] },
                { "id": "quest/away",
                  "trigger": { "type": "quest-complete", "quest": "quest/decide" },
                  "objectives": [
                      { "type": "reach-anchor", "id": "obj/arrive", "anchor": "anchor/landing-stone", "radius": 2 },
                      { "type": "reach-anchor", "id": "obj/away", "anchor": "anchor/exit",
                        "radius": 1, "after": ["obj/arrive"] }
                  ],
                  "on_objective_complete": {
                      "obj/arrive": [{ "type": "set-checkpoint", "anchor": "anchor/landing-stone" }]
                  },
                  "on_complete": [{ "type": "campaign-complete" }] }
            ]
        });
        if stake {
            content["state"] = json!([{ "id": "state/tokens", "scope": "player", "initial": 0 }]);
            content["stakes"] = json!([{
                "id": "stake/tokens", "state": "state/tokens", "collect_by": "anyone",
                "collected_message": "You take back what the landing took.",
                "forfeit": { "kind": "proportion", "percent": 50 },
                "marker_item": "minecraft:lantern", "max_live": 2, "on_full": "keep"
            }]);
        }
        q["content"] = content;
    });
    dir
}

/// The library with one anchor the landing alone carries, `anchor/landing-stone`
/// at the landing's spawn cell — `spawn` and `anchor/exit` are names the keep's
/// piece carries too.
fn library() -> PathBuf {
    let dir = common::shown_prefabs_dir("stake-across-areas");
    common::patch_file(&dir.join("cave-shore.json"), |m| {
        m["anchors"]["anchor/landing-stone"] = json!({ "pos": [6, 2, 8] });
    });
    dir
}

fn try_build(dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the campaign parses");
    let prefabs = PrefabRegistry::load_dir(&library()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn ledger(out: &BuildOutput) -> Value {
    serde_json::from_slice(
        out.get("validation/stake-gate.json")
            .expect("a campaign with a stake ships its stake ledger"),
    )
    .unwrap()
}

/// The control: the same route with no stake builds, so what the stake proof
/// says below is the stake proof's.
#[test]
fn the_crossing_route_builds_without_a_stake() {
    try_build(&campaign("no-stake", false)).expect("the two-area route is green");
}

/// A seat set on arrival in the second area is judged over what a player can
/// stand on while it holds — the landing — and the entry spawn over the keep
/// it holds in; nothing is stranded, and both seats bind rows.
#[test]
fn a_seat_set_after_a_crossing_is_judged_over_the_area_it_holds_in() {
    let out = match try_build(&campaign("stake", true)) {
        Ok(out) => out,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("{}: {message}", code.id())
        }
        Err(e) => panic!("{e:?}"),
    };
    let gate = ledger(&out);
    assert_eq!(gate["respawn_seats"], 2, "{gate}");
    assert_eq!(gate["stranded_cells"], 0, "{gate}");
    assert!(gate["rows_proved"].as_u64().unwrap() >= 2, "{gate}");
}

/// The layout graph says how the places connect, and the route's crossing is a
/// way the party is carried: a `carry` edge from the keep to the landing, which
/// the graph's critical path steps over and whose places are booked in two
/// areas, is realised by the compiler's crossing (`DW0934` asks for no link).
/// A carry back from the landing — a direction the route never crosses — is
/// realised by nothing and stays `DW0934`.
#[test]
fn a_carry_between_two_areas_is_realised_by_the_crossing_the_route_takes() {
    let graph = |edges: Value| {
        json!({
            "campaign_id": "branch-transport", "dsl_version": delvewright_dsl::DSL_VERSION,
            "stage": "layout-graph",
            "content": {
                "nodes": [
                    { "id": "node/keep", "intent": "hub", "size_class": "room" },
                    { "id": "node/landing", "intent": "landing", "size_class": "room" }
                ],
                "edges": edges,
                "entry": "node/keep", "goal": "node/landing",
                "critical_path": ["node/keep", "node/landing"],
                "beats": [
                    { "quest": "quest/decide", "objective": "obj/decide", "node": "node/keep" },
                    { "quest": "quest/away", "objective": "obj/arrive", "node": "node/landing" },
                    { "quest": "quest/away", "objective": "obj/away", "node": "node/landing" }
                ]
            }
        })
    };
    let codes = |tag: &str, edges: Value| -> Vec<String> {
        let dir = campaign(tag, false);
        std::fs::write(
            dir.join("layout-graph.json"),
            serde_json::to_string_pretty(&graph(edges)).unwrap(),
        )
        .unwrap();
        let loaded = load_campaign_dir(&dir).unwrap();
        let campaign = parse_campaign(&loaded.raw).expect("the campaign parses");
        delvec::compiler::link::check_carry_realised(&campaign)
            .into_iter()
            .map(|d| format!("{} {}", d.code, d.message))
            .collect()
    };
    let across = json!({ "class": "carry", "id": "edge/across", "a": "node/keep",
                         "b": "node/landing", "one_way": "a-to-b",
                         "gating": { "quest": "quest/decide" } });
    assert!(codes("graph-across", json!([across])).is_empty());
    let back = json!({ "class": "carry", "id": "edge/back", "a": "node/landing",
                       "b": "node/keep", "one_way": "a-to-b",
                       "gating": { "quest": "quest/decide" } });
    let refused = codes("graph-back", json!([across, back]));
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].starts_with("DW0934")
            && refused[0].contains("from `node/landing` to `node/keep`"),
        "{refused:?}"
    );
}

/// A bonfire armed by the beat the crossing leaves from: the exported path's
/// `rest` step (spec-0016) is spliced where the party can walk to the fire,
/// never between the beat and the crossing that carries the party out of the
/// fire's area at that same moment. This route never returns to the keep, so
/// no rest is exported at all; before, the rest followed `obj/decide` and the
/// bot was sent from the landing to a fire in another area.
#[test]
fn a_rest_is_never_spliced_where_a_crossing_has_carried_the_party_off() {
    let dir = campaign("bonfire", false);
    common::patch_file(&dir.join("quests.json"), |q| {
        q["content"]["quests"][0]["on_complete"] =
            json!([{ "type": "bonfire", "anchor": "spawn" }]);
    });
    let out = match try_build(&dir) {
        Ok(out) => out,
        Err(BuildFailure::Diagnostic { code, message }) => panic!("{}: {message}", code.id()),
        Err(e) => panic!("{e:?}"),
    };
    let path: Value =
        serde_json::from_slice(out.get("critical-path.json").expect("the path is exported"))
            .unwrap();
    let steps = path["steps"].as_array().unwrap();
    let x = |s: &Value| s["pos"][0].as_i64();
    for (i, s) in steps.iter().enumerate() {
        if s["action"] != "rest" {
            continue;
        }
        let fire = x(s).unwrap();
        // Areas stand 256 blocks apart along x: the step a rest follows and the
        // step after it stand in the fire's area.
        for n in [i.checked_sub(1), Some(i + 1)].into_iter().flatten() {
            if let Some(nx) = steps.get(n).and_then(x) {
                assert_eq!(
                    nx.div_euclid(256),
                    fire.div_euclid(256),
                    "step {n} beside the rest: {path}"
                );
            }
        }
    }
    assert!(
        !steps.iter().any(|s| s["action"] == "rest"),
        "the route never returns to the keep, so no rest is exported: {path}"
    );
}
