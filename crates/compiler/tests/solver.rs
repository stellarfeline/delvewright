//! Layout-solver + keep-crawl tests: deterministic multi-piece
//! assembly, the socket seal/clear strategy, inter-area transport, and the
//! `DW03xx` build/solver diagnostics.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::solver::{self, Splitmix64};
use delvewright_dsl::parse_campaign;

use std::path::Path;

fn build_campaign(dir: &Path) -> BuildOutput {
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
    .expect("emission succeeds")
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("missing {path}"))).unwrap()
}

/// keep-crawl assembles a multi-piece pool area, and the build is byte-identical
/// across runs (ADR-0006 determinism gate for the solver).
#[test]
fn keep_crawl_build_is_deterministic() {
    let a = build_campaign(&common::keep_crawl_dir());
    let b = build_campaign(&common::keep_crawl_dir());
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
}

/// The plan places the entry + required pieces + fillers within the DSL bounds,
/// and every emitted command validates against the vendored 1.21.11 tree.
#[test]
fn keep_crawl_layout_stats_and_commands() {
    let loaded = load_campaign_dir(&common::keep_crawl_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();

    // area 0 = single-prefab gatehouse; area 1 = the pool.
    assert_eq!(plan.areas.len(), 2);
    assert_eq!(plan.areas[0].pieces.len(), 1, "gatehouse is single-prefab");
    let keep = &plan.areas[1];
    let n = keep.pieces.len();
    assert!((5..=8).contains(&n), "pool has {n} pieces, expected 5..=8");

    // The required anchor-bearing pieces are present exactly once.
    let prefabs_placed: Vec<&str> = keep.pieces.iter().map(|p| p.prefab_id.as_str()).collect();
    assert_eq!(
        prefabs_placed
            .iter()
            .filter(|p| **p == "prefab/keep-spawn-hall")
            .count(),
        1,
        "exactly one entry piece"
    );
    assert_eq!(
        prefabs_placed
            .iter()
            .filter(|p| **p == "prefab/keep-gate-room")
            .count(),
        1,
        "exactly one gate-room (required by open-gate anchor/gate)"
    );
    assert_eq!(
        prefabs_placed
            .iter()
            .filter(|p| **p == "prefab/keep-shrine")
            .count(),
        1,
        "exactly one shrine (required by reach anchor/objective)"
    );

    // The shrine's objective anchor resolved to absolute coords in area/keep.
    assert!(
        plan.anchors
            .contains_key(&("area/keep".to_string(), "anchor/objective".to_string())),
        "shrine objective anchor resolved in the pool area"
    );

    let tree = CommandTree::v1_21_11();
    let out = build_campaign(&common::keep_crawl_dir());
    assert!(
        emit::validate_emitted(&out, &tree).is_empty(),
        "all emitted keep-crawl commands validate"
    );
}

/// The critical path crosses piece boundaries within the pool area (the money
/// shot) and areas: talk-to lands in the gatehouse, the reach anchor in the keep,
/// and completing the talk objective teleports the player into the keep + opens
/// the gate. All the pieces along the way are placed with `place template`.
#[test]
fn keep_crawl_critical_path_crosses_pieces_and_areas() {
    let out = build_campaign(&common::keep_crawl_dir());
    let cp: serde_json::Value =
        serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    let steps = cp["steps"].as_array().unwrap();
    // select-class, talk-to, reach, assert-complete.
    assert_eq!(steps[1]["action"], "talk-to");
    assert_eq!(steps[2]["action"], "reach");
    let talk_x = steps[1]["pos"][0].as_i64().unwrap();
    let reach_x = steps[2]["pos"][0].as_i64().unwrap();
    // gatehouse is at x≈0, the keep at x≈256 — different areas.
    assert!(
        talk_x < 128 && reach_x >= 128,
        "talk in gatehouse, reach in keep"
    );

    // The talk objective's completion opens the gate and teleports into the keep.
    let complete_talk = text(
        &out,
        "datapack/data/keep-crawl/function/complete_o_talk.mcfunction",
    );
    assert!(
        complete_talk.contains("replace minecraft:iron_bars"),
        "talk opens the gate"
    );
    assert!(
        complete_talk.contains("teleport @s 260 65 4"),
        "talk teleports the player to the keep entry spawn:\n{complete_talk}"
    );

    // place_all places every pool piece; setup_finish clears mated sockets.
    let place_all = text(
        &out,
        "datapack/data/keep-crawl/function/place_all.mcfunction",
    );
    let places = place_all
        .lines()
        .filter(|l| l.starts_with("place template keep-crawl:keep-"))
        .count();
    assert!(places >= 4, "multiple pool pieces placed, saw {places}");
    let finish = text(
        &out,
        "datapack/data/keep-crawl/function/setup_finish.mcfunction",
    );
    assert!(
        finish.contains("minecraft:air"),
        "mated jigsaw sockets cleared to air"
    );
}

/// keep-trial (v0.3): a branching keep exercising every gameplay verb builds
/// clean (all commands validate), assembles ≥7 pieces with a branch, resolves
/// each verb's critical-path step, wires the wave/flag/interact mechanics, and
/// double-builds byte-identically.
#[test]
fn keep_trial_builds_all_verbs_and_is_deterministic() {
    let a = build_campaign(&common::keep_trial_dir());
    let b = build_campaign(&common::keep_trial_dir());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "keep-trial byte mismatch in {path}");
    }

    let tree = CommandTree::v1_21_11();
    assert!(
        emit::validate_emitted(&a, &tree).is_empty(),
        "all emitted keep-trial commands validate"
    );

    // Layout: ≥7 pieces including a branch (tee/cross) and a corner room.
    let loaded = load_campaign_dir(&common::keep_trial_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let keep = &plan.areas[0];
    assert!(
        keep.pieces.len() >= 7,
        "keep-trial has {} pieces",
        keep.pieces.len()
    );
    let ids: Vec<&str> = keep.pieces.iter().map(|p| p.prefab_id.as_str()).collect();
    assert!(
        ids.iter().any(|p| p.contains("tee") || p.contains("cross")),
        "a branch piece is present: {ids:?}"
    );
    assert!(
        ids.contains(&"prefab/keep-room-small-a") && ids.contains(&"prefab/keep-shrine"),
        "both terminals (chest room + shrine) placed: {ids:?}"
    );

    // Critical path carries one step per verb.
    let cp: serde_json::Value =
        serde_json::from_slice(a.get("critical-path.json").unwrap()).unwrap();
    let actions: Vec<&str> = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["action"].as_str().unwrap())
        .collect();
    for verb in ["talk-to", "kill", "collect", "interact", "reach"] {
        assert!(
            actions.contains(&verb),
            "critical path has a {verb} step: {actions:?}"
        );
    }

    // Mechanics wired: wave spawn function + countdown. The interaction entity and
    // collect chest are placed at objective ACTIVATION now (gap 13), not at setup —
    // so late props are not visible/lootable early — and setup_finish no longer
    // carries them.
    let setup = text(
        &a,
        "datapack/data/keep-trial/function/setup_finish.mcfunction",
    );
    // (NPC bodies + their own interaction hitboxes still summon at setup; only the
    // objective props moved — assert on the objective-specific tags/blocks.)
    assert!(
        !setup.contains("dw_i_door") && !setup.contains("minecraft:chest"),
        "collect/interact objective props must not be placed at setup (gap 13)"
    );
    let act_door = text(
        &a,
        "datapack/data/keep-trial/function/activate_o_door.mcfunction",
    );
    assert!(
        act_door.contains("summon minecraft:interaction") && act_door.contains("dw_i_door"),
        "interact hitbox summoned at activation"
    );
    let act_key = text(
        &a,
        "datapack/data/keep-trial/function/activate_o_key.mcfunction",
    );
    assert!(
        act_key.contains("setblock") && act_key.contains("minecraft:chest"),
        "collect chest placed at activation"
    );
    let spawn = text(
        &a,
        "datapack/data/keep-trial/function/spawn_guards.mcfunction",
    );
    assert!(spawn.contains("summon minecraft:zombie") && spawn.contains("dw.wave"));

    // Per-verb PackTests emitted (incl. the gap 9 NPC-summon and gap 13
    // pre-held-collect assertions).
    for t in [
        "verb_kill",
        "verb_collect",
        "verb_interact",
        "verb_flag_gate",
        "npc_summons",
        "collect_preheld",
    ] {
        assert!(
            a.contains_key(&format!(
                "packtest-datapack/data/keep-trial/test/{t}.mcfunction"
            )),
            "PackTest {t} emitted"
        );
    }

    // verb_flag_gate runs on the shared-batch PackTest server (one dummy PER
    // test, all coexisting — round-5 island red): it must pin its own dummy by
    // tag, address it exclusively through the tag (no bare `@p`/`@a` writes a
    // sibling test could satisfy or that could land on a foreign dummy), and
    // actively CLEAR the withheld flag — a sibling template (`verb_interact`)
    // legitimately sets the same flag on `@a`, so "never set" is not 0 here.
    let gate = text(
        &a,
        "packtest-datapack/data/keep-trial/test/verb_flag_gate.mcfunction",
    );
    assert!(
        gate.contains("tag @p add dw_flagtest"),
        "flag-gate test pins its dummy: {gate}"
    );
    assert_eq!(
        gate.matches("@p").count(),
        1,
        "the pin is the only `@p` in the flag-gate test: {gate}"
    );
    assert!(
        !gate.contains("@a "),
        "no bare `@a` writes in the flag-gate test: {gate}"
    );
    let clear = gate
        .lines()
        .position(|l| l.starts_with("scoreboard players set #party dw.f_") && l.ends_with(" 0"))
        .expect("withheld flag is actively cleared to 0 on the party holder (spec-0018)");
    let assert0 = gate
        .lines()
        .position(|l| l.starts_with("assert score") && l.ends_with("matches 0"))
        .expect("gate asserts the objective stays 0");
    assert!(
        clear < assert0,
        "flag cleared before the withheld-phase assert: {gate}"
    );

    // Combat difficulty (peaceful would remove summoned wave mobs).
    let props = text(&a, "server/server.properties");
    assert!(
        props.contains("difficulty=easy"),
        "wave campaign runs non-peaceful"
    );
}

/// keep-vertical (v0.3): a keep spanning ≥2 elevation levels via stair pieces
/// builds clean (every command validates), places ≥1 stair, resolves the finale
/// reach onto a level above spawn, and double-builds byte-identically.
#[test]
fn keep_vertical_builds_vertical_and_is_deterministic() {
    let a = build_campaign(&common::keep_vertical_dir());
    let b = build_campaign(&common::keep_vertical_dir());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "keep-vertical byte mismatch in {path}");
    }
    let tree = CommandTree::v1_21_11();
    assert!(
        emit::validate_emitted(&a, &tree).is_empty(),
        "all emitted keep-vertical commands validate"
    );

    let loaded = load_campaign_dir(&common::keep_vertical_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let keep = &plan.areas[0];

    // A stair piece is placed, and pieces sit on ≥2 distinct floor levels.
    assert!(
        keep.pieces.iter().any(|p| p.prefab_id.contains("stair")),
        "a stair piece is placed"
    );
    let levels: std::collections::BTreeSet<i32> =
        keep.pieces.iter().map(|p| p.bbox().0[1]).collect();
    assert!(
        levels.len() >= 2,
        "layout spans ≥2 elevation levels, saw {levels:?}"
    );

    // The finale reach (shrine) sits on a level above the spawn.
    let cp: serde_json::Value =
        serde_json::from_slice(a.get("critical-path.json").unwrap()).unwrap();
    let steps = cp["steps"].as_array().unwrap();
    let spawn_y = steps
        .iter()
        .find(|s| s["action"] == "talk-to")
        .and_then(|s| s["pos"][1].as_i64())
        .unwrap();
    let reach_y = steps
        .iter()
        .find(|s| s["action"] == "reach")
        .and_then(|s| s["pos"][1].as_i64())
        .unwrap();
    assert!(
        reach_y > spawn_y,
        "finale reach (y={reach_y}) is above the spawn level (y={spawn_y})"
    );
}

/// The M2 dress-rehearsal presentation fixes, all gated on v0.3 so keep-trial
/// exercises them while v0.2 stays byte-identical (asserted separately).
#[test]
fn keep_trial_m2_presentation_fixes() {
    let a = build_campaign(&common::keep_trial_dir());
    let setup = text(
        &a,
        "datapack/data/keep-trial/function/setup_finish.mcfunction",
    );
    let tick = text(&a, "datapack/data/keep-trial/function/tick.mcfunction");

    // Fix 1: CustomName is a plain SNBT string component, not the JSON-string
    // form that renders literally on 1.21.11.
    assert!(
        setup.contains("CustomName:\"The Keeper\""),
        "NPC CustomName is a plain SNBT string"
    );
    assert!(
        !setup.contains("CustomName:'{"),
        "no legacy JSON-string CustomName in a v0.3 build"
    );
    let spawn = text(
        &a,
        "datapack/data/keep-trial/function/spawn_guards.mcfunction",
    );
    assert!(
        spawn.contains("CustomName:\"Keep Guard\""),
        "wave mob CustomName is a plain SNBT string"
    );

    // Fix 3 (+ gap 13): a visible, glowing, non-colliding marker at the interact
    // anchor, named from the objective title — now summoned at ACTIVATION (in
    // activate_o_door), not at setup, so it does not glow next to the player before
    // the objective is active.
    let act_door = text(
        &a,
        "datapack/data/keep-trial/function/activate_o_door.mcfunction",
    );
    assert!(
        act_door.contains("summon minecraft:item_display")
            && act_door.contains("Glowing:1b")
            && act_door.contains("CustomName:\"Unbar the Inner Door\""),
        "interact anchor gets a glowing named item_display marker at activation"
    );
    // Fix 3 (round 2): the reach anchor gets the same glowing marker treatment —
    // a distinct end_rod (vs. the interact lantern), named from the reach title —
    // so the finale altar can't be triggered "by wandering". Also activation-gated.
    let act_shrine = text(
        &a,
        "datapack/data/keep-trial/function/activate_o_shrine.mcfunction",
    );
    assert!(
        act_shrine.contains("item:{id:\"minecraft:end_rod\",count:1}")
            && act_shrine.contains("CustomName:\"Reach the Shrine\""),
        "reach anchor gets a glowing named end_rod marker at activation"
    );

    // Fix 4: activation announce (title + hint + sound), completion feedback, and a
    // finale fanfare.
    let ann = text(
        &a,
        "datapack/data/keep-trial/function/announce_o_key.mcfunction",
    );
    assert!(ann.contains("Take the Old Key") && ann.contains("The key sits in a chest"));
    assert!(ann.contains("playsound minecraft:block.note_block.pling"));
    assert!(
        tick.contains("run function keep-trial:announce_o_key"),
        "tick dispatches the announce"
    );
    let done = text(
        &a,
        "datapack/data/keep-trial/function/complete_o_slay.mcfunction",
    );
    assert!(
        done.contains("Objective complete: ") && done.contains("Clear the Guard"),
        "objective completion is announced"
    );
    assert!(done.contains("playsound minecraft:entity.experience_orb.pickup"));
    let cc = text(
        &a,
        "datapack/data/keep-trial/function/campaign_complete.mcfunction",
    );
    assert!(
        cc.contains("title @a title ") && cc.contains("Delve Complete"),
        "finale shows a title banner to the whole party"
    );
    assert!(cc.contains("playsound minecraft:ui.toast.challenge_complete"));

    // Fix 8: reach-anchor completion is a block region, not a point-radius
    // sphere — and the region is the one the author asked for.
    //
    // This assertion used to spell the region as `dx=2`, i.e. the fixed ±1 cube
    // the M2 repair hard-coded. That constant WAS the defect one layer down: it
    // replaced the authored `radius` rather than putting a floor under it, so the
    // number stopped reaching the datapack while the harness went on reading it
    // and aiming outside the box. `keep-trial` authors `radius: 2`, so the region
    // is now ±2 — strictly larger than the cube this line used to pin, so nothing
    // that passed before can fail now. The sphere half of the fix is untouched.
    // See `tests/reach_completion.rs` for the rule itself.
    assert!(
        tick.contains("dx=4,y=")
            && tick.contains("dz=4] run function keep-trial:complete_o_shrine"),
        "reach objective uses a block region sized by the authored radius"
    );
    assert!(
        !tick.contains("distance=.."),
        "v0.3 reach no longer uses the distance sphere"
    );
}

/// The v0.3-gated fixes do NOT leak into v0.2 emission: hello-world keeps the
/// legacy JSON-string CustomName and the point-radius reach sphere, and never
/// gains announce/marker/fanfare machinery (its byte-identity is separately
/// asserted in cli.rs).
#[test]
fn v02_emission_paths_are_untouched() {
    let out = build_campaign(&common::hello_world_dir());
    let setup = text(
        &out,
        "datapack/data/hello-world/function/setup_finish.mcfunction",
    );
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    assert!(
        setup.contains("CustomName:'{\"text\":\"The Keeper\"}'"),
        "v0.2 keeps the legacy JSON-string CustomName"
    );
    assert!(
        tick.contains("distance=..") && !tick.contains("dx=2"),
        "v0.2 keeps the point-radius reach sphere"
    );
    for (path, bytes) in &out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            let body = std::str::from_utf8(bytes).unwrap();
            assert!(
                !body.contains("playsound")
                    && !body.contains("item_display")
                    && !body.contains("dw.ann_"),
                "no v0.3 feedback machinery leaked into v0.2 at {path}"
            );
        }
    }
}

/// The socket seal strategy: a spine that ends at a through-room leaves an open
/// socket, which is sealed with wall material (`stone_bricks`). Solved directly
/// against the real pool so the wall-seal branch is exercised (keep-crawl's
/// fully-consumed linear chain has no open sockets).
#[test]
fn open_socket_is_sealed_with_wall() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut stream = Splitmix64::new(solver::stream_seed(1, "area/test"));
    // Require only the gate (gate-room, a 2-socket through room). With no dead-end
    // terminal, the spine ends at the gate-room, leaving its far socket open.
    let layout = solver::solve_area(
        &prefabs,
        "pool/stone-keep",
        &["anchor/gate".to_string()],
        3,
        3,
        [0, 64, 0],
        &mut stream,
    )
    .expect("solves");
    assert!(
        layout
            .seals
            .iter()
            .any(|s| s.block == "minecraft:stone_bricks"),
        "an open socket is sealed with stone_bricks"
    );
    assert!(
        layout.seals.iter().any(|s| s.block == "minecraft:air"),
        "mated sockets are cleared to air"
    );
}

/// The same seal strategy, on the assembly that never ran a solver: a
/// **single-prefab area**. Its lone piece has no second piece to mate with, so
/// every connector it declares is unmated by construction and owes a wall fill —
/// "every unmated socket is sealed with wall material" is a property of a PLACED
/// PIECE, not of having run the layout solver.
///
/// Keyed to the solver, it reached only pool areas, and a single-prefab area
/// shipped the connector's 3×3 doorway standing open onto whatever the horizon
/// says lies beyond the piece, with the `minecraft:jigsaw` authoring marker left
/// in its sill as a real block a player can stand on. The seal fill covers that
/// cell, which is why this test asserts containment of the socket rather than a
/// literal box.
///
/// The world-level consequence is `DW0322`: those doorway cells are reachable
/// walkable ground one step from a void drop. That symptom is content-dependent —
/// it needs something to make the doorway reachable — so it is deliberately not
/// what this test measures.
#[test]
fn a_single_prefab_areas_lone_piece_seals_the_sockets_it_cannot_mate() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let meta = prefabs
        .get("prefab/cave-shore")
        .expect("the library carries prefab/cave-shore");
    // Binding count: a library piece that stopped declaring connectors would make
    // every assertion below vacuously true.
    assert!(
        !meta.connectors.is_empty(),
        "this proof binds to a connector-bearing prefab; prefab/cave-shore declares none"
    );
    let connectors = meta.connectors.len();
    let socket_local = meta.connectors[0].local_pos;

    let dir = std::env::temp_dir().join(format!(
        "delvewright-single-prefab-seal-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let world_path = dir.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    world["content"]["areas"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "area/cove",
            "name": "The Cove",
            "prefab": "prefab/cave-shore"
        }));
    std::fs::write(&world_path, serde_json::to_string_pretty(&world).unwrap()).unwrap();

    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");

    let cove = plan
        .areas
        .iter()
        .find(|a| a.area_id == "area/cove")
        .expect("the cove is placed");
    assert_eq!(
        cove.seals.len(),
        connectors,
        "one seal per connector, all unmated: {:?}",
        cove.seals
    );
    let origin = cove.pieces[0].pos;
    let socket = [
        origin[0] + socket_local[0],
        origin[1] + socket_local[1],
        origin[2] + socket_local[2],
    ];
    for seal in &cove.seals {
        assert_eq!(
            seal.block, "minecraft:stone_bricks",
            "an unmated socket is walled, never cleared to air"
        );
    }
    let covers_socket = cove
        .seals
        .iter()
        .any(|s| (0..3).all(|axis| s.from[axis] <= socket[axis] && socket[axis] <= s.to[axis]));
    assert!(
        covers_socket,
        "the wall fill must cover the socket cell {socket:?} — that is where the jigsaw \
         marker stands and where the doorway opens: {:?}",
        cove.seals
    );

    // And it reaches the shipped world, not just the plan. The build also has to
    // SUCCEED: with the doorway open this campaign is `DW0322` — a reachable
    // walkable cell one step from a void drop, which is the finding this seal
    // closes.
    let out = build_campaign(&dir);
    let setup = std::str::from_utf8(
        out.get("datapack/data/hello-world/function/setup_finish.mcfunction")
            .expect("setup_finish is emitted"),
    )
    .unwrap();
    for seal in &cove.seals {
        let line = format!(
            "fill {} {} {} {} {} {} minecraft:stone_bricks",
            seal.from[0], seal.from[1], seal.from[2], seal.to[0], seal.to[1], seal.to[2]
        );
        assert!(
            setup.lines().any(|l| l == line),
            "expected `{line}` in setup_finish:\n{setup}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half, and the reason the change above moves no shipped bytes: a
/// prefab that declares no connector has no socket to seal, so a single-prefab
/// area binding one emits exactly what it did before. Every campaign and fixture
/// in the tree is of that shape.
#[test]
fn a_connectorless_single_prefab_area_still_gets_no_seals() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut examined = 0usize;
    for area in &plan.areas {
        let meta = prefabs.get(&area.pieces[0].prefab_id).unwrap();
        assert!(
            meta.connectors.is_empty(),
            "fixture drift: `{}` now declares connectors, so this is no longer the \
             connectorless case",
            area.pieces[0].prefab_id
        );
        assert!(
            area.seals.is_empty(),
            "no connector, no seal: {:?}",
            area.seals
        );
        examined += 1;
    }
    assert_eq!(examined, 1, "hello-world places exactly one area");
}

/// Branching growth (lifts the old `DW0304` one-terminal limit): requiring two
/// dead-end terminals carried by *distinct* pieces (boss-hall's `anchor/boss` +
/// small-a's `anchor/chest`) places **both** on separate branches off a tee/cross,
/// stays connected, and every required piece appears exactly once. (Two anchors on
/// the *same* piece — e.g. boss-hall's `anchor/boss` + `anchor/objective` — now
/// collapse to one piece via coverage-reuse; see `objective_reuses_boss_hall`.)
#[test]
fn branching_two_terminals_both_placed() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut stream = Splitmix64::new(solver::stream_seed(20260730, "area/keep"));
    let layout = solver::solve_area(
        &prefabs,
        "pool/stone-keep",
        // chest → small-a, boss → boss-hall: two dead-ends on distinct pieces.
        &["anchor/chest".to_string(), "anchor/boss".to_string()],
        7,
        10,
        [0, 64, 0],
        &mut stream,
    )
    .expect("branching layout solves");

    let ids: Vec<&str> = layout.pieces.iter().map(|p| p.prefab_id.as_str()).collect();
    assert_eq!(
        ids.iter()
            .filter(|p| **p == "prefab/keep-room-small-a")
            .count(),
        1,
        "exactly one small-a (chest) terminal"
    );
    assert_eq!(
        ids.iter()
            .filter(|p| **p == "prefab/keep-boss-hall")
            .count(),
        1,
        "exactly one boss-hall terminal"
    );
    // A branch piece (tee or cross, ≥3 sockets) is present to fork the two.
    assert!(
        ids.iter().any(|p| p.contains("tee") || p.contains("cross")),
        "a branch piece forks the terminals: {ids:?}"
    );
    // Connected: every piece except the entry mates ≥1 socket (all sockets start
    // unmated; a mated flag means it attached to the tree).
    for (i, piece) in layout.pieces.iter().enumerate() {
        if i == 0 {
            continue;
        }
        assert!(
            piece.mated.iter().any(|&m| m),
            "piece {} ({}) is disconnected",
            i,
            piece.prefab_id
        );
    }
    assert!((7..=10).contains(&layout.pieces.len()));
}

/// Branching is robust across seeds: a two-terminal layout (small-a chest +
/// boss-hall) solves for every seed in a wide sweep. The bounded retry (largest
/// terminal capped first) fits the 11×13 boss-hall even when a naive greedy pass
/// would fail. Guards against seed-dependent overlap flakiness.
#[test]
fn branching_solves_across_many_seeds() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut failures = 0;
    for seed in 0u64..200 {
        let mut s = Splitmix64::new(solver::stream_seed(seed, "area/keep"));
        let r = solver::solve_area(
            &prefabs,
            "pool/stone-keep",
            &["anchor/chest".to_string(), "anchor/boss".to_string()],
            7,
            9,
            [0, 64, 0],
            &mut s,
        );
        if let Ok(layout) = r {
            let ids: Vec<&str> = layout.pieces.iter().map(|p| p.prefab_id.as_str()).collect();
            assert!(
                ids.contains(&"prefab/keep-room-small-a"),
                "seed {seed}: no small-a"
            );
            assert!(
                ids.contains(&"prefab/keep-boss-hall"),
                "seed {seed}: no boss-hall"
            );
        } else {
            failures += 1;
        }
    }
    assert_eq!(failures, 0, "{failures}/200 seeds failed to solve");
}

/// Branching determinism: same seed → identical branching layout.
#[test]
fn branching_same_seed_same_layout() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let solve = || {
        let mut s = Splitmix64::new(solver::stream_seed(99, "area/keep"));
        solver::solve_area(
            &prefabs,
            "pool/stone-keep",
            &["anchor/chest".to_string(), "anchor/boss".to_string()],
            7,
            10,
            [0, 64, 0],
            &mut s,
        )
        .unwrap()
    };
    let a = solve();
    let b = solve();
    assert_eq!(a.pieces.len(), b.pieces.len());
    for (pa, pb) in a.pieces.iter().zip(&b.pieces) {
        assert_eq!(pa.prefab_id, pb.prefab_id);
        assert_eq!(pa.pos, pb.pos);
        assert_eq!(pa.rotation, pb.rotation);
    }
    assert_eq!(a.seals, b.seals);
}

/// Same seed + same DSL → identical solved layout (the solver-level determinism
/// invariant beneath the byte-identity gate).
#[test]
fn solver_same_seed_same_layout() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let solve = || {
        let mut s = Splitmix64::new(solver::stream_seed(20260730, "area/keep"));
        solver::solve_area(
            &prefabs,
            "pool/stone-keep",
            &["anchor/gate".to_string(), "anchor/objective".to_string()],
            5,
            8,
            [256, 64, 0],
            &mut s,
        )
        .unwrap()
    };
    let a = solve();
    let b = solve();
    assert_eq!(a.pieces.len(), b.pieces.len());
    for (pa, pb) in a.pieces.iter().zip(&b.pieces) {
        assert_eq!(pa.prefab_id, pb.prefab_id);
        assert_eq!(pa.pos, pb.pos);
        assert_eq!(pa.rotation, pb.rotation);
    }
    assert_eq!(a.seals, b.seals);
}

// ---------------------------------------------------------------------------
// M2 vertical-solver: role-aware carrier selection, DW0305, seed-sweep, stairs
// ---------------------------------------------------------------------------

/// The exact required-anchor set the hollow-vigil campaign shape produces in
/// `area/keep` (npc `anchor/exit` on the entry + npc `anchor/keeper-stand`; the
/// kill/collect/interact/reach/open-gate/boss anchors). Used by the acceptance
/// seed-sweep and the coverage/ambiguity tests.
fn hollow_vigil_anchors() -> Vec<String> {
    [
        "anchor/boss",
        "anchor/chest",
        "anchor/door",
        "anchor/exit",
        "anchor/gate",
        "anchor/keeper-stand",
        "anchor/objective",
        "anchor/wave",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Role-aware capping: an NPC anchored to `anchor/exit` (which the entry
/// spawn-hall already provides) must NOT force a second spawn-hall, and
/// `anchor/objective` must resolve to the boss-hall that `anchor/boss` forces
/// (coverage-reuse), NOT pull in a redundant shrine. So exactly one spawn-hall,
/// one boss-hall, and no shrine are placed for the hollow-vigil shape.
#[test]
fn objective_reuses_boss_hall_and_no_duplicate_entry() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut s = Splitmix64::new(solver::stream_seed(7, "area/keep"));
    let layout = solver::solve_area(
        &prefabs,
        "pool/stone-keep",
        &hollow_vigil_anchors(),
        10,
        15,
        [0, 64, 0],
        &mut s,
    )
    .expect("hollow-vigil shape solves");
    let ids: Vec<&str> = layout.pieces.iter().map(|p| p.prefab_id.as_str()).collect();
    assert_eq!(
        ids.iter()
            .filter(|p| **p == "prefab/keep-spawn-hall")
            .count(),
        1,
        "exactly one entry spawn-hall (anchor/exit reuses it), got {ids:?}"
    );
    assert_eq!(
        ids.iter()
            .filter(|p| **p == "prefab/keep-boss-hall")
            .count(),
        1,
        "boss-hall placed once"
    );
    assert_eq!(
        ids.iter().filter(|p| **p == "prefab/keep-shrine").count(),
        0,
        "no redundant shrine — boss-hall covers anchor/objective, got {ids:?}"
    );
}

/// Item 2 acceptance: ≥80% of seeds 1..=40 solve the hollow-vigil shape at pieces
/// {min 10, max 15}. (Measured before this work: 3/40. Prints the count so the
/// sweep number is visible with `--nocapture`.)
#[test]
fn hollow_vigil_seed_sweep_meets_target() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let req = hollow_vigil_anchors();
    let mut ok = 0u32;
    for seed in 1u64..=40 {
        let mut s = Splitmix64::new(solver::stream_seed(seed, "area/keep"));
        if solver::solve_area(
            &prefabs,
            "pool/stone-keep",
            &req,
            10,
            15,
            [0, 64, 0],
            &mut s,
        )
        .is_ok()
        {
            ok += 1;
        }
    }
    println!("hollow-vigil shape {{10,15}}: {ok}/40 seeds solvable");
    assert!(ok >= 32, "only {ok}/40 seeds solvable, need ≥32 (80%)");
}

/// DW0305: a campaign-referenced anchor defined by two *placed* pieces is a hard
/// error. `anchor/npc-stand` is on small-a, small-b and small-c; requiring
/// `anchor/chest` (small-a) + `anchor/wave` (small-b) forces both, and then
/// referencing `anchor/npc-stand` resolves ambiguously across the two.
#[test]
fn ambiguous_anchor_is_dw0305() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut s = Splitmix64::new(solver::stream_seed(1, "area/keep"));
    let err = solver::solve_area(
        &prefabs,
        "pool/stone-keep",
        &[
            "anchor/chest".to_string(),
            "anchor/wave".to_string(),
            "anchor/npc-stand".to_string(),
        ],
        5,
        9,
        [0, 64, 0],
        &mut s,
    )
    .expect_err("ambiguous anchor must fail");
    assert_eq!(err.code, solver::DW_AMBIGUOUS_ANCHOR);
    assert!(
        err.message.contains("anchor/npc-stand"),
        "names the ambiguous anchor: {}",
        err.message
    );
}

/// Vertical growth: a pool containing a stair connector produces a layout spanning
/// ≥2 elevation levels (distinct piece floor `y`s), and stays deterministic.
#[test]
fn vertical_pool_spans_multiple_levels() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let solve = || {
        let mut s = Splitmix64::new(solver::stream_seed(20260731, "area/keep"));
        solver::solve_area(
            &prefabs,
            "pool/vertical-keep",
            &["anchor/objective".to_string(), "anchor/wave".to_string()],
            6,
            9,
            [0, 64, 0],
            &mut s,
        )
        .expect("vertical layout solves")
    };
    let layout = solve();
    assert!(
        layout.pieces.iter().any(|p| p.prefab_id.contains("stair")),
        "a stair piece is placed: {:?}",
        layout
            .pieces
            .iter()
            .map(|p| p.prefab_id.as_str())
            .collect::<Vec<_>>()
    );
    let levels: std::collections::BTreeSet<i32> =
        layout.pieces.iter().map(|p| p.bbox_min[1]).collect();
    assert!(
        levels.len() >= 2,
        "layout spans ≥2 elevation levels, saw {levels:?}"
    );
    let b = solve();
    for (pa, pb) in layout.pieces.iter().zip(&b.pieces) {
        assert_eq!(pa.pos, pb.pos);
        assert_eq!(pa.rotation, pb.rotation);
    }
}

/// `DW0304` — layout infeasible for this pool. The branch-piece case: a pool with
/// no ≥3-socket member cannot fork a trunk, so it can never host two dead-end
/// terminals on distinct pieces. This is a **structural** (seed-independent)
/// failure, which is exactly why rerolling the seed is not a fix (ADR-0006).
///
/// The pool is synthesized in a private prefab copy — the shipped library's
/// `pool/stone-keep` deliberately carries a tee and a cross, so nothing in the real
/// content can reach this branch.
#[test]
fn a_pool_with_no_branch_piece_is_dw0304() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("solver-no-brancher");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let pools_path = dir.join("pools.json");
    let mut pools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pools_path).unwrap()).unwrap();
    pools["pools"]["pool/no-brancher"] = serde_json::json!({
        "members": [
            { "prefab": "prefab/keep-spawn-hall", "weight": 1, "role": "entry" },
            { "prefab": "prefab/keep-corridor-straight", "weight": 4, "role": "connector" },
            { "prefab": "prefab/keep-corridor-corner", "weight": 1, "role": "connector" },
            { "prefab": "prefab/keep-room-small-a", "weight": 1, "role": "room" },
            { "prefab": "prefab/keep-boss-hall", "weight": 1, "role": "terminal" }
        ]
    });
    std::fs::write(&pools_path, serde_json::to_string_pretty(&pools).unwrap()).unwrap();

    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let mut stream = Splitmix64::new(solver::stream_seed(20260730, "area/keep"));
    let err = solver::solve_area(
        &prefabs,
        "pool/no-brancher",
        // Two dead-end terminals on distinct pieces need a fork the pool cannot build.
        &["anchor/chest".to_string(), "anchor/boss".to_string()],
        7,
        10,
        [0, 64, 0],
        &mut stream,
    )
    .expect_err("a branchless pool cannot host two dead-end terminals");
    assert_eq!(err.code, solver::DW_INFEASIBLE);
    assert_eq!(solver::DW_INFEASIBLE, "DW0304");
}
