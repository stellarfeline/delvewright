//! `DW0981` — a question gets an answer; `DW0982` — a name is told before it is
//! used. Both over the hello-world fixture with one document edited.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::telling::{self, DW_NAME_UNTOLD, DW_QUESTION_UNANSWERED};
use delvewright_dsl::{Campaign, DSL_VERSION, L10nDoc, RawCampaign, Severity, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn hw(name: &str) -> serde_json::Value {
    serde_json::from_str(&read_hw(name)).unwrap()
}

fn parse(
    world: &serde_json::Value,
    quests: &serde_json::Value,
    dialogue: &serde_json::Value,
) -> Campaign {
    let raw = RawCampaign {
        world: world.to_string(),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// hello-world with the dialogue edited by `f`.
fn with_dialogue(f: impl FnOnce(&mut serde_json::Value)) -> Campaign {
    let mut d = hw("dialogue.json");
    f(&mut d);
    parse(&hw("world.json"), &hw("quests.json"), &d)
}

fn option(d: &mut serde_json::Value, node: usize, opt: usize) -> &mut serde_json::Value {
    &mut d["content"]["dialogues"][0]["nodes"][node]["options"][opt]
}

/// The fixture as written asks one question (`Who are you?`) and answers it in
/// `dlg/lore`, so it is clean — the check is not rejecting every question.
#[test]
fn an_answered_question_is_clean() {
    let c = with_dialogue(|_| {});
    assert!(
        telling::check_questions(&c, &BTreeMap::new()).is_empty(),
        "`Who are you?` leads to the Keeper's answer"
    );
}

/// The playtest shape: a question whose only effect completes an objective and
/// which has no `next`. The player asks and the dialog closes.
#[test]
fn a_question_that_closes_the_dialog_is_dw0981() {
    let c = with_dialogue(|d| {
        option(d, 0, 1)["label"] = "Will you open the door?".into();
    });
    let d = telling::check_questions(&c, &BTreeMap::new());
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW0981");
    assert_eq!(d[0].code, DW_QUESTION_UNANSWERED.id());
    assert_eq!(d[0].severity, Severity::Error);
    assert_eq!(d[0].path, "/content/dialogues/0/nodes/0/options/1");
    assert!(d[0].message.contains("no `next`"), "{}", d[0].message);
}

/// A `next` back to the node the option stands in repeats the line the player
/// asked about; that is not an answer.
#[test]
fn a_question_that_loops_to_its_own_node_is_dw0981() {
    let c = with_dialogue(|d| {
        option(d, 0, 0)["next"] = "dlg/greeting".into();
    });
    let d = telling::check_questions(&c, &BTreeMap::new());
    assert!(
        d.iter().any(|x| x.code == "DW0981"
            && x.path == "/content/dialogues/0/nodes/0/options/0"
            && x.message.contains("the node it stands in")),
        "{d:#?}"
    );
}

/// A question asked only in a sidecar rendition is still the player asking.
#[test]
fn a_question_asked_only_in_chinese_is_dw0981_naming_the_language() {
    let mut world = hw("world.json");
    world["content"]["languages"] = serde_json::json!(["zh-cn"]);
    let c = parse(&world, &hw("quests.json"), &hw("dialogue.json"));
    let doc: L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": { "dlg.keeper.greeting.opt.1.label": "能开门吗？" }
    }))
    .unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);
    let d = telling::check_questions(&c, &sidecars);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0981" && x.message.contains("zh-cn label")),
        "{d:#?}"
    );
}

/// The playtest shape for names: the player's own option names a thing shown
/// only as a label — here the area's name, `The Keep` — before anything has
/// said what it is.
#[test]
fn a_name_shown_only_as_a_label_is_dw0982() {
    let c = with_dialogue(|d| {
        option(d, 0, 1)["label"] = "Open the keep, please.".into();
    });
    let (d, bind) = telling::check_names_told_bound(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW0982");
    assert_eq!(d[0].code, DW_NAME_UNTOLD.id());
    assert_eq!(d[0].severity, Severity::Error);
    assert_eq!(d[0].path, "/content/dialogues/0/nodes/0/options/1/label");
    assert!(d[0].message.contains("area.keep.name"), "{}", d[0].message);
    assert_eq!(bind.untold, 1);
    assert!(
        bind.uses >= 1 && bind.walks >= 1 && bind.names >= 1,
        "{bind:?}"
    );
}

/// A line that says what the name is, read before the use, tells it.
#[test]
fn a_name_glossed_first_is_clean() {
    let c = with_dialogue(|d| {
        d["content"]["dialogues"][0]["nodes"][0]["text"] =
            "Halt. This is the Keep, the last hold on the moor, and the door stays shut.".into();
        option(d, 0, 1)["label"] = "Open the keep, please.".into();
    });
    let (d, bind) = telling::check_names_told_bound(&c);
    assert!(d.is_empty(), "{d:#?}");
    assert_eq!(bind.untold, 0);
}

/// The order is the play order: a gloss the player reads only after the use
/// does not tell it in time.
#[test]
fn a_gloss_read_after_the_use_is_still_dw0982() {
    let mut quests = hw("quests.json");
    let oc = &mut quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"];
    oc.as_array_mut().unwrap().push(serde_json::json!({
        "type": "narrate",
        "style": "chat",
        "text": "The Keep, the last hold on the moor, lets you in."
    }));
    let mut d = hw("dialogue.json");
    option(&mut d, 0, 1)["label"] = "Open the keep, please.".into();
    let c = parse(&hw("world.json"), &quests, &d);
    let (diags, _) = telling::check_names_told_bound(&c);
    assert!(
        diags
            .iter()
            .any(|x| x.code == "DW0982" && x.message.contains("before the first step")),
        "the label is on screen before the narration that glosses the name: {diags:#?}"
    );

    // …and the same gloss read first is in time.
    let mut d = hw("dialogue.json");
    d["content"]["dialogues"][0]["nodes"][0]["text"] =
        "The Keep, the last hold on the moor, is shut.".into();
    option(&mut d, 0, 1)["label"] = "Open the keep, please.".into();
    let c = parse(&hw("world.json"), &quests, &d);
    assert!(telling::check_names_told_bound(&c).0.is_empty());
}

/// The person the player is speaking with is introduced by their own dialog,
/// whose title carries their name.
#[test]
fn the_speaker_is_told_by_speaking() {
    let c = with_dialogue(|d| {
        option(d, 0, 0)["label"] = "Are you the Keeper?".into();
    });
    let (d, bind) = telling::check_names_told_bound(&c);
    assert!(d.is_empty(), "{d:#?}");
    assert!(bind.uses >= 1, "the use must be counted: {bind:?}");
}

/// hello-world with one wave of `rows` (`(entity, count, name)`) and, when
/// given, one actor named `actor`.
fn with_wave(rows: &[(&str, u32, Option<&str>)], actor: Option<&str>) -> Campaign {
    let mut quests = hw("quests.json");
    let mobs: Vec<serde_json::Value> = rows
        .iter()
        .map(|(e, n, name)| {
            let mut m = serde_json::json!({ "entity": e, "count": n });
            if let Some(name) = name {
                m["name"] = (*name).into();
            }
            m
        })
        .collect();
    quests["content"]["waves"] =
        serde_json::json!([{ "id": "wave/crowd", "anchor": "anchor/exit", "mobs": mobs }]);
    if let Some(a) = actor {
        quests["content"]["actors"] = serde_json::json!([{
            "id": "actor/one", "entity": "minecraft:zombie", "anchor": "anchor/exit", "name": a
        }]);
    }
    parse(&hw("world.json"), &quests, &hw("dialogue.json"))
}

/// The playtest shape: three zombies of one wave, all tagged `The Watch`.
#[test]
fn a_name_tag_on_a_crowd_is_dw0983() {
    let c = with_wave(&[("minecraft:zombie", 3, Some("The Watch"))], None);
    let (d, bind) = telling::check_name_tags(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW0983");
    assert_eq!(d[0].code, telling::DW_NAME_ON_A_CROWD.id());
    assert_eq!(d[0].severity, Severity::Error);
    assert_eq!(d[0].path, "/content/waves/0/mobs/0/name");
    assert_eq!(
        (bind.waves, bind.group_names, bind.refused_waves),
        (1, 1, 1)
    );
}

/// Two entries of one wave under one name are a crowd too, entry by entry.
#[test]
fn one_name_on_two_entries_is_dw0983_on_both() {
    let c = with_wave(
        &[
            ("minecraft:zombie", 1, Some("Footman")),
            ("minecraft:husk", 1, Some("Footman")),
        ],
        None,
    );
    let (d, _) = telling::check_name_tags(&c);
    assert_eq!(d.iter().filter(|x| x.code == "DW0983").count(), 2, "{d:#?}");
}

/// A single named body inside a wave — the elite or the boss — is a character,
/// and its unnamed escort is a crowd that wears nothing.
#[test]
fn a_named_elite_among_unnamed_bodies_is_clean() {
    let c = with_wave(
        &[
            ("minecraft:zombie", 4, None),
            ("minecraft:vindicator", 1, Some("The Watch Captain")),
        ],
        Some("The Porter"),
    );
    let (d, bind) = telling::check_name_tags(&c);
    assert!(d.is_empty(), "{d:#?}");
    assert_eq!(
        (bind.named_waves, bind.named_actors, bind.group_names),
        (1, 1, 0)
    );
}

/// An actor wearing a crowd's name claims to be one of the crowd.
#[test]
fn an_actor_wearing_a_crowds_name_is_dw0983() {
    let c = with_wave(&[("minecraft:zombie", 2, Some("Footman"))], Some("Footman"));
    let (d, bind) = telling::check_name_tags(&c);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0983" && x.path == "/content/actors/0/name"),
        "{d:#?}"
    );
    assert_eq!(bind.refused_actors, 1);
}
