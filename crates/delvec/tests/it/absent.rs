//! The party is every member, present or not (spec-0141).

use crate::common;

use std::collections::BTreeMap;
use std::path::Path;

use delvec::compiler::absent;
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
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
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &skins,
    )
    .expect("emission succeeds")
}


fn families() -> Vec<(&'static str, BuildOutput)> {
    vec![
        ("hello-world", build_dir(&common::hello_world_dir())),
        ("keep-crawl", build_dir(&common::keep_crawl_dir())),
        ("keep-trial", build_dir(&common::keep_trial_dir())),
        ("keep-vertical", build_dir(&common::keep_vertical_dir())),
        (
            "v04-showcase",
            build_dir(&common::compiler_fixtures_dir().join("v04-showcase")),
        ),
        (
            "v06-checkpoints",
            build_dir(&common::compiler_fixtures_dir().join("v06-checkpoints")),
        ),
        (
            "and-join",
            build_dir(&common::compiler_fixtures_dir().join("and-join")),
        ),
    ]
}

#[test]
fn zz_dump() {
    for (name, out) in families() {
        let c = absent::census(&out);
        eprintln!("== {name}: {}", c.binding());
        for (f, l) in &c.unserved {
            eprintln!("  UNSERVED {f}: {}", &l[..l.len().min(200)]);
        }
        for (f, l) in &c.unnamed {
            eprintln!("  UNNAMED {f}: {}", &l[..l.len().min(200)]);
        }
        eprintln!("  served {:?} named {:?}", c.served, c.named);
    }
}
