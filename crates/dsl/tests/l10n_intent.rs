//! The intent an inventory row carries for a transcreator: its kind of text
//! (`key_kind`) and the situation it is said in (`key_situations`).

mod common;

use delvewright_dsl::{TextKind, key_kind, key_situations, parse_campaign};

/// One key of every kind the scheme defines resolves to that kind; a key the
/// scheme does not define resolves to none.
#[test]
fn every_key_class_has_its_kind() {
    use TextKind::*;
    let table = [
        ("world.title", Title),
        ("world.boundary.message", Refusal),
        ("world.outro", Narration),
        ("area.hall.name", Name),
        ("npc.keeper.name", Name),
        ("state.tallow.name", Name),
        ("class.warder.name", Name),
        ("class.warder.blurb", Description),
        ("class.warder.kit.0.name", ItemName),
        ("quest.open.goal", Objective),
        ("obj.open.talk.title", Objective),
        ("obj.open.talk.hint", Objective),
        ("obj.open.door.missing_item_hint", Refusal),
        ("obj.open.find.item_name", ItemName),
        ("cast.open.keeper.0.bark.1", Bark),
        ("dlg.keeper.greeting.text", Dialogue),
        ("dlg.keeper.greeting.opt.0.label", OptionLabel),
        ("dlg.keeper.greeting.opt.0.tooltip", ButtonTooltip),
        ("wave.muster.mob.0.name", Name),
        ("wave.muster.mob.0.drop.1.name", ItemName),
        ("wave.muster.health_bar.title", Title),
        ("actor.moth.name", Name),
        ("actor.moth.drop.0.name", ItemName),
        ("actor.moth.health_bar.title", Title),
        ("loot.case.item.0.name", ItemName),
        ("lethal.pit.message", Narration),
        ("stake.tallow.collected", Narration),
        ("shop.counter.title", Title),
        ("shop.counter.offer.2.label", OptionLabel),
        ("shop.counter.offer.2.tooltip", ItemTooltip),
        ("fx.open.oc.talk.0.narrate", Narration),
        ("fx.open.oc.talk.0.seq.1.0.narrate", Narration),
        ("fx.open.done.1.give", ItemName),
        ("fx.trig.bonfire.0.rest_prompt", Prompt),
        ("fx.trig.bonfire.0.rest_label", OptionLabel),
        ("fx.trig.bonfire.0.save_label", OptionLabel),
        ("fx.open.oc.talk.2.sealed_hint", Refusal),
    ];
    for (key, kind) in table {
        assert_eq!(key_kind(key), Some(kind), "{key}");
    }
    for key in [
        "dlg.keeper.greeting",
        "fx.open.done.0.spawn",
        "nope.x.name",
        "",
    ] {
        assert_eq!(key_kind(key), None, "{key}");
    }
}

/// A dialogue line hears which answers it offers; an option hears the line it
/// answers; an objective's title hears its quest and hint; a bark hears what
/// its speaker is doing — the authoring context the player never sees.
#[test]
fn situations_carry_the_campaign_context_by_key() {
    let mut raw = common::valid_raw();
    let mut quests: serde_json::Value = serde_json::from_str(&raw.quests).unwrap();
    quests["content"]["quests"][0]["cast"]["npc/keeper"] = serde_json::json!({
        "at": "anchor/keeper-stand",
        "doing": "barring the door with his body",
        "dialogue": { "barks": ["Mind the step."] }
    });
    raw.quests = quests.to_string();
    let c = parse_campaign(&raw).expect("parses");
    let s = key_situations(&c);

    let greeting = &s["dlg.keeper.greeting.text"];
    assert_eq!(
        greeting,
        &vec!["The player can answer: Who are you? / Open the door, please.".to_string()]
    );
    let label = &s["dlg.keeper.greeting.opt.0.label"];
    assert!(
        label[0].starts_with("Answers the NPC line: Halt, traveler."),
        "{label:?}"
    );
    let bark: Vec<&String> = s
        .iter()
        .filter(|(k, _)| k.starts_with("cast."))
        .flat_map(|(_, v)| v)
        .collect();
    assert!(
        bark.iter()
            .any(|l| l.ends_with("during this quest: barring the door with his body")),
        "{bark:?}"
    );
    let goal_key = s.keys().find(|k| k.starts_with("quest.")).unwrap();
    assert!(
        !s[goal_key].iter().any(|l| l.starts_with("Quest: ")),
        "a goal is not told its own goal"
    );
    // Names have nothing to add.
    assert!(s["npc.keeper.name"].is_empty());
}
