//! **A press is answered by one mechanism** (DSL v0.11).
//!
//! ## The finding
//!
//! The drowned-bell remake's central idiom is the souls shortcut loop: you walk
//! the long way round and open a barred door from the far side. Before you have
//! opened it, pressing that door from the wrong side must answer — *"the door
//! cannot be opened from this side."* It answered nothing, and the gate the party
//! is most likely to press was the one gate in the engine that stayed silent.
//!
//! The door already had a **body** (`crate::wrongside`,
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
//! ## And then: the compiler does not word it either
//!
//! The wording *may* be "The way is sealed." and it must be
//! creator-customisable — but the design is **no default at all:
//! if it is not defined, the compiler errors.** So a sealed shortcut door the
//! campaign never answers is `DW0429`, not a line the engine invents.
//!
//! A baked default is the compiler making a design statement — about tone, about
//! what this specific door is — on the author's behalf, and then never telling
//! them it did. An error makes the author say it. It is also the only end state
//! where the docs, the code and the player agree: `wrongside.rs` and the
//! reference claimed for two versions that a door's wording "defaults", no code
//! defaulted anything, and the door said nothing. The repair was never to make
//! the claimed default real.
//!
//! `close-gate`'s `sealed_hint` is out of scope and still bakes its canonical
//! English; the policy is a property of the body class
//! (`plan::press_answer_sites`), so extending the ruling is a changed arm.
//!
//! These tests pin the two reds (a barred door with nothing to say is refused;
//! the campaign could not have said it either), the one mechanism, and the
//! lifetime.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign};

/// The `souls-shortcut` fixture: a doorway slab sealed from world-load, opened
/// from the far side, with the author's own **left**-click answer already on it.
/// Its right-click was the silence.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("souls-shortcut parses");
    let d = diagnostics(&campaign);
    assert!(d.is_empty(), "fixture must validate clean: {d:#?}");
    campaign
}

/// The diagnostics the campaign is **answerable for** — raised, then put through
/// the obligation fence, which is the list `delvec` prints and exits on.
///
/// The fence is load-bearing for this file specifically: the base fixture
/// declares 0.9.0 and is deliberately a silent door, so `DW0429` is raised
/// against it on every call and grandfathered on every call. Reading the raw
/// list here would make the fixture's own silence — the thing the tests below
/// bump to 0.11.0 in order to observe — look like a broken fixture.
fn diagnostics(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    common::fenced_diagnostics(
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

/// The `safe_local` form of the authored answer's trigger id — what names its
/// function, its advancement and its entity tag. There is no compiler-derived
/// name here: a door's answer is the campaign's, so its id is the
/// campaign's too.
const DOOR_TRIG: &str = "from_the_wrong_side";

// ---------------------------------------------------------------------------
// Red 1 — the door said nothing
// ---------------------------------------------------------------------------

/// **The finding, as a machine test.** A campaign that bars a door must answer a
/// right-click on it, and that answer is an ordinary trigger: an advancement
/// watching the door's own bodies, a dispatch that runs as the presser, and a
/// line on that player's actionbar.
///
/// On `origin/main` a shortcut door had none of the three and no way to acquire
/// them — `seal_hint_fns` and the `seal_<safe>` advancement were built over
/// `plan.seal_hints` alone, and `DW0372` structurally forbids a `close-gate` on a
/// shortcut gate, so no door could ever enter that list.
#[test]
fn a_sealed_shortcut_door_answers_a_press() {
    let out = build(&answered_fixture());

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
        answer.contains("title @s actionbar")
            && answer.contains("The door cannot be opened from this side."),
        "the CAMPAIGN'S line reaches the presser's actionbar, not the party's chat: {answer}"
    );
}

/// The answer is not a mechanism of its own: it **rides** the bodies the door
/// already owns and summons nothing. A second co-located box is the exact
/// ray-pick tie `DW0422` forbids.
#[test]
fn the_answer_rides_the_door_it_does_not_build_a_second_one() {
    let out = build(&answered_fixture());
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
    let out = build(&answered_fixture());
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
// The ruling: the compiler does not word a door for the author
// ---------------------------------------------------------------------------

/// **`DW0429`.** A campaign at 0.11.0 that bars a door and says nothing about it
/// does not compile. The compiler had every ingredient to invent a line here and
/// deliberately does not: a baked default decides this door's tone on the
/// author's behalf and never discloses that it did.
#[test]
fn a_barred_door_with_nothing_to_say_is_dw0429() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    let diags = diagnostics(&c);
    let d = diags
        .iter()
        .find(|d| d.code == "DW0429")
        .unwrap_or_else(|| panic!("expected DW0429, got {diags:#?}"));
    assert!(
        d.message.contains("shortcut/inner-door") && d.message.contains("anchor/door"),
        "DW0429 names the door and its gate: {}",
        d.message
    );
    // The prescription must be writable as given: every field it names exists on
    // the surface it points at. This PR's own history is the reason to check —
    // `DW0425` spent two versions telling authors to write `on_wrong_side`, a
    // field no schema has ever had.
    let prescription = d
        .message
        .split("Prescription:")
        .nth(1)
        .expect("DW0429 prescribes a fix");
    let json = prescription[prescription.find('{').unwrap()..prescription.rfind('}').unwrap() + 1]
        .replace("<name>", "the-bars")
        .replace(
            "<what the door says>",
            "The door cannot be opened from this side.",
        );
    let parsed = serde_json::from_str::<EnvTrigger>(&json)
        .unwrap_or_else(|e| panic!("DW0429's own prescription does not parse: {e}\n{json}"));
    let mut fixed = c.clone();
    fixed.quests.content.triggers.push(parsed);
    assert!(
        diagnostics(&fixed).is_empty(),
        "…and writing exactly what it prescribes clears it: {:#?}",
        diagnostics(&fixed)
    );
}

/// **The fence.** `DW0429` is a tightening, so it cannot reach a campaign written
/// before it existed. The `souls-shortcut` fixture is a 0.9.0 campaign whose door
/// says nothing: it still validates, still builds, and still emits exactly what
/// it emitted before this version — no answer at all.
#[test]
fn a_pre_0_11_campaign_with_a_silent_door_still_compiles() {
    let c = fixture();
    assert_eq!(c.quests.dsl_version, "0.9.0");
    assert!(
        diagnostics(&c).is_empty(),
        "a pre-0.11 campaign is not held to the obligation: {:#?}",
        diagnostics(&c)
    );
    let out = build(&c);
    assert!(
        !all_functions(&out).contains("dw_press_"),
        "and the compiler invents nothing for it either: {}",
        all_functions(&out)
    );
}

/// Any `use` trigger on the gate discharges the obligation, whatever it does —
/// the same predicate the synthesis reads (`QuestsContent::answers_press_at`), so
/// the refusal and the synthesis can never disagree about what counts as an
/// answer. A `strike` does **not**: pressing a door is a right-click, and a
/// left-click answer is a different gesture the player may never make.
#[test]
fn the_obligation_is_discharged_by_any_use_trigger_and_not_by_a_strike() {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    // The fixture already carries a `strike` answer on this very gate.
    assert!(
        diagnostics(&c).iter().any(|d| d.code == "DW0429"),
        "a left-click answer leaves the press unanswered"
    );
    let mut door_opener = authored_answer();
    door_opener.effects = vec![
        serde_json::from_str(r#"{ "type": "play-sound", "sound": "minecraft:block.chain.hit" }"#)
            .expect("effect parses"),
    ];
    c.quests.content.triggers.push(door_opener);
    assert!(
        !diagnostics(&c).iter().any(|d| d.code == "DW0429"),
        "a `use` trigger that only plays a sound is still the author answering: {:#?}",
        diagnostics(&c)
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

/// The fixture with its wrong-side answer authored, at 0.11.0 — the shape every
/// 0.11 campaign that bars a door must have.
fn answered_fixture() -> Campaign {
    let mut c = fixture();
    c.quests.dsl_version = "0.11.0".to_string();
    c.quests.content.triggers.push(authored_answer());
    assert!(
        diagnostics(&c).is_empty(),
        "the answered fixture validates clean: {:#?}",
        diagnostics(&c)
    );
    c
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
    // The exact id `plan::press_answer_trigger_id` would mint for this door.
    t.id = delvewright_dsl::TriggerId("trigger/dw-press-door-inner-door".to_string());
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

/// **The binding count, stated.** The compiler's press answers bind to the
/// pressable bodies whose class lets it speak — which, since the ruling, is
/// `close-gate` seals alone. A shortcut door is in the same list and carries
/// `SilencePolicy::Authored`, so it is examined and deliberately produces nothing.
///
/// Zero synthesized answers here is therefore a *pass*, not the vacuity CLAUDE.md
/// warns about: the door is bound (the obligation `DW0429` fires on it), and what
/// binds to nothing is only the compiler's licence to invent.
#[test]
fn the_press_answers_bind_to_the_pressable_class() {
    let c = answered_fixture();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    assert_eq!(
        plan.seal_hints.len(),
        0,
        "this fixture has no `close-gate`, which is why the old answer bound to nothing"
    );
    assert_eq!(plan.shortcuts.len(), 1);
    assert!(
        plan.press_answers.is_empty(),
        "a door's wording is the author's; the compiler supplies none: {:?}",
        plan.press_answers
    );
}

/// **The policy is keyed to the VERSION, not to the verb.** Above the fence every
/// pressable body carries the same obligation; the two grandfathered arms below
/// it differ from each other only because the two classes historically did.
///
/// This assertion is the guard on the defect CLAUDE.md's worked example is about:
/// if someone later gives a door and a seal different defaulting rules at one
/// version, this fails.
#[test]
fn the_silence_policy_is_uniform_above_the_fence() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    use delvewright_compiler::plan::SilencePolicy;

    let answered = answered_fixture();
    let plan = Plan::build(&answered, &prefabs).expect("plan builds");
    let above = delvewright_compiler::plan::press_answer_policies(&plan);
    assert!(
        !above.is_empty() && above.iter().all(|(.., p)| *p == SilencePolicy::Authored),
        "at 0.11.0 nothing may be worded by the compiler: {above:?}"
    );

    let pre = fixture();
    let plan = Plan::build(&pre, &prefabs).expect("plan builds");
    let below = delvewright_compiler::plan::press_answer_policies(&plan);
    assert_eq!(
        below,
        vec![(
            "shortcut door",
            "anchor/door".to_string(),
            SilencePolicy::Silent
        )],
        "below it, a door keeps the silence it always had"
    );
}

/// **`DW0422`'s binding, stated.** The hitbox-contest proof was written over
/// `close-gate` seals alone, and a shortcut door then gained press
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
