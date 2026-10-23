//! **A gate's bar may be a door** (`compiler::region_fill`).
//!
//! A gate region names one block, and a door is two block states: its lower
//! half and its upper half. Every consumer of a gate region used to write the
//! one declared state into every cell, so a two-high iron door as a gate's bar
//! stood a lower half in its upper cell, which the server drops (`DW1002`), and
//! the iron door — the bar `DW1063` names first — could not be built.
//!
//! The fixture is hello-world with `hello-room`'s `anchor/door` made one iron
//! door per column over the doorway's two lower courses, the template holding
//! the door's two halves there as a piece exporting it would. The party opens
//! it on the talk and the campaign closes it again at the end, so a world-load
//! seal, an `open-gate` and a `close-gate` all act on it.
//!
//! The agreement: every consumer of the gate region is asked the block it puts
//! in each cell — the shipped world, the emitted `close-gate`, the generated
//! PackTest's staged seal, the critical path's configuration after the close —
//! and each must answer what [`region_fill::cells`] answers, cell for cell.
//! The planted red: the old shape, the declared state in every cell, is refused
//! by `DW1002` where the derived shape is held.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::assembled;
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::{Plan, ResolvedAnchor};
use delvec::compiler::region_fill;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::prefab::Region;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

/// The declared bar: an iron door's lower half, shut.
const DOOR: &str =
    "minecraft:iron_door[facing=south,half=lower,hinge=left,open=false,powered=false]";

/// The doorway's two lower courses, piece-local: two columns, two high.
const LOCAL: ([i32; 3], [i32; 3]) = ([4, 1, 6], [5, 2, 6]);

fn quests_doc() -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [
          {{ "type": "close-gate", "anchor": "anchor/door",
             "sealed_hint": "The iron door will not open for you." }},
          {{ "type": "campaign-complete" }}
        ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn campaign() -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: common::declared_quests_doc(&quests_doc(), &common::hello_world_dir()),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The library with `hello-room`'s gate made an iron door.
fn prefabs() -> PrefabRegistry {
    let mut reg = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let mut meta = reg.get("prefab/hello-room").expect("hello-room").clone();
    let door = meta
        .anchors
        .get_mut("anchor/door")
        .expect("hello-room declares anchor/door");
    door.region = Some(Region {
        from: LOCAL.0,
        to: LOCAL.1,
    });
    door.block = Some(DOOR.to_string());
    reg.insert(meta);
    reg
}

/// The plan's templates, `hello-room`'s holding the door's halves in the
/// doorway, as a piece that exports the door as its gate holds them.
fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    use delvec::admit::structure::{PaletteEntry, Structure};
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let mut bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file))
                    .unwrap_or_else(|e| panic!("{}: {e}", t.structure_file));
                if piece.prefab_id == "prefab/hello-room" {
                    let mut s = Structure::read(&bytes).unwrap();
                    for (c, state) in region_fill::cells(LOCAL, DOOR) {
                        let (name, props) = state.split_once('[').unwrap();
                        let props: Vec<(&str, &str)> = props
                            .trim_end_matches(']')
                            .split(',')
                            .map(|kv| kv.split_once('=').unwrap())
                            .collect();
                        s.set_cell(c, PaletteEntry::with_props(name, &props), None);
                    }
                    bytes = s.write();
                }
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

struct Built {
    plan_campaign: Campaign,
    prefabs: PrefabRegistry,
}

impl Built {
    fn new() -> Self {
        let prefabs = prefabs();
        let c = campaign();
        let d = common::validation_diagnostics(
            &c,
            &FullItemRegistry::v1_21_11(),
            &prefabs,
            &FullEntityRegistry::v1_21_11(),
        );
        assert!(
            d.iter()
                .all(|d| d.severity != delvewright_dsl::Severity::Error),
            "the door campaign validates: {d:#?}"
        );
        Built {
            plan_campaign: c,
            prefabs,
        }
    }

    fn plan(&self) -> Plan<'_> {
        Plan::build(&self.plan_campaign, &self.prefabs).expect("plan builds")
    }
}

fn build(plan: &Plan, prefabs: &PrefabRegistry) -> Result<BuildOutput, String> {
    emit::build(
        plan,
        &BTreeMap::new(),
        &structures(plan),
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| format!("{e:?}"))
}

/// The world region `anchor/door` resolves to.
fn region(plan: &Plan) -> ([i32; 3], [i32; 3]) {
    plan.anchors
        .iter()
        .find_map(|((_, n), r)| match r {
            ResolvedAnchor::Gate { from, to, .. } if n == "anchor/door" => Some((*from, *to)),
            _ => None,
        })
        .expect("anchor/door resolves to a gate")
}

/// What `region_fill` derives for the gate, cell by cell.
fn derived(region: ([i32; 3], [i32; 3])) -> BTreeMap<[i32; 3], String> {
    region_fill::cells(region, DOOR)
        .map(|(c, s)| (c, s.into_owned()))
        .collect()
}

/// The cells of `region` a list of `fill` commands leaves, executed as the
/// server executes them: each `fill` writes its one state over its box; a
/// `replace`-filtered or air fill is not a lay.
fn laid_by(lines: &str, region: ([i32; 3], [i32; 3])) -> BTreeMap<[i32; 3], String> {
    let inside = |c: [i32; 3]| {
        (0..3).all(|i| region.0[i].min(region.1[i]) <= c[i] && c[i] <= region.0[i].max(region.1[i]))
    };
    let mut out = BTreeMap::new();
    for line in lines.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.first() != Some(&"fill") || t.len() != 8 || t[7] == "minecraft:air" {
            continue;
        }
        let n = |i: usize| t[i].parse::<i32>().ok();
        let (Some(a), Some(b)) = (
            (|| Some([n(1)?, n(2)?, n(3)?]))(),
            (|| Some([n(4)?, n(5)?, n(6)?]))(),
        ) else {
            continue;
        };
        if !(inside(a) && inside(b)) {
            continue;
        }
        for c in assembled::region_cells(a, b) {
            out.insert(c, t[7].to_string());
        }
    }
    out
}

fn file(out: &BuildOutput, suffix: &str) -> String {
    out.iter()
        .find(|(p, _)| p.ends_with(suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no `{suffix}` in the build"))
}

/// **The agreement.** Every consumer of the gate region puts in each cell the
/// block `region_fill` derives: the shipped world, the emitted `close-gate`,
/// the PackTest's staged seal, and the critical path's configuration once the
/// close has fired. A consumer that writes the declared state everywhere, or
/// derives the halves its own way, answers differently and reds here.
#[test]
fn every_consumer_of_a_door_gate_lays_the_halves_region_fill_derives() {
    let built = Built::new();
    let plan = built.plan();
    let region = region(&plan);
    let want = derived(region);
    assert_eq!(want.len(), 4, "two columns, two courses: {want:?}");
    assert_eq!(
        want.values().filter(|s| s.contains("half=upper")).count(),
        2,
        "the upper course is the door's upper half: {want:?}"
    );

    let out = build(&plan, &built.prefabs).expect("a door gate builds");
    let structures = structures(&plan);
    let assembled = assembled::assemble(&plan, &structures);

    // 1. The shipped world: the assembly with its world-load seals.
    let shipped = assembled::shipped_blocks(
        &plan,
        &assembled.blocks,
        &[],
        &assembled.gate_seals,
        Some(region),
    );
    let shipped: BTreeMap<[i32; 3], String> = shipped
        .into_iter()
        .map(|(c, s)| (c, s.as_str().to_string()))
        .collect();
    assert_eq!(shipped, want, "the shipped world");

    // 2. The emitted `close-gate`: every shipped function, executed.
    let mut functions = String::new();
    for (p, b) in &out {
        if p.starts_with("datapack/") && p.ends_with(".mcfunction") {
            functions.push_str(std::str::from_utf8(b).unwrap());
            functions.push('\n');
        }
    }
    assert_eq!(laid_by(&functions, region), want, "the emitted close-gate");

    // 3. The generated PackTest's staged seal.
    let seal = file(&out, "/test/v08_seal_answers.mcfunction");
    assert_eq!(laid_by(&seal, region), want, "the PackTest's staged seal");

    // 4. The critical path's configuration once the close has fired.
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures);
    let config = delvec::compiler::view::beat::PathModel::critical(&plan).at(
        &plan,
        &world,
        plan.critical_path.len(),
    );
    let laid: BTreeMap<[i32; 3], String> = config
        .laid()
        .into_iter()
        .filter(|(c, _)| want.contains_key(c))
        .map(|(c, b)| (c, b.unwrap_or_else(|| "minecraft:air".to_string())))
        .collect();
    assert_eq!(laid, want, "the configuration after the close");

    // 5. The opening: one clear over the whole region, filtered to the door's
    // id, and a configuration between the open and the close holding every
    // cell of both halves empty.
    let (lo, hi) = region;
    let clear = format!(
        "fill {} {} {} {} {} {} minecraft:air replace minecraft:iron_door",
        lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
    );
    assert!(
        functions.lines().any(|l| l.trim() == clear),
        "the open-gate clears both halves in one command: {clear}"
    );
    let model = delvec::compiler::view::beat::PathModel::critical(&plan);
    let opened = (0..plan.critical_path.len()).any(|arrival| {
        let laid = model.at(&plan, &world, arrival).laid();
        want.keys()
            .all(|c| laid.get(c).is_some_and(Option::is_none))
    });
    assert!(opened, "some configuration holds the opened doorway empty");
}

/// **The planted red.** The old shape — the declared state in every cell of
/// the doorway — stands a lower half over a lower half, and `DW1002` refuses
/// the upper cell; the derived shape is held. Read over the world the build
/// judges, so the refusal is the build's own.
#[test]
fn one_state_across_a_door_gate_is_refused_and_the_halves_are_held() {
    let built = Built::new();
    let plan = built.plan();
    let region = region(&plan);
    let structures = structures(&plan);
    let assembled = assembled::assemble(&plan, &structures);
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures);
    let base = delvec::compiler::view::beat::picture_base(&plan, &assembled, &[]);

    let held = delvec::compiler::attached::judge(&plan, &world, &base);
    let in_door = |b: &delvec::compiler::attached::Binding| -> Vec<[i32; 3]> {
        b.dropped
            .iter()
            .filter(|d| region_fill::cells(region, DOOR).any(|(c, _)| c == d.cell))
            .map(|d| d.cell)
            .collect()
    };
    assert!(
        in_door(&held).is_empty(),
        "the derived halves are held: {:?}",
        held.dropped
    );

    let mut old = base.clone();
    for c in assembled::region_cells(region.0, region.1) {
        old.insert(c, delvec::compiler::blockstate::BlockState::new(DOOR));
    }
    let planted = delvec::compiler::attached::judge(&plan, &world, &old);
    let upper: Vec<[i32; 3]> = assembled::region_cells(region.0, region.1)
        .filter(|c| c[1] == region.0[1].max(region.1[1]))
        .collect();
    assert_eq!(
        in_door(&planted),
        upper,
        "one state across the doorway drops the lower half standing in each upper cell"
    );
    // The judge whose refusal the build raises as `DW1002`.
    assert!(
        planted
            .refusal()
            .is_some_and(|r| r.contains("minecraft:iron_door")),
        "{}",
        planted.line()
    );
}
