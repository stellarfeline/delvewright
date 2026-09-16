//! `DW0898` — a piece is declared where the pinned game will show it, on that
//! body (spec-0067 §4.2, §5).
//!
//! Each case dresses one body in the hello-world campaign and reads the
//! diagnostics the validation stage raises, with the compiler's full item and
//! entity registries injected exactly as `delvec validate` injects them.

mod common;

use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, Diagnostic, EquipmentBinding, parse_campaign};

/// hello-world with `actors` and `waves` replaced.
fn campaign(actors: serde_json::Value, waves: serde_json::Value) -> Campaign {
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let mut raw = loaded.raw;
    let mut quests: serde_json::Value = serde_json::from_str(&raw.quests).unwrap();
    quests["content"]["actors"] = actors;
    quests["content"]["waves"] = waves;
    raw.quests = serde_json::to_string_pretty(&quests).unwrap();
    parse_campaign(&raw).expect("the dressed campaign parses")
}

fn actor(entity: &str, equipment: serde_json::Value) -> serde_json::Value {
    serde_json::json!([{
        "id": "actor/dressed",
        "entity": entity,
        "anchor": "anchor/altar",
        "equipment": equipment
    }])
}

/// Every `DW0898` the validation stage raises.
fn fit(c: &Campaign) -> Vec<Diagnostic> {
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    delvewright_dsl::validate_campaign_with(c, &items, &prefabs, &entities)
        .into_iter()
        .filter(|d| d.code == "DW0898")
        .collect()
}

fn one(c: &Campaign) -> Diagnostic {
    let d = fit(c);
    assert_eq!(d.len(), 1, "exactly one DW0898: {d:#?}");
    d.into_iter().next().unwrap()
}

/// Criterion 8, shape 1: the body does not show the piece.
#[test]
fn a_body_that_does_not_draw_the_slot_is_refused() {
    // A chestplate on a horse names the slots a horse draws.
    let d = one(&campaign(
        actor(
            "minecraft:horse",
            serde_json::json!({"chest": "minecraft:iron_chestplate"}),
        ),
        serde_json::json!([]),
    ));
    assert_eq!(d.path, "/content/actors/0/equipment/chest");
    assert!(
        d.message.contains("the body does not show it"),
        "{}",
        d.message
    );
    assert!(
        d.message.contains("`body` (animal)") && d.message.contains("`saddle` (animal)"),
        "names the slots a horse draws: {}",
        d.message
    );

    // A sword in a creeper's hand: the creeper draws nothing.
    let d = one(&campaign(
        actor(
            "minecraft:creeper",
            serde_json::json!({"main_hand": "minecraft:iron_sword"}),
        ),
        serde_json::json!([]),
    ));
    assert!(
        d.message.contains("draws no equipment slot at all"),
        "{}",
        d.message
    );
    assert!(d.message.contains("`attributes`"), "{}", d.message);

    // A villager draws a head ITEM, not head armour.
    let d = one(&campaign(
        actor(
            "minecraft:villager",
            serde_json::json!({"head": "minecraft:iron_helmet"}),
        ),
        serde_json::json!([]),
    ));
    assert!(d.message.contains("kind armour"), "{}", d.message);
    assert!(
        fit(&campaign(
            actor(
                "minecraft:villager",
                serde_json::json!({"head": "minecraft:carved_pumpkin"}),
            ),
            serde_json::json!([]),
        ))
        .is_empty(),
        "a pumpkin on a villager's head is drawn"
    );

    // A body that is not a living entity is refused on every slot.
    let d = fit(&campaign(
        actor(
            "minecraft:oak_boat",
            serde_json::json!({
                "head": "minecraft:carved_pumpkin",
                "main_hand": "minecraft:stick",
                "body": "minecraft:white_carpet"
            }),
        ),
        serde_json::json!([]),
    ));
    assert_eq!(d.len(), 3, "{d:#?}");
    assert!(
        d.iter()
            .all(|d| d.message.contains("is not a living entity")),
        "{d:#?}"
    );
}

/// Criterion 9, shape 2: the item declares another slot; the hands are exempt.
#[test]
fn an_item_in_a_slot_it_does_not_declare_is_refused() {
    let d = one(&campaign(
        actor(
            "minecraft:zombie",
            serde_json::json!({"legs": "minecraft:diamond_helmet"}),
        ),
        serde_json::json!([]),
    ));
    assert!(
        d.message.contains("declares the `head` slot"),
        "{}",
        d.message
    );
    assert!(
        fit(&campaign(
            actor(
                "minecraft:zombie",
                serde_json::json!({"main_hand": "minecraft:diamond_helmet"}),
            ),
            serde_json::json!([]),
        ))
        .is_empty(),
        "a held helmet is a held helmet"
    );
}

/// Criterion 10, shape 3: the item's allowed entities exclude the body.
#[test]
fn an_item_that_excludes_the_body_is_refused() {
    // A saddle on a zombie: shapes 1 and 3 in one message, the eleven admitted
    // types spelled out.
    let d = one(&campaign(
        actor(
            "minecraft:zombie",
            serde_json::json!({"saddle": "minecraft:saddle"}),
        ),
        serde_json::json!([]),
    ));
    assert!(d.message.contains("(2 shape(s))"), "{}", d.message);
    assert!(
        d.message.contains("the body does not show it"),
        "{}",
        d.message
    );
    assert!(d.message.contains("the wrong body"), "{}", d.message);
    for admitted in [
        "minecraft:camel",
        "minecraft:camel_husk",
        "minecraft:donkey",
        "minecraft:horse",
        "minecraft:mule",
        "minecraft:nautilus",
        "minecraft:pig",
        "minecraft:skeleton_horse",
        "minecraft:strider",
        "minecraft:zombie_horse",
        "minecraft:zombie_nautilus",
    ] {
        assert!(
            d.message.contains(&format!("`{admitted}`")),
            "names {admitted}: {}",
            d.message
        );
    }

    // Horse armour on a skeleton horse: the body draws `horse_body`, the item
    // admits it not — shape 3 alone.
    let d = one(&campaign(
        actor(
            "minecraft:skeleton_horse",
            serde_json::json!({"body": "minecraft:iron_horse_armor"}),
        ),
        serde_json::json!([]),
    ));
    assert!(d.message.contains("(1 shape(s))"), "{}", d.message);
    assert!(d.message.contains("the wrong body"), "{}", d.message);

    // The saddle on a horse is green.
    assert!(
        fit(&campaign(
            actor(
                "minecraft:horse",
                serde_json::json!({"body": "minecraft:iron_horse_armor", "saddle": "minecraft:saddle"}),
            ),
            serde_json::json!([]),
        ))
        .is_empty()
    );

    // A skinned actor is judged as the mannequin it ships as: head armour on a
    // skinned villager is drawn.
    let mut skinned = actor(
        "minecraft:villager",
        serde_json::json!({"head": "minecraft:iron_helmet"}),
    );
    skinned[0]["skin"] = serde_json::json!({"texture_id": "dressed", "model": "wide"});
    assert!(
        fit(&campaign(skinned, serde_json::json!([]))).is_empty(),
        "a skinned actor is a mannequin, which draws head armour"
    );
}

/// Criterion 11: an item the registry says nothing about, in a slot the body
/// draws for it, is not judged.
#[test]
fn an_undeclared_item_in_a_drawn_slot_is_silent() {
    assert!(
        fit(&campaign(
            actor(
                "minecraft:zombie",
                serde_json::json!({"head": "minecraft:carved_pumpkin"}),
            ),
            serde_json::json!([]),
        ))
        .is_empty()
    );
    assert!(
        fit(&campaign(
            actor(
                "minecraft:zombie",
                serde_json::json!({"head": "minecraft:stone"})
            ),
            serde_json::json!([]),
        ))
        .is_empty()
    );
}

/// Wave mobs wear the same type and meet the same rule.
#[test]
fn a_wave_mob_meets_the_same_rule() {
    let d = one(&campaign(
        serde_json::json!([]),
        serde_json::json!([{
            "id": "wave/w",
            "anchor": "anchor/altar",
            "mobs": [{
                "entity": "minecraft:spider",
                "count": 1,
                "equipment": {"head": "minecraft:iron_helmet"}
            }]
        }]),
    ));
    assert_eq!(d.path, "/content/waves/0/mobs/0/equipment/head");
}

/// Criterion 13: a drop names a `saddle` the body wears; without one it is
/// `DW0490`.
#[test]
fn a_saddle_drop_needs_a_saddle() {
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let with = |equipment: serde_json::Value| {
        let mut a = actor("minecraft:horse", equipment);
        a[0]["tier"] = serde_json::json!("boss");
        a[0]["vulnerable"] = serde_json::json!(true);
        a[0]["drops"] = serde_json::json!([{"slot": "saddle"}]);
        let c = campaign(a, serde_json::json!([]));
        delvewright_dsl::validate_campaign_with(&c, &items, &prefabs, &entities)
            .into_iter()
            .filter(|d| d.code == "DW0490")
            .count()
    };
    assert_eq!(with(serde_json::json!({"saddle": "minecraft:saddle"})), 0);
    assert_eq!(
        with(serde_json::json!({"body": "minecraft:iron_horse_armor"})),
        1
    );
}

/// The binding line states what the rule examined (spec-0067 §5).
#[test]
fn the_binding_counts_what_the_rule_examined() {
    let items = FullItemRegistry::v1_21_11();
    let c = campaign(
        actor(
            "minecraft:horse",
            serde_json::json!({
                "body": "minecraft:iron_horse_armor",
                "saddle": "minecraft:saddle",
                "chest": "minecraft:iron_chestplate"
            }),
        ),
        serde_json::json!([{
            "id": "wave/w",
            "anchor": "anchor/altar",
            "mobs": [{"entity": "minecraft:zombie", "count": 1, "equipment": {"head": "minecraft:stone"}}]
        }]),
    );
    let b = EquipmentBinding::of(&c, &items);
    assert_eq!(
        b.line(),
        "equipment binding: 2 body(ies) dressed over 2 entity type(s), 4 piece(s) declared over 4 \
         slot(s) in use, 3 piece(s) with a registry-declared slot, 2 with an allowed-entity list, \
         1 refused (DW0898)."
    );
    let empty = campaign(serde_json::json!([]), serde_json::json!([]));
    assert_eq!(
        EquipmentBinding::of(&empty, &items).line(),
        "equipment binding: 0 body(ies) dressed over 0 entity type(s), 0 piece(s) declared over 0 \
         slot(s) in use, 0 piece(s) with a registry-declared slot, 0 with an allowed-entity list, \
         0 refused (DW0898)."
    );
}
