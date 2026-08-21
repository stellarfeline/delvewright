//! DSL v0.8 `collect` container adoption at the build tier.
//!
//! The DSL tier cannot know whether an anchor's cell holds a barrel — that needs
//! the assembled world — so "the container is really there" is the build error
//! `DW0438`, the sibling of spec-0021's `DW0431`. hello-room furnishes no
//! container, which makes it the negative fixture; a stage-7 batch that fills a
//! barrel at the anchor is the positive one (the same edited-model path `DW0431`
//! already honours: a batch can legitimately be what puts the barrel there).

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit;
use delvewright_compiler::plan::{Plan, Step};
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};

/// hello-world's quests stage with one `collect`, parameterised on the adoption
/// fields the objective declares.
fn quests_doc(fields: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.8.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "collect", "id": "obj/cheese", "item": "minecraft:bread", "count": 3,
             "anchor": "anchor/exit", "after": ["obj/talk"]{fields} }}
        ],
        "on_objective_complete": {{ "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ] }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// A stage-7 batch that stands a barrel on the collect's anchor cell — the
/// prefab furniture the objective adopts.
const BARREL_EDITS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "world-edits",
  "content": {
    "batches": [
      {
        "id": "batch/beach-camp",
        "area": "area/keep",
        "note": "Stand a barrel at the exit anchor so a collect objective can adopt it.",
        "edits": [
          {
            "verb": "select",
            "name": "region/barrel",
            "shape": {
              "kind": "box",
              "frame": { "kind": "anchor-relative", "anchor": "anchor/exit" },
              "min": [0, 0, 0],
              "max": [0, 0, 0]
            }
          },
          {
            "verb": "fill",
            "region": "region/barrel",
            "recipe": { "blocks": [ { "block": "minecraft:barrel", "weight": 1.0 } ] }
          }
        ]
      }
    ]
  }
}"#;

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str, edits: Option<&str>) -> Campaign {
    parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: edits.map(str::to_string),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses")
}

fn try_build(fields: &str, edits: Option<&str>) -> Result<emit::BuildOutput, emit::BuildFailure> {
    let c = parse_hw(&quests_doc(fields), edits);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&c, &items, &prefabs, &entities);
    assert!(diags.is_empty(), "campaign must validate clean: {diags:#?}");
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn text(out: &emit::BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "{path} emitted; files: {:?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

const ADOPTED: &str = r#", "container": "anchor/exit", "item_name": "Cheese", "fill_count": 2"#;

/// Adopting a cell that holds no container fails the build. The alternative is an
/// `item replace block` that fails SILENTLY on the server and ships an
/// uncompletable objective with nothing anywhere to pick up.
#[test]
fn adopting_a_container_that_is_not_there_is_dw0438() {
    let err = try_build(ADOPTED, None).expect_err("an empty cell must fail the build");
    match err {
        emit::BuildFailure::Diagnostic { code, message } => {
            assert_eq!(code, "DW0438");
            assert!(
                message.contains("obj/cheese") && message.contains("anchor/exit"),
                "the message must name the objective and the anchor: {message}"
            );
            assert!(
                message.contains("prefab"),
                "the prescription must point at the prefab: {message}"
            );
        }
        other => panic!("expected a DW0438 diagnostic, got {other:?}"),
    }
}

/// The adopted barrel is filled WHERE IT STANDS: no chest is conjured, the
/// objective's own stack lands in `container.0` carrying its display name, and
/// each padding stack repeats it in the slots after it.
#[test]
fn an_adopted_barrel_is_filled_in_place_named_and_padded() {
    let out = try_build(ADOPTED, Some(BARREL_EDITS)).expect("builds over the edited world");
    let f = text(
        &out,
        "datapack/data/hello-world/function/activate_o_cheese.mcfunction",
    );

    assert!(
        !f.contains("setblock"),
        "an adopted container must not be re-placed by the compiler:\n{f}"
    );
    let fills: Vec<&str> = f
        .lines()
        .filter(|l| l.starts_with("item replace block"))
        .collect();
    assert_eq!(fills.len(), 3, "one required stack + 2 padding:\n{f}");
    for (n, line) in fills.iter().enumerate() {
        assert!(line.contains(&format!("container.{n} with")), "{line}");
        assert!(
            line.contains("minecraft:bread[custom_name={\"italic\":false,\"text\":\"Cheese\"}] 3")
                || line.contains(
                    "minecraft:bread[custom_name={\"text\":\"Cheese\",\"italic\":false}] 3"
                ),
            "every slot carries the named stack: {line}"
        );
    }
    // All three fills address ONE cell — the barrel — not three.
    let cells: std::collections::BTreeSet<&str> = fills
        .iter()
        .map(|l| l.split(" container.").next().unwrap())
        .collect();
    assert_eq!(cells.len(), 1, "one container, three slots:\n{f}");
}

/// A `collect` that adopts nothing still conjures its own chest and fills the one
/// slot — the pre-0.8 emission, unchanged.
#[test]
fn an_unadopted_collect_still_places_its_own_chest() {
    let out = try_build("", None).expect("builds");
    let f = text(
        &out,
        "datapack/data/hello-world/function/activate_o_cheese.mcfunction",
    );
    assert!(f.contains("minecraft:chest"), "{f}");
    assert_eq!(
        f.lines()
            .filter(|l| l.starts_with("item replace block"))
            .count(),
        1,
        "one slot, no components:\n{f}"
    );
    assert!(
        f.contains("container.0 with minecraft:bread 3"),
        "the pre-0.8 fill line, verbatim:\n{f}"
    );
}

/// The critical-path step the bot walks to is the CONTAINER's cell: the harness
/// opens the block standing at that position, so an adopted barrel three blocks
/// off the objective anchor would otherwise be a guaranteed stall.
#[test]
fn the_critical_path_step_follows_the_adopted_container() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let campaign = parse_hw(&quests_doc(ADOPTED), None);
    let adopted = Plan::build(&campaign, &prefabs).expect("plan");
    let fill = adopted
        .collect_fills
        .iter()
        .find(|f| f.objective_id == "obj/cheese")
        .expect("the adoption resolved");
    let step_pos = adopted
        .critical_path
        .iter()
        .find_map(|s| match s {
            Step::Collect {
                objective_id, pos, ..
            } if objective_id == "obj/cheese" => Some(*pos),
            _ => None,
        })
        .expect("a collect step");
    assert_eq!(
        step_pos, fill.cell,
        "the bot must be sent to the container it has to open"
    );
    assert_eq!(fill.slots, 3, "one required stack + 2 padding");
}

/// The generated PackTest proves on a real server what no compile-time check can:
/// the fill LANDED in the adopted container, the padding occupies the slots after
/// slot 0, and the objective completes for the NAMED stack — the custom-name
/// component must not change what the adjudication sees.
#[test]
fn the_generated_packtest_drives_the_adopted_container_and_the_named_stack() {
    let out = try_build(ADOPTED, Some(BARREL_EDITS)).expect("builds");
    let pt = text(
        &out,
        "packtest-datapack/data/hello-world/test/collect_container.mcfunction",
    );
    assert!(
        pt.contains("function hello-world:activate_o_cheese"),
        "the template must drive the objective's own activation:\n{pt}"
    );
    assert!(
        pt.contains("if items block") && pt.contains("container.* minecraft:bread"),
        "the fill must be counted in the adopted container:\n{pt}"
    );
    // 3 slots x 3 bread: a dropped fill reads 0, padding that overwrote slot 0
    // reads one stack short.
    assert!(
        pt.contains("assert score #cadp dw.sys matches 9"),
        "the asserted total must be the stack repeated per filled slot:\n{pt}"
    );
    assert!(
        pt.contains("item replace entity") && pt.contains("custom_name"),
        "the stack presented to the adjudication must be the NAMED one:\n{pt}"
    );
    assert!(
        !pt.contains("give @a[tag=dw_t_cadp,limit=1] minecraft:bread 3"),
        "handing over the plain item would make the named-stack assertion vacuous:\n{pt}"
    );
}

/// No adoption anywhere -> no adoption PackTest (byte-identity for every campaign
/// written before the field).
#[test]
fn an_unadopted_campaign_emits_no_adoption_packtest() {
    let out = try_build("", None).expect("builds");
    assert!(
        !out.keys().any(|k| k.contains("collect_container")),
        "the adoption packtest must only appear when a collect adopts a container"
    );
}
