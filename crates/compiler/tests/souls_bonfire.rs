//! spec-0016 §1 (souls-mode bonfires) end-to-end tests, driven by the
//! `souls-bonfire` fixture: the v0.6 checkpoint showcase with its
//! `set-checkpoint` replaced by a `bonfire` (with `on_rest`) and its critical
//! wave marked `respawns_on_rest`. A clean build proves the bonfire inherits the
//! DW0315 (no-stranding) and DW0316 (standable placement) obligations — a
//! bonfire IS a checkpoint to those proofs.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

/// Build the fixture. A clean build is itself the DW0315/DW0316 proof for the
/// bonfire (the checkpoint proofs run over `plan.checkpoints`, which a bonfire
/// joins).
fn build_fixture() -> BuildOutput {
    let dir = fixture_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-bonfire parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "souls-bonfire must validate clean: {diags:#?}"
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
    .expect("every emitted command validates (DW0315/DW0316 hold for the bonfire)")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

/// A `bonfire` does NOT move the respawn point when its beat fires — it only
/// ARMS the rest affordance. This is the whole difference from `set-checkpoint`
/// (spec-0016 §1): the checkpoint moves when the party rests.
#[test]
fn bonfire_arms_a_rest_affordance_and_does_not_move_the_checkpoint() {
    let out = build_fixture();
    let all = all_functions(&out);
    // The arming line: a guarded interaction summon, never a bare one.
    assert!(
        all.contains(
            "execute unless entity @e[tag=dw_bonfire_0] run summon minecraft:interaction "
        ),
        "the bonfire beat summons its rest affordance, guarded on absence"
    );
    assert!(
        all.contains("Tags:[\"dw_bonfire_0\"]"),
        "the affordance carries the bonfire's stable content-ordered tag"
    );
    // The beat that arms the bonfire must NOT itself carry `spawnpoint @a` — that
    // is the rest function's job.
    let arming = fn_body(&out, "complete_o_slay");
    assert!(
        !arming.contains("spawnpoint @a"),
        "arming a bonfire must not move the respawn point: {arming}"
    );
}

/// Resting moves the party respawn point and mirrors it into `dw:cp` — the same
/// shared contract `set-checkpoint` writes (spec-0013's boundary return reads it).
#[test]
fn resting_moves_the_party_respawn_point() {
    let out = build_fixture();
    let rest = fn_body(&out, "bonfire_rest_0");
    assert!(
        rest.lines().any(|l| l.starts_with("spawnpoint @a ")),
        "rest sets the party spawnpoint: {rest}"
    );
    assert!(
        rest.contains("data modify storage dw:cp pos set value ["),
        "rest mirrors the cell into dw:cp: {rest}"
    );
    assert!(
        rest.contains("scoreboard players set #cp dw.sys 0"),
        "rest marks itself the active checkpoint: {rest}"
    );
}

/// Right-clicking a bonfire opens a CHOICE, never an immediate rest (owner
/// ruling 2026-08-03 — the campfire must be a real interaction). The click is
/// picked up by the vanilla `player_interacted_with_entity` advancement, which is
/// what makes `@s` the clicking player and therefore what makes a `dialog show`
/// possible at all; the advancement revokes itself so a bonfire is re-openable
/// forever (a rest point is used, never consumed).
#[test]
fn right_click_opens_the_choice_and_the_bonfire_stays_reusable() {
    let out = build_fixture();
    let adv = std::str::from_utf8(
        out.get(&format!("datapack/data/{NS}/advancement/bf_0.json"))
            .expect("one advancement per bonfire"),
    )
    .unwrap();
    assert!(
        adv.contains("minecraft:player_interacted_with_entity")
            && adv.contains("dw_bonfire_0")
            && adv.contains(&format!("{NS}:bonfire_open_0")),
        "the bonfire's right-click runs its opener as the clicking player: {adv}"
    );
    let open = fn_body(&out, "bonfire_open_0");
    assert!(
        open.contains(&format!("advancement revoke @s only {NS}:bf_0")),
        "the opener re-arms itself, so a bonfire may be visited again: {open}"
    );
    assert!(
        open.contains(&format!("dialog show @s {NS}:bonfire_0")),
        "the click shows the two-option dialog, not a rest: {open}"
    );
    // The answer channel is a TRIGGER: a dialog button runs its command as the
    // player, and `/trigger` is the only command a non-operator player may run.
    assert!(
        open.contains("scoreboard players reset @s dw.rest")
            && open.contains("scoreboard players enable @s dw.rest"),
        "a stale answer is cleared before the channel is opened: {open}"
    );
    assert!(
        fn_body(&out, "setup").contains("scoreboard objectives add dw.rest trigger"),
        "dw.rest must be a trigger objective"
    );
}

/// EXACTLY two options, in the owner's order: *rest and save*, then *save only*.
/// The dialog is what the ruling is about, so this pins its whole shape.
#[test]
fn the_rest_dialog_offers_exactly_two_options() {
    let out = build_fixture();
    let dialog: serde_json::Value = serde_json::from_slice(
        out.get(&format!("datapack/data/{NS}/dialog/bonfire_0.json"))
            .expect("one rest dialog per bonfire"),
    )
    .unwrap();
    assert_eq!(dialog["type"], "minecraft:multi_action");
    assert_eq!(dialog["title"], "Bonfire");
    let actions = dialog["actions"].as_array().expect("actions is a list");
    assert_eq!(actions.len(), 2, "exactly two options: {dialog:#?}");
    assert_eq!(actions[0]["label"], "Rest and save");
    assert_eq!(actions[0]["action"]["command"], "/trigger dw.rest set 2");
    assert_eq!(actions[1]["label"], "Save only");
    assert_eq!(actions[1]["action"]["command"], "/trigger dw.rest set 1");
    // Both labels are captions, not sentences (#215's fixed-width button rule).
    for a in actions {
        let label = a["label"].as_str().unwrap();
        assert!(label.chars().count() <= 20, "label too wide: `{label}`");
    }
}

/// The tick turns each answer into the chosen function, per bonfire. `dw.rest_at`
/// is what keeps a campaign with several bonfires from routing every answer to
/// the first one.
#[test]
fn the_two_answers_dispatch_to_two_different_functions() {
    let out = build_fixture();
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute as @a[scores={{dw.rest=1,dw.rest_at=0}}] run function {NS}:bonfire_pick_save_0"
        )),
        "answer 1 = save only: {tick}"
    );
    assert!(
        tick.contains(&format!(
            "execute as @a[scores={{dw.rest=2,dw.rest_at=0}}] run function {NS}:bonfire_pick_rest_0"
        )),
        "answer 2 = rest and save: {tick}"
    );
    // No one-shot sentinel guards either dispatch: a bonfire is rested at many
    // times over a delve (contrast the one-shot `#trapdis_<id>`).
    assert!(
        !tick.contains("unless score #bonfire_0"),
        "resting must not be one-shot: {tick}"
    );
}

/// **Save only sets the checkpoint. Nothing else.** (Owner ruling, verbatim.)
/// This is the assertion that keeps the cheap option from quietly growing a
/// heal, a re-seat or an `on_rest` beat.
#[test]
fn save_only_is_the_checkpoint_and_nothing_else() {
    let out = build_fixture();
    let save = fn_body(&out, "bonfire_save_0");
    let rest = fn_body(&out, "bonfire_rest_0");
    assert_eq!(
        save.lines().collect::<Vec<_>>(),
        vec![
            "spawnpoint @a 44 65 2",
            "data modify storage dw:cp pos set value [44, 65, 2]",
            "scoreboard players set #cp dw.sys 0",
        ],
        "save-only is exactly the three checkpoint lines: {save}"
    );
    // Everything a rest adds on top is genuinely absent from save-only.
    for extra in [
        "wave_reseat_guards",
        "You rest at the shrine fire.",
        "bonfire_restore",
    ] {
        assert!(!save.contains(extra), "save-only must not {extra}: {save}");
    }
    assert!(
        rest.contains("wave_reseat_guards") && rest.contains("You rest at the shrine fire."),
        "…while the rest path still does all of it: {rest}"
    );
    let pick = fn_body(&out, "bonfire_pick_save_0");
    assert!(
        pick.contains("scoreboard players reset @s dw.rest")
            && pick.contains(&format!("function {NS}:bonfire_save_0")),
        "the save-only pick consumes the answer and saves: {pick}"
    );
}

/// **Rest = full restore + the party-wide save.** Healing and feeding are
/// `instant_health`/`saturation` because vanilla exposes no `/health` or `/food`
/// command and `/data merge entity` refuses players — those two effects ARE the
/// primitive. Curing is enumerated rather than `effect clear @s`, which would
/// also strip the per-area night-vision mitigation clock and any beneficial
/// effect the story granted.
#[test]
fn rest_restores_the_player_then_saves_the_party() {
    let out = build_fixture();
    let pick = fn_body(&out, "bonfire_pick_rest_0");
    assert_eq!(
        pick.lines().collect::<Vec<_>>(),
        vec![
            "scoreboard players reset @s dw.rest",
            &format!("function {NS}:bonfire_restore"),
            &format!("function {NS}:bonfire_rest_0"),
        ],
        "restore the resting player, then run the party-wide rest: {pick}"
    );
    let restore = fn_body(&out, "bonfire_restore");
    assert!(
        restore.starts_with(
            "effect give @s minecraft:instant_health 1 9 true\n\
             effect give @s minecraft:saturation 1 9 true\n"
        ),
        "health and hunger first: {restore}"
    );
    for harmful in [
        "minecraft:poison",
        "minecraft:wither",
        "minecraft:blindness",
    ] {
        assert!(
            restore.contains(&format!("effect clear @s {harmful}")),
            "a rest cures {harmful}: {restore}"
        );
    }
    assert!(
        !restore.contains("effect clear @s\n") && !restore.contains("minecraft:night_vision"),
        "a rest must never blanket-clear effects (it would strip the area \
         night-vision mitigation): {restore}"
    );
    assert!(
        restore
            .trim_end()
            .ends_with(&format!("function {NS}:bonfire_flask")),
        "and it refills the flask last: {restore}"
    );
}

/// The flask: resting replenishes the resting player's OWN class kit entry to its
/// declared count. `clear` + `give` rather than `item replace`, because a kit item
/// has no fixed slot — and the class is read off the `dw_class_<class>` tag the
/// class apply adds, which is emitted only for a campaign that declares a flask.
#[test]
fn resting_replenishes_the_flask_to_its_declared_count() {
    let out = build_fixture();
    let flask = fn_body(&out, "bonfire_flask");
    assert_eq!(
        flask.lines().collect::<Vec<_>>(),
        vec![
            "execute if entity @s[tag=dw_class_warden] run clear @s minecraft:potion",
            "execute if entity @s[tag=dw_class_warden] run give @s \
             minecraft:potion[custom_name={\"italic\":false,\"text\":\"Ashen Flask\"}] 3",
        ],
        "the flask is cleared and re-given at the declared count: {flask}"
    );
    assert!(
        fn_body(&out, "class_apply_warden").contains("tag @s add dw_class_warden"),
        "taking the class records which flask is yours"
    );
    // Death is a rest's twin (spec-0016 §1): vanilla already returns the dead
    // player at full health, but not with a full flask, so the respawn path
    // refills it too — otherwise retry costs a second walk to the same fire.
    assert!(
        fn_body(&out, "cp_on_respawn_0").contains(&format!("function {NS}:bonfire_flask")),
        "a respawn at a bonfire refills the flask"
    );
}

/// One authored `on_rest` bundle, two audiences (spec-0018). Resting is a PARTY
/// event dispatched from the tick, so its player-facing effects address `@a` —
/// the party rests together. A respawn belongs to the ONE player who died, so the
/// same bundle addresses `@s` there. Party state (`set-flag`) names no player on
/// either path and fires exactly once.
#[test]
fn on_rest_runs_at_the_right_audience_on_both_paths() {
    let out = build_fixture();
    let rest = fn_body(&out, "bonfire_rest_0");
    let respawn = fn_body(&out, "cp_on_respawn_0");
    assert!(
        rest.contains("tellraw @a {\"text\":\"You rest at the shrine fire.\"}"),
        "the whole party sees the rest: {rest}"
    );
    assert!(
        respawn.contains("tellraw @s {\"text\":\"You rest at the shrine fire.\"}"),
        "only the player who died sees it on the respawn path: {respawn}"
    );
    for (label, body) in [("rest", rest), ("respawn", respawn)] {
        assert!(
            body.contains("scoreboard players set #party dw.f_rested 1"),
            "the on_rest set-flag is party state on the {label} path: {body}"
        );
    }
}

/// A `respawns_on_rest` wave is re-seated on every rest and on every respawn at a
/// bonfire — but only once the party has actually met it (the seated sentinel).
#[test]
fn respawns_on_rest_wave_is_reseated_by_rest_and_respawn() {
    let out = build_fixture();
    let spawn = fn_body(&out, "spawn_guards");
    assert!(
        spawn.contains("scoreboard players set #wseat_guards dw.sys 1"),
        "spawning the wave marks it seated: {spawn}"
    );
    let reseat = fn_body(&out, "wave_reseat_guards");
    assert_eq!(
        reseat.lines().collect::<Vec<_>>(),
        vec![
            "kill @e[tag=dw_wave_guards]",
            &format!("function {NS}:spawn_guards")
        ],
        "the re-seat clears survivors then re-runs the authored spawn"
    );
    let guard = format!(
        "execute if score #wseat_guards dw.sys matches 1 run function {NS}:wave_reseat_guards"
    );
    assert!(
        fn_body(&out, "bonfire_rest_0").contains(&guard),
        "a rest re-seats the wave"
    );
    assert!(
        fn_body(&out, "cp_on_respawn_0").contains(&guard),
        "a respawn at the bonfire re-seats it too"
    );
    // An unmarked wave is never re-seated.
    assert!(
        !all_functions(&out).contains("wave_reseat_ambush"),
        "only a `respawns_on_rest` wave gets a re-seat function"
    );
}

/// The proven path RESTS (bell round-3 finding, 2026-08-03). A bonfire arms an
/// affordance and moves nothing until the party rests — souls-correct, and also
/// invisible to a ladder that walked past every fire without touching one, so
/// die-retry respawned at world spawn instead of at the bonfire it had just
/// passed. Resting is the intended loop, so the exported path performs it, right
/// after the beat that arms the fire.
#[test]
fn the_exported_critical_path_rests_at_each_bonfire() {
    let out = build_fixture();
    let path: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").expect("path emitted")).unwrap();
    let steps = path["steps"].as_array().unwrap();
    let (i, rest) = steps
        .iter()
        .enumerate()
        .find(|(_, s)| s["action"] == "rest")
        .expect("the path rests at the bonfire");
    assert_eq!(rest["bonfire"], 0);
    assert_eq!(rest["anchor"], "anchor/objective");
    assert_eq!(rest["pos"], serde_json::json!([44, 65, 2]));
    // The "rest and save" answer, verbatim — the same chat line the dialog button
    // runs. (The right-click that ENABLES the trigger is the harness's job.)
    assert_eq!(rest["command"], "/trigger dw.rest set 2");
    // Spliced after the arming beat, never before it: the fixture arms the
    // bonfire on `obj/slay`, so the rest follows that kill step.
    assert_eq!(
        steps[i - 1]["objective"],
        "obj/slay",
        "the rest follows the beat that arms the bonfire: {steps:#?}"
    );
    // A path export change only — no OTHER step moved, so every `fire_step` index
    // and every nav proof still sees what it always saw.
    assert_eq!(
        steps.iter().filter(|s| s["action"] == "rest").count(),
        1,
        "one rest per bonfire"
    );
}

/// The generated PackTest suite covers both runtime behaviours the bonfire adds:
/// a rest moves the party checkpoint, and a rest re-seats a met wave (and only a
/// met one). Batch-model compliant: each template clears its own entity/score
/// residue at entry and exit.
#[test]
fn bonfire_runtime_behaviour_is_packtested() {
    let out = build_fixture();
    let rest = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_bonfire_rest.mcfunction"
        ))
        .expect("bonfire rest PackTest emitted"),
    )
    .unwrap();
    assert!(
        rest.contains(&format!("function {NS}:bonfire_rest_0")),
        "the template drives the REAL rest function: {rest}"
    );
    assert!(
        rest.contains("data modify storage dw:cp pos set value [0, 0, 0]")
            && rest.matches("assert score").count() == 3,
        "the mirror is scrubbed then asserted on all three axes: {rest}"
    );

    let reseat = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_bonfire_reseat.mcfunction"
        ))
        .expect("bonfire re-seat PackTest emitted"),
    )
    .unwrap();
    assert!(
        reseat.contains("assert score #bu_bfs dw.sys matches 0"),
        "an unmet wave is not conjured by a rest: {reseat}"
    );
    assert!(
        reseat.contains("assert score #br_bfs dw.sys matches 2"),
        "a met, wiped wave stands again at its authored count after a rest: {reseat}"
    );
    // The owner's no-chip-through ruling: a SURVIVOR the party chipped must be
    // removed and replaced, not left standing at whatever health it had. Proven by
    // identity, not arithmetic — the brand cannot survive a re-summon.
    assert!(
        reseat.contains("data modify entity @e[tag=dw_wave_guards,limit=1] Health set value 1.0f")
            && reseat.contains("tag @e[tag=dw_wave_guards,limit=1] add dw_bfchip"),
        "the template chips and brands one survivor: {reseat}"
    );
    assert!(
        reseat.contains("assert score #bp_bfs dw.sys matches 1")
            && reseat.contains("assert score #bc_bfs dw.sys matches 0")
            && reseat.contains("assert score #bf_bfs dw.sys matches 2"),
        "the branded survivor is gone after a rest and the wave stands full: {reseat}"
    );
    assert!(
        reseat
            .trim_end()
            .ends_with("scoreboard players set #wseat_guards dw.sys 0"),
        "the template leaves no residue for the shared batch: {reseat}"
    );
}

/// The owner's ruling is a claim about two options *differing at runtime*, so a
/// live server has to see the difference. Health cannot carry it — PackTest fake
/// players are immune to `/damage`, so a dummy can never be hurt and therefore
/// never be seen to be healed — but the flask can: `clear <player> <item> 0`
/// counts without removing.
#[test]
fn the_two_options_are_packtested_apart() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_bonfire_options.mcfunction"
        ))
        .expect("the option PackTest is emitted"),
    )
    .unwrap();
    assert!(
        t.contains(&format!("run function {NS}:bonfire_pick_save_0"))
            && t.contains(&format!("run function {NS}:bonfire_pick_rest_0")),
        "the template drives BOTH real option functions: {t}"
    );
    // Save-only leaves the single baseline flask alone …
    assert!(
        t.contains("assert score #bo_save dw.sys matches 1"),
        "save-only must not refill the flask: {t}"
    );
    // … and still moves the checkpoint …
    assert!(
        t.contains("assert score #bo_cp dw.sys matches 44"),
        "save-only still saves: {t}"
    );
    // … while the rest brings it back to the declared count.
    assert!(
        t.contains("assert score #bo_rest dw.sys matches 3"),
        "a rest replenishes the flask to its declared count: {t}"
    );
    assert!(
        t.trim_end().ends_with("remove dw_class_warden"),
        "the template leaves no residue for the shared batch: {t}"
    );
}

// --- spec-0021 coexistence with the affordance hardware pass (#192) ---

/// An **equipped actor** and the compiler-owned affordance hardware must
/// coexist: the bonfire is permanent hardware (`retired_by: None`), so `DW0421`
/// treats ANY `kill` reaching its `dw_hw_*` tag in the shipped datapack as an
/// erasure. spec-0021 emission must therefore stay clear of it — actor gear is
/// summon NBT, container fills are `item replace block`, and the only `kill`
/// spec-0021 emits at all lives in `packtest-datapack/`, which the proof
/// deliberately does not judge (ADR-0003).
///
/// This builds the bonfire fixture with an equipped actor spliced in, so both
/// passes run over one datapack. A clean build IS the coexistence proof:
/// `affordance::check` runs on the finished tree and would fail the build.
#[test]
fn an_equipped_actor_coexists_with_affordance_hardware() {
    let dir = fixture_dir();
    let mut loaded = load_campaign_dir(&dir).unwrap();

    // Splice an equipped actor into the fixture's stage-5 doc, in memory.
    let mut q: serde_json::Value = serde_json::from_str(&loaded.raw.quests).unwrap();
    q["content"]["actors"] = serde_json::json!([{
        "id": "actor/elite",
        "entity": "minecraft:wither_skeleton",
        // NOT the bonfire's own anchor: a body standing on an affordance
        // eclipses it (`DW0359`), which is a separate, correct proof.
        "anchor": "anchor/wave",
        "equipment": {
            "head": { "item": "minecraft:netherite_helmet",
                      "enchantments": { "minecraft:protection": 4 } },
            "main_hand": { "item": "minecraft:netherite_sword",
                           "enchantments": { "minecraft:sharpness": 5 } }
        }
    }]);
    loaded.raw.quests = serde_json::to_string(&q).unwrap();

    let campaign = parse_campaign(&loaded.raw).expect("parses with an equipped actor");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "must validate clean: {diags:#?}");

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
    // DW0420/DW0421 run inside `emit::build` over the finished tree.
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("affordance proofs hold with spec-0021 emission present");

    // The gear really is there (so the build proved something, not nothing)…
    let spawn = fn_body(&out, "spawn_actor_elite");
    assert!(
        spawn.contains("minecraft:netherite_sword"),
        "the equipped actor must actually be emitted:\n{spawn}"
    );
    // …and the bonfire's visible hardware survives in the shipped datapack.
    let all = all_functions(&out);
    assert!(
        all.contains("dw_hw_dw_bonfire_"),
        "the bonfire's compiler-owned hardware must still be summoned"
    );
}

/// The seat RETIRES when the fight is over (drowned-bell run seven, 2026-08-03).
///
/// `#wseat_<wave>` was written by `spawn_<wave>` and by nothing else, so a wave
/// stayed seated for the rest of the delve — and every later rest and every later
/// death re-summoned an encounter the party had already beaten. `tick` guards the
/// objective's completion line with `unless score #party <obj> matches 1`, so
/// nothing could ever consume those mobs again; on the bell they were a spec-0016
/// §6 lane squad, and they marched three encounters downstream and killed the bot
/// on a `reach` step. The wave's own `kill` objective is what ends the fight, so
/// its completion is what retires the seat.
#[test]
fn completing_the_kill_objective_retires_the_wave_seat() {
    let out = build_fixture();
    let complete = fn_body(&out, "complete_o_slay");
    assert!(
        complete.contains("scoreboard players set #wseat_guards dw.sys 0"),
        "completing the wave's kill objective retires its seat: {complete}"
    );
    // Order matters: an `on_objective_complete` that re-spawns this very wave must
    // still leave it seated, so the retirement precedes the effect bundle.
    let retire = complete
        .find("scoreboard players set #wseat_guards dw.sys 0")
        .expect("retirement line present");
    let check = complete
        .find(&format!("function {NS}:check_q_"))
        .expect("the quest check closes the function");
    assert!(
        retire < check,
        "the seat retires inside the completion body, before the quest check: {complete}"
    );
    // Nothing else in the delve retires a seat: the seat is the wave's own
    // encounter state, and only spawning or beating that encounter may move it.
    let all = all_functions(&out);
    assert_eq!(
        all.matches("scoreboard players set #wseat_guards dw.sys 0")
            .count(),
        1,
        "exactly one retirement site: {all}"
    );
    assert_eq!(
        all.matches("scoreboard players set #wseat_guards dw.sys 1")
            .count(),
        1,
        "exactly one seating site (`spawn_guards`): {all}"
    );
}

/// The DEATH path owed the same proof the rest path already had. This template
/// drives the real chain from the only thing the engine sees as a death — the
/// `dw.deaths`/`dw.death_ack` edge — through `cp_respawn_check`, and asserts the
/// authored count, zero survivors carried across the death, and a retired seat
/// that stays retired.
#[test]
fn the_death_path_reseat_is_packtested() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_reseat_death.mcfunction"
        ))
        .expect("death-path re-seat PackTest emitted"),
    )
    .unwrap();
    // Driven from the death EDGE, not from the re-seat function it should reach:
    // the whole point is that the death path arrives there at all.
    assert_eq!(
        t.matches("dw.deaths 1").count(),
        3,
        "three deaths are staged (unmet, chipped, retired): {t}"
    );
    assert!(
        t.contains(&format!("run function {NS}:cp_respawn_check"))
            && !t.contains(&format!("function {NS}:cp_on_respawn_")),
        "the template enters at the death-count edge, never at the hook: {t}"
    );
    assert!(
        t.contains("assert score #ru_rsd dw.sys matches 0"),
        "an unmet wave is not conjured by a death: {t}"
    );
    assert!(
        t.contains("tag @e[tag=dw_wave_guards] add dw_rsd_life")
            && t.contains("kill @e[tag=dw_wave_guards,limit=1]")
            && t.contains("assert score #rc_rsd dw.sys matches 1"),
        "this life's mobs are branded and the wave is chipped: {t}"
    );
    assert!(
        t.contains("assert score #rn_rsd dw.sys matches 2"),
        "the wave comes back at its authored count: {t}"
    );
    assert!(
        t.contains(
            "execute store result score #rp_rsd dw.sys if entity \
             @e[tag=dw_wave_guards,tag=dw_rsd_life]"
        ) && t.contains("assert score #rp_rsd dw.sys matches 0"),
        "not one branded mob survives the death: {t}"
    );
    assert!(
        t.contains("assert score #rr_rsd dw.sys matches 0"),
        "a retired seat re-seats nothing on a later death: {t}"
    );
}
