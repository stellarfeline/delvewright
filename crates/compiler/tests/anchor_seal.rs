//! `DW0360` — the build-time resolved-anchor-or-diagnostic seal.
//!
//! Emission fails **open** on an unresolved anchor: `open-gate`/`close-gate` scan
//! `plan.anchors` for a name match and fall out of the loop, `set-block` and
//! friends bail out of an `if let Some(pos)`, and a cutscene waypoint degrades to
//! the world origin. So a typo'd or unassembled anchor used to emit *nothing* —
//! a door that never opens, in a delve that compiled clean. `DW0142` catches what
//! the DSL can see (an area's declared anchor set); this seal makes the rule total
//! by re-asking the question of the *assembled* world, at every nesting depth.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

/// A hello-world `quests` doc whose `obj/talk` completion fires `effects` (a raw
/// JSON array body, no surrounding brackets).
fn quests_doc(effects: &str) -> String {
    quests_doc_with("", effects)
}

/// The same, with a caller-supplied `content` prelude (a `traps` array, trailing
/// comma included) so a fixture can put its effect at the `traps[].payload` root.
fn quests_doc_with(prelude: &str, effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    {prelude}
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str, dialogue: Option<&str>) -> Campaign {
    parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue
            .map(str::to_string)
            .unwrap_or_else(|| read_hw("dialogue.json")),
        world_edits: None,
    })
    .expect("campaign parses")
}

/// Plan + emit the campaign, returning the raw build result.
fn try_build(effects: &str) -> Result<BuildOutput, BuildFailure> {
    build_campaign(&quests_doc(effects), None)
}

/// Plan + emit a caller-supplied stage 5 (+ optional stage 6).
fn build_campaign(quests: &str, dialogue: Option<&str>) -> Result<BuildOutput, BuildFailure> {
    let campaign = parse_hw(quests, dialogue);
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
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn expect_dw0360(effects: &str, what: &str) {
    assert_dw0360(try_build(effects), what);
}

fn assert_dw0360(built: Result<BuildOutput, BuildFailure>, what: &str) {
    match built {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW0360", "{what}: wrong code, message was: {message}");
        }
        Err(other) => panic!("{what}: expected DW0360, got {other:?}"),
        Ok(_) => panic!("{what}: expected DW0360, but the build succeeded"),
    }
}

/// Every emitted `datapack/` function body, concatenated — what actually ships.
fn all_function_text(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .collect()
}

/// The control: the same nesting shape on a real anchor builds, and the gate fill
/// really is emitted from inside the sequence step. The island nests its gate
/// fills exactly this way — the seal must not forbid nesting, only unresolved
/// anchors.
#[test]
fn nested_open_gate_on_a_real_anchor_builds_and_emits_the_fill() {
    let out = try_build(
        r#"{ "type": "sequence", "steps": [
             { "at_ticks": 0, "effects": [
                 { "type": "open-gate", "anchor": "anchor/door" } ] } ] }"#,
    )
    .expect("a nested open-gate on a real anchor must build");
    let all = all_function_text(&out);
    assert!(
        all.lines()
            .any(|l| l.trim_start().starts_with("fill ") && l.contains("minecraft:air replace")),
        "the nested open-gate must still emit its fill:\n{all}"
    );
}

/// A typo'd `open-gate` anchor **nested in a `sequence` step** fails the build with
/// `DW0360` instead of emitting nothing.
#[test]
fn typod_nested_open_gate_anchor_is_dw0360() {
    expect_dw0360(
        r#"{ "type": "sequence", "steps": [
             { "at_ticks": 0, "effects": [
                 { "type": "open-gate", "anchor": "anchor/dorr" } ] } ] }"#,
        "typo'd nested open-gate",
    );
}

/// A typo'd `set-block` anchor two levels down — inside a `move-npc`'s `on_arrive`,
/// inside a `sequence` step — is `DW0360` too.
#[test]
fn typod_anchor_two_levels_down_is_dw0360() {
    expect_dw0360(
        r#"{ "type": "sequence", "steps": [
             { "at_ticks": 0, "effects": [
                 { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
                   "on_arrive": [
                     { "type": "set-block", "anchor": "anchor/nowhere",
                       "block": "minecraft:cobblestone" } ] } ] } ] }"#,
        "typo'd anchor inside on_arrive inside a sequence",
    );
}

/// A typo'd `set-checkpoint` anchor is `DW0360` — the checkpoint would otherwise
/// bind to nothing and the party would respawn at the world spawn.
#[test]
fn typod_set_checkpoint_anchor_is_dw0360() {
    expect_dw0360(
        r#"{ "type": "set-checkpoint", "anchor": "anchor/knowhere" }"#,
        "typo'd set-checkpoint anchor",
    );
}

/// A typo'd cutscene **camera waypoint** anchor is `DW0360`. This one degraded
/// most quietly of all: an unresolved waypoint silently resolved to the world
/// origin, so the shot flew off to `[0, 64, 0]` and the delve still compiled.
#[test]
fn typod_cutscene_waypoint_anchor_is_dw0360() {
    expect_dw0360(
        r#"{ "type": "cutscene", "seconds": 3,
             "path": [ { "anchor": "anchor/exit" }, { "anchor": "anchor/elsewhere" } ] }"#,
        "typo'd cutscene waypoint anchor",
    );
}

// ---------------------------------------------------------------------------
// The two roots the seal did not reach (task #24)
// ---------------------------------------------------------------------------
//
// The seal's own doc calls it "the backstop that makes the rule total", but its
// walk hand-listed three of the five roots `plan::for_each_effect_root`
// enumerates: a `traps[].payload` and a dialogue option's `set-checkpoint`
// `on_respawn` bundle were never asked the question at all. Emission lowers both,
// and lowers them failing **open** exactly like every other root — so a typo'd
// anchor there emitted nothing, said nothing, and shipped.
//
// Each root gets a control (the same shape on a REAL anchor builds and its
// command really is in the emitted pack) and the red case. The control is what
// keeps the red case from being vacuous: it proves the root is lowered, so a
// missing diagnostic there is a silent drop rather than a no-op.

/// A trap whose `payload` fires `effects` (a raw JSON array body), as a `content`
/// prelude for [`quests_doc_with`].
fn trap_prelude(effects: &str) -> String {
    format!(
        r#""traps": [
      {{ "id": "trap/alarm-chest", "at": "anchor/exit", "trigger": "trapped-chest",
         "lethality": "harmful",
         "payload": [ {effects} ] }}
    ],"#
    )
}

/// hello-world's dialogue with the "Open the door" option additionally hanging a
/// `set-checkpoint` on `anchor/exit`, whose `on_respawn` bundle fires `effects`.
fn respawn_dialogue(effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {{
    "dialogues": [
      {{ "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        {{ "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            {{ "label": "Open the door, please.",
              "effects": [
                {{ "type": "complete-objective", "objective": "obj/talk" }},
                {{ "type": "set-checkpoint", "anchor": "anchor/exit",
                  "on_respawn": [ {effects} ] }}
              ] }}
          ] }}
      ] }}
    ]
  }}
}}"#
    )
}

/// Control for root 4: an `open-gate` in a `traps[].payload` on a real anchor
/// builds, and its fill really is in the shipped pack.
#[test]
fn open_gate_in_a_trap_payload_emits_its_fill() {
    let out = build_campaign(
        &quests_doc_with(
            &trap_prelude(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
            r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
        ),
        None,
    )
    .expect("an open-gate in a trap payload must build");
    let all = all_function_text(&out);
    assert!(
        all.lines()
            .any(|l| l.trim_start().starts_with("fill ") && l.contains("minecraft:air replace")),
        "a trap payload is lowered — its open-gate must emit a fill:\n{all}"
    );
}

/// A typo'd anchor in a `traps[].payload` is `DW0360`.
///
/// Red before task #24: the seal never walked root 4, so this built **clean** and
/// `trap_fire_alarm_chest.mcfunction` shipped with the `open-gate` simply absent —
/// a trap that springs and does nothing, with no diagnostic anywhere.
#[test]
fn typod_anchor_in_a_trap_payload_is_dw0360() {
    assert_dw0360(
        build_campaign(
            &quests_doc_with(
                &trap_prelude(r#"{ "type": "open-gate", "anchor": "anchor/dorr" }"#),
                r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
            ),
            None,
        ),
        "typo'd open-gate anchor in a trap payload",
    );
}

/// The same, one level deeper: the payload's `sequence` step carries the typo, so
/// the fix must descend the root's nesting and not merely glance at its top level.
#[test]
fn typod_anchor_nested_in_a_trap_payload_is_dw0360() {
    assert_dw0360(
        build_campaign(
            &quests_doc_with(
                &trap_prelude(
                    r#"{ "type": "sequence", "steps": [
                         { "at_ticks": 0, "effects": [
                             { "type": "set-block", "anchor": "anchor/nowhere",
                               "block": "minecraft:cobblestone" } ] } ] }"#,
                ),
                r#"{ "type": "narrate", "style": "chat", "text": "The bar lifts." }"#,
            ),
            None,
        ),
        "typo'd set-block anchor inside a sequence inside a trap payload",
    );
}

/// Control for root 5: a `set-block` in a dialogue option's `on_respawn` bundle on
/// a real anchor builds, and its `setblock` really is in the shipped pack (it is
/// lowered into `cp_on_respawn_<i>`).
#[test]
fn set_block_in_a_dialogue_respawn_bundle_emits_its_setblock() {
    let out = build_campaign(
        &quests_doc(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
        Some(&respawn_dialogue(
            r#"{ "type": "set-block", "anchor": "anchor/exit", "block": "minecraft:cobblestone" }"#,
        )),
    )
    .expect("a set-block in a dialogue on_respawn bundle must build");
    let all = all_function_text(&out);
    assert!(
        all.lines()
            .any(|l| l.trim_start().starts_with("setblock ") && l.contains("minecraft:cobblestone")),
        "a dialogue on_respawn bundle is lowered — its set-block must emit:\n{all}"
    );
}

/// A typo'd anchor in a dialogue option's `set-checkpoint` `on_respawn` bundle is
/// `DW0360`.
///
/// Red before task #24 for the same reason as root 4, and quieter still: the
/// bundle only runs after a death, so nothing before the party's first respawn
/// could even hint that the block was never placed.
#[test]
fn typod_anchor_in_a_dialogue_respawn_bundle_is_dw0360() {
    assert_dw0360(
        build_campaign(
            &quests_doc(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
            Some(&respawn_dialogue(
                r#"{ "type": "set-block", "anchor": "anchor/nowhere",
                     "block": "minecraft:cobblestone" }"#,
            )),
        ),
        "typo'd set-block anchor in a dialogue on_respawn bundle",
    );
}
