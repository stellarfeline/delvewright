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

mod common;

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
