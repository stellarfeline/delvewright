//! **The motivating scenario: a lift and a recovery stake in one room.**
//!
//! A `teleport` moves ENTITIES, not blocks. The ground under a recovery stake is
//! untouched by a ride; what travels is the marker itself, away from the position
//! the collecting player's ledger recorded. Nothing then reports it: the next
//! tick, `stk_gc_<s>` reads the marker's (new) position, finds no player holding
//! a wager there, and retires it — so the wager is not merely uncollectable, it
//! is deleted.
//!
//! ## Why this could not be a box test
//!
//! `DW0526` is about **footing** and a marker's position is chosen at RUNTIME, so
//! no compile-time geometry test knows where it will be. `DW0542` tests the
//! *cell-keyed* affordance authority, and a stake has no compile-time cell to
//! offer it — inheriting it would have been a green that examined nothing.
//! Both refusals are correct, and the reason they are both correct is the same:
//! **the question is not where this thing is, it is what this thing is.**
//!
//! So the fix is a class carried by the object (`compiler::affordance`,
//! `DW0545`): every entity the engine summons declares whether its position IS
//! engine state (`dw_fixture` — a place) or belongs to a body that carries it
//! (`dw_borne` — an NPC's dialogue hitbox, which must ride whatever its speaker
//! rides). Every selector narrowed by a positional box then excludes the fixture
//! class, and `DW0545` proves both halves over the shipped datapack.
//!
//! ## What lives here and what does not
//!
//! This file is the **compile-time** half: the emission really carries the class
//! and the exclusion, on the fixture that actually pairs a lift with a stake, and
//! the ledger states what it bound to. The **runtime** half cannot live here at
//! all — whether vanilla's `tag=!…` keeps a real entity out of a real `tp`'s
//! reach is not a fact any Rust test can witness — so it is the generated
//! PackTest template this fixture emits, asserted here to exist and to be
//! non-vacuous (it moves a body out of the same box it holds the marker in), and
//! executed by the `packtest` profile.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

/// The fixture's namespace — the emitted function prefix.
const NS: &str = "lift-stake";

/// A private working directory per CALLER — the tests run in parallel threads of
/// one binary, and a shared scratch directory is a race whose symptom is a
/// missing file (an intermittent red, which is a finding, not a re-run).
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build the `lift-stake` fixture at a private path.
fn build(who: &str) -> BuildOutput {
    let src = common::compiler_fixtures_dir().join("lift-stake");
    let dir = tmp(&format!("fixture-class-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the lift-stake fixture parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        !diags
            .iter()
            .any(|d| d.severity == delvewright_dsl::Severity::Error),
        "the fixture must validate clean: {diags:?}"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("the lift-stake plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the lift-stake fixture builds clean")
}

/// One emitted delve function's body.
fn func(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8_lossy(
        out.get(&path)
            .unwrap_or_else(|| panic!("no emitted function `{name}`")),
    )
    .to_string()
}

/// Every shipped delve function body, as one string.
fn all_delve_functions(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(p, b)| format!("### {p}\n{}\n", String::from_utf8_lossy(b)))
        .collect()
}

/// The generated PackTest templates for the fixture class, by path.
fn fixture_templates(out: &BuildOutput) -> Vec<(String, String)> {
    out.iter()
        .filter(|(p, _)| p.starts_with("packtest-datapack/") && p.contains("/test/fixture_"))
        .map(|(p, b)| (p.clone(), String::from_utf8_lossy(b).to_string()))
        .collect()
}

// ------------------------------------------------------------------ the defect --

/// **The motivating scenario, at the emission.** The marker the stake leaves is
/// summoned by `stk_fill_<s>` at a runtime position, and BOTH halves of it — the
/// `minecraft:interaction` the collector right-clicks and the `item_display` the
/// player sees — declare the fixture class. A half that did not would be carried
/// off on the next ride while its twin stayed, which is the drowned-bell shape
/// arriving by teleport.
#[test]
fn the_stake_marker_declares_itself_a_place() {
    let out = build("marker");
    let fill = func(&out, "stk_fill_embers");
    for half in ["minecraft:interaction", "minecraft:item_display"] {
        let line = fill
            .lines()
            .find(|l| l.contains(&format!("summon {half} ")))
            .unwrap_or_else(|| panic!("`stk_fill_embers` summons a {half}:\n{fill}"));
        assert!(
            line.contains("\"dw_fixture\""),
            "a stake marker is a PLACE — its position is what the ledger recorded, so a ride \
             must leave it standing. Emitted: {line}"
        );
    }
}

/// **The other end of the same rule.** Every `teleport` in the fixture excludes
/// the class, so the ride carries the party and leaves the marker.
#[test]
fn every_teleport_in_the_room_leaves_the_marker_where_it_stands() {
    let out = build("teleport");
    let all = all_delve_functions(&out);
    let tps: Vec<&str> = all.lines().filter(|l| l.starts_with("tp @e[x=")).collect();
    assert_eq!(
        tps.len(),
        2,
        "the fixture's two call levers each ride the car once:\n{all}"
    );
    for line in tps {
        assert!(
            line.contains("tag=!dw_fixture"),
            "a lift that carries a recovery stake's marker carries it out from under its own \
             ledger, and `stk_gc_<s>` deletes it on the next tick. Emitted: {line}"
        );
        assert!(
            !line.contains("type=!"),
            "the narrowing must be the CLASS, never a type roster: a type says nothing about \
             whether the thing wearing it is a place or a passenger, and on a moving verb the \
             roster would strip an NPC's dialogue hitbox off its body. Emitted: {line}"
        );
    }
}

/// A lethal volume answers the same question, and answers it the same way. Its
/// five-type roster already happens to cover every fixture in the engine today —
/// which is exactly why it is not the proof: the roster is keyed to what a volume
/// does to an entity, and the day a fixture is summoned as a sixth type it stops
/// covering anything.
#[test]
fn the_other_region_verb_reads_the_same_class() {
    let out = build("lethal");
    // The lift fixture declares no lethal volume, so this states the shared rule
    // over the whole shipped tree rather than over one verb: every box-narrowed
    // entity selector, whatever wrote it, carries the exclusion.
    let all = all_delve_functions(&out);
    let boxed: Vec<&str> = all
        .lines()
        .filter(|l| l.contains("@e[") && l.contains("x=") && l.contains("dx="))
        .collect();
    assert!(
        !boxed.is_empty(),
        "a proof that examined zero region selectors proves nothing:\n{all}"
    );
    for line in boxed {
        assert!(line.contains("tag=!dw_fixture"), "{line}");
    }
}

// ------------------------------------------------------------------ the ledger --

/// The binding ledger exists and states what it examined
/// (`docs/reference/playtest-methodology.md` rule 1). A zero on either count is a
/// finding: no fixtures means every exclusion in the build is inert, and no box
/// selectors means the clause the defect lives in looked at nothing.
#[test]
fn the_fixture_gate_ledger_states_its_binding() {
    let out = build("ledger");
    let raw = out
        .get("validation/fixture-gate.json")
        .expect("every build emits the fixture-class ledger");
    let gate: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(gate["unbound"], false, "{gate}");
    assert!(
        gate["fixtures_declared"].as_u64().unwrap() > 0,
        "an empty class makes every exclusion decorative: {gate}"
    );
    assert_eq!(
        gate["box_selectors_examined"], 2,
        "the fixture's two rides: {gate}"
    );
    assert_eq!(
        gate["packtest_templates"], 2,
        "one runtime template per (teleport × stake) pair — the half no compile-time test can \
         witness: {gate}"
    );
}

// ----------------------------------------------------------------- the runtime --

/// The runtime half is generated, calls the campaign's REAL functions, and
/// **cannot pass by the teleport doing nothing**.
///
/// That last clause is the one worth asserting rather than reading: a template
/// that only checked "the marker is still in the box" would be green on an engine
/// whose teleport was broken outright — a gate that can only fail in the
/// direction that never happens. So it puts a plain body in the same box and
/// requires it to have LEFT.
#[test]
fn the_runtime_template_is_generated_and_not_one_directional() {
    let out = build("packtest");
    let templates = fixture_templates(&out);
    assert_eq!(
        templates.len(),
        2,
        "one per (teleport × stake) pair: {:?}",
        templates.iter().map(|(p, _)| p).collect::<Vec<_>>()
    );
    for (path, body) in &templates {
        // It drives the real emission, never a command it re-typed.
        assert!(
            body.contains(&format!("function {NS}:stk_fill_embers")),
            "{path} must put down a REAL marker:\n{body}"
        );
        assert!(
            body.lines()
                .any(|l| l.starts_with(&format!("function {NS}:teleport_"))),
            "{path} must ride the REAL teleport:\n{body}"
        );
        // Bound before it is judged: the marker really is inside the volume.
        assert!(
            body.contains("#fx_in_") && body.contains("#fx_hw_"),
            "{path} must first assert both halves are inside the box:\n{body}"
        );
        // The passenger really left…
        assert!(
            body.contains("#fx_body_") && body.contains("matches 0"),
            "{path} must assert a body LEFT the box, or a teleport that did nothing would \
             pass:\n{body}"
        );
        // …and the place really stayed.
        assert!(
            body.contains("#fx_stay_") && body.contains("#fx_hwstay_"),
            "{path} must assert both halves of the marker stayed:\n{body}"
        );
    }
}

/// A campaign with a teleport and no stake generates no such template, and one
/// with a stake and no teleport generates none either — the pair is the
/// obligation. `crates/delvec/tests/v10_teleport.rs` and `economy` are those
/// two campaigns, so this states the third case: the fixture that has both emits
/// exactly the pairs it has.
#[test]
fn the_template_binds_to_the_pair_and_not_to_either_half() {
    let out = build("pairs");
    let names: Vec<String> = fixture_templates(&out)
        .into_iter()
        .map(|(p, _)| p.rsplit('/').next().unwrap().to_string())
        .collect();
    assert!(
        names.iter().all(|n| n.ends_with("_embers.mcfunction")),
        "every template names the stake it is about: {names:?}"
    );
}

// ----------------------------------------------------- the effect-root ledger --

/// The effect-root walk publishes its binding as a file, not as a stderr string.
///
/// Most effect-shaped proofs in this compiler are only as good as the roots
/// `for_each_effect_root` reaches, and until `validation/effect-roots.json`
/// existed that number was a **string on stderr** — so nothing downstream could
/// assert a build's effect walk had bound to anything at all. Asserted on
/// `lift-stake` because it is a campaign with a genuinely mixed root profile:
/// some roots carry bundles and some do not, which is exactly the shape a ledger
/// has to be able to report honestly.
#[test]
fn the_effect_root_ledger_is_machine_readable() {
    let out = build("effect-roots");
    let raw = out
        .get("validation/effect-roots.json")
        .expect("every build emits the effect-root binding ledger");
    let gate: serde_json::Value = serde_json::from_slice(raw).unwrap();

    // The walk must enumerate every root it knows about, every time. A smaller
    // number means a root stopped being enumerated, which is the silent defect
    // the single enumeration exists to prevent.
    assert_eq!(
        gate["roots_enumerated"], gate["roots_total"],
        "a walk that enumerated fewer roots than exist proves less than it claims: {gate}"
    );
    assert!(
        gate["bundles"].as_u64().unwrap() > 0,
        "a build whose effect walk reaches zero bundles makes every effect-shaped \
         proof over it vacuous: {gate}"
    );
    assert!(
        gate["effects"].as_u64().unwrap() > 0,
        "bundles with no effects in them is the same vacuity one level down: {gate}"
    );

    // `sites` names every root, including the ones this campaign has none at —
    // a zero that is present and named is a measurement; a zero that is absent
    // is indistinguishable from a root nobody enumerated.
    let sites = gate["sites"].as_object().expect("per-root site counts");
    assert_eq!(
        sites.len() as u64,
        gate["roots_total"].as_u64().unwrap(),
        "every root is named in `sites`, present or empty: {gate}"
    );
    let unbound = gate["unbound_roots"]
        .as_array()
        .expect("unbound_roots is a list");
    assert_eq!(
        unbound.len(),
        sites.values().filter(|v| v.as_u64() == Some(0)).count(),
        "`unbound_roots` must be exactly the roots with no bundles: {gate}"
    );
}
