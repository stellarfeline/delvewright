//! **Every campaign-wide effect walk sees every effect root** — the sweep that
//! closes the three-of-five family.
//!
//! An *effect root* is a `Vec<QuestEffect>` emission can lower. There are five,
//! and four of them hang off the quests stage while the fifth hangs off dialogue.
//! Every walk that needed "every effect" used to enumerate the roots by hand, and
//! each such copy missed a different subset: six were found and fixed one at a
//! time, by six unrelated investigations, and a sweep afterwards found **thirteen
//! more** still naming three or four of the five.
//!
//! None of them was red. That is the signature of this class: a walk that visits
//! four of five roots produces correct-looking output over any campaign that
//! happens not to use the fifth, so it stays green until content routes an effect
//! through the root it cannot see.
//!
//! The fix is structural — one enumeration (`delvewright_dsl::effects`), inherited
//! by every walk — so most of these assertions would pass for a walk that had
//! merely been patched by hand. What they are here to catch is the *next* root:
//! they are written per walker and per root, so a walk that stops inheriting shows
//! up by name.
//!
//! Companion proofs already in the tree, one walker each: `gate_model_roots`,
//! `timeline_effect_roots`, `flow_effect_roots`, `flag_objective_roots`,
//! `anchor_seal`, `l10n_effect_roots`.

mod common;

use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EffectRootKind, RawCampaign, parse_campaign};

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
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn validate(c: &Campaign) -> Vec<String> {
    delvewright_dsl::validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs(),
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| d.code)
    .collect()
}

/// hello-world's stage 5 with a caller-supplied `content` prelude and a
/// caller-supplied tail on the `obj/exit` objective.
fn quests_doc(prelude: &str, exit_tail: &str) -> String {
    quests_doc_with(prelude, exit_tail, "")
}

/// As [`quests_doc`], plus extra effects appended to `obj/talk`'s
/// `on_objective_complete` bundle (leading comma included) — where a fixture puts
/// a producer at whatever nesting depth it wants to test.
fn quests_doc_with(prelude: &str, exit_tail: &str, talk_tail: &str) -> String {
    quests_doc_versioned("0.6.0", prelude, exit_tail, talk_tail, "")
}

/// As [`quests_doc_with`], with the stage's `dsl_version` and a trailing
/// `content` section (leading comma included) under the caller's control — the
/// v0.10 `on_death` root lives at the content level, not inside a quest.
fn quests_doc_versioned(
    version: &str,
    prelude: &str,
    exit_tail: &str,
    talk_tail: &str,
    content_tail: &str,
) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{talk_tail} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]{content_tail}
  }}
}}"#
    )
}

// ---------------------------------------------------------------------------
// 1. the enumeration itself
// ---------------------------------------------------------------------------

/// The walk enumerates all seven roots on **every** campaign, including one that
/// uses none of them — the distinction the binding ledger exists to draw.
///
/// "This walk reached the root" and "this campaign has a bundle there" are
/// different facts. Conflating them is how a proof reports a vacuous green as a
/// pass (CLAUDE.md): a gate over `traps[].payload` on a campaign with no traps
/// examined zero objects, and its green says nothing whatever about the gate.
#[test]
fn the_walk_enumerates_every_root_and_reports_what_it_bound_to() {
    let c = parse_hw(&quests_doc("", ""), None);
    let binding = delvewright_dsl::for_each_effect_root(&c, &mut |_, _| {});

    assert_eq!(
        binding.roots_enumerated,
        EffectRootKind::COUNT,
        "all {} roots enumerated, on a campaign that uses only two of them",
        EffectRootKind::COUNT
    );
    assert_eq!(
        EffectRootKind::COUNT,
        7,
        "seven roots (spec-0022 + v0.6, then spec-0031's R6 shortcut `on_unlock` \
         and R7 campaign `on_death`)"
    );

    // hello-world has one quest with one `on_objective_complete` bundle and one
    // `on_complete`, and nothing else.
    let n = |k: EffectRootKind| binding.sites.iter().find(|(kk, _)| *kk == k).unwrap().1;
    assert_eq!(n(EffectRootKind::ObjectiveComplete), 1);
    assert_eq!(n(EffectRootKind::QuestComplete), 1);
    assert_eq!(n(EffectRootKind::Trigger), 0);
    assert_eq!(n(EffectRootKind::TrapPayload), 0);
    assert_eq!(n(EffectRootKind::DialogueRespawn), 0);
    assert_eq!(n(EffectRootKind::ShortcutUnlock), 0);
    assert_eq!(n(EffectRootKind::OnDeath), 0);

    // …and the ledger says so out loud, rather than leaving a reader to notice an
    // empty count on their own.
    assert_eq!(
        binding.unbound_roots(),
        vec![
            EffectRootKind::Trigger,
            EffectRootKind::TrapPayload,
            EffectRootKind::DialogueRespawn,
            EffectRootKind::ShortcutUnlock,
            EffectRootKind::OnDeath,
        ],
        "a root this campaign has no bundle at is NAMED as unbound"
    );
    assert!(
        binding.summary().contains("roots 7/7"),
        "{}",
        binding.summary()
    );
}

/// A campaign that exercises all seven roots binds all seven. The control for the
/// test above: without it, "enumerated 7" could be true of a walk that visits seven
/// roots and finds nothing at any of them.
#[test]
fn a_campaign_using_every_root_binds_every_root() {
    let c = parse_hw(
        &quests_doc_versioned("0.10.0", ALL_ROOTS_PRELUDE, "", "", ON_DEATH_TAIL),
        Some(RESPAWN_DIALOGUE),
    );
    let binding = delvewright_dsl::for_each_effect_root(&c, &mut |_, _| {});
    assert!(
        binding.unbound_roots().is_empty(),
        "every root binds: {}",
        binding.summary()
    );
    assert_eq!(binding.roots_enumerated, EffectRootKind::COUNT);
}

/// An `on_death` declared but **empty** binds no root. The distinction matters:
/// R7 is a single campaign-wide list, so a walk that visited it unconditionally
/// would report every campaign in the repo as bound at R7 and the ledger would
/// stop being able to say "this campaign has no death beat" — a vacuous green
/// wearing a binding count (CLAUDE.md).
#[test]
fn an_empty_on_death_binds_nothing() {
    let c = parse_hw(
        &quests_doc_versioned("0.10.0", "", "", "", r#", "on_death": []"#),
        None,
    );
    let binding = delvewright_dsl::for_each_effect_root(&c, &mut |_, _| {});
    assert!(
        binding.unbound_roots().contains(&EffectRootKind::OnDeath),
        "an empty death beat is UNBOUND, not bound-to-nothing: {}",
        binding.summary()
    );
    assert_eq!(binding.roots_enumerated, EffectRootKind::COUNT);
}

/// A trigger, a trap and a shortcut, each with a one-effect bundle.
const ALL_ROOTS_PRELUDE: &str = r#""triggers": [
      { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "approach", "range": 3 },
        "effects": [ { "type": "set-flag", "flag": "flag/woken" } ] }
    ],
    "traps": [
      { "id": "trap/alarm-chest", "at": "anchor/exit", "trigger": "trapped-chest",
        "lethality": "harmful",
        "payload": [ { "type": "set-flag", "flag": "flag/sprung" } ] }
    ],
    "shortcuts": [
      { "id": "shortcut/inner-door", "gate": "anchor/door", "unlock": "anchor/exit",
        "on_unlock": [ { "type": "set-flag", "flag": "flag/opened" } ] }
    ],"#;

/// The campaign's `on_death` bundle — root 7, the only one that is a single list
/// hanging off the stage document itself.
const ON_DEATH_TAIL: &str = r#",
    "on_death": [ { "type": "set-flag", "flag": "flag/fell" } ]"#;

/// A dialogue whose option sets a checkpoint carrying an `on_respawn` bundle —
/// effect root 5, the one that hangs off the dialogue stage.
const RESPAWN_DIALOGUE: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      { "npc": "npc/keeper", "root": "dlg/root",
        "nodes": [
          { "id": "dlg/root", "text": "The door is barred.",
            "options": [
              { "label": "Steady me.",
                "effects": [
                  { "type": "set-checkpoint", "anchor": "anchor/exit",
                    "on_respawn": [ { "type": "set-flag", "flag": "flag/revived" } ] }
                ] }
            ] }
        ] }
    ]
  }
}"#;

// ---------------------------------------------------------------------------
// 2. `dsl::validate`'s flag-producer inventory (`collect_declared_flags`)
// ---------------------------------------------------------------------------

/// **The motivating scenario, red→green.** A trigger gated on a flag whose only
/// producer is nested in a `move-npc`'s `on_arrive` bundle.
///
/// `collect_declared_flags` — the inventory a *trigger's* `requires_flags` is
/// checked against — was both root-narrow (R1–R3) and **shallow**: it did not
/// descend a single nested list. So this flag looked to it like a flag nothing
/// declares, and legitimate content died as `DW0172` while the datapack really
/// does set it.
///
/// This is not hypothetical. `nobodys-cave-island` ships two such flags
/// (`flag/eury-hidden`, `flag/antiphos-posted`, both set inside `on_arrive`
/// bundles); the campaign stayed green only because it happens to gate an
/// *objective* on them rather than a trigger, and objectives are checked against a
/// different, deeper inventory. Three inventories, three answers.
#[test]
fn a_flag_set_only_in_a_nested_bundle_is_declared_for_a_trigger_gate() {
    let c = parse_hw(&quests_doc_with(GATED_TRIGGER, "", NESTED_PRODUCER), None);
    assert_eq!(
        validate(&c),
        Vec::<String>::new(),
        "a `set-flag` in an `on_arrive` bundle is a declared flag: the producer \
         inventory descends nesting now"
    );
}

/// The same flag, produced at the top level instead — the control proving the
/// fixture's gate really is checked at all. Without it, the test above would pass
/// for a check that had simply stopped running.
#[test]
fn the_control_a_top_level_producer_was_always_declared() {
    let c = parse_hw(
        &quests_doc_with(GATED_TRIGGER, "", TOP_LEVEL_PRODUCER),
        None,
    );
    assert_eq!(validate(&c), Vec::<String>::new());
}

/// …and the negative control: no producer anywhere, so the check fires. This is
/// what proves the two tests above are not green because the diagnostic is dead.
#[test]
fn the_negative_control_no_producer_anywhere_is_still_flagged() {
    let c = parse_hw(&quests_doc(GATED_TRIGGER, ""), None);
    assert!(
        validate(&c).contains(&"DW0172".to_string()),
        "a trigger gated on a flag nothing sets is still DW0172: {:?}",
        validate(&c)
    );
}

/// A trigger gated on `flag/posted`. Whether that gate is legitimate depends
/// entirely on whether the producer inventory can see the flag's producer.
const GATED_TRIGGER: &str = r#""triggers": [
      { "id": "trigger/late", "at": "anchor/exit", "on": { "on": "approach", "range": 3 },
        "requires_flags": ["flag/posted"],
        "effects": [ { "type": "narrate", "style": "chat", "text": "Someone is already here." } ] }
    ],"#;

/// The producer, one level down, in a `move-npc`'s `on_arrive` reaction bundle —
/// the exact shape `nobodys-cave-island` ships twice.
const NESTED_PRODUCER: &str = r#", { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
             "on_arrive": [ { "type": "set-flag", "flag": "flag/posted" } ] }"#;

/// The same producer at the top level — the control.
const TOP_LEVEL_PRODUCER: &str = r#", { "type": "set-flag", "flag": "flag/posted" }"#;
