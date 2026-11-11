//! spec-0134 §9 — an effect aimed at a body. `audience` names a wave's or an
//! actor's bodies; `give-effect`, `clear-effect` and `damage` address them
//! through the engine's own tag, `in` composing as box arguments; `damage …
//! by: actor` credits the acting player. Refusals: a body audience on a verb
//! that reaches a player only (`DW0942`), `by` beside an audience of players
//! (`DW0942`), `by: actor` where nobody acts (`DW0503`), an undeclared body
//! (`DW0112`). The rename leaves no `damage-players` in the repository's
//! documents.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, Diagnostic, parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

fn campaign(
    effects: Vec<Value>,
    trigger_effects: Vec<Value>,
) -> (Campaign, BTreeMap<String, Vec<u8>>) {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    let q = &mut c.quests.content;
    let anchor = q.waves[0].anchor.as_str().to_string();
    q.actors.push(
        serde_json::from_value(json!({"id": "actor/moth", "entity": "minecraft:bat",
            "anchor": anchor, "vulnerable": true}))
        .unwrap(),
    );
    let bundle = q.quests[0]
        .on_objective_complete
        .get_mut(&delvewright_dsl::ObjectiveId("obj/talk".to_string()))
        .unwrap();
    for e in effects {
        bundle.push(serde_json::from_value(e).unwrap());
    }
    for e in trigger_effects {
        q.triggers[0]
            .effects
            .push(serde_json::from_value(e).unwrap());
    }
    (c, loaded.inputs)
}

fn diags(c: &Campaign) -> Vec<Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn build(c: &Campaign, inputs: &BTreeMap<String, Vec<u8>>) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
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
        inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn all_lines(out: &BuildOutput) -> Vec<String> {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .flat_map(|(_, b)| {
            String::from_utf8_lossy(b)
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn with<'a>(d: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    d.iter().filter(|x| x.code == code).collect()
}

#[test]
fn a_body_audience_takes_the_bodys_tag_and_composes_with_in() {
    let (c, inputs) = campaign(
        vec![
            json!({"type": "give-effect", "effect": "minecraft:glowing", "seconds": 10,
                   "audience": {"wave": "wave/guards"}}),
            json!({"type": "damage", "amount": 4, "by": "actor",
                   "audience": {"actor": "actor/moth"},
                   "in": {"anchor": "anchor/keeper-stand", "extent": [3, 2, 3]}}),
        ],
        vec![
            json!({"type": "clear-effect", "effect": "minecraft:glowing",
                    "audience": {"wave": "wave/guards"}}),
        ],
    );
    let d = diags(&c);
    let errors: Vec<_> = d
        .iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let lines = all_lines(&build(&c, &inputs));
    let has = |needle: &str| lines.iter().any(|l| l == needle);
    assert!(
        has("effect give @e[tag=dw_wave_guards] minecraft:glowing 10 0 false"),
        "{:#?}",
        lines
            .iter()
            .filter(|l| l.contains("glowing"))
            .collect::<Vec<_>>()
    );
    assert!(has("effect clear @e[tag=dw_wave_guards] minecraft:glowing"));
    let i = lines
        .iter()
        .position(|l| l == "tag @s add dw_damage_by")
        .expect("the actor is tagged");
    assert!(
        lines[i + 1].starts_with("execute as @e[tag=dw_actor_moth,x=")
            && lines[i + 1]
.ends_with(",tag=!dw_fixture] run damage @s 4 minecraft:generic by @a[tag=dw_damage_by,limit=1]"),
        "{}",
        lines[i + 1]
    );
    assert_eq!(lines[i + 2], "tag @s remove dw_damage_by");
}

#[test]
fn a_body_audience_on_a_line_is_dw0942() {
    let (c, _) = campaign(
        vec![json!({"type": "narrate", "text": "Glow.", "audience": {"wave": "wave/guards"}})],
        vec![],
    );
    let d = diags(&c);
    let hit = with(&d, "DW0942");
    assert_eq!(hit.len(), 1, "{d:#?}");
    assert!(hit[0].path.ends_with("/audience"), "{}", hit[0].path);
}

#[test]
fn by_beside_players_is_dw0942_and_where_nobody_acts_is_dw0503() {
    let (c, _) = campaign(
        vec![json!({"type": "damage", "amount": 2, "by": "actor"})],
        vec![],
    );
    let d = diags(&c);
    assert!(
        with(&d, "DW0942").iter().any(|x| x.path.ends_with("/by")),
        "{d:#?}"
    );
    let (c, _) = campaign(
        vec![],
        vec![json!({"type": "damage", "amount": 2, "by": "actor",
                    "audience": {"wave": "wave/guards"}})],
    );
    let d = diags(&c);
    assert!(
        with(&d, "DW0503").iter().any(|x| x.path.ends_with("/by")),
        "{d:#?}"
    );
}

#[test]
fn an_undeclared_body_is_dw0112() {
    let (c, _) = campaign(
        vec![
            json!({"type": "give-effect", "effect": "minecraft:glowing", "seconds": 5,
                   "audience": {"actor": "actor/nobody"}}),
        ],
        vec![],
    );
    let d = diags(&c);
    assert!(
        with(&d, "DW0112")
            .iter()
            .any(|x| x.path.ends_with("/audience/actor")),
        "{d:#?}"
    );
}

/// The rename: no document of the repository's own (decision records aside)
/// carries the old verb name.
#[test]
fn no_document_carries_the_old_verb() {
    let root = common::repo_root();
    let out = std::process::Command::new("git")
        .args(["grep", "-l", "damage-players", "--", "*.json"])
        .current_dir(&root)
        .output()
        .expect("git grep runs");
    let hits = String::from_utf8_lossy(&out.stdout);
    assert!(hits.trim().is_empty(), "documents still name it:\n{hits}");
}
