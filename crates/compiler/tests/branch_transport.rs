//! Branch-aware inter-area transport (task #186): a crossing that only happens
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
//! (`area/galley`) that only the bolt branch ever enters. The default (hold)
//! path never leaves the keep, so the crossing is branch-only by construction.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::{self, Plan};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::Value;

/// The galley's `spawn` anchor in world coordinates — the destination the bolt
/// branch's crossing has to land on. Asserted against the exported branch path
/// below, so a fixture drift fails loudly instead of silently.
const GALLEY_SPAWN: [i32; 3] = [260, 69, 11];

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("branch-transport")
}

fn build_campaign(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");

    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
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

/// The bolt branch crosses into the galley when it completes `obj/bolt`; the
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
        serde_json::json!(GALLEY_SPAWN),
        "fixture drift: the bolt branch must cross into area/galley at obj/bolt"
    );

    // The delivery: `complete_obj_bolt` carries the crossing, conditioned on the
    // bolt branch's own flag assignment (`flag/flee` set, `flag/wait` unset).
    let body = complete_fn(&out, "obj/bolt");
    let expected = format!(
        "execute if score {p} {flee} matches 1 unless score {p} {wait} matches 1 run teleport @s {} {} {}",
        GALLEY_SPAWN[0],
        GALLEY_SPAWN[1],
        GALLEY_SPAWN[2],
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
