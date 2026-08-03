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
//!
//! **spec-0018 extends this in two ways.** A division-of-labour template needs
//! more than one player, so it spawns its own extra members with `/dummy <name>
//! spawn` and addresses them by that name — `@a[name=…,limit=1]` is as exclusive
//! as a tag and is admitted alongside it. And progression state now lives on the
//! batch-global `#party` holder rather than on each test's dummy: safe inside a
//! template, which is one atomic mcfunction, but NOT across ticks — so any
//! template that `await`s must be the sole owner of every party score it touches
//! (`party_state_across_ticks_is_owned`).

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A per-call suffix for the materialized scratch campaigns below: three tests
/// now build the same fixture families, and cargo runs them in parallel — a
/// process-id-only directory name let two of them scribble over each other
/// mid-copy (a half-written `dialogue.json` is a `DW0100`).
fn scratch_dir(kind: &str) -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "dw-packtest-batch-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

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
    let dst = scratch_dir("actors");
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

/// hello-world reshaped into the island's sealed-handoff beat (round-6 QA): the
/// keeper deferred, a puppet whose arrival despawns itself and spawns the
/// keeper, a `close-gate` on the door, and a strike trigger on the keeper's
/// stand — so the `v06_arrive_handoff` and `v04_strike_talk` templates are
/// emitted and swept by the batch-model rules below.
fn build_handoff_hello_world() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("handoff");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    let np = dst.join("npcs.json");
    let n = std::fs::read_to_string(&np)
        .unwrap()
        .replacen("\"0.2.0\"", "\"0.6.0\"", 1)
        .replace(
            "\"base_entity\": \"minecraft:villager\",",
            "\"base_entity\": \"minecraft:villager\",\n        \"deferred\": true,",
        );
    assert!(n.contains("deferred"), "npcs.json patch applied");
    std::fs::write(&np, n).unwrap();
    let search = r#"            {
              "type": "open-gate",
              "anchor": "anchor/door"
            }"#;
    let replace = r#"            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-actor", "actor": "actor/giant" },
            { "type": "move-actor", "actor": "actor/giant", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" },
                             { "type": "spawn-npc", "npc": "npc/keeper" } ] }"#;
    let triggers = r#"    ],
    "triggers": [
      { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "strike" },
        "once": true,
        "effects": [ { "type": "narrate", "style": "chat", "text": "He stirs." } ] }
    ],
    "actors": [
      { "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper", "anchor": "anchor/keeper-stand", "facing": "east" }
    ]
  }
}"#;
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp)
        .unwrap()
        .replace("\"dsl_version\": \"0.2.0\"", "\"dsl_version\": \"0.6.0\"")
        .replace(search, replace)
        // Seal the door only at quest end — after the critical path has walked
        // through it (a close-gate across the forced path is DW0311).
        .replace(
            "\"on_complete\": [\n          {\n            \"type\": \"campaign-complete\"\n          }\n        ]",
            "\"on_complete\": [\n          { \"type\": \"close-gate\", \"anchor\": \"anchor/door\" },\n          { \"type\": \"campaign-complete\" }\n        ]",
        )
        .replace("    ]\n  }\n}", triggers);
    assert!(
        q.contains("spawn-npc") && q.contains("close-gate"),
        "quests.json patch applied"
    );
    std::fs::write(&qp, q).unwrap();
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// The generated PackTest **templates** of a build: `(file name, body)`.
///
/// Only `data/<ns>/test/` — the files PackTest discovers as tests. The suite
/// datapack also carries ordinary `data/<ns>/function/` mechanism functions
/// (e.g. the scheduled-executor probe), which are runtime code the templates
/// drive, not templates, and are deliberately party-wide.
fn templates(out: &BuildOutput) -> Vec<(String, String)> {
    out.iter()
        .filter(|(p, _)| {
            p.starts_with("packtest-datapack/")
                && p.contains("/test/")
                && p.ends_with(".mcfunction")
        })
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
        // spec-0016 §6: a lane's shared waypoint index is real runtime state the
        // TD templates drive (like the move drivers above); each initializes it
        // explicitly on entry.
        || h.starts_with("#lane_")
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

/// The fixture families every batch-model rule is swept over.
fn suites() -> Vec<(&'static str, BuildOutput)> {
    vec![
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
        // spec-0018: the AND-join family, whose templates spawn extra members.
        (
            "and-join",
            build_dir(&common::compiler_fixtures_dir().join("and-join")),
        ),
        // spec-0016 §6: the TD lane + aggro-edge templates.
        (
            "souls-td-lanes",
            build_dir(&common::compiler_fixtures_dir().join("souls-td-lanes")),
        ),
        // spec-0020: the cast-ledger templates (root swap, bark cycle, silent scene).
        (
            "cast-ledger",
            build_dir(&common::compiler_fixtures_dir().join("cast-ledger")),
        ),
        ("hello-world+actors", build_actor_hello_world()),
        ("hello-world+handoff", build_handoff_hello_world()),
        // task #125: the branch-aware campaign template (phased drive + await)
        // and the scheduled-ending await template must obey the batch model too.
        (
            "branch-two-endings",
            build_dir(&common::compiler_fixtures_dir().join("branch-two-endings")),
        ),
        (
            "hello-world+scheduled-ending",
            build_scheduled_hello_world(),
        ),
    ]
}

/// hello-world with its finale `campaign-complete` moved 240t into a closing
/// `sequence` (the-wake's shape, task #125), so the awaiting campaign template
/// is emitted and swept by the batch-model rules.
fn build_scheduled_hello_world() -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("sched-ending");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    let search = r#"        "on_complete": [
          {
            "type": "campaign-complete"
          }
        ]"#;
    let replace = r#"        "on_complete": [
          {
            "type": "sequence",
            "steps": [
              { "at_ticks": 240, "effects": [ { "type": "campaign-complete" } ] }
            ]
          }
        ]"#;
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp)
        .unwrap()
        .replace("\"dsl_version\": \"0.2.0\"", "\"dsl_version\": \"0.6.0\"")
        .replace(search, replace);
    assert!(q.contains("at_ticks"), "quests.json patch applied");
    std::fs::write(&qp, q).unwrap();
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

/// Own members (spec-0018): a template that spawns extra dummies names them
/// under a prefix no other template uses, and removes every one it spawned —
/// a leaked member is a foreign player in every sibling's `@a` for the rest of
/// the run.
#[test]
fn spawned_members_are_uniquely_named_and_removed() {
    for (suite, out) in suites() {
        let mut owner: BTreeMap<String, String> = BTreeMap::new();
        for (file, body) in templates(&out) {
            let spawned: Vec<String> = body
                .lines()
                .filter_map(|l| l.trim().strip_prefix("dummy "))
                .filter_map(|r| r.strip_suffix(" spawn"))
                .map(str::to_string)
                .collect();
            for name in &spawned {
                assert!(
                    name.len() <= 16,
                    "{suite}/{file}: `{name}` is not a legal player name (>16 chars)"
                );
                if let Some(prev) = owner.insert(name.clone(), file.clone()) {
                    panic!("{suite}: member `{name}` is spawned by both {prev} and {file}");
                }
                assert!(
                    body.contains(&format!("dummy {name} leave")),
                    "{suite}/{file}: spawned member `{name}` is never removed:\n{body}"
                );
            }
        }
    }
}

/// Own scores, extended to party state (spec-0018): the `#party` holder is
/// batch-global. Inside one template that is harmless — a template is a single
/// atomic mcfunction, so its baseline, drive and assert all land in one tick
/// with no sibling in between. A template that `await`s spans ticks, and must
/// therefore be the ONLY template touching each party score it uses.
#[test]
fn party_state_across_ticks_is_owned() {
    for (suite, out) in suites() {
        let tpls = templates(&out);
        // score -> templates touching it, and which of those span ticks.
        let mut users: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut waiters: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (file, body) in &tpls {
            let spans_ticks = body
                .lines()
                .any(|l| l.trim_start().starts_with("await ") || l.contains("schedule function"));
            for line in body.lines() {
                for w in line.split_whitespace().collect::<Vec<_>>().windows(2) {
                    if w[0] != "#party" {
                        continue;
                    }
                    let e = users.entry(w[1].to_string()).or_default();
                    if !e.contains(file) {
                        e.push(file.clone());
                    }
                    if spans_ticks {
                        let e = waiters.entry(w[1].to_string()).or_default();
                        if !e.contains(file) {
                            e.push(file.clone());
                        }
                    }
                }
            }
        }
        for (score, waiting) in &waiters {
            let all = &users[score];
            assert_eq!(
                all.len(),
                1,
                "{suite}: `#party {score}` is awaited across ticks by {waiting:?} but also \
                 touched by {all:?} — a sibling's baseline write in a later tick would \
                 make the await's verdict depend on batch order"
            );
        }
    }
}

#[test]
fn packtest_templates_are_interleaving_independent() {
    let suites: Vec<(&str, BuildOutput)> = suites();

    // The round-6 handoff/strike-talk family must really be in scope.
    for t in ["v06_arrive_handoff", "v04_strike_talk", "v04_strike_npc"] {
        assert!(
            suites
                .iter()
                .find(|(s, _)| *s == "hello-world+handoff")
                .map(|(_, out)| out)
                .expect("handoff suite present")
                .contains_key(&format!(
                    "packtest-datapack/data/hello-world/test/{t}.mcfunction"
                )),
            "handoff template {t} emitted"
        );
    }

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
                // Rule 2 — own dummy: every `@a` is scoped to a tag this test
                // set, or to the NAME of a member it spawned itself (spec-0018
                // division-of-labour templates). A bare `@a` write hits every
                // coexisting dummy in the batch.
                let mut rest = line;
                while let Some(i) = rest.find("@a") {
                    let after = &rest[i + 2..];
                    assert!(
                        after.starts_with("[tag=") || after.starts_with("[name="),
                        "{suite}/{file}: bare `@a` (must be `@a[tag=…,limit=1]` or \
                         `@a[name=…,limit=1]`):\n{line}"
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
