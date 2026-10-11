//! DSL v0.6 `world.difficulty` + actor `attributes`.
//!
//! Difficulty was a compiler constant: `easy` for any campaign with a wave,
//! `peaceful` for one without. Easy halves incoming player damage
//! (`min(dmg / 2 + 1, dmg)`), so "the enemies are too weak" was in part a setting
//! nobody could declare. It is now a stage-1 field — an additive v0.6 surface,
//! absent by default so every
//! earlier campaign keeps the derivation.
//!
//! `peaceful` is refused (`DW0468`): on peaceful the server discards every
//! body whose type is not allowed in peaceful, so the delve's whole cast of
//! threats would silently not exist. Absent, the engine derives the lowest
//! difficulty that keeps every staged body (`delvewright_dsl::derived_difficulty`);
//! a declared fight by a species peaceful keeps, on that derived peaceful, is the
//! advisory `DW0469`.
//!
//! Actor `attributes` is the same v0.4 [`MobAttributes`] shape a wave mob takes,
//! fenced on the stage actors themselves are fenced on.

mod common;

use delvewright_dsl::{
    Campaign, DSL_VERSION, EntityRegistry, RawCampaign, Severity, VendoredAnchorRegistry,
    VendoredItemRegistry, WorldDifficulty, bodies_removed_on_peaceful, check_campaign,
    derived_difficulty, effective_difficulty, parse_campaign, peaceful_despawn,
    removed_on_peaceful, validate_campaign_with,
};

fn raw_with(world: Option<&str>, quests: Option<&str>) -> RawCampaign {
    RawCampaign {
        world: world
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("world.json")),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("quests.json")),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

/// hello-world's world stage at `version` with a declared `difficulty`.
fn world_with_difficulty(value: &str) -> String {
    common::read_valid("world.json")
        .replacen("\"0.2.0\"", &format!("\"{DSL_VERSION}\""), 1)
        .replacen(
            "\"target_minutes\": 5,",
            &format!("\"target_minutes\": 5,\n    \"difficulty\": \"{value}\","),
            1,
        )
}

/// A v0.6 quests document with one scripted actor, spawned and unleashed from the
/// quest's first beat. `attrs` is spliced into the actor object (`""` for none).
fn quests_with_actor(attrs: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-actor", "actor": "actor/giant" }},
            {{ "type": "unleash-actor", "actor": "actor/giant" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
         "anchor": "anchor/keeper-stand"{attrs} }}
    ]
  }}
}}"#
    )
}

fn codes(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw).iter().map(|d| d.code.clone()).collect()
}

// --- the field itself ---------------------------------------------------------

/// The three legal keywords all validate clean on a v0.6 world stage.
#[test]
fn easy_normal_hard_all_validate() {
    for value in ["easy", "normal", "hard"] {
        let raw = raw_with(Some(&world_with_difficulty(value)), None);
        let d = check_campaign(&raw);
        assert!(d.is_empty(), "`{value}` must validate clean: {d:#?}");
    }
}

/// `peaceful` is refused with `DW0468`, and the message must carry the *reason*
/// (hostiles are discarded) — the whole point of spending a code on it rather
/// than letting the schema emit "unknown variant".
#[test]
fn peaceful_is_rejected_with_its_rationale() {
    let raw = raw_with(Some(&world_with_difficulty("peaceful")), None);
    let d = check_campaign(&raw);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0468")
        .expect("peaceful raises DW0468");
    assert_eq!(hit.severity, Severity::Error);
    assert_eq!(hit.stage, "world");
    assert_eq!(hit.path, "/content/difficulty");
    assert!(
        hit.message.contains("discards every")
            && hit.message.contains("hostile")
            && hit.message.contains("ambush"),
        "the diagnostic must explain that every wave/actor/ambush would vanish: {}",
        hit.message
    );
    assert!(
        hit.message.contains("easy") && hit.message.contains("hard"),
        "the diagnostic must prescribe the legal keywords: {}",
        hit.message
    );
}

/// A campaign that declares nothing is unchanged — no diagnostic, no field.
#[test]
fn absent_difficulty_is_clean() {
    assert!(check_campaign(&raw_with(None, None)).is_empty());
}

// --- the derivation keeps every staged body -------------------------------------

/// A v0.6 quests document with one actor of `entity`, spawned on the first beat;
/// `unleash` adds an `unleash-actor` after it, `extra` is spliced into the actor.
fn quests_with_one_actor(entity: &str, unleash: bool, extra: &str) -> String {
    let unleash = if unleash {
        r#", { "type": "unleash-actor", "actor": "actor/figure" }"#
    } else {
        ""
    };
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-actor", "actor": "actor/figure" }}{unleash}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/figure", "entity": "{entity}", "anchor": "anchor/keeper-stand"{extra} }}
    ]
  }}
}}"#
    )
}

fn parsed(raw: &RawCampaign) -> Campaign {
    parse_campaign(raw).unwrap_or_else(|d| panic!("fixture parses: {d:#?}"))
}

/// The issue's shape: a still husk staged as a figure — never unleashed, never
/// `vulnerable` — in a campaign with no waves. Peaceful discards a husk, so the
/// derivation must not be peaceful.
#[test]
fn a_still_hostile_actor_derives_easy() {
    let c = parsed(&raw_with(
        None,
        Some(&quests_with_one_actor("minecraft:husk", false, "")),
    ));
    assert_eq!(derived_difficulty(&c), WorldDifficulty::Easy);
    assert_eq!(effective_difficulty(&c), WorldDifficulty::Easy);
    let removed = bodies_removed_on_peaceful(&c);
    assert_eq!(removed.len(), 1, "{removed:#?}");
    assert_eq!(removed[0].id, "actor/figure");
    assert_eq!(removed[0].entity, "minecraft:husk");
}

/// A body of a species peaceful keeps leaves the derivation at peaceful.
#[test]
fn a_still_peaceful_kept_actor_derives_peaceful() {
    let c = parsed(&raw_with(
        None,
        Some(&quests_with_one_actor("minecraft:villager", false, "")),
    ));
    assert_eq!(derived_difficulty(&c), WorldDifficulty::Peaceful);
    assert!(bodies_removed_on_peaceful(&c).is_empty());
}

/// A skinned actor ships as a mannequin, which peaceful keeps — until it is
/// unleashed, when its real-AI twin is the declared entity.
#[test]
fn a_skinned_actor_counts_its_twin_only_when_unleashed() {
    let skin = r#", "skin": { "texture_id": "figure", "model": "wide" }"#;
    let still = parsed(&raw_with(
        None,
        Some(&quests_with_one_actor("minecraft:husk", false, skin)),
    ));
    assert_eq!(derived_difficulty(&still), WorldDifficulty::Peaceful);
    let woken = parsed(&raw_with(
        None,
        Some(&quests_with_one_actor("minecraft:husk", true, skin)),
    ));
    assert_eq!(derived_difficulty(&woken), WorldDifficulty::Easy);
    let removed = bodies_removed_on_peaceful(&woken);
    assert_eq!(removed.len(), 1, "{removed:#?}");
    assert_eq!(removed[0].what, "actor-twin");
}

/// An NPC is a staged body too: a zombie villager keeper derives `easy`.
#[test]
fn a_hostile_npc_derives_easy() {
    let mut raw = raw_with(None, None);
    assert_eq!(
        raw.npcs.matches("\"minecraft:villager\"").count(),
        1,
        "the fixture has one villager NPC to swap"
    );
    raw.npcs = raw
        .npcs
        .replace("\"minecraft:villager\"", "\"minecraft:zombie_villager\"");
    assert_eq!(derived_difficulty(&parsed(&raw)), WorldDifficulty::Easy);
    assert_eq!(
        derived_difficulty(&parsed(&raw_with(None, None))),
        WorldDifficulty::Peaceful
    );
}

/// A volley projectile is a staged body: peaceful discards a shulker bullet.
#[test]
fn a_shulker_bullet_volley_derives_easy() {
    let mut c = parsed(&raw_with(None, None));
    assert_eq!(derived_difficulty(&c), WorldDifficulty::Peaceful);
    let volley: delvewright_dsl::QuestEffect = serde_json::from_str(
        r#"{ "type": "volley", "projectile": "minecraft:shulker_bullet",
             "from_anchor": "anchor/keeper-stand",
             "kill_zone": { "anchor": "anchor/exit", "extent": [1, 1, 1] } }"#,
    )
    .expect("volley parses");
    c.quests.content.quests[0]
        .on_complete
        .insert(0, volley.clone());
    assert_eq!(derived_difficulty(&c), WorldDifficulty::Easy);
    // ...and an arrow volley does not.
    let mut arrow = volley;
    if let delvewright_dsl::Verb::Volley { projectile, .. } = &mut arrow.verb {
        *projectile = Some("minecraft:arrow".to_string());
    }
    c.quests.content.quests[0].on_complete[0] = arrow;
    assert_eq!(derived_difficulty(&c), WorldDifficulty::Peaceful);
}

/// A declaration wins over the derivation.
#[test]
fn a_declaration_wins_over_the_derivation() {
    let c = parsed(&raw_with(
        Some(&world_with_difficulty("hard")),
        Some(&quests_with_one_actor("minecraft:husk", false, "")),
    ));
    assert_eq!(effective_difficulty(&c), WorldDifficulty::Hard);
}

/// The vendored table answers what the pinned jar answers for the bodies the
/// derivation's reasoning names.
#[test]
fn the_peaceful_table_holds_the_jar_facts() {
    for id in [
        "husk",
        "minecraft:zombie",
        "minecraft:shulker_bullet",
        "minecraft:wither",
    ] {
        assert!(removed_on_peaceful(id), "{id} is discarded on peaceful");
    }
    for id in [
        "minecraft:villager",
        "minecraft:mannequin",
        "minecraft:iron_golem",
        "minecraft:ender_dragon",
        "minecraft:piglin",
        "minecraft:arrow",
    ] {
        assert!(!removed_on_peaceful(id), "{id} survives peaceful");
    }
    assert_eq!(
        peaceful_despawn().len(),
        38,
        "the 1.21.11 table has 38 rows"
    );
}

// --- DW0469: a declared fight on the derived peaceful ---------------------------

/// Accepts every entity id: the DSL tier's vendored entity subset holds only
/// hostile species, and `DW0469` is about a fighter peaceful keeps.
struct AnyEntity;
impl EntityRegistry for AnyEntity {
    fn contains(&self, _: &str) -> bool {
        true
    }
}

fn diagnostics_any_entity(raw: &RawCampaign) -> Vec<delvewright_dsl::Diagnostic> {
    validate_campaign_with(
        &parsed(raw),
        &VendoredItemRegistry::v1_21_11(),
        &VendoredAnchorRegistry::hello_world(),
        &AnyEntity,
    )
}

/// An unleashed fighter of a species peaceful keeps, no waves, no declaration:
/// the derivation is peaceful, and on peaceful its blows on a player scale to
/// zero. Advisory.
#[test]
fn a_peaceful_kept_fighter_on_the_derived_peaceful_warns() {
    let raw = raw_with(
        None,
        Some(&quests_with_one_actor("minecraft:iron_golem", true, "")),
    );
    let d = diagnostics_any_entity(&raw);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0469")
        .unwrap_or_else(|| panic!("a fighter on the derived peaceful must warn: {d:#?}"));
    assert_eq!(hit.severity, Severity::Warning);
    assert_eq!(hit.path, "/content/difficulty");
    assert!(
        hit.message.contains("peaceful")
            && hit.message.contains("actor/figure (minecraft:iron_golem)"),
        "the warning names the setting and the fighter: {}",
        hit.message
    );
    assert!(
        d.iter().all(|x| x.severity == Severity::Warning),
        "DW0469 must not fail the run: {d:#?}"
    );
}

/// A fighter peaceful discards moves the derivation to `easy`, so there is no
/// peaceful to warn about.
#[test]
fn a_hostile_fighter_derives_easy_and_does_not_warn() {
    let raw = raw_with(None, Some(&quests_with_actor("")));
    assert_eq!(derived_difficulty(&parsed(&raw)), WorldDifficulty::Easy);
    assert!(!codes(&raw).contains(&"DW0469".to_string()));
}

/// Declaring a difficulty settles the question — the warning is gone.
#[test]
fn declared_difficulty_silences_the_actor_warning() {
    let raw = raw_with(
        Some(&world_with_difficulty("normal")),
        Some(&quests_with_one_actor("minecraft:iron_golem", true, "")),
    );
    assert!(
        !diagnostics_any_entity(&raw)
            .iter()
            .any(|x| x.code == "DW0469"),
        "a declared difficulty answers the question DW0469 asks"
    );
}

/// A campaign with no actors never sees the warning, whatever its difficulty.
#[test]
fn no_actors_means_no_warning() {
    assert!(!codes(&raw_with(None, None)).contains(&"DW0469".to_string()));
}

// --- actor attributes ---------------------------------------------------------

/// Actor `attributes` takes the wave-mob shape and validates clean under v0.6.
#[test]
fn actor_attributes_validate_under_v06() {
    let attrs =
        r#", "attributes": { "max_health": 200.0, "attack_damage": 12.0, "follow_range": 24.0 }"#;
    let raw = raw_with(
        Some(&world_with_difficulty("hard")),
        Some(&quests_with_actor(attrs)),
    );
    let d = check_campaign(&raw);
    assert!(d.is_empty(), "actor attributes must validate clean: {d:#?}");
}

/// An unknown attribute name is a schema rejection, not a silently-ignored field
/// — the shape is `deny_unknown_fields`, shared with the wave-mob surface.
#[test]
fn unknown_actor_attribute_is_a_schema_error() {
    let attrs = r#", "attributes": { "armor_toughness": 8.0 }"#;
    let raw = raw_with(
        Some(&world_with_difficulty("hard")),
        Some(&quests_with_actor(attrs)),
    );
    assert!(
        codes(&raw).contains(&"DW0100".to_string()),
        "an attribute the DSL does not expose must be rejected, not dropped"
    );
}
