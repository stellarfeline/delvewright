//! DSL v0.10 (spec-0031) end-to-end emission for the two verbs that had emission
//! and no surface: the status-effect pair and the region `teleport`.
//!
//! **Acceptance criterion 7 lives here.** "The selection is total" is a property
//! of an emitted selector, and the only way to know it is to read the selector
//! the compiler actually wrote. So `the_teleport_selection_is_total_over_bodies`
//! parses the emitted `tp` command and asserts its selector carries the six box
//! terms plus exactly one narrowing — `tag=!dw_fixture` — and nothing else: no
//! `type=`, no second `tag=`, no `limit=`, no `sort=`, no `nbt=`.
//!
//! Total over **bodies** is what the criterion always meant and what the owner's
//! cargo-lift ruling asks for. A fixture is not a passenger: its position IS
//! engine state (a recovery stake's marker, an affordance hitbox), so carrying it
//! does not move a thing, it rewrites a fact — and for a stake the tick after the
//! ride deletes the marker and the wager with it. The class is decided at the
//! OBJECT (`compiler::affordance`, `DW0543`), which is why one negated tag can
//! stand where a type roster cannot.
//!
//! That roster is still a deliberate divergence from `lethal_volumes[]` (#347),
//! which exempts five engine machinery types by name: a teleport must never
//! exempt a TYPE, because an NPC is a body plus a co-located
//! `minecraft:interaction` and the type says nothing about which of the two it
//! is. The reasoning is in `compiler::teleport`; the test that makes the
//! compile-time half real is `a_teleport_over_a_bound_affordance_is_dw0542`, and
//! the runtime half is `crates/compiler/tests/fixture_class.rs`.
//!
//! The status-effect half asserts the emitted `effect give` / `effect clear`, and
//! — more importantly — that the *engine's own* night-vision grant now goes
//! through the same formatter. Its byte-for-byte stability is proven by
//! `crates/compiler/tests/relight.rs` and `vision_camera_coverage.rs`, which
//! assert the exact pre-existing line and were not touched.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};

/// The fixture campaign's namespace — the emitted function prefix.
const NS: &str = "cast-ledger";

/// A private working directory per CALLER — the tests run in parallel threads of
/// one binary, and a shared scratch directory is a race whose symptom is a
/// missing file (an intermittent red, which is a finding, not a re-run).
fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const WORLD: &str = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "difficulty": "normal",
    "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
  }
}"#;

const DIALOGUE: &str = r#"{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      {
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {
            "id": "dlg/greeting",
            "text": "Halt. The door stays shut until the toll is paid.",
            "options": [
              { "label": "Pay what he asks.",
                "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] }
            ]
          }
        ]
      },
      {
        "npc": "npc/sleeper",
        "root": "dlg/snore",
        "nodes": [ { "id": "dlg/snore", "text": "...mm. Not my watch.", "options": [] } ]
      }
    ]
  }
}"#;

/// The worked shape, minus the lift: blind whoever is standing in the box, move
/// everything in that box to the far anchor, and let the blindness expire on its
/// own. No `clear-effect` anywhere — that is the prescribed pattern, and it is
/// why this fixture validates clean under `DW0540`.
fn quests(teleport_extent: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.10.0",
  "campaign_id": "cast-ledger",
  "stage": "quests",
  "content": {{
    "triggers": [
      {{
        "id": "trigger/door-latch",
        "at": "anchor/door",
        "on": {{ "on": "use" }},
        "effects": [ {{ "type": "narrate", "text": "The latch will not give." }} ]
      }}
    ],
    "quests": [
      {{
        "id": "quest/ask",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }} ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "sequence", "steps": [
              {{ "at_ticks": 0, "effects": [
                {{ "type": "give-effect", "effect": "minecraft:blindness", "seconds": 2,
                   "hide_particles": true,
                   "in": {{ "anchor": "anchor/keeper-stand", "extent": [1, 2, 1] }} }} ] }},
              {{ "at_ticks": 2, "effects": [
                {{ "type": "teleport",
                   "from": {{ "anchor": "anchor/keeper-stand", "extent": {teleport_extent} }},
                   "to": "anchor/exit" }} ] }}
            ] }},
            {{ "type": "clear-effect", "effect": "minecraft:poison" }}
          ]
        }},
        "on_complete": [ {{ "type": "narrate", "text": "The bolt slides back." }} ]
      }},
      {{
        "id": "quest/leave",
        "trigger": {{ "type": "quest-complete", "quest": "quest/ask" }},
        "objectives": [
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// Materialize and build the campaign. `Err` carries the build diagnostic, which
/// is what the `DW0542` test reads.
fn try_build(who: &str, teleport_extent: &str) -> Result<BuildOutput, (String, String)> {
    let dir = tmp(&format!("v10-teleport-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(
            common::compiler_fixtures_dir().join("cast-ledger").join(f),
            dir.join(f),
        )
        .unwrap();
    }
    std::fs::write(dir.join("world.json"), WORLD).unwrap();
    std::fs::write(dir.join("quests.json"), quests(teleport_extent)).unwrap();
    std::fs::write(dir.join("dialogue.json"), DIALOGUE).unwrap();

    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("v10-teleport parses");
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let items = FullItemRegistry::v1_21_11();
    let entities = FullEntityRegistry::v1_21_11();
    let diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
    assert!(
        diags.is_empty(),
        "the v10-teleport fixture must validate clean: {diags:#?}"
    );

    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(prefab_dir.join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
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
        &BTreeMap::new(),
    )
    .map_err(|e| match e {
        emit::BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a diagnostic, got {other:?}"),
    })
}

fn build(who: &str) -> BuildOutput {
    try_build(who, "[1, 2, 1]").expect("every emitted command validates")
}

/// Everything emitted, as one string.
fn all_functions(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(p, b)| format!("### {p}\n{}\n", String::from_utf8_lossy(b)))
        .collect()
}

/// The one emitted `tp` line whose selector is an `@e[…]` box — the teleport's.
fn teleport_line(out: &BuildOutput) -> String {
    let all = all_functions(out);
    let hits: Vec<&str> = all.lines().filter(|l| l.starts_with("tp @e[x=")).collect();
    assert_eq!(
        hits.len(),
        1,
        "exactly one `teleport` is declared, so exactly one such line is emitted:\n{all}"
    );
    hits[0].to_string()
}

/// **Acceptance criterion 7.** The emitted selector is the volume, narrowed by
/// exactly one thing: the fixture class. Read off the emission, so a future
/// exemption cannot be added without this failing.
///
/// The assertion pins the permitted filter by its exact spelling rather than
/// counting terms, so it stays the same proof it always was: any `type=`,
/// `limit=`, `sort=` or `nbt=` term still reds here, and so does a second tag.
/// What it now also states is *which* narrowing is licit and why there is exactly
/// one — a negated CLASS, decided at the object, never a roster of types decided
/// inside this verb (`DW0543`, `compiler::affordance`).
#[test]
fn the_teleport_selection_is_total_over_bodies() {
    let out = build("total");
    let line = teleport_line(&out);
    let args = line
        .split_once('[')
        .and_then(|(_, r)| r.split_once(']'))
        .expect("an `@e[...]` selector")
        .0;
    let terms: Vec<&str> = args.split(',').collect();
    let keys: Vec<&str> = terms
        .iter()
        .map(|t| t.split_once('=').expect("a `key=value` term").0)
        .collect();
    assert_eq!(
        keys,
        ["x", "dx", "y", "dy", "z", "dz", "tag"],
        "the selection must be TOTAL over BODIES: every body inside the volume is moved, and \
         the only term that may narrow it is the fixture-class exclusion. `lethal_volumes[]` \
         exempts five machinery TYPES by name; a teleport must not, because an NPC is a body \
         plus a co-located `minecraft:interaction` and exempting that type would move the \
         speaker and leave its dialogue box behind. See `compiler::teleport`. Emitted: {line}"
    );
    assert_eq!(
        terms[6], "tag=!dw_fixture",
        "the one permitted narrowing is the fixture class — an entity whose position IS engine \
         state (a stake marker, an affordance hitbox), which a lift must leave where the \
         ledger recorded it. Any other tag term is a bespoke exemption wearing the class's \
         clothes. Emitted: {line}"
    );
    // …and the destination is a compile-time literal, not a runtime search.
    let dest: Vec<&str> = line.rsplitn(4, ' ').collect();
    assert_eq!(dest.len(), 4, "three absolute coordinates: {line}");
    for c in &dest[..3] {
        assert!(
            c.parse::<f64>().is_ok(),
            "destination coordinate `{c}` must be an absolute literal: {line}"
        );
    }
}

/// The teleport's `from` volume becomes the selector box `anchor ± extent`, in
/// vanilla's `dx`-is-a-span spelling — the same `box_selector_args` a lethal
/// volume uses, so the two verbs cannot disagree by one block about "inside".
#[test]
fn the_volume_is_the_anchor_centred_box() {
    let out = build("box");
    let line = teleport_line(&out);
    // `anchor/keeper-stand` is prefab-local [5,1,4] with extent [1,2,1]; the
    // absolute placement is what the plan resolves, so the assertion is on the
    // SPANS, which are placement-independent.
    for (k, v) in [("dx", 2), ("dy", 4), ("dz", 2)] {
        assert!(
            line.contains(&format!("{k}={v},")) || line.contains(&format!("{k}={v}]")),
            "`{k}` must be 2*extent ({v}): {line}"
        );
    }
}

/// `DW0542`: a volume that covers an affordance the engine bound to hardware it
/// cannot move. The widened extent swallows `anchor/exit`, where this fixture's
/// second NPC stands and where `obj/exit`'s own machinery sits.
#[test]
fn a_teleport_over_a_bound_affordance_is_dw0542() {
    let (code, message) = try_build("bound", "[12, 6, 12]")
        .expect_err("a volume covering an engine affordance must fail the build");
    assert_eq!(code, "DW0542", "{message}");
    assert!(
        message.contains("teleport"),
        "the message must name the verb: {message}"
    );
}

/// The binding ledger exists, states what it examined, and is not unbound.
#[test]
fn the_teleport_gate_ledger_states_its_binding() {
    let out = build("ledger");
    let raw = out
        .get("validation/teleport-gate.json")
        .expect("a campaign that declares a teleport emits its ledger");
    let gate: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(gate["teleports"]["declared"], 1);
    assert_eq!(gate["teleports"]["resolved"], 1);
    assert_eq!(gate["unbound"], false);
    assert_eq!(
        gate["cells"], 45,
        "3 x 5 x 3 cells, inclusive on both corners: {gate}"
    );
    assert!(
        gate["affordances_examined"].as_u64().unwrap() > 0,
        "a proof that examined zero affordances proves nothing: {gate}"
    );
}

/// A campaign that declares no teleport emits no ledger — no artifact, no byte
/// moved for anybody who has not opted in.
#[test]
fn a_campaign_without_a_teleport_emits_no_ledger() {
    let dir = tmp("v10-teleport-none");
    for f in common::STAGE_FILES {
        std::fs::copy(
            common::compiler_fixtures_dir().join("cast-ledger").join(f),
            dir.join(f),
        )
        .unwrap();
    }
    let prefab_dir = common::prefabs_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            structures.insert(
                piece.structure_file.clone(),
                std::fs::read(prefab_dir.join(&piece.structure_file)).unwrap(),
            );
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .unwrap();
    assert!(!out.contains_key("validation/teleport-gate.json"));
}

/// `give-effect` lowers to vanilla's full five-token `effect give`, narrowed by
/// the declared `in` box; `clear-effect` lowers to `effect clear`.
#[test]
fn the_status_effect_verbs_lower_to_vanilla_commands() {
    let out = build("effects");
    let all = all_functions(&out);
    let give = all
        .lines()
        .find(|l| l.contains("effect give") && l.contains("minecraft:blindness"))
        .unwrap_or_else(|| panic!("no blindness grant emitted:\n{all}"));
    assert!(
        give.ends_with("minecraft:blindness 2 0 true"),
        "the full form — duration, amplifier, hideParticles — leaves nothing to a vanilla \
         default: {give}"
    );
    assert!(
        give.contains("@a[x=") && give.contains("dy=4"),
        "the `in` filter must narrow the audience to the declared box: {give}"
    );
    assert!(
        all.lines()
            .any(|l| l.trim() == "effect clear @a minecraft:poison"),
        "a `clear-effect` naming an id lowers to vanilla's id form:\n{all}"
    );
}

/// The `teleport` ignores the effect's audience by construction: its selector is
/// the volume, which has no party. A `@a`-dispatched bundle and a `@s` one emit
/// the same command, so the same box means the same thing wherever it is fired
/// from.
#[test]
fn the_teleport_selector_does_not_depend_on_the_audience() {
    let out = build("audience");
    let line = teleport_line(&out);
    assert!(
        !line.contains("@a") && !line.contains("@s"),
        "a teleport addresses the volume, never the party: {line}"
    );
}

/// The runtime half exists and is bound: one PackTest template per teleport,
/// calling the campaign's REAL generated function and putting the four machinery
/// types a lethal volume must exempt into the box beside a content body.
///
/// A compile-time-only green over a runtime mechanism is the vacuity CLAUDE.md
/// names, so the ledger reports the template count beside the teleport count and
/// this test reds if the two ever disagree.
#[test]
fn the_runtime_half_is_generated_and_counted() {
    let out = build("packtest");
    let (path, body) = out
        .iter()
        .find(|(p, _)| p.contains("/test/teleport_"))
        .unwrap_or_else(|| panic!("a teleport must generate its runtime template"));
    let t = String::from_utf8_lossy(body);
    let name = path
        .rsplit('/')
        .next()
        .unwrap()
        .trim_end_matches(".mcfunction");
    assert!(
        t.contains(&format!("function {NS}:{name}")),
        "the template must call the REAL generated function, not a command it re-typed:\n{t}"
    );
    for ty in [
        "minecraft:zombie",
        "minecraft:interaction",
        "minecraft:marker",
        "minecraft:text_display",
        "minecraft:item",
    ] {
        assert!(
            t.contains(&format!("summon {ty} ")),
            "the witnesses must include `{ty}` — three of these are types a lethal \
             volume MUST exempt, and this template is what says a teleport does not:\n{t}"
        );
    }
    // The scratch holders are suffixed with the teleport's own key: fake players
    // on `dw.sys` are batch-global, so a campaign with two rides sharing `#tp_in`
    // is one interleaving away from an intermittent red (`packtest_batch.rs`,
    // which now sweeps the `lift-stake` family and found exactly that).
    assert!(
        t.lines()
            .any(|l| l.starts_with("assert score #tp_in_") && l.ends_with("dw.sys matches 5")),
        "bound, not assumed: every witness is inside the box before anything moves:\n{t}"
    );
    assert!(
        t.lines()
            .any(|l| l.starts_with("assert score #tp_left_") && l.ends_with("dw.sys matches 0")),
        "…and none is left behind:\n{t}"
    );
    let gate: serde_json::Value =
        serde_json::from_slice(out.get("validation/teleport-gate.json").unwrap()).unwrap();
    assert_eq!(
        gate["packtest_templates"], gate["teleports"]["resolved"],
        "every resolved teleport owes a runtime template: {gate}"
    );
}

/// Nothing in the emitted tree tries to reset fall distance. The spike measured
/// that fall distance CARRIES across a teleport (Δ 0.0000 in 46/46 trials) and
/// explicitly did NOT measure what resets it, so any reset here would be
/// folklore. This test is the record that the omission is deliberate.
#[test]
fn the_teleport_does_not_pretend_to_be_a_rescue() {
    let out = build("fall");
    let all = all_functions(&out);
    assert!(
        !all.contains("fall_distance") && !all.contains("FallDistance"),
        "what resets fall distance was not measured; emitting a reset would be a mechanism \
         invented from recall:\n{all}"
    );
}
