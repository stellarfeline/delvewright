//! **One producer** (spec-0112 §4, criterion 2): every line that places a
//! block, a container item or a body outside the templates, or removes a body,
//! in the functions the placements ledger reads is one row of
//! `validation/placed.json`, and every row is one such line — read here off the
//! shipped `.mcfunction` files the way the server reads them, by a rule of its
//! own, not by the ledger's reader.
//!
//! The fixture is the gallery's primary, which carries every class the ledger
//! holds — relight fixtures, a trigger prop, NPCs, an `interact` prop, marked
//! `reach` objectives, a `collect` chest, `spawn-assembly` parts — with one
//! declared edit: the trapped chest's payload also sets a block, so the trap's
//! payload has a placing line to be held `Unforced`.
//!
//! Every other function that places is named here by its class — a region
//! write (a `RegionEvent`, laid by the configuration), or a body present only
//! once a player has played (a wave, an unleashed or re-stood actor, a
//! cutscene's stand-in, a volley's projectile, a dropped stake, a wave's
//! re-seat), or the sweep of bodies an unseen hand-off already removed. A placing
//! function in no class reds the audit by name.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The functions the ledger reads, by name prefix (`setup_finish` and `tick`
/// exactly).
const LEDGER: &[&str] = &[
    "activate_o_",
    "complete_o_",
    "complete_q_",
    "spawn_npc_",
    "despawn_npc_",
    "spawn_actor_",
    "despawn_actor_",
    "asm_summon_",
    "asm_despawn_",
    "trig_",
    "trap_",
    "loop_",
    "seal_arm_",
    "shortcut_open_",
    "holder_",
];

/// A placing function the ledger does not read, by class and prefix.
const REGION: &[&str] = &[
    "seal_",
    "collapse_",
    "tgate_",
    "shortcut_",
    "world_build",
    "world_ground",
];
const PLAYED: &[&str] = &[
    "spawn_",
    "wave_",
    "unleash_",
    "cs_",
    "actor_restand_",
    "volley_",
    "stk_",
    // A block highlight's display copy (spec-0104 §7): summoned by
    // `hl_hold_<a>` only while a player stands within its reach, removed by
    // `hl_off_<a>`; no frame's world holds it.
    "hl_hold_",
    "hl_off_",
];

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Whether a shipped line places or removes, by the server's own reading of
/// the command it runs: the command after an `execute … run` is a `summon`, a
/// `setblock`, an `item replace block`, a `kill` of a tagged set, or the unseen
/// hand-off of a tagged set.
fn placing(line: &str) -> Option<&str> {
    let cmd = match line.rfind(" run ") {
        Some(i) if line.starts_with("execute ") => &line[i + 5..],
        _ => line,
    };
    let places = cmd.starts_with("summon ")
        || cmd.starts_with("setblock ")
        || cmd.starts_with("item replace block ")
        || (cmd.starts_with("kill @e[tag=") && !cmd.contains(','));
    let unseen = line.starts_with("execute as @e[tag=")
        && cmd.starts_with("data merge entity @s {Tags:[\"dw_unseen\"");
    (places || unseen).then_some(if unseen { line } else { cmd })
}

fn class_of(name: &str) -> &'static str {
    // `tick` is read for its boxed presses' reconciliation pairs (spec-0117).
    if name == "setup_finish" || name == "tick" || LEDGER.iter().any(|p| name.starts_with(p)) {
        "ledger"
    } else if REGION.iter().any(|p| name.starts_with(p)) {
        "region"
    } else if PLAYED.iter().any(|p| name.starts_with(p)) {
        "played"
    } else if name == "unseen_sweep" {
        // Removes, a tick later, what an unseen hand-off row already took out
        // of every frame's world.
        "sweep"
    } else {
        "none"
    }
}

fn functions(out: &Path) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let root = out.join("datapack/data");
    let mut stack = vec![root];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "mcfunction")
                && p.components().any(|c| c.as_os_str() == "function")
            {
                let name = p.file_stem().unwrap().to_string_lossy().into_owned();
                found.insert(name, std::fs::read_to_string(&p).unwrap());
            }
        }
    }
    found
}

#[test]
fn every_placing_line_is_one_row_and_every_row_is_one_line() {
    let dir = tmp("placed-ledger");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    // The declared edit: the trapped chest's payload sets a block.
    common::patch_file(&campaign.join("quests.json"), |v| {
        let traps = v["content"]["traps"].as_array_mut().unwrap();
        let chest = traps
            .iter_mut()
            .find(|t| t["id"] == "trap/chest")
            .expect("the gallery's trapped chest");
        chest["payload"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "type": "set-block",
                "anchor": "anchor/strongbox",
                "block": "minecraft:gold_block"
            }));
    });
    let out = dir.join("build");
    let r = Command::new(BIN)
        .arg("build")
        .arg(&campaign)
        .arg("-o")
        .arg(&out)
        .arg("--prefabs")
        .arg(&prefabs)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{log}");
    assert!(log.contains("placements ledger: "), "{log}");

    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("validation/placed.json")).unwrap())
            .unwrap();
    let rows = ledger["rows"].as_array().unwrap();
    // (function, command) -> how many rows (one per firing).
    let mut by_line: BTreeMap<(String, String), usize> = BTreeMap::new();
    for r in rows {
        *by_line
            .entry((
                r["function"].as_str().unwrap().to_string(),
                r["command"].as_str().unwrap().to_string(),
            ))
            .or_default() += 1;
    }

    let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut unclassed = Vec::new();
    let mut lines_held: BTreeSet<(String, String)> = BTreeSet::new();
    for (name, body) in functions(&out) {
        let placing_lines: Vec<&str> = body.lines().filter_map(placing).collect();
        if placing_lines.is_empty() {
            continue;
        }
        let class = class_of(&name);
        let c = counts.entry(class).or_default();
        c.0 += 1;
        c.1 += placing_lines.len();
        match class {
            "ledger" => {
                for l in placing_lines {
                    let key = (name.clone(), l.to_string());
                    assert!(
                        by_line.contains_key(&key),
                        "`{l}` in `{name}` is a placing line with no ledger row"
                    );
                    lines_held.insert(key);
                }
            }
            "none" => unclassed.push(name),
            _ => {}
        }
    }
    assert!(
        unclassed.is_empty(),
        "placing functions in no class: {unclassed:?}"
    );
    for key in by_line.keys() {
        assert!(
            lines_held.contains(key),
            "the row `{}` of `{}` is no line of that function",
            key.1,
            key.0
        );
    }
    let (lf, ll) = counts.get("ledger").copied().unwrap_or_default();
    eprintln!(
        "ledger audit: {ll} placing line(s) in {lf} ledger function(s) held to {} distinct \
         row(s) ({} row(s) with every firing); {} line(s) in {} region-write function(s); {} \
         line(s) in {} function(s) of bodies present only once played",
        by_line.len(),
        rows.len(),
        counts.get("region").map_or(0, |c| c.1),
        counts.get("region").map_or(0, |c| c.0),
        counts.get("played").map_or(0, |c| c.1),
        counts.get("played").map_or(0, |c| c.0),
    );
    assert_eq!(ll, lines_held.len(), "one row per line");
    assert!(ll > 0 && lf > 0, "the audit read no ledger line");

    // Every class the fixture declares is a row.
    let has = |f: &dyn Fn(&str) -> bool| rows.iter().any(|r| f(r["command"].as_str().unwrap()));
    for (what, f) in [
        (
            "a relight fixture",
            &(|c: &str| c.contains("minecraft:lantern[")) as &dyn Fn(&str) -> bool,
        ),
        ("a trigger prop", &|c: &str| c.contains("minecraft:lever[")),
        ("an NPC", &|c: &str| {
            c.starts_with("summon minecraft:mannequin")
                || c.starts_with("summon minecraft:villager")
        }),
        ("an interact prop", &|c: &str| {
            c.contains("minecraft:stone_button")
        }),
        // An objective stands no marker (spec-0104 §6), so the summon class
        // a marked reach held is an item holder's body.
        ("an item holder's body", &|c: &str| {
            c.contains("\"dw_holder\"")
        }),
        ("a collect chest's item", &|c: &str| {
            c.starts_with("item replace block ")
        }),
        ("a spawn-assembly part", &|c: &str| {
            c.starts_with("summon minecraft:block_display")
        }),
    ] {
        assert!(has(f), "no ledger row of {what}");
    }
    let trap_rows: Vec<&serde_json::Value> = rows
        .iter()
        .filter(|r| r["function"].as_str().unwrap().starts_with("trap_"))
        .collect();
    assert!(
        trap_rows.iter().any(|r| r["command"]
            .as_str()
            .unwrap()
            .contains("minecraft:gold_block")),
        "the trapped chest's payload is a row: {trap_rows:?}"
    );
    assert!(
        trap_rows.iter().all(|r| r["fired"]["unforced"].is_string()),
        "a trap's payload is unforced: {trap_rows:?}"
    );
}
