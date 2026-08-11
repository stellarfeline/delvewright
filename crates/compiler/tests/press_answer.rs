//! **A press is answered by one mechanism** (DSL v0.11, owner design ruling
//! 2026-08-06).
//!
//! ## The finding
//!
//! The drowned-bell remake's central idiom is the souls shortcut loop: you walk
//! the long way round and open a barred door from the far side. Before you have
//! opened it, pressing that door from the wrong side must answer — *"the door
//! cannot be opened from this side."* It answered nothing, and the gate the party
//! is most likely to press was the one gate in the engine that stayed silent.
//!
//! Task #50 had already given the door a **body** (`crate::wrongside`,
//! `crate::pressable`): a click trigger anchored on the `gate` now rides hitboxes
//! standing in the open air on the sealed side. What it did not give it was an
//! **answer**. That half lived — entirely — inside one effect verb:
//! `close-gate.sealed_hint` had its own hitbox fleet, its own advancement, its
//! own actionbar command and its own baked English, and nothing else in the
//! engine could reach any of it.
//!
//! ## Why the fix is not a field on `Shortcut`
//!
//! CLAUDE.md, on this precise case: *a second bespoke field is the defect, not
//! the fix*. And one layer past that — `EnvTrigger{at, on, effects}` **already
//! is** "give any scene object a click response, and the response is any effect".
//! `sealed_hint` was never a missing feature; it was a private copy of a general
//! one. Two things stopped the general one from saying what the copy said:
//!
//! * `narrate` had no `actionbar` style — the reply strip every compiler-written
//!   line already used, and the only channel a reply belongs on;
//! * a click trigger's bundle was dispatched from the tick with no executor, so
//!   it addressed `@a` and could not answer the one player who pressed.
//!
//! v0.11 adds exactly those two. A press answer is then an ordinary
//! `EnvTrigger{on: use, audience: presser}` carrying an ordinary
//! `narrate{style: actionbar}` — authored by the campaign, or synthesized by the
//! compiler for a sealed body the campaign leaves silent — and the shortcut door
//! is not a special case at all. It is a consumer.
//!
//! These tests pin the two reds (the door says nothing; the campaign cannot say
//! it either), the one mechanism, and the lifetime.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign, validate_campaign_with};

/// The `souls-shortcut` fixture: a doorway slab sealed from world-load, opened
/// from the far side, with the author's own **left**-click answer already on it.
/// Its right-click was the silence.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-shortcut parses");
    assert!(
        diagnostics(&campaign).is_empty(),
        "fixture must validate clean"
    );
    campaign
}

fn diagnostics(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn try_build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).expect("every emitted command validates")
}

fn function(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/function/{name}.mcfunction");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no shipped function `{name}`"))
}

fn advancement(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/advancement/{name}.json");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no advancement `{name}`"))
}

/// Every shipped `.mcfunction` body, concatenated.
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

/// The door's own press-answer names, derived exactly as the compiler derives
/// them (`plan::press_answer_trigger_id` → `safe_local`).
const DOOR_TRIG: &str = "dw_press_door_inner_door";

// ---------------------------------------------------------------------------
// Red 1 — the door said nothing
// ---------------------------------------------------------------------------

/// **The owner's finding, as a machine test.** A sealed shortcut door answers a
/// right-click: an advancement watching its bodies, a dispatch that runs as the
/// presser, and a line on that player's actionbar.
///
/// On `origin/main` the shipped tree contained **none** of the three for a
/// shortcut door — `seal_hint_fns` and the `seal_<safe>` advancement were built
/// over `plan.seal_hints` alone, and `DW0372` structurally forbids a `close-gate`
/// on a shortcut gate, so there was no way for a door ever to enter that list.
/// The module docs claimed the wording "defaults"; no code defaulted it.
#[test]
fn a_sealed_shortcut_door_answers_a_press() {
    let out = build(&fixture());

    let adv = advancement(&out, &format!("press_{DOOR_TRIG}"));
    assert!(
        adv.contains("minecraft:player_interacted_with_entity")
            && adv.contains(&format!("dw_trig_{DOOR_TRIG}")),
        "a right-click on the door must dispatch as the player who pressed it: {adv}"
    );

    let dispatch = function(&out, &format!("press_{DOOR_TRIG}"));
    assert!(
        dispatch.contains("advancement revoke @s only"),
        "the door answers EVERY press, not only the first: {dispatch}"
    );

    let answer = function(&out, &format!("trig_{DOOR_TRIG}"));
    assert!(
        answer.contains("title @s actionbar"),
        "the answer reaches the presser's actionbar, not the party's chat: {answer}"
    );
    assert!(
        answer.contains("delvewright.ui.gate.sealed"),
        "an unauthored answer is translatable chrome, not a baked English literal: {answer}"
    );
}

/// The answer is not a mechanism of its own: it **rides** the bodies the door
/// already owns and summons nothing. A second co-located box is the exact
/// ray-pick tie `DW0422` forbids.
#[test]
fn the_answer_rides_the_door_it_does_not_build_a_second_one() {
    let out = build(&fixture());
    let arm = function(&out, "ws_arm_inner_door");
    assert_eq!(
        arm.matches(&format!("\"dw_trig_{DOOR_TRIG}\"")).count(),
        6,
        "the answer's tag rides every one of the door's six bodies: {arm}"
    );
    assert!(
        !function(&out, "setup_finish").contains(&format!("Tags:[\"dw_trig_{DOOR_TRIG}\"]")),
        "…and it summons none of its own"
    );
}

/// **The lifetime, which is not `close-gate`'s.** A shortcut opens permanently
/// (`DW0372`: there is no re-seal verb), so a door that kept saying it cannot be
/// opened after you opened it would be worse than the silence this closes.
///
/// Nothing has to remember that. The answer rides the door's bodies, and
/// `shortcut_open_<id>` kills them — so the answer retires with the thing it was
/// about, structurally, in the same function that lifts the bars.
#[test]
fn the_answer_dies_with_the_door() {
    let out = build(&fixture());
    let open = function(&out, "shortcut_open_inner_door");
    assert!(
        open.contains("kill @e[tag=dw_ws_inner_door]"),
        "the unlock retires the door's bodies, and the answer rides them: {open}"
    );
    // The proof that riding is what makes this hold: the answer's own tag exists
    // nowhere except on those bodies, so there is nothing left to answer with.
    let armed = all_functions(&out)
        .lines()
        .filter(|l| l.starts_with("summon minecraft:interaction"))
        .filter(|l| l.contains(&format!("dw_trig_{DOOR_TRIG}")))
        .count();
    assert_eq!(
        armed, 6,
        "every body carrying the answer is one `ws_arm_inner_door` summons"
    );
}

// ---------------------------------------------------------------------------
// Red 2 — the campaign could not say it either
// ---------------------------------------------------------------------------

/// **The authoring red.** A campaign that wants its own wrong-side line writes
/// the general verb — a `use` trigger on the gate, answering the presser on the
/// actionbar. Both halves of that sentence were inexpressible before v0.11:
/// `narrate` had no `actionbar`, and a trigger could not address a presser.
///
/// Under a pre-0.11 `dsl_version` each is `DW0141`, which is the fence; the point
/// of the test is the pair of paths that produce it.
#[test]
fn the_general_answer_is_reserved_before_v011() {
    let mut c = fixture();
    c.quests.content.triggers.push(authored_answer());
    let diags = diagnostics(&c);
    let reserved: Vec<&delvewright_dsl::Diagnostic> =
        diags.iter().filter(|d| d.code == "DW0141").collect();
    assert!(
        reserved.iter().any(|d| d.path.ends_with("/style")),
        "the `actionbar` channel is fenced: {diags:#?}"
    );
    assert!(
        reserved.iter().any(|d| d.path.ends_with("/audience")),
        "the presser addressee is fenced: {diags:#?}"
    );
}

/// …and at 0.11.0 it compiles, and produces exactly the shape the compiler's own
/// default produces — same advancement criterion, same `@s` actionbar command.
/// That equality is the whole claim: there is one mechanism, and the engine's
/// answer is an ordinary consumer of it.
#[test]
fn the_campaign_can_write_its_own_wrong_side_answer() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    c.quests.content.triggers.push(authored_answer());
    assert!(
        diagnostics(&c).is_empty(),
        "the authored answer validates clean at 0.11.0: {:#?}",
        diagnostics(&c)
    );
    let out = build(&c);

    let adv = advancement(&out, "press_from_the_wrong_side");
    assert!(
        adv.contains("minecraft:player_interacted_with_entity")
            && adv.contains("dw_trig_from_the_wrong_side"),
        "the authored answer runs as the presser, by the same criterion: {adv}"
    );
    let answer = function(&out, "trig_from_the_wrong_side");
    assert!(
        answer.contains("title @s actionbar")
            && answer.contains("The door cannot be opened from this side."),
        "the campaign's own line, on the presser's actionbar: {answer}"
    );
}

/// **The engine does not talk over the campaign.** Once the author answers the
/// press at that anchor, the compiler supplies nothing — one press, one answer.
#[test]
fn an_authored_answer_replaces_the_compilers() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    c.quests.content.triggers.push(authored_answer());
    let out = build(&c);
    assert!(
        !all_functions(&out).contains(&format!("trig_{DOOR_TRIG}")),
        "the compiler's default must stand down: {}",
        all_functions(&out)
    );
    assert!(
        !all_functions(&out).contains("delvewright.ui.gate.sealed"),
        "…and its chrome must not ship either"
    );
}

/// The campaign's own wrong-side answer, on the general verb.
fn authored_answer() -> EnvTrigger {
    serde_json::from_str::<EnvTrigger>(
        r#"{ "id": "trigger/from-the-wrong-side", "at": "anchor/door",
             "on": { "on": "use" }, "once": false, "audience": "presser",
             "effects": [ { "type": "narrate", "style": "actionbar",
                            "text": "The door cannot be opened from this side." } ] }"#,
    )
    .expect("the authored answer parses")
}

// ---------------------------------------------------------------------------
// The diagnostics the surface needs
// ---------------------------------------------------------------------------

/// `DW0427` — vanilla can attribute a right-click to a player and nothing else.
/// A left-click is recorded in the entity's `attack` NBT as a UUID no command can
/// become, so `audience: presser` on a `strike` is refused rather than
/// approximated (CLAUDE.md: a capability with no vanilla primitive under it is
/// excluded, never faked downstream).
#[test]
fn a_presser_answer_on_a_left_click_is_dw0427() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    let mut t = authored_answer();
    t.on = delvewright_dsl::TriggerOn::Strike;
    c.quests.content.triggers.push(t);
    let diags = diagnostics(&c);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0427")
        .unwrap_or_else(|| panic!("expected DW0427, got {diags:#?}"));
    assert!(
        d.message.contains("trigger/from-the-wrong-side") && d.message.contains("strike"),
        "DW0427 names the trigger and the click it watches: {}",
        d.message
    );
}

/// `DW0428` — the compiler synthesizes triggers of its own, so it reserves an id
/// namespace for them. Two triggers with one id would share one `dw_trig_…` tag
/// and one emitted function, and one of them would silently disappear.
#[test]
fn an_authored_trigger_in_the_reserved_namespace_is_dw0428() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    let mut t = authored_answer();
    t.id = delvewright_dsl::TriggerId(format!("trigger/{DOOR_TRIG}").replace('_', "-"));
    c.quests.content.triggers.push(t);
    let diags = diagnostics(&c);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0428")
        .unwrap_or_else(|| panic!("expected DW0428, got {diags:#?}"));
    assert!(
        d.message.contains("dw-"),
        "DW0428 names the reserved prefix: {}",
        d.message
    );
}

// ---------------------------------------------------------------------------
// Binding — a green gate that binds to nothing is vacuous
// ---------------------------------------------------------------------------

/// **The binding count, stated.** The compiler's press answers bind to the union
/// of `close-gate` seals and shortcut doors, and on this fixture that union is
/// exactly one door — which is one more than the zero the old, verb-keyed answer
/// bound to here.
#[test]
fn the_press_answers_bind_to_the_pressable_class() {
    let c = fixture();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    assert_eq!(
        plan.seal_hints.len(),
        0,
        "this fixture has no `close-gate`, which is why the old answer bound to nothing"
    );
    assert_eq!(plan.shortcuts.len(), 1);
    assert_eq!(
        plan.press_answers.len(),
        1,
        "one pressable body, one answer: {:?}",
        plan.press_answers
    );
    assert_eq!(plan.press_answers[0].owner, "shortcut door");
    assert_eq!(plan.press_answers[0].anchor, "anchor/door");
}

/// **`DW0422`'s binding, stated.** The hitbox-contest proof was written over
/// `close-gate` seals alone, and task #50 then gave a shortcut door press
/// hitboxes of its own — standing in the open air on the sealed side, which is
/// exactly where a lever or an objective marker plausibly stands — without
/// widening it. On this fixture it therefore examined **zero** bodies: a green
/// that bound to nothing.
///
/// It now walks the pressable class, so the same fixture binds one body of six
/// cells. Nothing about a ray-pick contest was ever specific to closing a gate.
#[test]
fn the_hitbox_contest_proof_binds_to_the_door_too() {
    let c = fixture();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let binding = delvewright_compiler::eclipse::pressable_body_binding(&plan);
    assert_eq!(
        binding,
        vec![("shortcut door", "anchor/door".to_string(), 6)],
        "DW0422 must examine the door's six press cells; before v0.11 it examined none"
    );
}

/// A campaign with neither a seal nor a shortcut synthesizes nothing at all — the
/// byte-identity half of the claim, at the source.
#[test]
fn a_campaign_with_nothing_sealed_gets_no_press_answer() {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    c.quests.content.triggers.clear();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    assert!(plan.press_answers.is_empty());
    let out = try_build(&c).expect("builds");
    assert!(
        !all_functions(&out).contains("dw_press_"),
        "nothing of the press-answer path is emitted"
    );
}

/// Determinism (ADR-0006): the synthesized triggers are two fixed orders
/// concatenated, so two plans of one campaign produce the identical list.
#[test]
fn the_synthesis_is_deterministic() {
    let c = fixture();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let first: Vec<String> = Plan::build(&c, &prefabs)
        .unwrap()
        .press_answers
        .iter()
        .map(|p| p.trigger_id.clone())
        .collect();
    for _ in 0..8 {
        let again: Vec<String> = Plan::build(&c, &prefabs)
            .unwrap()
            .press_answers
            .iter()
            .map(|p| p.trigger_id.clone())
            .collect();
        assert_eq!(first, again);
    }
}
