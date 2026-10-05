//! spec-0088: **a killing volume live from a story stage** — `when` on
//! `lethal_volumes[]`, the volume held per quest configuration, and `DW0891`
//! judged in every configuration a body can meet it in.
//!
//! Every case here is built over one synthetic room, the **lid room**, bound into
//! the hello-world campaign in place of `hello-room`: a hall fifteen cells long
//! on a floor four courses thick, with a one-cell pit sunk into that floor at its
//! middle. The pit's top course is the lid — floor like any other until a beat
//! clears it — and the volume sits at the pit's bottom, three courses under the
//! rim, so no rim cell is in its keep-out. The party talks to the keeper at the
//! far end, which is the beat that sets the flag and clears the lid, and walks
//! to an exit on whichever side the case names.
//!
//! Variants change one thing each: the lid absent (an open pit), a strip of
//! magma across the hall's waist, a seat for a checkpoint, a world-edits batch
//! that puts the campaign on the edit-replay arm.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign};

/// The lid room's prefab id.
const ROOM: &str = "lid-room";

/// The lid room's shape, room space. Floor solid `y ∈ 0..=3`, walked at `y = 4`,
/// walls `y ∈ 4..=6`, ceiling `y = 7`.
const SIZE: [i32; 3] = [15, 8, 9];

/// The pit's column `(x, z)`: air at `y ∈ 1..=2`, the lid at `y = 3`.
const PIT: (i32, i32) = (7, 4);

/// What the room is built with — one switch per variant.
#[derive(Clone, Copy, Default)]
struct Room {
    /// No lid: the pit is open from world-load.
    open_pit: bool,
    /// Magma under the hall's waist, `x ∈ 6..=8`, every interior `z` — the floor
    /// a walk-level volume at the waist catches, shown.
    magma_waist: bool,
    /// Magma under the 3×3 round the lid, `x ∈ 6..=8`, `z ∈ 3..=5` — the floor
    /// a walk-level volume over the lid catches, shown before the flip.
    magma_lid: bool,
}

/// The lid room's non-air cells.
fn cells(room: Room) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut by: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..sy {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                if y <= 3 || y == sy - 1 || shell {
                    by.insert([x, y, z], "minecraft:stone");
                }
            }
        }
    }
    // The pit: two courses of air under the lid, and the lid itself when open.
    by.remove(&[PIT.0, 1, PIT.1]);
    by.remove(&[PIT.0, 2, PIT.1]);
    if room.open_pit {
        by.remove(&[PIT.0, 3, PIT.1]);
    }
    if room.magma_waist {
        for x in 6..=8 {
            for z in 1..sz - 1 {
                if by.contains_key(&[x, 3, z]) {
                    by.insert([x, 3, z], "minecraft:magma_block");
                }
            }
        }
    }
    if room.magma_lid {
        for x in 6..=8 {
            for z in 3..=5 {
                if by.contains_key(&[x, 3, z]) {
                    by.insert([x, 3, z], "minecraft:magma_block");
                }
            }
        }
    }
    // Light in the ceiling: the lighting gate is not what this file is about.
    for x in [3, 7, 11] {
        for z in [2, 6] {
            by.insert([x, sy - 1, z], "minecraft:glowstone");
        }
    }
    by.into_iter().collect()
}

/// The lid room's anchors.
fn anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [2, 4, 4], "facing": "east", "role": "entry" },
        "anchor/keeper-stand": { "pos": [12, 4, 6], "facing": "west" },
        "anchor/exit-west": { "pos": [2, 4, 2] },
        "anchor/exit-east": { "pos": [12, 4, 2] },
        "anchor/seat": { "pos": [2, 4, 6] },
        "anchor/pit": { "pos": [PIT.0, 1, PIT.1] },
        "anchor/lid": { "pos": [PIT.0, 3, PIT.1] },
        "anchor/lid-top": { "pos": [PIT.0, 4, PIT.1] },
        "anchor/waist": { "pos": [7, 4, 4] }
    })
}

/// A fresh directory under the test target's scratch space.
fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("staged-lethal")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One case: the room, the volume(s), the keeper's bundle, the exit, and the
/// optional extras.
struct Case {
    room: Room,
    /// The `lethal_volumes` array body.
    volumes: String,
    /// Effects appended to `obj/talk`'s bundle.
    talk: String,
    /// Effects appended to `obj/exit`'s bundle.
    exit: String,
    /// `anchor/exit-west` or `anchor/exit-east`.
    exit_anchor: &'static str,
    /// Extra stage-5 keys, spliced into `content` (a leading comma included).
    extra: String,
    /// A world-edits document, putting the campaign on the edit-replay arm.
    edits: bool,
}

impl Case {
    fn new(volumes: &str) -> Self {
        Case {
            room: Room::default(),
            volumes: volumes.to_string(),
            talk: String::new(),
            exit: String::new(),
            exit_anchor: "anchor/exit-west",
            extra: String::new(),
            edits: false,
        }
    }
    fn talk(mut self, e: &str) -> Self {
        self.talk = e.to_string();
        self
    }
    fn exit(mut self, e: &str) -> Self {
        self.exit = e.to_string();
        self
    }
    fn room(mut self, r: Room) -> Self {
        self.room = r;
        self
    }
    fn exit_at(mut self, a: &'static str) -> Self {
        self.exit_anchor = a;
        self
    }
    fn extra(mut self, e: &str) -> Self {
        self.extra = e.to_string();
        self
    }
    fn edits(mut self) -> Self {
        self.edits = true;
        self
    }

    /// Write the campaign and its one-prefab library; return both directories.
    fn write(&self, tag: &str) -> (PathBuf, PathBuf) {
        let root = scratch(tag);
        let lib = root.join("prefabs");
        common::write_single_prefab(&lib, ROOM, SIZE, &cells(self.room), anchors());
        let dir = common::campaign_bound_to(&root.join("campaign"), ROOM);
        // The cell over the lid measures dark under the ceiling's light; the
        // lighting gate is not what this file is about.
        common::patch_file(&dir.join("world.json"), |v| {
            v["content"]["areas"][0]["mitigation"] = serde_json::json!("night-vision");
        });
        let quests = format!(
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
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "{exit}",
             "radius": 1, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {talk} ],
          "obj/exit": [ {exit_fx} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "lethal_volumes": [ {volumes} ]{extra}
  }}
}}"#,
            exit = self.exit_anchor,
            talk = self.talk,
            exit_fx = self.exit,
            volumes = self.volumes,
            extra = self.extra,
        );
        std::fs::write(dir.join("quests.json"), quests).unwrap();
        if self.edits {
            std::fs::write(dir.join("world-edits.json"), one_batch()).unwrap();
        }
        common::declare_story_dir(&dir);
        (dir, lib)
    }
}

/// The smallest world-edits document that puts the campaign on the
/// edit-replay arm: a floor patch re-dressed in cobblestone, a full cube for a
/// full cube, so no proof's geometry moves.
fn one_batch() -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "world-edits",
  "content": {{
    "batches": [
      {{
        "id": "batch/dress-a-corner",
        "area": "area/keep",
        "note": "a floor patch, so this campaign takes the edit-replay arm",
        "edits": [
          {{ "verb": "select", "name": "region/corner",
             "shape": {{ "kind": "box",
               "frame": {{ "kind": "piece-local", "piece": 0, "prefab": "prefab/{ROOM}" }},
               "min": [1, 3, 1], "max": [2, 3, 2] }} }},
          {{ "verb": "replace", "region": "region/corner",
             "matching": ["minecraft:stone"],
             "recipe": {{ "blocks": [{{ "block": "minecraft:cobblestone", "weight": 1.0 }}] }} }}
        ]
      }}
    ]
  }}
}}"#
    )
}

/// The campaign at `dir`, parsed and tagged as `delvec build` does.
fn campaign(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the case parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

/// Validation diagnostics for the case at `dir`.
fn validate(dir: &Path, lib: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    delvewright_dsl::validate_campaign_with(
        &c,
        &delvec::compiler::registry::FullItemRegistry::v1_21_11(),
        &prefabs,
        &delvec::compiler::registry::FullEntityRegistry::v1_21_11(),
    )
}

/// The structures a plan needs, read from the case's library.
fn structures(plan: &Plan, lib: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                out.insert(
                    t.structure_file.clone(),
                    std::fs::read(lib.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    out
}

/// Build the case through the real `emit::build` path, with its warnings.
fn try_build(dir: &Path, lib: &Path) -> Result<(BuildOutput, Vec<String>), BuildFailure> {
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let s = structures(&plan, lib);
    let inputs = load_campaign_dir(dir).unwrap().inputs;
    emit::build_with_warnings(
        &plan,
        &inputs,
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map(|(out, warnings)| {
        (
            out,
            warnings
                .iter()
                .map(|w| format!("{}: {}", w.code, w.message))
                .collect(),
        )
    })
}

fn build(case: &Case, tag: &str) -> (BuildOutput, Vec<String>) {
    let (dir, lib) = case.write(tag);
    match try_build(&dir, &lib) {
        Ok(x) => x,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("expected the build to succeed, got {code}: {message}")
        }
        Err(e) => panic!("expected the build to succeed, got {e:?}"),
    }
}

/// The refusal a case is built into: `(code, message)`.
fn refusal(case: &Case, tag: &str) -> (String, String) {
    let (dir, lib) = case.write(tag);
    match try_build(&dir, &lib) {
        Ok(_) => panic!("expected the build to be refused"),
        Err(BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            (code.to_string(), message)
        }
        Err(e) => panic!("expected a diagnostic, got {e:?}"),
    }
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "`{path}` is emitted; have {:#?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

fn json(out: &BuildOutput, path: &str) -> serde_json::Value {
    serde_json::from_str(&text(out, path)).unwrap()
}

// --- the volumes ----------------------------------------------------------

/// The pit at the bottom of the lid, live from the beat that clears it.
const PIT_STAGED: &str = r#"{
  "id": "lethal/the-pit",
  "region": { "anchor": "anchor/pit", "extent": [0, 0, 0] },
  "message": "The floor was a lid, and the pit under it has no bottom you would survive.",
  "damage_type": "fall",
  "when": { "requires_flags": ["flag/lid-fell"] }
}"#;

/// The same pit, live from world-load.
const PIT_ALWAYS: &str = r#"{
  "id": "lethal/the-pit",
  "region": { "anchor": "anchor/pit", "extent": [0, 0, 0] },
  "message": "The floor was a lid, and the pit under it has no bottom you would survive.",
  "damage_type": "fall"
}"#;

/// The beat that drops the lid: the flag and the clear, in one bundle.
const DROP_LID: &str = r#"{ "type": "set-flag", "flag": "flag/lid-fell" },
  { "type": "clear-region", "region": { "anchor": "anchor/lid", "extent": [0, 0, 0] } }"#;

/// The flag alone.
const SET_FLAG: &str = r#"{ "type": "set-flag", "flag": "flag/lid-fell" }"#;

/// The clear alone.
const CLEAR_LID: &str =
    r#"{ "type": "clear-region", "region": { "anchor": "anchor/lid", "extent": [0, 0, 0] } }"#;

// --- AC1: the surface ------------------------------------------------------

/// `when` on a lethal volume is the same `Guard` an effect's `when` is, in the
/// exported schema.
#[test]
fn the_schema_exports_when_as_the_one_guard() {
    let schema = delvewright_dsl::stage_schema(delvewright_dsl::envelope::Stage::Quests);
    let defs = &schema["$defs"];
    let lv = &defs["LethalVolume"]["properties"]["when"];
    let as_text = lv.to_string();
    assert!(
        as_text.contains("#/$defs/Guard"),
        "LethalVolume.when must reference the shared Guard: {lv}"
    );
    // The effect's `when` names the same definition.
    let qe = defs["QuestEffect"].to_string();
    assert!(
        qe.contains("#/$defs/Guard"),
        "QuestEffect carries the Guard"
    );
    assert!(defs["Guard"]["properties"]["requires_state"].is_object());
}

/// A campaign with one staged volume builds green, and its binding reports two
/// configurations for it.
#[test]
fn a_staged_pit_builds_green_and_is_judged_in_two_configurations() {
    let (out, warnings) = build(&Case::new(PIT_STAGED).talk(DROP_LID), "green");
    let gate = json(&out, "validation/lethal-gate.json");
    eprintln!("{}", serde_json::to_string_pretty(&gate["staged"]).unwrap());
    let staged = gate["staged"].as_array().unwrap();
    assert_eq!(staged.len(), 1, "{gate}");
    assert_eq!(staged[0]["configurations"]["judged"], 2, "{gate}");
    let rows = gate["danger_visibility"]["volumes"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "K = 2: {gate}");
    assert_eq!(gate["danger_visibility"]["pairs"], 2);
    let before = &rows[0];
    let after = &rows[1];
    assert_eq!(before["live"], false, "{before}");
    assert!(
        before["reached_by"].is_null(),
        "before the flip nothing reaches the pit: {before}"
    );
    assert_eq!(after["live"], true, "{after}");
    let by = after["reached_by"].as_str().unwrap_or_default();
    assert!(
        by.contains("fall"),
        "after the flip a fall reaches it: {after}"
    );
    assert!(
        !warnings.iter().any(|w| w.starts_with("DW0954")),
        "the forced route arms it: {warnings:?}"
    );
}

// --- AC2: the eighth consumer ---------------------------------------------

/// The volume is a gate consumer, visited by the one walk and counted by its
/// binding ledger.
#[test]
fn the_volume_is_the_eighth_gate_consumer() {
    use delvewright_dsl::gate::{GateConsumer, for_each_gate};
    assert!(GateConsumer::ALL.contains(&GateConsumer::LethalVolume));
    let (dir, _) = Case::new(&format!("{PIT_STAGED}, {PIT_ALWAYS_TWO}"))
        .talk(DROP_LID)
        .write("consumer");
    let c = campaign(&dir);
    let mut seen: Vec<String> = Vec::new();
    let binding = for_each_gate(&c, &mut |site, gate| {
        if site.consumer == GateConsumer::LethalVolume {
            seen.push(format!("{} {}", site.path, gate.terms()));
        }
    });
    assert_eq!(
        seen,
        vec![
            "/content/lethal_volumes/0/when 1".to_string(),
            "/content/lethal_volumes/1/when 0".to_string()
        ],
        "every volume is visited, staged or not, at its `when`"
    );
    assert!(
        binding.summary().contains("lethal volume=2"),
        "{}",
        binding.summary()
    );
}

/// A second volume live from world-load, in the room's far corner.
const PIT_ALWAYS_TWO: &str = r#"{
  "id": "lethal/the-corner",
  "region": { "anchor": "anchor/pit", "extent": [0, 0, 0] },
  "message": "The corner has no floor."
}"#;

// --- AC3: the document arm --------------------------------------------------

/// `when: {}` is not a stage, and a term on a `player` datum is not a fact about
/// the place: both `DW0953`, from validation alone.
#[test]
fn a_gate_that_cannot_stage_a_volume_is_dw0953_at_the_document() {
    let empty = PIT_STAGED.replace(r#"{ "requires_flags": ["flag/lid-fell"] }"#, "{}");
    assert!(empty.contains(r#""when": {}"#), "{empty}");
    let (dir, lib) = Case::new(&empty).talk(DROP_LID).write("dw0953-empty");
    let d = validate(&dir, &lib);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0953")
        .unwrap_or_else(|| panic!("`when: {{}}` is DW0953: {d:#?}"));
    assert!(hit.message.contains("lethal/the-pit"), "{}", hit.message);
    assert_eq!(hit.path, "/content/lethal_volumes/0/when");

    let player = PIT_STAGED.replace(
        r#"{ "requires_flags": ["flag/lid-fell"] }"#,
        r#"{ "requires_state": [{ "state": "state/nerve", "op": "at-least", "value": 1 }] }"#,
    );
    let state = r#", "state": [{ "id": "state/nerve", "initial": 0, "scope": "player",
        "note": "a datum each player holds" }]"#;
    let (dir, lib) = Case::new(&player)
        .talk(&format!(
            r#"{DROP_LID}, {{ "type": "set-state", "state": "state/nerve", "value": 1 }}"#
        ))
        .extra(state)
        .write("dw0953-player");
    let d = validate(&dir, &lib);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0953")
        .unwrap_or_else(|| panic!("a player-scoped term is DW0953: {d:#?}"));
    assert!(
        hit.message.contains("lethal/the-pit") && hit.message.contains("state/nerve"),
        "names the volume and the term: {}",
        hit.message
    );
    assert!(
        !d.iter().any(|x| x.code == "DW0503"),
        "one fault, one code: {d:#?}"
    );
}

// --- AC5: the world per configuration --------------------------------------

/// A volume across the hall's waist, at the walk plane, over magma.
const WAIST: &str = r#"{
  "id": "lethal/the-waist",
  "region": { "anchor": "anchor/waist", "extent": [0, 0, 3] },
  "message": "The floor across the hall's waist is molten stone.",
  "damage_type": "fire",
  "shown_by": ["minecraft:magma_block"],
  "when": { "requires_flags": ["flag/lid-fell"] }
}"#;

fn waist_room() -> Room {
    Room {
        magma_waist: true,
        ..Room::default()
    }
}

/// The volume's cells on the leg from the keeper to the west exit: crossed
/// while the volume is dead, refused once it has woken — moved by nothing but
/// where the `set-flag` fires.
#[test]
fn a_leg_before_the_flip_crosses_the_volume_and_after_it_does_not() {
    // Armed at the exit: the keeper-to-exit leg is walked before the flip.
    let (dir, lib) = Case::new(WAIST)
        .room(waist_room())
        .exit(SET_FLAG)
        .write("leg-before");
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan, &lib));
    let routes = delvec::compiler::nav::critical_path_routes(&plan, &world);
    let waist = |p: &[i32; 3]| p[0] == plan.anchors.values().map(|_| 0).sum::<i32>() + 7;
    let _ = waist;
    let crosses: Vec<usize> = routes
        .iter()
        .filter(|r| r.cells.iter().any(|c| plan.lethal_volumes[0].contains(*c)))
        .map(|r| r.to_step)
        .collect();
    assert!(
        crosses.contains(&2),
        "the leg to the exit (step 2) crosses the dead volume's cells: {crosses:?}"
    );
    assert!(try_build(&dir, &lib).is_ok(), "and the build is green");

    // Armed at the keeper: the same leg is walked after the flip, and the waist
    // spans the hall, so there is no way round.
    let (code, msg) = refusal(
        &Case::new(WAIST).room(waist_room()).talk(SET_FLAG),
        "leg-after",
    );
    assert_eq!(code, "DW0510", "{msg}");
    assert!(msg.contains("lethal/the-waist"), "{msg}");
    assert!(
        msg.contains("critical step 2") && msg.contains("requires `flag/lid-fell`"),
        "names the configuration and its terms: {msg}"
    );
}

/// `Premises::of_plan` carries only the unstaged volumes: a staged volume
/// reaches a world through the region state and no other door.
#[test]
fn the_premises_carry_only_unstaged_volumes() {
    let premises_of = |volume: &str, tag: &str| {
        let (dir, lib) = Case::new(volume).talk(DROP_LID).write(tag);
        let c = campaign(&dir);
        let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
        let plan = Plan::build(&c, &prefabs).unwrap();
        let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan, &lib));
        (
            format!(
                "{:?}",
                delvec::compiler::nav::Premises::of_plan(&plan, Vec::new())
            ),
            world.lethal_cells(),
        )
    };
    let (staged, staged_cells) = premises_of(PIT_STAGED, "premises-staged");
    let (always, always_cells) = premises_of(PIT_ALWAYS, "premises-always");
    assert!(staged.contains("lethal_regions: []"), "{staged}");
    assert!(staged.contains("StagedVolume"), "{staged}");
    assert!(!always.contains("lethal_regions: []"), "{always}");
    assert!(always.contains("staged_lethal: []"), "{always}");
    assert_eq!((staged_cells, always_cells), (0, 1));
}

// --- AC6: every consumer ---------------------------------------------------

/// A checkpoint seated before the flip whose only way on crosses the volume
/// once it wakes is refused; the same seat with the volume live from world-load
/// somewhere it blocks nothing is not.
#[test]
fn a_seat_cut_off_by_a_volume_that_wakes_is_refused() {
    let seat = r#", { "type": "set-checkpoint", "anchor": "anchor/seat" }"#;
    let (code, msg) = refusal(
        &Case::new(WAIST)
            .room(waist_room())
            .talk(&format!("{SET_FLAG}{seat}"))
            .exit_at("anchor/exit-east"),
        "seat-cut",
    );
    assert!(code == "DW0315" || code == "DW0316", "{code}: {msg}");
    build(
        &Case::new(PIT_ALWAYS)
            .talk(&format!("{CLEAR_LID}{seat}"))
            .exit_at("anchor/exit-east"),
        "seat-free",
    );
}

/// A pocket that exists only while the volume is dead is named with its state:
/// the open pit, armed at the last beat, is a hole a body falls into and cannot
/// climb out of until then; live from world-load, the same hole kills, and
/// there is no pocket.
#[test]
fn a_pocket_only_while_the_volume_is_dead_names_its_state() {
    let open = Room {
        open_pit: true,
        ..Room::default()
    };
    let (code, msg) = refusal(
        &Case::new(PIT_STAGED).room(open).exit(SET_FLAG),
        "pocket-dead",
    );
    assert_eq!(code, "DW0921", "{msg}");
    assert!(msg.contains("lethal volume `lethal/the-pit` dead"), "{msg}");
    build(&Case::new(PIT_ALWAYS).room(open), "pocket-live");
}

/// The world-edits replay reaches the same verdicts as `emit::build`'s own arm.
#[test]
fn the_edit_replay_reaches_the_same_verdicts() {
    build(&Case::new(PIT_STAGED).talk(DROP_LID).edits(), "edits-green");
    let (code, msg) = refusal(
        &Case::new(WAIST).room(waist_room()).talk(SET_FLAG).edits(),
        "edits-dw0510",
    );
    assert_eq!(code, "DW0510", "{msg}");
    let (code, _) = refusal(
        &Case::new(&raised(PIT_STAGED)).talk(DROP_LID).edits(),
        "edits-dw0891",
    );
    assert_eq!(code, "DW0891");
}

// --- AC7: danger is visible, per configuration ------------------------------

/// The staged pit's volume raised to the walk plane over the lid.
fn raised(volume: &str) -> String {
    volume.replace(r#""anchor": "anchor/pit""#, r#""anchor": "anchor/lid-top""#)
}

/// Raised to the floor course, the volume catches the floor round the lid
/// before the flip, in a configuration whose bytes show nothing: the fourth
/// shape, naming that configuration and the cells by floor, and never telling
/// the author to mark the floor unwalkable.
#[test]
fn a_volume_raised_to_the_floor_is_the_fourth_shape_before_the_flip() {
    let (code, msg) = refusal(&Case::new(&raised(PIT_STAGED)).talk(DROP_LID), "raised");
    assert_eq!(code, "DW0891", "{msg}");
    assert!(msg.contains("goes live at critical step 2"), "{msg}");
    assert!(
        msg.contains("the configuration arriving at critical step 0, before its gate"),
        "names the configuration before: {msg}"
    );
    assert!(msg.contains("y=68 (9 cell(s)"), "the cells by floor: {msg}");
    assert!(!msg.contains("unwalkable"), "{msg}");

    // The same floor authored in magma, and declared: green.
    let shown = raised(PIT_STAGED).replace(
        r#""damage_type": "fall","#,
        r#""damage_type": "fall", "shown_by": ["minecraft:magma_block"],"#,
    );
    build(
        &Case::new(&shown)
            .room(Room {
                magma_lid: true,
                ..Room::default()
            })
            .talk(DROP_LID),
        "raised-shown",
    );
}

/// The `shown` reading takes a configuration's bytes: magma laid by a
/// `fill-region` in the arming bundle shows the floor in the live configuration
/// and not in the one before it — where the volume is refused.
#[test]
fn shown_is_read_in_the_configurations_own_bytes() {
    let shown = raised(PIT_STAGED).replace(
        r#""damage_type": "fall","#,
        r#""damage_type": "fall", "shown_by": ["minecraft:magma_block"],"#,
    );
    let lay = r#"{ "type": "set-flag", "flag": "flag/lid-fell" },
      { "type": "fill-region", "region": { "anchor": "anchor/lid", "extent": [1, 0, 1] },
        "block": "minecraft:magma_block" }"#;
    let (dir, lib) = Case::new(&shown).talk(lay).write("shown-bytes");
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let s = structures(&plan, &lib);
    let assembled = delvec::compiler::assembled::assemble(&plan, &s);
    let world = delvec::compiler::nav::World::from_plan(&plan, &s);
    let entry = plan.entry_point("area/keep");
    let (binding, verdict) =
        delvec::compiler::lethal::check_danger_is_visible(&plan, &world, &assembled.blocks, entry);
    let rows: Vec<(bool, usize, usize)> = binding
        .volumes
        .iter()
        .map(|r| (r.live, r.caught.len(), r.shown.len()))
        .collect();
    eprintln!("{}\n{rows:?}", binding.line());
    let before = binding
        .volumes
        .iter()
        .find(|r| !r.live)
        .expect("a before row");
    let after = binding.volumes.iter().find(|r| r.live).expect("a live row");
    assert!(
        !before.caught.is_empty() && before.shown.is_empty(),
        "{rows:?}"
    );
    assert!(!after.shown.is_empty(), "{rows:?}");
    let err = verdict.expect_err("refused before the flip");
    assert_eq!(err.code.id(), "DW0891");
    assert!(err.message.contains("goes live at"), "{}", err.message);
}

// --- AC8: the population is the lethality-free one --------------------------

/// Over the live configuration's lethal-applied world the waist catches
/// nothing — the router refuses its keep-out — and over the counterfactual it
/// catches the magma floor. The check reads the second.
#[test]
fn the_population_is_the_lethality_free_one_in_the_live_configuration() {
    let (dir, lib) = Case::new(WAIST)
        .room(waist_room())
        .talk(SET_FLAG)
        .exit_at("anchor/exit-east")
        .write("population");
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan, &lib));
    let (configs, _) = delvec::compiler::nav::path_configurations(&plan, &world);
    let live = configs
        .iter()
        .find(|c| c.live.first().is_some_and(|l| l.may))
        .expect("a live configuration");
    let applied = live.world(&world).expect("the live configuration writes");
    let roots = delvec::compiler::lethal::population_roots(&plan, plan.entry_point("area/keep"));
    let (klo, khi) = delvewright_dsl::metrics::keep_out_box(
        delvewright_dsl::metrics::Body::PLAYER,
        plan.lethal_volumes[0].region.0,
        plan.lethal_volumes[0].region.1,
    );
    let caught = |w: &delvec::compiler::nav::World| {
        w.reachable_walkable(&roots)
            .into_iter()
            .filter(|c| (0..3).all(|i| klo[i] <= c[i] && c[i] <= khi[i]))
            .count()
    };
    let over_applied = caught(&applied);
    let over_open = caught(&applied.without_exclusions());
    eprintln!(
        "caught: {over_applied} over the lethal-applied world, {over_open} over the counterfactual"
    );
    assert_eq!(over_applied, 0);
    assert!(over_open > 0);
}

// --- AC9: posted places and waves ------------------------------------------

/// An NPC posted in a volume dead at world-load is refused as if it were live.
#[test]
fn an_npc_posted_in_a_staged_volume_is_dw0511_as_live() {
    let at_keeper = PIT_STAGED.replace(
        r#""anchor": "anchor/pit""#,
        r#""anchor": "anchor/keeper-stand""#,
    );
    let (code, msg) = refusal(&Case::new(&at_keeper).exit(SET_FLAG), "post");
    assert_eq!(code, "DW0511", "{msg}");
    assert!(
        msg.contains("is live from a story stage; the body is judged against it as live"),
        "{msg}"
    );
}

/// A wave seated beside a staged pit whose members can drop into it is refused
/// as if the pit were live.
#[test]
fn a_wave_that_can_drop_into_a_staged_pit_is_dw0922_as_live() {
    let open = Room {
        open_pit: true,
        ..Room::default()
    };
    let wave = r#", "waves": [{ "id": "wave/sentries", "anchor": "anchor/lid-top",
        "mobs": [{ "entity": "minecraft:zombie", "count": 2, "name": "Sentry" }] }]"#;
    let spawn = r#"{ "type": "spawn-wave", "wave": "wave/sentries" }"#;
    let (code, msg) = refusal(
        &Case::new(PIT_STAGED)
            .room(open)
            .talk(spawn)
            .exit(SET_FLAG)
            .extra(wave),
        "wave",
    );
    assert_eq!(code, "DW0922", "{msg}");
    assert!(msg.contains("live from a story stage"), "{msg}");
}

// --- AC10: emission and PackTest -------------------------------------------

/// A staged volume ticks through its guard; an unstaged one is driven as it
/// always was; the guard's line is the one reduction rendered by the one
/// formatter; the shut template exists.
#[test]
fn a_staged_volume_is_emitted_behind_its_gate() {
    let (out, _) = build(
        &Case::new(&format!("{PIT_STAGED}, {PIT_ALWAYS_TWO}")).talk(DROP_LID),
        "emission",
    );
    let ns = "hello-world";
    let tick = text(
        &out,
        &format!("datapack/data/{ns}/function/tick.mcfunction"),
    );
    assert!(
        tick.contains(&format!("function {ns}:lethal_the_pit_tick\n")),
        "{tick}"
    );
    assert!(
        tick.contains(&format!("function {ns}:lethal_the_corner\n")),
        "{tick}"
    );
    assert!(
        !tick.contains(&format!("function {ns}:lethal_the_pit\n")),
        "{tick}"
    );
    let guard = text(
        &out,
        &format!("datapack/data/{ns}/function/lethal_the_pit_tick.mcfunction"),
    );
    let (dir, lib) = Case::new(PIT_STAGED).talk(DROP_LID).write("emission-terms");
    let c = campaign(&dir);
    let plan = Plan::build(&c, &PrefabRegistry::load_dir(&lib).unwrap()).unwrap();
    let terms: Vec<String> = plan
        .gate_terms(c.quests.content.lethal_volumes[0].gate())
        .iter()
        .map(|t| t.clause(false))
        .collect();
    assert_eq!(
        guard.trim_end(),
        format!(
            "execute {} run function {ns}:lethal_the_pit",
            terms.join(" ")
        ),
        "the guard is gate_terms rendered by clause(false)"
    );
    assert_eq!(
        guard.trim_end(),
        format!("execute if score #party dw.f_lid_fell matches 1 run function {ns}:lethal_the_pit")
    );
    let open = text(
        &out,
        &format!("packtest-datapack/data/{ns}/test/lethal_the_pit.mcfunction"),
    );
    assert!(
        open.contains("scoreboard players set #party dw.f_lid_fell 1"),
        "{open}"
    );
    assert!(
        open.contains(&format!("function {ns}:lethal_the_pit_tick\n")),
        "{open}"
    );
    let shut = text(
        &out,
        &format!("packtest-datapack/data/{ns}/test/lethal_the_pit_shut.mcfunction"),
    );
    assert!(
        shut.contains("scoreboard players reset #party dw.f_lid_fell"),
        "{shut}"
    );
    assert!(
        shut.contains(&format!("function {ns}:lethal_the_pit_tick\n")),
        "{shut}"
    );
    assert!(
        shut.contains("assert score #hp_lshut dw.sys matches 20"),
        "{shut}"
    );
    assert!(
        shut.contains("assert score #in_lshut dw.sys matches 1"),
        "{shut}"
    );
    assert!(
        !out.keys()
            .any(|k| k.ends_with("lethal_the_corner_shut.mcfunction")),
        "an unstaged volume has no shut template"
    );
    let plan_json = json(&out, "validation/death-plan.json");
    assert_eq!(plan_json["format_version"], 4);
    let rows = plan_json["lethal_volumes"].as_array().unwrap();
    assert_eq!(rows[0]["gate"]["terms"][0]["objective"], "dw.f_lid_fell");
    assert_eq!(rows[1]["gate"]["terms"], serde_json::json!([]));
}

// --- AC12: the advisory ------------------------------------------------------

/// A staged volume whose flag only a shop offer sets is never live on the
/// forced route: `DW0954`, naming the volume and the term. Set by a forced
/// objective, it is not.
#[test]
fn a_volume_only_a_purchase_arms_is_dw0954() {
    let shop = r#", "shops": [{ "id": "shop/counter", "anchor": "anchor/seat", "title": "The Counter",
        "marker_item": "minecraft:emerald",
        "offers": [{ "label": "Pull the lid", "effects": [
          { "type": "set-flag", "flag": "flag/lid-fell" },
          { "type": "clear-region", "region": { "anchor": "anchor/lid", "extent": [0, 0, 0] } }
        ] }] }]"#;
    let (_, warnings) = build(&Case::new(PIT_STAGED).extra(shop), "shop");
    let w = warnings
        .iter()
        .find(|w| w.starts_with("DW0954"))
        .unwrap_or_else(|| panic!("DW0954 among {warnings:#?}"));
    assert!(
        w.contains("lethal/the-pit") && w.contains("flag/lid-fell"),
        "{w}"
    );
    let (_, warnings) = build(&Case::new(PIT_STAGED).talk(DROP_LID), "shop-forced");
    assert!(
        !warnings.iter().any(|w| w.starts_with("DW0954")),
        "{warnings:#?}"
    );
}

// --- AC13: the binding line and the ledger ---------------------------------

/// The line states N, S, C and K from the objects; the ledger carries the
/// staged rows and one danger row per pair.
#[test]
fn the_binding_line_counts_volumes_configurations_and_pairs() {
    let (dir, lib) = Case::new(&format!("{PIT_STAGED}, {PIT_ALWAYS_TWO}"))
        .talk(DROP_LID)
        .write("binding");
    let c = campaign(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).unwrap();
    let s = structures(&plan, &lib);
    let assembled = delvec::compiler::assembled::assemble(&plan, &s);
    let world = delvec::compiler::nav::World::from_plan(&plan, &s);
    let (binding, verdict) = delvec::compiler::lethal::check_danger_is_visible(
        &plan,
        &world,
        &assembled.blocks,
        plan.entry_point("area/keep"),
    );
    verdict.unwrap();
    let line = binding.line();
    eprintln!("{line}");
    assert!(
        line.contains("2 volume(s), 1 staged; judged over 3 configuration(s) as 3 (volume, configuration) pair(s)"),
        "{line}"
    );
}

/// A staged volume the path can never meet live or arm — its gate reads a
/// party datum nothing writes, held below the value it asks for — is judged
/// in no configuration, and that is a red, not a line.
#[test]
fn a_staged_volume_judged_in_no_configuration_is_red() {
    let never = PIT_STAGED.replace(
        r#"{ "requires_flags": ["flag/lid-fell"] }"#,
        r#"{ "requires_state": [{ "state": "state/depth", "op": "at-least", "value": 5 }] }"#,
    );
    let state = r#", "state": [{ "id": "state/depth", "initial": 0, "scope": "party",
        "note": "how deep the floor has sunk" }]"#;
    let (code, msg) = refusal(&Case::new(&never).talk(CLEAR_LID).extra(state), "zero");
    assert_eq!(code, "DW0891", "{msg}");
    assert!(msg.contains("judged it in none"), "{msg}");
}
