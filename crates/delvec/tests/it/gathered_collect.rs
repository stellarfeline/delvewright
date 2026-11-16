//! A `collect` whose item the world supplies (`gathered`, DSL v0.80).
//!
//! A fished cod reaches the party's hands by a vanilla act — no kit, no chest,
//! no drop, no `give-item` — so before `gathered` an `interact` presenting it
//! was refused on every path (`DW0204`), and the only collect that supplied it
//! put a chest of cod at the anchor. Pinned here, on hello-world:
//!
//! * the red: the interact presenting cod with nothing supplying it is
//!   refused (`DW0849` at validation; `DW0204` on the path once a source
//!   exists off it);
//! * the green: a gathered collect of 5 cod in front of it is a supply, and
//!   the build is clean;
//! * what it emits: no chest and no fill at activation, the possession
//!   advancement with its count, the exported step marked `gathered`, and the
//!   generated `collect_gathered` PackTest;
//! * the perturbation: the same collect without `gathered` conjures its chest
//!   and fills it, and its step carries no `gathered`.

use crate::common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// hello-world with the keeper's talk followed by `middle` (zero or more
/// objectives), then an `interact` at the exit presenting cod.
fn fixture(tag: &str, middle: Vec<Value>) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("gathered-collect-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quests.json"), |q| {
        let objs = q["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        let exit = objs.pop().unwrap();
        assert_eq!(exit["id"], "obj/exit");
        let before = middle.last().map_or("obj/talk".to_string(), |o| {
            o["id"].as_str().unwrap().to_string()
        });
        objs.extend(middle);
        objs.push(json!({
            "type": "interact", "id": "obj/feed", "anchor": "anchor/exit",
            "prop": {"block": "minecraft:torch"},
            "requires_item": "minecraft:cod", "after": [before],
            "title": "Show the cat the cod", "hint": "The cat waits at the exit.",
            "answers": {"not_yet": "Not yet.", "missing_item": "The cat wants fish.",
                        "done": "Fed."},
            "happening": {"text": "the party feeds the cat", "verb": "loses"}
        }));
    });
    dir
}

fn fish(gathered: bool) -> Value {
    let mut o = json!({
        "type": "collect", "id": "obj/fish", "item": "minecraft:cod", "count": 5,
        "anchor": "anchor/keeper-stand", "after": ["obj/talk"],
        "title": "Catch five cod", "hint": "The pool by the keeper.",
        "happening": {"text": "the party catches five cod", "verb": "gains"}
    });
    if gathered {
        o["gathered"] = json!(true);
    }
    o
}

fn build(dir: &Path) -> (i32, String, PathBuf) {
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("delvec runs");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    )
}

/// Every emitted file under `out` whose path ends with `suffix`, as text.
fn emitted(out: &Path, suffix: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(suffix) {
                found.push((
                    p.to_string_lossy().to_string(),
                    std::fs::read_to_string(&p).unwrap_or_default(),
                ));
            }
        }
    }
    found.sort();
    found
}

fn fish_step(out: &Path) -> Value {
    let path: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("critical-path.json")).unwrap())
            .unwrap();
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/fish")
        .cloned()
        .expect("the critical path collects the fish")
}

/// The red: nothing in the campaign hands the party a cod, so the interact
/// presenting one is refused — at validation by `DW0849` (no source of the
/// item anywhere), the refusal a fished cod met before `gathered`.
#[test]
fn a_presented_cod_nothing_hands_over_is_refused() {
    let (code, log, _) = build(&fixture("red", vec![]));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW0849 [error]")
            && l.contains("`obj/feed`")
            && l.contains("`minecraft:cod`")),
        "{log}"
    );
}

#[test]
fn a_gathered_collect_supplies_the_presented_cod_and_places_nothing() {
    let (code, log, out) = build(&fixture("green", vec![fish(true)]));
    assert_eq!(code, 0, "{log}");
    assert!(!log.contains("DW0204") && !log.contains("DW0849"), "{log}");

    // Nothing placed: no activation function, and no chest or fill anywhere.
    assert!(
        emitted(&out, "/activate_o_fish.mcfunction").is_empty(),
        "a gathered collect has nothing to activate"
    );
    for (p, text) in emitted(&out, ".mcfunction") {
        if p.contains("packtest-datapack") {
            continue;
        }
        assert!(
            !text
                .lines()
                .any(|l| l.contains("item replace block") && l.contains("minecraft:cod")),
            "{p} fills a block with the gathered item"
        );
    }

    // Judged by possession: the completion advancement counts five cod.
    let adv = emitted(&out, "/advancement/c_fish.json");
    assert_eq!(adv.len(), 1, "one completion advancement");
    let v: Value = serde_json::from_str(&adv[0].1).unwrap();
    let crit = v["criteria"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .clone();
    assert_eq!(crit["trigger"], "minecraft:inventory_changed", "{crit}");
    assert_eq!(
        crit["conditions"]["items"][0]["items"], "minecraft:cod",
        "{crit}"
    );
    assert_eq!(crit["conditions"]["items"][0]["count"]["min"], 5, "{crit}");

    // The bot is told the world supplies it.
    let step = fish_step(&out);
    assert_eq!(step["gathered"], true, "{step}");

    // The PackTest: no chest at the anchor, then possession by `give` completes.
    let t = emitted(&out, "/test/collect_gathered.mcfunction");
    assert_eq!(t.len(), 1, "the gathered template is generated");
    let body = &t[0].1;
    assert!(body.contains("assert not block "), "{body}");
    assert!(body.contains(" minecraft:chest"), "{body}");
    assert!(
        body.contains("give @a[tag=dw_t_cgat,limit=1] minecraft:cod 5"),
        "{body}"
    );
    let asserts: Vec<&str> = body
        .lines()
        .filter(|l| l.starts_with("assert score #party"))
        .collect();
    assert_eq!(asserts.len(), 2, "open before the give, done after: {body}");
    assert!(asserts[0].ends_with("matches 0") && asserts[1].ends_with("matches 1"));
}

/// The perturbation: `gathered` removed, the same collect conjures its chest
/// and fills it, and its exported step says nothing of the world.
#[test]
fn without_gathered_the_collect_places_its_chest() {
    let (code, log, out) = build(&fixture("chest", vec![fish(false)]));
    assert_eq!(code, 0, "{log}");
    let act = emitted(&out, "/activate_o_fish.mcfunction");
    assert_eq!(act.len(), 1, "the chest collect activates");
    assert!(act[0].1.contains("minecraft:chest"), "{}", act[0].1);
    assert!(
        act[0].1.contains("item replace block") && act[0].1.contains("minecraft:cod 5"),
        "{}",
        act[0].1
    );
    assert!(fish_step(&out).get("gathered").is_none());
    assert!(emitted(&out, "/test/collect_gathered.mcfunction").is_empty());
}
