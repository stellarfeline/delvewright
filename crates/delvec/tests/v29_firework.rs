//! DSL v0.29 (spec-0068): what a firework emits, and the proof it owes.
//!
//! What this file pins down:
//! * **the emission** (criterion 2) — one `summon` per effect, read off the
//!   build rather than recalled: the entity, the written `LifeTime` per flight,
//!   the `FireworksItem` key, `flight_duration`, the five shape tokens spelled
//!   as the component spells them, and colours as `[I;…]` packed integers.
//!   Every emitted command is walked against the pinned command tree by
//!   `emit::build` itself, and two builds are byte-identical (ADR-0006);
//! * **the roof** (criterion 4) — a rocket under a ceiling is `DW0899` naming
//!   the first solid cell of its column, end to end through a real build, and
//!   the same rule asked of a column of exactly eight and of a flight-2 rocket
//!   over it;
//! * **the reach** (criterion 5) — a posted body within five blocks of the
//!   burst cell is `DW0899` naming the post, and the post enumeration is
//!   `DW0511`'s own, asserted by reading one from the other;
//! * **the binding line** (criterion 6) — the counts a run prints, and that a
//!   campaign declaring nothing prints zeroes rather than nothing.
//!
//! # Why the reach is asked of `firework::judge` rather than of a fixture
//!
//! A burst stands at least eight blocks over its mark, and the reach is five, so
//! a post can only be caught from **three or more courses above the launch
//! plane** — a wall walk, a gallery, a tower. No campaign fixture in this
//! repository has an anchor like that over open sky, and building one to
//! exercise arithmetic would make the test about the fixture. So the rule is
//! asked directly, of real `PostedPlace` values and a real block map, and the
//! wiring that carries a campaign's posts into it is asserted separately by
//! `the_posts_are_dw0511s_own_enumeration`.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::firework::{FireworkGate, Launch, judge};
use delvec::compiler::lethal::{ChosenBy, PostedPlace, posted_places};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::firework;
use delvewright_dsl::metrics::Body;
use delvewright_dsl::{parse_campaign, validate_campaign_with};

/// A private working directory per caller — the tests run in parallel threads of
/// one binary, and a shared scratch directory is a race whose symptom is a
/// missing file.
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The hall's `on_complete` bundle with one firework in front of it.
///
/// `offset` places the mark. The hall is eleven by six by eleven with its
/// ceiling four cells over `anchor/exit`, so `[0, 5, 0]` stands the rocket on
/// the hall's ROOF, under open sky, and `[0, 0, 0]` fires it into the ceiling.
fn quests_with_firework(offset: [i32; 3], flight: u32, explosions: &str) -> String {
    let base = std::fs::read_to_string(common::hello_world_dir().join("quests.json")).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&base).unwrap();
    let fw = serde_json::json!({
        "type": "firework",
        "at": { "anchor": "anchor/exit", "offset": offset },
        "flight": flight,
        "explosions": serde_json::from_str::<serde_json::Value>(explosions).unwrap(),
    });
    let on_complete = doc["content"]["quests"][0]["on_complete"]
        .as_array_mut()
        .unwrap();
    on_complete.insert(0, fw);
    serde_json::to_string_pretty(&doc).unwrap()
}

/// One gold `large_ball`, the simplest rocket the surface can write.
const ONE_BALL: &str = r##"[{ "shape": "large_ball", "colors": ["#ffd700"] }]"##;

/// Materialize hello-world with the given quests document and build it.
fn try_build(who: &str, quests: &str) -> Result<BuildOutput, (String, String)> {
    let dir = tmp(&format!("v29-firework-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    std::fs::write(dir.join("quests.json"), quests).unwrap();

    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("the fixture parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "the fixture validates clean: {diags:#?}");

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
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

/// Everything emitted, as one string.
fn all_functions(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(p, b)| format!("### {p}\n{}\n", String::from_utf8_lossy(b)))
        .collect()
}

/// The one emitted `summon minecraft:firework_rocket` line.
fn rocket_line(out: &BuildOutput) -> String {
    let all = all_functions(out);
    let hits: Vec<&str> = all
        .lines()
        .filter(|l| l.starts_with("summon minecraft:firework_rocket "))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one firework is declared, so exactly one rocket is summoned:\n{all}"
    );
    hits[0].to_string()
}

// ---------------------------------------------------------------------------
// Criterion 2 — the emission
// ---------------------------------------------------------------------------

/// The emitted line is the command the component reads, read off the build.
///
/// `emit::build` walks every line it writes against `CommandTree::v1_21_11`
/// before it returns, so reaching this assertion at all is the command tree's
/// verdict; what is asserted here is the payload the tree cannot see.
#[test]
fn the_emitted_rocket_carries_the_component_the_page_names() {
    let out = try_build("emit", &quests_with_firework([0, 5, 0], 1, ONE_BALL))
        .expect("a rocket on the hall's roof has sky over it");
    let line = rocket_line(&out);
    assert!(
        line.contains(&format!("LifeTime:{}", firework::lifetime_ticks(1))),
        "the emitter writes `LifeTime`, never leaving it to the game: {line}"
    );
    assert!(
        line.contains(&format!("{}:{{", firework::ITEM_FIELD)),
        "the entity's item field is `{}`: {line}",
        firework::ITEM_FIELD
    );
    assert!(
        line.contains(&format!("\"{}\"", firework::FIREWORKS_COMPONENT)),
        "the bursts ride in the `{}` component: {line}",
        firework::FIREWORKS_COMPONENT
    );
    assert!(
        line.contains("flight_duration:1b"),
        "the flight is a byte the component reads: {line}"
    );
    assert!(
        line.contains("shape:\"large_ball\""),
        "the shape is spelled as the component spells it: {line}"
    );
    assert!(
        line.contains("colors:[I;16766720]"),
        "`#ffd700` is emitted as the packed integer vanilla stores: {line}"
    );
}

/// `LifeTime` is the floor of the game's own range, per crafted flight.
#[test]
fn lifetime_is_written_per_flight() {
    for (flight, life) in [(1u32, 20), (2, 30), (3, 40)] {
        let out = try_build(
            &format!("life-{flight}"),
            &quests_with_firework([0, 5, 0], flight, ONE_BALL),
        )
        .expect("a rocket on the hall's roof has sky over it");
        let line = rocket_line(&out);
        assert!(
            line.contains(&format!("LifeTime:{life},")),
            "flight {flight} writes `LifeTime:{life}`: {line}"
        );
        assert!(
            line.contains(&format!("flight_duration:{flight}b")),
            "flight {flight} writes its own duration: {line}"
        );
    }
}

/// Every shape the game has, every optional field, and both colour lists — one
/// rocket of five stars, which is what the gallery's court fires.
#[test]
fn every_shape_and_every_optional_field_reaches_the_component() {
    let five = r##"[
      { "shape": "large_ball", "colors": ["#ffd700", "#ffffff"],
        "fade_colors": ["#8b0000"], "trail": true, "twinkle": true },
      { "shape": "small_ball", "colors": ["#4fa3ff"] },
      { "shape": "star", "colors": ["#ffe066"] },
      { "shape": "creeper", "colors": ["#5aa02c"] },
      { "shape": "burst", "colors": ["#ffffff"] }
    ]"##;
    let out = try_build("shapes", &quests_with_firework([0, 5, 0], 1, five))
        .expect("a rocket on the hall's roof has sky over it");
    let line = rocket_line(&out);
    for shape in firework::FireworkShape::ALL {
        assert!(
            line.contains(&format!("shape:\"{}\"", shape.token())),
            "`{}` reaches the component: {line}",
            shape.token()
        );
    }
    assert!(
        line.contains("colors:[I;16766720,16777215]"),
        "a two-colour star packs both: {line}"
    );
    assert!(
        line.contains("fade_colors:[I;9109504]"),
        "a fade is packed the same way: {line}"
    );
    assert!(line.contains("has_trail:1b"), "a trail is written: {line}");
    assert!(
        line.contains("has_twinkle:1b"),
        "a twinkle is written: {line}"
    );
    // A star that declares neither writes neither, so a campaign that wants the
    // plain burst gets the plain burst and not a pair of explicit falses.
    assert!(
        !line.contains("has_trail:0b") && !line.contains("has_twinkle:0b"),
        "an undeclared flag is absent, not false: {line}"
    );
}

/// Two builds of one campaign are byte-identical (ADR-0006) — the emitter reads
/// no clock and rolls nothing, which is the whole reason `LifeTime` is written.
#[test]
fn two_builds_are_byte_identical() {
    let q = quests_with_firework([0, 5, 0], 2, ONE_BALL);
    let a = try_build("det-a", &q).expect("builds");
    let b = try_build("det-b", &q).expect("builds");
    assert_eq!(
        all_functions(&a),
        all_functions(&b),
        "a firework moves no byte between two builds of one campaign"
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — the roof, end to end
// ---------------------------------------------------------------------------

/// A rocket fired inside the hall is `DW0899`, and the refusal names the cell
/// that stops it, the height the flight needed and what to do about it.
#[test]
fn a_rocket_under_the_halls_ceiling_is_dw0899() {
    let (code, message) = try_build("roof", &quests_with_firework([0, 0, 0], 1, ONE_BALL))
        .expect_err("the hall is six courses tall and the rocket needs eight");
    assert_eq!(code, "DW0899");
    assert!(
        message.contains("under a roof") && message.contains("is solid"),
        "the refusal names the shape and the cell: {message}"
    );
    assert!(
        message.contains("anchor/exit"),
        "the refusal names the mark: {message}"
    );
    assert!(
        message.contains("8 cells of open air"),
        "the refusal names the height the flight needed: {message}"
    );
}

/// The same declaration, moved onto the hall's roof, is green — the perturbation
/// that says the refusal above is about the ceiling and not about the verb.
#[test]
fn the_same_rocket_under_open_sky_is_green() {
    try_build("open", &quests_with_firework([0, 5, 0], 1, ONE_BALL))
        .expect("eight cells of open air over the mark");
}

// ---------------------------------------------------------------------------
// Criteria 4 and 5 — the rule, asked directly
// ---------------------------------------------------------------------------

/// A launch at the origin cell, at the given flight.
fn launch(flight: u8) -> Launch {
    Launch {
        path: "/content/quests/0/on_complete/0".to_string(),
        mark: "anchor/court".to_string(),
        cell: [0, 64, 0],
        flight,
    }
}

/// A post at `cell`, labelled as `DW0511` labels an NPC's.
fn npc_post(cell: [i32; 3]) -> PostedPlace {
    PostedPlace {
        label: "npc/warden`'s post `anchor/walk".to_string(),
        cell,
        body: Body::PLAYER,
        chosen_by: ChosenBy::Campaign,
    }
}

fn stone_at(cells: &[[i32; 3]]) -> BTreeMap<[i32; 3], String> {
    cells
        .iter()
        .map(|c| (*c, "minecraft:stone".to_string()))
        .collect()
}

/// A solid cell six blocks up refuses a flight-1 rocket and the message names
/// that cell; an open column of eight is green; a flight-2 rocket over the same
/// open eight is refused and names the eighteen it needed.
#[test]
fn the_roof_rule_reads_the_column_the_flight_needs() {
    let roofed = stone_at(&[[0, 70, 0]]);
    let (gate, findings) = judge(&[launch(1)], &[], &roofed);
    assert_eq!(gate.cells(), 8, "flight 1 walks eight cells");
    assert_eq!(findings.len(), 1, "a roof six blocks up is refused");
    assert!(
        findings[0].message.contains("[0, 70, 0]"),
        "the refusal names the solid cell: {}",
        findings[0].message
    );
    assert_eq!(gate.refused, 1);

    let open = BTreeMap::new();
    let (gate, findings) = judge(&[launch(1)], &[], &open);
    assert!(
        findings.is_empty(),
        "eight open cells are enough: {findings:?}"
    );
    assert_eq!(gate.cells(), 8);
    assert_eq!(gate.refused, 0);

    // Eight cells of air and then the world's own stone: flight 2 needs eighteen.
    let eight_then_stone = stone_at(&[[0, 73, 0]]);
    let (gate, findings) = judge(&[launch(2)], &[], &eight_then_stone);
    assert_eq!(gate.cells(), 18, "flight 2 walks eighteen cells");
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].message.contains("18 cells of open air"),
        "the refusal names the height flight 2 needed: {}",
        findings[0].message
    );
}

/// A posted body four cells from the burst is refused and named; six cells away
/// is green. The burst stands eight over the mark, so both posts are on the
/// wall walk a rocket over the court would reach — the only geometry this shape
/// can fire on.
#[test]
fn a_posted_body_in_reach_of_the_burst_is_dw0899() {
    let open = BTreeMap::new();
    let burst = [0, 72, 0];

    let near = npc_post([4, burst[1], 0]);
    let (gate, findings) = judge(&[launch(1)], std::slice::from_ref(&near), &open);
    assert_eq!(gate.posts, 1, "one post examined against one burst");
    assert_eq!(
        findings.len(),
        1,
        "four cells is inside the five-block reach"
    );
    assert!(
        findings[0].message.contains("npc/warden"),
        "the refusal names the post: {}",
        findings[0].message
    );
    assert!(
        findings[0].message.contains("[0, 72, 0]"),
        "the refusal names the burst cell: {}",
        findings[0].message
    );

    let far = npc_post([6, burst[1], 0]);
    let (gate, findings) = judge(&[launch(1)], std::slice::from_ref(&far), &open);
    assert_eq!(gate.posts, 1, "the same post is examined either way");
    assert!(findings.is_empty(), "six cells is clear: {findings:?}");
}

/// A post on the launch plane is never in reach, whatever its horizontal
/// distance: the burst is eight blocks up and the reach is five. Stated as a
/// test because it is the rule's whole shape — what it catches is a body
/// **above** the launch plane, and a reader who assumed otherwise would write a
/// campaign this proof says nothing about.
#[test]
fn a_post_on_the_launch_plane_is_never_in_reach() {
    let open = BTreeMap::new();
    for dx in 0..=5 {
        let post = npc_post([dx, 64, 0]);
        let (_, findings) = judge(&[launch(1)], std::slice::from_ref(&post), &open);
        assert!(
            findings.is_empty(),
            "a post {dx} cells from the mark, on its own plane, is eight below the burst"
        );
    }
}

/// The reach is judged with no line-of-sight credit: a wall between the burst
/// and the post does not excuse it. Same post, same burst, a solid cell in
/// between — still refused.
#[test]
fn a_wall_between_the_burst_and_the_post_is_not_credited() {
    let post = npc_post([4, 72, 0]);
    let wall = stone_at(&[[2, 72, 0]]);
    let (_, findings) = judge(&[launch(1)], std::slice::from_ref(&post), &wall);
    assert_eq!(
        findings.len(),
        1,
        "a wall the page says blocks the damage is not modelled"
    );
}

/// The binding line states every denominator, zeroes included.
#[test]
fn the_binding_line_states_what_was_examined() {
    let empty = FireworkGate::default();
    assert_eq!(
        empty.line(),
        "firework binding: 0 firework(s) declared, 0 burst column(s) checked to 0 cell(s), \
         0 post(s) within reach examined, 0 refused"
    );
    let (gate, _) = judge(
        &[launch(1)],
        std::slice::from_ref(&npc_post([20, 64, 20])),
        &BTreeMap::new(),
    );
    assert_eq!(
        gate.line(),
        "firework binding: 1 firework(s) declared, 1 burst column(s) checked to 8 cell(s), \
         1 post(s) within reach examined, 0 refused"
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — the enumeration is `DW0511`'s own
// ---------------------------------------------------------------------------

/// The posts this proof examines are the posts `DW0511` examines: one
/// enumeration, read from both sides.
///
/// The perturbation is a **post class**, not a post: the hall's campaign gains
/// a `cast` placement, which is one of the five families `DW0511` enumerates,
/// and the firework's own count moves with it. A private list in this module
/// would not.
#[test]
fn the_posts_are_dw0511s_own_enumeration() {
    let prefab_dir = common::prefabs_dir();
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();

    let read = |who: &str, quests: String| -> (usize, usize) {
        let dir = tmp(&format!("v29-firework-posts-{who}"));
        for f in common::STAGE_FILES {
            std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
        }
        std::fs::write(dir.join("quests.json"), quests).unwrap();
        let loaded = load_campaign_dir(&dir).unwrap();
        let campaign = parse_campaign(&loaded.raw).expect("parses");
        let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
        let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for piece in &area.pieces {
                for t in &piece.templates {
                    let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                    structures.insert(t.structure_file.clone(), bytes);
                }
            }
        }
        let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
        let entry = plan.campaign_start().map(|(_, pos)| pos);
        let seats = BTreeMap::new();
        let dw0511 = posted_places(&plan, entry, &seats).len();
        let (gate, _) = delvec::compiler::firework::check(&plan, &blocks, entry, &seats);
        assert_eq!(gate.rows.len(), 1, "one firework resolved");
        (dw0511, gate.posts)
    };

    let plain = quests_with_firework([0, 5, 0], 1, ONE_BALL);
    let (before_511, before_fw) = read("before", plain.clone());
    assert!(before_511 > 0, "the hall posts bodies");
    assert_eq!(
        before_fw, before_511,
        "one firework examines every post `DW0511` knows"
    );

    // Add a **respawn seat** — one of the five families `DW0511` enumerates, and
    // one this campaign does not already have (it already posts an entry spawn,
    // an npc and a `cast` placement).
    let mut doc: serde_json::Value = serde_json::from_str(&plain).unwrap();
    doc["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "type": "set-checkpoint", "anchor": "anchor/exit" }));
    let (after_511, after_fw) = read("after", serde_json::to_string_pretty(&doc).unwrap());
    assert_eq!(
        after_511,
        before_511 + 1,
        "`DW0511` sees the new post class"
    );
    assert_eq!(
        after_fw, after_511,
        "and the firework's own count is read from the same enumeration"
    );
}
