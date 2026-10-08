//! The derived happening subject (spec-0071 §3).

use super::QuestEffect;

/// An effect with a `happening` that states no subject.
fn beat(verb: serde_json::Value) -> QuestEffect {
    let mut v = verb;
    v["happening"] = serde_json::json!({ "verb": "opens", "text": "It gives." });
    serde_json::from_value(v).expect("effect fixture parses")
}

fn subject(v: serde_json::Value) -> Option<String> {
    beat(v).happening_subject().map(|s| {
        assert!(s.derived, "the fixture states no subject");
        s.id.to_string()
    })
}

/// **The census, over every verb the DSL declares.** Which verbs resolve a
/// subject is a fact about the objects an effect names, so the answer is
/// computed here rather than listed anywhere: the table below is held to the
/// variant set of the exported schema, and a thirty-ninth verb reds this
/// test until somebody answers for it.
///
/// Run with `--nocapture` to print the census a document quotes.
#[test]
fn every_verb_answers_the_census() {
    // One minimal instance per verb, in the schema's own order.
    let table: Vec<(&str, serde_json::Value, Option<&str>)> = vec![
        (
            "open-gate",
            serde_json::json!({"type":"open-gate","anchor":"anchor/door"}),
            Some("anchor/door"),
        ),
        (
            "close-gate",
            serde_json::json!({"type":"close-gate","anchor":"anchor/door"}),
            Some("anchor/door"),
        ),
        (
            "campaign-complete",
            serde_json::json!({"type":"campaign-complete"}),
            None,
        ),
        (
            "give-item",
            serde_json::json!({"type":"give-item","item":"minecraft:bread","count":1}),
            None,
        ),
        (
            "set-flag",
            serde_json::json!({"type":"set-flag","flag":"flag/heard"}),
            None,
        ),
        (
            "set-state",
            serde_json::json!({"type":"set-state","state":"state/purse","value":1}),
            None,
        ),
        (
            "add-state",
            serde_json::json!({"type":"add-state","state":"state/purse","amount":-1}),
            None,
        ),
        (
            "drop-stake",
            serde_json::json!({"type":"drop-stake","stake":"stake/purse"}),
            None,
        ),
        (
            "clear-state",
            serde_json::json!({"type":"clear-state","state":"state/purse"}),
            None,
        ),
        (
            "spawn-wave",
            serde_json::json!({"type":"spawn-wave","wave":"wave/muster"}),
            Some("wave/muster"),
        ),
        (
            "narrate",
            serde_json::json!({"type":"narrate","text":"The hall answers."}),
            None,
        ),
        (
            "set-block",
            serde_json::json!({"type":"set-block","anchor":"anchor/altar","block":"minecraft:stone"}),
            Some("anchor/altar"),
        ),
        (
            "fill-region",
            serde_json::json!({"type":"fill-region","region":{"anchor":"anchor/pit","extent":[2,2,2]},"block":"minecraft:water"}),
            Some("anchor/pit"),
        ),
        (
            "set-atmosphere",
            serde_json::json!({"type":"set-atmosphere","atmosphere":"atmosphere/wrong","region":{"anchor":"anchor/hall","extent":[4,2,4]}}),
            Some("anchor/hall"),
        ),
        (
            "clear-region",
            serde_json::json!({"type":"clear-region","region":{"anchor":"anchor/pit","extent":[2,2,2]}}),
            Some("anchor/pit"),
        ),
        (
            "open-way",
            serde_json::json!({"type":"open-way","piece":"prefab/span","way":"gap"}),
            None,
        ),
        (
            "despawn-npc",
            serde_json::json!({"type":"despawn-npc","npc":"npc/keeper"}),
            Some("npc/keeper"),
        ),
        (
            "move-npc",
            serde_json::json!({"type":"move-npc","npc":"npc/keeper","to":{"anchor":"anchor/door"}}),
            None,
        ),
        (
            "cutscene",
            serde_json::json!({"type":"cutscene","path":[{"anchor":"anchor/hall"},{"anchor":"anchor/door"}],"seconds":4}),
            None,
        ),
        (
            "set-time",
            serde_json::json!({"type":"set-time","time":"noon"}),
            None,
        ),
        (
            "set-weather",
            serde_json::json!({"type":"set-weather","weather":"clear"}),
            None,
        ),
        (
            "play-sound",
            serde_json::json!({"type":"play-sound","sound":"minecraft:block.bell.use","at":{"at":"anchor","anchor":"anchor/bell"}}),
            Some("anchor/bell"),
        ),
        (
            "damage-players",
            serde_json::json!({"type":"damage-players","amount":2}),
            None,
        ),
        (
            "set-checkpoint",
            serde_json::json!({"type":"set-checkpoint","anchor":"anchor/seat"}),
            Some("anchor/seat"),
        ),
        (
            "bonfire",
            serde_json::json!({"type":"bonfire","anchor":"anchor/seat"}),
            Some("anchor/seat"),
        ),
        (
            "begin-stealth",
            serde_json::json!({"type":"begin-stealth","zones":[{"anchor":"anchor/yard","extent":[4,2,4]}]}),
            Some("anchor/yard"),
        ),
        (
            "end-stealth",
            serde_json::json!({"type":"end-stealth"}),
            None,
        ),
        (
            "spawn-npc",
            serde_json::json!({"type":"spawn-npc","npc":"npc/keeper"}),
            Some("npc/keeper"),
        ),
        (
            "spawn-actor",
            serde_json::json!({"type":"spawn-actor","actor":"actor/giant"}),
            Some("actor/giant"),
        ),
        (
            "despawn-actor",
            serde_json::json!({"type":"despawn-actor","actor":"actor/giant","style":"vanish"}),
            Some("actor/giant"),
        ),
        (
            "move-actor",
            serde_json::json!({"type":"move-actor","actor":"actor/giant","to":{"anchor":"anchor/door"}}),
            None,
        ),
        (
            "unleash-actor",
            serde_json::json!({"type":"unleash-actor","actor":"actor/giant"}),
            Some("actor/giant"),
        ),
        (
            "sequence",
            serde_json::json!({"type":"sequence","steps":[]}),
            None,
        ),
        (
            "volley",
            serde_json::json!({"type":"volley","from_anchor":"anchor/slot","kill_zone":{"anchor":"anchor/lane","extent":[3,2,3]}}),
            None,
        ),
        (
            "collapse",
            serde_json::json!({"type":"collapse","region_anchor":{"anchor":"anchor/roof","extent":[3,1,3]}}),
            Some("anchor/roof"),
        ),
        (
            "give-effect",
            serde_json::json!({"type":"give-effect","effect":"minecraft:speed","seconds":10}),
            None,
        ),
        (
            "clear-effect",
            serde_json::json!({"type":"clear-effect"}),
            None,
        ),
        (
            "teleport",
            serde_json::json!({"type":"teleport","from":{"anchor":"anchor/hall","extent":[4,2,4]},"to":{"anchor":"anchor/door"}}),
            None,
        ),
        (
            "firework",
            serde_json::json!({"type":"firework","at":{"anchor":"anchor/court"},"explosions":[{"shape":"star","colors":["#ffd700"]}]}),
            Some("anchor/court"),
        ),
        // spec-0082: an assembly is not a story subject (no `happening` is
        // demanded of its verbs), so none of the three resolves one.
        (
            "spawn-assembly",
            serde_json::json!({"type":"spawn-assembly","assembly":"assembly/limb"}),
            None,
        ),
        (
            "despawn-assembly",
            serde_json::json!({"type":"despawn-assembly","assembly":"assembly/limb"}),
            None,
        ),
        (
            "play-clip",
            serde_json::json!({"type":"play-clip","assembly":"assembly/limb","clip":"idle"}),
            None,
        ),
        (
            "arm-strikes",
            serde_json::json!({"type":"arm-strikes","assembly":"assembly/limb"}),
            None,
        ),
        (
            "particle",
            serde_json::json!({"type":"particle","particle":"minecraft:soul","at":{"anchor":"anchor/well"}}),
            Some("anchor/well"),
        ),
        (
            "lightning",
            serde_json::json!({"type":"lightning","at":{"anchor":"anchor/court"}}),
            Some("anchor/court"),
        ),
    ];
    // The binding: the table answers for every verb the schema declares, and
    // for no name the schema does not.
    let schema = crate::schema::stage_schema(crate::envelope::Stage::Quests);
    let declared: std::collections::BTreeSet<String> = schema["$defs"]["QuestEffect"]["oneOf"]
        .as_array()
        .expect("QuestEffect is a tagged union")
        .iter()
        .map(|b| {
            b["properties"]["type"]["const"]
                .as_str()
                .expect("a tag")
                .to_string()
        })
        .collect();
    let answered: std::collections::BTreeSet<String> =
        table.iter().map(|(t, _, _)| (*t).to_string()).collect();
    assert_eq!(
        answered, declared,
        "the census answers for exactly the verbs the DSL declares"
    );
    for (tag, effect, want) in &table {
        let got = subject(effect.clone());
        assert_eq!(
            got.as_deref(),
            *want,
            "`{tag}` resolves {want:?}; it names {:?}",
            beat(effect.clone()).subject_objects()
        );
        println!(
            "| `{tag}` | {} | {} |",
            want.unwrap_or("—"),
            beat(effect.clone()).subject_objects().join(", ")
        );
    }
}

/// **Some verbs answer per instance, not per verb**, because the object is
/// in an optional field: an `in` filter, a second stealth zone, a camera
/// path of one waypoint. The rule is over the objects an effect names, so
/// the same verb resolves in one document and not in another — and that is
/// the answer, not a gap in it.
#[test]
fn a_verb_whose_object_is_optional_answers_per_instance() {
    for (what, effect, want) in [
        (
            "damage-players with an `in` filter",
            serde_json::json!({"type":"damage-players","amount":2,"in":{"anchor":"anchor/lane","extent":[3,2,3]}}),
            Some("anchor/lane"),
        ),
        (
            "give-effect with an `in` filter",
            serde_json::json!({"type":"give-effect","effect":"minecraft:speed","seconds":10,"in":{"anchor":"anchor/lane","extent":[3,2,3]}}),
            Some("anchor/lane"),
        ),
        (
            "clear-effect with an `in` filter",
            serde_json::json!({"type":"clear-effect","in":{"anchor":"anchor/lane","extent":[3,2,3]}}),
            Some("anchor/lane"),
        ),
        (
            "play-sound with no `at`",
            serde_json::json!({"type":"play-sound","sound":"minecraft:block.bell.use"}),
            None,
        ),
        (
            "begin-stealth over two zones",
            serde_json::json!({"type":"begin-stealth","zones":[
                {"anchor":"anchor/yard","extent":[4,2,4]},
                {"anchor":"anchor/lane","extent":[4,2,4]}]}),
            None,
        ),
        (
            "a cutscene whose path names one cell",
            serde_json::json!({"type":"cutscene","path":[{"anchor":"anchor/hall"}],"seconds":4}),
            Some("anchor/hall"),
        ),
    ] {
        assert_eq!(subject(effect.clone()).as_deref(), want, "{what}");
    }
}

/// The four the spec illustrates, beside the ones the rule covers without
/// naming them.
#[test]
fn an_effect_with_one_object_is_about_that_object() {
    for (effect, want) in [
        (
            serde_json::json!({ "type": "open-gate", "anchor": "anchor/door" }),
            "anchor/door",
        ),
        (
            serde_json::json!({ "type": "close-gate", "anchor": "anchor/door" }),
            "anchor/door",
        ),
        (
            serde_json::json!({ "type": "spawn-actor", "actor": "actor/giant" }),
            "actor/giant",
        ),
        (
            serde_json::json!({ "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" }),
            "actor/giant",
        ),
        (
            serde_json::json!({ "type": "unleash-actor", "actor": "actor/giant" }),
            "actor/giant",
        ),
        (
            serde_json::json!({ "type": "spawn-wave", "wave": "wave/muster" }),
            "wave/muster",
        ),
        (
            serde_json::json!({ "type": "spawn-npc", "npc": "npc/keeper" }),
            "npc/keeper",
        ),
        (
            serde_json::json!({ "type": "despawn-npc", "npc": "npc/keeper" }),
            "npc/keeper",
        ),
        (
            serde_json::json!({ "type": "set-block", "anchor": "anchor/altar", "block": "minecraft:stone" }),
            "anchor/altar",
        ),
    ] {
        assert_eq!(subject(effect.clone()).as_deref(), Some(want), "{effect}");
    }
}

/// Several objects, or none, resolve nothing: naming one of them would be
/// the compiler guessing which the beat is about. A `move-actor` walks an
/// actor to an anchor and names both.
#[test]
fn an_effect_with_no_single_object_resolves_nothing() {
    for effect in [
        serde_json::json!({ "type": "move-actor", "actor": "actor/giant", "to": { "anchor": "anchor/door" } }),
        serde_json::json!({ "type": "move-npc", "npc": "npc/keeper", "to": { "anchor": "anchor/door" } }),
        serde_json::json!({ "type": "teleport", "from": { "anchor": "anchor/hall", "extent": [2, 2, 2] }, "to": { "anchor": "anchor/door" } }),
        serde_json::json!({ "type": "narrate", "text": "The hall answers." }),
        serde_json::json!({ "type": "set-flag", "flag": "flag/heard" }),
    ] {
        assert_eq!(subject(effect.clone()), None, "{effect}");
    }
}

/// A stated subject wins, and says so.
#[test]
fn a_stated_subject_is_not_derived() {
    let e: QuestEffect = serde_json::from_value(serde_json::json!({
        "type": "open-gate",
        "anchor": "anchor/door",
        "happening": { "verb": "opens", "text": "He lifts it.", "subject": "npc/keeper" }
    }))
    .unwrap();
    let s = e.happening_subject().expect("a stated subject");
    assert_eq!((s.id, s.derived), ("npc/keeper", false));
}

/// No `happening`, no subject: the derivation is about a beat, not about an
/// effect.
#[test]
fn an_effect_with_no_happening_has_no_subject() {
    let e: QuestEffect =
        serde_json::from_value(serde_json::json!({ "type": "open-gate", "anchor": "anchor/door" }))
            .unwrap();
    assert!(e.happening_subject().is_none());
    assert_eq!(e.subject_objects(), vec!["anchor/door"]);
}
