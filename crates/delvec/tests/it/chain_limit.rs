//! A world build over the game's command chain limit still finishes (`DW0984`'s
//! emission half).
//!
//! The pinned server stops one execution context at `max_command_sequence_length`
//! (65,536) commands and drops the rest with a log line nothing reads. A stage-7
//! stamp of tens of thousands of writes, emitted as one function called from the
//! function that latches setup complete, therefore never finishes, and the world
//! is never marked placed. The fixture here is such a stamp: a white-noise fill
//! under the keep whose coalesced writes number past the limit.
//!
//! The assertions read the emitted files the way the server runs them, and share
//! nothing with `compiler::chain`: no function holds more commands than the
//! limit, the steps chain from tick to tick through `schedule function` alone,
//! they carry block writes and nothing else, and only the function they end in
//! latches the world placed.
//!
//! The work every tick repeats cannot move to a later tick. A campaign whose
//! tick measures past the limit builds, and its `load` declares the quota its
//! sources need (`gamerule max_command_sequence_length`), which the server
//! reads when it makes each later context: the second fixture is v06-edits
//! with its one reach objective repeated until the compass's per-objective
//! aims carry the tick past the limit.

use crate::common;

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_dsl::DSL_VERSION;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");
/// The pinned server's default `max_command_sequence_length`.
const LIMIT: usize = 65_536;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The v06-edits fixture with one batch: a 48³ fill under the keep from eight
/// equally weighted blocks at a noise scale fine enough that neighbouring cells
/// rarely share a block, so the x-run coalescing leaves a write per cell or two.
fn over_limit_stamp() -> PathBuf {
    let dir = tmp("chain-limit-stamp");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("v06-edits"), &dir);
    let blocks = [
        "minecraft:stone",
        "minecraft:andesite",
        "minecraft:diorite",
        "minecraft:granite",
        "minecraft:cobblestone",
        "minecraft:tuff",
        "minecraft:deepslate",
        "minecraft:calcite",
    ];
    let doc = serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [{
            "id": "batch/deep-stamp",
            "area": "area/keep",
            "edits": [
                {
                    "verb": "select",
                    "name": "region/under",
                    "shape": {
                        "kind": "box",
                        "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                        "min": [-18, -48, -18],
                        "max": [29, -1, 29]
                    }
                },
                {
                    "verb": "fill",
                    "region": "region/under",
                    "recipe": {
                        "blocks": blocks.iter().map(|b| serde_json::json!({ "block": b, "weight": 1.0 })).collect::<Vec<_>>(),
                        "scale": 10.0
                    }
                }
            ]
        }]}
    });
    std::fs::write(
        dir.join("world-edits.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
    dir
}

/// The commands a function body runs: every line but blanks and comments.
fn commands(body: &str) -> Vec<&str> {
    body.lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .collect()
}

fn function(out: &Path, name: &str) -> Option<String> {
    std::fs::read_to_string(out.join(format!(
        "datapack/data/hello-world/function/{name}.mcfunction"
    )))
    .ok()
}

#[test]
fn a_world_build_over_the_chain_limit_runs_in_steps_and_finishes() {
    let src = over_limit_stamp();
    let out = tmp("chain-limit-stamp-out");
    let r = Command::new(BIN)
        .args([
            "build",
            src.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            &common::prefabs_dir().display().to_string(),
        ])
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec");
    assert!(
        r.status.success(),
        "build failed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );

    // No function the server can run holds more commands than one context
    // runs. The old emitter put the whole stamp in `world_edits` — 96,979
    // commands at this fixture — so this is the line that reds on it.
    let fdir = out.join("datapack/data/hello-world/function");
    let mut largest = (0usize, String::new());
    for e in std::fs::read_dir(&fdir).unwrap() {
        let p = e.unwrap().path();
        let n = commands(&std::fs::read_to_string(&p).unwrap()).len();
        if n > largest.0 {
            largest = (n, p.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        largest.0 <= LIMIT,
        "`{}` holds {} commands, past the {LIMIT} one tick runs",
        largest.1,
        largest.0
    );

    // The steps: `place_verify` enters the first, each schedules the next a
    // tick later, the last schedules `setup_finish`, and only `setup_finish`
    // latches `#placed` 1.
    let verify = function(&out, "place_verify").expect("place_verify");
    assert!(
        verify
            .lines()
            .last()
            .is_some_and(|l| l.ends_with("run function hello-world:world_build")),
        "{verify}"
    );
    let mut writes: Vec<String> = Vec::new();
    let mut name = "world_build".to_string();
    let mut steps = 0usize;
    loop {
        let body = function(&out, &name).unwrap_or_else(|| panic!("step `{name}` emitted"));
        let cmds = commands(&body);
        steps += 1;
        let (last, rest) = cmds.split_last().expect("a step has a body");
        let rest = if steps == 1 {
            assert_eq!(rest[0], "scoreboard players set #placed dw.sys 2", "{name}");
            &rest[1..]
        } else {
            rest
        };
        for l in rest {
            assert!(
                l.starts_with("fill ") || l.starts_with("setblock "),
                "a step carries block writes only, got `{l}` in `{name}`"
            );
            assert!(!l.contains("function "), "a step calls nothing: `{l}`");
        }
        writes.extend(rest.iter().map(|l| l.to_string()));
        let next = last
            .strip_prefix("schedule function hello-world:")
            .and_then(|t| t.strip_suffix(" 1t"))
            .unwrap_or_else(|| panic!("`{name}` ends by scheduling the next: `{last}`"));
        if next == "setup_finish" {
            break;
        }
        name = next.to_string();
    }
    assert!(steps > 1, "an over-limit build takes more than one step");
    assert!(
        writes.len() > LIMIT,
        "the fixture binds: {} writes, over the {LIMIT} limit",
        writes.len()
    );
    let finish = function(&out, "setup_finish").expect("setup_finish");
    assert!(
        !finish.contains("world_build") && !finish.contains("world_edits"),
        "setup_finish runs no block build in its own tick:\n{finish}"
    );
    assert_eq!(
        commands(&finish).last().copied(),
        Some("scoreboard players set #placed dw.sys 1")
    );
    let tick = function(&out, "tick").expect("tick");
    assert!(
        tick.contains("if score #placed dw.sys matches 0 run function hello-world:place_all"),
        "the tick stops re-placing templates once the build has begun:\n{tick}"
    );

    // The engine's own ledger agrees, and says how many steps it counted.
    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("validation/chain-length.json")).unwrap())
            .unwrap();
    assert_eq!(ledger["steps"], serde_json::json!(steps));
    assert_eq!(ledger["refused"], serde_json::json!([]));
    assert!(ledger["longest"]["chain"].as_u64().unwrap() <= LIMIT as u64);
}

/// The v06-edits fixture with its reach objective repeated `n` times: one
/// quest, every copy at the same door after the same talk, each one a compass
/// target the tick aims at.
fn over_limit_tick(n: usize) -> PathBuf {
    let dir = tmp("chain-limit-tick");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("v06-edits"), &dir);
    let path = dir.join("quests.json");
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let objectives = doc["content"]["quests"][0]["objectives"]
        .as_array_mut()
        .unwrap();
    let at = objectives
        .iter()
        .position(|o| o["type"] == "reach-anchor")
        .expect("a reach objective");
    let one = objectives.remove(at);
    for k in 0..n {
        let mut o = one.clone();
        o["id"] = serde_json::json!(format!("obj/exit-{k}"));
        o["title"] = serde_json::json!(format!("Go through the door, pass {k}"));
        o["happening"]["text"] = serde_json::json!(format!("the party completes obj/exit-{k}"));
        objectives.insert(at + k, o);
    }
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    dir
}

/// The `execute` sub-commands that each cost a stage before `run`, as the
/// pinned command tree names them (`run` aside).
const STAGES: [&str; 13] = [
    "align",
    "anchored",
    "as",
    "at",
    "facing",
    "if",
    "in",
    "on",
    "positioned",
    "rotated",
    "store",
    "summon",
    "unless",
];

/// The most players a delve is made for: every source a player selector forks
/// into is counted this many times.
const PARTY: u64 = 4;

/// How many sources the entity argument `sel` forks into at a full party.
fn fan(sel: &str) -> u64 {
    let limit = sel
        .split(['[', ',', ']'])
        .find_map(|t| t.strip_prefix("limit="))
        .map(|n| n.parse::<u64>().unwrap());
    let player = sel.contains("type=minecraft:player") || sel.contains("type=player,");
    match &sel[..2.min(sel.len())] {
        "@s" | "@n" => limit.unwrap_or(1),
        "@p" | "@r" => limit.unwrap_or(1).min(PARTY),
        "@a" => limit.map_or(PARTY, |l| l.min(PARTY)),
        "@e" if player => limit.map_or(PARTY, |l| l.min(PARTY)),
        "@e" => limit.unwrap_or_else(|| panic!("an unbounded fork in the fixture: {sel}")),
        _ => 1,
    }
}

/// One call's quota at a full party, read off the files the server loads: the
/// call, then per line its `execute` stages once, and its command or, for a
/// call, the callee's quota once per source the line's forks leave. No
/// same-tick cycle is expected in this fixture's pack.
fn quota(
    fns: &std::collections::BTreeMap<String, String>,
    f: &str,
    memo: &mut std::collections::BTreeMap<String, u64>,
) -> u64 {
    if let Some(q) = memo.get(f) {
        return *q;
    }
    let Some(body) = fns.get(f) else {
        return 1;
    };
    let mut q = 1u64;
    for l in commands(body) {
        let t: Vec<&str> = l.trim_start_matches('$').split_whitespace().collect();
        let run = t.iter().rposition(|w| *w == "run");
        let terminal = t.first() == Some(&"execute") && run.is_none();
        let head = run.map_or(if terminal { &t[..] } else { &t[..0] }, |r| &t[..r]);
        let sources: u64 = (0..head.len().saturating_sub(1))
            .filter(|&i| {
                matches!(head[i], "as" | "at") || (head[i] == "entity" && i > 0 && head[i - 1] == "facing")
            })
            .map(|i| fan(head[i + 1]))
            .product();
        assert!(
            !head.windows(2).any(|w| w == ["on", "passengers"]),
            "an unbounded fork in the fixture: {l}"
        );
        if terminal {
            // Its last stage is the command itself, once per source.
            q += (t.iter().filter(|w| STAGES.contains(w)).count() as u64).saturating_sub(1)
                + sources;
            continue;
        }
        q += head.iter().filter(|w| STAGES.contains(w)).count() as u64;
        let tail = run.map_or(&t[..], |r| &t[r + 1..]);
        if tail.first() == Some(&"function") && tail.get(1).is_some_and(|c| *c != f) {
            q += sources * quota(fns, tail[1], memo);
        } else {
            q += sources;
        }
    }
    memo.insert(f.to_string(), q);
    q
}

#[test]
fn a_tick_past_the_chain_limit_builds_and_its_load_declares_the_quota() {
    let src = over_limit_tick(80);
    let out = tmp("chain-limit-tick-out");
    let r = Command::new(BIN)
        .args([
            "build",
            src.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            &common::prefabs_dir().display().to_string(),
        ])
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec");
    assert!(
        r.status.success(),
        "build failed:\n{}\n{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );

    let ns = "hello-world";
    let fdir = out.join(format!("datapack/data/{ns}/function"));
    let mut fns = std::collections::BTreeMap::new();
    for e in walk(&fdir) {
        let rel = e
            .strip_prefix(&fdir)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(name) = rel.strip_suffix(".mcfunction") {
            fns.insert(format!("{ns}:{name}"), std::fs::read_to_string(&e).unwrap());
        }
    }
    let mut memo = std::collections::BTreeMap::new();
    let tick = quota(&fns, &format!("{ns}:tick"), &mut memo);
    assert!(
        tick > LIMIT as u64,
        "the fixture binds: the tick's quota {tick} is past the {LIMIT} limit"
    );

    // The declaration: the first line of `load`, and nowhere else.
    let load = function_in(&out, ns, "load").expect("load");
    let first = commands(&load)[0];
    let declared: u64 = first
        .strip_prefix("gamerule max_command_sequence_length ")
        .unwrap_or_else(|| panic!("`load` opens with the declaration, got `{first}`"))
        .parse()
        .unwrap();
    for (f, body) in &fns {
        if f != &format!("{ns}:load") {
            assert!(
                !body.contains("max_command_sequence_length"),
                "`{f}` declares a quota"
            );
        }
    }
    // Every source outside `load` within the declared quota, the longest at
    // it; `load` and `setup` (the context that runs the line) within the
    // default.
    let in_load = [format!("{ns}:load"), format!("{ns}:setup")];
    let mut longest = 0u64;
    for f in fns.keys() {
        let q = quota(&fns, f, &mut memo);
        if in_load.contains(f) {
            assert!(q <= LIMIT as u64, "`{f}` runs in `load` at {q}");
        } else {
            assert!(
                q <= declared,
                "`{f}` measures {q} past the declared {declared}"
            );
            longest = longest.max(q);
        }
    }
    assert_eq!(longest, declared, "the quota declared is the one measured");
    assert_eq!(tick, declared, "the tick is the longest source");

    let ledger: serde_json::Value =
        serde_json::from_slice(&std::fs::read(out.join("validation/chain-length.json")).unwrap())
            .unwrap();
    assert_eq!(ledger["declared_limit"], serde_json::json!(declared));
    assert_eq!(ledger["refused"], serde_json::json!([]));
}

fn function_in(out: &Path, ns: &str, name: &str) -> Option<String> {
    std::fs::read_to_string(out.join(format!("datapack/data/{ns}/function/{name}.mcfunction"))).ok()
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            v.extend(walk(&p));
        } else {
            v.push(p);
        }
    }
    v.sort();
    v
}
