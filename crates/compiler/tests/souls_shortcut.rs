//! spec-0016 §2 (shortcut doors) end-to-end tests, driven by the `souls-shortcut`
//! fixture: hello-world with its inner door reclassified from an `open-gate`
//! objective reward into a `shortcut`, plus a stage-7 `carve` that opens the LONG
//! way round through the same wall. That carve is what makes the fixture a real
//! souls loop rather than a locked door — and a clean build is exactly the
//! DW0373 (long route exists) + DW0374 (opening it pays) proof.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-shortcut";

fn build_fixture() -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-shortcut parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "souls-shortcut must validate clean: {diags:#?}"
    );

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates (DW0373/DW0374 hold on the fixture)")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The unlock affordance is placed at world init on the FAR side, and the gate
/// itself gets no setup command at all — it is sealed by the prefab, which is why
/// permanence can be structural rather than a runtime discipline.
#[test]
fn setup_places_the_unlock_affordance_and_leaves_the_gate_alone() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains("Tags:[\"dw_sc_inner_door\"]"),
        "the far-side unlock affordance is summoned at world init: {setup}"
    );
    assert!(
        !setup.contains("minecraft:iron_bars"),
        "the gate is sealed by the prefab; setup must not fill it: {setup}"
    );
}

/// Unlocking clears the gate region with the anchor's own declared block and is
/// latched by a one-shot sentinel — the runtime half of permanence.
#[test]
fn unlock_clears_the_gate_once_and_forever() {
    let out = build_fixture();
    let open = fn_body(&out, "shortcut_open_inner_door");
    let lines: Vec<&str> = open.lines().collect();
    assert_eq!(
        lines[0], "scoreboard players set #sc_inner_door dw.sys 1",
        "the sentinel latches first: {open}"
    );
    assert!(
        lines[1].starts_with("fill ")
            && lines[1].ends_with("minecraft:air replace minecraft:iron_bars"),
        "the gate region is cleared with the anchor's declared block: {open}"
    );
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute unless score #sc_inner_door dw.sys matches 1 if entity @e[tag=dw_sc_inner_door,nbt={{interaction:{{}}}}] run function {NS}:shortcut_open_inner_door"
        )),
        "the unlock is polled once, guarded by the sentinel: {tick}"
    );
}

/// Nothing anywhere in the shipped datapack ever re-fills a shortcut gate. This is
/// the emission-side counterpart of `DW0372`: the validator forbids authoring the
/// re-seal, and this asserts the compiler never emits one on its own.
#[test]
fn no_emitted_function_ever_reseals_the_shortcut_gate() {
    let out = build_fixture();
    for (path, bytes) in &out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        for line in body.lines() {
            // A seal is a `fill … <gate block>` with NO `replace` clause; the
            // unlock's own `fill … minecraft:air replace minecraft:iron_bars` is
            // the open, and is the only line allowed to name the gate block last.
            assert!(
                !(line.starts_with("fill ")
                    && line.ends_with(" minecraft:iron_bars")
                    && !line.contains(" replace ")),
                "{path} re-seals the shortcut gate: {line}"
            );
        }
    }
}

/// The `on_unlock` beat rides the same audience contract as every other
/// tick-dispatched bundle (spec-0018): the poll has no `@s`, so a player-facing
/// effect addresses the party rather than a nonexistent actor. Opening a shortcut
/// is a party fact — everyone's route just changed.
#[test]
fn on_unlock_reaches_the_party() {
    let out = build_fixture();
    let open = fn_body(&out, "shortcut_open_inner_door");
    assert!(
        open.contains("title @a subtitle"),
        "the on_unlock narrate addresses every player, not a nonexistent @s: {open}"
    );
    assert!(
        !open.contains("@s"),
        "nothing in a tick-dispatched bundle may address @s: {open}"
    );
}

/// The generated PackTest drives the real unlock on a live server: sealed before,
/// air after, and still air after a second pass (permanence).
#[test]
fn shortcut_runtime_behaviour_is_packtested() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_shortcut.mcfunction"
        ))
        .expect("shortcut PackTest emitted"),
    )
    .unwrap();
    assert!(
        t.contains(&format!("function {NS}:shortcut_open_inner_door")),
        "the template drives the REAL unlock function: {t}"
    );
    assert_eq!(
        t.matches("function souls-shortcut:shortcut_open_inner_door")
            .count(),
        2,
        "it unlocks twice to prove the second pass cannot re-seal: {t}"
    );
    assert!(
        t.contains("assert score #sb_scut dw.sys matches 1")
            && t.contains("assert score #sa_scut dw.sys matches 1")
            && t.contains("assert score #sp_scut dw.sys matches 1"),
        "sealed-before / open-after / still-open asserts all present: {t}"
    );
}
