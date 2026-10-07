//! DSL v0.36 (spec-0092): what a lightning strike emits, and the proofs it owes.
//!
//! What this file pins down:
//! * **the emission** (criterion 2) — one `summon minecraft:lightning_bolt` per
//!   effect at the mark's cell centre on its plane, the same cell the strike
//!   gate reads; every line walked against the pinned command tree by
//!   `emit::build`; two builds byte-identical (ADR-0006);
//! * **the reach** (criterion 3) — a body whose box meets the bolt's is
//!   `DW0958`, end to end on the keeper (a villager the bolt would make a
//!   witch), and asked of `lightning::in_reach` one cell either side of every
//!   face; the posts are `DW0511`'s own enumeration;
//! * **the struck block** (criterion 4) — copper and a lightning rod under the
//!   mark are `DW0959`, stone is green;
//! * **no fire** (criterion 5) — a campaign declaring a strike seals the
//!   gamerule that decides it at `0`;
//! * **the binding line** (criterion 7) and the artifact.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::blockstate::{BlockMap, BlockState};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::lethal::{ChosenBy, PostedPlace, posted_places};
use delvec::compiler::lightning::{LightningGate, Strike, in_reach, judge};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::metrics::Body;
use delvewright_dsl::{lightning, parse_campaign, validate_campaign_with};

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The hall's `on_complete` bundle with one strike in front of it, at `anchor`
/// plus `offset`.
fn quests_with_strike(anchor: &str, offset: [i32; 3]) -> String {
    let base = std::fs::read_to_string(common::hello_world_dir().join("quests.json")).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&base).unwrap();
    doc["content"]["quests"][0]["on_complete"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            serde_json::json!({ "type": "lightning", "at": { "anchor": anchor, "offset": offset } }),
        );
    serde_json::to_string_pretty(&doc).unwrap()
}

fn materialise(who: &str, quests: &str) -> PathBuf {
    let dir = tmp(&format!("v36-lightning-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    std::fs::write(dir.join("quests.json"), quests).unwrap();
    dir
}

fn structures_of(plan: &Plan, prefab_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut structures = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    structures
}

fn try_build(who: &str, quests: &str) -> Result<BuildOutput, (String, String)> {
    let dir = materialise(who, quests);
    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the fixture parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "the fixture validates clean: {diags:#?}");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let structures = structures_of(&plan, &prefab_dir);
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| match e {
        emit::BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a diagnostic, got {other:?}"),
    })
}

fn all_functions(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction") && p.starts_with("datapack/"))
        .map(|(p, b)| format!("### {p}\n{}\n", String::from_utf8_lossy(b)))
        .collect()
}

fn bolt_lines(out: &BuildOutput) -> Vec<String> {
    all_functions(out)
        .lines()
        .filter(|l| l.starts_with("summon minecraft:lightning_bolt "))
        .map(str::to_string)
        .collect()
}

fn gate_json(out: &BuildOutput) -> Option<serde_json::Value> {
    out.iter()
        .find(|(p, _)| p.as_str() == "validation/lightning-gate.json")
        .map(|(_, b)| serde_json::from_slice(b).unwrap())
}

/// On the hall's roof, over the exit: clear of the keeper and the arrival.
const ROOF: [i32; 3] = [0, 5, 0];

// ---------------------------------------------------------------------------
// Criterion 2 — the emission
// ---------------------------------------------------------------------------

#[test]
fn one_strike_summons_one_bolt_at_the_cell_the_gate_read() {
    let out = try_build("emit", &quests_with_strike("anchor/exit", ROOF))
        .expect("a strike on the roof is clear of every post");
    let lines = bolt_lines(&out);
    assert_eq!(lines.len(), 1, "one strike, one bolt: {lines:?}");
    let gate = gate_json(&out).expect("a campaign declaring a strike writes the artifact");
    let cell: Vec<i64> = gate["strikes"][0]["cell"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    assert_eq!(
        lines[0],
        format!(
            "summon minecraft:lightning_bolt {}.5 {}.0 {}.5",
            cell[0], cell[1], cell[2]
        ),
        "the bolt stands at the cell's centre, on the mark's plane"
    );
    assert_eq!(gate["declared"], 1);
    assert_eq!(gate["refused"], 0);
}

#[test]
fn two_builds_are_byte_identical() {
    let q = quests_with_strike("anchor/exit", ROOF);
    let a = try_build("det-a", &q).expect("builds");
    let b = try_build("det-b", &q).expect("builds");
    assert_eq!(all_functions(&a), all_functions(&b));
}

#[test]
fn a_campaign_with_no_strike_writes_no_artifact() {
    let base = std::fs::read_to_string(common::hello_world_dir().join("quests.json")).unwrap();
    let out = try_build("none", &base).expect("hello-world builds");
    assert!(
        gate_json(&out).is_none(),
        "nobody who has not opted in moves a byte"
    );
    assert!(bolt_lines(&out).is_empty());
}

// ---------------------------------------------------------------------------
// Criterion 5 — no fire
// ---------------------------------------------------------------------------

/// The bolt lights fire only where a non-spectator player stands strictly
/// closer than this gamerule's radius; sealed at 0, no distance qualifies. The
/// claim in spec-0092 §2.3 holds exactly while this line is emitted.
#[test]
fn a_campaign_with_a_strike_seals_the_fire_gamerule_at_zero() {
    let out = try_build("fire", &quests_with_strike("anchor/exit", ROOF)).expect("builds");
    let want = format!("gamerule {} 0", lightning::FIRE_GAMERULE);
    assert!(
        all_functions(&out).lines().any(|l| l == want),
        "`{want}` is emitted in the setup"
    );
}

// ---------------------------------------------------------------------------
// Criterion 3 — the reach
// ---------------------------------------------------------------------------

/// A bolt on the keeper's own stand: the keeper is a villager, and the bolt
/// would turn him into a witch.
#[test]
fn a_strike_on_the_keepers_stand_is_dw0958() {
    let (code, message) = try_build(
        "keeper",
        &quests_with_strike("anchor/keeper-stand", [0, 0, 0]),
    )
    .expect_err("the keeper stands in the bolt's box");
    assert_eq!(code, "DW0958");
    assert!(
        message.contains("npc/keeper"),
        "the refusal names the post: {message}"
    );
    assert!(
        message.contains("witch"),
        "and says what the bolt does: {message}"
    );
}

fn post(cell: [i32; 3]) -> PostedPlace {
    PostedPlace {
        label: "npc/warden`'s post `anchor/walk".to_string(),
        cell,
        body: Body::PLAYER,
        chosen_by: ChosenBy::Campaign,
    }
}

/// One cell inside and one cell outside each face of the box, for a player's
/// body (0.6 wide, 1.8 tall) and a bolt at the origin.
#[test]
fn the_reach_is_the_bolts_box_against_the_bodys() {
    let o = [0, 64, 0];
    let p = Body::PLAYER;
    for (inside, outside) in [
        ([3, 64, 0], [4, 64, 0]),
        ([-3, 64, 0], [-4, 64, 0]),
        ([0, 64, 3], [0, 64, 4]),
        ([0, 64, -3], [0, 64, -4]),
        ([0, 72, 0], [0, 73, 0]),
        ([0, 60, 0], [0, 59, 0]),
    ] {
        assert!(in_reach(o, inside, p), "{inside:?} is in reach");
        assert!(!in_reach(o, outside, p), "{outside:?} is not");
    }
}

#[test]
fn a_posted_body_in_reach_is_dw0958_and_one_out_is_green() {
    let strike = Strike {
        path: "/content/quests/0/on_complete/0".to_string(),
        mark: "anchor/court".to_string(),
        cell: [0, 64, 0],
    };
    let (_, near) = judge(
        std::slice::from_ref(&strike),
        &[post([2, 64, 1])],
        &BlockMap::new(),
    );
    assert_eq!(near.len(), 1);
    assert_eq!(near[0].code.to_string(), "DW0958");
    assert!(near[0].message.contains("npc/warden"));
    let (gate, far) = judge(
        std::slice::from_ref(&strike),
        &[post([4, 64, 0])],
        &BlockMap::new(),
    );
    assert!(far.is_empty(), "{far:?}");
    assert_eq!(gate.posts, 1);
}

/// The posts this proof examines are `DW0511`'s: one enumeration, read from both
/// sides, and a post class added to the campaign moves both counts.
#[test]
fn the_posts_are_dw0511s_own_enumeration() {
    let prefab_dir = common::prefabs_dir();
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let read = |who: &str, quests: String| -> (usize, usize) {
        let dir = materialise(&format!("posts-{who}"), &quests);
        let loaded = load_campaign_dir(&dir).unwrap();
        let campaign = parse_campaign(&loaded.raw).expect("parses");
        let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
        let structures = structures_of(&plan, &prefab_dir);
        let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
        let entry = plan.campaign_start().map(|(_, pos)| pos);
        let seats = BTreeMap::new();
        let dw0511 = posted_places(&plan, entry, &seats).len();
        let (gate, _) = delvec::compiler::lightning::check(&plan, &blocks, entry, &seats);
        assert_eq!(gate.rows.len(), 1, "one strike resolved");
        (dw0511, gate.posts)
    };
    let plain = quests_with_strike("anchor/exit", ROOF);
    let (b511, bstrike) = read("before", plain.clone());
    assert!(b511 > 0);
    assert_eq!(bstrike, b511);
    let mut doc: serde_json::Value = serde_json::from_str(&plain).unwrap();
    doc["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "type": "set-checkpoint", "anchor": "anchor/exit" }));
    let (a511, astrike) = read("after", serde_json::to_string_pretty(&doc).unwrap());
    assert_eq!(a511, b511 + 1, "`DW0511` sees the new post class");
    assert_eq!(
        astrike, a511,
        "and the strike gate reads the same enumeration"
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — the struck block
// ---------------------------------------------------------------------------

#[test]
fn a_struck_block_the_game_rewrites_is_dw0959_and_stone_is_green() {
    let strike = Strike {
        path: "/content/quests/0/on_complete/0".to_string(),
        mark: "anchor/court".to_string(),
        cell: [0, 64, 0],
    };
    let under = |block: &str| -> BlockMap {
        let mut m = BlockMap::new();
        m.insert([0, 63, 0], BlockState::new(block));
        m
    };
    for block in [
        "minecraft:waxed_copper_block",
        "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]",
        "minecraft:exposed_cut_copper",
    ] {
        let (_, f) = judge(std::slice::from_ref(&strike), &[], &under(block));
        assert_eq!(f.len(), 1, "{block}");
        assert_eq!(f[0].code.to_string(), "DW0959", "{block}");
        assert!(
            f[0].message.contains(block.split('[').next().unwrap()),
            "{}",
            f[0].message
        );
    }
    let (gate, f) = judge(
        std::slice::from_ref(&strike),
        &[],
        &under("minecraft:stone"),
    );
    assert!(f.is_empty());
    assert_eq!(gate.rows[0].struck.as_deref(), Some("minecraft:stone"));
}

// ---------------------------------------------------------------------------
// Criterion 7 — the binding line
// ---------------------------------------------------------------------------

#[test]
fn the_binding_line_states_what_was_examined() {
    assert_eq!(
        LightningGate::default().line(),
        "lightning binding: 0 strike(s) declared, 0 struck block(s) read, 0 post(s) within reach \
         examined, 0 refused"
    );
    let strike = Strike {
        path: "/x".to_string(),
        mark: "anchor/court".to_string(),
        cell: [0, 64, 0],
    };
    let (gate, _) = judge(&[strike], &[post([20, 64, 20])], &BlockMap::new());
    assert_eq!(
        gate.line(),
        "lightning binding: 1 strike(s) declared, 1 struck block(s) read, 1 post(s) within reach \
         examined, 0 refused"
    );
}
