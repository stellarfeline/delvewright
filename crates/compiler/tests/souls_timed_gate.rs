//! spec-0016 §4 (timed gates) end-to-end tests, driven by the `souls-timed-gate`
//! fixture: hello-world's inner door put on a 60-open / 40-closed clock, with the
//! same stage-7 `carve` bypass the shortcut fixture uses (so sealing the door for
//! half of every cycle never strands the critical path). A clean build is the
//! `DW0378` ≥20%-of-cycle proof on real prefab geometry.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

const NS: &str = "souls-timed-gate";

fn build_fixture() -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-timed-gate parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.is_empty(),
        "souls-timed-gate must validate clean: {diags:#?}"
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
    .expect("every emitted command validates (DW0378 holds on the fixture)")
}

/// The observability proof (spec-0016 §4 addendum, `DW0388`) over the fixture's
/// REAL prefab geometry: the barred door sits at the end of a room the party walks
/// the length of, so a player can stand well back and watch a whole cycle before
/// stepping into the span. The synthetic-world fixtures in `compiler::nav` pin the
/// blind-corner failure and the watch-bay fix cell by cell; this pins that the rule
/// does not fire on a plain, legible piece of shipped level geometry — a proof that
/// reds a normal room would be a proof nobody could author against.
#[test]
fn the_fixture_gate_can_be_watched_before_it_is_entered() {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-timed-gate parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    // The campaign declares a timed gate, so the proof has something to judge …
    assert_eq!(
        delvewright_compiler::nav::timed_hazards(&plan).len(),
        1,
        "the fixture's portcullis is the hazard under proof"
    );
    let (_, warnings) = emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("the build succeeds — DW0388 is not raised at error tier either");
    // … and it passes, at either tier.
    assert!(
        !warnings.iter().any(|d| d.code == "DW0388"),
        "a gate at the end of an open room is observable: {warnings:#?}"
    );
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The clock is a self-sustaining two-function ping-pong: each half does its
/// world edit and schedules the other. No per-tick polling, no state to drift,
/// and `schedule` is replace-mode so it can never double up.
#[test]
fn the_clock_is_a_self_sustaining_ping_pong() {
    let out = build_fixture();
    assert_eq!(
        fn_body(&out, "tgate_open_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            "fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars",
            &format!("schedule function {NS}:tgate_close_inner_door 60t"),
        ],
        "open clears the region with the anchor's declared block, then arms the close"
    );
    assert_eq!(
        fn_body(&out, "tgate_close_inner_door")
            .lines()
            .collect::<Vec<_>>(),
        vec![
            // spec-0016 §4 addendum: the fixture opts into `crush`, so the
            // judgement rides the closing tick — BEFORE the fill, while the
            // victim is still standing in an open gateway rather than already
            // encased (where vanilla suffocation, not the portcullis, would be
            // the thing killing them).
            "execute as @a[x=4,dx=1,y=65,dy=2,z=6,dz=0,tag=!dw_cutscene] run damage @s 1000 minecraft:generic",
            "fill 4 65 6 5 67 6 minecraft:iron_bars",
            &format!("schedule function {NS}:tgate_open_inner_door 40t"),
        ],
        "close judges anyone caught in the region, seals it back, then arms the open"
    );
    // Neither half is on the tick — the chain carries itself.
    let tick = fn_body(&out, "tick");
    assert!(
        !tick.contains("tgate_"),
        "a timed gate costs nothing per tick: {tick}"
    );
}

/// The gate is sealed by the prefab at world-load, so the clock's first act is an
/// OPEN. A `phase` of 0 opens immediately; a larger one holds it shut first.
#[test]
fn setup_starts_the_clock_on_an_open() {
    let out = build_fixture();
    let setup = fn_body(&out, "setup_finish");
    assert!(
        setup.contains(&format!("function {NS}:tgate_open_inner_door")),
        "the clock starts on an open: {setup}"
    );
    assert!(
        !setup.contains("schedule function souls-timed-gate:tgate_open_inner_door"),
        "phase 0 opens immediately rather than scheduling: {setup}"
    );
}

/// The generated PackTest drives both halves of the real clock on a live server
/// and asserts the region's block after each — the machine-checkable half of
/// "a deterministic clock over the gate region".
#[test]
fn clock_runtime_behaviour_is_packtested() {
    let out = build_fixture();
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_timed_gate.mcfunction"
        ))
        .expect("timed-gate PackTest emitted"),
    )
    .unwrap();
    assert!(
        t.contains(&format!("function {NS}:tgate_open_inner_door"))
            && t.contains(&format!("function {NS}:tgate_close_inner_door")),
        "the template drives BOTH halves of the real clock: {t}"
    );
    for score in ["#tg_sealed", "#tg_open", "#tg_shut"] {
        assert!(
            t.contains(&format!("assert score {score} dw.sys matches 1")),
            "missing the {score} assertion: {t}"
        );
    }
}

/// The validation metadata the RUNTIME rung needs (task #81). The static proof
/// (`DW0378`) already shows the window is readable; the harness bot could not act on
/// it, because nothing told it a gate existed — so the portcullis filling mid-approach
/// aborted the pathfinder and failed the leg as if the geometry were broken. The
/// waypoints artifact therefore carries the gate table (region + clock) and marks every
/// leg whose proven route walks through one.
#[test]
fn waypoints_artifact_exports_the_gate_table_and_marks_the_crossing_leg() {
    let out = build_fixture();
    let raw = out
        .get("validation/critical-path-waypoints.json")
        .expect("waypoints artifact emitted (the fixture walks a leg)");
    let v: serde_json::Value = serde_json::from_slice(raw).expect("valid JSON");

    let gates = v["timed_gates"].as_array().expect("gate table exported");
    assert_eq!(gates.len(), 1, "one declared timed gate: {gates:#?}");
    let g = &gates[0];
    assert_eq!(g["id"], "timed-gate/inner-door");
    assert_eq!(g["open_ticks"], 60);
    assert_eq!(g["closed_ticks"], 40);
    assert_eq!(g["phase"], 0);
    assert_eq!(g["block"], "minecraft:iron_bars");
    // task #140: `crush` is part of the exported contract — the harness must know a
    // closing edge kills, or it walks the bot into one blind (the tide-mill death).
    // The fixture opts into crush (see `the_clock_is_a_self_sustaining_ping_pong`).
    assert_eq!(
        g["crush"], true,
        "the fixture's crushing gate exports as such: {g:#?}"
    );
    // Canonical inclusive bbox, min ≤ max componentwise — the harness reads every
    // cell of it to observe the open/closed edge.
    let min = g["region"]["min"].as_array().expect("region min");
    let max = g["region"]["max"].as_array().expect("region max");
    for axis in 0..3 {
        assert!(
            min[axis].as_i64() <= max[axis].as_i64(),
            "region min must not exceed max on axis {axis}: {g:#?}"
        );
    }

    // The fixture's walked leg runs straight through the door column, so it is marked
    // — and the mark names a gate the table declares.
    let legs = v["legs"].as_array().expect("legs");
    let crossing: Vec<&serde_json::Value> = legs
        .iter()
        .filter(|l| l.get("timed_gates").is_some())
        .collect();
    assert!(
        !crossing.is_empty(),
        "the fixture's critical path walks through the timed door: {legs:#?}"
    );
    for leg in crossing {
        assert_eq!(
            leg["timed_gates"],
            serde_json::json!(["timed-gate/inner-door"]),
            "a marked leg names declared gates, in declared order: {leg:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// spec-0016 §4 addendum — the portcullis judgement (`crush`)
// ---------------------------------------------------------------------------

/// Build the fixture with `crush` forced to the given value, so the two
/// emissions can be compared directly.
fn build_with_crush(crush: bool) -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("souls-timed-gate parses");
    campaign.quests.content.timed_gates[0].crush = crush;
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
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
    .expect("both crush settings emit valid commands")
}

/// **`crush` defaults to false and is inert when false.** A campaign authored
/// before the addendum existed must compile to the same bytes it always did, so
/// the ONLY file the flag may touch is the closing half of the clock (plus the
/// PackTest fixtures it adds). Everything else — including the manifest's hashes
/// of every other file — is untouched.
#[test]
fn crush_false_is_inert() {
    let off = build_with_crush(false);
    let on = build_with_crush(true);

    // With the flag off, the closing half is exactly the pre-addendum two lines.
    assert_eq!(
        std::str::from_utf8(
            off.get(&format!(
                "datapack/data/{NS}/function/tgate_close_inner_door.mcfunction"
            ))
            .unwrap()
        )
        .unwrap()
        .lines()
        .collect::<Vec<_>>(),
        vec![
            "fill 4 65 6 5 67 6 minecraft:iron_bars",
            &format!("schedule function {NS}:tgate_open_inner_door 40t"),
        ],
        "a gate that does not opt in emits no damage at all"
    );
    // …and nothing anywhere in the shipped datapack damages a player.
    for (path, bytes) in &off {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            let body = std::str::from_utf8(bytes).unwrap();
            assert!(
                !body.contains("damage @s 1000"),
                "crush:false must emit no crush anywhere, found one in {path}"
            );
        }
    }

    // The two builds differ ONLY in the closing function, the crush PackTests, and
    // the waypoints artifact's exported `crush` fact — task #140: the runtime
    // harness must be TOLD a closing edge kills, or it walks the bot into one blind
    // (the manifest hashes those files, so it differs too — and must).
    let differing: Vec<&String> = off
        .keys()
        .chain(on.keys())
        .filter(|k| off.get(*k) != on.get(*k))
        .collect();
    let expected = [
        format!("datapack/data/{NS}/function/tgate_close_inner_door.mcfunction"),
        format!("packtest-datapack/data/{NS}/test/souls_timed_gate_crush.mcfunction"),
        "validation/critical-path-waypoints.json".to_string(),
        "manifest.json".to_string(),
    ];
    for path in &differing {
        assert!(
            expected.contains(path),
            "crush must not perturb `{path}` — byte-identity for every campaign \
             that does not opt in"
        );
    }
}

/// **`crush` is exported to the runtime rung** (task #140). The first live
/// `crush: true` gate killed the mineflayer bot: the harness's gate machinery was
/// purely reactive (wait for a window only AFTER a hop fails), which is safe when a
/// closing gate merely aborts the path and lethal when it kills. The staged-entry
/// fix needs to know WHICH gates crush — that fact is compiler-owned, so the
/// waypoints artifact carries it (no-hack layering: export the fact, never make the
/// harness infer a lethal mechanic from folklore).
#[test]
fn waypoints_artifact_exports_the_crush_fact() {
    for crush in [false, true] {
        let out = build_with_crush(crush);
        let raw = out
            .get("validation/critical-path-waypoints.json")
            .expect("waypoints artifact emitted");
        let v: serde_json::Value = serde_json::from_slice(raw).expect("valid JSON");
        let gates = v["timed_gates"].as_array().expect("gate table exported");
        assert_eq!(gates.len(), 1);
        assert_eq!(
            gates[0]["crush"],
            serde_json::json!(crush),
            "the exported fact mirrors the plan: {:#?}",
            gates[0]
        );
    }
}

/// The judgement is **region-scoped and lethal**, and it lands before the fill.
/// A crush that ran after the seal would be indistinguishable from suffocation,
/// which is slow, gear-dependent and escapable — the opposite of a portcullis.
#[test]
fn crush_is_region_scoped_lethal_and_precedes_the_seal() {
    let out = build_with_crush(true);
    let body = std::str::from_utf8(
        out.get(&format!(
            "datapack/data/{NS}/function/tgate_close_inner_door.mcfunction"
        ))
        .unwrap(),
    )
    .unwrap();
    let damage = body
        .lines()
        .position(|l| l.contains("damage @s"))
        .expect("the closing tick judges");
    let fill = body
        .lines()
        .position(|l| l.starts_with("fill "))
        .expect("the closing tick seals");
    assert!(damage < fill, "judgement precedes the seal: {body}");

    let line = body.lines().nth(damage).unwrap();
    // The gate region is 4..5 x, 65..67 y, 6 z — the selector spans exactly it.
    assert!(
        line.contains("x=4,dx=1,y=65,dy=2,z=6,dz=0"),
        "the selector covers exactly the gate region: {line}"
    );
    // `/damage` takes ONE entity, so the party form must re-bind through
    // `execute as`, never widen the target to `@a`.
    assert!(
        line.starts_with("execute as @a[") && line.contains("run damage @s "),
        "the party form re-binds rather than widening /damage: {line}"
    );
    // A player watching a cutscene is never harmed by campaign machinery.
    assert!(
        line.contains("tag=!dw_cutscene"),
        "the cutscene guard holds here too: {line}"
    );
}

/// The generated crush PackTest asserts **scoping**, live, on real geometry:
/// the emitted selector holds the player standing in the gate and releases them
/// two blocks clear of it. It cannot assert death — PackTest fake players are
/// immune to `/damage` (measured on the pinned toolserver) — so lethality is
/// pinned by `crush_is_region_scoped_lethal_and_precedes_the_seal` above and was
/// verified end-to-end against a real mineflayer client.
#[test]
fn crush_runtime_scoping_is_packtested() {
    let out = build_with_crush(true);
    let t = std::str::from_utf8(
        out.get(&format!(
            "packtest-datapack/data/{NS}/test/souls_timed_gate_crush.mcfunction"
        ))
        .expect("crush PackTest emitted when the gate opts in"),
    )
    .unwrap();
    // The template drives the REAL clock and tests the REAL selector.
    assert!(
        t.contains(&format!("function {NS}:tgate_open_inner_door")),
        "the template opens the real gate first: {t}"
    );
    assert!(
        t.contains("x=4,dx=1,y=65,dy=2,z=6,dz=0"),
        "it tests the same region selector the closing tick runs: {t}"
    );
    assert!(
        t.contains("assert score #cr_in dw.sys matches 1")
            && t.contains("assert score #cr_out dw.sys matches 0"),
        "both directions are asserted — in the gate, and clear of it: {t}"
    );
    // Sibling templates share ONE world, so the subject must be `@s`, never `@a`.
    assert!(
        !t.contains("if entity @a["),
        "the assertion binds @s so a sibling test's dummy is never counted: {t}"
    );
}
