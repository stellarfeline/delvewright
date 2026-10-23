//! Suite-wide PackTest batch-model invariants (task: total conversion of the
//! generated templates; see `pin_dummy` in `emit/packtest.rs` and the "PackTest batch
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
//!
//! **That last rule has a blind spot, and it is why `DW0807` exists.** The check
//! reads each template's OWN text, so it sees a party score only where the
//! template writes the token itself. The campaign-playthrough template contains
//! no `#party` token at all — it calls `pt_camp_run_<n>`, which lives outside
//! `test/` and which drives the real campaign's completion functions — so the
//! one template that genuinely holds the whole party ledger across ticks
//! contributes nothing to the check, and the rule is green over it while being
//! violated by it. Read transitively the rule is also unsatisfiable: that
//! template touches EVERY party score, so "sole owner" could never hold while
//! any sibling used one.
//!
//! So the obligation is inverted rather than widened, and it moves to the party
//! that can discharge it: a template that DRIVES the outcome it asserts on must
//! WRITE every `#party` term the gates on that outcome's path read, whoever else
//! touches it. That is `delvec::compiler::batchstate` (`DW0807`), bound in
//! the emitter over the shipped bytes and asserted here by
//! `every_template_owns_the_gate_it_asserts_on`.
//!
//! **"Drives" is not "ticks", and reading it as `tick` left the hole open.** The
//! campaign-playthrough template calls `complete_<objective>` directly, is
//! decided by `check_q_<quest>`'s `unless score #party dw.q_<quest> matches 1`
//! one dispatch below that, and asserts a score written two dispatches below in
//! `campaign_complete` — so a check built from `tick`'s own lines and one level
//! of writes did not count it, let alone judge it, and it shipped an
//! intermittent whose message is the batch model's own signature (*Expected
//! #party dw.campaign to match 1, but got 0 on tick 0*). The repair is on both
//! sides: `emit::campaign_progression_baseline` re-baselines the whole party
//! ledger, and `batchstate` reads writes transitively, treats any gated dispatch
//! on the drive's path as a gate, counts `await` as an assertion, and inlines a
//! template's own hoisted helper functions before judging.

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

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::DSL_VERSION;
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
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
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
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!(DSL_VERSION);
        common::objective_effects(d, 0, "obj/talk").extend([
            serde_json::json!({ "type": "spawn-actor", "actor": "actor/giant" }),
            serde_json::json!({
                "type": "move-actor", "actor": "actor/giant", "to": { "anchor": "anchor/exit" },
                "on_arrive": [
                    { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" }
                ]
            }),
            serde_json::json!({ "type": "unleash-actor", "actor": "actor/giant" }),
        ]);
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/giant", "entity": "minecraft:zombie", "answers": { "struck": "It does not stir.", "touched": "Cold, and still." }, "name": "The Sleeper",
              "anchor": "spawn", "facing": "east" }
        ]);
    });
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
    common::patch_file(&dst.join("npcs.json"), |d| {
        d["dsl_version"] = serde_json::json!(DSL_VERSION);
        d["content"]["npcs"][0]["deferred"] = serde_json::json!(true);
    });
    common::patch_file(&dst.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!(DSL_VERSION);
        common::objective_effects(d, 0, "obj/talk").extend([
            serde_json::json!({ "type": "spawn-actor", "actor": "actor/giant" }),
            serde_json::json!({
                "type": "move-actor", "actor": "actor/giant", "to": { "anchor": "anchor/exit" },
                "on_arrive": [
                    { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" },
                    { "type": "spawn-npc", "npc": "npc/keeper" }
                ]
            }),
        ]);
        // Seal the door only at quest end — after the critical path has walked
        // through it (a close-gate across the forced path is DW0311).
        let on_complete = d["content"]["quests"][0]["on_complete"]
            .as_array_mut()
            .expect("the quest ends the campaign");
        on_complete.insert(
            0,
            serde_json::json!({ "type": "close-gate", "anchor": "anchor/door" }),
        );
        d["content"]["triggers"] = serde_json::json!([
            { "id": "trigger/wake", "at": "anchor/keeper-stand", "on": { "on": "strike" },
              "once": true, "answers": { "done": "He is already awake." },
              "effects": [ { "type": "narrate", "style": "chat", "text": "He stirs." } ] },
            common::door_answer_value("The door is shut behind you.")
        ]);
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/giant", "entity": "minecraft:zombie", "answers": { "struck": "It does not stir.", "touched": "Cold, and still." }, "name": "The Sleeper",
              "anchor": "spawn", "facing": "east" }
        ]);
    });
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
        // The census ANSWER holders. `wave_census_<wave>` — an emitted campaign
        // function, not a template — sets all three to 0 and then counts into
        // them, so they are pack-owned runtime state by the same reading as the
        // move drivers above, and no template can suffix a name the body owns.
        // Listed here only once there is a census template per wave; with one
        // template they were unshared by accident, which is not the same as being
        // scratch. The safety is stronger than `#lane_`'s, not weaker: the reset
        // is performed by the very call whose answer is read, inside one atomic
        // template, and every ASSERTION reads a per-wave copy (`#wcn_<n>_<wave>`)
        // that is ordinary per-test scratch and still judged as such.
        || h.starts_with("#wcen_")
        // The party-unique kit latches. `class_apply_<class>` — again an emitted
        // campaign function — is what sets `#kit_<class>_<k>`; a template only
        // ever RESETS it, which is the residue rule, and every template that
        // drives an apply must. Same reading as the census answers above, and the
        // same reason it surfaced now: with one template driving one class it was
        // unshared by accident.
        || h.starts_with("#kit_")
        // The live-cutscene count. The pack's cutscene functions own it; a
        // `watch_<body>` template saves it into its own `#wcl_<k>`, holds it at
        // 1 across one real `tick` and puts it back, all inside one atomic
        // function, and asserts only on its own suffixed holders. Same reading as
        // the census answers above, and it surfaced the same way: with one
        // watcher per suite it was unshared by accident, and a talkable NPC
        // watching by default made two.
        || h == "#cs_live"
}

/// Root cause of the round-6 island flake (`v06_spawn_idempotent` expected 1,
/// got 0 on byte-identical packs): `spawn_actor_<id>`'s idempotence guard is
/// `unless entity @e[tag=dw_actor_<id>]` — a tag the unleashed twin also
/// carries — and `v06_unleash` left its twin alive. Batch order decided whether
/// the idempotence test's spawns no-op'd against that pupless twin. The
/// contract now: every actor template clears the actor tag on entry (own init)
/// and leaves no actor entity behind (no poison for a sibling).
///
/// Swept over EVERY actor template the suite emits, not a fixed list of four
/// names: the spawn/idempotence/unleash templates are now written per declared
/// actor (`DW0811`'s `actor` claim), so a list of names would go quietly out of
/// date the moment a fixture declares a second actor — which is the same defect
/// one layer out.
#[test]
fn actor_templates_clear_on_entry_and_leave_no_residue() {
    let out = build_actor_hello_world();
    let names: Vec<String> = out
        .keys()
        .filter_map(|k| {
            k.strip_prefix("packtest-datapack/data/hello-world/test/")?
                .strip_suffix(".mcfunction")
                .map(str::to_string)
        })
        .filter(|t| {
            t.starts_with("v06_spawn_despawn")
                || t.starts_with("v06_spawn_idempotent")
                || t.starts_with("v06_unleash")
                || t.starts_with("v06_move_actor")
        })
        .collect();
    assert_eq!(
        names.len(),
        4,
        "the fixture's one actor gets one of each actor template: {names:?}"
    );
    for t in &names {
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
        out["packtest-datapack/data/hello-world/test/v06_unleash_giant.mcfunction"].clone(),
    )
    .unwrap();
    let twin_assert = unleash
        .find("assert score #twin_unl_giant dw.sys matches 1")
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
        // The branch-aware campaign template (phased drive + await)
        // and the scheduled-ending await template must obey the batch model too.
        (
            "branch-two-endings",
            build_dir(&common::compiler_fixtures_dir().join("branch-two-endings")),
        ),
        (
            "hello-world+scheduled-ending",
            build_scheduled_hello_world(),
        ),
        // The fixture-class family. Three templates in one campaign
        // touch the SAME stake's marker tag (`v10_stake_embers` and one
        // `fixture_<teleport>_<stake>` per ride), which is the first time a
        // generated suite has had more than one owner of a live entity tag — the
        // exact shape "own init" exists for, and the exact shape an intermittent
        // red is made of.
        (
            "lift-stake",
            build_dir(&common::compiler_fixtures_dir().join("lift-stake")),
        ),
    ]
}

/// hello-world with its finale `campaign-complete` moved 240t into a closing
/// `sequence` (the-wake's shape), so the awaiting campaign template
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
            "happening": {
              "text": "the delve is complete",
              "verb": "survives"
            },
            "type": "campaign-complete"
          }
        ]"#;
    let replace = r#"        "on_complete": [
          {
            "type": "sequence",
            "steps": [
              { "at_ticks": 240, "effects": [ { "type": "campaign-complete",
                  "happening": { "verb": "survives", "text": "the delve is complete" } } ] }
            ]
          }
        ]"#;
    let qp = dst.join("quests.json");
    let q = std::fs::read_to_string(&qp)
        .unwrap()
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

/// The `#party` scores a template reads AFTER a tick boundary it scheduled:
/// every `if score #party X` / `unless score #party X` in a function the
/// template, or a helper it calls, reaches through `schedule function`, with
/// helpers followed transitively inside the packtest datapack. Returned as
/// `(score, template)` pairs, in template order.
fn scores_read_after_a_schedule(out: &BuildOutput) -> Vec<(String, String)> {
    let helper = |ns_fn: &str| -> Option<String> {
        let (ns, f) = ns_fn.split_once(':')?;
        out.get(&format!(
            "packtest-datapack/data/{ns}/function/{f}.mcfunction"
        ))
        .map(|b| String::from_utf8(b.clone()).unwrap())
    };
    let callees = |body: &str, kind: &str| -> Vec<String> {
        body.lines()
            .flat_map(|l| {
                let w: Vec<&str> = l.split_whitespace().collect();
                w.windows(2)
                    .filter(|p| p[0] == kind)
                    .map(|p| p[1].to_string())
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mut found = Vec::new();
    for (file, body) in templates(out) {
        // Every packtest helper the template reaches, and which of them it
        // reaches through a `schedule` (a later tick).
        let mut seen: Vec<String> = Vec::new();
        let mut later: Vec<String> = Vec::new();
        let mut stack: Vec<(String, bool)> = Vec::new();
        for c in callees(&body, "function") {
            stack.push((c, false));
        }
        // `schedule function ns:f 20t`: the name follows `function`.
        for l in body.lines().filter(|l| l.contains("schedule function ")) {
            if let Some(n) = l
                .split("schedule function ")
                .nth(1)
                .and_then(|r| r.split_whitespace().next())
            {
                stack.push((n.to_string(), true));
            }
        }
        while let Some((f, is_later)) = stack.pop() {
            let key = format!("{f}:{is_later}");
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let Some(b) = helper(&f) else { continue };
            if is_later && !later.contains(&f) {
                later.push(f.clone());
            }
            for c in callees(&b, "function") {
                stack.push((c, is_later));
            }
            for l in b.lines().filter(|l| l.contains("schedule function ")) {
                if let Some(n) = l
                    .split("schedule function ")
                    .nth(1)
                    .and_then(|r| r.split_whitespace().next())
                {
                    stack.push((n.to_string(), true));
                }
            }
        }
        for f in &later {
            let b = helper(f).unwrap();
            for l in b.lines() {
                let w: Vec<&str> = l.split_whitespace().collect();
                for t in w.windows(4) {
                    if (t[0] == "if" || t[0] == "unless") && t[1] == "score" && t[2] == "#party" {
                        let pair = (t[3].to_string(), file.clone());
                        if !found.contains(&pair) {
                            found.push(pair);
                        }
                    }
                }
            }
        }
    }
    found
}

/// Every sibling that touches a `#party` score another template reads after a
/// tick boundary it scheduled: `(score, reader, sibling)`.
fn siblings_touching_a_later_read(out: &BuildOutput) -> Vec<(String, String, String)> {
    let tpls = templates(out);
    let mut bad = Vec::new();
    for (score, reader) in scores_read_after_a_schedule(out) {
        for (file, body) in &tpls {
            if *file == reader {
                continue;
            }
            let touches = body.lines().any(|l| {
                l.split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|w| w[0] == "#party" && w[1] == score)
            });
            if touches {
                bad.push((score.clone(), reader.clone(), file.clone()));
            }
        }
    }
    bad
}

/// The branch-shape campaign template drives each phase inside a hoisted
/// helper and reads the completion score in a helper it schedules 20 ticks
/// later (`pt_camp_check_<i>`). `party_state_across_ticks_is_owned` reads only
/// a template's own text, so that read is invisible to it, and a sibling that
/// zeroes the completion score in the gap — any template that opens with
/// `campaign_progression_baseline` inline — turns one phase red by batch
/// order. A score read after a scheduled tick boundary is touched by no other
/// template.
#[test]
fn a_party_score_read_after_a_schedule_is_no_siblings() {
    let mut bound = 0usize;
    for (suite, out) in suites() {
        bound += scores_read_after_a_schedule(&out).len();
        let bad = siblings_touching_a_later_read(&out);
        assert!(
            bad.is_empty(),
            "{suite}: `#party` score(s) read after a scheduled tick boundary are also \
             touched by a sibling template, whose write between the two ticks decides \
             the reader's verdict by batch order: {bad:?}"
        );
    }
    assert!(
        bound > 0,
        "no suite reads a `#party` score after a scheduled tick boundary: the rule bound zero"
    );
}

/// The rule above reds on the shape that shipped: a sibling that zeroes the
/// completion score beside a branch-shape campaign template.
#[test]
fn a_sibling_zeroing_the_completion_score_is_caught() {
    let mut out = build_dir(&common::compiler_fixtures_dir().join("branch-two-endings"));
    let read = scores_read_after_a_schedule(&out);
    let (score, reader) = read
        .first()
        .cloned()
        .expect("branch shape reads after a schedule");
    assert_eq!(reader, "campaign.mcfunction", "{read:?}");
    assert!(siblings_touching_a_later_read(&out).is_empty());
    let ns_dir = out
        .keys()
        .find(|k| {
            k.starts_with("packtest-datapack/data/") && k.ends_with("/test/campaign.mcfunction")
        })
        .unwrap()
        .trim_end_matches("campaign.mcfunction")
        .to_string();
    out.insert(
        format!("{ns_dir}zeroes_the_ledger.mcfunction"),
        format!("# @dummy\nscoreboard players set #party {score} 0\n").into_bytes(),
    );
    let bad = siblings_touching_a_later_read(&out);
    assert_eq!(
        bad,
        vec![(score, reader, "zeroes_the_ledger.mcfunction".to_string())]
    );
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

    // The actor family (the round-6 island flake) must really be in scope. Named
    // by PREFIX: the spawn/idempotence/unleash templates are per declared actor
    // (`DW0811`'s `actor` claim), so the suffix is the actor's own id.
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
                .keys()
                .any(|k| k.starts_with(&format!("packtest-datapack/data/hello-world/test/{t}"))),
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

/// **Every generated template owns the gate its own assertion depends on**
/// (`DW0807`, `delvec::compiler::batchstate`).
///
/// The batch model's third leg, and the one that was missing. The invariants
/// above pin a template's dummy and its scratch holders; neither can see a term
/// a template merely READS. `#party` is batch-global progression state and the
/// campaign-playthrough template holds the whole ledger across ticks, so a
/// template that runs the real `tick` and asserts on a gated outcome without
/// writing that gate's terms has a verdict decided by batch order rather than by
/// the datapack.
///
/// Run over every fixture suite, and the binding is asserted rather than
/// assumed: a run that judged nothing would be vacuous, not a pass.
#[test]
fn every_template_owns_the_gate_it_asserts_on() {
    let mut judged_total = 0usize;
    for (suite, out) in suites() {
        let ns = out
            .keys()
            .find_map(|p| {
                p.strip_prefix("packtest-datapack/data/")
                    .and_then(|r| r.split('/').next())
            })
            .unwrap_or_else(|| panic!("{suite}: no generated PackTest tree"))
            .to_string();
        match delvec::compiler::batchstate::check_tree(&ns, &out) {
            Ok(b) => {
                assert!(
                    b.templates > 0,
                    "{suite}: the batch-state check examined zero templates"
                );
                // The denominator, printed rather than assumed: a widening of
                // this check is only reviewable against the population it
                // examined before and after.
                eprintln!(
                    "batch-state binding [{suite}]: templates={} driving={} judged={}",
                    b.templates, b.driving, b.judged
                );
                judged_total += b.judged;
            }
            Err(e) => panic!("{suite}: {} — {}", e.code.id(), e.message),
        }
    }
    assert!(
        judged_total > 0,
        "the batch-state ownership invariant judged zero templates across every fixture \
         suite — a green that compared nothing is vacuous, not a pass"
    );
}

/// What a template's lines leave `#placed` at when a tick can run the load:
/// the names of the templates (with the 1-based line, or `end`) that run `tick`
/// or `await` with the load re-armed, or end re-armed; how many templates run
/// `tick` after `setup` at all; and how many run `setup` at all.
///
/// `setup` re-arms the load (`#placed` 0, or 3 with ground), so the next tick
/// resumes the placement loop and, for a world with no build step, runs
/// `setup_finish` inline. The batch server's world is already built: a template
/// that ticks re-armed re-runs the whole load (every unguarded NPC summon,
/// every holder and container refill) on the shared server, inside its own
/// tick. An `await` hands the server real ticks, and so does a template's end:
/// the next real tick re-runs the load in front of whichever template runs
/// next. `await score #placed dw.sys matches 1` is the one wait that settles
/// the load: it lets the real chain run and resumes only once it has. The
/// template's own helper functions (`data/<ns>/function/` of the suite
/// datapack) are inlined, because a line does not stop belonging to the
/// template when the emitter hoists it.
fn ticks_with_the_load_re_armed(out: &BuildOutput) -> (Vec<String>, usize, usize) {
    let ns = out
        .keys()
        .find_map(|p| {
            p.strip_prefix("packtest-datapack/data/")
                .and_then(|r| r.split('/').next())
        })
        .expect("a generated PackTest tree")
        .to_string();
    let helper = |name: &str| -> Option<String> {
        out.get(&format!(
            "packtest-datapack/data/{ns}/function/{name}.mcfunction"
        ))
        .map(|b| String::from_utf8(b.clone()).unwrap())
    };
    let called = |line: &str, f: &str| {
        let target = format!("function {ns}:{f}");
        line == target || line.ends_with(&format!(" run {target}"))
    };
    fn walk(
        body: &str,
        placed: &mut u32,
        ticks_after_setup: &mut bool,
        ran_setup: &mut bool,
        bad: &mut Option<String>,
        depth: usize,
        ctx: &dyn Fn(&str) -> Step,
    ) {
        for (i, raw) in body.lines().enumerate() {
            let line = raw.trim();
            match ctx(line) {
                Step::Setup => {
                    *placed = 0;
                    *ran_setup = true;
                }
                Step::Latch(v) => *placed = v,
                Step::AwaitSettled => *placed = 1,
                step @ (Step::Tick | Step::Await) => {
                    if matches!(step, Step::Tick) && *ran_setup {
                        *ticks_after_setup = true;
                    }
                    if *placed != 1 && bad.is_none() {
                        *bad = Some(format!("line {}", i + 1));
                    }
                }
                Step::Helper(b) if depth < 8 => walk(
                    &b,
                    placed,
                    ticks_after_setup,
                    ran_setup,
                    bad,
                    depth + 1,
                    ctx,
                ),
                _ => {}
            }
        }
    }
    let ctx = |line: &str| -> Step {
        if called(line, "setup") {
            return Step::Setup;
        }
        if called(line, "tick") {
            return Step::Tick;
        }
        if line == "await score #placed dw.sys matches 1" {
            return Step::AwaitSettled;
        }
        if line.starts_with("await ") {
            return Step::Await;
        }
        if let Some(v) = line
            .strip_prefix("scoreboard players set #placed dw.sys ")
            .and_then(|v| v.parse().ok())
        {
            return Step::Latch(v);
        }
        if let Some(name) = line
            .rsplit_once(&format!("function {ns}:"))
            .map(|(_, n)| n.split_whitespace().next().unwrap_or(""))
            && let Some(b) = helper(name)
        {
            return Step::Helper(b);
        }
        Step::Other
    };
    let mut bad = Vec::new();
    let (mut bound, mut setups) = (0, 0);
    for (name, body) in templates(out) {
        // The batch server holds a built world: every template starts settled.
        let (mut placed, mut after, mut ran, mut hit) = (1, false, false, None);
        walk(&body, &mut placed, &mut after, &mut ran, &mut hit, 0, &ctx);
        bound += usize::from(after);
        setups += usize::from(ran);
        if hit.is_none() && placed != 1 {
            hit = Some(format!("end (`#placed` left at {placed})"));
        }
        if let Some(at) = hit {
            bad.push(format!("{name} {at}"));
        }
    }
    (bad, bound, setups)
}

enum Step {
    Setup,
    Tick,
    Await,
    AwaitSettled,
    Latch(u32),
    Helper(String),
    Other,
}

/// **No template runs `tick`, awaits, or ends with the load re-armed.** A
/// template that re-runs `setup` takes the settled setup (`settled_setup` in
/// `emit/packtest.rs`), so its tick judges the effect it drove, not a reload of
/// the world, and neither it nor the server's next real tick puts a second copy
/// of any body on the shared server.
///
/// Swept over every fixture suite, with both bindings — templates that tick
/// after a `setup`, and templates that run `setup` at all — counted and
/// required non-zero.
#[test]
fn no_template_ticks_with_the_load_re_armed() {
    let (mut bound_total, mut setups_total) = (0usize, 0usize);
    let mut bad_all = Vec::new();
    for (suite, out) in suites() {
        let (bad, bound, setups) = ticks_with_the_load_re_armed(&out);
        eprintln!(
            "settled-load binding [{suite}]: {setups} of {} template(s) run setup, \
             {bound} tick after it, {} re-armed",
            templates(&out).len(),
            bad.len()
        );
        bound_total += bound;
        setups_total += setups;
        bad_all.extend(bad.into_iter().map(|b| format!("{suite}: {b}")));
    }
    assert!(
        bound_total > 0,
        "no template in any fixture suite ticks after setup — the rule bound nothing"
    );
    assert!(
        setups_total > 0,
        "no template in any fixture suite runs setup — the end rule bound nothing"
    );
    assert!(
        bad_all.is_empty(),
        "{} template(s) run `tick` or `await`, or end, with the load re-armed by \
         `setup`; give each the settled setup (`settled_setup`):\n{}",
        bad_all.len(),
        bad_all.join("\n")
    );
}
