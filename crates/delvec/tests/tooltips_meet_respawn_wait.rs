//! Where spec-0078 (every dialog button carries an optional hover tooltip) meets
//! spec-0077 (a fallen player waits as a spectator before rejoining).
//!
//! Each feature proves itself in its own file (`button_tooltips.rs`,
//! `respawn_wait.rs`); this file proves the two pairs they share:
//!
//! * **the bonfire** — the tooltips sit on its rest/save buttons, and the wait
//!   releases a fallen player onto its seat. Neither feature may move the
//!   other's bytes, and a rest is not a release.
//! * **the dialog** — a waiting player is a spectator carrying the observation
//!   tag; no dialog, and so no button, may be put in front of one. Every
//!   `dialog show` is followed back to the root that can reach it, and each
//!   root must be one a waiting player cannot supply.
//!
//! The live half (a waiting body that clicks the bonfire opens nothing; a rest
//! during a wait leaves the waiter waiting and releases them onto the fire) is
//! `harness/probe/respawn-wait-party.ts`, step 8.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::observer;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{RespawnWait, parse_campaign};
use serde_json::{Value, json};

const BONFIRE_NS: &str = "souls-bonfire";
const SHOP_NS: &str = "economy";

/// Add `fields` to every `bonfire` effect anywhere in `v`; returns how many.
fn patch_bonfires(v: &mut Value, fields: &Value) -> usize {
    match v {
        Value::Object(map) => {
            let mut n = 0;
            if map.get("type") == Some(&json!("bonfire")) {
                for (k, f) in fields.as_object().expect("fields is an object") {
                    map.insert(k.clone(), f.clone());
                }
                n += 1;
            }
            for child in map.values_mut() {
                n += patch_bonfires(child, fields);
            }
            n
        }
        Value::Array(items) => items.iter_mut().map(|c| patch_bonfires(c, fields)).sum(),
        _ => 0,
    }
}

/// Build compiler fixture `ns`, with `tooltips` added to its bonfire (when
/// stated) and `wait` declared on its world, through the all-languages path that
/// tags every player-visible string with its key, as `delvec build` does.
fn build(ns: &str, tooltips: Option<Value>, wait: Option<RespawnWait>) -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(ns);
    let mut loaded = load_campaign_dir(&dir).unwrap();
    if let Some(fields) = &tooltips {
        let mut patched = 0;
        loaded.raw.quests = common::patch_doc(&loaded.raw.quests, |q| {
            patched = patch_bonfires(q, fields);
        });
        assert_eq!(patched, 1, "binding: `{ns}` holds exactly one bonfire");
    }
    let mut c = parse_campaign(&loaded.raw).unwrap_or_else(|e| panic!("{ns} parses: {e:?}"));
    c.world.content.respawn_wait = wait;
    delvewright_dsl::tag_translatables(&mut c);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &c.npcs.content.npcs {
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
        &skins,
    )
    .unwrap_or_else(|e| panic!("{ns} builds: {e:?}"))
}

const WAIT: RespawnWait = RespawnWait {
    seconds: 10,
    alone: false,
};

fn tips() -> Value {
    json!({
        "rest_tooltip": "Rest: the dead rise again, and the fire keeps you.",
        "save_tooltip": "Save: the fire keeps you; the dead stay dead."
    })
}

fn fn_body<'a>(out: &'a BuildOutput, ns: &str, name: &str) -> &'a str {
    let path = format!("datapack/data/{ns}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

/// The datapack paths whose bytes differ between two builds.
fn moved(a: &BuildOutput, b: &BuildOutput) -> Vec<String> {
    let keys: BTreeSet<&String> = a
        .keys()
        .chain(b.keys())
        .filter(|k| k.starts_with("datapack/"))
        .collect();
    keys.into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect()
}

/// **The bonfire pair, emitted.** The four builds `{no tooltip, tooltips} ×
/// {no wait, wait}` of the one-bonfire fixture:
///
/// * stating the tooltips moves the bonfire dialog and nothing else in the
///   datapack, with the wait declared exactly as without it (spec-0078's byte
///   identity holds under spec-0077);
/// * declaring the wait leaves the bonfire dialog byte-identical, with the
///   tooltips stated exactly as without them, and the wait's own functions are
///   byte-identical either way (spec-0077 never touches a button);
/// * with both: the dialog carries both tooltips on its two `/trigger dw.rest`
///   buttons, the release fires the respawn that seats the player on the
///   bonfire's own checkpoint cell, and a rest neither reads nor writes the
///   wait's clock — a rest during a wait is not a release.
#[test]
fn bonfire_tooltips_and_the_wait_never_move_each_others_bytes() {
    let ns = BONFIRE_NS;
    let dialog = format!("datapack/data/{ns}/dialog/bonfire_0.json");
    let plain = build(ns, None, None);
    let tipped = build(ns, Some(tips()), None);
    let waiting = build(ns, None, Some(WAIT));
    let both = build(ns, Some(tips()), Some(WAIT));

    assert_eq!(moved(&plain, &tipped), vec![dialog.clone()]);
    assert_eq!(
        moved(&waiting, &both),
        vec![dialog.clone()],
        "under a declared wait, the tooltips move the bonfire dialog and nothing else"
    );
    assert_eq!(plain.get(&dialog), waiting.get(&dialog));
    assert_eq!(
        tipped.get(&dialog),
        both.get(&dialog),
        "the wait moved the tooltipped bonfire dialog"
    );
    let rw: Vec<&String> = waiting
        .keys()
        .filter(|k| k.contains("/function/rw_"))
        .collect();
    assert!(rw.len() >= 4, "binding: {} rw_ function(s)", rw.len());
    for k in &rw {
        assert_eq!(waiting.get(*k), both.get(*k), "{k} moved with the tooltips");
    }

    let d: Value = serde_json::from_slice(both.get(&dialog).unwrap()).unwrap();
    let actions = d["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 2, "{d:#?}");
    for (a, (field, cmd)) in actions.iter().zip([
        ("rest_tooltip", "/trigger dw.rest set 2"),
        ("save_tooltip", "/trigger dw.rest set 1"),
    ]) {
        assert!(
            a["tooltip"]["translate"]
                .as_str()
                .is_some_and(|k| k.ends_with(field)),
            "{a:#?}"
        );
        assert_eq!(a["action"]["command"], cmd);
    }

    let release = fn_body(&both, ns, "rw_release");
    assert!(
        release.ends_with(&format!("function {ns}:cp_respawn_fire\n")),
        "{release}"
    );
    let fire = fn_body(&both, ns, "cp_respawn_fire");
    let rest_cp = fn_body(&both, ns, "bonfire_rest_0")
        .lines()
        .find_map(|l| l.strip_prefix("scoreboard players set #cp dw.sys "))
        .expect("the rest sets the active checkpoint")
        .to_string();
    assert!(
        fire.contains(&format!(
            "execute if score #cp dw.sys matches {rest_cp} run function {ns}:cp_seat_{rest_cp}"
        )),
        "the release seats on the checkpoint the bonfire's rest makes active: {fire}"
    );
    for f in ["bonfire_pick_rest_0", "bonfire_rest_0", "bonfire_restore"] {
        let body = fn_body(&both, ns, f);
        assert!(
            !body.contains("dw.rwait"),
            "{f} touches the wait's clock: {body}"
        );
        assert!(
            !body.contains("rw_release"),
            "{f} releases a waiter: {body}"
        );
    }
}

/// How a root reaches a `dialog show`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Root {
    /// An advancement whose every criterion is `player_interacted_with_entity`:
    /// a spectator's use of an entity does not fire it.
    Interaction(String),
    /// A tick line whose `as` selector is bound by a trigger objective's score:
    /// the answer to a dialog the player already has open.
    DialogAnswer(String),
    /// The class gate: a player with no class yet. A waiting player has one.
    Unclassed(String),
    /// Anything else — a root a waiting player might stand in.
    Other(String),
}

/// Every function in the shipped datapack, by `ns:path`.
fn functions(out: &BuildOutput) -> BTreeMap<String, String> {
    out.iter()
        .filter_map(|(p, b)| {
            let rest = p.strip_prefix("datapack/data/")?;
            let (ns, path) = rest.split_once("/function/")?;
            let path = path.strip_suffix(".mcfunction")?;
            Some((
                format!("{ns}:{path}"),
                String::from_utf8_lossy(b).into_owned(),
            ))
        })
        .collect()
}

/// Advancement rewards: function → (advancement path, every criterion trigger).
fn advancement_rewards(out: &BuildOutput) -> BTreeMap<String, Vec<(String, BTreeSet<String>)>> {
    let mut map: BTreeMap<String, Vec<(String, BTreeSet<String>)>> = BTreeMap::new();
    for (p, b) in out.iter() {
        if !(p.starts_with("datapack/data/") && p.contains("/advancement/")) {
            continue;
        }
        let j: Value = serde_json::from_slice(b).unwrap();
        if let Some(f) = j["rewards"]["function"].as_str() {
            let triggers = j["criteria"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(_, c)| c["trigger"].as_str().map(str::to_string))
                .collect();
            map.entry(f.to_string())
                .or_default()
                .push((p.clone(), triggers));
        }
    }
    map
}

/// The functions the vanilla `#minecraft:tick` / `#minecraft:load` tags run.
fn tagged_roots(out: &BuildOutput) -> BTreeSet<String> {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/data/minecraft/tags/function/"))
        .flat_map(|(_, b)| {
            let j: Value = serde_json::from_slice(b).unwrap();
            j["values"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Classify the call `line` (in a function a vanilla tag runs) as a root.
fn classify_tick_line(line: &str) -> Root {
    let l = line.trim();
    let answer = l.starts_with("execute as @a[scores={")
        && l.split_once("scores={")
            .and_then(|(_, r)| r.split_once('}'))
            .is_some_and(|(s, _)| s.starts_with("dw."));
    if answer {
        Root::DialogAnswer(l.to_string())
    } else if l.starts_with("execute as @a unless score @s dw.classed matches 1 ") {
        Root::Unclassed(l.to_string())
    } else {
        Root::Other(l.to_string())
    }
}

/// Every root that reaches a function holding a `dialog show`, and how many
/// such functions there are. Panics if a `dialog show` targets anyone but `@s`.
fn dialog_roots(out: &BuildOutput) -> (usize, BTreeSet<Root>) {
    let fns = functions(out);
    let advs = advancement_rewards(out);
    let tagged = tagged_roots(out);
    let mut callers: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (name, body) in &fns {
        for line in body.lines() {
            for (i, _) in line.match_indices("function ") {
                let tgt = line[i + "function ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or("");
                if fns.contains_key(tgt) {
                    callers.entry(tgt).or_default().push((name, line));
                }
            }
        }
    }
    let shows: Vec<&String> = fns
        .iter()
        .filter(|(_, b)| b.contains("dialog show"))
        .map(|(n, _)| n)
        .collect();
    for n in &shows {
        for l in fns[*n].lines().filter(|l| l.contains("dialog show")) {
            let target = l
                .split_once("dialog show ")
                .map(|(_, r)| r.split_whitespace().next().unwrap_or(""))
                .unwrap_or("");
            assert_eq!(
                target, "@s",
                "{n}: a dialog shown to anyone but `@s`: `{l}`"
            );
        }
    }
    let mut roots = BTreeSet::new();
    let mut stack: Vec<&str> = shows.iter().map(|s| s.as_str()).collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    while let Some(n) = stack.pop() {
        if !seen.insert(n) {
            continue;
        }
        for (adv, triggers) in advs.get(n).into_iter().flatten() {
            if !triggers.is_empty()
                && triggers
                    .iter()
                    .all(|t| t == "minecraft:player_interacted_with_entity")
            {
                roots.insert(Root::Interaction(adv.clone()));
            } else {
                roots.insert(Root::Other(format!("{adv} {triggers:?}")));
            }
        }
        if tagged.contains(n) {
            roots.insert(Root::Other(format!("#minecraft tag runs {n} directly")));
        }
        let cs = callers.get(n).cloned().unwrap_or_default();
        if cs.is_empty() && !advs.contains_key(n) && !tagged.contains(n) {
            roots.insert(Root::Other(format!("{n} has no caller")));
        }
        for (caller, line) in cs {
            if tagged.contains(caller) {
                roots.insert(classify_tick_line(line));
            } else {
                stack.push(caller);
            }
        }
    }
    (shows.len(), roots)
}

fn assert_no_dialog_reaches_a_waiter(out: &BuildOutput, ns: &str) -> BTreeSet<Root> {
    assert!(
        out.contains_key("validation/observer-census.json"),
        "{ns}: the build declares no wait"
    );
    let (shows, roots) = dialog_roots(out);
    let others: Vec<&Root> = roots
        .iter()
        .filter(|r| matches!(r, Root::Other(_)))
        .collect();
    println!(
        "{ns}: {shows} function(s) hold a `dialog show`, reached from {} root(s): {} interaction, {} dialog answer, {} unclassed, {} other",
        roots.len(),
        roots
            .iter()
            .filter(|r| matches!(r, Root::Interaction(_)))
            .count(),
        roots
            .iter()
            .filter(|r| matches!(r, Root::DialogAnswer(_)))
            .count(),
        roots
            .iter()
            .filter(|r| matches!(r, Root::Unclassed(_)))
            .count(),
        others.len()
    );
    assert!(
        shows > 0 && !roots.is_empty(),
        "binding: {shows} / {roots:?}"
    );
    assert!(
        others.is_empty(),
        "{ns}: a dialog is reachable from a root a waiting player can stand in: {others:#?}"
    );
    let census = observer::census(out);
    assert!(census.unguarded.is_empty(), "{:?}", census.unguarded);
    roots
}

/// **The dialog pair.** With a wait declared, on the bonfire fixture (class,
/// bonfire and dialogue dialogs, bonfire tooltips stated) and on the shop
/// fixture (class, shop and dialogue dialogs): every `dialog show` is shown to
/// `@s`, and every root that reaches one is an entity interaction (which a
/// spectator cannot fire — proven live by the probe), the answer to a dialog
/// the player already has open, or the class gate. Both fixtures bind the
/// interaction root, so the bonfire, the shop and the NPC openers are judged.
#[test]
fn a_waiting_player_is_offered_no_dialog() {
    let bonfire = build(BONFIRE_NS, Some(tips()), Some(WAIT));
    let b = assert_no_dialog_reaches_a_waiter(&bonfire, BONFIRE_NS);
    assert!(
        b.iter()
            .any(|r| matches!(r, Root::Interaction(a) if a.ends_with("/bf_0.json"))),
        "binding: the bonfire opener is not among {b:#?}"
    );
    let shop = build(SHOP_NS, None, Some(WAIT));
    let s = assert_no_dialog_reaches_a_waiter(&shop, SHOP_NS);
    assert!(
        s.iter()
            .any(|r| matches!(r, Root::Interaction(a) if a.contains("/advancement/shop_"))),
        "binding: the shop opener is not among {s:#?}"
    );
}

/// Perturbation toward the shape the pair test exists to catch: a tick line that
/// opens the bonfire's dialog for every player — a waiting one included — reds
/// it, naming the root.
#[test]
fn a_dialog_opened_from_a_bare_tick_line_is_caught() {
    let ns = BONFIRE_NS;
    let mut out = build(ns, Some(tips()), Some(WAIT));
    let tick = format!("datapack/data/{ns}/function/tick.mcfunction");
    let mut body = String::from_utf8(out.get(&tick).unwrap().clone()).unwrap();
    body.push_str(&format!("execute as @a run function {ns}:bonfire_open_0\n"));
    out.insert(tick, body.into_bytes());
    let (_, roots) = dialog_roots(&out);
    assert!(
        roots.contains(&Root::Other(format!(
            "execute as @a run function {ns}:bonfire_open_0"
        ))),
        "{roots:#?}"
    );
}
