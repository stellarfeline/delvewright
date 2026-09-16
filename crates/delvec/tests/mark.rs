//! **A body stands at a mark** (spec-0066): an anchor plus an integer block
//! offset, for a body, a walk's destination and a cast row alike.
//!
//! One file, one fixture (hello-world on `hello-room`), one test per acceptance
//! criterion the plan and the emission can answer:
//!
//! | criterion | test |
//! |---|---|
//! | 3 — one resolution | [`two_actors_on_one_anchor_stand_one_block_apart_at_two_offsets`], [`two_actors_at_one_offset_from_one_anchor_are_dw0896`] |
//! | 4 — destinations | [`a_walk_to_a_mark_ends_on_the_mark_cell`], [`a_destination_mark_inside_a_wall_snaps_as_a_destination_does`] |
//! | 5 — the ledger | [`a_cast_row_naming_the_bare_anchor_of_a_body_at_an_offset_is_dw0461`] |
//! | 6 — the room | [`an_offset_leaving_the_piece_is_dw0897_at_build`], [`an_offset_reaching_the_last_cell_of_the_piece_is_green`] |
//!
//! `hello-room` is 11 × 6 × 11 with a stone shell, a dividing wall at local
//! z = 6, `spawn` at local `[5, 1, 2]`, `anchor/keeper-stand` at `[5, 1, 4]` and
//! `anchor/exit` at `[5, 1, 8]`; every cell named below is read from the plan,
//! never typed as a world coordinate.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::cohabit::{DW_ONE_MARK_TWO_BODIES, check_one_body_per_mark};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::mark::{DW_MARK_LEAVES_PIECE, check_marks_in_piece};
use delvec::compiler::nav;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, ExitTier, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const FN_DIR: &str = "datapack/data/hello-world/function";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world with `npcs[]` and `quests[]` rewritten by `edit`.
fn campaign_with(edit: impl FnOnce(&mut Value, &mut Value)) -> Campaign {
    let mut npcs: Value = serde_json::from_str(&read_hw("npcs.json")).unwrap();
    let mut quests: Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    edit(&mut npcs, &mut quests);
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs.to_string(),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("the fixture campaign parses")
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

fn build(c: &Campaign, prefabs: &PrefabRegistry) -> Result<BuildOutput, BuildFailure> {
    let plan = Plan::build(c, prefabs).expect("plan builds");
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn text(out: &BuildOutput, name: &str) -> String {
    let path = format!("{FN_DIR}/{name}.mcfunction");
    out.iter()
        .find(|(p, _)| p.as_str() == path)
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| {
            let names: Vec<&str> = out
                .keys()
                .map(|p| p.as_str())
                .filter(|p| p.starts_with(FN_DIR))
                .collect();
            panic!("expected `{path}`; this build emitted {names:?}")
        })
}

/// The `happening` a staging beat owes (`DW0481`).
fn happening(subject: &str) -> Value {
    json!({ "subject": subject, "verb": "arrives", "text": "a fixture beat" })
}

/// A `sequence` summoning `first` at tick 0 and `second` at tick 30, with
/// nothing between them that removes anybody — the shape `DW0896` judges.
fn rank_of_two(first: &str, second: &str) -> Value {
    json!({
        "type": "sequence",
        "steps": [
            { "at_ticks": 0, "effects": [
                { "type": "spawn-actor", "actor": first, "happening": happening(first) } ] },
            { "at_ticks": 30, "effects": [
                { "type": "spawn-actor", "actor": second, "happening": happening(second) } ] },
        ],
    })
}

/// Two actors on `spawn`, at `a` and `b`, entered by one sequence.
fn two_on_spawn(a: [i32; 3], b: [i32; 3]) -> Campaign {
    campaign_with(|_, quests| {
        quests["content"]["actors"] = json!([
            { "id": "actor/first", "entity": "minecraft:villager", "anchor": "spawn", "offset": a },
            { "id": "actor/second", "entity": "minecraft:villager", "anchor": "spawn", "offset": b },
        ]);
        common::objective_effects(quests, 0, "obj/talk")
            .push(rank_of_two("actor/first", "actor/second"));
    })
}

/// The `x y z` triple of the one `summon` line in a function.
fn summon_xyz(body: &str) -> [f64; 3] {
    let line = body
        .lines()
        .find(|l| l.contains("summon "))
        .unwrap_or_else(|| panic!("no summon line in:\n{body}"));
    let words: Vec<&str> = line.split_whitespace().collect();
    let at = words.iter().position(|w| *w == "summon").unwrap() + 2;
    [
        words[at].parse().unwrap(),
        words[at + 1].parse().unwrap(),
        words[at + 2].parse().unwrap(),
    ]
}

// ---------------------------------------------------------------------------
// Criterion 3 — one resolution
// ---------------------------------------------------------------------------

/// Two actors on one anchor at `[0,0,0]` and `[1,0,0]` are summoned one block
/// apart on x, and `DW0896` finds two cells, not one.
#[test]
fn two_actors_on_one_anchor_stand_one_block_apart_at_two_offsets() {
    let prefabs = prefabs();
    let c = two_on_spawn([0, 0, 0], [1, 0, 0]);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let (binding, verdict) = check_one_body_per_mark(&plan);
    assert!(
        verdict.is_ok(),
        "two offsets from one anchor are two cells: {:?}",
        verdict.err().map(|f| f.message)
    );
    assert_eq!(binding.placed, 3, "keeper + two actors placed: {binding:?}");
    assert_eq!(binding.cells, 3, "three bodies, three cells: {binding:?}");

    let out = build(&c, &prefabs).unwrap_or_else(|e| panic!("builds green: {e:?}"));
    let first = summon_xyz(&text(&out, "spawn_actor_first"));
    let second = summon_xyz(&text(&out, "spawn_actor_second"));
    assert_eq!(
        [
            second[0] - first[0],
            second[1] - first[1],
            second[2] - first[2]
        ],
        [1.0, 0.0, 0.0],
        "the offset moves the summon by exactly one block on x: {first:?} vs {second:?}"
    );
    let spawn = plan.point_any("spawn").unwrap();
    assert_eq!(
        first,
        [
            spawn[0] as f64 + 0.5,
            spawn[1] as f64,
            spawn[2] as f64 + 0.5
        ],
        "a zero offset is the anchor's own cell centre"
    );
}

/// The same two actors at one offset are one cell, and `DW0896` refuses them.
#[test]
fn two_actors_at_one_offset_from_one_anchor_are_dw0896() {
    let prefabs = prefabs();
    let c = two_on_spawn([1, 0, 0], [1, 0, 0]);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let (_, verdict) = check_one_body_per_mark(&plan);
    let f = verdict.expect_err("equal offsets from one anchor are one cell");
    assert_eq!(f.code, DW_ONE_MARK_TWO_BODIES, "{}", f.message);
    assert!(
        f.message.contains("spawn + [1, 0, 0]"),
        "the message spells the shared mark: {}",
        f.message
    );
    assert!(
        f.message.contains("an offset apiece"),
        "the remedy names the offset move: {}",
        f.message
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — destinations
// ---------------------------------------------------------------------------

/// hello-world with a walker spawned on `spawn` and walked to `to`.
fn walker_to(to: Value) -> Campaign {
    campaign_with(|_, quests| {
        quests["content"]["actors"] =
            json!([{ "id": "actor/walker", "entity": "minecraft:villager", "anchor": "spawn" }]);
        let effects = common::objective_effects(quests, 0, "obj/talk");
        effects.push(json!({
            "type": "spawn-actor", "actor": "actor/walker", "happening": happening("actor/walker")
        }));
        effects.push(json!({
            "type": "move-actor", "actor": "actor/walker", "to": to,
            "happening": happening("actor/walker")
        }));
    })
}

/// The `x y z` of the last `tp` a walk driver emits.
fn last_tp(body: &str) -> [f64; 3] {
    let line = body
        .lines()
        .rfind(|l| l.contains(" run tp "))
        .unwrap_or_else(|| panic!("no tp line in:\n{body}"));
    let words: Vec<&str> = line.split_whitespace().collect();
    let n = words.len();
    [
        words[n - 5].parse().unwrap(),
        words[n - 4].parse().unwrap(),
        words[n - 3].parse().unwrap(),
    ]
}

/// A walk to `anchor/exit + [2, 0, 0]` ends on that cell's centre.
#[test]
fn a_walk_to_a_mark_ends_on_the_mark_cell() {
    let prefabs = prefabs();
    let c = walker_to(json!({ "anchor": "anchor/exit", "offset": [2, 0, 0] }));
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let exit = plan.point_any("anchor/exit").unwrap();
    let out = build(&c, &prefabs).unwrap_or_else(|e| panic!("builds green: {e:?}"));
    let end = last_tp(&text(&out, "ma_tick_walker_exit_o2_0_0"));
    assert_eq!(
        end,
        [exit[0] as f64 + 2.5, exit[1] as f64, exit[2] as f64 + 0.5],
        "the walk ends on the mark's cell centre, two blocks east of the anchor"
    );
}

/// A destination mark inside the room's outer wall snaps to the nearest cell
/// the walker can stand on, through the same snap a destination anchor takes,
/// and the walk ends there.
#[test]
fn a_destination_mark_inside_a_wall_snaps_as_a_destination_does() {
    let prefabs = prefabs();
    let c = walker_to(json!({ "anchor": "anchor/exit", "offset": [0, 0, 2] }));
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let exit = plan.point_any("anchor/exit").unwrap();
    let wall = [exit[0], exit[1], exit[2] + 2];
    let world = nav::World::from_plan(&plan, &structures(&plan));
    let fp = nav::entity_footprint("minecraft:villager");
    let expected = world
        .snap_standable_fp(wall, nav::SNAP_RADIUS, &fp)
        .expect("a standable cell lies within the snap radius of the wall cell");
    assert_ne!(
        expected, wall,
        "the mark is inside the wall, so the snap must move it"
    );
    let moves = nav::plan_actor_moves(&plan, &world).expect("the walk routes");
    let leg = moves
        .iter()
        .find(|m| m.actor == "actor/walker")
        .expect("the walker's leg is planned");
    assert_eq!(
        leg.target, expected,
        "the walk's target is the snap of the mark's cell"
    );
    let out = build(&c, &prefabs).unwrap_or_else(|e| panic!("builds green: {e:?}"));
    let end = last_tp(&text(&out, "ma_tick_walker_exit_o0_0_2"));
    assert_eq!(
        end,
        [
            expected[0] as f64 + 0.5,
            expected[1] as f64,
            expected[2] as f64 + 0.5
        ],
        "the walk ends on the snapped cell"
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — the ledger
// ---------------------------------------------------------------------------

/// The keeper stands at `anchor/keeper-stand + [1, 0, 0]`, and the quest's cast
/// row spells `at` as `at`.
fn keeper_at_offset_cast(at: Value) -> Campaign {
    campaign_with(|npcs, quests| {
        npcs["content"]["npcs"][0]["offset"] = json!([1, 0, 0]);
        quests["content"]["quests"][0]["cast"]["npc/keeper"]["at"] = at;
    })
}

fn dw0461(c: &Campaign) -> Vec<String> {
    delvec::compiler::cast::check_cast(c)
        .into_iter()
        .filter(|d| d.code == "DW0461")
        .map(|d| d.message)
        .collect()
}

/// A body at an offset and a cast row naming its bare anchor is `DW0461`; the
/// same row spelling the mark is green, through the plan's place arm too.
#[test]
fn a_cast_row_naming_the_bare_anchor_of_a_body_at_an_offset_is_dw0461() {
    let bare = keeper_at_offset_cast(json!("anchor/keeper-stand"));
    let found = dw0461(&bare);
    assert_eq!(
        found.len(),
        1,
        "the bare anchor is not where the body stands: {found:#?}"
    );
    assert!(
        found[0].contains("anchor/keeper-stand + [1, 0, 0]"),
        "the message spells the mark the history left the body on: {}",
        found[0]
    );

    let spelled =
        keeper_at_offset_cast(json!({ "anchor": "anchor/keeper-stand", "offset": [1, 0, 0] }));
    assert!(
        dw0461(&spelled).is_empty(),
        "the row spelling the mark agrees with the world: {:#?}",
        dw0461(&spelled)
    );
    let prefabs = prefabs();
    let out = build(&spelled, &prefabs).unwrap_or_else(|e| panic!("builds green: {e:?}"));
    let plan = Plan::build(&spelled, &prefabs).expect("plan builds");
    let stand = plan.point_any("anchor/keeper-stand").unwrap();
    let summon = out
        .values()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .flat_map(|t| {
            t.lines()
                .filter(|l| l.contains("summon minecraft:villager") && l.contains("dw_npc"))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .next()
        .expect("the keeper is summoned");
    assert_eq!(
        summon_xyz(&summon),
        [
            stand[0] as f64 + 1.5,
            stand[1] as f64,
            stand[2] as f64 + 0.5
        ],
        "the keeper's summon stands at the mark: {summon}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 6 — the room
// ---------------------------------------------------------------------------

/// The keeper on `anchor/keeper-stand` at `offset`, and the cast row spelling
/// the same mark.
fn keeper_at(offset: [i32; 3]) -> Campaign {
    campaign_with(|npcs, quests| {
        npcs["content"]["npcs"][0]["offset"] = json!(offset);
        quests["content"]["quests"][0]["cast"]["npc/keeper"]["at"] =
            json!({ "anchor": "anchor/keeper-stand", "offset": offset });
    })
}

/// The inclusive box of the piece `anchor/keeper-stand` belongs to, and its cell.
fn keeper_piece(plan: &Plan) -> ([i32; 3], [i32; 3], [i32; 3]) {
    let (area, cell) = plan.point_any_site("anchor/keeper-stand").unwrap();
    let (lo, hi) = plan.piece_bounds(&area, cell);
    (cell, lo, hi)
}

/// An offset one block past the piece's high corner is `DW0897`, at build tier,
/// and the message names the cell it reaches and the box it left.
#[test]
fn an_offset_leaving_the_piece_is_dw0897_at_build() {
    assert_eq!(DW_MARK_LEAVES_PIECE.exit_tier(), ExitTier::Build);
    let prefabs = prefabs();
    let probe_campaign = keeper_at([0, 0, 0]);
    let probe = Plan::build(&probe_campaign, &prefabs).expect("plan builds");
    let (cell, lo, hi) = keeper_piece(&probe);
    let offset = [hi[0] - cell[0] + 1, 0, 0];
    let c = keeper_at(offset);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let (binding, verdict) = check_marks_in_piece(&plan);
    let f = verdict.expect_err("a mark past the piece box is refused");
    assert_eq!(f.code, DW_MARK_LEAVES_PIECE);
    let reached = [cell[0] + offset[0], cell[1], cell[2]];
    assert!(
        f.message.contains(&format!("{reached:?}")),
        "the message names the cell reached {reached:?}: {}",
        f.message
    );
    assert!(
        f.message.contains(&format!("{lo:?}..={hi:?}")),
        "the message names the box left: {}",
        f.message
    );
    assert!(
        f.message.contains("anchor/keeper-stand"),
        "the message names the anchor: {}",
        f.message
    );
    // The body and the cast row that spells its mark both leave the piece.
    assert_eq!(binding.refused, 2, "{binding:?}");
    assert_eq!(binding.offset_bodies, 1, "{binding:?}");
    assert_eq!(binding.cast_marks, 1, "{binding:?}");

    match build(&c, &prefabs) {
        Err(BuildFailure::Diagnostic { code, .. }) => assert_eq!(code, DW_MARK_LEAVES_PIECE),
        Err(e) => panic!("expected DW0897 from the build, got {e:?}"),
        Ok(_) => panic!("a mark past the piece box must not build"),
    }
}

/// An offset reaching the piece's high corner — its last cell — is inside the
/// box, and `DW0897` passes it.
#[test]
fn an_offset_reaching_the_last_cell_of_the_piece_is_green() {
    let prefabs = prefabs();
    let probe_campaign = keeper_at([0, 0, 0]);
    let probe = Plan::build(&probe_campaign, &prefabs).expect("plan builds");
    let (cell, _, hi) = keeper_piece(&probe);
    let c = keeper_at([hi[0] - cell[0], hi[1] - cell[1], hi[2] - cell[2]]);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let (binding, verdict) = check_marks_in_piece(&plan);
    assert!(
        verdict.is_ok(),
        "the piece's last cell is inside its box: {:?}",
        verdict.err().map(|f| f.message)
    );
    assert_eq!(binding.refused, 0, "{binding:?}");
    assert_eq!(binding.offset_bodies, 1, "{binding:?}");
    let line = binding.line();
    assert!(
        line.contains("1 with a non-zero offset") && line.contains("0 refused"),
        "{line}"
    );
}
