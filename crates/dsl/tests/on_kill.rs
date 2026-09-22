//! A kill pays (spec-0074), document tier: the `on_kill` surface the schema
//! exports, and the two refusals that need only the documents — `DW0913` (a
//! bundle no credited kill can reach) and `DW0100` (an empty bundle).
//!
//! The pair that needs the rest points (`DW0914`/`DW0915`) is compiler-side and
//! is exercised in `crates/delvec/tests/on_kill.rs`.

use delvewright_dsl::{
    Campaign, DSL_VERSION, Diagnostic, RawCampaign, Stage, parse_campaign, stage_schema,
    validate_campaign,
};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("hello-world/{name}: {e}"))
}

/// A hello-world campaign whose objective `obj/talk` fires `on_talk` and whose
/// quests stage carries `extra` inside `content`.
fn campaign(on_talk: &str, extra: &str) -> Campaign {
    let quests = format!(
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{on_talk} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "state": [ {{ "id": "state/purse", "scope": "party", "initial": 0 }} ]{extra}
  }}
}}"#
    );
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

const BUNDLE: &str =
    r#"{ "effects": [ { "type": "add-state", "state": "state/purse", "amount": 1 } ] }"#;

fn wave(id: &str, on_kill: &str) -> String {
    format!(
        r#"{{ "id": "{id}", "anchor": "anchor/exit",
             "mobs": [ {{ "entity": "minecraft:zombie", "count": 2 }} ],
             "on_kill": {on_kill} }}"#
    )
}

fn actor(id: &str, vulnerable: bool, on_kill: &str) -> String {
    format!(
        r#"{{ "id": "{id}", "entity": "minecraft:zombie", "anchor": "anchor/exit",
             "vulnerable": {vulnerable}, "on_kill": {on_kill} }}"#
    )
}

fn with(code: &str, d: &[Diagnostic]) -> Vec<Diagnostic> {
    d.iter().filter(|x| x.code == code).cloned().collect()
}

/// A wave no beat spawns has no bodies and no kill machinery, so its bundle can
/// never fire: `DW0913`, at the bundle, naming the wave.
#[test]
fn a_bundle_on_a_wave_nothing_spawns_is_dw0913() {
    let c = campaign(
        "",
        &format!(r#", "waves": [ {} ]"#, wave("wave/idle", BUNDLE)),
    );
    let d = with("DW0913", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/waves/0/on_kill");
    assert!(d[0].message.contains("wave/idle"), "{}", d[0].message);
    assert!(d[0].message.contains("spawn"), "{}", d[0].message);
}

/// The same wave, spawned by a beat, resolves an area and its bundle stands.
#[test]
fn a_bundle_on_a_spawned_wave_stands() {
    let c = campaign(
        r#", { "type": "spawn-wave", "wave": "wave/idle",
               "happening": { "verb": "arrives", "text": "They come." } }"#,
        &format!(r#", "waves": [ {} ]"#, wave("wave/idle", BUNDLE)),
    );
    assert!(with("DW0913", &validate_campaign(&c)).is_empty());
}

/// A wave spawned only from another wave's `on_kill` is seated where that wave
/// stands (`wave_area` resolves through the owner), so its bundle stands too.
#[test]
fn a_wave_spawned_by_a_kill_resolves_through_the_fight_that_spawns_it() {
    let spawns_second = r#"{ "effects": [
        { "type": "spawn-wave", "wave": "wave/second",
          "happening": { "verb": "arrives", "text": "More come." } } ] }"#;
    let c = campaign(
        r#", { "type": "spawn-wave", "wave": "wave/first",
               "happening": { "verb": "arrives", "text": "They come." } }"#,
        &format!(
            r#", "waves": [ {}, {} ]"#,
            wave("wave/first", spawns_second),
            wave("wave/second", BUNDLE)
        ),
    );
    assert!(
        with("DW0913", &validate_campaign(&c)).is_empty(),
        "{:#?}",
        validate_campaign(&c)
    );
    let first = delvewright_dsl::wave_area(&c, "wave/first");
    assert!(first.is_some(), "the owner wave is seated by a quest");
    assert_eq!(delvewright_dsl::wave_area(&c, "wave/second"), first);
}

/// An actor never unleashed and not `vulnerable` is `Invulnerable` for the whole
/// delve: `DW0913`, naming the actor. `vulnerable` or an `unleash-actor`
/// discharges it.
#[test]
fn a_bundle_on_an_actor_nobody_can_kill_is_dw0913_until_it_can_be() {
    let caged = campaign(
        "",
        &format!(r#", "actors": [ {} ]"#, actor("actor/usher", false, BUNDLE)),
    );
    let d = with("DW0913", &validate_campaign(&caged));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/actors/0/on_kill");
    assert!(d[0].message.contains("actor/usher"), "{}", d[0].message);
    assert!(d[0].message.contains("vulnerable"), "{}", d[0].message);

    let vulnerable = campaign(
        "",
        &format!(r#", "actors": [ {} ]"#, actor("actor/usher", true, BUNDLE)),
    );
    assert!(with("DW0913", &validate_campaign(&vulnerable)).is_empty());

    let unleashed = campaign(
        r#", { "type": "unleash-actor", "actor": "actor/usher",
               "happening": { "verb": "arrives", "text": "It stands." } }"#,
        &format!(r#", "actors": [ {} ]"#, actor("actor/usher", false, BUNDLE)),
    );
    assert!(with("DW0913", &validate_campaign(&unleashed)).is_empty());
}

/// An empty bundle is the exported schema's `minItems: 1`, which serde does not
/// enforce, so it is refused at the schema tier by name.
#[test]
fn an_empty_bundle_is_dw0100() {
    let c = campaign(
        "",
        &format!(
            r#", "actors": [ {} ]"#,
            actor("actor/usher", true, r#"{ "effects": [] }"#)
        ),
    );
    let d = with("DW0100", &validate_campaign(&c));
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/actors/0/on_kill/effects");
}

/// **The surface** (acceptance criterion 1): `on_kill` is one type on both
/// `Wave` and `Actor`; `fires` is two variants with no default; `effects` has
/// `minItems: 1`.
#[test]
fn the_schema_exports_one_on_kill_type_on_both_fight_classes() {
    let schema = stage_schema(Stage::Quests);
    let defs = &schema["$defs"];
    let reference = |ty: &str| -> String {
        let prop = &defs[ty]["properties"]["on_kill"];
        let r = prop["$ref"]
            .as_str()
            .or_else(|| {
                prop["anyOf"]
                    .as_array()?
                    .iter()
                    .find_map(|b| b["$ref"].as_str())
            })
            .unwrap_or_else(|| panic!("{ty}.on_kill is not a reference: {prop}"));
        r.to_string()
    };
    assert_eq!(reference("Wave"), reference("Actor"), "one type on both");
    assert_eq!(reference("Wave"), "#/$defs/OnKill");
    let on_kill = &defs["OnKill"];
    assert_eq!(on_kill["properties"]["effects"]["minItems"], 1);
    let required: Vec<&str> = on_kill["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert_eq!(
        required,
        vec!["effects"],
        "`fires` is optional in the document"
    );
    let fires = &on_kill["properties"]["fires"];
    assert!(
        fires.get("default").is_none(),
        "`fires` has no default: {fires}"
    );
    let text = serde_json::to_string(&defs["KillFires"]).unwrap();
    for variant in ["first-kill", "every-kill"] {
        assert!(text.contains(variant), "{variant} missing: {text}");
    }
    let n = defs["KillFires"]["enum"]
        .as_array()
        .map(Vec::len)
        .or_else(|| defs["KillFires"]["oneOf"].as_array().map(Vec::len))
        .unwrap_or(0);
    assert_eq!(n, 2, "two variants: {text}");
}
