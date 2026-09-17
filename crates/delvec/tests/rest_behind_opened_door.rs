//! **The arrivals past the last objective** — the quest states `DW0525` walks.
//!
//! `DW0525` asks whether a body leaving a respawn seat can get back to everything
//! the entry reaches, under every quest state that can hold while that seat is in
//! force. Those states are the critical path's arrival steps, and the critical path
//! is `[select-class, objective…, assert-complete]` — so its last two arrivals (the
//! completion assertion, and the one-past-the-end sentinel the sweep ends on) are
//! not objectives. `Plan::strict_ancestor_steps` therefore owes those two arrivals
//! a row of their own, holding every objective on the path; with the rows keyed by
//! objective alone, nothing but step `0` counts as fired at the two arrivals where
//! everything has fired, and only what the world is built holding survives.
//!
//! Both directions of that matter, and they are the first two tests below.
//!
//! * A rest point behind a door the critical path OPENS reads walled in — a true
//!   campaign refused, and a refusal no repair answers but building the door open,
//!   which changes what the player sees.
//! * A rest point whose one way out the LAST objective SEALS reads open — a false
//!   campaign admitted, which is the direction that ships a purse the player can
//!   never reach again.
//!
//! The third asks what that "every objective" set may credit (spec-0051): an
//! objective the party can skip is on the path like any other, and the answer is
//! that forcedness is decided a layer earlier.
//!
//! `hello-room` is the geometry: one room, one 2-wide doorway through a dividing
//! wall, `anchor/door` the six cells of `iron_bars` the prefab authors across it,
//! and `anchor/exit` beyond. Prefabs come from `campaigns/prefabs`
//! (`common::prefabs_dir()`); the mirror case uses a private copy of that library
//! (`CARGO_TARGET_TMPDIR`) carrying one extra anchor, for the reason
//! [`open_gate_prefabs`] gives.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world classes doc with the wanderer's bread declared the **flask**.
/// A campaign that places a `bonfire` and declares no flask anywhere in its kits is
/// `DW0476`, so this is what makes a rest point expressible on this fixture at all.
fn classes_with_a_flask() -> String {
    let src = hw("classes.json");
    let out = src.replace(
        r#"            "count": 3,
            "item": "minecraft:bread""#,
        r#"            "count": 3,
            "flask": true,
            "item": "minecraft:bread""#,
    );
    assert_ne!(out, src, "the flask really is in the kit");
    out
}

/// The purse, the stake and the death that leaves one. `DW0525` is scoped to a
/// campaign that declares `stakes[]` — without these the placement table is never
/// built and neither case below is judged at all.
const PURSE_AND_STAKE: &str = r#"
    "state": [
      { "id": "state/embers", "scope": "player", "initial": 5, "name": "Embers",
        "note": "what a death takes" }
    ],
    "stakes": [
      { "id": "stake/embers", "state": "state/embers",
        "collected_message": "You take back what the drop took." }
    ],
    "on_death": [ { "type": "drop-stake", "stake": "stake/embers" } ],"#;

/// A hello-world `quests` doc: talk to the keeper, then reach `anchor/exit` beyond
/// the dividing wall. `talk_effects` is the bundle the first beat fires and
/// `exit_effects` the bundle the last one fires — the two seams both cases need.
fn quests_doc(talk_effects: &str, exit_effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{{PURSE_AND_STAKE}
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
          "obj/talk": [ {talk_effects} ],
          "obj/exit": [ {exit_effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn parse_hw(quests: &str) -> Campaign {
    parse_hw_planned(quests, &hw("quest-plan.json"))
}

fn parse_hw_planned(quests: &str, quest_plan: &str) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: classes_with_a_flask(),
        quest_plan: quest_plan.to_string(),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

/// A private copy of the prefab library whose `hello-room` declares a **second
/// gate**, `anchor/inner-door`, across the near room at `z = 3` — nine cells wide
/// and three high, all of them **air** in the `.nbt`.
///
/// The mirror case needs a gate the world builds OPEN, and the shipped library's
/// one such gate (`island-mountain`'s boulder) belongs to a tileset no fixture
/// campaign places. A gate the prefab authors shut is a world-load fill at step
/// `0`, and step `0` is the one firing the defect never drops — so on a barred
/// door the wrong verdict is masked by a right one and nothing can be measured.
/// Authored open, the region is passable until the campaign's own `close-gate`
/// fills it, and that firing is the whole question.
///
/// A world-edit cannot stand in for this: `Assembled::gate_seals` is measured when
/// the pieces are placed, before any batch runs, so unbarring `anchor/door` with a
/// `replace` leaves the model still calling it shut (it reds `DW0317` instead).
fn open_gate_prefabs() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("rest-behind-opened-door-prefabs");
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);

    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta["anchors"].as_object_mut().expect("hello-room anchors");
    for (name, shape) in [
        (
            "anchor/inner-door",
            serde_json::json!({
                "region": { "from": [1, 1, 3], "to": [9, 3, 3] },
                "block": "minecraft:iron_bars"
            }),
        ),
        // The rest point's own stand: beyond the inner door from the party's
        // spawn, and far enough from the keeper that his body does not eclipse
        // the affordance (`DW0359`).
        ("anchor/hearth", serde_json::json!({ "pos": [3, 1, 5] })),
    ] {
        assert!(
            anchors.insert(name.to_string(), shape).is_none(),
            "`{name}` is new — the library does not already declare it"
        );
    }
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn structures(plan: &Plan, prefabs_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

fn try_build_with(c: &Campaign, prefabs_dir: &Path) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let s = structures(&plan, prefabs_dir);
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// **A rest point behind a door the critical path has already opened builds.**
///
/// The bonfire is armed by the LAST objective, which stands beyond a doorway the
/// FIRST objective's `open-gate` unbars. By the time a body can rest there the door
/// has been open for a whole beat — the party walked through it to get here, and
/// `DW0311` proved that walk over this same seal model. Every quest state this seat
/// can be in force across is a state with that door open.
#[test]
fn a_rest_point_behind_a_story_opened_door_is_reachable() {
    let campaign = parse_hw(&quests_doc(
        r#"{ "type": "open-gate", "anchor": "anchor/door" }"#,
        r#"{ "type": "bonfire", "anchor": "anchor/exit" }"#,
    ));
    match try_build_with(&campaign, &common::prefabs_dir()) {
        Ok(_) => {}
        Err(emit::BuildFailure::Diagnostic { code, message }) => panic!(
            "a rest point behind a door the party is FORCED to open reads walled in only when the \
             arrivals the judgement walks forget the openings: {code}: {message}"
        ),
        Err(other) => panic!("expected a clean build, got {other:?}"),
    }
}

/// **The mirror image: a rest point the LAST beat seals in is refused.**
///
/// The direction that ships. `anchor/inner-door` stands open at world-load; the
/// bonfire is armed at the keeper's stand by the last objective, and the SAME beat
/// bars the inner door between that stand and the party's own spawn. A body that
/// rests there, dies back at the spawn and respawns at the bonfire is walled out of
/// the half of the keep its purse is lying in, forever. No arrival after that beat
/// is an objective, so a judgement keyed on objective steps alone never sees the
/// `close-gate` and calls the seat open.
///
/// The binding this rests on is stated rather than assumed: the added gate must be
/// measured **open** at world-load, or the case is the first test over again.
#[test]
fn a_rest_point_the_last_beat_seals_in_is_refused() {
    let prefabs_dir = open_gate_prefabs();
    let campaign = parse_hw(&quests_doc(
        r#"{ "type": "open-gate", "anchor": "anchor/door" }"#,
        r#"{ "type": "bonfire", "anchor": "anchor/hearth" },
                        { "type": "close-gate", "anchor": "anchor/inner-door" }"#,
    ));

    // The binding: two gates resolved, and exactly one of them authored shut.
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan, &prefabs_dir));
    let ledger = world.gate_seal_ledger();
    assert_eq!(
        ledger["gates_examined"], 2,
        "both gates resolve: {ledger:#?}"
    );
    assert_eq!(
        ledger["sealed_at_world_load"], 1,
        "`anchor/door` is barred and `anchor/inner-door` is air, so only ONE gate is a \
         world-load seal — otherwise this case is the first test over again: {ledger:#?}"
    );

    match try_build_with(&campaign, &prefabs_dir) {
        Ok(_) => panic!(
            "a bonfire whose one way home the same beat bars can strand a purse forever, and \
             this build is admitted: the arrivals past the last objective drop the \
             `close-gate`"
        ),
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            assert_eq!(
                code,
                delvec::compiler::stake::DW_STAKE_NO_ROUTE_BACK,
                "a sealed-in rest point is `DW0525`, no way back"
            );
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The optional half: what the arrivals past the last objective may credit
// ---------------------------------------------------------------------------

/// A two-quest plan whose finale depends on an errand declared `mandatory`
/// according to the argument — so the errand's objective sits on the exported
/// critical path either way, and only its forcedness differs.
fn two_quest_plan(errand_is_mandatory: bool) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {{
    "finale": "quest/open-the-door",
    "quests": [
      {{ "id": "quest/side-errand", "act": 1, "area": "area/keep", "npcs": [],
         "goal": "Look in on the Keeper's stand.", "depends_on": [],
         "mandatory": {errand_is_mandatory} }},
      {{ "id": "quest/open-the-door", "act": 1, "area": "area/keep",
         "npcs": ["npc/keeper"], "goal": "Get the Keeper to let you rest.",
         "depends_on": ["quest/side-errand"], "mandatory": true }}
    ]
  }}
}}"#
    )
}

/// The stage-5 pair: the errand opens `anchor/door`, and the bonfire behind it is
/// armed by the finale's own beat. The rest point's only way home is that door.
fn errand_opens_the_door_quests() -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{{PURSE_AND_STAKE}
    "quests": [
      {{
        "id": "quest/side-errand",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "reach-anchor", "id": "obj/errand",
             "anchor": "anchor/keeper-stand", "radius": 2 }}
        ],
        "on_objective_complete": {{
          "obj/errand": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": []
      }},
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "quest-complete", "quest": "quest/side-errand" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "bonfire", "anchor": "anchor/exit" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// **A door only OPTIONAL content opens does not free the rest point behind it.**
///
/// The arrivals past the last objective credit every objective on the exported
/// path, and `quest/side-errand` is on that path — the finale depends on it, so
/// `obj/errand` has a step and that step is in the set. The question spec-0051
/// makes worth asking is whether the party can be made to reach it, and the
/// campaign below says no: the errand is `mandatory: false`.
///
/// **The mechanism that excludes it is forcedness, and it is applied one layer
/// earlier**, where the constitution puts it: `plan::collect_region_events` drops
/// a write that does not FILL when its root is unforced (`if !write.fills() &&
/// !forced { continue; }`), and `open-gate` is `RegionWrite::Unseal`. So the
/// opening is never in `plan.region_events` at all and no ancestor relation can
/// credit it. The complement holds for the same reason: an unforced FILL is kept,
/// because crediting a wall the party may find standing is the conservative
/// reading, and the arrivals past the last objective must credit it too.
///
/// That is why this set is every objective on the path and not "every objective
/// whose quest is mandatory": forcedness has one authority, and re-deciding it
/// here would be a second one — which would also, for a fill, be the answer that
/// ships.
///
/// The pair is the element. The same campaign with the errand declared
/// `mandatory: true` builds clean, so the refusal below is about optionality and
/// not about anything else in the shape.
#[test]
fn a_door_only_optional_content_opens_does_not_free_a_rest_point() {
    let optional = parse_hw_planned(&errand_opens_the_door_quests(), &two_quest_plan(false));

    // The binding, so the refusal below is about the set this change adds and not
    // about a shape that never reaches it. The path is
    // `[select-class, obj/errand, obj/talk, assert-complete]`, and the two arrivals
    // past the last objective carry BOTH objective steps — the errand's included,
    // although the party may never play it.
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&optional, &prefabs).expect("plan builds");
    assert_eq!(
        plan.critical_path.len(),
        4,
        "select-class, the errand, the talk, and the completion assertion"
    );
    let every: BTreeSet<usize> = [1, 2].into_iter().collect();
    for arrival in [3usize, 4] {
        assert_eq!(
            plan.strict_ancestor_steps.get(&arrival),
            Some(&every),
            "arrival {arrival} carries every objective on the path"
        );
    }
    // And the opening is not in the model to be credited: `collect_region_events`
    // kept every FILL and dropped the unforced unseal, so nothing here can open
    // that door however the ancestor relation answers.
    assert!(
        plan.region_events.iter().all(|e| e.fills()),
        "an unforced `open-gate` is dropped before the seal model sees it: {:#?}",
        plan.region_events
    );

    match try_build_with(&optional, &common::prefabs_dir()) {
        Ok(_) => panic!(
            "the only `open-gate` on the rest point's door hangs off a quest nobody has to \
             play, and this build is admitted"
        ),
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            assert_eq!(
                code,
                delvec::compiler::stake::DW_STAKE_NO_ROUTE_BACK,
                "a rest point whose door only optional content opens is `DW0525`"
            );
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }

    let forced = parse_hw_planned(&errand_opens_the_door_quests(), &two_quest_plan(true));
    let forced_plan = Plan::build(&forced, &prefabs).expect("plan builds");
    assert_eq!(
        forced_plan
            .region_events
            .iter()
            .filter(|e| !e.fills())
            .count(),
        1,
        "declared mandatory, the same `open-gate` IS in the model: {:#?}",
        forced_plan.region_events
    );
    match try_build_with(&forced, &common::prefabs_dir()) {
        Ok(_) => {}
        Err(emit::BuildFailure::Diagnostic { code, message }) => panic!(
            "declaring the same errand mandatory makes the party open that door, so the rest \
             point behind it is reachable: {code}: {message}"
        ),
        Err(other) => panic!("expected a clean build, got {other:?}"),
    }
}
