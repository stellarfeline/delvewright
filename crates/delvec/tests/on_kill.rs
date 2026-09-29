//! A kill pays (spec-0074 §4, §8.2, §8.3): whether a fight comes back, and the
//! `fires` judgement that is owed exactly there.
//!
//! `DW0914` (`every-kill` on a fight that never comes back) and `DW0915` (no
//! `fires` on one that does) are one pair over one predicate
//! (`onkill::fight_comes_back`). The pair is exercised three ways on a fight of
//! each kind, as §8 demands: on a fight that comes back `every-kill` and
//! `first-kill` pass and an absent `fires` reds `DW0915`; on one that does not,
//! `every-kill` reds `DW0914` and the other two pass.

mod common;

use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::onkill::{ComesBack, check_on_kill_fires, fight_comes_back};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, Fight, parse_campaign};

/// `souls-bonfire`: `wave/guards` (`respawns_on_rest`, 2 bodies, spawned once),
/// `wave/ambush` (2 bodies, spawned once, untiered) and a bonfire.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("fixture parses")
}

fn bundle(fires: Option<&str>) -> Option<delvewright_dsl::OnKill> {
    let fires = fires
        .map(|f| format!(r#""fires": "{f}", "#))
        .unwrap_or_default();
    Some(
        serde_json::from_str(&format!(
            r#"{{ {fires}"effects": [ {{ "type": "play-sound",
                "sound": "minecraft:entity.experience_orb.pickup" }} ] }}"#
        ))
        .unwrap(),
    )
}

fn codes(c: &Campaign) -> Vec<(String, String, String)> {
    check_on_kill_fires(c)
        .into_iter()
        .map(|d| (d.code.to_string(), d.path, d.message))
        .collect()
}

/// **The pair, on a fight that comes back** — `wave/guards` is
/// `respawns_on_rest` in a campaign with a bonfire. Absent reds `DW0915` naming
/// the wave and the site that brings it back; both values pass.
#[test]
fn on_a_fight_that_comes_back_fires_is_owed_and_either_value_stands() {
    let mut absent = fixture();
    absent.quests.content.waves[0].on_kill = bundle(None);
    let d = codes(&absent);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].0, "DW0915");
    assert_eq!(d[0].1, "/content/waves/0/on_kill");
    for needle in [
        "wave/guards",
        "respawns_on_rest",
        "bonfire",
        "first-kill",
        "every-kill",
    ] {
        assert!(d[0].2.contains(needle), "missing `{needle}`: {}", d[0].2);
    }
    for fires in ["first-kill", "every-kill"] {
        let mut c = fixture();
        c.quests.content.waves[0].on_kill = bundle(Some(fires));
        assert!(
            codes(&c).is_empty(),
            "`{fires}` on a fight that comes back stands"
        );
    }
}

/// **The pair, on a fight that never comes back** — `wave/ambush` is spawned
/// once and is neither `respawns_on_rest` nor billed. `every-kill` reds
/// `DW0914` naming what the wave lacks; absent and `first-kill` pass.
#[test]
fn on_a_fight_that_never_comes_back_every_kill_is_inert_and_the_rest_stand() {
    let mut every = fixture();
    every.quests.content.waves[1].on_kill = bundle(Some("every-kill"));
    let d = codes(&every);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].0, "DW0914");
    assert_eq!(d[0].1, "/content/waves/1/on_kill/fires");
    for needle in ["wave/ambush", "respawns_on_rest", "elite", "first-kill"] {
        assert!(d[0].2.contains(needle), "missing `{needle}`: {}", d[0].2);
    }
    for fires in [None, Some("first-kill")] {
        let mut c = fixture();
        c.quests.content.waves[1].on_kill = bundle(fires);
        assert!(
            codes(&c).is_empty(),
            "{fires:?} on a fight that never returns stands"
        );
    }
}

/// Every way a fight comes back is one answer of the one predicate, and the
/// build-time question (`Plan::fight_comes_back`, asked with the plan's
/// collected rest points) agrees with the document-time one.
#[test]
fn every_way_a_fight_comes_back_is_named() {
    let c = fixture();
    let guards = Fight::Wave(&c.quests.content.waves[0]);
    let ambush = Fight::Wave(&c.quests.content.waves[1]);
    assert_eq!(
        fight_comes_back(&c, true, guards),
        Some(ComesBack::RespawnsOnRest)
    );
    assert_eq!(fight_comes_back(&c, true, ambush), None);
    assert_eq!(
        fight_comes_back(&c, false, guards),
        None,
        "without a rest point nothing re-seats it"
    );
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    assert_eq!(
        plan.fight_comes_back(guards),
        Some(ComesBack::RespawnsOnRest)
    );
    assert_eq!(plan.fight_comes_back(ambush), None);

    // A billed wave: re-seated while it stands.
    let mut billed = fixture();
    billed.quests.content.waves[1].tier = Some(delvewright_dsl::EncounterTier::Elite);
    assert_eq!(
        fight_comes_back(&billed, true, Fight::Wave(&billed.quests.content.waves[1])),
        Some(ComesBack::Undefeated(delvewright_dsl::EncounterTier::Elite))
    );

    // A seating beat that can fire more than once: a trigger with `once: false`.
    let mut repeating = fixture();
    let spawn: delvewright_dsl::QuestEffect = serde_json::from_str(
        r#"{ "type": "spawn-wave", "wave": "wave/ambush",
             "happening": { "verb": "arrives", "text": "Again." } }"#,
    )
    .unwrap();
    repeating.quests.content.triggers[0].once = false;
    repeating.quests.content.triggers[0]
        .effects
        .push(spawn.clone());
    match fight_comes_back(
        &repeating,
        false,
        Fight::Wave(&repeating.quests.content.waves[1]),
    ) {
        Some(ComesBack::Repeats { path, why }) => {
            assert_eq!(path, "/content/triggers/0/effects/1");
            assert!(why.contains("once: false"), "{why}");
        }
        other => panic!("a repeating trigger re-seats the wave: {other:?}"),
    }

    // Two beats that each fire once seat it twice.
    let mut twice = fixture();
    twice.quests.content.triggers[0].effects.push(spawn);
    match fight_comes_back(&twice, false, Fight::Wave(&twice.quests.content.waves[1])) {
        Some(ComesBack::Repeats { path, why }) => {
            assert_eq!(path, "/content/triggers/0/effects/1");
            assert!(why.contains("second beat"), "{why}");
        }
        other => panic!("a second seating beat re-seats the wave: {other:?}"),
    }

    // An unleashed actor in a campaign with a rest point: re-stood while it
    // stands.
    let mut hostile = fixture();
    let anchor = hostile.quests.content.waves[0].anchor.as_str().to_string();
    hostile.quests.content.actors.push(
        serde_json::from_str(&format!(
            r#"{{ "id": "actor/moth", "entity": "minecraft:bat", "anchor": "{anchor}" }}"#
        ))
        .unwrap(),
    );
    hostile.quests.content.quests[0].on_complete.insert(
        0,
        serde_json::from_str(
            r#"{ "type": "unleash-actor", "actor": "actor/moth",
                     "happening": { "verb": "arrives", "text": "It wakes." } }"#,
        )
        .unwrap(),
    );
    let moth = Fight::Actor(&hostile.quests.content.actors[0]);
    assert_eq!(
        fight_comes_back(&hostile, true, moth),
        Some(ComesBack::Unleashed)
    );
    assert_eq!(fight_comes_back(&hostile, false, moth), None);
}
