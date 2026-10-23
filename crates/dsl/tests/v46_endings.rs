//! spec-0108, the DSL half: an ending is an id (§3.1), a cutscene has a `then`
//! dispatched from the server source (§3.6), the campaign may declare what every
//! ending reads (§3.5), and every verb says whether it changes the world (§3.7).

mod common;

use delvewright_dsl::{
    Diagnostic, NestedDispatch, QuestEffect, RawCampaign, Stage, check_campaign, stage_schema,
};
use serde_json::{Value, json};

fn codes(d: &[Diagnostic], code: &str) -> Vec<Diagnostic> {
    d.iter().filter(|x| x.code == code).cloned().collect()
}

/// The hello-world campaign with its quests document patched.
fn quests_patched(f: impl FnOnce(&mut Value)) -> RawCampaign {
    let mut raw = common::valid_raw();
    raw.quests = common::patch_doc(&common::read_valid("quests.json"), f);
    raw
}

fn plan_patched(f: impl FnOnce(&mut Value)) -> RawCampaign {
    let mut raw = common::valid_raw();
    raw.quest_plan = common::patch_doc(&common::read_valid("quest-plan.json"), f);
    raw
}

fn set_ending(q: &mut Value, ending: &str) {
    q["content"]["quests"][0]["on_complete"][0]["ending"] = json!(ending);
}

/// §3.1: a `campaign-complete` whose `ending` is a sentence is `DW0110` at the
/// effect's own `ending`, and the message spells the form the field takes.
#[test]
fn an_ending_that_is_a_sentence_is_dw0110_at_its_path() {
    let d = check_campaign(&quests_patched(|q| {
        set_ending(q, "The hall is read end to end.")
    }));
    let got = codes(&d, "DW0110");
    assert_eq!(got.len(), 1, "{got:#?}");
    assert_eq!(got[0].path, "/content/quests/0/on_complete/0/ending");
    assert!(
        got[0].message.contains("`ending/<kebab>`"),
        "{}",
        got[0].message
    );
}

/// §3.1, at any nesting depth: an ending inside a `sequence` step of an
/// objective's bundle is held to the same form, at its own nested path.
#[test]
fn a_nested_ending_is_held_to_the_form_too() {
    let d = check_campaign(&quests_patched(|q| {
        let quest = &mut q["content"]["quests"][0];
        let end = quest["on_complete"][0].take();
        quest["on_complete"] = json!([]);
        let mut end = end;
        end["ending"] = json!("Ending With Capitals");
        quest["on_objective_complete"]["obj/exit"] =
            json!([{"type": "sequence", "steps": [{"at_ticks": 10, "effects": [end]}]}]);
    }));
    let got = codes(&d, "DW0110");
    assert_eq!(got.len(), 1, "{got:#?}");
    assert_eq!(
        got[0].path,
        "/content/quests/0/on_objective_complete/obj/exit/0/steps/0/effects/0/ending"
    );
}

/// The control: a well-formed id raises nothing, and neither does an ending
/// that names none.
#[test]
fn a_well_formed_ending_and_an_unnamed_one_are_silent() {
    let d = check_campaign(&quests_patched(|q| set_ending(q, "ending/the-door-opens")));
    assert!(codes(&d, "DW0110").is_empty(), "{d:#?}");
    let d = check_campaign(&common::valid_raw());
    assert!(codes(&d, "DW0110").is_empty(), "{d:#?}");
}

/// A cutscene over the exit with a `then` holding `extra`, in the quest's
/// `on_complete`, with a `player`-scoped and a `party`-scoped datum declared and
/// read so that only the rule under test can speak.
fn with_then(extra: Value) -> RawCampaign {
    quests_patched(|q| {
        let content = &mut q["content"];
        content["state"] = json!([
            {"id": "state/coins", "scope": "player"},
            {"id": "state/tally", "scope": "party"}
        ]);
        let quest = &mut content["quests"][0];
        quest["on_complete"].as_array_mut().unwrap().push(json!({
            "type": "cutscene",
            "path": [{"anchor": "anchor/exit", "offset": [0, 3, 0]}],
            "seconds": 2,
            "then": [extra]
        }));
        quest["on_complete"].as_array_mut().unwrap().push(json!({
            "type": "narrate",
            "text": "Counted.",
            "style": "chat",
            "when": {"requires_state": [
                {"state": "state/coins", "op": "at-least", "value": 1},
                {"state": "state/tally", "op": "at-least", "value": 1}
            ]}
        }));
    })
}

/// §3.6: a `then` runs from `cs_end_<bare>`, a scheduled function with no
/// acting player, so a `player`-scoped write in it is `DW0503` at the write's
/// own nested path — the seam is read from `nested_effect_dispatch`.
#[test]
fn a_player_scoped_write_in_then_is_dw0503() {
    let d = check_campaign(&with_then(
        json!({"type": "add-state", "state": "state/coins", "amount": 1}),
    ));
    let got = codes(&d, "DW0503");
    assert_eq!(got.len(), 1, "{got:#?}");
    assert!(got[0].message.contains("state/coins"), "{}", got[0].message);
    // The control: the same write on a party datum is legal from the server.
    let d = check_campaign(&with_then(
        json!({"type": "add-state", "state": "state/tally", "amount": 1}),
    ));
    assert!(codes(&d, "DW0503").is_empty(), "{:#?}", codes(&d, "DW0503"));
}

/// §3.6: the `then` list is a nested effect list in every walk, dispatched from
/// the server, labelled `then` for paths and keys alike.
#[test]
fn then_is_a_server_dispatched_nested_list() {
    let eff: QuestEffect = serde_json::from_value(json!({
        "type": "cutscene",
        "path": [{"anchor": "anchor/exit"}],
        "seconds": 1,
        "then": [{"type": "set-flag", "flag": "flag/after"}]
    }))
    .unwrap();
    let dispatch = eff.nested_effect_dispatch();
    assert_eq!(dispatch.len(), 1);
    assert_eq!(dispatch[0].1, NestedDispatch::Server);
    assert_eq!(eff.nested_effect_lists().len(), 1);
    let labeled = eff.nested_effect_lists_labeled();
    assert_eq!(
        (labeled[0].0.as_str(), labeled[0].1.as_str()),
        ("then", "then")
    );
    // An empty `then` is omitted on the way out, so a cutscene that never states
    // one serializes exactly as it did before the field existed.
    let bare: QuestEffect = serde_json::from_value(json!({
        "type": "cutscene", "path": [{"anchor": "anchor/exit"}], "seconds": 1
    }))
    .unwrap();
    assert!(serde_json::to_value(&bare).unwrap().get("then").is_none());
}

/// §3.5 and criterion 3: the stage-4 schema exports `endings_read` with exactly
/// `at_least` and `deeds_at_least`; the stage-5 schema exports `then`.
#[test]
fn the_schemas_export_endings_read_and_then() {
    let plan = serde_json::to_string(&stage_schema(Stage::QuestPlan)).unwrap();
    let v: Value = serde_json::from_str(&plan).unwrap();
    let defs = v
        .get("$defs")
        .or_else(|| v.get("definitions"))
        .expect("schema definitions");
    let er = &defs["EndingsRead"];
    let props: Vec<&String> = er["properties"].as_object().unwrap().keys().collect();
    assert_eq!(props, vec!["at_least", "deeds_at_least"], "{er:#}");
    assert_eq!(er["additionalProperties"], json!(false), "{er:#}");
    let quests = serde_json::to_string(&stage_schema(Stage::Quests)).unwrap();
    assert!(
        quests.contains("\"then\""),
        "the quests schema exports `then`"
    );
}

/// §3.5: the declaration parses, and an unknown term is refused like any other
/// unknown field (`DW0100`) — the object has two terms and nothing else.
#[test]
fn endings_read_parses_and_refuses_an_unknown_term() {
    let d = check_campaign(&plan_patched(|p| {
        p["content"]["endings_read"] = json!({"at_least": 3, "deeds_at_least": 1});
    }));
    assert!(codes(&d, "DW0100").is_empty(), "{d:#?}");
    let d = check_campaign(&plan_patched(|p| {
        p["content"]["endings_read"] = json!({"at_least": 3, "said_at_least": 1});
    }));
    assert!(!codes(&d, "DW0100").is_empty(), "{d:#?}");
}

/// §3.7 and criterion 7: every `Verb` variant is in exactly one of the two
/// classes, by name. The list is the census the spec states (45 variants, 38
/// that change the world, 7 that say); a variant missing from it fails here, and
/// `Verb::changes_world` has no wildcard, so a new verb does not compile until
/// it is classified.
#[test]
fn every_verb_is_in_exactly_one_aftermath_class() {
    const CHANGES: [&str; 38] = [
        "set-block",
        "fill-region",
        "clear-region",
        "collapse",
        "open-gate",
        "close-gate",
        "open-way",
        "teleport",
        "damage-players",
        "give-effect",
        "clear-effect",
        "give-item",
        "set-time",
        "set-weather",
        "set-atmosphere",
        "spawn-npc",
        "despawn-npc",
        "move-npc",
        "spawn-actor",
        "despawn-actor",
        "move-actor",
        "unleash-actor",
        "spawn-wave",
        "spawn-assembly",
        "despawn-assembly",
        "volley",
        "lightning",
        "set-flag",
        "set-state",
        "add-state",
        "clear-state",
        "drop-stake",
        "set-checkpoint",
        "bonfire",
        "begin-stealth",
        "end-stealth",
        "arm-strikes",
        "play-clip",
    ];
    const SAYS: [&str; 8] = [
        "narrate",
        "play-sound",
        "stop-sound",
        "firework",
        "particle",
        "cutscene",
        "campaign-complete",
        "sequence",
    ];
    let mut named: Vec<&str> = CHANGES.iter().chain(SAYS.iter()).copied().collect();
    named.sort_unstable();
    let before = named.len();
    named.dedup();
    assert_eq!(named.len(), before, "a verb is named in both classes");
    assert_eq!(named.len(), 46);
    // Every effect the gallery declares (spec-0039: one instance of every
    // surface, probes included), parsed as the engine parses it. A verb the
    // lists above do not name fails here the day its gallery element lands.
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../gallery");
    let mut files = Vec::new();
    collect_json(&root, &mut files);
    assert!(
        !files.is_empty(),
        "no gallery documents under {}",
        root.display()
    );
    for f in files {
        let Ok(v) = serde_json::from_str::<Value>(&std::fs::read_to_string(&f).unwrap()) else {
            continue;
        };
        let mut effs = Vec::new();
        collect_effects(&v, &mut effs);
        for e in effs {
            let tag = e.verb.tag();
            let in_changes = CHANGES.contains(&tag);
            let in_says = SAYS.contains(&tag);
            assert!(
                in_changes ^ in_says,
                "`{tag}` is in neither class ({})",
                f.display()
            );
            assert_eq!(e.verb.changes_world(), in_changes, "`{tag}`");
            seen.insert(tag.to_string());
        }
    }
    let unseen: Vec<&&str> = named.iter().filter(|t| !seen.contains(**t)).collect();
    assert!(
        unseen.is_empty(),
        "no gallery instance of {unseen:?}: the class is unchecked there"
    );
}

fn collect_json(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_json(&p, out);
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
}

/// Every object in `v` that parses as a `QuestEffect`.
fn collect_effects(v: &Value, out: &mut Vec<QuestEffect>) {
    match v {
        Value::Object(m) => {
            if m.get("type").is_some_and(Value::is_string)
                && let Ok(e) = serde_json::from_value::<QuestEffect>(v.clone())
            {
                out.push(e);
            }
            for x in m.values() {
                collect_effects(x, out);
            }
        }
        Value::Array(a) => {
            for x in a {
                collect_effects(x, out);
            }
        }
        _ => {}
    }
}
