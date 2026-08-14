//! **A gate the campaign never opens** (`DW0317`) — the world-load seal model.
//!
//! The defect these tests pin is a modelling default, not a missing lint. The
//! occupancy model cleared every gate anchor's region unconditionally, so a gate's
//! state in the static model was a function of what *sealed* it and never of what
//! *opened* it. That default can only fail to notice an obstruction, never invent
//! one — and the mistake an author actually makes is forgetting to open a door.
//!
//! Under that default a campaign missing its one `open-gate` builds with exit
//! **0** while its objective stands physically behind six cells of `iron_bars`,
//! and the runtime bot says *"No path to the goal!"* — a symptom that names
//! nothing. The pair below is that red→green, on the in-repo `hello-world`
//! fixture: `hello-room`'s `anchor/door` bars all six cells of its doorway.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// A hello-world `quests` doc whose party must walk from the keeper to
/// `anchor/exit` — i.e. THROUGH `anchor/door`, six cells of `iron_bars` the
/// prefab authors shut. `opener` is spliced into `on_objective_complete` for the
/// talk beat.
fn quests_doc(opener: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
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
          "obj/talk": [ {opener} ]
        }},
        "on_complete": []
      }}
    ]
  }}
}}"#
    )
}

/// The same campaign with the gate opened only on `on_complete` — after the exit
/// has already been reached.
fn quests_doc_opened_too_late() -> String {
    r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {},
        "on_complete": [ { "type": "open-gate", "anchor": "anchor/door" } ]
      }
    ]
  }
}"#
    .to_string()
}

/// The same campaign whose only `open-gate` lives in the campaign's `on_death`
/// bundle (DSL v0.10 R7) — declared, lowered, and not something the party can be
/// made to do.
fn quests_doc_opened_only_on_death() -> String {
    r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "on_death": [ { "type": "open-gate", "anchor": "anchor/door" } ],
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {},
        "on_complete": []
      }
    ]
  }
}"#
    .to_string()
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).expect("prefab library loads")
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            out.insert(piece.structure_file.clone(), bytes);
        }
    }
    out
}

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, String> {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
    let structures = structures(&plan);
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
    .map_err(|e| format!("{e:?}"))
}

/// Validate first, so a failure below is the nav model's verdict and not a
/// malformed document.
fn validated(quests: &str, prefabs: &PrefabRegistry) -> Campaign {
    let c = parse_hw(quests);
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let d = validate_campaign_with(&c, &items, prefabs, &entities);
    assert!(
        d.is_empty(),
        "the campaign under test must be valid: {d:#?}"
    );
    c
}

/// **The red.** No `open-gate` anywhere: the party is asked to walk through a
/// doorway the prefab bars, and nothing ever lifts the bars.
#[test]
fn a_gate_nothing_opens_blocks_the_critical_path_dw0317() {
    let p = prefabs();
    let c = validated(
        &quests_doc(r#"{ "type": "set-flag", "flag": "flag/told" }"#),
        &p,
    );
    let err = build(&c, &p).expect_err("a delve the party cannot walk must not build");
    assert!(
        err.contains("DW0317"),
        "the missing `open-gate` is DW0317, got: {err}"
    );
    assert!(
        err.contains("anchor/door"),
        "the diagnostic must NAME the gate, not just report an unroutable leg: {err}"
    );
    assert!(
        err.contains("no firing the party is forced to make ever opens it"),
        "and must say what the campaign does to it: {err}"
    );
}

/// **The green.** The identical campaign with the one `open-gate` restored
/// builds — the seal is lifted by the firing that opens it, exactly as a
/// `close-gate` is cancelled by one.
#[test]
fn the_same_campaign_with_its_open_gate_builds() {
    let p = prefabs();
    let c = validated(
        &quests_doc(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
        &p,
    );
    build(&c, &p).expect("an opened gate is passable from the step that opens it");
}

/// **The division of labour with `DW0306`, asserted rather than assumed.** A gate
/// opened *later in the objective order* is the older piece-connectivity proof's
/// case and it gets there first, at plan time, before any world is assembled.
/// `DW0317` is for what that proof structurally cannot see: `DW0306` builds its
/// gate set from the anchors an `open-gate` NAMES
/// (`plan::collect_open_gate_anchors`), so a gate no `open-gate` mentions is not a
/// gate to it at all — which is the same one-directional default, one layer up.
#[test]
fn a_gate_opened_after_the_leg_is_dw0306_the_older_proofs_case() {
    let p = prefabs();
    let c = validated(&quests_doc_opened_too_late(), &p);
    let err = Plan::build(&c, &p)
        .err()
        .expect("a gate opened after the leg does not open it");
    assert_eq!(err.code.id(), "DW0306", "{err:?}");
}

/// **An optional opener is not an opener.** The campaign's only `open-gate` hangs
/// off `on_death` (R7) — a bundle nobody is forced to fire — so the model does not
/// credit it, by the identical rule that keeps every shortcut gate sealed. This
/// also pins the half `DW0306` cannot reach: its gate set is built from quest
/// effects and triggers only, so an `on_death` `open-gate` is invisible to it and
/// the delve compiled clean before this proof existed.
#[test]
fn a_gate_opened_only_from_an_optional_bundle_is_dw0317() {
    let p = prefabs();
    let c = validated(&quests_doc_opened_only_on_death(), &p);
    let err = build(&c, &p).expect_err("an optional firing may seal a region, never open one");
    assert!(err.contains("DW0317"), "{err}");
    assert!(err.contains("anchor/door"), "{err}");
    assert!(
        err.contains("optional bundle"),
        "the message must say WHY the declared `open-gate` did not count: {err}"
    );
}

/// The binding ledger ships with the build and states what it examined
/// (CLAUDE.md: *a green gate that binds to nothing is vacuous, not a pass*).
#[test]
fn the_build_ships_the_gate_seal_binding_ledger() {
    let p = prefabs();
    let c = validated(
        &quests_doc(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
        &p,
    );
    let out = build(&c, &p).expect("builds");
    let bytes = out
        .get("validation/gate-seal.json")
        .expect("a campaign with a gate anchor ships the ledger");
    let j: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    assert_eq!(j["gates_examined"], 1);
    assert_eq!(j["sealed_at_world_load"], 1);
    assert_eq!(j["modelled_as_sealed"], 1);
    assert_eq!(j["unbound"], false);
    assert_eq!(j["gates"][0]["anchor"], "anchor/door");
    assert_eq!(
        j["gates"][0]["blocked_at_world_load"], 6,
        "hello-room bars all six cells of its doorway"
    );
}

/// **Emission does not move.** The world-load seal is a proof-layer fact: it
/// changes what the compiler will *accept*, never what it writes for a campaign it
/// accepts. (The one measured exception in the corpus is spec-0032's stake
/// placement, which is a placement chosen FROM reachability — see the PR and
/// `docs/reference/compiler.md`.)
#[test]
fn a_campaign_that_opens_its_gate_emits_the_same_datapack() {
    let p = prefabs();
    let c = validated(
        &quests_doc(r#"{ "type": "open-gate", "anchor": "anchor/door" }"#),
        &p,
    );
    let out = build(&c, &p).expect("builds");
    // The `open-gate` fill is `replace`-filtered to the anchor's declared block —
    // the command whose ABSENCE is the whole bug, and the reason an `Unseal`
    // cancels the world-load fill rather than clearing the region outright.
    let f = out
        .get("datapack/data/hello-world/function/complete_o_talk.mcfunction")
        .expect("the talk beat's function");
    let text = String::from_utf8(f.clone()).unwrap();
    assert!(
        text.contains("minecraft:air replace minecraft:iron_bars"),
        "the open is a replace-filtered clear: {text}"
    );
}
