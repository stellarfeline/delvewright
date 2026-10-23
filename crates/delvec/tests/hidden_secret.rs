//! **A hidden way's secret is kept until its shot** (spec-0118 §6.4,
//! `DW1060`), over the blockout fixture: its cell door, opened when the keeper
//! yields, declared `learned: hidden` with a secret, a `cutscene` of its seam
//! in the beat that opens it, and the strings that tell or keep the word.
//!
//! Each test is one red→green half over one string: the check reads the
//! play order's placement of every string ([`delvec::compiler::telling`]),
//! never a placement of its own.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::telling::{SecretBinding, check_secrets};
use delvewright_dsl::{Campaign, Diagnostic, L10nDoc};
use serde_json::{Value, json};

/// The blockout fixture's documents, the cell door declared hidden with
/// `secret`, the keeper's beat carrying the shot, and `edit` applied to the
/// quests document after.
fn campaign(secret: &[&str], shot: bool, edit: impl FnOnce(&mut Value)) -> Campaign {
    campaign_with_npcs(secret, shot, |_| {}, edit)
}

/// [`campaign`], with `npcs` applied to the NPCs document first.
fn campaign_with_npcs(
    secret: &[&str],
    shot: bool,
    npcs: impl FnOnce(&mut Value),
    edit: impl FnOnce(&mut Value),
) -> Campaign {
    let dir = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let mut loaded = delvec::compiler::load::load_campaign_dir(&dir).expect("readable");
    let mut people: Value = serde_json::from_str(&loaded.raw.npcs).unwrap();
    npcs(&mut people);
    loaded.raw.npcs = people.to_string();
    let mut graph: Value =
        serde_json::from_str(loaded.raw.layout_graph.as_deref().expect("a graph")).unwrap();
    for e in graph["content"]["edges"].as_array_mut().unwrap() {
        if e["id"] == "edge/hall-cell" {
            e["gating"]["learned"] = json!("hidden");
            e["gating"]["secret"] = json!(secret);
        }
    }
    loaded.raw.layout_graph = Some(graph.to_string());
    let mut quests: Value = serde_json::from_str(&loaded.raw.quests).unwrap();
    let list = &mut quests["content"]["quests"][0]["on_objective_complete"]["obj/ask"];
    if shot {
        list.as_array_mut().unwrap().push(json!({
            "type": "cutscene", "seconds": 3,
            "path": [{"anchor": "anchor/node-hall", "offset": [0, 2, 0]}],
            "look_at": {"anchor": "anchor/seam-hall-cell"},
        }));
    }
    edit(&mut quests);
    loaded.raw.quests = quests.to_string();
    delvewright_dsl::parse_campaign(&loaded.raw).expect("the fixture parses")
}

/// A narrate pushed onto the keeper's beat, after everything in it.
fn tell_after(q: &mut Value, text: &str) {
    q["content"]["quests"][0]["on_objective_complete"]["obj/ask"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "narrate", "style": "chat", "text": text}));
}

fn judged(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> (Vec<Diagnostic>, SecretBinding) {
    check_secrets(c, sidecars)
}

fn none() -> BTreeMap<String, L10nDoc> {
    BTreeMap::new()
}

/// The word told only after the shot passes; the binding states its counts.
#[test]
fn a_word_told_after_the_shot_passes() {
    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "Beyond the door, a vault.")
    });
    let (d, b) = judged(&c, &none());
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(
        (b.hidden_ways, b.words, b.told_after, b.refused),
        (1, 1, 1, 0),
        "{}",
        b.line()
    );
    assert!(
        b.line()
            .starts_with("secrets (DW1060): 1 hidden way(s), 1 secret word(s);")
    );
}

/// A narrate on an arrival trigger — a string the walk does not date —
/// mentioning the word is refused as read before the shot.
#[test]
fn a_secret_on_an_arrival_narrate_is_told_before_the_shot() {
    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "Beyond the door, a vault.");
        q["content"]["triggers"] = json!([{
            "id": "trigger/arrive", "at": "anchor/node-hall",
            "on": {"on": "approach", "range": 3},
            "effects": [{"type": "narrate", "style": "chat", "text": "A vault lies behind the keeper's door."}]
        }]);
    });
    let (d, b) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, "DW1060");
    let m = &d[0].message;
    assert!(m.contains("`vault`"), "{m}");
    assert!(
        m.contains("before the shot (undated, read as the start)"),
        "{m}"
    );
    assert!(
        m.contains("fx.trig.arrive"),
        "the string's key is named: {m}"
    );
    assert_eq!(b.refused, 1, "{}", b.line());
}

/// The same word in the hint of the objective whose completion fires the
/// shot — readable from the start of the quest — is refused naming the step.
#[test]
fn a_secret_in_an_earlier_hint_is_refused_naming_the_step() {
    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "Beyond the door, a vault.");
        q["content"]["quests"][0]["objectives"][0]["hint"] = json!("Ask the keeper for the vault.");
    });
    let (d, _) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    let m = &d[0].message;
    assert!(m.contains("obj.walk-the-whole.ask.hint"), "{m}");
    assert!(m.contains("from the start of the critical path"), "{m}");
}

/// A narrate in the shot's own beat, after the `cutscene`, tells after it; one
/// before the `cutscene` in the same list tells before it.
#[test]
fn the_shots_own_beat_tells_after_the_shot_and_not_before() {
    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "The vault stands open.")
    });
    let (d, _) = judged(&c, &none());
    assert!(d.is_empty(), "{d:?}");

    let c = campaign(&["vault"], true, |q| {
        let list = q["content"]["quests"][0]["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap();
        list.insert(
            0,
            json!({"type": "narrate", "style": "chat", "text": "The vault is opening."}),
        );
        list.push(json!({"type": "narrate", "style": "chat", "text": "The vault stands open."}));
    });
    let (d, _) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].message.contains("before the shot"), "{}", d[0].message);
}

/// The word in a `zh-cn` sidecar's rendition alone, the English silent, is
/// refused naming the language.
#[test]
fn a_secret_in_a_sidecar_alone_is_refused_naming_the_language() {
    let mut c = campaign(&["vault", "密室"], true, |q| {
        tell_after(q, "Beyond the door, a vault.");
        q["content"]["quests"][0]["objectives"][0]["hint"] = json!("Ask the keeper.");
    });
    c.world.content.languages = vec!["zh-cn".to_string()];
    let doc: L10nDoc = serde_json::from_value(json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": "blockout",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": {
            "obj.walk-the-whole.ask.hint": "向看守询问密室。",
            "fx.walk-the-whole.oc.ask.3.narrate": "门后是一间密室。"
        }
    }))
    .expect("a sidecar");
    let sidecars: BTreeMap<String, L10nDoc> = [("zh-cn".to_string(), doc)].into_iter().collect();
    let (d, _) = judged(&c, &sidecars);
    assert!(
        d.iter()
            .any(|x| x.message.contains("`密室`") && x.message.contains("in the `zh-cn` sidecar")),
        "{d:?}"
    );
}

/// A secret word no string anywhere mentions is refused as never told.
#[test]
fn a_secret_nobody_tells_is_refused() {
    let c = campaign(&["vault", "flue"], true, |q| {
        tell_after(q, "Beyond the door, a vault.")
    });
    let (d, b) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(d[0].message.contains("`flue`"), "{}", d[0].message);
    assert!(
        d[0].message
            .contains("no string the player reads at or after the shot"),
        "{}",
        d[0].message
    );
    assert_eq!((b.told_after, b.refused), (1, 1), "{}", b.line());
}

/// With no shot to read against, the check judges nothing and says so —
/// `DW1054` refuses the way at the build.
#[test]
fn a_hidden_way_with_no_shot_is_counted_and_not_judged() {
    let c = campaign(&["vault"], false, |q| {
        q["content"]["quests"][0]["objectives"][0]["hint"] = json!("Ask for the vault.");
    });
    let (d, b) = judged(&c, &none());
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(b.no_shot, 1, "{}", b.line());
}

/// A body's answer to a blow (spec-0119) is a string the player reads, and
/// the walk does not date it: one naming the secret is refused as read before
/// the shot. The pair `DW1060` and `answers` share the one inventory, so a
/// body's line is placed like any other string, with no copy of its own.
#[test]
fn a_secret_in_a_bodys_answer_is_told_before_the_shot() {
    let c = campaign_with_npcs(
        &["vault"],
        true,
        |n| {
            n["content"]["npcs"][0]["answers"] =
                json!({ "struck": "The keeper does not flinch; the vault is not yours." });
        },
        |q| tell_after(q, "Beyond the door, a vault."),
    );
    let (d, b) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code, "DW1060");
    let m = &d[0].message;
    assert!(m.contains("npc.keeper.answers.struck"), "{m}");
    assert!(
        m.contains("before the shot (undated, read as the start)"),
        "{m}"
    );
    assert_eq!(b.refused, 1, "{}", b.line());
}

/// An objective that is not announced (spec-0093, `announcement: hidden`)
/// never puts its hint on a screen, so its hint naming the secret is not read
/// before the shot; the same hint on the announced objective is. The one
/// placement reads `announced` as `DW0982` does (spec-0104 §7).
#[test]
fn an_unannounced_objectives_hint_is_not_read() {
    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "Beyond the door, a vault.");
        let o = &mut q["content"]["quests"][0]["objectives"][0];
        o["hint"] = json!("Ask the keeper for the vault.");
        o["announcement"] = json!("hidden");
    });
    let (d, b) = judged(&c, &none());
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(b.refused, 0, "{}", b.line());

    let c = campaign(&["vault"], true, |q| {
        tell_after(q, "Beyond the door, a vault.");
        q["content"]["quests"][0]["objectives"][0]["hint"] = json!("Ask the keeper for the vault.");
    });
    let (d, _) = judged(&c, &none());
    assert_eq!(d.len(), 1, "{d:?}");
    assert!(
        d[0].message.contains("obj.walk-the-whole.ask.hint"),
        "{}",
        d[0].message
    );
}
