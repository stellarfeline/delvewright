//! The branch/flag **flow** model sees every root emission can lower an effect
//! from (the fourth and last of the three-of-five family).
//!
//! `compiler::flow` is the completability proof: `DW0201`/`DW0202`/`DW0203`
//! reachability, the `DW0204` path replay, the `DW0205` skip walk, and the
//! critical path the compiler exports. It reads the campaign twice —
//!
//! * **producers** (`Flow::new`): every `set-flag` the model may credit, and
//! * **readers** (`flow::gate_flags`): every flag a gate anywhere reads, which
//!   is what decides whether a dialogue choice group is enumerated as XOR
//!   branch worlds or left unconstrained
//!
//! — and both halves enumerated **three** effect roots where emission reaches
//! five. A `set-flag` in a `traps[].payload` was invisible, so an objective
//! gated on it died as a spurious `DW0203` while the datapack really did set the
//! flag; and a `requires_flags` inside a payload was invisible too, so a branch
//! choice that only such a gate reads never split the worlds.
//!
//! The two halves interlock, which is why they land together. The reader half is
//! inert on its own — before the producer half, nothing downstream of a payload
//! was modelled at all — and the producer half **alone** makes the branch
//! fixture below worse than it started: one unconstrained world holds both
//! mutually exclusive branch flags at once, the payload produces both, the
//! finale "completes", and the real finding (two endings that cannot both be
//! played) resurfaces as a `DW0204` complaint about the exported path.
//!
//! Policy per root is stated once, in `Flow::new`, and follows the precedents
//! `flow` already had:
//!
//! * a `traps[].payload` is **ambient**, gated on the trap's own
//!   `requires_flags` — exactly what an environment trigger's `effects` and a
//!   trap's `disarm.sets_flag` already are, and for the same reason: the party
//!   can always go and spring it;
//! * a dialogue-hosted `set-checkpoint` `on_respawn` bundle is a **reaction
//!   bundle**, so it is never a producer — the stance `collect_flags` already
//!   takes for the identical bundle rooted in the quests stage.

mod common;

use std::collections::BTreeSet;

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::flow::gate_flags;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

// ---------------------------------------------------------------------------
// fixture plumbing
// ---------------------------------------------------------------------------

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The campaign validates cleanly, so any `DW02xx` below is the flow model
/// talking and never a schema slip in the fixture text.
fn assert_validates(c: &Campaign) {
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = delvewright_dsl::validate_campaign_with(c, &items, &prefabs(), &entities);
    assert!(d.is_empty(), "fixture must validate cleanly: {d:#?}");
}

/// The reachability codes, in report order.
fn codes(c: &Campaign) -> Vec<String> {
    analyze_campaign(c, &prefabs())
        .into_iter()
        .map(|d| d.code)
        .collect()
}

/// hello-world's stage 5 with a caller-supplied `content` prelude (`triggers` /
/// `traps` arrays, trailing comma included) and a caller-supplied tail on the
/// `obj/exit` objective (`, "requires_flags": [...]`).
fn quests_doc(prelude: &str, exit_tail: &str) -> String {
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
             "radius": 2, "after": ["obj/talk"]{exit_tail} }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// `obj/exit` is gated on a flag nothing in the quests stage sets.
const EXIT_NEEDS_ALARM: &str = r#", "requires_flags": ["flag/alarm"]"#;

/// The flag's only producer is a **trap payload** (spec-0022) — an effect root
/// the model could not see.
const ALARM_FROM_TRAP_PAYLOAD: &str = r#""traps": [
      { "id": "trap/alarm-chest", "at": "anchor/exit", "trigger": "trapped-chest",
        "lethality": "harmful",
        "payload": [ { "type": "set-flag", "flag": "flag/alarm" } ] }
    ],"#;

/// The same producer in an **environment trigger** — the precedent the payload
/// follows. Player-initiated, ambient, no DAG position.
const ALARM_FROM_TRIGGER: &str = r#""triggers": [
      { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "approach", "range": 3 },
        "effects": [ { "type": "set-flag", "flag": "flag/alarm" } ] }
    ],"#;

/// The same producer in a **dialogue-hosted `on_respawn` reaction bundle**. It
/// fires only when somebody dies, so it is not a producer — the rule
/// `collect_flags` already applies to the identical bundle in the quests stage,
/// and the one the `DW0203` message itself states.
const ALARM_FROM_DIALOGUE_RESPAWN: &str = r#"{
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
                  "on_respawn": [ { "type": "set-flag", "flag": "flag/alarm" } ] }
              ] }
          ] }
      ] }
    ]
  }
}"#;

// ---------------------------------------------------------------------------
// the producer half
// ---------------------------------------------------------------------------

/// A flag whose only producer is a `traps[].payload` **is** producible.
///
/// Red against `origin/main`: `DW0203` + `DW0201` — the model never looked
/// inside a payload, so legitimate content died as unreachable while the
/// emitted `trap_fire_alarm_chest.mcfunction` really does set the flag.
#[test]
fn a_flag_set_only_in_a_trap_payload_is_producible() {
    let c = parse_hw(&quests_doc(ALARM_FROM_TRAP_PAYLOAD, EXIT_NEEDS_ALARM), None);
    assert_validates(&c);
    assert!(
        codes(&c).is_empty(),
        "a trap payload is a producer — the party can always go and spring it: {:?}",
        codes(&c)
    );
}

/// The precedent it follows, stated as a test: the identical `set-flag` in an
/// environment trigger has always been producible. Green throughout — the
/// widening does not invent a stance, it applies the one already there to the
/// root that was missing.
#[test]
fn an_environment_trigger_is_the_precedent_the_payload_follows() {
    let c = parse_hw(&quests_doc(ALARM_FROM_TRIGGER, EXIT_NEEDS_ALARM), None);
    assert_validates(&c);
    assert!(codes(&c).is_empty(), "{:?}", codes(&c));
}

/// …and the stance the widening must NOT reach: a `set-flag` in a dialogue
/// option's `set-checkpoint` `on_respawn` bundle stays a non-producer. Reaction
/// bundles fire at statically unknowable times; nobody is forced to die.
///
/// Red-both-ways on purpose (`DW0203` before and after): seeing a root is not
/// the same as crediting it. This is the pin that would catch the widening
/// over-reaching into root 5.
///
/// The fixture used to also fail validation with a `DW0172`, and that assertion
/// was carried here as the **record of a second, separate blind spot**: the DSL
/// layer's own producer inventory (`dsl::validate`) enumerated four of the five
/// roots, so a `set-flag` in a dialogue-hosted `on_respawn` bundle looked to it
/// like a flag nothing declares. The effect-root sweep closed that one, so the
/// fixture now validates clean — and this assertion is the red→green
/// demonstration of it, kept as an equality (not a `!contains`) so a regression
/// there is a named failure rather than a silent one.
///
/// **The two halves say different things and must not be collapsed.** Validating
/// clean means the DSL layer now SEES the root. The `DW0203`/`DW0201` below mean
/// `flow` sees it and deliberately does not CREDIT it. Seeing a root and crediting
/// it are different decisions, and this is the pin that keeps the second from
/// riding along with the first.
#[test]
fn a_dialogue_respawn_bundle_is_seen_but_still_never_a_producer() {
    let c = parse_hw(
        &quests_doc("", EXIT_NEEDS_ALARM),
        Some(ALARM_FROM_DIALOGUE_RESPAWN),
    );
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let validation: Vec<String> =
        delvewright_dsl::validate_campaign_with(&c, &items, &prefabs(), &entities)
            .into_iter()
            .map(|d| d.code)
            .collect();
    assert_eq!(
        validation,
        Vec::<String>::new(),
        "the DSL producer inventory reaches root 5 now: a `set-flag` in a dialogue \
         `on_respawn` bundle is a declared flag, not a `DW0172`"
    );

    let got = codes(&c);
    assert!(
        got.contains(&"DW0203".to_string()) && got.contains(&"DW0201".to_string()),
        "an `on_respawn` bundle is SEEN by the root walk but is still not a producer \
         — reaction bundles fire at statically unknowable times: {got:?}"
    );
}

// ---------------------------------------------------------------------------
// the reader half
// ---------------------------------------------------------------------------

/// A trap payload whose two `set-flag`s are gated on the two alternatives of one
/// dialogue choice, and one mainline objective behind each produced flag. A real
/// player takes exactly ONE of the two options, so the finale can never complete
/// — `DW0201`, and nothing else.
const BRANCHED_PAYLOAD_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "traps": [
      { "id": "trap/brazier", "at": "anchor/exit", "trigger": "trapped-chest",
        "lethality": "harmful",
        "payload": [
          { "type": "set-flag", "flag": "flag/lit",  "requires_flags": ["flag/lantern"] },
          { "type": "set-flag", "flag": "flag/tied", "requires_flags": ["flag/rope"] }
        ] }
    ],
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/burn", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/lit"] },
          { "type": "reach-anchor", "id": "obj/climb", "anchor": "anchor/keeper-stand",
            "radius": 2, "after": ["obj/talk"], "requires_flags": ["flag/tied"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

const BRANCHED_PAYLOAD_DIALOGUE: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
        { "id": "dlg/greeting",
          "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
          "options": [
            { "label": "I take the lantern from the wall.",
              "effects": [ { "type": "set-flag", "flag": "flag/lantern" } ] },
            { "label": "I take the rope from the wall.",
              "effects": [ { "type": "set-flag", "flag": "flag/rope" } ] },
            { "label": "Open the door, please.",
              "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] }
          ] }
      ] }
    ]
  }
}"#;

/// A gate **inside a trap payload** puts its flag into the branch model.
///
/// The choice group's two flags are read nowhere else, so with the narrow
/// inventory the group never split the worlds: one unconstrained world held
/// `flag/lantern` AND `flag/rope`, the payload produced both `flag/lit` and
/// `flag/tied`, and a campaign no player can finish analyzed **clean**.
///
/// Three states, and the middle one is why the two halves land together:
///
/// | model | verdict |
/// |---|---|
/// | `origin/main` | `DW0203` ×2 + `DW0201` — right answer, wrong reason (the payload produced nothing at all) |
/// | producer half only | `DW0204` — the finale is believed completable, a path is exported, and the replay then chokes on it: the finding misdiagnosed as an export defect |
/// | both halves | `DW0201` alone: each objective IS completable, on its own branch; the finale needs both, and no branch has both |
#[test]
fn a_payload_gate_splits_the_branch_worlds() {
    let c = parse_hw(BRANCHED_PAYLOAD_QUESTS, Some(BRANCHED_PAYLOAD_DIALOGUE));
    assert_validates(&c);
    assert_eq!(
        codes(&c),
        vec!["DW0201"],
        "each objective is reachable on its own branch, and the finale needs both"
    );
}

// ---------------------------------------------------------------------------
// the enumeration itself
// ---------------------------------------------------------------------------

/// A campaign carrying one `requires_flags` gate at each of the five effect
/// roots, each naming a flag after its root.
const FIVE_ROOT_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "triggers": [
      { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "approach", "range": 3 },
        "effects": [ { "type": "narrate", "style": "chat", "text": "The moor stirs.",
                       "requires_flags": ["flag/root-trigger"] } ] }
    ],
    "traps": [
      { "id": "trap/chest", "at": "anchor/exit", "trigger": "trapped-chest",
        "lethality": "harmful",
        "payload": [ { "type": "narrate", "style": "chat", "text": "The lid slams.",
                       "requires_flags": ["flag/root-trap"] } ] }
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
            { "type": "narrate", "style": "chat", "text": "The bar lifts.",
              "requires_flags": ["flag/root-objective"] }
          ]
        },
        "on_complete": [
          { "type": "narrate", "style": "chat", "text": "The moor takes the keep back.",
            "requires_flags": ["flag/root-quest"] },
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
                  "on_respawn": [ { "type": "narrate", "style": "chat", "text": "You wake by the gate.",
                                    "requires_flags": ["flag/root-respawn"] } ] }
              ] }
          ] }
      ] }
    ]
  }
}"#;

/// The gate-flag inventory reaches every root emission does. Reads the roots off
/// the inventory's own output, so a root dropped or an inventory re-hand-rolled
/// is a diff here rather than a silent precision hole in the branch model.
#[test]
fn the_gate_flag_inventory_reaches_all_five_roots() {
    let c = parse_hw(FIVE_ROOT_QUESTS, Some(FIVE_ROOT_DIALOGUE));
    let seen: BTreeSet<String> = gate_flags(&c)
        .into_iter()
        .filter(|f| f.starts_with("flag/root-"))
        .collect();
    let want: BTreeSet<String> = [
        "flag/root-objective",
        "flag/root-quest",
        "flag/root-trigger",
        "flag/root-trap",
        "flag/root-respawn",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(seen, want);
}
