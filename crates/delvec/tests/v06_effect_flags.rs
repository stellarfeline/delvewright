//! DSL v0.6 end-to-end emission: per-effect `requires_flags` wraps a
//! gated effect's commands in a per-player `execute if score @s dw.f_<flag>
//! matches 1 run …` guard, and a block field carries a vanilla blockstate suffix
//! verbatim into `setblock`. Driven by the `v06-flags` fixture campaign.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("v06-flags")
}

/// Build the v0.6 fixture, asserting DSL validation is clean under the full
/// registries, and return the emitted output.
fn build_v06() -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v06-flags parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "v06-flags must validate clean: {diags:#?}"
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/hello-world/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// A gated effect's command is wrapped in a **party** flag guard (spec-0018:
/// story flags are one holder, so the gate asks the party, not a player); the
/// ungated `set-flag`/`open-gate` on the same objective are NOT wrapped.
#[test]
fn gated_effect_is_wrapped_in_party_flag_guard() {
    let out = build_v06();
    let talk = fn_body(&out, "complete_o_talk");
    // The gated narrate fires only once the party holds the flag — and then it
    // reaches every member.
    assert!(
        talk.contains(
            "execute if score #party dw.f_opened matches 1 run tellraw @a \
             {\"text\":\"Only the hand that opened it hears the hinge give.\"}"
        ),
        "gated narrate must be wrapped in `execute if score #party dw.f_opened matches 1 run …`:\n{talk}"
    );
    // The ungated set-flag on the same objective stays a bare command.
    assert!(
        talk.contains("scoreboard players set #party dw.f_opened 1"),
        "ungated set-flag must remain unwrapped:\n{talk}"
    );
}

/// A block field's vanilla blockstate suffix reaches `setblock` verbatim.
#[test]
fn blockstate_suffix_reaches_setblock_verbatim() {
    let out = build_v06();
    let talk = fn_body(&out, "complete_o_talk");
    assert!(
        talk.contains("minecraft:grindstone[face=floor]"),
        "blockstate suffix must pass through to setblock verbatim:\n{talk}"
    );
}

/// The build is byte-identical across two runs (determinism, ADR-0006).
#[test]
fn v06_build_is_deterministic() {
    let a = build_v06();
    let b = build_v06();
    assert_eq!(a.len(), b.len(), "same file set across builds");
    for (path, bytes) in &a {
        assert_eq!(
            Some(bytes),
            b.get(path),
            "file {path} differs between two builds"
        );
    }
}
