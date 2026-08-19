//! Party-shared progression (spec-0018).
//!
//! The contract in one line: **progress is a fact about the party, not about a
//! player.** Objective completion, quest activation/completion, story flags, the
//! announce-once latches and campaign completion all live on one fake-player
//! holder (`#party`), so any member's completing action advances everyone and an
//! `after: [obj/a, obj/b]` AND-join becomes a division of labour.
//!
//! The load-bearing test here is [`no_per_player_progression_scoreboard_remains`]
//! — the mechanical negative, in the shape of the stealth-sneak-removal
//! precedent: sweep every emitted pack of every fixture family and prove that no
//! progression score is addressed by anything other than the party holder. A
//! partial migration is exactly the failure mode that would soft-lock a real
//! party (player A's objective set, player B's guard still shut), and it is
//! invisible to any single-player test.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::analyze::analyze_campaign;
use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

// ---------------------------------------------------------------------------
// builders
// ---------------------------------------------------------------------------

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
        "unpinned",
        &skins,
    )
    .expect("emission succeeds")
}

/// The two-room AND-join fixture (spec-0018 acceptance criterion 1): two levers
/// in two different rooms, a joint successor, `min_players: 2`.
fn and_join_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("and-join")
}

/// Every fixture family, so the sweep below is not a claim about one campaign.
fn every_family() -> Vec<(&'static str, BuildOutput)> {
    vec![
        ("hello-world", build_dir(&common::hello_world_dir())),
        ("keep-crawl", build_dir(&common::keep_crawl_dir())),
        ("keep-trial", build_dir(&common::keep_trial_dir())),
        ("keep-vertical", build_dir(&common::keep_vertical_dir())),
        (
            "v04-showcase",
            build_dir(&common::compiler_fixtures_dir().join("v04-showcase")),
        ),
        (
            "v06-checkpoints",
            build_dir(&common::compiler_fixtures_dir().join("v06-checkpoints")),
        ),
        ("and-join", build_dir(&and_join_dir())),
    ]
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("expected build output `{path}`"))
            .clone(),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// the mechanical negative
// ---------------------------------------------------------------------------

/// Is `tok` the name of a **progression** scoreboard objective — the state
/// spec-0018 moved to the party holder?
///
/// Deliberately NOT included, because they are genuinely per-player and must
/// stay that way: `dw.class`/`dw.classed`/`dw.dlg_shown` (class selection),
/// `dw.dlg_<npc>`/`dw.i_<obj>` (this player's click), `dw.dmask` (this player's
/// dialog screen), `dw.hold` (this player's inventory), `dw.deaths`/
/// `dw.death_ack` (this player's respawn edge), `dw.st_grace`/`dw.st_safe`
/// (this player's exposure clock), and `dw.sys`/`dw.wave` (fake-player holders
/// that were never per-player).
fn is_progression_objective(tok: &str) -> bool {
    tok == "dw.campaign"
        || tok.starts_with("dw.o_")
        || tok.starts_with("dw.q_")
        || tok.starts_with("dw.qa_")
        || tok.starts_with("dw.f_")
        || tok.starts_with("dw.ann_")
}

/// **No per-player progression scoreboard remains in any emitted pack.**
///
/// Two rules over every emitted `.mcfunction` line of every pack (shipped
/// datapack, PackTest suite, creator overlay):
///
/// 1. wherever a progression objective is read or written, the holder token
///    immediately before it is `#party` — never `@s`, `@a`, `@p` or a tagged
///    dummy. The one exception is `scoreboard objectives add <name> dummy`,
///    which declares the objective and names no holder;
/// 2. no selector filters players by a progression score
///    (`@a[scores={dw.f_…=1..}]`) — under party state nothing ever writes one
///    onto a player, so such a selector could only ever match nothing.
#[test]
fn no_per_player_progression_scoreboard_remains() {
    let mut bad: Vec<String> = Vec::new();
    for (suite, out) in every_family() {
        for (path, bytes) in out.iter() {
            if !path.ends_with(".mcfunction") {
                continue;
            }
            let body = String::from_utf8_lossy(bytes);
            for line in body.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if line.starts_with("scoreboard objectives ") {
                    continue; // declaration, not an access
                }
                let toks: Vec<&str> = line.split_whitespace().collect();
                for (i, tok) in toks.iter().enumerate() {
                    if !is_progression_objective(tok) {
                        continue;
                    }
                    let holder = if i == 0 { "" } else { toks[i - 1] };
                    if holder != "#party" {
                        bad.push(format!("  {suite} {path}\n    [holder `{holder}`] {line}"));
                    }
                }
                for needle in [
                    "scores={dw.o_",
                    "scores={dw.q",
                    "scores={dw.f_",
                    "scores={dw.ann_",
                ] {
                    if line.contains(needle) {
                        bad.push(format!("  {suite} {path}\n    [selector] {line}"));
                    }
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "spec-0018: progression is party state — every objective/quest/flag/\
         announce/campaign score must be addressed on `#party`. These emitted \
         commands still address a player, so two players would hold divergent \
         progress and any split party would soft-lock:\n{}",
        bad.join("\n")
    );
}

// ---------------------------------------------------------------------------
// party emission
// ---------------------------------------------------------------------------

/// One player's completing action advances the party, and the beat's UI reaches
/// all of it: `complete_<obj>` writes `#party`, and its confirmation line, sound
/// and marker address `@a`.
#[test]
fn completion_writes_the_party_holder_and_addresses_everyone() {
    let out = build_dir(&common::keep_trial_dir());
    let body = text(
        &out,
        "datapack/data/keep-trial/function/complete_o_slay.mcfunction",
    );
    assert!(
        body.starts_with("scoreboard players set #party dw.o_slay 1"),
        "the party score flips first: {body}"
    );
    assert!(
        body.contains("tellraw @a ") && body.contains("playsound") && body.contains(" @a"),
        "the completion feedback reaches the whole party: {body}"
    );
    // The announce latch is party-level too: one "New objective" per objective,
    // not one per player who happens to be standing in the right room.
    let ann = text(
        &out,
        "datapack/data/keep-trial/function/announce_o_slay.mcfunction",
    );
    assert!(
        ann.contains("tellraw @a ") && ann.contains("scoreboard players set #party dw.ann_slay 1"),
        "the announce addresses the party and latches on the holder: {ann}"
    );
}

/// The activation/announce tick drivers no longer need a player at all — their
/// whole predicate is party state — while the drivers that must test a PLAYER
/// (proximity, held items, a fired trigger) keep their `as @a`.
#[test]
fn tick_drivers_split_party_predicates_from_player_tests() {
    let out = build_dir(&common::compiler_fixtures_dir().join("v04-showcase"));
    let tick = text(&out, "datapack/data/v04-showcase/function/tick.mcfunction");
    let announce = tick
        .lines()
        .find(|l| l.contains(":announce_o_slay"))
        .expect("announce driver emitted");
    assert!(
        !announce.contains("@a") && announce.contains("#party"),
        "an announce driver is pure party state: {announce}"
    );
    let reach = tick
        .lines()
        .find(|l| l.contains(":complete_o_shrine"))
        .expect("reach driver emitted");
    assert!(
        reach.starts_with("execute as @a ") && reach.contains("if entity @s[x="),
        "a reach driver still tests a real player's position: {reach}"
    );
}

/// A `give-item` arms the whole party by default; `carrier: "one"` hands a single
/// copy to the player whose action earned it.
#[test]
fn give_item_carrier_selects_party_or_completing_player() {
    let out = build_dir(&and_join_dir());
    let talk = text(
        &out,
        "datapack/data/and-join/function/complete_o_talk.mcfunction",
    );
    assert!(
        talk.contains("give @a minecraft:torch"),
        "the default give-item arms the whole party: {talk}"
    );
    // The class kit's `carrier: "one"` item enters the party exactly once.
    let kit = text(
        &out,
        "datapack/data/and-join/function/class_apply_warden.mcfunction",
    );
    assert!(
        kit.contains("give @s minecraft:iron_sword 1"),
        "ordinary kit items stay per-player gear: {kit}"
    );
    assert!(
        kit.lines()
            .any(|l| l.starts_with("execute unless score #kit_warden_")
                && l.contains("run give @s minecraft:lantern")),
        "a `carrier: one` kit item is latched to one copy per party: {kit}"
    );
}

// ---------------------------------------------------------------------------
// the lobby (min_players)
// ---------------------------------------------------------------------------

/// A campaign that declares `min_players: n` refuses to START below n: the class
/// dialog is gated on the live player count, and the waiting players get a
/// self-updating "x / n" actionbar. A 1-player campaign emits neither line.
#[test]
fn min_players_gates_the_lobby() {
    let gated = build_dir(&and_join_dir());
    let tick = text(&gated, "datapack/data/and-join/function/tick.mcfunction");
    assert!(
        tick.contains("execute store result score #lobby dw.sys if entity @a"),
        "the lobby counts the party each tick: {tick}"
    );
    let show = tick
        .lines()
        .find(|l| l.contains(":show_class"))
        .expect("class dialog driver emitted");
    assert!(
        show.starts_with("execute if score #lobby dw.sys matches 2.. "),
        "the class dialog stays shut below the declared party size: {show}"
    );
    assert!(
        tick.lines().any(|l| l.contains("#lobby dw.sys matches ..1")
            && l.contains("title @s actionbar")
            && l.contains("Waiting for the party")),
        "waiting players are told what they are waiting for: {tick}"
    );

    // Default (absent) `min_players` = 1: no lobby machinery at all.
    let ungated = build_dir(&common::hello_world_dir());
    let hw = text(
        &ungated,
        "datapack/data/hello-world/function/tick.mcfunction",
    );
    assert!(
        !hw.contains("#lobby"),
        "a party-of-one campaign emits no lobby gate: {hw}"
    );
    assert!(
        hw.lines()
            .any(|l| l.starts_with("execute as @a unless score @s dw.classed matches 1")),
        "…and its class driver is unchanged: {hw}"
    );
}

// ---------------------------------------------------------------------------
// the division-of-labour PackTest
// ---------------------------------------------------------------------------

/// The generated n-dummy template for an AND-join. Its shape IS the claim: n
/// different players, one arm each, and the successor opens for the party — with
/// the negative half (one arm is not enough) in the middle.
#[test]
fn and_join_emits_an_n_dummy_division_of_labour_packtest() {
    let out = build_dir(&and_join_dir());
    let t = text(
        &out,
        "packtest-datapack/data/and-join/test/party_join_shrine.mcfunction",
    );

    // Members: the framework dummy (`@s`) plus one this template spawns itself,
    // and removes again — PackTest's `# @dummy` gives exactly one.
    assert!(t.contains("# @dummy"), "the framework member: {t}");
    assert!(
        t.contains("dummy dwj0p1 spawn") && t.contains("dummy dwj0p1 leave"),
        "the second member is spawned and removed by this template alone: {t}"
    );

    // Each arm is completed by a DIFFERENT member.
    let north = t
        .find("execute as @s run function and-join:complete_o_ward_north")
        .expect("member 1 draws the north ward");
    let south = t
        .find("execute as @a[name=dwj0p1,limit=1] run function and-join:complete_o_ward_south")
        .expect("member 2 draws the south ward");
    assert!(north < south, "arms are drawn in member order: {t}");

    // The join's REAL emitted guard is probed, not a restatement of it.
    let guard = "execute if score #party dw.qa_wards matches 1 if score #party dw.o_ward_north \
                 matches 1 if score #party dw.o_ward_south matches 1 unless score #party \
                 dw.o_shrine matches 1 run scoreboard players set #pj_shrine dw.sys 1";
    assert!(t.contains(guard), "probes the emitted pending guard: {t}");

    // shut → still shut after ONE arm → open after both.
    let shut: Vec<usize> = t
        .match_indices("assert score #pj_shrine dw.sys matches 0")
        .map(|(i, _)| i)
        .collect();
    let open = t
        .find("assert score #pj_shrine dw.sys matches 1")
        .expect("the join opens once both arms are drawn");
    assert_eq!(shut.len(), 2, "shut before any arm AND after only one: {t}");
    assert!(
        shut[0] < north && north < shut[1] && shut[1] < south && south < open,
        "phase order (shut → arm 1 → still shut → arm 2 → open): {t}"
    );

    // Every member sees the successor: the LAST member consumes it.
    let consume = t
        .find("execute as @a[name=dwj0p1,limit=1] run function and-join:complete_o_shrine")
        .expect("a member other than the first completes the join");
    assert!(
        open < consume,
        "the successor is consumed after it opens: {t}"
    );
    assert!(
        t[consume..].contains("assert score #party dw.o_shrine matches 1"),
        "and the party carries the result: {t}"
    );
}

/// A campaign with no AND-join emits no division-of-labour template (the shipped
/// single-player families stay byte-identical in that respect).
#[test]
fn no_and_join_no_party_template() {
    let out = build_dir(&common::hello_world_dir());
    assert!(
        !out.keys().any(|p| p.contains("/test/party_join_")),
        "hello-world has no AND-join, so no division-of-labour template"
    );
}

// ---------------------------------------------------------------------------
// the analyzer's n-agent proof (DW0358)
// ---------------------------------------------------------------------------

/// Completability is proven with `min_players` agents. The two-room fixture
/// declares 2 and offers a 2-arm join whose arms are independently reachable, so
/// it analyzes clean; a campaign that declares 2 with nothing parallel to do is
/// `DW0358`.
#[test]
fn party_division_is_proven_or_dw0358() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();

    let loaded = load_campaign_dir(&and_join_dir()).unwrap();
    let ok = parse_campaign(&loaded.raw).expect("fixture parses");
    let diags = analyze_campaign(&ok, &prefabs);
    assert!(
        diags.is_empty(),
        "a real 2-arm join proves the 2-agent division: {diags:#?}"
    );

    // hello-world is one serial chain; declaring a mandatory party of 2 over it
    // is a claim nothing in the content backs.
    let hw = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let mut raw = hw.raw.clone();
    raw.world = raw.world.replacen("\"0.2.0\"", "\"0.6.0\"", 1).replacen(
        "\"target_minutes\": 5,",
        "\"target_minutes\": 5,\n    \"min_players\": 2,",
        1,
    );
    let serial = parse_campaign(&raw).expect("patched hello-world parses");
    let diags = analyze_campaign(&serial, &prefabs);
    assert!(
        diags.iter().any(|d| d.code == "DW0358"),
        "a mandatory-2 design with no parallel work must be DW0358: {diags:#?}"
    );

    // …and the same campaign at the default party of one is clean: the
    // single-agent proof is exactly the pre-spec-0018 verdict.
    let solo = parse_campaign(&hw.raw).expect("hello-world parses");
    assert!(
        analyze_campaign(&solo, &prefabs).is_empty(),
        "min_players 1 keeps the unchanged single-agent proof"
    );
}
