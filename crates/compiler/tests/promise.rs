//! **An objective keeps the promise its prompt makes** (`DW0860`–`DW0863`).
//!
//! Fixture shape: hello-world's world/classes/npcs/dialogue, plus a two-quest DAG
//! whose stage-5 document is supplied per test. Every test drives
//! `promise::check` on a parsed campaign, so the rules are exercised exactly as
//! `delvec validate` runs them.
//!
//! Each rule gets four things, because a refusal that is only shown refusing has
//! not been shown to be about anything: the refusal, the correct document staying
//! clean, a **perturbation in the vacuous direction** (remove the thing the rule
//! demands and check the gate goes red rather than quiet), and — for `DW0860` —
//! the arithmetic pinned to the released campaign that motivated it.

mod common;

use delvewright_compiler::promise;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        common::repo_root()
            .join("crates/dsl/fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

const NPCS: &str = r#"{
  "dsl_version": "0.9.0", "campaign_id": "hello-world", "stage": "npcs",
  "content": { "npcs": [
    { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
      "area": "area/keep", "anchor": "anchor/keeper-stand", "base_entity": "minecraft:villager",
      "persona": { "archetype": "stoic gatekeeper", "speech_style": "Terse.", "motivation": "Guard the gate." } }
  ] }
}"#;

const QUEST_PLAN: &str = r#"{
  "dsl_version": "0.9.0", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": { "quests": [
    { "id": "quest/one", "goal": "Speak with the Keeper.", "area": "area/keep",
      "npcs": ["npc/keeper"], "depends_on": [], "mandatory": true, "act": 1 },
    { "id": "quest/two", "goal": "Leave the keep.", "area": "area/keep",
      "npcs": [], "depends_on": ["quest/one"], "mandatory": true, "act": 1 }
  ], "finale": "quest/two" }
}"#;

const DIALOGUE: &str = r#"{
  "dsl_version": "0.9.0", "campaign_id": "hello-world", "stage": "dialogue",
  "content": { "dialogues": [
    { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
      { "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "Open the door.", "effects": [{ "type": "complete-objective", "objective": "obj/talk" }] } ] } ] }
  ] }
}"#;

/// A two-quest stage-5 document. `second` is `quest/two`'s objective list and
/// `complete` its `on_complete` bundle, so one fixture serves every rule.
fn quests(second: &str, complete: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.9.0", "campaign_id": "hello-world", "stage": "quests",
  "content": {{
    "waves": [ {{ "id": "wave/garrison", "anchor": "anchor/exit",
                  "mobs": [ {{ "entity": "minecraft:zombie", "count": 2 }} ] }} ],
    "quests": [
    {{ "id": "quest/one", "trigger": {{ "type": "campaign-start" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
                          "title": "Speak to the Keeper" }} ],
       "on_complete": [ {{ "type": "spawn-wave", "wave": "wave/garrison" }} ] }},
    {{ "id": "quest/two", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "objectives": [ {second} ],
       "on_complete": [ {complete} ] }}
  ] }}
}}"#
    )
}

fn campaign(second: &str, complete: &str) -> Campaign {
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests(second, complete),
        dialogue: DIALOGUE.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("fixture must parse")
}

fn codes(c: &Campaign) -> Vec<String> {
    promise::check(c).0.iter().map(|d| d.code.clone()).collect()
}

/// The `on_complete` bundle used by every test that is not about `DW0860`.
const DONE: &str = r#"{ "type": "campaign-complete" }"#;

/// A fully-signed reach objective — the clean baseline for the prompt rules.
const CLEAN_REACH: &str = r#"{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
    "radius": 2, "title": "Leave the Keep", "hint": "The gate stands open behind you." }"#;

#[test]
fn a_fully_signed_campaign_raises_nothing() {
    let d = promise::check(&campaign(CLEAN_REACH, DONE)).0;
    assert!(d.is_empty(), "expected a clean campaign, got: {d:#?}");
}

// --- DW0862: a prompt the emitter will never show --------------------------

/// A `hint` with no `title` is prose that reaches no player: the activation
/// announcement is guarded on the title and the hint's line is nested inside it.
#[test]
fn hint_without_title_is_dw0862() {
    let obj = r#"{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
        "radius": 2, "hint": "The gate stands open behind you." }"#;
    let c = campaign(obj, DONE);
    let d = promise::check(&c)
        .0
        .into_iter()
        .find(|d| d.code == promise::DW_PROMPT_UNSHOWN)
        .expect("a hint with no title must be refused");
    assert_eq!(d.code, "DW0862");
    assert!(d.message.contains("obj/exit"), "{}", d.message);
    assert!(d.message.contains("title"), "{}", d.message);
    assert_eq!(d.path, "/content/quests/1/objectives/0/hint");
}

/// An objective with neither string is not this rule's business: `DW0862` is
/// about a prompt that was WRITTEN and is not shown, never about one nobody
/// wrote. Pins the boundary so a later widening is a decision rather than drift.
#[test]
fn an_objective_with_no_prompt_at_all_is_not_dw0862() {
    let obj =
        r#"{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }"#;
    let got = codes(&campaign(obj, DONE));
    assert!(
        !got.iter().any(|c| c == "DW0862"),
        "an untitled, unhinted objective is silent, not contradictory: {got:?}"
    );
}

// --- DW0863: a fight nothing points at -------------------------------------

/// A `kill` objective is the one kind the compiler leaves nothing in the world
/// for, so it owes both lines. Each half is refused on its own.
#[test]
fn a_kill_objective_without_both_lines_is_dw0863() {
    for (obj, expect) in [
        (
            r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison" }"#,
            "neither a `title` nor a `hint`",
        ),
        (
            r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison", "hint": "They come through the breach." }"#,
            "no `title`",
        ),
        (
            r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison", "title": "Hold the Breach" }"#,
            "no `hint`",
        ),
    ] {
        let c = campaign(obj, DONE);
        let d = promise::check(&c)
            .0
            .into_iter()
            .find(|d| d.code == promise::DW_FIGHT_UNSIGNED)
            .unwrap_or_else(|| panic!("a kill objective missing {expect} must be DW0863"));
        assert_eq!(d.code, "DW0863");
        assert!(d.message.contains(expect), "{}", d.message);
        assert!(d.message.contains("wave/garrison"), "{}", d.message);
        assert!(d.message.contains("obj/purge"), "{}", d.message);
    }
}

/// A `kill` objective carrying both lines is clean — and the same document with
/// the two lines removed is not. The pair is the perturbation: it shows the rule
/// is answering about THESE fields rather than passing for some other reason.
#[test]
fn a_signed_kill_objective_is_clean_and_stripping_it_reds() {
    let signed = r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison",
        "title": "Hold the Breach", "hint": "They come up the stair from the muster." }"#;
    let stripped = r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison" }"#;
    let clean = codes(&campaign(signed, DONE));
    let red = codes(&campaign(stripped, DONE));
    assert!(!clean.iter().any(|c| c == "DW0863"), "{clean:?}");
    assert!(red.iter().any(|c| c == "DW0863"), "{red:?}");
}

// --- DW0861: an adopted container nothing distinguishes --------------------

/// Adoption is what creates the ambiguity, so the rule keys off `container`.
#[test]
fn an_adopted_container_without_both_marks_is_dw0861() {
    for (obj, expect) in [
        (
            r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie", "count": 1,
                 "anchor": "anchor/exit", "container": "anchor/keeper-stand" }"#,
            "neither a `title` nor an `item_name`",
        ),
        (
            r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie", "count": 1,
                 "anchor": "anchor/exit", "container": "anchor/keeper-stand", "item_name": "Kefalotyri" }"#,
            "no `title`",
        ),
        (
            r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie", "count": 1,
                 "anchor": "anchor/exit", "container": "anchor/keeper-stand", "title": "Take the Cheese" }"#,
            "no `item_name`",
        ),
    ] {
        let c = campaign(obj, DONE);
        let d = promise::check(&c)
            .0
            .into_iter()
            .find(|d| d.code == promise::DW_ADOPTED_CONTAINER_UNMARKED)
            .unwrap_or_else(|| panic!("an adopted container missing {expect} must be DW0861"));
        assert_eq!(d.code, "DW0861");
        assert!(d.message.contains(expect), "{}", d.message);
        assert!(d.message.contains("anchor/keeper-stand"), "{}", d.message);
    }
}

/// A `collect` that does NOT adopt is untouched: the compiler's own chest is a
/// new object that appears when the objective activates, so it announces itself.
/// This is the boundary that makes the rule about adoption rather than about
/// `collect`, and it is the one a careless widening would erase.
#[test]
fn a_collect_that_conjures_its_own_chest_is_not_dw0861() {
    let obj = r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie",
        "count": 1, "anchor": "anchor/exit" }"#;
    let got = codes(&campaign(obj, DONE));
    assert!(
        !got.iter().any(|c| c == "DW0861"),
        "a conjured chest is not adopted scenery: {got:?}"
    );
}

// --- DW0860: a failure clock nothing explained -----------------------------

/// `grace_ticks` ticks of clock and not a word before it.
#[test]
fn a_failure_clock_with_no_prompt_is_dw0860() {
    let bundle = r#"{ "type": "begin-stealth", "grace_ticks": 40,
        "zones": [ { "anchor": "anchor/exit", "extent": [4, 3, 4] } ],
        "on_caught": [ { "type": "narrate", "text": "Seen." } ] }"#;
    let c = campaign(CLEAN_REACH, bundle);
    let d = promise::check(&c)
        .0
        .into_iter()
        .find(|d| d.code == promise::DW_CLOCK_UNREAD)
        .expect("an unexplained failure clock must be refused");
    assert_eq!(d.code, "DW0860");
    assert!(
        d.message.contains("no `narrate` fires before it"),
        "{}",
        d.message
    );
    assert!(d.message.contains("40"), "{}", d.message);
}

/// The explanation may not live in `on_caught`: that line is read AFTER the
/// punishment, which is the defect rather than the fix. Pins the one wrong
/// repair the message names, so the rule cannot be satisfied by moving the prose
/// into the consequence.
#[test]
fn a_prompt_inside_on_caught_does_not_satisfy_dw0860() {
    let bundle = r#"{ "type": "begin-stealth", "grace_ticks": 400,
        "zones": [ { "anchor": "anchor/exit", "extent": [4, 3, 4] } ],
        "on_caught": [ { "type": "narrate", "text": "Keep out of the light." } ] }"#;
    let got = codes(&campaign(CLEAN_REACH, bundle));
    assert!(
        got.iter().any(|c| c == "DW0860"),
        "prose in on_caught is read after the punishment: {got:?}"
    );
}

/// A prompt before the clock, with time to read it, is clean — and the identical
/// document with the grace shortened is not. Varying ONE number is what shows the
/// arithmetic is load-bearing rather than the presence of the narrate alone.
#[test]
fn the_clock_must_outlast_the_reading() {
    let text = "Keep out of the light until the march has passed.";
    let needed = promise::read_ticks(text);
    assert_eq!(
        needed,
        20 + 2 * 49,
        "49 characters at 2 ticks, after 20 to appear"
    );

    let bundle = |grace: u32| {
        format!(
            r#"{{ "type": "narrate", "text": "{text}" }},
               {{ "type": "begin-stealth", "grace_ticks": {grace},
                  "zones": [ {{ "anchor": "anchor/exit", "extent": [4, 3, 4] }} ],
                  "on_caught": [ {{ "type": "narrate", "text": "Seen." }} ] }}"#
        )
    };
    let enough = codes(&campaign(CLEAN_REACH, &bundle(needed)));
    assert!(!enough.iter().any(|c| c == "DW0860"), "{enough:?}");

    let c = campaign(CLEAN_REACH, &bundle(needed - 1));
    let d = promise::check(&c)
        .0
        .into_iter()
        .find(|d| d.code == promise::DW_CLOCK_UNREAD)
        .expect("one tick short of readable must be refused");
    assert!(d.message.contains(&needed.to_string()), "{}", d.message);
    assert!(d.message.contains("49 characters"), "{}", d.message);
}

/// A `sequence` moves the arming down its own timeline, so the interval the
/// party gets is the step offset plus the grace. The default `grace_ticks` of 20
/// is one second — far under any real line — so a beat that is legal at all is
/// legal *because* of the offset, which is what this pins.
#[test]
fn a_sequence_offset_counts_toward_the_reading_time() {
    let text = "Keep out of the light until the march has passed.";
    let needed = promise::read_ticks(text);
    let seq = |at: u32| {
        format!(
            r#"{{ "type": "sequence", "steps": [
                 {{ "at_ticks": 0, "effects": [ {{ "type": "narrate", "text": "{text}" }} ] }},
                 {{ "at_ticks": {at}, "effects": [
                    {{ "type": "begin-stealth", "grace_ticks": 20,
                       "zones": [ {{ "anchor": "anchor/exit", "extent": [4, 3, 4] }} ],
                       "on_caught": [ {{ "type": "narrate", "text": "Seen." }} ] }} ] }} ] }}"#
        )
    };
    let late = codes(&campaign(CLEAN_REACH, &seq(needed - 20)));
    assert!(!late.iter().any(|c| c == "DW0860"), "{late:?}");
    let early = codes(&campaign(CLEAN_REACH, &seq(0)));
    assert!(
        early.iter().any(|c| c == "DW0860"),
        "the same beat with the offset removed must red: {early:?}"
    );
}

/// The prompt the clock races is the LAST one before it, not the sum of the
/// bundle and not the longest. Three mutually-exclusive retellings of one line —
/// ordinary branch authoring — must not add up into a refusal.
///
/// This is the shape the released island actually ships: three long branch
/// variants of the same beat, then the short instruction, then the arming.
#[test]
fn branch_variants_before_the_instruction_do_not_accumulate() {
    let long = "The hills answer him, and his own kind call in at the stone, and he roars back the name you laid down over the fire.";
    let instruction = "Get up the ramp.";
    let grace = promise::read_ticks(instruction);
    assert!(
        grace < promise::read_ticks(long),
        "the fixture is toothless unless the variants are longer than the instruction"
    );
    let bundle = format!(
        r#"{{ "type": "narrate", "text": "{long}" }},
           {{ "type": "narrate", "text": "{long}" }},
           {{ "type": "narrate", "text": "{long}" }},
           {{ "type": "narrate", "text": "{instruction}" }},
           {{ "type": "begin-stealth", "grace_ticks": {grace},
              "zones": [ {{ "anchor": "anchor/exit", "extent": [4, 3, 4] }} ],
              "on_caught": [ {{ "type": "narrate", "text": "Seen." }} ] }}"#
    );
    let got = codes(&campaign(CLEAN_REACH, &bundle));
    assert!(
        !got.iter().any(|c| c == "DW0860"),
        "the last line before the clock is the one being read: {got:?}"
    );
}

/// A `begin-stealth` with an empty `on_caught` is not a failure clock: nothing
/// happens when the grace runs out, so there is nothing to have been warned
/// about. Pins the population boundary.
#[test]
fn a_stealth_beat_that_punishes_nothing_is_not_a_failure_clock() {
    let bundle = r#"{ "type": "begin-stealth", "grace_ticks": 1,
        "zones": [ { "anchor": "anchor/exit", "extent": [4, 3, 4] } ], "on_caught": [] }"#;
    let (d, b) = promise::check(&campaign(CLEAN_REACH, bundle));
    assert!(!d.iter().any(|x| x.code == "DW0860"), "{d:#?}");
    assert_eq!(
        b.failure_clocks, 0,
        "a consequence-free beat is not a clock"
    );
}

// --- the binding is measured, never asserted as a constant -----------------

/// The binding line states what was examined. Its counts are computed from the
/// documents, so a walk that silently stopped visiting a class shows up here as a
/// zero rather than as a quiet pass.
#[test]
fn the_binding_counts_what_it_examined() {
    let obj = r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie", "count": 1,
        "anchor": "anchor/exit", "container": "anchor/keeper-stand",
        "title": "Take the Cheese", "item_name": "Kefalotyri" }"#;
    let bundle = r#"{ "type": "begin-stealth", "grace_ticks": 400,
        "zones": [ { "anchor": "anchor/exit", "extent": [4, 3, 4] } ],
        "on_caught": [ { "type": "narrate", "text": "Seen." } ] },
        { "type": "narrate", "text": "Hi." }"#;
    let (_, b) = promise::check(&campaign(obj, bundle));
    assert_eq!(b.objectives, 2, "one talk-to and one collect");
    assert_eq!(b.kill_objectives, 0);
    assert_eq!(b.adopted_containers, 1);
    assert_eq!(b.failure_clocks, 1);
    assert_eq!(
        b.effect_roots,
        delvewright_dsl::EffectRootKind::COUNT,
        "the clock walk must reach every effect root, not a remembered subset"
    );
}

/// **The vacuity pin.** Perturb every objective in the fixture toward the shape
/// each rule exists to refuse, and count how many name themselves. A gate whose
/// binding is written down rather than computed is the defect the binding line
/// exists to expose, so this counts rather than asserts.
#[test]
fn every_rule_fires_on_its_own_perturbation() {
    let cases: [(&str, &str, &str); 4] = [
        (
            "DW0860",
            CLEAN_REACH,
            r#"{ "type": "begin-stealth", "grace_ticks": 40,
                 "zones": [ { "anchor": "anchor/exit", "extent": [4, 3, 4] } ],
                 "on_caught": [ { "type": "narrate", "text": "Seen." } ] }"#,
        ),
        (
            "DW0861",
            r#"{ "type": "collect", "id": "obj/cheese", "item": "minecraft:pumpkin_pie", "count": 1,
                 "anchor": "anchor/exit", "container": "anchor/keeper-stand" }"#,
            DONE,
        ),
        (
            "DW0862",
            r#"{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
                 "hint": "The gate stands open." }"#,
            DONE,
        ),
        (
            "DW0863",
            r#"{ "type": "kill", "id": "obj/purge", "wave": "wave/garrison" }"#,
            DONE,
        ),
    ];
    let fired = cases
        .iter()
        .filter(|(code, obj, complete)| codes(&campaign(obj, complete)).iter().any(|c| c == code))
        .count();
    assert_eq!(
        fired,
        cases.len(),
        "each of the four rules must name itself on its own perturbation"
    );

    // ...and the unperturbed document names none of them, so the count above is
    // about the perturbation rather than about the fixture.
    let base = codes(&campaign(CLEAN_REACH, DONE));
    let leaked = cases
        .iter()
        .filter(|(c, _, _)| base.iter().any(|b| b == c))
        .count();
    assert_eq!(
        leaked, 0,
        "the clean fixture must raise none of the four: {base:?}"
    );
}
