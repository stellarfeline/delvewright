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
        delvewright_compiler::light::has_night_vision(&campaign),
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

/// Display gating combines two axes into one per-node availability mask
/// (`dw.dmask`), bit `i` = the node's i-th gated option is displayable (task #54).
/// `dlg/greet` has three options: an ungated one ("Who are you?"), a completing
/// option ("I'll clear the keep." → obj/talk, bit 0, objective-state axis), and a
/// flag-gated one ("What lies past the door?" → flag/summoned, bit 1, flag axis).
/// So variant `__m<mask>` shows the ungated option always, the completing option
/// iff bit 0, the flag option iff bit 1.
#[test]
fn dialogue_display_gating_variants() {
    let out = build_showcase();
    let variant = |mask: u32| -> String {
        std::str::from_utf8(
            &out[&format!("datapack/data/v04-showcase/dialog/keeper_greet__m{mask}.json")],
        )
        .unwrap()
        .to_string()
    };
    let ungated = "Who are you?";
    let completing = "I'll clear the keep.";
    let flag_gated = "What lies past the door?";

    // The ungated option is present in every variant.
    for mask in 0..4 {
        assert!(
            variant(mask).contains(ungated),
            "ungated option always shown"
        );
    }
    // m0: nothing displayable → only the ungated option.
    assert!(!variant(0).contains(completing) && !variant(0).contains(flag_gated));
    // m1 (bit 0): completing option visible, flag option still hidden.
    assert!(variant(1).contains(completing) && !variant(1).contains(flag_gated));
    // m2 (bit 1): flag option visible, completing option hidden.
    assert!(variant(2).contains(flag_gated) && !variant(2).contains(completing));
    // m3: both.
    assert!(variant(3).contains(completing) && variant(3).contains(flag_gated));

    // The mask function mirrors the click-handler guard: bit 0 (the completing
    // option) is set iff obj/talk's quest is active AND the objective is not yet
    // complete; bit 1 iff the flag is set.
    let dmask = fn_body(&out, "dmask_keeper_greet");
    assert!(
        dmask.contains(
            "execute if score @s dw.qa_greet matches 1 unless score @s dw.o_talk matches 1 \
             run scoreboard players add @s dw.dmask 1"
        ),
        "completing option's availability bit mirrors the click guard: {dmask}"
    );
    assert!(
        dmask.contains(
            "execute if score @s dw.f_summoned matches 1 run scoreboard players add @s dw.dmask 2"
        ),
        "flag option's availability bit is the flag score: {dmask}"
    );
    // The chooser computes the mask, then shows the matching variant.
    let show = fn_body(&out, "show_keeper_greet");
    assert!(show.contains("function v04-showcase:dmask_keeper_greet"));
    assert!(show.contains("dialog show @s v04-showcase:keeper_greet__m3"));
}

/// The generated PackTest drives the availability mask through the objective-state
/// axis transitions (hidden before the quest activates, shown while active, hidden
/// again after completion) plus the flag axis in isolation (task #54).
#[test]
fn dialogue_visibility_packtest_covers_both_axes() {
    let out = build_showcase();
    let pt = std::str::from_utf8(
        &out["packtest-datapack/data/v04-showcase/test/v04_dialogue_visibility.mcfunction"],
    )
    .unwrap();
    // Quest inactive → mask 0 (option hidden).
    assert!(pt.contains("scoreboard players set @a dw.qa_greet 0"));
    // Quest active, objective incomplete → the completing option's bit (1).
    assert!(pt.contains("scoreboard players set @a dw.qa_greet 1"));
    assert!(pt.contains("assert score #dm dw.sys matches 1"));
    // Objective complete → hidden again (mask 0).
    assert!(pt.contains("scoreboard players set @a dw.o_talk 1"));
    // Flag axis in isolation → the flag option's bit (2).
    assert!(pt.contains("scoreboard players set @a dw.f_summoned 1"));
    assert!(pt.contains("assert score #dm dw.sys matches 2"));
    // Every phase runs the emitted mask function (no re-implementation).
    assert!(pt.contains("execute as @a run function v04-showcase:dmask_keeper_greet"));
    assert_eq!(
        pt.matches("assert score #dm dw.sys matches 0").count(),
        2,
        "hidden asserted before activation and after completion"
    );
    // The mask read must be single-entity: `scoreboard players get`/`operation`
    // reject a multi-entity selector, so a bare `@a` read is a load-time command
    // error (caught live by PackTest). Read via `as @a … = @s`.
    assert!(
        !pt.contains("get @a dw.dmask"),
        "the dmask read must not use a multi-entity `@a` selector: {pt}"
    );
    assert!(
        pt.contains("execute as @a run scoreboard players operation #dm dw.sys = @s dw.dmask"),
        "the dmask read copies @s (single) into the assert scratch: {pt}"
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

/// Task #45: objective-marker lifecycle. Completing an interact objective kills
/// the `minecraft:interaction` hitbox + wayfinding marker it summoned (both carry
/// `dw_i_<obj>`); completing a reach objective kills its `dw_r_<obj>` marker. The
/// prop BLOCK (lever) is scenery and is NOT removed. A regression PackTest proves
/// the interaction count drops to 0 on completion.
#[test]
fn completed_objectives_despawn_their_summoned_markers() {
    let out = build_showcase();

    // interact obj/door: complete kills its interaction-hitbox tag (dw_i_door);
    // the prop lever setblock stays in the world (affordance scenery, not killed).
    let door = fn_body(&out, "complete_o_door");
    assert!(
        door.contains("kill @e[tag=dw_i_door]"),
        "interact completion despawns its summoned interaction entity: {door}"
    );
    assert!(
        !door.contains("setblock") || !door.contains("minecraft:air"),
        "interact completion does not remove the prop block: {door}"
    );

    // reach obj/shrine: complete kills its end-rod marker tag (dw_r_shrine).
    let shrine = fn_body(&out, "complete_o_shrine");
    assert!(
        shrine.contains("kill @e[tag=dw_r_shrine]"),
        "reach completion despawns its summoned marker: {shrine}"
    );

    // talk-to obj/talk summons no per-objective entity → no cleanup kill.
    let talk = fn_body(&out, "complete_o_talk");
    assert!(
        !talk.contains("kill @e[tag=dw_i_") && !talk.contains("kill @e[tag=dw_r_"),
        "talk-to completion emits no marker cleanup: {talk}"
    );

    // Regression PackTest: after activate + complete, the interaction count is 0.
    let pt = std::str::from_utf8(
        &out["packtest-datapack/data/v04-showcase/test/v04_interact_cleanup.mcfunction"],
    )
    .unwrap();
    assert!(
        pt.contains("assert score #before dw.sys matches 1..")
            && pt.contains("function v04-showcase:complete_o_door")
            && pt.contains("assert score #after dw.sys matches 0"),
        "interact-cleanup PackTest asserts hitbox exists then is gone: {pt}"
    );
}

/// Task #41: every wave mob is summoned onto a distinct, compiler-validated
/// standable cell inside its own wave's area — never inside a block, and never on
/// the blind `+x` line the old emitter used (which could string a flock across a
/// socket seam toward void). Rebuilds the assembled occupancy world the emitter
/// placed mobs over and checks every wave summon coordinate for both the kill-less
/// ambush and the killed-guards wave.
#[test]
fn wave_mobs_land_on_distinct_standable_in_room_cells() {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    // The exact occupancy world the emitter seated mobs over: assembled geometry
    // plus any colliding relight fixtures (spec-0010), matching emit::build.
    let relight = delvewright_compiler::light::relight(
        &plan,
        &structures,
        delvewright_compiler::light::has_night_vision(&campaign),
    );
    let world = delvewright_compiler::nav::World::from_plan_with_extra(
        &plan,
        &structures,
        &relight.extra_solid,
    );

    let out = build_showcase();
    // Both showcase waves resolve to area/keep (wave_area_resolves_from_spawn_site).
    let area = plan
        .areas
        .iter()
        .find(|a| a.area_id == "area/keep")
        .expect("area/keep placed");
    let (lo, hi) = area.bounds();
    for fn_name in ["spawn_guards", "spawn_ambush"] {
        let cells = summon_cells(fn_body(&out, fn_name));
        assert!(
            cells.len() >= 2,
            "{fn_name}: expected the wave's mob summons"
        );
        let uniq: std::collections::BTreeSet<_> = cells.iter().copied().collect();
        assert_eq!(uniq.len(), cells.len(), "{fn_name}: two mobs share a cell");
        for c in &cells {
            assert!(
                world.is_standable(*c),
                "{fn_name}: mob at {c:?} is not on standable footing"
            );
            assert!(
                (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]),
                "{fn_name}: mob at {c:?} spilled beyond area bounds {lo:?}..={hi:?}"
            );
        }
    }
}

/// The `[x, y, z]` cell of every `summon` line in a spawn-function body.
fn summon_cells(body: &str) -> Vec<[i32; 3]> {
    body.lines()
        .filter_map(|l| {
            let t: Vec<&str> = l.split_whitespace().collect();
            if t.first() == Some(&"summon") && t.len() >= 5 {
                Some([t[2].parse().ok()?, t[3].parse().ok()?, t[4].parse().ok()?])
            } else {
                None
            }
        })
        .collect()
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
