//! The arming-before-adjudication invariant in `tick.mcfunction` (task #124).
//!
//! `tick`'s objective-completion loop is the only place a quest gets ARMED: a
//! completion line runs `complete_<obj>` → `check_q_<quest>` → `complete_q_<quest>`,
//! and that writes `#party dw.qa_<next>` for every quest triggered by this one's
//! completion. Every other quest gate in the tick only READS those scores.
//!
//! So a quest's completion lines must be emitted before the lines of any quest it
//! arms. When they are not, an `interact` loses a click in silence: it adjudicates
//! under `if score #party dw.qa_<q> matches 1` and the very next line resets the
//! trigger UNCONDITIONALLY, so a click already pending when the quest is armed
//! later in the same tick is consumed with no effect. A human clicks again and
//! never knows; the validation bot clicks once and times out.
//!
//! Declaration order gave this for free on every campaign built so far — which is
//! precisely why it needed pinning: nothing made it true. Nothing in the DSL
//! requires a quest to be declared after the quest that arms it; `quest-complete`
//! is a reference, not an ordering constraint.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("emission succeeds")
}

/// `(quest id, the quest whose completion arms it, its objective ids)` straight
/// out of the campaign's own `quests.json` — the test never re-derives what the
/// DSL states.
type QuestShape = Vec<(String, Option<String>, Vec<String>)>;

fn quest_shape(dir: &Path) -> QuestShape {
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("quests.json")).unwrap()).unwrap();
    raw["content"]["quests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| {
            let armer = (q["trigger"]["type"] == "quest-complete")
                .then(|| q["trigger"]["quest"].as_str().unwrap().to_string());
            let objs = q["objectives"]
                .as_array()
                .unwrap()
                .iter()
                .map(|o| o["id"].as_str().unwrap().to_string())
                .collect();
            (q["id"].as_str().unwrap().to_string(), armer, objs)
        })
        .collect()
}

/// `safe_local`, as the emitted function names spell it.
fn safe(id: &str) -> String {
    id.rsplit('/')
        .next()
        .unwrap()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The first and last tick line that dispatches any of `objectives`' completion
/// functions. `None` when this quest has no tick-driven objective at all — a
/// `talk-to`-only quest completes from a dialogue function, which the tick
/// dispatches far above this loop, so it is never part of the ordering question.
fn line_span(tick: &str, ns: &str, objectives: &[String]) -> Option<(usize, usize)> {
    let mut span: Option<(usize, usize)> = None;
    for (i, line) in tick.lines().enumerate() {
        if objectives
            .iter()
            .any(|o| line.contains(&format!("{ns}:complete_o_{}", safe(o))))
        {
            span = Some(match span {
                None => (i, i),
                Some((lo, _)) => (lo, i),
            });
        }
    }
    span
}

/// The invariant over one built campaign. Returns how many arming edges it was
/// able to check, so a caller can tell "held everywhere" from "had nothing to
/// look at".
fn check_arming_order(name: &str, dir: &Path, ns: &str, out: &BuildOutput) -> usize {
    let tick = String::from_utf8(
        out.get(&format!("datapack/data/{ns}/function/tick.mcfunction"))
            .unwrap_or_else(|| panic!("{name}: no tick"))
            .clone(),
    )
    .unwrap();
    let shape = quest_shape(dir);
    let mut checked = 0;
    for (qid, armer, objs) in &shape {
        let Some(armer_id) = armer else { continue };
        let Some((_, armer_end)) = shape
            .iter()
            .find(|(id, _, _)| id == armer_id)
            .and_then(|(_, _, o)| line_span(&tick, ns, o))
        else {
            continue;
        };
        let Some((armed_start, _)) = line_span(&tick, ns, objs) else {
            continue;
        };
        assert!(
            armer_end < armed_start,
            "{name}: `{armer_id}` arms `{qid}`, but its last completion line ({armer_end}) comes \
             AFTER the first line of the quest it arms ({armed_start}). A click pending on `{qid}` \
             when `{armer_id}` completes would be adjudicated against an unarmed quest and then \
             reset — silently spent.",
        );
        checked += 1;
    }
    checked
}

fn families() -> Vec<(&'static str, PathBuf, &'static str)> {
    vec![
        ("keep-trial", common::keep_trial_dir(), "keep-trial"),
        ("keep-crawl", common::keep_crawl_dir(), "keep-crawl"),
        (
            "keep-vertical",
            common::keep_vertical_dir(),
            "keep-vertical",
        ),
        (
            "v04-showcase",
            common::compiler_fixtures_dir().join("v04-showcase"),
            "v04-showcase",
        ),
        (
            "souls-bonfire",
            common::compiler_fixtures_dir().join("souls-bonfire"),
            "souls-bonfire",
        ),
        (
            "v06-checkpoints",
            common::compiler_fixtures_dir().join("v06-checkpoints"),
            "v06-checkpoints",
        ),
    ]
}

/// Mechanical sweep: no shipped fixture may violate the invariant.
///
/// Every fixture today is armed by a `talk-to`-only quest, so its arming happens
/// in a dialogue function the tick dispatches long before this loop and there is
/// nothing here to order — the sweep is a guard for the fixtures that come next,
/// not a proof about the ones that are here. The positive case, with a real
/// tick-driven arming edge, is
/// [`a_campaign_declared_out_of_arming_order_still_arms_first`].
#[test]
fn no_shipped_fixture_adjudicates_before_it_arms() {
    for (name, dir, ns) in families() {
        let out = build_dir(&dir);
        check_arming_order(name, &dir, ns, &out);
    }
}

/// Materialize keep-trial with a THIRD quest — an `interact` armed by
/// `quest/trial`, whose own objectives complete from the tick — and declare it
/// FIRST, before the quest that arms it.
///
/// This is the drowned bell's shape (`obj/raise-it` in `quest/ring-it-home`,
/// armed by the kill that ends `quest/the-keeper`) reduced to a fixture, and it
/// is legal DSL: nothing requires a quest to be declared after its armer.
fn out_of_order_campaign() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dw-tick-arming-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &dir);
    // The new quest adds player-visible strings the sidecar cannot know about,
    // and this fixture is about emission order, not i18n coverage.
    common::make_english_only(&dir);

    let plan_path = dir.join("quest-plan.json");
    let mut plan: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&plan_path).unwrap()).unwrap();
    plan["content"]["quests"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "quest/after",
            "goal": "Ring the bell the shrine answers to.",
            "area": "area/keep",
            "npcs": ["npc/keeper"],
            "depends_on": ["quest/trial"],
            "mandatory": true,
            "act": 3
        }));
    plan["content"]["finale"] = serde_json::json!("quest/after");
    std::fs::write(&plan_path, serde_json::to_string_pretty(&plan).unwrap()).unwrap();

    let quests_path = dir.join("quests.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&quests_path).unwrap()).unwrap();
    let list = quests["content"]["quests"].as_array_mut().unwrap();
    for q in list.iter_mut() {
        if q["id"] == "quest/trial" {
            q["on_complete"] = serde_json::json!([]);
        }
    }
    let after = serde_json::json!({
        "id": "quest/after",
        "trigger": { "type": "quest-complete", "quest": "quest/trial" },
        "objectives": [{
            "type": "interact",
            "id": "obj/ring",
            "title": "Ring the Bell",
            "hint": "The bell above the shrine still has a rope.",
            "anchor": "anchor/gate"
        }],
        "on_objective_complete": {},
        "on_complete": [{ "type": "campaign-complete" }]
    });
    list.insert(0, after);
    std::fs::write(&quests_path, serde_json::to_string_pretty(&quests).unwrap()).unwrap();
    dir
}

#[test]
fn a_campaign_declared_out_of_arming_order_still_arms_first() {
    let dir = out_of_order_campaign();
    let out = build_dir(&dir);
    let checked = check_arming_order("keep-trial+after", &dir, "keep-trial", &out);
    assert!(
        checked > 0,
        "the fixture must present a real tick-driven arming edge, or it proves nothing"
    );

    let tick = String::from_utf8(
        out.get("datapack/data/keep-trial/function/tick.mcfunction")
            .unwrap()
            .clone(),
    )
    .unwrap();
    // Declared first, emitted last: the arming graph decides, not the JSON array.
    let door = tick
        .find("keep-trial:complete_o_door")
        .expect("trial's interact adjudicates");
    let shrine = tick
        .find("keep-trial:complete_o_shrine")
        .expect("trial's last objective adjudicates — this is the line that ARMS `quest/after`");
    let ring = tick
        .find("keep-trial:complete_o_ring")
        .expect("after's interact adjudicates");
    assert!(
        door < ring && shrine < ring,
        "every line of the arming quest must precede the armed quest's interact"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of the owner's ruling: a trigger fired before its quest is
/// armed is DISCARDED, never banked.
///
/// The reset line carries no guard at all. If it ever grew one, a click fired
/// long before arming would sit in the score and auto-complete the objective the
/// moment the quest armed, with no real click behind it — the objective would
/// complete itself. Losing a click is a bug; fabricating one is worse, so the
/// unconditional reset is the correct half of the pair and the ordering above is
/// what makes losing it impossible.
#[test]
fn the_interact_trigger_reset_is_unconditional() {
    let out = build_dir(&common::keep_trial_dir());
    let tick = String::from_utf8(
        out.get("datapack/data/keep-trial/function/tick.mcfunction")
            .unwrap()
            .clone(),
    )
    .unwrap();
    let reset = tick
        .lines()
        .find(|l| l.contains("scoreboard players reset @s dw.i_door"))
        .expect("the interact trigger is reset every tick");
    assert_eq!(
        reset, "execute as @a[scores={dw.i_door=1..}] run scoreboard players reset @s dw.i_door",
        "the reset takes the trigger and NOTHING else: no quest gate, no objective gate — a \
         premature click is spent, never banked",
    );
    // …and it comes after the adjudication, so one click yields at most one
    // completion and is then gone.
    assert!(
        tick.find("keep-trial:complete_o_door").unwrap()
            < tick.find("scoreboard players reset @s dw.i_door").unwrap()
    );
}
