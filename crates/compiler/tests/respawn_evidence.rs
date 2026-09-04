//! **The three evidence routes `DW0478` accepts** (spec-0044), end to end.
//!
//! The criterion condemned six placements on the project's one released campaign
//! and was right about none of them, so what a red *claims* was too much: whether
//! a retry loop is winnable is a combat question this compiler refuses to
//! simulate. The claim now shrinks to what declarations carry — *nothing the
//! campaign declares separates this respawn from a soft-lock* — and three routes
//! supply that separation: an unconditional post-reset fold, an onset bound, and
//! a forced beat that already delivers the same encounter no-more-gently.
//!
//! **Every case here is a pair, red and green.** A credit that cannot be made to
//! disappear proves nothing, so each route is exercised twice: once with the
//! evidence present, once with it absent and the pair still red. The geometry
//! demanded of a compared, uncredited pair is unchanged, to the block.
//!
//! Fixtures are `souls-bonfire` with its one respawn point respelled and moved,
//! plus — for the one route that must NOT credit — `souls-td-lanes`, whose squad
//! marches a corridor the forced path crosses.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

use serde_json::json;

struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-evidence-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        TempCampaign(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempCampaign {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn build_from(fixture: &str, dir: &Path) -> Result<BuildOutput, BuildFailure> {
    build_with_skins(dir, &common::compiler_fixtures_dir().join(fixture))
}

fn build_with_skins(dir: &Path, skins_from: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags
            .iter()
            .all(|d| d.severity != delvewright_dsl::Severity::Error),
        "the mutation must stay schema-valid so the BUILD tier is what is under test: {diags:#?}"
    );
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
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                skins_from
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
}

fn ledger(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/respawn-safety.json")
            .expect("every build that assembles a world states this proof's binding count"),
    )
    .unwrap()
}

fn diagnostic(err: BuildFailure) -> (String, String) {
    match err {
        BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a diagnostic, got {other:?}"),
    }
}

/// Materialize a fixture and rewrite its `quests.json`.
fn campaign_from(fixture: &str, dst: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    common::materialize_from(
        &common::compiler_fixtures_dir().join(fixture),
        &json!({}),
        dst,
    );
    let p = dst.join("quests.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    mutate(&mut quests);
    std::fs::write(&p, serde_json::to_string_pretty(&quests).unwrap()).unwrap();
}

/// Lift `souls-bonfire`'s one `bonfire` out of its bundle and re-seat it as a
/// plain `set-checkpoint` at `anchor`, armed at `arm_at`.
///
/// **Where it is armed is the reign, and the reign is what a dominance credit is
/// measured inside.** `obj/slay` is the fixture's own arming beat and leaves the
/// whole remaining walk of the keep inside the reign; `obj/shrine` is the last
/// beat there is, so nothing forced follows it.
fn reseat(
    quests: &mut serde_json::Value,
    anchor: &str,
    arm_at: &str,
    on_respawn: serde_json::Value,
) {
    let mut found = false;
    for q in quests["content"]["quests"].as_array_mut().unwrap() {
        let Some(map) = q["on_objective_complete"].as_object_mut() else {
            continue;
        };
        for (_, effs) in map.iter_mut() {
            let list = effs.as_array_mut().unwrap();
            if let Some(i) = list
                .iter()
                .position(|e| e["type"] == "bonfire" || e["type"] == "set-checkpoint")
            {
                list.remove(i);
                found = true;
            }
        }
    }
    assert!(
        found,
        "the fixture carries exactly one respawn-point effect"
    );
    // `respawns_on_rest` hangs off a bonfire that is now gone (`DW0370`).
    for w in quests["content"]["waves"].as_array_mut().unwrap() {
        w.as_object_mut().unwrap().remove("respawns_on_rest");
    }
    let seat = json!({
        "type": "set-checkpoint",
        "anchor": anchor,
        "on_respawn": on_respawn,
    });
    let quest = quests["content"]["quests"].as_array_mut().unwrap();
    let last = quest.last_mut().unwrap();
    let bundle = last["on_objective_complete"]
        .as_object_mut()
        .unwrap()
        .entry(arm_at.to_string())
        .or_insert_with(|| json!([]));
    bundle.as_array_mut().unwrap().push(seat);
}

/// Move a wave's anchor. The reset route needs a seat whose ONLY overlapping
/// force is one a reset can remove — and no verb removes a wave, so a wave left
/// in range would keep the build red whatever the fold does, and the test would
/// be measuring the wrong thing.
fn move_wave(quests: &mut serde_json::Value, id: &str, anchor: &str) {
    for w in quests["content"]["waves"].as_array_mut().unwrap() {
        if w["id"] == id {
            w["anchor"] = json!(anchor);
            return;
        }
    }
    panic!("no wave `{id}` in this fixture");
}

/// Stage a hostile actor at `anchor`, given real AI at the campaign's first beat.
///
/// `minecraft:husk` deliberately: it is the one common undead vanilla's own
/// `#minecraft:burn_in_daylight` tag excludes, so the fixture cannot pick up
/// `DW0496` and change what is under test.
fn hostile_actor(quests: &mut serde_json::Value, id: &str, anchor: &str) {
    let actors = quests["content"]
        .as_object_mut()
        .unwrap()
        .entry("actors".to_string())
        .or_insert_with(|| json!([]));
    actors.as_array_mut().unwrap().push(json!({
        "id": id,
        "entity": "minecraft:husk",
        "anchor": anchor,
    }));
    let quest = &mut quests["content"]["quests"].as_array_mut().unwrap()[0];
    let bundle = quest["on_objective_complete"]
        .as_object_mut()
        .unwrap()
        .entry("obj/talk".to_string())
        .or_insert_with(|| json!([]));
    let list = bundle.as_array_mut().unwrap();
    list.push(json!({"type": "spawn-actor", "actor": id}));
    list.push(json!({"type": "unleash-actor", "actor": id}));
}

// ---------------------------------------------------------------------------
// Route 1 — the reset (spec-0044 §3)
// ---------------------------------------------------------------------------

/// **The reset credit, red then green.** A seat standing on a live hostile is a
/// red; the same seat whose own `on_respawn` unconditionally takes that body off
/// the board is not, because the world the reset leaves does not hold it.
///
/// The demand the credit meets is a fact the defect contradicts: the defect is
/// *"this force perceives the arrival"*, and the credit is *"this force is not
/// there at the arrival"* — two contradictory readings of the same emitted bytes.
#[test]
fn a_reset_that_removes_the_force_is_credited_and_its_absence_is_red() {
    let red = TempCampaign::new("reset-red");
    campaign_from("souls-bonfire", red.path(), |q| {
        reseat(q, "anchor/door", "obj/shrine", json!([]));
        move_wave(q, "wave/guards", "anchor/chest");
        hostile_actor(q, "actor/warden", "anchor/exit");
    });
    let (code, message) = diagnostic(build_from("souls-bonfire", red.path()).expect_err(
        "a seat on a live hostile, at a beat no forced walk follows, is a soft-lock claim",
    ));
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("actor/warden") && message.contains("anchor/door"),
        "{message}"
    );
    assert!(
        message.contains("SEPARATES THIS RETRY FROM A SOFT-LOCK"),
        "the red must claim only what declarations carry: {message}"
    );

    let green = TempCampaign::new("reset-green");
    campaign_from("souls-bonfire", green.path(), |q| {
        reseat(
            q,
            "anchor/door",
            "obj/shrine",
            json!([{"type": "despawn-actor", "actor": "actor/warden", "style": "vanish"}]),
        );
        move_wave(q, "wave/guards", "anchor/chest");
        hostile_actor(q, "actor/warden", "anchor/exit");
    });
    let out = build_from("souls-bonfire", green.path())
        .expect("the reset removes the body the seat stands on");
    let l = ledger(&out);
    let credit = &l["rest_points"][0]["credited"][0];
    assert_eq!(
        credit["id"], "actor/warden",
        "the credit is recorded against the pair it answers: {l}"
    );
    assert_eq!(
        credit["kind"], "reset",
        "the kind is computed from the object, and this one is a fold: {l}"
    );
    assert!(
        credit["reason"].as_str().unwrap().contains("despawn-actor"),
        "the ledger names the effect the credit is read off: {l}"
    );
    assert!(
        l["rest_points"][0]["compared_against"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "actor/warden"),
        "a credited pair is COMPARED, not skipped — the ledger must not lose it: {l}"
    );
}

/// **And a conditional reset is never credited.** Wrap the same despawn in a
/// flag-gated bundle and the pair is red again: the post-reset world must hold in
/// every state a death can occur in, so "true in the author's head, false in some
/// flag state" has no route in.
#[test]
fn a_flag_gated_reset_is_not_a_reset() {
    let tmp = TempCampaign::new("reset-gated");
    campaign_from("souls-bonfire", tmp.path(), |q| {
        reseat(
            q,
            "anchor/door",
            "obj/shrine",
            json!([{
                "type": "move-npc",
                "npc": "npc/keeper",
                "to_anchor": "anchor/door",
                "requires_flags": ["flag/cleared"],
                "on_arrive": [
                    {"type": "despawn-actor", "actor": "actor/warden", "style": "vanish"}
                ]
            }]),
        );
        move_wave(q, "wave/guards", "anchor/chest");
        hostile_actor(q, "actor/warden", "anchor/exit");
        // Something must set the flag, or it is not a gate at all.
        let quest = &mut q["content"]["quests"].as_array_mut().unwrap()[0];
        quest["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "set-flag", "flag": "flag/cleared"}));
    });
    let (code, message) = diagnostic(
        build_from("souls-bonfire", tmp.path())
            .expect_err("a removal that only sometimes happens is not a removal"),
    );
    assert_eq!(code, "DW0478");
    assert!(message.contains("actor/warden"), "{message}");
}

// ---------------------------------------------------------------------------
// Route 3 — dominance (spec-0044 §6)
// ---------------------------------------------------------------------------

/// **Dominance, red then green, on one geometry.** The seat stands on the guard
/// wave's own seated cell — and this keep's remaining forced walk routes back
/// through that room. Armed BEFORE that walk, the retry delivers an encounter the
/// path already delivers no-more-gently and the pair is credited; armed after the
/// last of it, nothing forced follows and the same geometry is red.
///
/// Only the arming beat differs between the two builds, which is what makes this
/// a test of the reign bound rather than of the geometry.
#[test]
fn a_forced_beat_inside_the_reign_dominates_and_one_outside_it_does_not() {
    let green = TempCampaign::new("dom-green");
    campaign_from("souls-bonfire", green.path(), |q| {
        reseat(q, "anchor/keeper-stand", "obj/slay", json!([]));
    });
    let out = build_from("souls-bonfire", green.path())
        .expect("the forced walk re-crosses the guard room");
    let l = ledger(&out);
    let credits = l["rest_points"][0]["credited"].as_array().unwrap();
    let guards = credits
        .iter()
        .find(|c| c["id"] == "wave/guards")
        .unwrap_or_else(|| panic!("the guard pair must be answered by a credit: {l}"));
    assert_eq!(guards["kind"], "dominated", "{l}");
    let reason = guards["reason"].as_str().unwrap();
    assert!(
        reason.contains("blocks from that force's stationary cells")
            && reason.contains("while the seat stands"),
        "the credit states BOTH distances: {reason}"
    );

    let red = TempCampaign::new("dom-red");
    campaign_from("souls-bonfire", red.path(), |q| {
        reseat(q, "anchor/keeper-stand", "obj/shrine", json!([]));
    });
    let (code, message) = diagnostic(
        build_from("souls-bonfire", red.path())
            .expect_err("a beat in a different reign proves nothing about this retry"),
    );
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("anchor/keeper-stand") && message.contains("wave/guards"),
        "{message}"
    );
}

/// **A lane's march corridor never dominates**, and this is the case the whole
/// criterion was created for: a fire beside the end of a siege lane, which killed
/// a live playtest run at 17.7 blocks from a 16-`follow_range` lane.
///
/// The forced path crosses this squad's corridor — the lane's own waypoints are
/// path anchors — so a dominance route that read lane cells would credit exactly
/// the placement the rule exists to refuse. It stays red, and the message says it
/// is the MARCH that reaches the seat.
#[test]
fn a_lane_march_corridor_never_dominates() {
    let tmp = TempCampaign::new("lane");
    campaign_from("souls-td-lanes", tmp.path(), |q| {
        let quest = &mut q["content"]["quests"].as_array_mut().unwrap()[0];
        quest["on_objective_complete"]["obj/take-post"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "set-checkpoint", "anchor": "anchor/keeper-stand"}));
    });
    let (code, message) = diagnostic(
        build_from("souls-td-lanes", tmp.path())
            .expect_err("a seat on a marching squad's corridor is the motivating red"),
    );
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("lane path cell") && message.contains("marching drift"),
        "the corridor is what reaches the seat, and the message must say so: {message}"
    );
}

// ---------------------------------------------------------------------------
// The report (spec-0044 §5)
// ---------------------------------------------------------------------------

/// **One build states every violating pair.** The diagnostic used to return at
/// the first, so enumerating a campaign's violations at all needed a patched
/// binary — which is how six false verdicts hid behind one.
#[test]
fn every_violating_pair_is_stated_in_one_build() {
    let tmp = TempCampaign::new("all-pairs");
    campaign_from("souls-bonfire", tmp.path(), |q| {
        reseat(q, "anchor/door", "obj/shrine", json!([]));
        hostile_actor(q, "actor/warden", "anchor/exit");
    });
    let (code, message) = diagnostic(
        build_from("souls-bonfire", tmp.path()).expect_err("two forces reach this seat"),
    );
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("actor/warden") && message.contains("wave/guards"),
        "both violating pairs must be named by ONE build: {message}"
    );
    assert!(
        message.contains("pairs in total"),
        "and the message must say how many there are: {message}"
    );
}
