//! Suite-wide PackTest batch-model invariants (task: total conversion of the
//! generated templates; see `pin_dummy` in `emit.rs` and the "PackTest batch
//! model" section of `docs/reference/compiler.md`).
//!
//! PackTest runs the whole generated suite as ONE batch on one shared server:
//! every `# @dummy` test spawns its own dummy, all dummies coexist, and the
//! test functions execute over the same server ticks in an order the compiler
//! does not control. The hard rule is **every generated test is
//! interleaving-independent: own dummy, own scores, own init** — enforced here
//! mechanically over every fixture family so a new template cannot regress to
//! bare `@p`/`@a` player addressing (round-5 island reds: `v06_stealth`,
//! `verb_flag_gate`) or a cross-template scratch holder.

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// Build any valid campaign directory (loading `skins/` when the campaign
/// declares skinned NPCs), mirroring the other integration suites' builders.
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
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &skins,
    )
    .expect("emission succeeds")
}

/// hello-world patched to declare an actor and every actor verb (the same patch
/// `cli.rs::v06_actor_datapack_emits_the_mechanics` applies), so the four
/// `v06_spawn_*`/`v06_unleash`/`v06_move_actor` templates are covered too.
fn build_actor_hello_world() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = std::env::temp_dir().join(format!("dw-packtest-batch-actors-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    let search = r#"            {
              "type": "open-gate",
              "anchor": "anchor/door"
            }"#;
    let replace = r#"            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-actor", "actor": "actor/giant" },
            { "type": "move-actor", "actor": "actor/giant", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" } ] },
            { "type": "unleash-actor", "actor": "actor/giant" }"#;
    let actors = "    ],\n    \"actors\": [\n      { \"id\": \"actor/giant\", \"entity\": \"minecraft:zombie\", \"name\": \"The Sleeper\", \"anchor\": \"anchor/keeper-stand\", \"facing\": \"east\" }\n    ]\n  }\n}";
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp)
        .unwrap()
        .replace("\"dsl_version\": \"0.2.0\"", "\"dsl_version\": \"0.6.0\"")
        .replace(search, replace)
        .replace("    ]\n  }\n}", actors);
    assert!(q.contains("spawn-actor"), "quests.json patch applied");
    std::fs::write(&qp, q).unwrap();
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// The generated PackTest templates of a build: `(file name, body)`.
fn templates(out: &BuildOutput) -> Vec<(String, String)> {
    out.iter()
        .filter(|(p, _)| p.starts_with("packtest-datapack/") && p.ends_with(".mcfunction"))
        .map(|(p, b)| {
            (
                p.rsplit('/').next().unwrap().to_string(),
                String::from_utf8(b.clone()).unwrap(),
            )
        })
        .collect()
}

/// Real (runtime) `dw.sys` holders templates legitimately share: session and
/// placement markers, trigger latches, and the move drivers the tests
/// fast-forward. Everything else under `#… dw.sys` is per-test scratch and must
/// belong to exactly one template.
fn is_runtime_holder(h: &str) -> bool {
    h == "#stealth"
        || h == "#placed"
        || h.starts_with("#trig_")
        || h.starts_with("#mt_")
        || h.starts_with("#at_")
        || h.starts_with("#arun_")
}

/// Root cause of the round-6 island flake (`v06_spawn_idempotent` expected 1,
/// got 0 on byte-identical packs): `spawn_actor_<id>`'s idempotence guard is
/// `unless entity @e[tag=dw_actor_<id>]` — a tag the unleashed twin also
/// carries — and `v06_unleash` left its twin alive. Batch order decided whether
/// the idempotence test's spawns no-op'd against that pupless twin. The
/// contract now: every actor template clears the actor tag on entry (own init)
/// and leaves no actor entity behind (no poison for a sibling).
#[test]
fn actor_templates_clear_on_entry_and_leave_no_residue() {
    let out = build_actor_hello_world();
    for t in [
        "v06_spawn_despawn",
        "v06_spawn_idempotent",
        "v06_unleash",
        "v06_move_actor",
    ] {
        let body = String::from_utf8(
            out[&format!("packtest-datapack/data/hello-world/test/{t}.mcfunction")].clone(),
        )
        .unwrap();
        let clear = "kill @e[tag=dw_actor_giant]";
        let first_clear = body
            .find(clear)
            .unwrap_or_else(|| panic!("{t} clears the actor tag:\n{body}"));
        let first_spawn = body
            .find(":spawn_actor_giant")
            .unwrap_or_else(|| panic!("{t} drives the real spawn:\n{body}"));
        assert!(
            first_clear < first_spawn,
            "{t}: the entry clear precedes the first spawn:\n{body}"
        );
        // No residue: the last actor-entity-affecting command is a kill of the
        // full actor tag (covers puppet AND twin), so nothing survives the test.
        let last_kill = body.rfind(clear).unwrap();
        let last_spawn = body.rfind(":spawn_actor_giant").unwrap();
        assert!(
            last_kill > last_spawn,
            "{t}: a final actor-tag kill follows the last spawn (no residue):\n{body}"
        );
    }
    // The unleash test's residue was the poison: its final kill must come AFTER
    // the twin assert (the assertion itself is untouched).
    let unleash = String::from_utf8(
        out["packtest-datapack/data/hello-world/test/v06_unleash.mcfunction"].clone(),
    )
    .unwrap();
    let twin_assert = unleash
        .find("assert score #twin_unl dw.sys matches 1")
        .expect("twin assert survives");
    assert!(
        unleash.rfind("kill @e[tag=dw_actor_giant]").unwrap() > twin_assert,
        "unleash kills its twin after asserting it:\n{unleash}"
    );
}

#[test]
fn packtest_templates_are_interleaving_independent() {
    let suites: Vec<(&str, BuildOutput)> = vec![
        ("hello-world", build_dir(&common::hello_world_dir())),
        ("keep-trial", build_dir(&common::keep_trial_dir())),
        (
            "v04-showcase",
            build_dir(&common::compiler_fixtures_dir().join("v04-showcase")),
        ),
        (
            "v06-checkpoints",
            build_dir(&common::compiler_fixtures_dir().join("v06-checkpoints")),
        ),
        ("hello-world+actors", build_actor_hello_world()),
    ];

    // The actor family (the round-6 island flake) must really be in scope.
    for t in [
        "v06_spawn_despawn",
        "v06_spawn_idempotent",
        "v06_unleash",
        "v06_move_actor",
    ] {
        assert!(
            suites
                .iter()
                .find(|(s, _)| *s == "hello-world+actors")
                .map(|(_, out)| out)
                .expect("actor suite present")
                .contains_key(&format!(
                    "packtest-datapack/data/hello-world/test/{t}.mcfunction"
                )),
            "actor template {t} emitted"
        );
    }

    for (suite, out) in &suites {
        let tpls = templates(out);
        assert!(!tpls.is_empty(), "{suite}: packtest templates emitted");

        // Scratch-holder ownership: `#… dw.sys` → the set of templates using it.
        let mut holders: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for (file, body) in &tpls {
            let mut pin_lines = 0usize;
            for line in body.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.is_empty() {
                    continue; // comment / directive, not a command
                }
                // Rule 1 — own dummy: `@p` may appear ONLY as the pin
                // (`tag @p add …`, first post-setup line, while the test's own
                // dummy is still the nearest player), at most once per file.
                if line.contains("@p") {
                    assert!(
                        line.starts_with("tag @p add "),
                        "{suite}/{file}: `@p` outside the pin line — after a tp \
                         it resolves to a NEIGHBOR test's dummy:\n{line}"
                    );
                    pin_lines += 1;
                    assert!(
                        pin_lines <= 1,
                        "{suite}/{file}: more than one pin line:\n{body}"
                    );
                }
                // Rule 2 — own dummy: every `@a` is tag-scoped. A bare `@a`
                // write hits every coexisting dummy in the batch.
                let mut rest = line;
                while let Some(i) = rest.find("@a") {
                    let after = &rest[i + 2..];
                    assert!(
                        after.starts_with("[tag="),
                        "{suite}/{file}: bare `@a` (must be `@a[tag=…,limit=1]`):\n{line}"
                    );
                    rest = after;
                }
                // Rule 3 — own scores: collect `#holder dw.sys` pairs.
                let toks: Vec<&str> = line.split_whitespace().collect();
                for w in toks.windows(2) {
                    if w[0].starts_with('#') && w[1] == "dw.sys" && !is_runtime_holder(w[0]) {
                        let users = holders.entry(w[0].to_string()).or_default();
                        if !users.contains(file) {
                            users.push(file.clone());
                        }
                    }
                }
            }
        }

        for (holder, users) in &holders {
            assert!(
                users.len() == 1,
                "{suite}: scratch holder `{holder}` is shared by {users:?} — \
                 fake players on dw.sys are batch-global, each template must \
                 suffix its own"
            );
        }
    }
}
