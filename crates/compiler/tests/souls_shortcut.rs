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
        setup.contains("Tags:[\"dw_fixture\",\"dw_sc_inner_door\"]"),
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

// ---------------------------------------------------------------------------
// DW0306 vs a shortcut-owned gate (the half-split heuristic)
// ---------------------------------------------------------------------------
//
// `DW0306` (gate-aware reachability, `plan::check_gate_reachability`) models a
// sealed gate by splitting its carrying piece into two halves along the gate
// plane and connecting them only by the gate cut-edge. That is deliberately
// coarse: an in-piece bypass AROUND the gate is invisible to it, so a far-side
// objective behind a gate no earlier objective opens always reads as a deadlock.
//
// A souls shortcut is exactly that shape — the unlock sits on the FAR side, so a
// plain `open-gate` reward would be self-deadlocking by construction. Shortcuts
// are exempt from the heuristic **by construction**, not by a special case:
// `collect_open_gate_anchors` builds the heuristic's gate set from `open-gate`
// effect anchors only, and a `shortcut` gate has no `open-gate` effect. Its
// deadlock obligation is discharged by a strictly stronger proof instead —
// `Plan::build` seals every shortcut gate at step 0, so the cell-level critical
// path proof (`DW0311`) has to find the long route over real geometry.
//
// The three tests below pin all three halves of that argument: the shortcut is
// green, the same geometry as a plain gate is still red, and the exemption is not
// a hole.

/// A stage-document mutation: the stage file's stem and the edit to apply to it.
type StagePatch = (&'static str, fn(&mut serde_json::Value));

/// Materialize the `souls-shortcut` fixture into a fresh temp dir, applying each
/// [`StagePatch`] to its stage document.
fn fixture_variant(name: &str, patch: &[StagePatch]) -> std::path::PathBuf {
    let src = common::compiler_fixtures_dir().join(NS);
    let dst = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, dst.join(path.file_name().unwrap())).unwrap();
    }
    for (stage, mutate) in patch {
        let file = dst.join(format!("{stage}.json"));
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
        mutate(&mut doc);
        std::fs::write(&file, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    }
    dst
}

/// Plan a campaign directory, returning the `DW03xx` plan error if there is one.
fn plan_code(dir: &std::path::Path) -> Result<(), delvewright_dsl::DwCode> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("variant parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    Plan::build(&campaign, &prefabs)
        .map(|_| ())
        .map_err(|e| e.code)
}

/// **Green**: the fixture is a single piece with an internal gate (`anchor/door`)
/// and a `reach-anchor` objective on the FAR side (`anchor/exit`) — the exact
/// shape the `DW0306` half-split heuristic reports as a deadlock — declared as a
/// `shortcut` with a real in-piece long route (the stage-7 `carve` at x=8). It
/// plans and builds clean: no `DW0306`, and `DW0373`/`DW0374` satisfied (the full
/// `emit::build` in `build_fixture` is where those two run).
#[test]
fn a_shortcut_owned_gate_is_not_a_dw0306_deadlock() {
    let dir = common::compiler_fixtures_dir().join(NS);
    assert_eq!(
        plan_code(&dir),
        Ok(()),
        "a shortcut-owned gate must not be reported as a gate deadlock — the \
         shortcut runs its own (stronger) proofs"
    );
    // The whole pipeline, including the DW0373 long-route / DW0374 payoff proofs.
    let _ = build_fixture();
}

/// **Red, unchanged**: the identical geometry with the shortcut replaced by a
/// plain `open-gate` reward on the far-side objective is still `DW0306`. This is
/// what makes the test above non-vacuous — the gate really is in the shape the
/// heuristic rejects — and it proves `DW0306` is not weakened for ordinary gates
/// (see also `cli::gate_deadlock_exits_3_with_dw0305`, untouched).
#[test]
fn the_same_gate_as_a_plain_open_gate_is_still_dw0306() {
    let dir = fixture_variant(
        "shortcut-as-plain-gate",
        &[("quests", |doc| {
            let c = doc.get_mut("content").unwrap().as_object_mut().unwrap();
            c.remove("shortcuts");
            c["quests"][0]["on_objective_complete"] = serde_json::json!({
                "obj/exit": [{ "type": "open-gate", "anchor": "anchor/door" }]
            });
        })],
    );
    assert_eq!(
        plan_code(&dir),
        Err(delvewright_compiler::plan::DW_GATE_DEADLOCK),
        "an ORDINARY gate whose only opener is the far-side objective itself must \
         still be a DW0306 deadlock"
    );
}

/// **The exemption is not a hole**: delete the long route (empty the stage-7 edit
/// batches) and the shortcut fixture fails again — at `DW0311`, the cell-level
/// critical-path walkability proof, because `Plan::build` seals every shortcut
/// gate at step 0. A shortcut gate is therefore proven MORE strictly than the
/// piece-graph heuristic would manage, not less.
#[test]
fn a_shortcut_with_no_long_route_is_rejected_by_the_critical_path_proof() {
    let dir = fixture_variant(
        "shortcut-without-long-route",
        &[("world-edits", |doc| {
            doc["content"]["batches"] = serde_json::json!([]);
        })],
    );
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("variant parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("planning still succeeds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let err = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect_err("with the bypass carved away the gate is the ONLY route — must fail");
    let emit::BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic, got a command-validation failure");
    };
    assert_eq!(
        code, "DW0311",
        "the sealed shortcut gate must break the critical path: {message}"
    );
}

// ---------------------------------------------------------------------------
// Affordance hardware — the drowned-bell soft-lock (DW0420 / DW0421)
// ---------------------------------------------------------------------------

/// **The unlock is VISIBLE.** Reproduced live on 1.21.11 during the drowned-bell
/// playtest: the unlock cell was bare air holding one `minecraft:interaction`,
/// an invisible entity, and the only thing the player could see there belonged
/// to an unrelated `reach-anchor` objective that killed its own marker on
/// completion. The lever appeared to vanish; the delve soft-locked.
///
/// The compiler now owns the affordance's visibility outright, rather than
/// hoping the tileset dressed the cell.
#[test]
fn the_unlock_affordance_has_visible_hardware_at_its_own_cell() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup_finish");

    let hitbox = setup
        .lines()
        .find(|l| l.contains("summon minecraft:interaction") && l.contains("dw_sc_"))
        .expect("the unlock hitbox is summoned");
    let hardware = setup
        .lines()
        .find(|l| l.contains("summon minecraft:item_display") && l.contains("dw_hw_dw_sc_"))
        .expect("…and so is its visible hardware — an invisible affordance is a soft-lock");

    // Same cell: hardware anywhere else marks the wrong thing.
    let cell = |line: &str| {
        line.split_whitespace()
            .skip(2)
            .take(3)
            .collect::<Vec<_>>()
            .join(" ")
    };
    assert_eq!(
        cell(hitbox),
        cell(hardware),
        "hardware stands exactly where the player must click:\n{hitbox}\n{hardware}"
    );
    assert!(
        hardware.contains("Glowing:1b"),
        "the hardware is findable in a dark undercroft: {hardware}"
    );
}

/// **Only the shortcut's own opening may retire its hardware.** The bar is
/// thrown, the door is open, the lever is spent — and nothing else in the
/// datapack is allowed to reach it. This is the erasure half of the defect: an
/// unrelated `kill` matching the hardware tag would leave a live affordance
/// invisible again.
#[test]
fn only_the_open_retires_the_unlock_hardware() {
    let out = build_fixture();
    for (path, bytes) in &out {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        for line in body.lines().filter(|l| l.contains("dw_hw_dw_sc_")) {
            if !line.contains("kill @") {
                continue;
            }
            assert!(
                path.contains("/shortcut_open_"),
                "only the shortcut's own open may retire its hardware; `{path}` \
                 does it too: {line}"
            );
        }
    }
}
