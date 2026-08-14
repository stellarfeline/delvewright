//! The **staged-walk timeline** sees every root emission can lower an effect
//! from (task #169 — the last of the three-of-five family).
//!
//! `compiler::timeline::walk_campaign` replays each effect bundle in order and
//! hands every effect the gate regions an *earlier effect in its own bundle*
//! provably sealed. That state is what `DW0410` checks a `move-actor` /
//! `move-npc` against, and `nav::all_effects` is defined as the same walk with
//! the states dropped — so it is also what decides which walks get **planned and
//! emitted at all**.
//!
//! It used to enumerate three effect roots where emission reaches five. A
//! `move-actor` inside a `traps[].payload` or a dialogue option's
//! `set-checkpoint` `on_respawn` bundle was therefore lowered — the payload
//! really does emit `function <ns>:ma_<actor>_<anchor>` — while no planner ever saw it:
//! the walk was never proven, and the function it calls was never generated.
//!
//! Both fixtures below put a `close-gate` and a walk that needs that gate in
//! **one** bundle, rooted where the old walk could not look. The seal is
//! unconditional and precedes the walk in its own list, so the walk is
//! impossible from the moment the bundle runs — `DW0410`, exactly as it is when
//! the same two effects sit in a quest's `on_complete`.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

/// A hello-world `quests` doc carrying the stage-5 puppet plus a caller-supplied
/// raw `traps` array body (no surrounding brackets).
///
/// `actor/ram` stands at `anchor/keeper-stand` (room side, local z=4) and every
/// walk below targets `anchor/exit` (local z=8), reachable only through the
/// `anchor/door` gate region — the island's round-8 geometry in miniature.
///
/// The quest line **opens** `anchor/door` on its first objective, so the player's
/// own forced route is clear and the DAG-causal seal model (`DW0311`) stays
/// silent: a later `open-gate` from a forced root wins the region over an
/// optional root's close (the rule task #167 settled). Anything these fixtures
/// red on is therefore the timeline proof talking.
fn quests_doc(traps: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "actors": [
      {{ "id": "actor/ram", "entity": "minecraft:sheep", "anchor": "anchor/keeper-stand" }}
    ],
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "traps": [ {traps} ]
  }}
}}"#
    )
}

/// A trap whose payload seals `anchor/door` and *then* walks the ram through it.
/// Both effects are in one list, so the seal has provably landed before the walk
/// starts — the plain-effect-list form of the island defect, rooted in a payload.
const TRAP_SEALS_THEN_WALKS: &str = r#"{
  "id": "trap/spring-the-door",
  "at": "anchor/exit",
  "trigger": "trapped-chest",
  "lethality": "harmful",
  "payload": [
    { "type": "close-gate", "anchor": "anchor/door" },
    { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" }
  ]
}"#;

/// The same trap with the two effects the other way round: the ram walks while
/// the door is still open, and the seal lands after. Nothing is proven wrong, so
/// the build must stay clean — the widening is a proof, never a blanket veto on
/// payload-rooted walks.
const TRAP_WALKS_THEN_SEALS: &str = r#"{
  "id": "trap/spring-the-door",
  "at": "anchor/exit",
  "trigger": "trapped-chest",
  "lethality": "harmful",
  "payload": [
    { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" },
    { "type": "close-gate", "anchor": "anchor/door" }
  ]
}"#;

/// A dialogue doc whose option sets a checkpoint whose `on_respawn` bundle seals
/// `anchor/door` and then walks the ram through it. `DialogueEffect` carries no
/// movement verb of its own, which is why the walk stopped at the quests stage —
/// but the bundle is a plain `Vec<QuestEffect>` and is really lowered, into
/// `cp_on_respawn_<i>`.
const DIALOGUE_SEALS_THEN_WALKS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        { "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            { "label": "Open the door, please.",
              "effects": [
                { "type": "complete-objective", "objective": "obj/talk" },
                { "type": "set-checkpoint", "anchor": "anchor/exit",
                  "on_respawn": [
                    { "type": "close-gate", "anchor": "anchor/door" },
                    { "type": "move-actor", "actor": "actor/ram", "to_anchor": "anchor/exit" }
                  ] }
              ] }
          ] }
      ] }
    ]
  }
}"#;

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str, dialogue: Option<&str>) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue
            .map(str::to_string)
            .unwrap_or_else(|| read_hw("dialogue.json")),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

/// The campaign validates cleanly, so any build failure below is a nav proof
/// talking and never a schema slip in the fixture text.
fn assert_validates(c: &Campaign) {
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = common::fenced_diagnostics(c, &items, &prefabs(), &entities);
    assert!(d.is_empty(), "fixture must validate cleanly: {d:#?}");
}

fn try_build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, BuildFailure> {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
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
        &BTreeMap::new(),
        &structures,
        &tree,
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

/// The coded diagnostic a build failed with.
fn failure_code(err: BuildFailure) -> (String, String) {
    match err {
        BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a coded diagnostic, got {other:?}"),
    }
}

/// A walk in a **trap payload**, across a gate the same payload has already
/// sealed, is `DW0410`.
///
/// Red against `origin/main`: the build **succeeds** — `walk_campaign` never
/// looked inside `traps[].payload`, so the walk was neither proven nor planned,
/// and the payload's `function <ns>:ma_<actor>_<anchor>` call had no function behind it.
#[test]
fn a_trap_payload_walk_across_its_own_seal_is_dw0410() {
    let c = parse_hw(&quests_doc(TRAP_SEALS_THEN_WALKS), None);
    assert_validates(&c);
    let Err(err) = try_build(&c, &prefabs()) else {
        panic!("a payload walk across the payload's own seal must fail the timeline proof");
    };
    let (code, message) = failure_code(err);
    assert_eq!(code, "DW0410", "{message}");
    for needle in ["move-actor", "actor/ram", "anchor/door", "close-gate"] {
        assert!(
            message.contains(needle),
            "DW0410 message must name `{needle}`: {message}"
        );
    }
}

/// …and so is one in a dialogue option's `set-checkpoint` `on_respawn` bundle.
///
/// Red against `origin/main`: the build **succeeds** — the old walk stopped at
/// the quests stage entirely, so this bundle was invisible to every nav proof.
#[test]
fn a_dialogue_respawn_walk_across_its_own_seal_is_dw0410() {
    let c = parse_hw(&quests_doc(""), Some(DIALOGUE_SEALS_THEN_WALKS));
    assert_validates(&c);
    let Err(err) = try_build(&c, &prefabs()) else {
        panic!("an on_respawn walk across the bundle's own seal must fail the timeline proof");
    };
    let (code, message) = failure_code(err);
    assert_eq!(code, "DW0410", "{message}");
    for needle in ["move-actor", "actor/ram", "anchor/door", "close-gate"] {
        assert!(
            message.contains(needle),
            "DW0410 message must name `{needle}`: {message}"
        );
    }
}

/// Order is the whole content of the proof: the same payload with the walk
/// **before** the seal builds clean. Seeing a root is not the same as
/// suspecting it.
#[test]
fn a_trap_payload_walk_before_its_seal_builds_clean() {
    let c = parse_hw(&quests_doc(TRAP_WALKS_THEN_SEALS), None);
    assert_validates(&c);
    if let Err(err) = try_build(&c, &prefabs()) {
        panic!(
            "a walk that finishes before the seal lands is legal, got {:?}",
            failure_code(err)
        );
    }
}

/// The payload-rooted walk is not merely *proven* — it is **planned**, which is
/// what makes the `function <ns>:ma_<actor>_<anchor>` the payload emits resolve. Asserts
/// the driver functions exist in the built datapack.
#[test]
fn a_trap_payload_walk_is_planned_and_emitted() {
    let c = parse_hw(&quests_doc(TRAP_WALKS_THEN_SEALS), None);
    let out = match try_build(&c, &prefabs()) {
        Ok(out) => out,
        Err(err) => panic!("builds, got {:?}", failure_code(err)),
    };
    let fire = out
        .iter()
        .find(|(p, _)| p.ends_with("/trap_fire_spring_the_door.mcfunction"))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .expect("the trap's payload function is emitted");
    let called = fire
        .lines()
        .find(|l| l.contains(":ma_"))
        .expect("the payload calls a move-actor driver")
        .trim()
        .to_string();
    let f = called.rsplit(':').next().unwrap().to_string();
    assert!(
        out.keys().any(|p| p.ends_with(&format!("/{f}.mcfunction"))),
        "the payload calls `{f}` — it must be generated, not dangle"
    );
}

// --- the enumeration itself ---------------------------------------------------

/// A campaign exercising **all five** roots at once, each carrying one `narrate`
/// whose text names its root, so the walk's own output states which roots it
/// reached.
const FIVE_ROOT_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "triggers": [
      { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "approach", "range": 3 },
        "effects": [ { "type": "narrate", "style": "chat", "text": "root: trigger" } ] }
    ],
    "traps": [
      { "id": "trap/chest", "at": "anchor/exit", "trigger": "trapped-chest", "lethality": "harmful",
        "payload": [ { "type": "narrate", "style": "chat", "text": "root: trap payload" } ] }
    ],
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "narrate", "style": "chat", "text": "root: objective complete" }
          ]
        },
        "on_complete": [
          { "type": "narrate", "style": "chat", "text": "root: quest complete" },
          { "type": "campaign-complete" }
        ]
      }
    ]
  }
}"#;

const FIVE_ROOT_DIALOGUE: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        { "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            { "label": "Open the door, please.",
              "effects": [
                { "type": "complete-objective", "objective": "obj/talk" },
                { "type": "set-checkpoint", "anchor": "anchor/exit",
                  "on_respawn": [
                    { "type": "narrate", "style": "chat", "text": "root: dialogue respawn" }
                  ] }
              ] }
          ] }
      ] }
    ]
  }
}"#;

/// The staged walk reaches every root emission does, in the one order
/// `plan::for_each_effect_root` fixes. This is the pin that would have caught the
/// gap: it reads the roots off the walk's own output, so a root dropped or
/// reordered is a diff here, not a silent proof hole three consumers deep.
#[test]
fn the_walk_reaches_all_five_roots_in_the_fixed_order() {
    let c = parse_hw(FIVE_ROOT_QUESTS, Some(FIVE_ROOT_DIALOGUE));
    assert_validates(&c);
    let plan = Plan::build(&c, &prefabs()).expect("plan builds");
    let roots: Vec<&str> = delvewright_compiler::timeline::walk(&plan)
        .into_iter()
        .filter_map(|(e, _)| match e {
            delvewright_dsl::QuestEffect::Narrate { text, .. } => text.strip_prefix("root: "),
            _ => None,
        })
        .collect();
    assert_eq!(
        roots,
        vec![
            "objective complete",
            "quest complete",
            "trigger",
            "trap payload",
            "dialogue respawn",
        ]
    );
}
