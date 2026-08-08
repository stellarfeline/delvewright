//! task #184 — a `timed-gate` `disarm`: the readable → avoidable → **disable-able**
//! third rung (souls dossier §5.2). Driven by the `souls-timed-gate-disarm`
//! fixture: the `souls-timed-gate` portcullis (60 open / 40 closed, `crush: true`)
//! given a jam lever, with the same stage-7 `carve` bypass that makes the lever
//! reachable while the gate is shut — which is what `DW0393` proves.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-timed-gate-disarm";

fn build_fixture() -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-timed-gate-disarm parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.is_empty(),
        "souls-timed-gate-disarm must validate clean: {diags:#?}"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates; DW0378/DW0388/DW0393/DW0420 all hold")
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The clock's ping-pong gains **one guard and nothing else**. The open's `fill`
/// stays unguarded on purpose: a jam landing while the gate is shut leaves one
/// open already in flight, and that open is what parks the portcullis in its
/// resting position — suppressing it would freeze the gate CLOSED, the opposite
/// of a disarm.
#[test]
fn the_clock_lines_are_guarded_by_the_jam_sentinel() {
    let out = build_fixture();
    assert_eq!(
        fn_body(&out, "tgate_open_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars",
            &format!(
                "execute unless score #tgdis_inner_door dw.sys matches 1 run schedule function \
                 {NS}:tgate_close_inner_door 60t"
            ),
        ],
        "opening is unconditional; only the next hop of the clock is guarded"
    );
    assert_eq!(
        fn_body(&out, "tgate_close_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "execute unless score #tgdis_inner_door dw.sys matches 1 as \
             @a[x=4,dx=1,y=65,dy=2,z=6,dz=0,tag=!dw_cutscene] run damage @s 1000 minecraft:generic",
            "execute unless score #tgdis_inner_door dw.sys matches 1 run fill 4 65 6 5 67 6 \
             minecraft:iron_bars",
            &format!(
                "execute unless score #tgdis_inner_door dw.sys matches 1 run schedule function \
                 {NS}:tgate_open_inner_door 40t"
            ),
        ],
        "every line of the closing half — judgement, seal and next hop — is inside the guard"
    );
}

/// **A disarmed gate can never crush.** The judgement is not a separate rule with
/// its own condition: it lives *inside* the suppressed clock, on the very line the
/// guard covers, so there is no closing tick left to be caught by. Asserted
/// structurally (the damage command is only ever reachable through the guard)
/// rather than by reading the sentinel, because the structural form is what makes
/// it impossible to regress by adding a fourth line later.
#[test]
fn a_disarmed_gate_can_never_crush() {
    let out = build_fixture();
    for (path, bytes) in &out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        for line in body.lines() {
            if !line.contains("damage @s") {
                continue;
            }
            assert!(
                line.starts_with("execute unless score #tgdis_inner_door dw.sys matches 1 "),
                "every crush command must sit behind the jam guard — found `{line}` in {path}"
            );
        }
    }
}

/// The jam function itself, and the order that IS the semantics: latch, raise the
/// party flag, clear the span once (the portcullis comes to rest OPEN), retire the
/// affordance's visible hardware.
#[test]
fn the_jam_latches_flags_opens_and_retires_its_hardware() {
    let out = build_fixture();
    assert_eq!(
        fn_body(&out, "tgate_disarm_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "scoreboard players set #tgdis_inner_door dw.sys 1",
            "scoreboard players set #party dw.f_portcullis_jammed 1",
            "fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars",
            "kill @e[tag=dw_hw_dw_tgdis_inner_door]",
        ],
    );
    // No emitted function anywhere re-arms the clock: nothing but the jam itself
    // ever clears the sentinel, and nothing re-schedules the close outside the
    // guard. Permanence is structural, exactly as a shortcut's is.
    for (path, bytes) in &out {
        if !(path.starts_with("datapack/") && path.ends_with(".mcfunction")) {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap();
        assert!(
            !body.contains("scoreboard players set #tgdis_inner_door dw.sys 0"),
            "nothing may un-latch the jam sentinel: {path}"
        );
    }
}

/// The affordance is a right-click target **plus compiler-owned visible
/// hardware** — the `DW0420` obligation a trap disarm and a shortcut unlock
/// already carry. A bare `minecraft:interaction` is a lever nobody can see, which
/// is the drowned-bell soft-lock; the whole build fails rather than shipping one,
/// so reaching this assertion at all is the proof.
#[test]
fn the_jam_lever_is_summoned_with_visible_hardware() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains("summon minecraft:interaction") && setup.contains("\"dw_tgdis_inner_door\""),
        "the interaction hitbox is summoned: {setup}"
    );
    assert!(
        setup.contains("summon minecraft:item_display")
            && setup.contains("\"dw_hw_dw_tgdis_inner_door\""),
        "…and the visible hardware paired with it (DW0420): {setup}"
    );
    // Detection is the same one-shot poll a shortcut/trap disarm uses.
    let tick = fn_body(&out, "tick");
    assert!(
        tick.contains(&format!(
            "execute unless score #tgdis_inner_door dw.sys matches 1 if entity \
             @e[tag=dw_tgdis_inner_door,nbt={{interaction:{{}}}}] run function \
             {NS}:tgate_disarm_inner_door"
        )),
        "the jam fires once, off the interaction primitive: {tick}"
    );
    // The clock itself still costs nothing per tick.
    assert!(
        !tick.contains("tgate_open_") && !tick.contains("tgate_close_"),
        "a timed gate costs nothing per tick, disarmable or not: {tick}"
    );
}

/// The observability proof (`DW0388`) treats a disarm-capable gate **identically**:
/// observability is about the pre-disarm read — the party has to be able to watch
/// the clock in order to decide the jam is worth the walk. The gate is still a
/// judged hazard, so it still needs its watch bay.
#[test]
fn a_disarmable_gate_is_still_a_hazard_for_the_observability_proof() {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    assert!(
        plan.timed_gates[0].disarm.is_some(),
        "the fixture's gate declares a jam lever"
    );
    assert_eq!(
        delvewright_compiler::nav::timed_hazards(&plan).len(),
        1,
        "a disarmable gate is still a timed hazard under proof (DW0388)"
    );
}

/// The generated PackTest drives the REAL jam and then the REAL closing function
/// across several former cycle boundaries: each is exactly what the schedule would
/// have re-entered, and each must leave the span air. Without the guard the first
/// one re-seals.
#[test]
fn post_disarm_permanence_is_packtested_across_cycle_boundaries() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_timed_gate_disarm.mcfunction"
        ))
        .expect("disarm PackTest emitted when the gate declares one"),
    )
    .unwrap();
    assert!(
        t.contains(&format!("function {NS}:tgate_disarm_inner_door")),
        "it pulls the real lever: {t}"
    );
    // Three former boundaries, each asserted open — and the `#tgd_c<n>` assertion
    // sits IMMEDIATELY after its close, with no intervening open. Measured: a
    // template that asserted after the following open passed against a
    // deliberately unguarded build, because the open clears the span either way.
    for n in 1..=3 {
        let probe = format!("assert score #tgd_c{n} dw.sys matches 1");
        let at = t
            .find(&probe)
            .unwrap_or_else(|| panic!("missing boundary {n}: {t}"));
        let before = &t[..at];
        let last_close = before
            .rfind(&format!("function {NS}:tgate_close_inner_door"))
            .expect("a close precedes the assertion");
        assert!(
            !before[last_close..].contains(&format!("function {NS}:tgate_open_inner_door")),
            "boundary {n} must be asserted before the open half runs, else the open \
             satisfies it and the guard is untested: {t}"
        );
        assert!(
            t.contains(&format!("assert score #tgd_o{n} dw.sys matches 1")),
            "…and the dead open half is asserted a no-op: {t}"
        );
    }
    assert_eq!(
        t.matches(&format!("function {NS}:tgate_close_inner_door"))
            .count(),
        4,
        "one armed close (which must seal) plus three disarmed ones (which must not): {t}"
    );
    assert!(
        t.contains("assert score #tgd_armed dw.sys matches 1"),
        "the template first proves the clock really was live: {t}"
    );
}

/// A gate with **no** `disarm` emits none of this: no guard, no jam function, no
/// tick line, no PackTest. The `souls-timed-gate` fixture next door is that
/// campaign, and its own tests pin its exact bytes — this asserts the new surface
/// adds nothing to it.
#[test]
fn a_gate_without_a_disarm_is_untouched() {
    let dir = common::compiler_fixtures_dir().join("souls-timed-gate");
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("builds");
    for (path, bytes) in &out {
        let body = std::str::from_utf8(bytes).unwrap_or("");
        assert!(
            !body.contains("tgdis_"),
            "a gate with no disarm emits no jam machinery at all, found some in {path}"
        );
    }
    assert!(
        !out.keys()
            .any(|k| k.contains("souls_timed_gate_disarm.mcfunction")),
        "…and no disarm PackTest"
    );
}

/// Every generated template pins the jam score it depends on, because PackTest
/// shares one server across siblings and orders none of them.
///
/// The motivating failure, seen three times in CI on 2026-08-05 (twice on an
/// unrelated grammar PR, once on a DOCS-ONLY PR whose emitted bytes were
/// identical to main): `souls_timed_gate` failed its re-seal assertion with
/// `Expected #tg_shut dw.sys to match 1, but got 0`. `souls_timed_gate_disarm`
/// ends with the gate DISARMED — that is its whole subject — and `#tgdis_<id>`
/// persists on the shared server, so whichever sibling ran afterwards had its
/// `tgate_close_` swallowed by the clock's own jam guard and read air where it
/// expected stone. Nothing was wrong with the gate; the test inherited state.
///
/// Asserted on the FAMILY, not on the one template that happened to fail: any
/// generated template that calls a guarded clock line must pin the guard first.
#[test]
fn every_generated_template_pins_the_jam_it_could_inherit() {
    let out = build_fixture();
    // Prefix, not the fixture's gate id: the obligation is on the family.
    let pin = "scoreboard players set #tgdis_";

    for name in [
        "souls_timed_gate",
        "souls_timed_gate_crush",
        "souls_timed_gate_disarm",
    ] {
        let path = format!("packtest-datapack/data/{NS}/test/{name}.mcfunction");
        let body = std::str::from_utf8(
            out.get(&path)
                .unwrap_or_else(|| panic!("missing template {path}")),
        )
        .unwrap();
        let pin_at = body
            .find(pin)
            .unwrap_or_else(|| panic!("{name} never pins the jam score it inherits:\n{body}"));

        // The pin is worthless after the fact: it must land before the first call
        // into the clock, which is what the guard swallows.
        if let Some(call_at) = body.find(":tgate_") {
            assert!(
                pin_at < call_at,
                "{name} pins the jam AFTER it has already called the clock:\n{body}"
            );
        }
    }
}
