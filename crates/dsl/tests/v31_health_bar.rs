//! DSL v0.31 (spec-0073): a fight shows its health.
//!
//! What this file pins down:
//! * the surface — `health_bar { title?, range, color?, style? }` on `waves[]`
//!   and `actors[]`, one shared type, validates clean on every shape the spec's
//!   §3 table names, and the exported schema carries it with `range`'s
//!   `4..=64` and no enum for `color`/`style` (acceptance criterion 1);
//! * the document-tier refusals: `DW0909` (a bar over a body whose health
//!   cannot move), `DW0910` (a bar with nothing to title it, blank included),
//!   and `DW0100` for a `range` outside the schema's bound (criterion 4);
//! * the advisory: `DW0912` fires on a `boss`-billed wave and a `boss`-billed
//!   actor with no bar, as a warning, and is silent on a fixture carrying a
//!   `boss` fight WITH a bar and an `elite` fight WITHOUT one — the
//!   perturbation only this rule can tell apart (criterion 4);
//! * the l10n inventory: a stated title is a key of its own, a derived one is
//!   not (criterion 6, `docs/reference/i18n.md`).
//!
//! `DW0911` — a colour or style the pinned game does not draw — is judged
//! against the pinned command tree, which is `delvec`'s data, so its red
//! fixture lives beside the compiler (`crates/delvec/tests/emit.rs`).

mod common;

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::{
    DSL_VERSION, Diagnostic, HealthBarBinding, RawCampaign, Severity, check_campaign,
    l10n_inventory, parse_campaign, stage_schema,
};
use serde_json::Value;

/// hello-world's quests stage with the given `waves` and `actors` arrays, every
/// wave spawned and the listed actors staged and (optionally) unleashed from the
/// first objective's completion.
fn quests(waves: &str, actors: &str, unleash: &[&str], stage: &[&str]) -> String {
    let mut effects = vec![r#"{ "type": "open-gate", "anchor": "anchor/door" }"#.to_string()];
    let wave_ids: Vec<String> = serde_json::from_str::<Value>(waves)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap().to_string())
        .collect();
    for w in &wave_ids {
        effects.push(format!(r#"{{ "type": "spawn-wave", "wave": "{w}" }}"#));
    }
    for a in stage {
        effects.push(format!(r#"{{ "type": "spawn-actor", "actor": "{a}" }}"#));
    }
    for a in unleash {
        effects.push(format!(r#"{{ "type": "unleash-actor", "actor": "{a}" }}"#));
    }
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
          "obj/talk": [ {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "waves": {waves},
    "actors": {actors}
  }}
}}"#,
        effects = effects.join(",\n            ")
    )
}

fn raw(quests: String) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

fn diags(q: String) -> Vec<Diagnostic> {
    check_campaign(&raw(q))
}

fn with_code<'a>(d: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    d.iter().filter(|x| x.code == code).collect()
}

/// A one-entry named wave with a bar and no title (the derived title).
const LANE: &str = r#"{ "id": "wave/porter", "anchor": "anchor/keeper-stand",
    "mobs": [ { "entity": "minecraft:vindicator", "count": 1, "name": "The Porter",
                "attributes": { "max_health": 90.0 } } ],
    "tier": "elite",
    "health_bar": { "range": 16 } }"#;
/// A two-entry wave whose bar states its title and style.
const CHOIR: &str = r#"{ "id": "wave/choir", "anchor": "anchor/keeper-stand",
    "mobs": [ { "entity": "minecraft:drowned", "count": 4, "name": "Drowned Chorister" },
              { "entity": "minecraft:zombie", "count": 2 } ],
    "health_bar": { "title": "The Drowned Choir", "range": 24, "style": "notched_6" } }"#;

/// An actor with a name and a bar, optionally `vulnerable`.
fn actor(id: &str, name: Option<&str>, vulnerable: bool, bar: &str) -> String {
    let name = name
        .map(|n| format!(r#", "name": "{n}""#))
        .unwrap_or_default();
    let vul = if vulnerable {
        r#", "vulnerable": true"#
    } else {
        ""
    };
    format!(
        r#"{{ "id": "{id}", "entity": "minecraft:wither_skeleton",
             "anchor": "anchor/keeper-stand"{name}{vul}{bar} }}"#
    )
}

/// Every shape spec-0073 §3 names validates clean: a one named body with a
/// derived title, a body that is many with a stated title and style, a staged
/// elite that is unleashed, and a damageable puppet nothing unleashes.
#[test]
fn every_shape_the_spec_names_validates_clean() {
    let actors = format!(
        "[{}, {}]",
        actor(
            "actor/knight",
            Some("The Kneeling Knight"),
            false,
            r#", "tier": "boss", "health_bar": { "range": 20, "color": "red" }"#
        ),
        actor(
            "actor/creep",
            Some("A Creep"),
            true,
            r#", "health_bar": { "range": 8 }"#
        ),
    );
    let d = diags(quests(
        &format!("[{LANE}, {CHOIR}]"),
        &actors,
        &["actor/knight"],
        &["actor/knight", "actor/creep"],
    ));
    assert!(d.is_empty(), "the bar surface must validate clean: {d:#?}");
    let c = parse_campaign(&raw(quests(
        &format!("[{LANE}, {CHOIR}]"),
        &actors,
        &["actor/knight"],
        &["actor/knight", "actor/creep"],
    )))
    .unwrap();
    let b = HealthBarBinding::of(&c);
    assert_eq!(
        (b.fights, b.with_bar, b.boss, b.boss_with_bar),
        (4, 4, 1, 1)
    );
    assert_eq!((b.refused, b.advised), (0, 0));
    assert!(
        b.line().contains("4 of 4 fight(s) carry a bar"),
        "{}",
        b.line()
    );
}

/// The exported schema carries the field on both classes, one shared type, with
/// `range`'s bound and NO enum for `color`/`style` — the vocabulary is the
/// pinned command tree's, never a second copy in the schema.
#[test]
fn the_schema_exports_the_bar_on_waves_and_actors() {
    let schema = serde_json::to_value(stage_schema(Stage::Quests)).unwrap();
    let text = schema.to_string();
    let defs = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .expect("schema has definitions");
    let bar = &defs["HealthBar"];
    let props = bar["properties"]
        .as_object()
        .expect("HealthBar has properties");
    let mut names: Vec<&str> = props.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["color", "range", "style", "title"]);
    assert_eq!(bar["required"], serde_json::json!(["range"]));
    assert_eq!(props["range"]["minimum"], 4);
    assert_eq!(props["range"]["maximum"], 64);
    for field in ["color", "style"] {
        assert!(
            props[field].get("enum").is_none() && !props[field].to_string().contains("\"enum\""),
            "`{field}` carries no enum: {}",
            props[field]
        );
    }
    for class in ["Wave", "Actor"] {
        let p = &defs[class]["properties"]["health_bar"];
        assert!(
            p.to_string().contains("HealthBar"),
            "`{class}.health_bar` is the shared type: {p}"
        );
    }
    assert!(text.contains("health_bar"));
}

/// `DW0909`: a bar on an actor that is neither `vulnerable` nor unleashed reads
/// an invulnerable puppet and could never move. Unleashing it, or marking it
/// `vulnerable`, is each on its own enough to clear the refusal.
#[test]
fn dw0909_a_bar_over_a_body_whose_health_cannot_move() {
    let bar = r#", "health_bar": { "range": 12 }"#;
    let still = actor("actor/effigy", Some("The Effigy"), false, bar);
    let d = diags(quests(
        &format!("[{LANE}]"),
        &format!("[{still}]"),
        &[],
        &["actor/effigy"],
    ));
    let hits = with_code(&d, "DW0909");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].path, "/content/actors/0/health_bar");
    assert_eq!(hits[0].severity, Severity::Error);
    assert!(
        hits[0].message.contains("actor/effigy"),
        "{}",
        hits[0].message
    );

    let unleashed = diags(quests(
        &format!("[{LANE}]"),
        &format!("[{still}]"),
        &["actor/effigy"],
        &["actor/effigy"],
    ));
    assert!(with_code(&unleashed, "DW0909").is_empty(), "{unleashed:#?}");
    let vulnerable = actor("actor/effigy", Some("The Effigy"), true, bar);
    let d = diags(quests(
        &format!("[{LANE}]"),
        &format!("[{vulnerable}]"),
        &[],
        &["actor/effigy"],
    ));
    assert!(with_code(&d, "DW0909").is_empty(), "{d:#?}");
}

/// `DW0910`: a bar with nothing to title it. A wave of two entries, an unnamed
/// one-entry wave, an unnamed actor and a blank `title` are each refused; a
/// stated title clears every one of them.
#[test]
fn dw0910_a_bar_with_nothing_to_title_it() {
    let untitled_choir = CHOIR.replace(r#""title": "The Drowned Choir", "#, "");
    let d = diags(quests(&format!("[{untitled_choir}]"), "[]", &[], &[]));
    let hits = with_code(&d, "DW0910");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].path, "/content/waves/0/health_bar");
    assert!(
        hits[0].message.contains("2 mob entries"),
        "{}",
        hits[0].message
    );

    let unnamed = LANE.replace(r#", "name": "The Porter""#, "");
    let d = diags(quests(&format!("[{unnamed}]"), "[]", &[], &[]));
    assert_eq!(with_code(&d, "DW0910").len(), 1, "{d:#?}");

    let nameless_actor = actor(
        "actor/creep",
        None,
        true,
        r#", "health_bar": { "range": 8 }"#,
    );
    let d = diags(quests(
        &format!("[{LANE}]"),
        &format!("[{nameless_actor}]"),
        &[],
        &["actor/creep"],
    ));
    let hits = with_code(&d, "DW0910");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].path, "/content/actors/0/health_bar");

    let blank = CHOIR.replace("The Drowned Choir", "  ");
    let d = diags(quests(&format!("[{blank}]"), "[]", &[], &[]));
    let hits = with_code(&d, "DW0910");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].path, "/content/waves/0/health_bar/title");

    let titled_actor = actor(
        "actor/creep",
        None,
        true,
        r#", "health_bar": { "range": 8, "title": "A Creep" }"#,
    );
    let d = diags(quests(
        &format!("[{LANE}, {CHOIR}]"),
        &format!("[{titled_actor}]"),
        &[],
        &["actor/creep"],
    ));
    assert!(with_code(&d, "DW0910").is_empty(), "{d:#?}");
}

/// `range` outside `4..=64` is `DW0100` — the exported schema's own bound,
/// restated at the document tier because serde does not enforce it. Both ends
/// are inclusive.
#[test]
fn a_range_outside_the_schema_bound_is_dw0100() {
    for (range, refused) in [(3, true), (4, false), (64, false), (65, true)] {
        let wave = LANE.replace(r#""range": 16"#, &format!(r#""range": {range}"#));
        let d = diags(quests(&format!("[{wave}]"), "[]", &[], &[]));
        let hits: Vec<_> = with_code(&d, "DW0100")
            .into_iter()
            .filter(|x| x.path == "/content/waves/0/health_bar/range")
            .collect();
        assert_eq!(hits.len(), usize::from(refused), "range {range}: {d:#?}");
    }
}

/// `DW0912` fires on a `boss`-billed wave and on a `boss`-billed actor that
/// declare no bar, as a WARNING — nothing refuses — and names each.
#[test]
fn dw0912_advises_a_boss_fight_with_no_bar() {
    let boss_wave = LANE
        .replace(r#""tier": "elite""#, r#""tier": "boss""#)
        .replace(
            r#",
    "health_bar": { "range": 16 }"#,
            "",
        );
    let boss_actor = actor(
        "actor/knight",
        Some("The Kneeling Knight"),
        false,
        r#", "tier": "boss""#,
    );
    let d = diags(quests(
        &format!("[{boss_wave}]"),
        &format!("[{boss_actor}]"),
        &["actor/knight"],
        &["actor/knight"],
    ));
    let hits = with_code(&d, "DW0912");
    assert_eq!(hits.len(), 2, "{d:#?}");
    assert!(hits.iter().all(|x| x.severity == Severity::Warning));
    assert_eq!(hits[0].path, "/content/waves/0/tier");
    assert_eq!(hits[1].path, "/content/actors/0/tier");
    assert!(
        d.iter().all(|x| x.severity == Severity::Warning),
        "the advisory refuses nothing: {d:#?}"
    );
}

/// The silent side, and the perturbation only this rule can tell apart: a
/// `boss` fight WITH a bar and an `elite` fight WITHOUT one. Moving the bar
/// from the boss to the elite is what makes it fire.
#[test]
fn dw0912_is_silent_on_a_boss_with_a_bar_and_an_elite_without() {
    let boss_with = LANE.replace(r#""tier": "elite""#, r#""tier": "boss""#);
    let elite_without = actor(
        "actor/knight",
        Some("The Kneeling Knight"),
        false,
        r#", "tier": "elite""#,
    );
    let d = diags(quests(
        &format!("[{boss_with}]"),
        &format!("[{elite_without}]"),
        &["actor/knight"],
        &["actor/knight"],
    ));
    assert!(d.is_empty(), "{d:#?}");

    // The same two fights with the bar moved: now the boss has none.
    let boss_without = boss_with.replace(
        r#",
    "health_bar": { "range": 16 }"#,
        "",
    );
    let elite_with = actor(
        "actor/knight",
        Some("The Kneeling Knight"),
        false,
        r#", "tier": "elite", "health_bar": { "range": 16 }"#,
    );
    let d = diags(quests(
        &format!("[{boss_without}]"),
        &format!("[{elite_with}]"),
        &["actor/knight"],
        &["actor/knight"],
    ));
    let hits = with_code(&d, "DW0912");
    assert_eq!(hits.len(), 1, "{d:#?}");
    assert_eq!(hits[0].path, "/content/waves/0/tier");
}

/// A stated title is a key of its own; a derived one is the name's key and
/// adds nothing, so the character is translated once.
#[test]
fn a_stated_title_is_inventoried_and_a_derived_one_is_not() {
    let c = parse_campaign(&raw(quests(&format!("[{LANE}, {CHOIR}]"), "[]", &[], &[]))).unwrap();
    let inv = l10n_inventory(&c);
    assert_eq!(
        inv.get("wave.choir.health_bar.title").map(String::as_str),
        Some("The Drowned Choir")
    );
    assert!(
        !inv.contains_key("wave.porter.health_bar.title"),
        "{inv:#?}"
    );
    assert_eq!(
        inv.get("wave.porter.mob.0.name").map(String::as_str),
        Some("The Porter")
    );
}
