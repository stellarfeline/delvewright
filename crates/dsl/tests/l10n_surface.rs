//! **Every string field in the DSL is classified, or CI is red.**
//!
//! spec-0029's `DW0185` closes one half of "no player-visible string ships as an
//! untranslatable literal": a string the inventory *knows about* enters emission
//! carrying its key, and an emitter that fails to lower it into a component leaks
//! the tag into the built tree, where the compiler's own output scan fails the
//! build.
//!
//! It cannot close the other half. A string the inventory **never saw** is never
//! tagged, so it leaks nothing, trips nothing, and ships English in a fully
//! translated delve — silently, for as long as nobody looks. That is exactly how
//! `actors[].name` shipped: the island's giant stood still under his Chinese name
//! and walked into a cutscene under an English one, and the sheep were English in
//! every frame, for twenty-odd playtest rounds.
//!
//! This file is that half. It enumerates every string-valued property of the seven
//! stage schemas — which are **derived from the Rust types**, so the enumeration
//! is complete by construction rather than by anybody's diligence — and requires
//! each to carry an explicit classification. Adding a `String` field anywhere in
//! the DSL therefore fails this test until somebody says, in writing, whether a
//! player reads it.
//!
//! It is a test rather than a `DW` diagnostic on purpose: the defect it catches is
//! in the **compiler**, not in a campaign. No campaign input can produce it, so
//! there is nothing for a build-time diagnostic to fire on — the same reasoning
//! that makes `DW0185` a build-tier output scan rather than a validation rule.

use std::collections::BTreeSet;

use delvewright_dsl::envelope::Stage;
use serde_json::Value;

/// What a string-valued DSL field is, for translation purposes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// A player reads it: it is in the l10n inventory (`dsl::l10n::each_string`)
    /// and ships as a translatable component.
    Inventoried,
    /// A DSL id or a pinned-registry id (`minecraft:…`). Machine vocabulary, the
    /// same in every language.
    Reference,
    /// A serde discriminator or envelope field. Grammar, not content.
    Machine,
    /// Authoring or validation context the player never sees. The `&'static str`
    /// is why — it is the justification, and an empty one fails.
    NotPlayerVisible(&'static str),
}

use Kind::*;

/// **The classification.** One row per string-valued property of the stage
/// schemas, `(type, field, kind)`.
///
/// Rows are checked against the live schemas in both directions: an unclassified
/// property fails, and a row for a property that no longer exists fails too, so
/// the table cannot rot into a list of things that used to be true.
const SURFACE: &[(&str, &str, Kind)] = &[
    ("<root>", "dsl_version", Machine),
    ("Actor", "entity", Reference),
    ("Actor", "name", Inventoried),
    ("Area", "name", Inventoried),
    ("Boundary", "message", Inventoried),
    ("BranchDecl", "leads_to", Reference),
    ("CastBarks", "barks", Inventoried),
    (
        "CastPlacement",
        "doing",
        NotPlayerVisible(
            "the scene ledger's authoring note — what an NPC is doing this quest; \
                          stage 6 writes its lines against it, no player sees it (spec-0020)",
        ),
    ),
    ("Class", "blurb", Inventoried),
    ("Class", "name", Inventoried),
    ("DialogueEffect", "type", Machine),
    ("DialogueNode", "text", Inventoried),
    ("DialogueOption", "label", Inventoried),
    ("DialogueOption", "tooltip", Inventoried),
    (
        "EditBatch",
        "note",
        NotPlayerVisible("why an edit batch exists; machine-ignored authoring context (spec-0017)"),
    ),
    ("EditFrame", "kind", Machine),
    ("EnchantedItem", "item", Reference),
    ("Happening", "subject", Reference),
    (
        "Happening",
        "text",
        NotPlayerVisible(
            "one line of prose stating what a story node does to the story; read by \
                          the branch chronicle and by DW0485, never shown in game (spec-0025)",
        ),
    ),
    ("ItemDrop", "item", Reference),
    ("ItemDrop", "name", Inventoried),
    ("KitItem", "item", Reference),
    ("KitItem", "name", Inventoried),
    ("LootItem", "item", Reference),
    ("LootItem", "name", Inventoried),
    ("MobEffect", "effect", Reference),
    ("MorphOp", "kind", Machine),
    ("Npc", "base_entity", Reference),
    ("Npc", "name", Inventoried),
    ("NpcSkin", "texture_id", Reference),
    ("Objective", "hint", Inventoried),
    ("Objective", "item", Reference),
    ("Objective", "item_name", Inventoried),
    ("Objective", "missing_item_hint", Inventoried),
    ("Objective", "requires_item", Reference),
    ("Objective", "title", Inventoried),
    ("Objective", "type", Machine),
    ("PaletteBlock", "block", Reference),
    (
        "Persona",
        "archetype",
        NotPlayerVisible(
            "persona: the NPC's voice, written for stage 6 and for a translator's \
                          context; never rendered (spec-0001)",
        ),
    ),
    (
        "Persona",
        "backstory",
        NotPlayerVisible("persona — see `archetype`"),
    ),
    (
        "Persona",
        "demeanor",
        NotPlayerVisible("persona — see `archetype`"),
    ),
    (
        "Persona",
        "motivation",
        NotPlayerVisible("persona — see `archetype`"),
    ),
    (
        "Persona",
        "secret",
        NotPlayerVisible("persona — see `archetype`"),
    ),
    (
        "Persona",
        "speech_style",
        NotPlayerVisible("persona — see `archetype`"),
    ),
    ("PlannedQuest", "goal", Inventoried),
    ("PotionContents", "color", Reference),
    ("PotionContents", "potion", Reference),
    ("PotionEffect", "effect", Reference),
    ("Prop", "block", Reference),
    ("QuestEffect", "block", Reference),
    ("QuestEffect", "falling_block", Reference),
    ("QuestEffect", "item", Reference),
    ("QuestEffect", "name", Inventoried),
    ("QuestEffect", "projectile", Reference),
    ("QuestEffect", "prompt", Inventoried),
    ("QuestEffect", "rest_label", Inventoried),
    ("QuestEffect", "save_label", Inventoried),
    ("QuestEffect", "sealed_hint", Inventoried),
    ("QuestEffect", "sound", Reference),
    ("QuestEffect", "text", Inventoried),
    ("QuestEffect", "then_floor", Reference),
    ("QuestEffect", "type", Machine),
    ("RegionShape", "blocks", Reference),
    ("RegionShape", "kind", Machine),
    (
        "Relationship",
        "attitude",
        NotPlayerVisible(
            "persona relationship — how one NPC regards another; authoring context \
                          for stage 6 (spec-0001)",
        ),
    ),
    ("SoundAt", "actor", Reference),
    ("SoundAt", "at", Machine),
    (
        "StateDecl",
        "note",
        NotPlayerVisible(
            "what a runtime datum MEANS, written for the author — the forcing function \
                          that makes someone say what the number is, exactly as \
                          `CastPlacement.doing` does for a scene. Never machine-checked and \
                          never rendered (spec-0031)",
        ),
    ),
    ("Trigger", "type", Machine),
    ("TriggerOn", "on", Machine),
    ("WaveMob", "entity", Reference),
    ("WaveMob", "name", Inventoried),
    ("WorldContent", "languages", Reference),
    ("WorldContent", "outro", Inventoried),
    (
        "WorldContent",
        "premise",
        NotPlayerVisible("the delve's brief, written for the generator; never rendered"),
    ),
    (
        "WorldContent",
        "theme",
        NotPlayerVisible("the delve's brief, written for the generator; never rendered"),
    ),
    ("WorldContent", "title", Inventoried),
    ("WorldEdit", "matching", Reference),
    ("WorldEdit", "verb", Machine),
];

/// Every string-valued property of the seven stage schemas, as `(type, field)`.
fn schema_string_fields() -> BTreeSet<(String, String)> {
    let mut out = BTreeSet::new();
    for stage in [
        Stage::World,
        Stage::Npcs,
        Stage::Classes,
        Stage::QuestPlan,
        Stage::Quests,
        Stage::Dialogue,
        Stage::WorldEdits,
    ] {
        let schema = delvewright_dsl::stage_schema(stage);
        if let Some(defs) = schema.get("$defs").and_then(Value::as_object) {
            for (name, def) in defs {
                collect(name, def, &mut out);
            }
        }
        collect("<root>", &schema, &mut out);
    }
    out
}

fn collect(name: &str, def: &Value, out: &mut BTreeSet<(String, String)>) {
    if let Some(props) = def.get("properties").and_then(Value::as_object) {
        for (field, ty) in props {
            if is_stringy(ty) {
                out.insert((name.to_string(), field.to_string()));
            }
        }
    }
    // An enum variant's fields live under `oneOf`/`anyOf`; `Option<T>` under
    // `anyOf` too. Both are the same type as far as classification goes.
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(list) = def.get(key).and_then(Value::as_array) {
            for sub in list {
                collect(name, sub, out);
            }
        }
    }
}

/// Whether a property schema carries author-written text: a `string`, an
/// `Option<String>`, or a `Vec<String>`.
fn is_stringy(ty: &Value) -> bool {
    match ty.get("type") {
        Some(Value::String(s)) if s == "array" => ty.get("items").is_some_and(is_stringy),
        Some(Value::String(s)) => s == "string",
        Some(Value::Array(a)) => a.iter().any(|x| x == "string"),
        _ => {
            if let Some(items) = ty.get("items") {
                return is_stringy(items);
            }
            ["oneOf", "anyOf", "allOf"].iter().any(|k| {
                ty.get(k)
                    .and_then(Value::as_array)
                    .is_some_and(|l| l.iter().any(is_stringy))
            })
        }
    }
}

/// The classification covers the live schema **exactly**: no unclassified field,
/// no stale row. This is the gate — a new `String` anywhere in the DSL lands here
/// before it can land in a delve.
#[test]
fn every_dsl_string_field_is_classified() {
    let live = schema_string_fields();
    let table: BTreeSet<(String, String)> = SURFACE
        .iter()
        .map(|(t, f, _)| ((*t).to_string(), (*f).to_string()))
        .collect();

    assert!(
        !live.is_empty(),
        "binding: the schema walk found no string fields at all — the walk is broken, \
         not the DSL"
    );
    println!(
        "l10n surface binding: {} string-valued stage-document fields examined",
        live.len()
    );

    let unclassified: Vec<_> = live.difference(&table).collect();
    assert!(
        unclassified.is_empty(),
        "{} DSL string field(s) have no l10n classification. A field a player reads must \
         enter `dsl::l10n::each_string` (and this table as `Inventoried`); one they never \
         read is `Reference` / `Machine` / `NotPlayerVisible(<why>)`. Until it is classified \
         nobody has decided, which is how `actors[].name` shipped English-only: {unclassified:#?}",
        unclassified.len()
    );

    let stale: Vec<_> = table.difference(&live).collect();
    assert!(
        stale.is_empty(),
        "{} classification row(s) name a field the DSL no longer has — delete them so the \
         table stays a statement about the present: {stale:#?}",
        stale.len()
    );
}

/// The table itself is well formed: unique rows, and every exclusion carries a
/// real reason. "Not player-visible, because" with nothing after `because` is how
/// a classification stops being a decision.
#[test]
fn the_classification_is_well_formed() {
    let mut seen = BTreeSet::new();
    let mut counts = [0usize; 4];
    for (ty, field, kind) in SURFACE {
        assert!(seen.insert((ty, field)), "duplicate row for `{ty}.{field}`");
        counts[match kind {
            Inventoried => 0,
            Reference => 1,
            Machine => 2,
            NotPlayerVisible(reason) => {
                assert!(
                    reason.len() > 20,
                    "`{ty}.{field}` is excluded without a real reason"
                );
                3
            }
        }] += 1;
    }
    println!(
        "classification: {} inventoried, {} reference, {} machine, {} not-player-visible \
         (of {} fields)",
        counts[0],
        counts[1],
        counts[2],
        counts[3],
        SURFACE.len()
    );
    assert!(counts[0] > 0, "binding: nothing is inventoried");
}

/// **The rows marked `Inventoried` really are.** A classification is only worth
/// something if it is checked against the traversal rather than believed: this
/// takes the keep-trial fixture, gives it the actor surface the fixtures do not
/// otherwise carry, and requires every key kind the table promises.
///
/// It also pins the **entity display-name rule** on the case the owner hit: a
/// puppet named exactly like an NPC shares that NPC's key (so a translator answers
/// once and the giant cannot be called two things), and two puppets sharing a name
/// share one key.
#[test]
fn the_inventoried_fields_reach_the_inventory() {
    let campaign = fixture_with_actors();
    let inv = delvewright_dsl::l10n_inventory(&campaign);
    assert!(!inv.is_empty(), "binding: the fixture inventories nothing");

    // Each row is `(what, key prefix, key suffix)` — the inventory names emitted
    // keys while the table above names Rust fields, and `each_string` is exactly
    // the mapping between them.
    for (what, prefix, suffix) in [
        ("world title", "world.title", ""),
        ("area name", "area.", ".name"),
        ("npc name", "npc.", ".name"),
        // The hole this file exists for. A regression removes this key kind and
        // nothing else in the suite notices.
        ("actor name", "actor.", ".name"),
        ("wave mob name", "wave.", ".name"),
        ("class name", "class.", ".name"),
        ("objective title", "obj.", ".title"),
        ("dialogue text", "dlg.", ".text"),
        ("quest goal", "quest.", ".goal"),
        ("effect narrate", "fx.", ".narrate"),
    ] {
        assert!(
            inv.keys()
                .any(|k| k.starts_with(prefix) && k.ends_with(suffix)),
            "the fixture's `{what}` never reached the inventory"
        );
    }

    // The puppet that portrays the Keeper is inventoried under the KEEPER's key —
    // one question to the translator, one answer, one name on screen wherever the
    // character stands.
    assert_eq!(
        inv.get("npc.keeper.name").map(String::as_str),
        Some("The Keeper")
    );
    assert!(
        !inv.contains_key("actor.keeper-seated.name"),
        "a puppet named exactly like its NPC must not ask for a second translation"
    );
    // Two puppets, one name, one key — and the key belongs to the first declared.
    assert_eq!(
        inv.get("actor.ram-a.name").map(String::as_str),
        Some("Ram of the Cave")
    );
    assert!(
        !inv.contains_key("actor.ram-b.name"),
        "a second puppet with the same name must share the first's key"
    );
    // A distinct name still gets its own key.
    assert_eq!(
        inv.get("actor.ewe.name").map(String::as_str),
        Some("Ewe of the Cave")
    );

    // And nothing the table calls not-player-visible did.
    for forbidden in ["premise", "theme", "doing", "persona", "happening", "note"] {
        assert!(
            !inv.keys().any(|k| k.contains(forbidden)),
            "`{forbidden}` is classified as never rendered but reached the inventory"
        );
    }
    println!(
        "inventory binding: {} keys over the patched keep-trial",
        inv.len()
    );
}

/// `localize` follows the same shared keys: translating once translates every
/// body that carries the name. This is the property the shared key BUYS — without
/// it the four Polyphemus puppets could each hold a different rendering.
#[test]
fn a_shared_name_key_translates_every_body() {
    let mut campaign = fixture_with_actors();
    let mut tr = std::collections::BTreeMap::new();
    tr.insert("npc.keeper.name".to_string(), "看守人".to_string());
    tr.insert("actor.ram-a.name".to_string(), "洞中公羊".to_string());
    delvewright_dsl::localize(&mut campaign, &tr);

    let npc = &campaign.npcs.content.npcs[0];
    assert_eq!(npc.name, "看守人");
    let named: Vec<(&str, &str)> = campaign
        .quests
        .content
        .actors
        .iter()
        .filter_map(|a| Some((a.id.as_str(), a.name.as_deref()?)))
        .collect();
    assert_eq!(
        named,
        vec![
            ("actor/keeper-seated", "看守人"),
            ("actor/ram-a", "洞中公羊"),
            ("actor/ram-b", "洞中公羊"),
            ("actor/ewe", "Ewe of the Cave"),
        ],
        "one translation must reach every body that carries the name"
    );
}

/// The keep-trial fixture with four named puppets spliced in: one portraying its
/// NPC, two sharing a name, one distinct. The fixtures carry no `actors[].name`
/// otherwise, which is a large part of why the hole survived.
fn fixture_with_actors() -> delvewright_dsl::Campaign {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/valid/keep-trial");
    let read =
        |f: &str| std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
    let mut quests: Value = serde_json::from_str(&read("quests.json")).expect("quests parse");
    quests["dsl_version"] = Value::String("0.9.0".to_string());
    // A narrate, so the `fx.…` key kind binds too.
    quests["content"]["quests"][0]["on_complete"] = serde_json::json!([
        { "type": "narrate", "text": "The hall falls quiet." }
    ]);
    quests["content"]["actors"] = serde_json::json!([
        { "id": "actor/keeper-seated", "entity": "minecraft:villager",
          "name": "The Keeper", "anchor": "anchor/hall" },
        { "id": "actor/ram-a", "entity": "minecraft:sheep",
          "name": "Ram of the Cave", "anchor": "anchor/hall" },
        { "id": "actor/ram-b", "entity": "minecraft:sheep",
          "name": "Ram of the Cave", "anchor": "anchor/hall" },
        { "id": "actor/ewe", "entity": "minecraft:sheep",
          "name": "Ewe of the Cave", "anchor": "anchor/hall" },
    ]);
    delvewright_dsl::parse_campaign(&delvewright_dsl::RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: serde_json::to_string(&quests).expect("quests re-serialize"),
        dialogue: read("dialogue.json"),
        world_edits: None,
    })
    .expect("the patched keep-trial parses")
}
