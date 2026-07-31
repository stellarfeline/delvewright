//! DSL v0.4 (spec-0008 + addendum, spec-0009) end-to-end emission tests, driven
//! by the `v04-showcase` fixture campaign which exercises every new verb: a
//! skinned (mannequin) NPC, props, narration, wave tuning, NPC move/despawn,
//! environment triggers, a cutscene, named given items, and flag-gated dialogue.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join("v04-showcase")
}

/// Build the v0.4 showcase, returning the build output.
fn build_showcase() -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v04-showcase parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    // DSL validation with the full registries must be clean (no diagnostics).
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "v04-showcase must validate clean: {diags:#?}"
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
        "unpinned",
        &skins,
    )
    .expect("every emitted command validates")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/v04-showcase/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The mannequin summon carries a **component-form** `description` and never a
/// stringified-JSON text component (the `'{"text"` form renders as raw JSON above
/// the head on 1.21.11 — owner-verified). Tripwire over every emitted function.
#[test]
fn mannequin_description_is_component_form_no_stringified_json() {
    let out = build_showcase();
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains("summon minecraft:mannequin")
            && setup.contains("description:{text:\"The Keeper\"}"),
        "mannequin summon with component-form description expected"
    );
    // The mannequin declares its pose explicitly; omitting it serializes as `DYING`
    // (a gametest save-teardown warning). An NPC stands.
    assert!(
        setup.contains("pose:\"standing\""),
        "mannequin summon must set pose:\"standing\", not default to DYING"
    );
    for (path, bytes) in &out {
        if path.ends_with(".mcfunction") {
            let body = std::str::from_utf8(bytes).unwrap();
            assert!(
                !body.contains("'{\"text"),
                "stringified-JSON text component leaked into {path}"
            );
        }
    }
}

/// A skinned campaign emits a resource pack + SKINS.md; the manifest records the
/// pack SHA-1 (spec-0009).
#[test]
fn resource_pack_and_sha1_emitted() {
    let out = build_showcase();
    assert!(out.contains_key("resourcepack.zip"), "resource pack zip");
    assert!(out.contains_key("SKINS.md"), "SKINS.md note");
    let manifest = std::str::from_utf8(&out["manifest.json"]).unwrap();
    assert!(
        manifest.contains("\"resource_pack_sha1\""),
        "manifest records resource_pack_sha1"
    );
}

/// The critical path carries the harness contract fields: version `0.4.0`,
/// `sneak: true` on the stealth leg, and `cutscene_seconds` on the completing
/// step.
#[test]
fn critical_path_has_sneak_and_cutscene_seconds() {
    let out = build_showcase();
    let cp = std::str::from_utf8(&out["critical-path.json"]).unwrap();
    assert!(cp.contains("\"version\": \"0.4.0\""), "version 0.4.0");
    assert!(cp.contains("\"sneak\": true"), "sneak hint on stealth leg");
    assert!(
        cp.contains("\"cutscene_seconds\": 2"),
        "cutscene_seconds on step"
    );
}

/// Every v0.4 verb reaches emission: named give-item, narrate, prop, set-block,
/// wave attributes/effects, despawn, move, cutscene, and triggers.
#[test]
fn every_v04_verb_emitted() {
    let out = build_showcase();
    let talk = fn_body(&out, "complete_o_talk");
    assert!(
        talk.contains("give @s minecraft:paper[custom_name="),
        "named give-item"
    );
    assert!(
        talk.contains("title @s subtitle {\"text\":\"The guard rises around you.\"}"),
        "narrate subtitle"
    );

    // prop lever + set-block torch land in the objective activation / completion.
    let all: String = out
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        all.contains("setblock") && all.contains("minecraft:lever"),
        "interact prop"
    );
    assert!(all.contains("minecraft:torch"), "set-block torch");
    assert!(all.contains("kill @e[tag=dw_npc_keeper]"), "despawn-npc");

    let spawn = fn_body(&out, "spawn_guards");
    assert!(
        spawn.contains("attributes:[{id:\"minecraft:max_health\",base:12.0}"),
        "wave attributes"
    );
    assert!(
        spawn.contains("effect give @e[tag=dw_tmp] minecraft:slowness infinite 1 true"),
        "wave effect"
    );

    // generated driver functions (move / cutscene / triggers).
    for name in [
        "mv_keeper_objective",
        "cs_objective_2_2",
        "cs_tick_objective_2_2",
        "cs_end_objective_2_2",
        "trig_gate_ward",
        "trig_shrine_hum",
    ] {
        assert!(
            out.contains_key(&format!(
                "datapack/data/v04-showcase/function/{name}.mcfunction"
            )),
            "missing generated fn {name}"
        );
    }
    // cutscene never emits the server-noop same-entity re-spectate: it alternates
    // between two distinct cameras.
    let cst = fn_body(&out, "cs_tick_objective_2_2");
    assert!(
        cst.contains("dw_cama_") && cst.contains("dw_camb_"),
        "two-camera bounce"
    );
}

/// Flag-gated dialogue: the gated option is absent in the pre-flag variant and
/// present in the post-flag variant (spec-0008 §1).
#[test]
fn flag_gated_dialogue_variants() {
    let out = build_showcase();
    let m0 = std::str::from_utf8(&out["datapack/data/v04-showcase/dialog/keeper_greet__m0.json"])
        .unwrap();
    let m1 = std::str::from_utf8(&out["datapack/data/v04-showcase/dialog/keeper_greet__m1.json"])
        .unwrap();
    assert!(
        !m0.contains("What lies past the door?"),
        "gated option absent before flag"
    );
    assert!(
        m1.contains("What lies past the door?"),
        "gated option present after flag"
    );
}

/// Kill-less `spawn-wave` (spec-0008 §4 live threat): `wave/ambush` is spawned by a
/// `spawn-wave` on the `obj/door` interact step and is NEVER referenced by a `kill`
/// objective. It must still emit a `spawn_ambush` function that summons its mobs
/// (regression: `wave_spawn_pos` used to resolve a position only via a `kill`
/// objective, so the function was un-emitted and the effect's call dangled), and a
/// PackTest must assert the mobs exist.
#[test]
fn killless_spawn_wave_emits_function_and_packtest() {
    let out = build_showcase();

    // The spawn function exists and summons the wave's mobs at a resolved position.
    let spawn = fn_body(&out, "spawn_ambush");
    assert!(
        spawn.matches("summon minecraft:sheep").count() == 2,
        "kill-less wave spawns its two mobs: {spawn}"
    );

    // The `spawn-wave` effect's call is not dangling — the driver it lives in
    // references the emitted function.
    let door = fn_body(&out, "complete_o_door");
    assert!(
        door.contains("function v04-showcase:spawn_ambush"),
        "obj/door completion calls the emitted spawn function"
    );

    // A regression PackTest asserts the mobs exist after the wave fires.
    let pt = std::str::from_utf8(
        &out["packtest-datapack/data/v04-showcase/test/v04_killless_wave.mcfunction"],
    )
    .unwrap();
    assert!(
        pt.contains("function v04-showcase:spawn_ambush")
            && pt.contains("if entity @e[tag=dw_wave_ambush]")
            && pt.contains("assert score #kw dw.sys matches 2"),
        "kill-less wave PackTest spawns then asserts the mob count: {pt}"
    );
}

/// `wave_area` resolves a wave's spawn area from its `spawn-wave` site regardless
/// of objective type: `wave/ambush` (kill-less, spawned on the interact step) and
/// `wave/guards` (spawned then killed) both resolve to `area/keep`; an unspawned
/// wave id resolves to `None` (which the DW0309 build guard would reject).
#[test]
fn wave_area_resolves_from_spawn_site_not_kill() {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v04-showcase parses");
    use delvewright_compiler::plan::wave_area;
    assert_eq!(wave_area(&campaign, "wave/ambush"), Some("area/keep"));
    assert_eq!(wave_area(&campaign, "wave/guards"), Some("area/keep"));
    assert_eq!(wave_area(&campaign, "wave/nope"), None);
}
