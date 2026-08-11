//! `DW0478` over EVERY respawn point, and the ledger that says how many it
//! examined (staging-gate row `bell-08`).
//!
//! The owner's finding was *"rest points sat inside the aggro geometry of the
//! bodies the party was resting from"*. The check built for it quantified over
//! `plan.bonfires()` — the verb — so a campaign that spells its respawn points
//! `set-checkpoint` was examined ZERO times and reported green. A `bonfire` and
//! a `set-checkpoint` are siblings of one sum type, resolve to one
//! `CheckpointPlan`, and deliver a dead player by the identical vanilla
//! `spawnpoint`: the hazard belongs to the CELL.
//!
//! Every case is the `souls-bonfire` fixture with its one rest point respelled or
//! moved, so what the diagnostic reacts to is unambiguous.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

struct TempCampaign(std::path::PathBuf);

impl TempCampaign {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "delvewright-respawn-{tag}-{}-{:?}",
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

/// Materialize the fixture, then rewrite its one rest point.
fn campaign_with(dst: &Path, mutate: impl FnOnce(&mut serde_json::Value)) {
    common::materialize_from(&fixture_dir(), &serde_json::json!({}), dst);
    let p = dst.join("quests.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    mutate(&mut quests);
    std::fs::write(&p, serde_json::to_string_pretty(&quests).unwrap()).unwrap();
}

fn build(dir: &Path) -> Result<BuildOutput, BuildFailure> {
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
        "the fixture mutation must stay schema-valid so the BUILD tier is what is under \
         test: {diags:#?}"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                fixture_dir()
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
        "unpinned",
        &skins,
    )
}

/// Rewrite the fixture's `bonfire` effect into a plain `set-checkpoint`,
/// optionally moving it to a different anchor.
fn respell_as_checkpoint(quests: &mut serde_json::Value, anchor: Option<&str>) {
    fn walk(v: &mut serde_json::Value, anchor: Option<&str>, hit: &mut bool) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("type").and_then(|t| t.as_str()) == Some("bonfire") {
                    *hit = true;
                    o.insert("type".into(), serde_json::json!("set-checkpoint"));
                    if let Some(rest) = o.remove("on_rest") {
                        o.insert("on_respawn".into(), rest);
                    }
                    for k in ["prompt", "rest_label", "save_label"] {
                        o.remove(k);
                    }
                    if let Some(a) = anchor {
                        o.insert("anchor".into(), serde_json::json!(a));
                    }
                    return;
                }
                for (_, child) in o.iter_mut() {
                    walk(child, anchor, hit);
                }
            }
            serde_json::Value::Array(a) => {
                for child in a {
                    walk(child, anchor, hit);
                }
            }
            _ => {}
        }
    }
    let mut hit = false;
    walk(quests, anchor, &mut hit);
    assert!(
        hit,
        "the fixture must carry exactly the one bonfire this suite rewrites"
    );
    // `respawns_on_rest` hangs off the bonfire that is now gone (`DW0370`), and
    // this suite is about the safe-zone proof, not about that coupling.
    for w in quests["content"]["waves"].as_array_mut().unwrap() {
        w.as_object_mut().unwrap().remove("respawns_on_rest");
    }
}

fn ledger(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/respawn-safety.json")
            .expect("every build that assembles a world states this proof's binding count"),
    )
    .unwrap()
}

/// The control: the fixture's own bonfire, unchanged. It must still be examined
/// against every force, and the ledger must say so out loud.
#[test]
fn a_bonfire_is_examined_and_the_ledger_states_the_count() {
    let tmp = TempCampaign::new("bonfire");
    common::materialize_from(&fixture_dir(), &serde_json::json!({}), tmp.path());
    let out = build(tmp.path()).expect("the untouched fixture builds");
    let l = ledger(&out);
    assert_eq!(l["code"], "DW0478");
    assert_eq!(l["examined"], 1);
    assert_eq!(l["rest_points"][0]["kind"], "bonfire");
    assert!(
        l["rest_points"][0]["reign_end"].is_null(),
        "a bonfire never stops reigning: {l}"
    );
    assert!(
        l["pairs"].as_u64().unwrap() >= 2,
        "one rest point against both waves: {l}"
    );
    assert_eq!(l["unbound"], false);
    assert!(l["reason"].is_null());
}

/// **The binding that was missing.** The same cell, respelled as the sibling
/// verb, is examined by the same proof — before this it was examined zero times.
#[test]
fn a_plain_set_checkpoint_is_examined_by_the_same_proof() {
    let tmp = TempCampaign::new("checkpoint");
    campaign_with(tmp.path(), |q| respell_as_checkpoint(q, None));
    let out = build(tmp.path()).expect("a safe cell is safe whichever verb placed it");
    let l = ledger(&out);
    assert_eq!(l["examined"], 1);
    assert_eq!(
        l["rest_points"][0]["kind"], "set-checkpoint",
        "the proof now quantifies over the sibling too: {l}"
    );
    assert!(
        l["pairs"].as_u64().unwrap() >= 1,
        "and it actually compared it against something: {l}"
    );
    assert!(
        !l["rest_points"][0]["compared_against"]
            .as_array()
            .unwrap()
            .is_empty(),
        "{l}"
    );
}

/// …and the demand on it is the same demand. Move that checkpoint onto the cell
/// a wave is seated at and the build fails, exactly as a bonfire there would.
#[test]
fn a_set_checkpoint_on_a_seated_wave_is_dw0478() {
    let tmp = TempCampaign::new("unsafe");
    campaign_with(tmp.path(), |q| {
        respell_as_checkpoint(q, Some("anchor/keeper-stand"));
    });
    let err = build(tmp.path()).expect_err("a respawn cell inside a perception radius is a red");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a diagnostic");
    };
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("respawn point") && message.contains("set-checkpoint"),
        "the message must name the class it is talking about, not just the bonfire verb: {message}"
    );
    // Both cheap ways out of this red are a vacuous green, and the message has to
    // close both. Shrinking `follow_range` retunes the fight; deleting the
    // checkpoint deletes the die-retry stage's precondition, so the ladder goes
    // back to passing over zero scripted deaths — the state every campaign in
    // both repos was in on 2026-08-11.
    assert!(
        message.contains("Do NOT shrink `follow_range`"),
        "the message must still refuse the retune: {message}"
    );
    assert!(
        message.contains("die-retry"),
        "the message must also refuse the other cheap green — deleting the \
         checkpoint the retry proof binds on: {message}"
    );
}
