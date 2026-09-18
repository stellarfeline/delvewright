//! `DW0497` — the emitted call graph is closed.
//!
//! A `function <ns>:<name>` the compiler writes must point at a function the
//! compiler wrote. Vanilla resolves an unknown function to nothing, silently, so
//! the whole failure is invisible until a player notices the enemy that never
//! arrived (the island round-21 storm waves). Feature-blind by construction: the
//! rule is "a call has a callee", which needs no knowledge of waves, cutscenes
//! or traps, and therefore guards every future emitter split-brain too.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::integrity;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn parse_dir(dir: &std::path::Path) -> Campaign {
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("campaign parses")
}

fn build(campaign: &Campaign, dir: &std::path::Path) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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
        &common::campaign_inputs(dir),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// An emitted-tree slice: artifact path → body, exactly the shape `emit::build`
/// produces.
fn synthetic(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, body)| (path.to_string(), body.to_string()))
        .collect()
}

/// A synthetic emitted tree with one function calling a target nobody emitted —
/// the emitter split-brain reduced to its essence, and the shape the island's
/// `seq_under_ram` shipped. The diagnostic must name the caller, the line and
/// the missing target, because all three are needed to find which emitter
/// forgot to register what.
#[test]
fn dangling_internal_call_is_dw0497() {
    let fns = synthetic(&[(
        "datapack/data/isle/function/seq_under_ram_0.mcfunction",
        "say the ram grinds\nfunction isle:spawn_storm_shore\n",
    )]);
    let err = integrity::check_functions("isle", &fns)
        .expect_err("a dangling internal call must fail the build");
    assert_eq!(
        err.code, "DW0497",
        "wrong code; message was: {}",
        err.message
    );
    for needle in ["seq_under_ram_0", "spawn_storm_shore", "line 2"] {
        assert!(
            err.message.contains(needle),
            "the diagnostic must name the caller, the line and the missing \
             target (missing `{needle}`): {}",
            err.message
        );
    }
}

/// Scoped to the campaign's own namespace, and to functions rather than function
/// tags. Another pack's tree is not this compiler's to prove, and `#ns:tag`
/// names a tag whose membership is a separate artifact.
#[test]
fn foreign_namespaces_and_function_tags_are_not_dw0497() {
    let fns = synthetic(&[(
        "datapack/data/isle/function/tick.mcfunction",
        "function minecraft:other_pack_entry\nfunction #isle:some_group\n",
    )]);
    assert!(
        integrity::check_functions("isle", &fns).is_ok(),
        "only the campaign's own namespace is the compiler's to prove"
    );
}

/// Every call form the emitter actually produces is a call site. A checker that
/// only saw the bare form would miss `schedule function <ns>:lane_tick_…`, which
/// is exactly how a wave's march clock is armed.
#[test]
fn every_emitted_call_form_is_a_call_site() {
    for line in [
        "function isle:missing_target",
        "execute if score #x dw.sys matches 1 run function isle:missing_target",
        "schedule function isle:missing_target 30t",
        "execute as @a run schedule function isle:missing_target 30t replace",
        "return run function isle:missing_target",
    ] {
        let body = format!("{line}\n");
        let fns = synthetic(&[("datapack/data/isle/function/caller.mcfunction", &body)]);
        match integrity::check_functions("isle", &fns) {
            Err(e) => assert_eq!(e.code, "DW0497", "wrong code for `{line}`: {}", e.message),
            Ok(()) => panic!("`{line}` must be seen as a call site"),
        }
    }
}

/// The real proof that this is an invariant of emission and not a fixture
/// assert: every campaign fixture the compiler ships builds with a closed call
/// graph. `emit::build` runs the check itself, so a build that returns `Ok` has
/// already passed it — this re-asserts it explicitly so the failure names the
/// fixture.
#[test]
fn shipped_fixtures_emit_a_closed_call_graph() {
    for dir in [
        common::hello_world_dir(),
        common::keep_crawl_dir(),
        common::keep_trial_dir(),
        common::keep_vertical_dir(),
        common::cutscene_shots_dir(),
    ] {
        let campaign = parse_dir(&dir);
        let ns = campaign.world.campaign_id.as_str().to_string();
        let out = build(&campaign, &dir)
            .unwrap_or_else(|e| panic!("fixture `{}` must build: {e:?}", dir.display()));
        integrity::check_tree(&ns, &out).unwrap_or_else(|e| {
            panic!(
                "fixture `{}` emits a dangling call: {}",
                dir.display(),
                e.message
            )
        });
    }
}

/// spec-0073: which emitted functions put a body of a bar-carrying fight in the
/// world without re-capturing that bar's max afterwards. Discovered from the
/// bytes, not from a list of summoning functions: a function is a summoner of a
/// fight when one of its lines is a `summon` whose `Tags` carry the fight's own
/// tag, and it owes `function <ns>:hb_max_<key>` after its LAST such line.
/// Returns `(summoners examined, the ones that owe and do not pay)`.
fn uncaptured_summons(
    ns: &str,
    bars: &[delvec::compiler::healthbar::Bar<'_>],
    out: &BuildOutput,
) -> (usize, Vec<String>) {
    use delvewright_dsl::FightKind;
    let mut examined = 0;
    let mut missing = Vec::new();
    for bar in bars {
        let tag = match bar.fight.kind {
            FightKind::Wave => delvec::compiler::plan::wave_tag(bar.fight.id),
            FightKind::Actor => format!("dw_actor_{}", bar.safe),
        };
        let quoted = format!("\"{tag}\"");
        let capture = bar.capture_call(ns);
        for (path, bytes) in out {
            if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
                continue;
            }
            let body = std::str::from_utf8(bytes).unwrap();
            let lines: Vec<&str> = body.lines().collect();
            let Some(last) = lines
                .iter()
                .rposition(|l| l.contains("summon ") && l.contains(&quoted))
            else {
                continue;
            };
            examined += 1;
            if !lines[last + 1..].contains(&capture.as_str()) {
                missing.push(path.clone());
            }
        }
    }
    (examined, missing)
}

/// spec-0073 §3: **every function that summons a bar-carrying body calls its
/// capture**, so a bar's max is the max of the bodies actually standing —
/// through the first spawn, the unleash that swaps a puppet for its twin, and
/// every re-seat a rest runs. Built over the `souls-bonfire` fixture (the one
/// with a bonfire, so the re-seat summoners exist) with a bar on both waves and
/// on an unleashed actor. The walk binds to five summoners — `spawn_guards`,
/// `spawn_ambush`, `spawn_actor_barrow_warden`, `unleash_barrow_warden`,
/// `actor_restand_barrow_warden` — and a copy of the build with one capture
/// struck out is refused by name, so the check is shown able to see the defect.
#[test]
fn every_summon_of_a_bar_carrying_body_captures_the_bars_max() {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let mut c = parse_dir(&dir);
    for w in &mut c.quests.content.waves {
        w.health_bar = Some(
            serde_json::from_value(serde_json::json!({"range": 12, "title": "A Fight"})).unwrap(),
        );
    }
    c.quests.content.actors.push(
        serde_json::from_value(serde_json::json!({
            "id": "actor/barrow-warden",
            "entity": "minecraft:wither_skeleton",
            "name": "The Barrow Warden",
            "anchor": "anchor/wave",
            "health_bar": {"range": 16}
        }))
        .unwrap(),
    );
    let trigger = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/gate-ward")
        .expect("the fixture's strike trigger");
    for verb in ["spawn-actor", "unleash-actor"] {
        trigger.effects.push(
            serde_json::from_value(
                serde_json::json!({"type": verb, "actor": "actor/barrow-warden"}),
            )
            .unwrap(),
        );
    }
    let out = build(&c, &dir).expect("the bar-carrying fixture builds");
    let ns = "souls-bonfire";
    let bars = delvec::compiler::healthbar::bars(&c);
    assert_eq!(bars.len(), 3, "two waves and one actor carry a bar");
    let (examined, missing) = uncaptured_summons(ns, &bars, &out);
    assert_eq!(examined, 5, "summoners examined");
    assert!(
        missing.is_empty(),
        "summoners that never capture: {missing:?}"
    );

    // Perturbation toward the defect: strike the capture from one summoner.
    let mut broken = out.clone();
    let path = format!("datapack/data/{ns}/function/unleash_barrow_warden.mcfunction");
    let body = String::from_utf8(broken[&path].clone()).unwrap();
    let stripped: String = body
        .lines()
        .filter(|l| !l.contains(":hb_max_"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_ne!(body, stripped, "the unleash carried a capture to strike");
    broken.insert(path.clone(), stripped.into_bytes());
    let (examined, missing) = uncaptured_summons(ns, &bars, &broken);
    assert_eq!(examined, 5);
    assert_eq!(missing, vec![path]);
}
