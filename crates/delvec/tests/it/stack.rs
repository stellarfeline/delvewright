//! spec-0134 §5, the compiler's half: a rocket stack's stars are emitted as the
//! `minecraft:fireworks` component at every stack site that carries them — a
//! `give-item` and a class kit here, on the built hello-world; a `loot` fill in
//! the renderer's unit tests (`emit::loot`), which hello-world's anchors cannot
//! seat — and the `firework` effect's summoned rocket is rendered by the same
//! function, so a star means one thing everywhere.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};
use serde_json::json;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-stack-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn build(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the fixture must validate: {errors:#?}");
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
    .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
}

/// Every function line of the build containing `needle`.
fn lines_with(out: &BuildOutput, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (path, bytes) in out.iter() {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        for l in String::from_utf8_lossy(bytes).lines() {
            if l.contains(needle) {
                found.push(l.to_string());
            }
        }
    }
    found
}

/// The stars the fixture declares, and the component value they render to.
const STARS_VALUE: &str =
    "fireworks={flight_duration:2b,explosions:[{shape:\"star\",colors:[I;16766720],has_trail:1b}]}";

#[test]
fn a_rocket_given_and_kitted_carries_its_stars() {
    let dir = scratch("sites");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let stars = json!({"flight": 2, "explosions": [
        {"shape": "star", "colors": ["#ffd700"], "trail": true}
    ]});
    let fw = stars.clone();
    common::patch_file(&dir.join("quests.json"), move |q| {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(
                json!({"type": "give-item", "item": "minecraft:firework_rocket",
                         "count": 3, "fireworks": fw,
                         "happening": {"text": "the keeper hands over rockets", "verb": "learns"}}),
            );
    });
    common::patch_file(&dir.join("classes.json"), move |c| {
        c["content"]["classes"][0]["kit"]
            .as_array_mut()
            .unwrap()
            .push(json!({"item": "minecraft:firework_rocket", "count": 4, "fireworks": stars}));
    });
    let out = build(&dir);
    let lines = lines_with(&out, "minecraft:firework_rocket[");
    let give = lines
        .iter()
        .filter(|l| {
            l.contains(&format!(
                "give @a minecraft:firework_rocket[{STARS_VALUE}] 3"
            ))
        })
        .count();
    let kit = lines
        .iter()
        .filter(|l| {
            l.contains(&format!(
                "give @s minecraft:firework_rocket[{STARS_VALUE}] 4"
            ))
        })
        .count();
    assert_eq!(give, 1, "the give-item stack: {lines:#?}");
    assert_eq!(kit, 1, "the kit stack: {lines:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A stack with no `fireworks` is byte-identical to the emission before the
/// field: no `fireworks=` anywhere in hello-world's build.
#[test]
fn no_stars_no_component() {
    let dir = scratch("none");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let out = build(&dir);
    assert!(lines_with(&out, "fireworks=").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
