//! Branch-aware inter-area transport: a crossing that only happens
//! on ONE branch still has to move the party.
//!
//! `build_critical_path` derives the inter-area transport map for whatever
//! playthrough it is handed, so every branch's map already exists
//! (`Plan::branch_critical_path`). Emission, however, used to consult only the
//! **exported** path's map, so a crossing that exists solely on a branch emitted
//! no `teleport` — while `validation/branch-path-<slug>.json` promised one. The
//! island round-21 branch run stranded on exactly that split: `branch/flee`
//! walked to a deck it was never carried to.
//!
//! Fixture: `branch-transport` — `branch-two-endings` plus a second area
//! (`area/landing`) that only the bolt branch ever enters. The default (hold)
//! path never leaves the keep, so the crossing is branch-only by construction.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::{self, Plan};
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

/// The landing's `spawn` anchor in world coordinates — the destination the bolt
/// branch's crossing has to land on. Asserted against the exported branch path
/// below, so a fixture drift fails loudly instead of silently.
const LANDING_SPAWN: [i32; 3] = [262, 66, 8];

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("branch-transport")
}

fn try_build_campaign(dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("branch-transport")).unwrap();
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

fn build_campaign(dir: &Path) -> BuildOutput {
    try_build_campaign(dir).expect("emission succeeds")
}

/// Run `f` over a campaign's plan (the overlay's only input besides the plan's
/// own campaign).
fn with_plan<T>(dir: &Path, f: impl FnOnce(&Plan) -> T) -> T {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("branch-transport")).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    f(&plan)
}

/// A scratch campaign directory under the system temp dir, removed on drop.
/// (`tempfile` is not a dependency of this crate — same reasoning as `combat.rs`.)
struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-branch-transport-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        common::copy_dir_all(&fixture_dir(), &base);
        TempCampaign(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    /// Rewrite one stage document in place.
    fn patch(&self, stage: &str, edit: impl FnOnce(&mut Value)) {
        let file = self.0.join(format!("{stage}.json"));
        let mut doc: Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        edit(&mut doc);
        std::fs::write(&file, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }
}

impl Drop for TempCampaign {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A quest of a stage document's `content.quests`, by id.
fn quest<'a>(doc: &'a mut Value, id: &str) -> &'a mut Value {
    doc["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|q| q["id"] == id)
        .unwrap_or_else(|| panic!("no quest {id}"))
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("missing {path}"))).unwrap()
}

/// The `complete_<obj>` function body of one objective.
fn complete_fn<'a>(out: &'a BuildOutput, oid: &str) -> &'a str {
    text(
        out,
        &format!(
            "datapack/data/hello-world/function/complete_o_{}.mcfunction",
            plan::safe_local(oid)
        ),
    )
}

fn json_at(out: &BuildOutput, path: &str) -> Value {
    serde_json::from_str(text(out, path)).unwrap()
}

/// The bolt branch crosses into the landing when it completes `obj/bolt`; the
/// exported (hold) path never leaves the keep. The crossing must still be
/// emitted — gated on the flags that select the branch, so the hold path is
/// untouched by it.
#[test]
fn a_branch_only_crossing_emits_a_flag_gated_teleport() {
    let out = build_campaign(&fixture_dir());

    // The promise: the branch path the harness walks says obj/bolt teleports.
    let bolt = json_at(&out, "validation/branch-path-bolt.json");
    let promised = bolt["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/bolt")
        .and_then(|s| s.get("transport").cloned())
        .expect("obj/bolt step exists");
    assert_eq!(
        promised,
        serde_json::json!(LANDING_SPAWN),
        "fixture drift: the bolt branch must cross into area/landing at obj/bolt"
    );

    // The delivery: `complete_obj_bolt` carries the crossing, conditioned on the
    // bolt branch's own flag assignment (`flag/flee` set, `flag/wait` unset).
    let body = complete_fn(&out, "obj/bolt");
    let expected = format!(
        "execute if score {p} {flee} matches 1 unless score {p} {wait} matches 1 run teleport @s {} {} {}",
        LANDING_SPAWN[0],
        LANDING_SPAWN[1],
        LANDING_SPAWN[2],
        p = plan::PARTY,
        flee = plan::flag_score("flag/flee"),
        wait = plan::flag_score("flag/wait"),
    );
    assert!(
        body.lines().any(|l| l == expected),
        "expected branch-gated crossing\n  {expected}\nin complete_o_bolt:\n{body}"
    );

    // The hold path never crosses, so its own last beat carries no teleport.
    assert!(
        !complete_fn(&out, "obj/walk-out").contains("teleport @s"),
        "the hold branch stays in the keep and must gain no crossing"
    );
}

/// Generic contract: for EVERY branch path artifact, every step that promises a
/// transport has a matching `teleport` in that objective's completion bundle.
/// This is the r21 failure stated as an invariant rather than a single case —
/// `branch-path-<slug>.json` and the datapack are one contract.
#[test]
fn the_branch_path_json_and_the_emission_agree() {
    let out = build_campaign(&fixture_dir());
    let slugs: Vec<String> = out
        .keys()
        .filter_map(|k| {
            k.strip_prefix("validation/branch-path-")
                .and_then(|s| s.strip_suffix(".json"))
                .map(str::to_string)
        })
        .collect();
    assert!(!slugs.is_empty(), "the fixture must export branch paths");

    for slug in &slugs {
        let path = json_at(&out, &format!("validation/branch-path-{slug}.json"));
        for step in path["steps"].as_array().unwrap() {
            let (Some(oid), Some(dest)) = (
                step["objective"].as_str(),
                step.get("transport").and_then(|t| t.as_array()),
            ) else {
                continue;
            };
            let tail = format!(
                "teleport @s {} {} {}",
                dest[0].as_i64().unwrap(),
                dest[1].as_i64().unwrap(),
                dest[2].as_i64().unwrap()
            );
            let body = complete_fn(&out, oid);
            assert!(
                body.lines().any(|l| l.ends_with(&tail)),
                "branch `{slug}` promises `{tail}` at {oid}, but complete_o_{} does not deliver it:\n{body}",
                plan::safe_local(oid)
            );
        }
    }
}

/// `DW0494`: the exported path and a branch cross into DIFFERENT areas at the
/// SAME objective. Completing it can only put the party in one place, and the
/// exported path's crossing is unconditional, so there is nothing to gate on —
/// the build fails rather than emitting two teleports and letting command order
/// decide which one wins.
///
/// The variant: the hold branch gains a shore beat (`area/shore`) immediately
/// after `obj/decide`, while the bolt branch's own next beat moves into
/// `area/shore` — so `obj/decide` carries two contradictory crossings.
#[test]
fn a_shared_crossing_with_divergent_destinations_is_dw0494() {
    let tmp = TempCampaign::new("dw0494");
    tmp.patch("world", |w| {
        w["content"]["areas"].as_array_mut().unwrap().push(json!({
            "id": "area/shore",
            "name": "The Shore",
            "prefab": "prefab/cave-shore"
        }));
    });
    tmp.patch("quest-plan", |p| {
        // The bolt branch's next beat is already out at the landing.
        quest(p, "quest/bolt")["area"] = json!("area/landing");
        quest(p, "quest/hold")["depends_on"]
            .as_array_mut()
            .unwrap()
            .push(json!("quest/beacon"));
        p["content"]["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/beacon",
            "goal": "Light the shore beacon so the watch can be seen from the moor.",
            "area": "area/shore",
            "npcs": [],
            "depends_on": ["quest/decide"],
            "mandatory": true,
            "act": 2
        }));
    });
    tmp.patch("quests", |q| {
        quest(q, "quest/bolt")["objectives"][0]["anchor"] = json!("anchor/exit");
        q["content"]["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/beacon",
            "trigger": { "type": "quest-complete", "quest": "quest/decide" },
            "happening": {
                "verb": "arrives",
                "text": "The party climbs down to the shore where the old beacon still stands."
            },
            "objectives": [{
                "type": "reach-anchor",
                "id": "obj/beacon",
                "anchor": "anchor/exit",
                "radius": 2,
                "requires_flags": ["flag/wait"],
                "happening": {
                    "verb": "arrives",
                    "text": "They reach the beacon and the moor answers with nothing at all."
                }
            }],
            "on_objective_complete": {},
            "on_complete": [],
            "cast": {
                "npc/keeper": {
                    "at": "anchor/keeper-stand",
                    "doing": "holding the door while you go down to the water",
                    "dialogue": { "barks": ["Be quick."] }
                }
            }
        }));
    });

    match try_build_campaign(tmp.path()) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0494", "wrong diagnostic: {message}");
            assert!(
                message.contains("obj/decide"),
                "the diagnostic must name the objective that cannot cross twice: {message}"
            );
        }
        Err(other) => panic!("expected DW0494, got {other:?}"),
        Ok(_) => panic!("expected DW0494: obj/decide crosses into two different areas"),
    }
}

/// Byte-identity: a campaign whose branches cross nowhere the exported path does
/// not already cross gets an EMPTY overlay, so the new emission block is a
/// provable no-op and its datapack bytes are untouched by this task.
#[test]
fn a_campaign_without_branch_only_crossings_gets_an_empty_overlay() {
    for dir in [
        common::compiler_fixtures_dir().join("branch-two-endings"),
        common::hello_world_dir(),
        common::keep_crawl_dir(),
        common::keep_trial_dir(),
    ] {
        let overlay = with_plan(&dir, |plan| {
            emit::branch_transport_overlay(plan).expect("overlay computes")
        });
        assert!(
            overlay.is_empty(),
            "{} must gain no branch-only crossing, got {overlay:?}",
            dir.display()
        );
    }
    // And the no-branch reference emits no conditional teleport at all.
    let out = build_campaign(&common::compiler_fixtures_dir().join("branch-two-endings"));
    for (path, bytes) in &out {
        if !path.ends_with(".mcfunction") {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        assert!(
            !body.contains("run teleport @s"),
            "{path} gained a flag-gated crossing:\n{body}"
        );
    }
}

/// A branch proof credits only what its own path fires. The variant drops the
/// bolt branch's own `open-gate` (the flee-gated one on `obj/decide`), so the
/// door to `anchor/exit` opens only at `obj/watch` — a hold-branch objective the
/// bolt path never performs. The bolt branch must then fail its walk to
/// `anchor/exit` with `DW0317` (a gate nothing forced opens), because nothing on
/// its path opens the door; a bundle whose objective is absent from a path is
/// never forced on that path.
#[test]
fn a_branch_cannot_cross_a_door_only_another_branch_opens() {
    let tmp = TempCampaign::new("hold-only-door");
    tmp.patch("quests", |q| {
        let bundle = quest(q, "quest/decide")["on_objective_complete"]["obj/decide"]
            .as_array_mut()
            .unwrap();
        let before = bundle.len();
        bundle.retain(|e| e["type"] != "open-gate");
        assert_eq!(
            before - bundle.len(),
            1,
            "fixture drift: obj/decide must carry exactly one open-gate"
        );
    });

    match try_build_campaign(tmp.path()) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0317", "wrong diagnostic: {message}");
            assert!(
                message.starts_with("branch `branch/bolt`: "),
                "the diagnostic must name the branch that cannot walk: {message}"
            );
            assert!(
                message.contains("`anchor/door`")
                    && message.contains("no firing the party is forced to make ever opens it")
                    && message.contains("a beat of a branch this path does not take"),
                "the diagnostic must name the door and why nothing opens it: {message}"
            );
        }
        Err(other) => panic!("expected DW0317, got {other:?}"),
        Ok(_) => {
            panic!("expected DW0317: the bolt branch crosses a door only the hold branch opens")
        }
    }
}

/// A path proof credits only the lines its own flag state fires. The variant
/// drops the hold branch's own `open-gate` (on `obj/watch`), so the only opening
/// of the door left is the one on `obj/decide` guarded `when: requires_flags
/// [flag/flee]` — an objective the hold path DOES play, under a guard that never
/// holds on it. The hold path must then fail its walk out through the door with
/// `DW0317`: a guarded effect is forced only where the path satisfies its guard.
#[test]
fn a_path_cannot_cross_a_door_opened_only_under_another_branchs_flag() {
    let tmp = TempCampaign::new("flee-guarded-door");
    tmp.patch("quests", |q| {
        let bundle = quest(q, "quest/hold")["on_objective_complete"]["obj/watch"]
            .as_array_mut()
            .unwrap();
        let before = bundle.len();
        bundle.retain(|e| e["type"] != "open-gate");
        assert_eq!(
            before - bundle.len(),
            1,
            "fixture drift: obj/watch must carry exactly one open-gate"
        );
        let decide = &quest(q, "quest/decide")["on_objective_complete"]["obj/decide"];
        assert!(
            decide
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "open-gate"
                    && e["when"]["requires_flags"] == json!(["flag/flee"])),
            "fixture drift: obj/decide must carry the flee-guarded open-gate"
        );
    });

    match try_build_campaign(tmp.path()) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0317", "wrong diagnostic: {message}");
            assert!(
                message.contains("`anchor/door`")
                    && message.contains("no firing the party is forced to make ever opens it")
                    && message.contains("a line whose gate does not hold where this path plays it"),
                "the diagnostic must name the door and why nothing opens it: {message}"
            );
        }
        Err(other) => panic!("expected DW0317, got {other:?}"),
        Ok(_) => panic!(
            "expected DW0317: the hold path crosses a door opened only under the bolt branch's flag"
        ),
    }
}

/// A line whose gate holds only on a value no ordered walk can date is played by
/// the replay and never forced. The hold path's own `open-gate` (on `obj/watch`)
/// is gated `state/tally at-least 0` — true from the declared initial value.
/// Control: with nothing else writing the datum the walk dates it, the line is
/// forced, and the campaign builds. Variant: the campaign's `on_death` also
/// writes it, so its value at `obj/watch` is not a function of the path; the
/// opening is then not credited and the hold path's walk out is `DW0317`.
#[test]
fn a_door_opened_behind_an_undatable_gate_is_not_credited() {
    let tally = |tmp: &TempCampaign, on_death: bool| {
        tmp.patch("quests", |q| {
            q["content"]["state"] = json!([{
                "id": "state/tally",
                "initial": 0,
                "scope": "party",
                "note": "A count the watch keeps; only its datability is under test."
            }]);
            let bundle = quest(q, "quest/hold")["on_objective_complete"]["obj/watch"]
                .as_array_mut()
                .unwrap();
            let gate = bundle
                .iter_mut()
                .find(|e| e["type"] == "open-gate")
                .expect("fixture drift: obj/watch must carry the hold branch's open-gate");
            gate["when"] = json!({
                "requires_state": [{ "state": "state/tally", "op": "at-least", "value": 0 }]
            });
            if on_death {
                q["content"]["on_death"] =
                    json!([{ "type": "add-state", "state": "state/tally", "amount": 1 }]);
            }
        });
    };

    let control = TempCampaign::new("datable-gate");
    tally(&control, false);
    try_build_campaign(control.path()).unwrap_or_else(|e| {
        panic!("a gate the walk can date and that holds must carry the route: {e:?}")
    });

    let tmp = TempCampaign::new("undatable-gate");
    tally(&tmp, true);
    match try_build_campaign(tmp.path()) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0317", "wrong diagnostic: {message}");
            assert!(
                message.contains("`anchor/door`"),
                "the diagnostic must name the door: {message}"
            );
        }
        Err(other) => panic!("expected DW0317, got {other:?}"),
        Ok(_) => panic!("expected DW0317: the door opens behind a gate no ordered walk can decide"),
    }
}
