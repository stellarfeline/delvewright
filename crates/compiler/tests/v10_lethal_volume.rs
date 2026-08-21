//! DSL v0.10 lethal volumes (spec-0031): what the compiler emits, what the
//! completability proof refuses, and the runtime half the PackTest suite carries.
//!
//! The `hello-room` prefab is the fixture for every case here because its geometry
//! is a corridor with exactly one doorway (`anchor/door`, the gate region
//! `[4,1,6]..[5,3,6]`), which makes "the only route runs through the volume" a
//! two-line declaration rather than a synthetic world.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A hello-world `quests` doc with the given `lethal_volumes` body and an optional
/// extra `on_objective_complete` effect for `obj/talk`.
fn quests_doc(volumes: &str, talk_effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.10.0",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{talk_effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "lethal_volumes": [ {volumes} ]
  }}
}}"#
    )
}

/// Parse, then TAG the campaign's player-visible strings exactly as `delvec build`
/// does (`main.rs`, spec-0029 i18n v2). Without the tag every emitter lowers a
/// bare `{"text": …}` literal, so an untagged test would silently stop proving
/// that the death wording travels as a translatable component.
fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
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

fn try_build(c: &Campaign) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let s = structures(&plan);
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).expect("build succeeds")
}

fn failure_code(c: &Campaign) -> String {
    match try_build(c) {
        Ok(_) => panic!("expected the build to fail"),
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            code.to_string()
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
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

/// A volume off to one side of the keep, touching nothing the party must walk.
const HARMLESS: &str = r#"{
  "id": "lethal/the-drop",
  "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
  "message": "The undertow takes you.",
  "damage_type": "fall"
}"#;

// --- emission -------------------------------------------------------------

/// The volume emits a tick driver and the two-line body: players re-bound one at a
/// time so the wording reaches the one it is about, everything else in one
/// `/damage` minus the engine's own machinery.
#[test]
fn a_volume_emits_a_tick_driver_and_a_killing_body() {
    // On the exit cell: clear of the corridor, clear of the Keeper's post, and
    // reachable-by-radius, so this test is about emission alone.
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/pit",
             "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "message": "The pit takes you.", "damage_type": "fire" }"#,
        "",
    ));
    let out = build(&c);
    let tick = text(&out, "datapack/data/hello-world/function/tick.mcfunction");
    assert!(
        tick.contains("function hello-world:lethal_pit"),
        "the volume is driven every tick: {tick}"
    );

    let body = text(
        &out,
        "datapack/data/hello-world/function/lethal_pit.mcfunction",
    );
    assert!(
        body.contains("execute as @a[") && body.contains("tag=!dw_cutscene"),
        "players are re-bound and a cutscene watcher is never harmed: {body}"
    );
    assert!(
        body.contains("run function hello-world:lethal_pit_kill"),
        "the per-player half runs the volume's own kill function: {body}"
    );
    // Every engine-machinery type is excluded from the entity sweep — a volume
    // over a cutscene dolly must not erase the camera.
    for t in [
        "type=!minecraft:interaction",
        "type=!minecraft:marker",
        "type=!minecraft:item_display",
        "type=!minecraft:block_display",
        "type=!minecraft:text_display",
        "type=!minecraft:player",
    ] {
        assert!(body.contains(t), "the entity sweep excludes {t}: {body}");
    }

    let kill = text(
        &out,
        "datapack/data/hello-world/function/lethal_pit_kill.mcfunction",
    );
    assert!(
        kill.contains("tellraw @s") && kill.contains("The pit takes you."),
        "the wording reaches the player who died: {kill}"
    );
    // The wording is a CONSEQUENCE of the blow, never a prediction of it, and
    // this is the assertion that pins it: PackTest cannot read chat, so the
    // runtime template can only show the guard's condition coming out false —
    // that the message is actually conditioned on it is checked here.
    //
    // Measured on the pinned 1.21.11 toolserver, after getting it wrong twice:
    // `/damage` reports SUCCESS while doing nothing (a respawned player is
    // invulnerable for 59 ticks), so `execute store success` is inert; the guard
    // must read the outcome. A dummy at `Health: 20f` in a swinging volume was
    // still at `Health: 20f` after 202 ticks with `/damage` answering success
    // every tick.
    assert!(
        kill.contains("execute if score #leth_hp dw.sys matches ..0 run tellraw @s"),
        "the wording is conditioned on the player actually ending up dead — an \
         unconditional `tellraw` prints once per tick for three seconds after any \
         respawn, about a death that is not happening: {kill}"
    );
    assert!(
        !kill.contains("store success"),
        "the guard must not read `/damage`'s own result, which reports success even \
         when it does nothing (measured): {kill}"
    );
    // The blow must come first; a guard evaluated before the damage reads the
    // health the player had on the way in.
    let dmg_at = kill.find("damage @s").expect("the blow is emitted");
    let guard_at = kill.find("execute if score").expect("the guard is emitted");
    assert!(
        dmg_at < guard_at,
        "the blow precedes the guard that reads its outcome: {kill}"
    );
    // Delivered as a text component, which is what makes it translatable AND
    // readable by a player who declined the resource pack (spec-0029 §3).
    assert!(
        kill.contains("\"translate\"") && kill.contains("\"fallback\""),
        "the wording is a translate+fallback component, not a bare literal: {kill}"
    );
    // The declared damage type words vanilla's own broadcast.
    assert!(
        kill.contains("damage @s 1000 minecraft:on_fire"),
        "the kill uses the declared damage type: {kill}"
    );
}

/// A campaign that declares no volume emits nothing new — no tick line, no
/// function, no ledger. This is the byte-identity claim in its smallest form.
#[test]
fn no_volume_emits_nothing() {
    let c = parse_hw(&quests_doc("", ""));
    let out = build(&c);
    assert!(
        !out.keys().any(|k| k.contains("lethal")),
        "a campaign with no lethal volume emits no lethal artifact: {:#?}",
        out.keys().collect::<Vec<_>>()
    );
}

// --- DW0510: the completability proof knows about it -----------------------

/// The keep has exactly one doorway. A volume across it leaves the party no route
/// to the exit objective, and the build fails naming the volume — not with a
/// reachability complaint about geometry that is perfectly walkable.
#[test]
fn a_volume_across_the_only_route_is_dw0510() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/the-threshold",
             "region": { "anchor": "anchor/door", "extent": [3, 3, 0] },
             "message": "The threshold burns." }"#,
        "",
    ));
    assert_eq!(failure_code(&c), "DW0510");
}

/// The same proof, one step earlier: an objective whose only footing lies inside a
/// volume is a player killed by standing where the objective is.
#[test]
fn an_objective_buried_in_a_volume_is_dw0510() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/the-exit",
             "region": { "anchor": "anchor/exit", "extent": [6, 6, 6] },
             "message": "Nothing here is survivable." }"#,
        "",
    ));
    assert_eq!(failure_code(&c), "DW0510");
}

// --- DW0511: the death loop routing cannot see -----------------------------

/// A respawn seat inside a volume routes perfectly and kills the party on arrival,
/// forever. The seat is reached by teleport, so no reachability proof can see it.
#[test]
fn a_checkpoint_inside_a_volume_is_dw0511() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/on-the-seat",
             "region": { "anchor": "anchor/keeper-stand", "extent": [0, 0, 0] },
             "message": "You wake up dying." }"#,
        r#", { "type": "set-checkpoint", "anchor": "anchor/keeper-stand" }"#,
    ));
    assert_eq!(failure_code(&c), "DW0511");
}

/// The same code, the second family it covers: an NPC posted inside a volume is
/// deleted on the first tick — the volume's entity sweep exempts the engine's own
/// machinery and deliberately not content bodies — and no route proof can see it,
/// because a post is a declaration and not a walk. Found while writing this
/// feature's own CI fixture.
#[test]
fn an_npc_posted_inside_a_volume_is_dw0511() {
    let c = parse_hw(&quests_doc(
        r#"{ "id": "lethal/on-the-keeper",
             "region": { "anchor": "anchor/keeper-stand", "extent": [0, 0, 0] },
             "message": "The floor was never there." }"#,
        "",
    ));
    assert_eq!(failure_code(&c), "DW0511");
}

// --- the runtime half + the binding ledger ---------------------------------

/// One PackTest template per volume, and it really puts an entity in the box: the
/// template asserts the dummy is inside the volume's own selector before it asserts
/// the volume killed it, so a template that bound to nothing cannot pass.
#[test]
fn each_volume_gets_a_packtest_that_binds() {
    let c = parse_hw(&quests_doc(HARMLESS, ""));
    let out = build(&c);
    let t = text(
        &out,
        "packtest-datapack/data/hello-world/test/lethal_the_drop.mcfunction",
    );
    assert!(
        t.contains("summon minecraft:zombie"),
        "the template puts a real entity in the volume: {t}"
    );
    assert!(
        t.contains("assert score #in_leth dw.sys matches 1"),
        "the template proves its dummy is INSIDE the volume before asserting the kill: {t}"
    );
    assert!(
        t.contains("function hello-world:lethal_the_drop"),
        "the template drives the volume's real generated function: {t}"
    );
    assert!(
        t.contains("assert score #hp_leth dw.sys matches ..0"),
        "the template asserts the entity died: {t}"
    );

    // The second template, and what it binds to. A PackTest fake player is
    // permanently undamageable (measured: `Health: 20f` unchanged after 202 ticks
    // inside a swinging volume, `minecraft:generic` refused identically), so this
    // tier cannot witness a player DEATH — that belongs to the bot tier. What it
    // can witness is the opposite direction, and the dummy is the ideal fixture
    // for it: a body that provably never dies must never produce the claim.
    let claim = text(
        &out,
        "packtest-datapack/data/hello-world/test/lethal_the_drop_claim.mcfunction",
    );
    assert!(
        claim.contains("scoreboard players set #leth_hp dw.sys 0"),
        "the guard score is baselined to the DEAD sentinel, so a kill function that \
         never ran cannot pass by leaving it untouched: {claim}"
    );
    // The DRIVER, not the kill function: only the driver carries the `@a[<box>]`
    // re-bind, so only driving it binds the template to the player path existing.
    // Measured: with the player line deleted from the driver, a template that
    // called `lethal_<id>_kill` directly still passed 12/12.
    assert!(
        claim.contains("function hello-world:lethal_the_drop\n")
            && !claim.contains("run function hello-world:lethal_the_drop_kill")
            && claim.contains("assert score #leth_hp dw.sys matches 1.."),
        "the template drives the volume's DRIVER (which carries the player re-bind), \
         not its kill function: {claim}"
    );
}

/// The binding ledger states what the proofs looked at. A green over zero volumes
/// or zero legs is a vacuous pass, and this is what makes that legible without
/// re-deriving it from an empty diagnostics list.
#[test]
fn the_binding_ledger_states_its_counts() {
    let c = parse_hw(&quests_doc(HARMLESS, ""));
    let out = build(&c);
    let gate: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/lethal-gate.json")).unwrap();
    assert_eq!(gate["volumes"]["declared"], 1);
    assert_eq!(gate["volumes"]["resolved"], 1);
    assert_eq!(gate["packtest_templates"], 1);
    assert_eq!(gate["unbound"], false);
    assert!(
        gate["cells"].as_u64().unwrap() >= 1,
        "the volume closes at least one world cell: {gate}"
    );
    assert!(
        gate["respawn_seats_examined"].as_u64().unwrap() >= 1,
        "the entry spawn is always a seat: {gate}"
    );
    assert!(
        gate["critical_path_legs_examined"].as_u64().unwrap() >= 1,
        "the route proof examined at least one leg: {gate}"
    );
}

/// Determinism (ADR-0006): two builds of a lethal-volume campaign are byte-equal.
#[test]
fn a_lethal_volume_build_is_byte_identical_across_runs() {
    let c = parse_hw(&quests_doc(HARMLESS, ""));
    assert_eq!(build(&c), build(&c));
}

// --- the CI fixture -------------------------------------------------------

/// The `lethal-volume` fixture — the campaign the tier-2 PackTest pass boots —
/// validates clean and emits its template. Guarded here so a broken fixture
/// reddens in tier 1 (seconds) rather than in a toolserver boot (minutes), and so
/// the CI step can never quietly stop having a volume to prove.
#[test]
fn the_ci_fixture_validates_and_emits_its_template() {
    use delvewright_compiler::load::load_campaign_dir;
    use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry};

    let dir = common::compiler_fixtures_dir().join("lethal-volume");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the lethal-volume fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = delvewright_dsl::validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "the fixture validates clean: {diags:#?}");

    delvewright_dsl::tag_translatables(&mut c);
    let out = build(&c);
    assert!(
        out.contains_key("packtest-datapack/data/lethal-volume/test/lethal_the_drop.mcfunction"),
        "the fixture emits the template the tier-2 pass runs"
    );
}
