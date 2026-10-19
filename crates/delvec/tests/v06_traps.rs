//! DSL v0.6 traps (spec-0011) end-to-end. Built on hello-world with the `world`
//! and `quests` stages bumped to 0.6.0 and a `traps[]` section added. The prefab
//! library is copied and `hello-room.json` patched to expose an `anchor/trap`
//! on the walked critical path plus an `anchor/lever` disarm affordance beside
//! it — no island prefab carries trap anchors yet, so the fixture builds the
//! hardware contract locally.
//!
//! Covers: the disarm emission and the detection it guards, the `tnt_explodes`
//! seal (v0.6-gated), the trap-fire PackTest, the DW0342 completability proof
//! (a lethal forced-path trap with no discharge fails the build), and a
//! tripwire trap built from a piece that passed admission.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{DSL_VERSION, parse_campaign, validate_campaign_with};

const NS: &str = "hello-world";

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A private prefab copy whose `hello-room.json` gains an `anchor/trap` (trigger
/// cell [5,1,7] on the spawn→exit path) and an
/// `anchor/lever` disarm affordance at [3,1,6]. The trap stands one cell past
/// the doorway, not in it: the door is a gate region the compiler clears, so a
/// trigger placed inside it would be erased by the open-gate (`DW0917` reads the
/// world that results). The trigger block itself is written by
/// `common::plan_structures_with_trap_triggers`.
fn patched_prefabs(name: &str, trigger_block: Option<&str>) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta
        .get_mut("anchors")
        .and_then(|a| a.as_object_mut())
        .unwrap();
    let mut trap_anchor = serde_json::json!({ "pos": [5, 1, 7] });
    // The trigger hardware the prefab wired onto the trap cell, declared with its
    // blockstate exactly as a gate anchor declares its fill `block`. Only a
    // flag-gated trap needs it (`DW0363`), so it stays optional here.
    if let Some(tb) = trigger_block {
        trap_anchor["trigger_block"] = tb.into();
    }
    anchors.insert("anchor/trap".to_string(), trap_anchor);
    anchors.insert(
        "anchor/lever".to_string(),
        serde_json::json!({ "pos": [3, 1, 6] }),
    );
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn world_v06() -> serde_json::Value {
    serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "world",
        "content": {
            "title": "The Keeper's Door",
            "theme": "A lonely keep at the edge of the moor.",
            "premise": "One locked door stands between you and the road home.",
            "seed": 20260729,
            "target_minutes": 5,
            "time": "noon",
            "weather": "clear",
            "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
        }
    })
}

fn quests_v06(trap: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "quests": [ {
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
                ],
                "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
                "on_complete": [ { "type": "campaign-complete" } ]
            } ],
            "traps": [ trap ]
        }
    })
}

/// Build a hello-world variant with one trap against the patched prefabs.
fn build_with_trap(name: &str, trap: serde_json::Value) -> Result<BuildOutput, BuildFailure> {
    build_with_trap_hw(
        name,
        trap,
        Some("minecraft:oak_pressure_plate[powered=false]"),
    )
}

/// As [`build_with_trap`], with explicit control over the `trigger_block` the
/// prefab metadata declares (`None` = the anchor declares none).
fn build_with_trap_hw(
    name: &str,
    trap: serde_json::Value,
    trigger_block: Option<&str>,
) -> Result<BuildOutput, BuildFailure> {
    build_trap_world(name, trap, trigger_block, true)
}

/// The builder behind both: `place_trigger: false` builds the piece exactly as
/// the library ships it, with no trigger block at the trap's cell.
fn build_trap_world(
    name: &str,
    trap: serde_json::Value,
    trigger_block: Option<&str>,
    place_trigger: bool,
) -> Result<BuildOutput, BuildFailure> {
    let camp_dir = tmp(&format!("{name}-camp"));
    let patch = serde_json::json!({
        "documents": { "world": world_v06(), "quests": quests_v06(trap) }
    });
    common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
    let prefabs_dir = patched_prefabs(&format!("{name}-prefabs"), trigger_block);

    let loaded = load_campaign_dir(&camp_dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "must validate clean: {diags:#?}");

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let structures = if place_trigger {
        common::plan_structures_with_trap_triggers(&plan, &prefabs_dir)
    } else {
        let mut raw: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for piece in &area.pieces {
                for t in &piece.templates {
                    let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                    raw.insert(t.structure_file.clone(), bytes);
                }
            }
        }
        raw
    };
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
}

fn text(out: &BuildOutput, key: &str) -> String {
    String::from_utf8(
        out.get(key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .clone(),
    )
    .unwrap()
}

/// A harmful trap with a disarm builds clean and emits the disarm affordance +
/// function, the detection the disarm latch guards, the v0.6 `tnt_explodes`
/// seal, and a PackTest that fires the trap.
#[test]
fn trap_disarm_and_fire_emit_end_to_end() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "trapped-chest",
        "payload": [ { "type": "damage-players", "amount": 2 } ],
        "lethality": "harmful",
        "disarm": { "via": "anchor/lever", "sets_flag": "flag/darts-off" }
    });
    let out = build_with_trap("trap-clean", trap).expect("harmful trap builds");

    let setup_finish = text(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        setup_finish.contains("dw_trapdis_dart_hall"),
        "disarm affordance summon missing:\n{setup_finish}"
    );
    // The chest is the whole body: no click box stands in its cell to take the
    // click from it, so it opens on every click, before the trap fires and after.
    assert!(
        !out.values()
            .any(|bytes| String::from_utf8_lossy(bytes).contains("dw_trapfire_")),
        "nothing may summon a box over the trapped chest:\n{setup_finish}"
    );
    let adv = text(
        &out,
        &format!("datapack/data/{NS}/advancement/trap_open_dart_hall.json"),
    );
    let adv: serde_json::Value = serde_json::from_str(&adv).unwrap();
    assert_eq!(
        adv["criteria"]["open"]["trigger"], "minecraft:default_block_use",
        "{adv}"
    );
    assert_eq!(
        adv["criteria"]["open"]["conditions"]["location"][0]["predicate"]["block"]["blocks"][0],
        "minecraft:trapped_chest",
        "{adv}"
    );
    assert_eq!(
        adv["rewards"]["function"], "hello-world:trap_open_dart_hall",
        "{adv}"
    );
    let open = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_open_dart_hall.mcfunction"),
    );
    assert_eq!(
        open.lines().map(str::trim).collect::<Vec<_>>(),
        [
            "advancement revoke @s only hello-world:trap_open_dart_hall",
            "scoreboard players set #trapopen_dart_hall dw.sys 1",
        ],
        "{open}"
    );
    assert!(
        !setup_finish.contains("item replace block"),
        "a trap loads no container:\n{setup_finish}"
    );

    // v0.6-gated TNT seal appears in setup.
    let setup = text(
        &out,
        &format!("datapack/data/{NS}/function/setup.mcfunction"),
    );
    assert!(
        setup.contains("gamerule tnt_explodes false"),
        "v0.6 tnt_explodes seal missing:\n{setup}"
    );

    // The disarm function sets the flag and the latch the fire guard reads.
    let disarm = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_disarm_dart_hall.mcfunction"),
    );
    assert!(
        disarm.contains("scoreboard players set #trapdis_dart_hall dw.sys 1"),
        "disarm must set the latch the fire guard reads:\n{disarm}"
    );
    assert!(
        disarm.contains("scoreboard players set #party dw.f_darts_off 1"),
        "disarm must set the flag on the party holder (spec-0018) — one lever, \
         everyone's trap off, and it survives a relog:\n{disarm}"
    );
    let detect = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_detect.mcfunction"),
    );
    assert!(
        detect.contains(
            "execute unless score #trapfire_dart_hall dw.sys matches 1 unless score \
             #trapdis_dart_hall dw.sys matches 1 if score #trapopen_dart_hall dw.sys matches 1"
        ),
        "the detection must stand down once the trap is disarmed:\n{detect}"
    );
    // A `rearm` chest re-arms at the head of the pass, so the pass that fires it
    // leaves the latch set for a reader.
    assert_eq!(
        detect.lines().next().map(str::trim),
        Some("scoreboard players set #trapfire_dart_hall dw.sys 0"),
        "{detect}"
    );
    assert!(
        detect
            .lines()
            .any(|l| l.trim() == "scoreboard players set #trapopen_dart_hall dw.sys 0"),
        "the pass consumes the open it read:\n{detect}"
    );
    let tick = text(
        &out,
        &format!("datapack/data/{NS}/function/tick.mcfunction"),
    );
    assert!(
        tick.lines()
            .any(|l| l.trim() == "function hello-world:trap_detect"),
        "the tick must run the detection:\n{tick}"
    );

    // The PackTest fires the trap through the real detection, disarm unthrown.
    let pt = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_trap_fire_dart_hall.mcfunction"),
    );
    for expected in [
        "function hello-world:trap_fire_dart_hall",
        "scoreboard players set #trapdis_dart_hall dw.sys 0",
        "run advancement grant @s only hello-world:trap_open_dart_hall",
        "function hello-world:trap_detect",
        "assert score #trapfire_dart_hall dw.sys matches 1",
        "minecraft:trapped_chest",
        "assert score #trapbox_dart_hall dw.sys matches 0",
    ] {
        assert!(
            pt.contains(expected),
            "the fire PackTest lacks `{expected}`:\n{pt}"
        );
    }

    // The chest's cell, as the PackTest asserts it, holds no interaction box
    // from any emitter: every box `setup_finish` summons is read for the
    // cells its AABB covers.
    let chest: Vec<i32> = pt
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("assert block ")?
                .strip_suffix(" minecraft:trapped_chest")
                .map(|xyz| xyz.split(' ').map(|v| v.parse().unwrap()).collect())
        })
        .expect("the PackTest asserts the chest's cell");
    let nbt_f = |l: &str, key: &str| -> f64 {
        let at = l.find(&format!("{key}:")).unwrap() + key.len() + 1;
        l[at..].split('f').next().unwrap().parse().unwrap()
    };
    let mut boxes = 0;
    for l in setup_finish.lines().map(str::trim) {
        let Some(rest) = l.strip_prefix("summon minecraft:interaction ") else {
            continue;
        };
        boxes += 1;
        let p: Vec<f64> = rest
            .split(' ')
            .take(3)
            .map(|v| v.parse().unwrap())
            .collect();
        let (w, h) = (nbt_f(l, "width"), nbt_f(l, "height"));
        let covers = |axis: usize, lo: f64, hi: f64| {
            let (c0, c1) = (f64::from(chest[axis]), f64::from(chest[axis]) + 1.0);
            lo < c1 && hi > c0
        };
        assert!(
            !(covers(0, p[0] - w / 2.0, p[0] + w / 2.0)
                && covers(1, p[1], p[1] + h)
                && covers(2, p[2] - w / 2.0, p[2] + w / 2.0)),
            "an interaction box stands in the trapped chest's cell {chest:?}: {l}"
        );
    }
    assert!(boxes > 0, "the scan read no box at all:\n{setup_finish}");

    // Every emitted trap command validates against the vendored 1.21.11 Brigadier
    // tree (the ADR-0011 cross-check CI also runs via mecha).
    let errors = emit::validate_emitted(&out, &CommandTree::v1_21_11());
    assert!(
        errors.is_empty(),
        "emitted trap commands must validate: {errors:#?}"
    );
}

/// A lethal, `rearm` trap on the forced critical path with no disarm fails the
/// build with DW0342 (soft-loop; not avoidable/survivable/disarmable).
#[test]
fn lethal_forced_trap_without_discharge_is_dw0342() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "payload": [ { "type": "damage-players", "amount": 2 } ],
        "lethality": "lethal",
        "reset": "rearm"
    });
    match build_with_trap("trap-lethal", trap) {
        Err(BuildFailure::Diagnostic { code, .. }) => {
            assert_eq!(
                code, "DW0342",
                "expected the trap completability proof to fire"
            )
        }
        other => panic!("expected DW0342, got {other:?}"),
    }
}

/// The same lethal forced trap set to `once` is survivable → builds clean.
#[test]
fn lethal_forced_once_trap_builds() {
    let trap = serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "payload": [ { "type": "damage-players", "amount": 2 } ],
        "lethality": "lethal",
        "reset": "once"
    });
    assert!(
        build_with_trap("trap-once", trap).is_ok(),
        "a once-shot lethal trap is survivable and must build"
    );
}

// --- flag gating: the trap is physically off while the gate is shut -----------

/// A trap gated by `forbids_flags`. The disarm produces the flag, so the flag has
/// a declared producer (`DW0172`) and the gate is drivable end-to-end.
fn gated_trap() -> serde_json::Value {
    serde_json::json!({
        "id": "trap/dart-hall",
        "at": "anchor/trap",
        "trigger": "pressure-plate",
        "payload": [ { "type": "damage-players", "amount": 2 } ],
        "lethality": "harmful",
        "disarm": { "via": "anchor/lever", "sets_flag": "flag/darts-off" },
        "forbids_flags": ["flag/darts-off"]
    })
}

/// `requires_flags`/`forbids_flags` on a trap were populated in the plan and
/// checked by `DW0172`, and then read by **no emission site at all** — the
/// documented "inactive while the flag is set" behaviour did not exist. It is now
/// a physical gate: the trigger block leaves the world and comes back.
#[test]
fn a_flag_gated_trap_removes_and_restores_its_trigger() {
    let out = build_with_trap("trap-gated", gated_trap()).expect("a gated trap builds");

    // The trigger cell is [5,65,7] (local [5,1,7] at base y=64).
    let on = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_on_dart_hall.mcfunction"),
    );
    assert!(
        on.contains("setblock 5 65 7 minecraft:oak_pressure_plate[powered=false]"),
        "opening the gate must restore the AUTHORED trigger, blockstate and all:\n{on}"
    );
    assert!(
        on.contains("scoreboard players set #trapgate_dart_hall dw.sys 1"),
        "opening must flip the hardware sentinel:\n{on}"
    );
    let off = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_off_dart_hall.mcfunction"),
    );
    assert!(
        off.contains("setblock 5 65 7 minecraft:air"),
        "shutting the gate must take the trigger out of the world:\n{off}"
    );

    // The tick drives both directions, keyed on the sentinel so the `setblock`
    // fires on a transition rather than every tick. The clauses live in their
    // own function, which the tick calls, so the PackTest can run exactly them.
    let main_tick = text(
        &out,
        &format!("datapack/data/{NS}/function/tick.mcfunction"),
    );
    assert!(
        main_tick
            .lines()
            .any(|l| l.trim() == "function hello-world:trap_gate_tick"),
        "the tick must run the gate clauses:\n{main_tick}"
    );
    let tick = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_tick.mcfunction"),
    );
    let shut = "execute if score #trapgate_dart_hall dw.sys matches 1 \
                if score #party dw.f_darts_off matches 1 \
                run function hello-world:trap_gate_off_dart_hall";
    let open = "execute unless score #trapgate_dart_hall dw.sys matches 1 \
                unless score #party dw.f_darts_off matches 1 \
                run function hello-world:trap_gate_on_dart_hall";
    for expected in [shut, open] {
        assert!(
            tick.lines().any(|l| l.trim() == expected),
            "the tick must carry `{expected}`:\n{tick}"
        );
    }

    // The world-start seed arms the trap and then reads the gate through the
    // tick's own shutting clauses, so it starts in the state its gate says.
    let setup_finish = text(
        &out,
        &format!("datapack/data/{NS}/function/setup_finish.mcfunction"),
    );
    assert!(
        setup_finish.contains("function hello-world:trap_gate_init"),
        "setup must seed the gate:\n{setup_finish}"
    );
    let init = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_init.mcfunction"),
    );
    let init_lines: Vec<&str> = init.lines().map(str::trim).collect();
    assert_eq!(
        init_lines,
        ["function hello-world:trap_gate_on_dart_hall", shut],
        "the seed arms, then runs the tick's shutting clauses:\n{init}"
    );

    // And the behaviour is asserted in-game, not just in the emitted text.
    assert!(
        out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/v06_trap_gate.mcfunction"
        )),
        "the trap flag-gate PackTest must be emitted"
    );

    let errors = emit::validate_emitted(&out, &CommandTree::v1_21_11());
    assert!(
        errors.is_empty(),
        "emitted trap-gate commands must validate: {errors:#?}"
    );
    assert_flags_read_where_written(&out);
}

/// A `requires_flags` gate starts SHUT — no flag is set at world start — so setup
/// takes the trigger straight back out rather than leaving a live trap for the
/// tick to notice.
#[test]
fn a_requires_flags_gate_starts_shut() {
    let mut trap = gated_trap();
    trap["forbids_flags"] = serde_json::json!([]);
    trap["requires_flags"] = serde_json::json!(["flag/darts-off"]);
    let out = build_with_trap("trap-gated-req", trap).expect("a gated trap builds");
    let init = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_init.mcfunction"),
    );
    assert!(
        init.lines().any(|l| l.trim()
            == "execute if score #trapgate_dart_hall dw.sys matches 1 unless score #party dw.f_darts_off matches 1 run function hello-world:trap_gate_off_dart_hall"),
        "a requires-gate starts shut: the seed must read the unset flag and disarm:\n{init}"
    );

    // ...and it OPENS when the flag is set where the campaign writes it. Flags
    // live on the party holder only; a gate that asks whether some player
    // carries the score never opens.
    let gate_tick = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_gate_tick.mcfunction"),
    );
    let open = "execute unless score #trapgate_dart_hall dw.sys matches 1 \
                if score #party dw.f_darts_off matches 1 \
                run function hello-world:trap_gate_on_dart_hall";
    let shut = "execute if score #trapgate_dart_hall dw.sys matches 1 \
                unless score #party dw.f_darts_off matches 1 \
                run function hello-world:trap_gate_off_dart_hall";
    for expected in [open, shut] {
        assert!(
            gate_tick.lines().any(|l| l.trim() == expected),
            "the gate tick must carry `{expected}`:\n{gate_tick}"
        );
    }

    // The PackTest binds this trap too, drives the flag on the party holder,
    // and runs the real gate clauses rather than the two halves.
    let pt = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_trap_gate.mcfunction"),
    );
    assert!(
        pt.contains("scoreboard players set #party dw.f_darts_off 1")
            && pt.contains("function hello-world:trap_gate_tick")
            && !pt.contains("function hello-world:trap_gate_on_dart_hall"),
        "the gate PackTest must drive `#party` and run `trap_gate_tick`:\n{pt}"
    );
    assert_flags_read_where_written(&out);
}

/// The general form of the trap-gate defect: a flag is written on the party
/// holder and nowhere else, so no emitted command may read a flag objective off
/// a player selector. Such a read matches nobody, whatever the campaign does.
fn assert_flags_read_where_written(out: &BuildOutput) {
    let mut functions = 0;
    for (path, bytes) in out {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        functions += 1;
        let body = std::str::from_utf8(bytes).expect("utf-8 function");
        for line in body.lines() {
            assert!(
                !line.contains("scores={dw.f_"),
                "`{path}` reads a flag off a player selector, but flags are written \
                 only on the party holder:\n{line}"
            );
        }
    }
    assert!(functions > 0, "the scan bound no emitted function");
}

/// An **ungated** trap emits none of the gate machinery, so every campaign that
/// existed before this feature stays byte-identical (ADR-0006).
#[test]
fn an_ungated_trap_emits_no_gate_machinery() {
    let mut trap = gated_trap();
    trap["forbids_flags"] = serde_json::json!([]);
    let out = build_with_trap("trap-ungated", trap).expect("an ungated trap builds");
    let tick = text(
        &out,
        &format!("datapack/data/{NS}/function/tick.mcfunction"),
    );
    assert!(
        !tick.contains("trapgate_"),
        "an ungated trap must not gain a gate clause:\n{tick}"
    );
    assert!(
        !out.keys().any(|k| k.contains("trap_gate_")),
        "an ungated trap must emit no gate functions"
    );
}

/// A flag gate on a trap whose prefab declares no `trigger_block` is `DW0363`:
/// the compiler will not pretend to gate hardware it cannot name.
#[test]
fn a_gated_trap_without_declared_hardware_is_dw0363() {
    match build_with_trap_hw("trap-gated-nohw", gated_trap(), None) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0363", "message was: {message}");
            assert!(
                message.contains("trigger_block"),
                "the diagnostic must name the missing declaration: {message}"
            );
        }
        other => panic!("expected DW0363, got {other:?}"),
    }
}

/// A flag gate on a **trapped-chest** trigger is `DW0363` rather than folklore:
/// the chest is a block entity with an inventory, so removing it destroys state
/// the compiler never authored and cannot put back.
#[test]
fn a_gated_trapped_chest_is_dw0363() {
    let mut trap = gated_trap();
    trap["trigger"] = "trapped-chest".into();
    match build_with_trap_hw(
        "trap-gated-chest",
        trap,
        Some("minecraft:trapped_chest[facing=north]"),
    ) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0363", "message was: {message}");
            assert!(
                message.contains("block entity"),
                "the diagnostic must explain why the chest cannot be gated: {message}"
            );
        }
        other => panic!("expected DW0363, got {other:?}"),
    }
}

/// **The false chest that shipped as empty air** (`DW0917`). A trapped-chest
/// trap on an anchor whose piece places no trapped chest: the compiler's
/// detection is an invisible interaction hitbox, so without this check the build
/// succeeded and the player saw nothing to open. The same trap over the same
/// piece with the chest placed builds.
#[test]
fn a_trapped_chest_trap_with_no_chest_is_dw0917() {
    let trap = serde_json::json!({
        "id": "trap/false-chest",
        "at": "anchor/trap",
        "trigger": "trapped-chest",
        "payload": [ { "type": "narrate", "text": "The lid bites.", "style": "chat" } ],
        "lethality": "nonlethal"
    });
    match build_trap_world("trap-no-chest", trap.clone(), None, false) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0917", "{message}");
            assert!(message.contains("trap/false-chest"), "{message}");
            assert!(message.contains("[5, 65, 7]"), "{message}");
            assert!(message.contains("minecraft:trapped_chest"), "{message}");
        }
        Err(other) => panic!("expected DW0917, got {other:?}"),
        Ok(_) => panic!("a trapped-chest trap over empty air built"),
    }
    build_trap_world("trap-with-chest", trap, None, true).expect("the placed chest builds");
}

/// Every trap-judging check states what it bound to, for each trigger kind: the
/// world half (`validation/trap-binding.json`) and the fire half
/// (`validation/trap-fire.json`) both read the declared trap, and neither is
/// zero.
#[test]
fn each_trigger_kind_is_counted_by_the_trap_binding_ledgers() {
    for kind in ["pressure-plate", "tripwire", "trapped-chest"] {
        let trap = serde_json::json!({
            "id": "trap/counted",
            "at": "anchor/trap",
            "trigger": kind,
            "payload": [ { "type": "damage-players", "amount": 2 } ],
            "lethality": "harmful",
            "disarm": { "via": "anchor/lever", "sets_flag": "flag/counted-off" }
        });
        let out = build_trap_world(&format!("trap-binding-{kind}"), trap, None, true)
            .unwrap_or_else(|e| panic!("{kind}: {e:?}"));
        let world: serde_json::Value =
            serde_json::from_str(&text(&out, "validation/trap-binding.json")).unwrap();
        assert_eq!(world["examined"], 1, "{kind}: {world}");
        assert_eq!(world["traps"], 1, "{kind}");
        for k in ["pressure-plate", "tripwire", "trapped-chest"] {
            assert_eq!(
                world["by_trigger"][k],
                u64::from(k == kind),
                "{kind}: by_trigger[{k}]"
            );
        }
        assert_eq!(world["payload_effects"]["damage-players"], 1, "{kind}");
        assert_eq!(world["disarms"], 1, "{kind}");
        assert_eq!(world["hardware_cells"], 2, "{kind}: trigger + disarm cell");
        let fire: serde_json::Value =
            serde_json::from_str(&text(&out, "validation/trap-fire.json")).unwrap();
        assert_eq!(fire["traps"], 1, "{kind}");
        assert_eq!(fire["tests_emitted"], 1, "{kind}");
        assert_eq!(fire["bodies_judged"], 1, "{kind}");
        assert_eq!(fire["bodies_driven"], 1, "{kind}");
        assert_eq!(fire["examined"], 1, "{kind}");
    }
}

/// The hello room with a tripwire strung across the trap's row, wall to wall:
/// a hook against each wall, facing into the room, and the string between them
/// passing over the trap cell. `wire` false leaves the hooks and takes the
/// string out.
fn strung_room(dir: &Path, wire: bool) -> delvec::admit::structure::Structure {
    use delvec::admit::structure::{PaletteEntry, Structure};
    let path = dir.join("hello-room.nbt");
    let mut s = Structure::read(&std::fs::read(&path).unwrap()).unwrap();
    let (y, z) = (1, 7);
    let open = |s: &Structure, x: i32| {
        s.entry_at([x, y, z])
            .is_none_or(|e| e.name == "minecraft:air")
    };
    let mut west = 5;
    while open(&s, west - 1) {
        west -= 1;
    }
    let mut east = 5;
    while open(&s, east + 1) {
        east += 1;
    }
    assert!(
        west < 5 && east > 5,
        "the trap's row must leave room for a hook either side of the cell: {west}..={east}"
    );
    let hook = |facing| {
        PaletteEntry::with_props(
            "minecraft:tripwire_hook",
            &[
                ("attached", "true"),
                ("facing", facing),
                ("powered", "false"),
            ],
        )
    };
    s.set_cell([west, y, z], hook("east"), None);
    s.set_cell([east, y, z], hook("west"), None);
    for x in west + 1..east {
        let entry = if wire {
            PaletteEntry::with_props(
                "minecraft:tripwire",
                &[
                    ("attached", "true"),
                    ("disarmed", "false"),
                    ("east", "true"),
                    ("north", "false"),
                    ("powered", "false"),
                    ("south", "false"),
                    ("west", "true"),
                ],
            )
        } else {
            PaletteEntry::simple("minecraft:air")
        };
        s.set_cell([x, y, z], entry, None);
    }
    s.prune_palette();
    std::fs::write(&path, s.write()).unwrap();
    s
}

/// **A tripwire trap is built from a piece that passed admission** (`DW0917`
/// against `DW0730`). The piece strings a tripwire across the trap's row; the
/// audit admits it with the default allowlist; the campaign builds from those
/// exact bytes, with nothing written into the trap cell by the test, and emits
/// the trap's detection and its fire PackTest. The same piece with the string
/// taken out and the hooks left is `DW0917`: what the trap needs is the string.
#[test]
fn a_tripwire_trap_builds_from_an_admitted_piece() {
    use delvec::admit::allowlist::Allowlist;
    use delvec::admit::audit::audit;
    let trap = serde_json::json!({
        "id": "trap/wire",
        "at": "anchor/trap",
        "trigger": "tripwire",
        "payload": [ { "type": "damage-players", "amount": 2 } ],
        "lethality": "harmful",
        "reset": "once"
    });
    let build = |name: &str, wire: bool| {
        let camp_dir = tmp(&format!("{name}-camp"));
        let patch = serde_json::json!({
            "documents": { "world": world_v06(), "quests": quests_v06(trap.clone()) }
        });
        common::materialize_from(&common::hello_world_dir(), &patch, &camp_dir);
        let prefabs_dir = patched_prefabs(&format!("{name}-prefabs"), None);
        let piece = strung_room(&prefabs_dir, wire);
        let loaded = load_campaign_dir(&camp_dir).unwrap();
        let campaign = parse_campaign(&loaded.raw).expect("campaign parses");
        let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
        let diags = validate_campaign_with(
            &campaign,
            &FullItemRegistry::v1_21_11(),
            &prefabs,
            &FullEntityRegistry::v1_21_11(),
        );
        assert!(diags.is_empty(), "must validate clean: {diags:#?}");
        let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
        let mut raw: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for p in &area.pieces {
                for t in &p.templates {
                    raw.insert(
                        t.structure_file.clone(),
                        std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap(),
                    );
                }
            }
        }
        let out = emit::build(
            &plan,
            &loaded.inputs,
            &raw,
            &CommandTree::v1_21_11(),
            &prefabs,
            None,
            &BTreeMap::new(),
        );
        (piece, out)
    };

    let (piece, out) = build("trap-strung", true);
    let (rep, _) = audit("hello-room", &piece, &Allowlist::default_building());
    assert!(
        rep.findings.iter().all(|f| f.code != "DW0730"),
        "the strung piece must pass the palette allowlist: {:?}",
        rep.findings
    );
    for id in ["minecraft:tripwire", "minecraft:tripwire_hook"] {
        assert!(
            rep.palette.iter().any(|b| b == id),
            "{id} was not examined: {:?}",
            rep.palette
        );
    }
    let out = out.expect("a tripwire trap over an admitted, strung piece builds");
    let detect = text(
        &out,
        &format!("datapack/data/{NS}/function/trap_detect.mcfunction"),
    );
    assert!(
        detect.contains(
            "execute unless score #trapfire_wire dw.sys matches 1 if entity \
             @a[x=5,dx=0,y=65,dy=0,z=7,dz=0,tag=!dw_cutscene] run function \
             hello-world:trap_fire_wire"
        ),
        "{detect}"
    );
    let pt = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/v06_trap_fire_wire.mcfunction"),
    );
    assert!(
        pt.contains("tp @a[tag=dw_t_trap_wire,limit=1] 5.5 65 7.5"),
        "{pt}"
    );
    assert!(pt.contains("function hello-world:trap_detect"), "{pt}");

    match build("trap-hooks-only", false).1 {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0917", "{message}");
            assert!(message.contains("minecraft:tripwire"), "{message}");
        }
        Err(other) => panic!("expected DW0917, got {other:?}"),
        Ok(_) => panic!("a tripwire trap with no string built"),
    }
}
