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

/// An advancement's `rewards.function` is a call site too. Vanilla runs it when
/// the advancement is granted and resolves an unknown name to nothing, so a
/// kill advancement whose reward was never emitted is the same silent no-op as
/// a dangling `function` line.
#[test]
fn a_dangling_advancement_reward_is_dw0497() {
    let mut out: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    out.insert(
        "datapack/data/isle/advancement/k_edge.json".to_string(),
        br#"{"criteria":{},"rewards":{"function":"isle:k_reward_edge"}}"#.to_vec(),
    );
    let err = integrity::check_tree("isle", &out)
        .expect_err("a reward naming a function nobody emitted must fail the build");
    assert_eq!(err.code, "DW0497", "{}", err.message);
    for needle in ["advancement/k_edge.json", "k_reward_edge"] {
        assert!(
            err.message.contains(needle),
            "the diagnostic must name the advancement and its missing reward \
             (missing `{needle}`): {}",
            err.message
        );
    }
    out.insert(
        "datapack/data/isle/function/k_reward_edge.mcfunction".to_string(),
        b"say paid\n".to_vec(),
    );
    assert!(
        integrity::check_tree("isle", &out).is_ok(),
        "a reward whose function is emitted resolves"
    );
}

/// **Every reward names a function that exists** (spec-0074 criterion 2) — over
/// a build carrying every reward shape a fight has: the waves' `k_<wave>`, an
/// actor's `ka_<actor>` from an `on_kill`, and a wave NO beat seats, whose kill
/// advancement used to ship naming a `k_reward_<wave>` the pack never had
/// (the gallery README's finding). The binding is stated: the number of
/// advancements carrying a reward, which must include both kill families.
#[test]
fn every_reward_names_a_function_that_exists() {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let mut campaign = parse_dir(&dir);
    // A wave no beat spawns — declared, never seated.
    let mut idle = campaign.quests.content.waves[1].clone();
    idle.id = serde_json::from_str("\"wave/idle\"").unwrap();
    campaign.quests.content.waves.push(idle);
    // A wave bundle and an actor bundle, so both kill families are present.
    campaign.quests.content.waves[0].on_kill = Some(
        serde_json::from_str(
            r#"{ "fires": "every-kill",
                 "effects": [ { "type": "play-sound", "sound": "minecraft:entity.experience_orb.pickup" } ] }"#,
        )
        .unwrap(),
    );
    let anchor = campaign.quests.content.waves[0].anchor.as_str().to_string();
    let actor: delvewright_dsl::Actor = serde_json::from_str(&format!(
        r#"{{ "id": "actor/moth", "entity": "minecraft:bat", "anchor": "{anchor}",
              "vulnerable": true,
              "on_kill": {{ "effects": [ {{ "type": "play-sound",
                "sound": "minecraft:entity.experience_orb.pickup" }} ] }} }}"#
    ))
    .unwrap();
    campaign.quests.content.actors.push(actor);
    let ns = campaign.world.campaign_id.as_str().to_string();
    let out = build(&campaign, &dir)
        .unwrap_or_else(|e| panic!("the fight fixture must build with a closed call graph: {e:?}"));
    integrity::check_tree(&ns, &out).unwrap_or_else(|e| panic!("{}", e.message));
    let rewards: Vec<(&String, String)> = out
        .iter()
        .filter(|(p, _)| p.contains("/advancement/") && p.ends_with(".json"))
        .filter_map(|(p, b)| {
            let v: serde_json::Value = serde_json::from_slice(b).ok()?;
            Some((p, v["rewards"]["function"].as_str()?.to_string()))
        })
        .collect();
    let has = |needle: &str| rewards.iter().any(|(p, _)| p.contains(needle));
    assert!(
        has("/advancement/k_guards.json") && has("/advancement/ka_moth.json"),
        "the binding must include both kill families; rewards found: {rewards:#?}"
    );
    assert!(
        !has("/advancement/k_idle.json"),
        "a wave no beat seats has no kill machinery, so it ships no kill advancement"
    );
    for (path, reward) in &rewards {
        let name = reward.split_once(':').map(|(_, n)| n).unwrap_or(reward);
        assert!(
            out.contains_key(&format!("datapack/data/{ns}/function/{name}.mcfunction")),
            "`{path}` rewards `{reward}`, which is not emitted"
        );
    }
    eprintln!(
        "reward binding: {} advancement reward(s) resolved, over {} advancement(s)",
        rewards.len(),
        out.keys()
            .filter(|p| p.contains("/advancement/") && p.ends_with(".json"))
            .count()
    );
}
