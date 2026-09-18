//! A body the story removes is never seen to die.
//!
//! Vanilla `/kill` on a living entity is a death: the red flash, the fall-over
//! and the death particles, where the body stands. The compiler removes bodies
//! for the story — an NPC a `despawn-npc` sends away, an actor a `despawn-actor`
//! `vanish`es, the puppet an `unleash` replaces with its twin, the bodies a
//! bonfire re-seats — and none of those is a death the player is meant to
//! watch. The one removal that is: a `despawn-actor` the author wrote as
//! `style: kill`.
//!
//! Judged feature-blind over the shipped function tree of one build that
//! carries every removal kind: no line kills a body tag in place, and the only
//! kill of a removed body is the unseen sweep, under the world.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::{self, Plan};
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";
/// The unseen exit's vocabulary, spelled here rather than imported so the test
/// reads the shipped bytes as a stranger would: the sweep function, the one tag
/// a leaving body keeps, and the Y it waits at — under the overworld's floor.
const UNSEEN_SWEEP_FN: &str = "unseen_sweep";
const UNSEEN_TAG: &str = "dw_unseen";
const UNSEEN_Y: i32 = -128;
/// Staged, then unleashed from the fixture's strike trigger: its puppet is
/// replaced by a twin, and a bonfire re-stands the twin.
const ELITE: &str = "actor/barrow-warden";
/// Staged, never unleashed, and despawned when the quest completes.
const SCENERY: &str = "actor/kneeling-effigy";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// `souls-bonfire` — which already despawns its keeper and re-seats
/// `wave/guards` on rest — given an unleashed elite and a staged actor the
/// quest's completion despawns with `scenery_style`.
fn campaign(scenery_style: &str) -> Campaign {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    c.quests.dsl_version = DSL_VERSION.to_string();
    for (a, anchor) in [(ELITE, "anchor/wave"), (SCENERY, "anchor/npc-stand")] {
        c.quests.content.actors.push(
            serde_json::from_value(serde_json::json!({
                "id": a,
                "entity": "minecraft:wither_skeleton",
                "name": "The Barrow Warden",
                "anchor": anchor,
                "facing": "north"
            }))
            .expect("actor parses"),
        );
    }
    let trigger = c
        .quests
        .content
        .triggers
        .iter_mut()
        .find(|t| t.id.as_str() == "trigger/gate-ward")
        .expect("the fixture's strike trigger");
    for eff in [
        serde_json::json!({ "type": "spawn-actor", "actor": ELITE }),
        serde_json::json!({ "type": "spawn-actor", "actor": SCENERY }),
        serde_json::json!({ "type": "unleash-actor", "actor": ELITE }),
    ] {
        trigger
            .effects
            .push(serde_json::from_value(eff).expect("effect parses"));
    }
    let quest = c
        .quests
        .content
        .quests
        .iter_mut()
        .find(|q| q.on_complete.iter().any(|e| e.despawn_npc().is_some()))
        .expect("the fixture's quest that despawns the keeper");
    quest.on_complete.insert(
        0,
        serde_json::from_value(serde_json::json!({
            "type": "despawn-actor",
            "actor": SCENERY,
            "style": scenery_style
        }))
        .expect("despawn-actor parses"),
    );
    c
}

/// Validate + plan + emit; `emit::build` checks every emitted command against
/// the pinned 1.21.11 command tree.
fn build(campaign: &Campaign) -> (BuildOutput, Vec<String>) {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.is_empty(),
        "the fixture must validate clean: {diags:#?}"
    );
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
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    // Every tag a body the compiler places carries as its identity.
    let mut body_tags: Vec<String> = Vec::new();
    for n in &campaign.npcs.content.npcs {
        body_tags.push(format!("dw_npc_{}", plan::safe_local(n.id.as_str())));
    }
    for a in &campaign.quests.content.actors {
        let safe = plan::safe_local(a.id.as_str());
        body_tags.push(format!("dw_actor_{safe}"));
        body_tags.push(format!("dw_pup_{safe}"));
    }
    for w in &campaign.quests.content.waves {
        body_tags.push(plan::wave_tag(w.id.as_str()));
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
    .expect("every emitted command validates");
    (out, body_tags)
}

/// Every `(function, line)` of the shipped datapack that kills a body in place:
/// a `kill` whose selector names one of `body_tags` exactly.
fn in_place_kills(out: &BuildOutput, body_tags: &[String]) -> Vec<(String, String)> {
    let mut hits = Vec::new();
    for (path, bytes) in out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        for line in body.lines() {
            let Some(k) = line.find("kill @e[") else {
                continue;
            };
            let sel = &line[k..];
            if body_tags
                .iter()
                .any(|t| sel.contains(&format!("tag={t}]")) || sel.contains(&format!("tag={t},")))
            {
                hits.push((path.clone(), line.to_string()));
            }
        }
    }
    hits
}

fn func(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("{path} emitted"))
            .clone(),
    )
    .unwrap()
}

/// **No story removal kills a body where it stands.** The build carries a
/// `despawn-npc`, a `despawn-actor` `vanish`, an `unleash`, an actor re-stand
/// and a wave re-seat; none of them may emit an in-place `kill` of the body.
/// The binding is stated after the verdict: each of the five removed bodies is
/// removed somewhere in the tree, by an in-place kill or an unseen move.
#[test]
fn no_story_removal_kills_a_body_in_place() {
    let (out, tags) = build(&campaign("vanish"));
    let hits = in_place_kills(&out, &tags);
    assert!(
        hits.is_empty(),
        "{} in-place kill(s) of a body the story removes, over {} body tag(s): {hits:#?}",
        hits.len(),
        tags.len()
    );
    let all: String = out
        .iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect();
    let removed = [
        "dw_npc_keeper",
        "dw_actor_kneeling_effigy",
        "dw_pup_barrow_warden",
        "dw_actor_barrow_warden",
        "dw_wave_guards",
    ];
    for t in removed {
        assert!(
            all.contains(&format!("kill @e[tag={t}]"))
                || all.contains(&format!(
                    "execute as @e[tag={t}] at @s run tp @s ~ {UNSEEN_Y} ~"
                )),
            "binding: `{t}` is removed somewhere in the tree"
        );
    }
}

/// **Every unseen removal is moved under the world before anything kills it**,
/// and the only thing that kills it is the sweep: moved down its own column,
/// every tag replaced by the sweep's, the sweep scheduled — and the sweep kills
/// that tag and nothing else.
#[test]
fn an_unseen_removal_leaves_under_the_world_and_the_sweep_kills_it_there() {
    let (out, _) = build(&campaign("vanish"));
    for (name, tag) in [
        ("despawn_npc_keeper", "dw_npc_keeper"),
        ("unleash_barrow_warden", "dw_pup_barrow_warden"),
        ("actor_restand_barrow_warden", "dw_actor_barrow_warden"),
        ("wave_reseat_guards", "dw_wave_guards"),
    ] {
        let f = func(&out, name);
        let sched = f
            .find(&format!(
                "execute if entity @e[tag={tag}] run schedule function {NS}:{UNSEEN_SWEEP_FN} "
            ))
            .unwrap_or_else(|| panic!("{name} schedules the sweep:\n{f}"));
        let tp = f
            .find(&format!(
                "execute as @e[tag={tag}] at @s run tp @s ~ {UNSEEN_Y} ~"
            ))
            .unwrap_or_else(|| panic!("{name} moves the body down its own column:\n{f}"));
        let retag = f
            .find(&format!(
                "execute as @e[tag={tag}] run data merge entity @s {{Tags:[\"{UNSEEN_TAG}\"]"
            ))
            .unwrap_or_else(|| panic!("{name} replaces every tag:\n{f}"));
        assert!(
            sched < tp && tp < retag,
            "{name}: schedule while the body is still found, move, then retag:\n{f}"
        );
    }
    assert_eq!(
        func(&out, UNSEEN_SWEEP_FN).trim_end(),
        format!("kill @e[tag={UNSEEN_TAG}]")
    );
}

/// **The author's on-screen death survives.** `despawn-actor` `style: kill` is
/// the one removal the story writes as a death, and it stays vanilla `/kill`
/// where the body stands — exactly that line, and no other in-place kill.
#[test]
fn a_despawn_written_as_a_kill_still_dies_on_screen() {
    let (out, tags) = build(&campaign("kill"));
    let hits = in_place_kills(&out, &tags);
    assert_eq!(
        hits.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>(),
        vec!["kill @e[tag=dw_actor_kneeling_effigy]"],
        "{hits:#?}"
    );
}

/// `campaign("vanish")` with a fight whose PackTests need a removal's death in the
/// tick it happens: the unleashed elite, billed, carrying a declared drop (so
/// `souls_reseat_yields_nothing` meets it) and an `on_kill` (so
/// `kill_pays_removed_<f>` removes it every way the compiler can).
fn campaign_with_fights() -> Campaign {
    let mut c = campaign("vanish");
    let elite = c
        .quests
        .content
        .actors
        .iter_mut()
        .find(|a| a.id.as_str() == ELITE)
        .expect("the fixture's elite");
    elite.tier = Some(delvewright_dsl::EncounterTier::Elite);
    elite.drops.push(
        serde_json::from_value(serde_json::json!({
            "item": "minecraft:tripwire_hook",
            "name": "Warden Key"
        }))
        .expect("drop parses"),
    );
    elite.on_kill = Some(
        serde_json::from_value(serde_json::json!({
            "effects": [ { "type": "play-sound",
                           "sound": "minecraft:entity.experience_orb.pickup" } ]
        }))
        .expect("on_kill parses"),
    );
    c
}

/// **No generated PackTest runs the sweep.** The suite is one world: the sweep kills
/// every body waiting under it, so a template that ran it would kill the bodies a
/// sibling's removal parked in the same batch inside the delay the removal
/// promises them — the delay `v04_despawn_<npc>` watches. A template that needs a
/// removal's death in the tick it happens kills the waiting bodies in its own
/// dummy's column and nowhere else. A scheduled sweep (`schedule … replace`, the
/// removal's own line) is not a run: it waits the full delay.
///
/// Binding: every template file is read; each template this fixture emits that
/// needs a removal's death is shown reaching it by the column-scoped kill, and the
/// delay watcher is in the same suite.
#[test]
fn no_template_runs_the_sweep() {
    let (out, _) = build(&campaign_with_fights());
    let run = format!("function {NS}:{UNSEEN_SWEEP_FN}");
    let scoped = format!("positioned ~ {UNSEEN_Y} ~ run kill @e[tag={UNSEEN_TAG},distance=");
    let prefix = format!("packtest-datapack/data/{NS}/test/");
    let mut examined = 0usize;
    let mut runs: Vec<String> = Vec::new();
    let mut scoped_in: Vec<String> = Vec::new();
    for (path, bytes) in &out {
        let Some(name) = path
            .strip_prefix(&prefix)
            .and_then(|n| n.strip_suffix(".mcfunction"))
        else {
            continue;
        };
        examined += 1;
        let body = std::str::from_utf8(bytes).unwrap();
        for line in body.lines() {
            if line.contains(&run) && !line.contains("schedule function ") {
                runs.push(format!("{name}: {line}"));
            }
            if line.contains(&scoped) && !scoped_in.iter().any(|n| n == name) {
                scoped_in.push(name.to_string());
            }
        }
    }
    eprintln!(
        "template sweep binding: {examined} template(s) read; {} run the sweep; {} kill the \
         waiting bodies in their own dummy's column: {scoped_in:?}",
        runs.len(),
        scoped_in.len()
    );
    assert!(
        runs.is_empty(),
        "{} template line(s) of {examined} template(s) run the world-wide sweep, killing \
         every sibling's waiting body inside its promised delay:\n{}",
        runs.len(),
        runs.join("\n")
    );
    for t in [
        "souls_reseat_yields_nothing",
        "kill_pays_removed_a_barrow_warden",
    ] {
        assert!(
            scoped_in.iter().any(|n| n == t),
            "binding: `{t}` kills what its removals parked, in its own column \
             (scoped in {scoped_in:?})"
        );
    }
    assert!(
        out.contains_key(&format!("{prefix}v04_despawn_keeper.mcfunction")),
        "binding: the delay watcher `v04_despawn_keeper` is in the same suite"
    );
}
