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

/// Declare a `quests.json` `dsl_version` — the document whose declaration decides
/// whether a plain `set-checkpoint` is inside `DW0478`'s binding yet
/// (`nav::PLAIN_CHECKPOINT_BINDS_AT`). The fixture ships 0.6.0, so a test that
/// wants the widened binding has to say so, which is the whole point: adoption is
/// something a campaign DOES, never something an engine build does to it.
fn adopt_v011(quests: &mut serde_json::Value) {
    quests
        .as_object_mut()
        .unwrap()
        .insert("dsl_version".into(), serde_json::json!("0.11.0"));
}

/// Move the fixture's `bonfire` where it stands — same verb, new anchor.
fn move_bonfire(quests: &mut serde_json::Value, anchor: &str) {
    fn walk(v: &mut serde_json::Value, anchor: &str, hit: &mut bool) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("type").and_then(|t| t.as_str()) == Some("bonfire") {
                    *hit = true;
                    o.insert("anchor".into(), serde_json::json!(anchor));
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
        "the fixture must carry the one bonfire this suite moves"
    );
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
    campaign_with(tmp.path(), |q| {
        respell_as_checkpoint(q, None);
        adopt_v011(q);
    });
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
        adopt_v011(q);
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

// ---------------------------------------------------------------------------
// The obligation fence on the widening (owner ruling 2026-08-10, restated
// 2026-08-11 against the argument that this particular widening was exempt).
//
// The code is `every_version` and stays that way; the BINDING is what widened,
// so the binding is what carries the version — per respawn point, keyed to the
// stage document that authored the verb. These four tests are the two directions
// of that split, and the third is the one that matters most: the fence must not
// have bought the island's green with a weaker rule.
// ---------------------------------------------------------------------------

/// **The grandfather direction.** The identical geometry the test above rejects,
/// on a campaign whose `quests.json` still declares 0.6.0: it builds. Nothing in
/// this campaign's own documents changed, so no engine build may change its
/// verdict — the whole content of the ruling.
///
/// And the green is not silent. A fenced-out respawn point is published, named,
/// with the version that will bind it, because "this campaign has no respawn
/// point" and "this campaign has one and it was never looked at" are different
/// findings and the ledger has to be able to tell them apart.
#[test]
fn a_pre_0_11_set_checkpoint_on_a_seated_wave_is_grandfathered() {
    let tmp = TempCampaign::new("grandfathered");
    campaign_with(tmp.path(), |q| {
        respell_as_checkpoint(q, Some("anchor/keeper-stand"));
    });
    let out = build(tmp.path()).expect(
        "a 0.6.0 campaign compiled before the widening and must compile after it: the \
         compiler judges a campaign at the dsl_version it declares",
    );
    let l = ledger(&out);
    assert_eq!(l["examined"], 0);
    assert_eq!(l["unbound"], true);
    let g = l["grandfathered"]
        .as_array()
        .expect("the ledger publishes what the fence withheld");
    assert_eq!(g.len(), 1, "the one respawn point this campaign has: {l}");
    assert_eq!(g[0]["anchor"], "anchor/keeper-stand");
    assert_eq!(g[0]["kind"], "set-checkpoint");
    assert_eq!(
        g[0]["stage"], "quests",
        "the version question is asked of the document the verb was authored in: {l}"
    );
    assert_eq!(g[0]["binds_at_dsl_version"], "0.11.0");
    let why = l["reason"].as_str().expect("a zero binding names itself");
    assert!(
        why.contains("GRANDFATHERED") && why.contains("anchor/keeper-stand"),
        "the zero must read as an adoption worklist, not as a pass: {why}"
    );
}

/// **The bind direction, same bytes.** The one difference between this campaign
/// and the one above is the number in `quests.json` — and it is a red.
///
/// This is what keeps the fence from being a deletion: at and above 0.11 the rule
/// demands exactly what it demanded before, on exactly the same geometry.
#[test]
fn the_same_geometry_at_0_11_is_dw0478() {
    let old = TempCampaign::new("fence-old");
    campaign_with(old.path(), |q| {
        respell_as_checkpoint(q, Some("anchor/keeper-stand"));
    });
    let adopted = TempCampaign::new("fence-new");
    campaign_with(adopted.path(), |q| {
        respell_as_checkpoint(q, Some("anchor/keeper-stand"));
        adopt_v011(q);
    });
    // The two campaigns differ in one field, and it is not a geometric one.
    let read = |t: &TempCampaign| {
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(t.path().join("quests.json")).unwrap())
                .unwrap();
        v.as_object_mut().unwrap().remove("dsl_version");
        v
    };
    assert_eq!(
        read(&old),
        read(&adopted),
        "the pair must differ ONLY in the declared version, or this proves nothing about \
         the fence"
    );

    build(old.path()).expect("0.6.0: grandfathered");
    let err = build(adopted.path())
        .expect_err("0.11.0: the same cell inside the same perception radius is the same red");
    let BuildFailure::Diagnostic { code, .. } = err else {
        panic!("expected a diagnostic");
    };
    assert_eq!(code, "DW0478");
}

/// **The half that was never fenced and must never be.** A `bonfire` has been in
/// this proof's binding since the proof existed, at every declared version. The
/// campaign here declares 0.6.0 — below the widening — and its bonfire on the
/// guard's cell is still a red.
///
/// A `DwCode::since` on `DW0478` would have made this green. That is why the
/// version lives on the binding and not on the code: fencing the code would have
/// grandfathered a rule that had already bound, which is a weakening, not a
/// fence.
#[test]
fn a_bonfire_on_a_seated_wave_is_dw0478_below_the_widening_version() {
    let tmp = TempCampaign::new("bonfire-unsafe");
    campaign_with(tmp.path(), |q| move_bonfire(q, "anchor/keeper-stand"));
    let err = build(tmp.path())
        .expect_err("a bonfire in a perception radius is a soft-lock at every declared version");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a diagnostic");
    };
    assert_eq!(code, "DW0478");
    assert!(
        message.contains("bonfire") || message.contains("respawn point"),
        "{message}"
    );
}

/// A campaign that mixes the two kinds is judged per object, not per campaign: the
/// bonfire is examined at 0.6.0 while its sibling is withheld. Binding is a
/// property of the respawn point, so a campaign is never all-or-nothing.
#[test]
fn a_mixed_campaign_examines_the_bonfire_and_withholds_the_checkpoint() {
    let tmp = TempCampaign::new("mixed");
    campaign_with(tmp.path(), |q| {
        // Keep the fixture's bonfire and add a plain checkpoint on the guard's
        // cell — geometry that is DW0478 the moment `quests.json` adopts 0.11.
        let quests = q["content"]["quests"].as_array_mut().unwrap();
        let first = quests[0].as_object_mut().unwrap();
        let on_complete = first
            .entry("on_complete")
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .unwrap();
        on_complete.push(serde_json::json!({
            "type": "set-checkpoint",
            "anchor": "anchor/keeper-stand"
        }));
    });
    let out = build(tmp.path()).expect("the bonfire is where it always was, and it is safe");
    let l = ledger(&out);
    assert_eq!(l["examined"], 1, "the bonfire, at 0.6.0: {l}");
    assert_eq!(l["rest_points"][0]["kind"], "bonfire");
    assert_eq!(l["unbound"], false, "so the proof is NOT vacuous here: {l}");
    let g = l["grandfathered"].as_array().unwrap();
    assert_eq!(
        g.len(),
        1,
        "and the sibling is withheld, not forgotten: {l}"
    );
    assert_eq!(g[0]["kind"], "set-checkpoint");
}
