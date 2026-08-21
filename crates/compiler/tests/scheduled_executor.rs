//! The scheduled-executor invariant (AUDIT-P0), under party progression
//! (spec-0018).
//!
//! Vanilla's `schedule function …` re-invokes a function with the **server**
//! command source: no executor, so `@s` resolves to nothing and every
//! `@s`-addressed command in it silently fails. The compiler reaches three
//! bundles that way — `mv_arrive_<key>` / `ma_arrive_<key>` (fired from the
//! scheduled walk drivers) and every `seq_<key>_<i>` step (fired from the
//! scheduled timeline) — and used to emit them verbatim, so a scheduled
//! `set-flag`/`narrate`/`give-item`/`title`/`playsound` was dead on a live
//! server (the island's "Get Into the Shadows" soft-lock: two `on_arrive`
//! bundles set the flags `obj/take-cover` gates on).
//!
//! **What spec-0018 changed.** Progression state moved from the acting player to
//! the `#party` holder, so the classification these bundles are emitted under
//! changed with it: a `set-flag` is no longer "per-player" but a party-fact write
//! that names no selector at all, and is therefore immune to the seam *by
//! construction* — the island soft-lock class cannot recur for flags. What stays
//! executor-shaped is the genuinely player-facing set (`narrate`, `give-item`,
//! `play-sound`, `damage-players`), which now addresses `@a`: the party sees the
//! beat, and `@a` needs no executor either.
//!
//! The invariant below is the strongest available form of the lesson, and after
//! spec-0018 it is nearly vacuous *for the right reason*: walk the emitted call
//! graph from every `schedule` site and assert that no function reachable **only**
//! that way ever names `@s` outside an `as` binding. It is content-agnostic — a
//! future verb that forgets the seam fails here, in a unit test, not on the
//! owner's server.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

// ---------------------------------------------------------------------------
// command-line analysis
// ---------------------------------------------------------------------------

/// What one emitted command line tells us about the executor.
#[derive(Default)]
struct Scan {
    /// The line names `@s` while the executor is still whatever the caller
    /// gave us — i.e. `@s` is NOT re-bound by an `as` clause first.
    unbound_s: bool,
    /// Functions this line calls **inheriting** the caller's executor.
    inherited_calls: Vec<String>,
    /// Functions this line hands to the scheduler (always a server source).
    scheduled: Vec<String>,
}

/// Scan one command line.
///
/// Linear left-to-right over whitespace tokens, tracking whether `@s` has been
/// re-bound. `as` binds — except in `positioned as <targets>` / `rotated as
/// <targets>`, the only two `<subcommand> as` forms in the vanilla `execute`
/// grammar, which change position/rotation and leave the executor alone. `@s`
/// is only recognised as a whole token (`@s` or `@s[…]`), so an `@s` inside a
/// JSON text component is not mistaken for a selector.
fn scan_line(line: &str) -> Scan {
    let mut out = Scan::default();
    let mut bound = false;
    let toks: Vec<&str> = line.split_whitespace().collect();
    for (i, tok) in toks.iter().enumerate() {
        let prev = if i == 0 { "" } else { toks[i - 1] };
        if *tok == "as" && prev != "positioned" && prev != "rotated" {
            bound = true;
            continue;
        }
        if *tok == "function" {
            if let Some(id) = toks.get(i + 1) {
                if prev == "schedule" {
                    out.scheduled.push(id.to_string());
                } else if !bound {
                    out.inherited_calls.push(id.to_string());
                }
            }
            continue;
        }
        if !bound && (*tok == "@s" || tok.starts_with("@s[")) {
            out.unbound_s = true;
        }
    }
    out
}

/// The delve datapack's functions, keyed by their `<ns>:<name>` id.
fn datapack_functions(out: &BuildOutput, ns: &str) -> BTreeMap<String, String> {
    let prefix = format!("datapack/data/{ns}/function/");
    out.iter()
        .filter(|(p, _)| p.starts_with(&prefix) && p.ends_with(".mcfunction"))
        .map(|(p, b)| {
            let name = p[prefix.len()..p.len() - ".mcfunction".len()].to_string();
            (
                format!("{ns}:{name}"),
                String::from_utf8(b.clone()).unwrap(),
            )
        })
        .collect()
}

/// Every function reachable from a `schedule` site with the server command
/// source still in force: the scheduled functions themselves, plus everything
/// they call without re-binding the executor.
fn server_source_closure(fns: &BTreeMap<String, String>) -> BTreeSet<String> {
    let mut queue: Vec<String> = Vec::new();
    for body in fns.values() {
        for line in body.lines() {
            queue.extend(scan_line(line).scheduled);
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(body) = fns.get(&id) {
            for line in body.lines() {
                queue.extend(scan_line(line).inherited_calls);
            }
        }
    }
    seen
}

/// The datapack namespace of a build output (`datapack/data/<ns>/function/…`).
fn namespace_of(out: &BuildOutput) -> String {
    out.keys()
        .find_map(|p| {
            let rest = p.strip_prefix("datapack/data/")?;
            let ns = rest.split('/').next()?;
            rest.contains("/function/").then(|| ns.to_string())
        })
        .expect("build output has a datapack function directory")
}

/// The invariant: nothing in the server-source closure may address `@s`.
/// Returns the size of that closure (a v0.2 campaign schedules nothing, so an
/// empty closure is legitimate — the caller checks that *some* fixture covers
/// the scheduled paths).
fn assert_no_unbound_s(suite: &str, out: &BuildOutput) -> usize {
    let ns = namespace_of(out);
    let fns = datapack_functions(out, &ns);
    let closure = server_source_closure(&fns);
    let mut bad: Vec<String> = Vec::new();
    for id in &closure {
        let Some(body) = fns.get(id) else { continue };
        for line in body.lines() {
            if scan_line(line).unbound_s {
                bad.push(format!("  {id}: {line}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{suite}: `schedule` runs a function with the SERVER command source — these \
         schedule-reachable commands address an `@s` that does not exist, so they \
         silently do nothing on a live server:\n{}",
        bad.join("\n")
    );
    closure.len()
}

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    build_campaign(&campaign, &loaded.inputs)
}

fn build_campaign(campaign: &Campaign, inputs: &BTreeMap<String, Vec<u8>>) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world with a v0.6 `move-npc` whose `on_arrive` carries **both**
/// executor classes and a nested `sequence`: the exact shape the island uses
/// for its staged beats, and the shape the bug killed.
///
/// * per-player: `set-flag`, `narrate`, `give-item`, `play-sound` (one of them
///   `forbids_flags`-gated, so the per-player gate spelling is exercised);
/// * global: `open-gate`, `set-time` (`requires_flags`-gated → the party
///   predicate spelling), `set-weather`;
/// * nested `sequence` with an inline (`at_ticks: 0`) and a scheduled
///   (`at_ticks: 20`) step, so the timeline is reached through a scheduled
///   bundle — the nested-recursion case.
const SCHEDULED_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [
                { "type": "set-flag", "flag": "flag/arrived" },
                { "type": "narrate", "text": "The keeper takes his post." },
                { "type": "give-item", "item": "minecraft:torch", "count": 1 },
                { "type": "open-gate", "anchor": "anchor/door" },
                { "type": "set-time", "time": "day", "requires_flags": ["flag/arrived"] },
                { "type": "play-sound", "sound": "minecraft:block.note_block.pling",
                  "forbids_flags": ["flag/late"] },
                { "type": "sequence", "steps": [
                    { "at_ticks": 0, "effects": [
                      { "type": "narrate", "style": "title", "text": "At last." } ] },
                    { "at_ticks": 20, "effects": [
                      { "type": "set-flag", "flag": "flag/late" },
                      { "type": "set-weather", "weather": "clear" } ] }
                  ] }
              ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

fn build_scheduled_hello_world() -> BuildOutput {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: SCHEDULED_QUESTS.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    build_campaign(&campaign, &BTreeMap::new())
}

fn file(out: &BuildOutput, path: &str) -> String {
    out.iter()
        .find(|(p, _)| p.as_str() == path)
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("expected build output file `{path}`"))
}

const FN_DIR: &str = "datapack/data/hello-world/function";

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// The invariant, over every fixture family. This is the check that fails on
/// pre-fix output.
#[test]
fn no_schedule_reachable_function_addresses_an_unbound_s() {
    // The scheduled fixture must really exercise the paths: walk drivers, the
    // arrive bundle, the timeline start and both of its steps.
    let covered = assert_no_unbound_s("hello-world+scheduled", &build_scheduled_hello_world());
    assert!(
        covered >= 5,
        "the scheduled fixture must cover the driver + arrive + timeline chain, \
         got {covered} schedule-reachable functions"
    );
    for (suite, dir) in [
        ("keep-trial", common::keep_trial_dir()),
        ("keep-crawl", common::keep_crawl_dir()),
        ("keep-vertical", common::keep_vertical_dir()),
        (
            "v04-showcase",
            common::compiler_fixtures_dir().join("v04-showcase"),
        ),
        (
            "v06-checkpoints",
            common::compiler_fixtures_dir().join("v06-checkpoints"),
        ),
    ] {
        assert_no_unbound_s(suite, &build_dir(&dir));
    }
}

/// The multiplicity contract, read off the emitted arrive bundle under party
/// progression (spec-0018): **party-fact** effects name no selector and fire
/// exactly once; **player-facing** effects address `@a`, so the whole party sees
/// the beat. Neither form needs an executor — which is precisely why a scheduled
/// bundle can now carry a `set-flag` at all.
#[test]
fn arrive_bundle_splits_party_fact_and_player_facing_effects() {
    let out = build_scheduled_hello_world();
    let arrive = file(&out, &format!("{FN_DIR}/mv_arrive_keeper_exit.mcfunction"));
    let lines: Vec<&str> = arrive.lines().filter(|l| !l.is_empty()).collect();

    // Party fact: one write on the holder, no `as @a` wrapper (which would have
    // re-run it once per player for no effect).
    assert!(
        lines.contains(&"scoreboard players set #party dw.f_arrived 1"),
        "a scheduled `set-flag` writes the party holder, bare:\n{arrive}"
    );
    // Player-facing: addressed to the party, once.
    for needle in ["tellraw @a ", "give @a minecraft:torch 1"] {
        assert!(
            lines.iter().any(|l| l.starts_with(needle)),
            "player-facing effect must address the party (`{needle}`):\n{arrive}"
        );
    }
    // A player-facing effect's flag gate is a party read placed OUTSIDE the
    // audience — one question, asked once, for the whole party.
    assert!(
        lines.contains(
            &"execute unless score #party dw.f_late matches 1 run playsound \
              minecraft:block.note_block.pling master @a"
        ),
        "a gated player-facing effect reads `unless score #party …`:\n{arrive}"
    );
    // global: emitted bare — exactly once.
    assert!(
        lines.iter().any(|l| l.starts_with("fill ")),
        "the global `open-gate` fill must be emitted bare (once):\n{arrive}"
    );
    assert!(
        lines.contains(&"execute if score #party dw.f_arrived matches 1 run time set day"),
        "a gated global effect reads the same party score:\n{arrive}"
    );
    // Nothing in a scheduled bundle may name `@s` — the executor is not there.
    assert!(
        !arrive.contains("@s"),
        "a scheduled bundle addresses no `@s`:\n{arrive}"
    );
    // The nested sequence is a global timeline: one call, not one per player.
    let seq_calls: Vec<&&str> = lines
        .iter()
        .filter(|l| l.contains("function hello-world:seq_"))
        .collect();
    assert_eq!(
        seq_calls.len(),
        1,
        "the nested sequence is started exactly once:\n{arrive}"
    );
    assert!(
        seq_calls[0].starts_with("function hello-world:seq_"),
        "the sequence start is not wrapped in `as @a` (its steps re-bind the \
         party themselves; wrapping would multi-fire its globals and schedules):\n{arrive}"
    );
}

/// Every `sequence` step function is server-source-safe, the inline
/// (`at_ticks: 0`) one included: a timeline whose first beat behaved
/// differently from its second would be a trap, and the start function is
/// itself reachable from a scheduled bundle.
#[test]
fn sequence_steps_are_server_source_safe() {
    let out = build_scheduled_hello_world();
    let steps: Vec<(String, String)> = out
        .iter()
        .filter(|(p, _)| p.starts_with(&format!("{FN_DIR}/seq_")) && p.contains('_'))
        .map(|(p, b)| (p.clone(), String::from_utf8(b.clone()).unwrap()))
        .filter(|(_, b)| !b.contains("schedule function"))
        .collect();
    assert_eq!(steps.len(), 2, "two step functions emitted: {steps:#?}");
    let all = steps
        .iter()
        .map(|(_, b)| b.as_str())
        .collect::<Vec<_>>()
        .join("");
    assert!(
        all.lines()
            .filter(|l| !l.is_empty())
            .all(|l| !scan_line(l).unbound_s),
        "no sequence step may address an unbound `@s`:\n{all}"
    );
    assert!(
        all.contains("title @a title "),
        "the inline (at_ticks: 0) step's narrate addresses the party:\n{all}"
    );
    assert!(
        all.contains("scoreboard players set #party dw.f_late 1"),
        "the scheduled (at_ticks: 20) step's set-flag writes the party holder:\n{all}"
    );
    assert!(
        all.lines().any(|l| l == "weather clear"),
        "a global step effect stays bare (fires once):\n{all}"
    );
}

/// The unconditional `sched_executor` PackTest: every campaign (hello-world in
/// CI tier 2 included) proves the seam on a live server. It must drive the REAL
/// scheduler — an inline `function` call would run as the test's own dummy and
/// pass with the bug present, which is exactly how the pre-existing arrive
/// templates masked it.
#[test]
fn scheduled_executor_packtest_is_emitted_for_every_campaign() {
    for out in [
        build_dir(&common::hello_world_dir()),
        build_dir(&common::keep_trial_dir()),
        build_scheduled_hello_world(),
    ] {
        let ns = namespace_of(&out);
        let probe = file(
            &out,
            &format!("packtest-datapack/data/{ns}/function/pt_sched_probe.mcfunction"),
        );
        assert_eq!(
            probe.trim(),
            "scoreboard players set #party dw.f_pt_sched_probe 1",
            "the probe body comes from the real scheduled-bundle emitter"
        );
        let t = file(
            &out,
            &format!("packtest-datapack/data/{ns}/test/sched_executor.mcfunction"),
        );
        assert!(
            t.contains(&format!("schedule function {ns}:pt_sched_probe 2t")),
            "the probe must go through the vanilla scheduler, not an inline call:\n{t}"
        );
        assert!(
            !t.contains(&format!("function {ns}:pt_sched_probe\n")),
            "never call the probe inline — that supplies the executor the bug removes:\n{t}"
        );
        assert!(
            t.contains("await score #party dw.f_pt_sched_probe matches 1"),
            "the test awaits the flag on the party holder (spec-0018):\n{t}"
        );
        // Own init (batch model): the objective exists and the party score is
        // cleared before the await — "never set" is not 0 on a shared server.
        // This template owns that objective outright (it is test-only), which is
        // what makes an `await` on a batch-global holder safe.
        let clear = t
            .find("scoreboard players set #party dw.f_pt_sched_probe 0")
            .expect("own init clears the probe score");
        assert!(
            clear < t.find("schedule function").unwrap(),
            "clear before scheduling:\n{t}"
        );
    }
}

/// The content-path `sched_arrive_flag` PackTest: the real `move-npc` start
/// function, the real self-rescheduling driver, the flag asserted on the
/// dummy's own score. Emitted only for a campaign that has such a beat.
#[test]
fn arrive_flag_packtest_drives_the_real_driver() {
    let out = build_scheduled_hello_world();
    let t = file(
        &out,
        "packtest-datapack/data/hello-world/test/sched_arrive_flag.mcfunction",
    );
    assert!(
        t.contains("function hello-world:mv_keeper_exit"),
        "runs the real move-npc start function:\n{t}"
    );
    assert!(
        !t.contains("mv_tick_"),
        "never drives the tick function inline — the scheduler must do it:\n{t}"
    );
    assert!(
        t.contains("await score #party dw.f_arrived matches 1"),
        "awaits the arrival flag on the party holder (spec-0018):\n{t}"
    );
    // Own init + a timeout that outlives the real walk.
    assert!(
        t.contains("scoreboard players set #party dw.f_arrived 0")
            && t.contains("scoreboard players set #mrun_keeper_exit dw.sys 0"),
        "clears the party flag and the driver's re-entry latch:\n{t}"
    );
    let timeout: u32 = t
        .lines()
        .find_map(|l| l.strip_prefix("# @timeout "))
        .and_then(|v| v.parse().ok())
        .expect("explicit timeout");
    assert!(
        timeout > 100,
        "the timeout must outlive the real walk, got {timeout}"
    );
    // Campaigns without such a beat emit nothing.
    assert!(
        !build_dir(&common::hello_world_dir())
            .contains_key("packtest-datapack/data/hello-world/test/sched_arrive_flag.mcfunction"),
        "no move-npc arrival flag → no template"
    );
}

/// `scan_line` itself: `as` binds, `positioned as` / `rotated as` do not, and
/// an `@s` inside a JSON body is not a selector.
#[test]
fn scan_line_distinguishes_binding_from_positional_as() {
    assert!(scan_line("scoreboard players set @s dw.f_x 1").unbound_s);
    assert!(!scan_line("execute as @a run scoreboard players set @s dw.f_x 1").unbound_s);
    assert!(scan_line("execute positioned as @s run setblock ~ ~ ~ minecraft:stone").unbound_s);
    assert!(scan_line("execute rotated as @s run tp @s ~ ~ ~").unbound_s);
    assert!(!scan_line(r#"tellraw @a {"text":"@s"}"#).unbound_s);
    assert_eq!(
        scan_line("execute if score #t dw.sys matches 3 run function ns:arrive").inherited_calls,
        vec!["ns:arrive".to_string()]
    );
    assert!(
        scan_line("execute as @a run function ns:per_player")
            .inherited_calls
            .is_empty(),
        "a call under `as` inherits a PLAYER executor, not the server source"
    );
    assert_eq!(
        scan_line("schedule function ns:mv_tick_x 1t").scheduled,
        vec!["ns:mv_tick_x".to_string()]
    );
}
