//! spec-0027 acceptance 4: a grammar program becomes a `.nbt` + metadata pair
//! in a prefab-library directory, and the engine loads it.
//!
//! The point of this test is the seam, not the geometry. `crates/grammar` is
//! generation-time tooling that knows nothing about `delvec`; `PrefabRegistry`
//! is the engine's reader and refuses anything it does not understand with
//! `DW0346`. If the two ever disagree — an anchors map the engine requires and
//! the exporter omits, a lighting profile the exporter invents and the engine
//! has never heard of — this is where it shows, rather than in a campaign build
//! months later reporting "prefab not found".

use std::collections::BTreeMap;

use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{AnchorRegistry, LightingProfile, PrefabId};
use delvewright_grammar::library::{
    ambush_door, castle, causeway, cliff_path, drop_shaft, dumbwaiter, elite_ground, far_side_bar,
    rafter_hall, store_room, temple, watch_bay,
};
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};
// W3: the palette/prop family (W + S + M + X).
use delvewright_grammar::library::{boulder_stair, broken_grate, threshold_motif};

const REGION: Box3 = Box3::at_origin([13, 14, 21]);
const CASTLE_REGION: Box3 = Box3::at_origin([41, 14, 25]);
const CLIFF_REGION: Box3 = Box3::at_origin([3, 6, 30]);
const PASSAGE_REGION: Box3 = Box3::at_origin([7, 7, 24]);
const HALL_REGION: Box3 = Box3::at_origin([13, 6, 25]);
const DOOR_REGION: Box3 = Box3::at_origin([11, 5, 13]);
const STORE_REGION: Box3 = Box3::at_origin([7, 5, 14]);
const STAIR_REGION: Box3 = Box3::at_origin([9, 6, 27]);
const THRESHOLD_REGION: Box3 = Box3::at_origin([9, 6, 13]);
const GRATE_REGION: Box3 = Box3::at_origin([3, 5, 14]);
/// The topology family (task #182): vertical links, one-way bars, elite ground.
const SHAFT_REGION: Box3 = Box3::at_origin([4, 8, 6]);
const DUCT_REGION: Box3 = Box3::at_origin([6, 8, 8]);
const BAR_REGION: Box3 = Box3::at_origin([5, 5, 7]);
const CAUSEWAY_REGION: Box3 = Box3::at_origin([7, 10, 9]);
const ARENA_REGION: Box3 = Box3::at_origin([19, 5, 25]);

fn library_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-prefab-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_grammar_temple_lands_in_the_prefab_library_and_loads() {
    let dir = library_dir("temple");
    let export = export_prefab(
        &temple(),
        REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();
    export.write_to_dir(&dir).unwrap();

    let registry = PrefabRegistry::load_dir(&dir).unwrap();
    assert!(
        registry.load_diagnostics().is_empty(),
        "the engine refused the exported metadata: {:#?}",
        registry.load_diagnostics()
    );

    let meta = registry
        .get("prefab/grammar-temple")
        .expect("the exported prefab id is the one the registry indexes");
    assert_eq!(meta.structure.file, "grammar-temple.nbt");
    assert_eq!(meta.structure.id, "grammar-temple");
    assert_eq!(meta.structure.size, [13, 14, 21]);
    assert_eq!(meta.structure.data_version, 4671);
    assert_eq!(meta.structure.generator.as_deref(), Some("crates/grammar"));

    // Anchors-empty metadata is a *valid* prefab, not a broken one: it simply
    // offers no staging points yet. The registry must index it as such.
    assert!(meta.anchors.is_empty());
    let anchors = registry
        .anchors_for(&PrefabId("prefab/grammar-temple".to_string()))
        .expect("an anchorless prefab is still a known prefab");
    assert!(anchors.is_empty());
    assert!(meta.connectors.is_empty());

    // The lighting declaration survives the round trip as the honest one: this
    // piece owes a measurement and says so, rather than claiming `lit`.
    let lighting = meta.lighting.clone().expect("the export declares lighting");
    assert_eq!(lighting.profile, LightingProfile::Unmeasured);
    assert_eq!(lighting.measured_min_light, None);
    assert_eq!(lighting.measured, None);

    // ...and the structure the metadata points at is one the engine's own
    // decoder reads back cell for cell.
    let nbt = std::fs::read(dir.join(&meta.structure.file)).unwrap();
    let cells: BTreeMap<[i32; 3], String> =
        delvewright_compiler::assembled::structure_cells_stateful(&nbt)
            .into_iter()
            .map(|(pos, state, _)| (pos, state))
            .collect();
    let model = &export.expansion.model;
    let expected: BTreeMap<[i32; 3], String> = REGION
        .positions()
        .filter_map(|pos| {
            let block = model.get(pos).unwrap();
            (!block.is_air()).then(|| (pos, block.to_string()))
        })
        .collect();
    assert!(!expected.is_empty(), "the temple built nothing");
    assert_eq!(
        cells, expected,
        "the .nbt in the library is not the model that was exported"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The other half of the seam: an anchor a rule **declared** with `mark` has to
/// survive export, land in the metadata's `anchors` map, and come back out of
/// `PrefabRegistry` as an anchor the DSL can name. An anchor the engine cannot
/// see is not an anchor — it is a comment.
#[test]
fn an_anchor_a_rule_marked_comes_back_out_of_the_registry() {
    let dir = library_dir("castle");
    let export = export_prefab(
        &castle(),
        CASTLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-castle",
    )
    .unwrap();
    export.write_to_dir(&dir).unwrap();

    let registry = PrefabRegistry::load_dir(&dir).unwrap();
    assert!(
        registry.load_diagnostics().is_empty(),
        "the engine refused the exported metadata: {:#?}",
        registry.load_diagnostics()
    );

    let meta = registry.get("prefab/grammar-castle").expect("indexed");
    let anchor = meta
        .anchors
        .get("anchor/courtyard")
        .expect("the marked anchor is in the metadata the engine parsed");
    assert_eq!(anchor.pos, Some([20, 0, 12]));
    assert_eq!(anchor.facing.as_deref(), Some("north"));

    // ...and it is a name the DSL side can resolve, which is what an anchor is for.
    let names = registry
        .anchors_for(&PrefabId("prefab/grammar-castle".to_string()))
        .expect("a marked prefab is a known prefab");
    assert!(
        names.contains("anchor/courtyard"),
        "anchor names the DSL can reference: {names:?}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// The staging vocabulary (spec-0027 W1 and W2) is the first thing to put *many*
/// anchors through this seam, and indexed ones at that. `anchor/niche-1`,
/// `anchor/perch-7`, … are generated names — no one hand-lists them — so the
/// seam has to carry a set it was never told the size of, and the engine has to
/// hand every one of them back as a name the DSL can bind an actor to.
#[test]
fn the_staging_rules_indexed_anchors_all_reach_the_registry() {
    for (name, program, region, expected) in [
        (
            "grammar-cliff-path",
            cliff_path(),
            CLIFF_REGION,
            vec!["anchor/niche-1", "anchor/niche-watch-1"],
        ),
        (
            "grammar-gate-passage",
            watch_bay(),
            PASSAGE_REGION,
            vec!["anchor/watch", "anchor/gate"],
        ),
        (
            "grammar-rafter-hall",
            rafter_hall(),
            HALL_REGION,
            vec!["anchor/hall-door", "anchor/perch-1", "anchor/perch-7"],
        ),
        (
            "grammar-ambush-door",
            ambush_door(),
            DOOR_REGION,
            vec!["anchor/alcove", "anchor/threshold"],
        ),
        (
            "grammar-store-room",
            store_room(),
            STORE_REGION,
            vec!["anchor/store-line", "anchor/tell"],
        ),
        (
            "grammar-boulder-stair",
            boulder_stair::boulder_stair(),
            STAIR_REGION,
            vec!["anchor/stair-run", "anchor/volley-slot", "anchor/pocket-1"],
        ),
        (
            "grammar-threshold-motif",
            threshold_motif::threshold_motif(),
            THRESHOLD_REGION,
            vec!["anchor/threshold-narrate"],
        ),
        (
            "grammar-broken-grate",
            broken_grate::broken_grate(),
            GRATE_REGION,
            vec!["anchor/grate-secret"],
            "grammar-drop-shaft",
            drop_shaft(),
            SHAFT_REGION,
            vec!["anchor/spill", "anchor/landing"],
        ),
        (
            "grammar-dumbwaiter",
            dumbwaiter(),
            DUCT_REGION,
            vec!["anchor/hatch", "anchor/landing"],
        ),
        (
            "grammar-far-side-bar",
            far_side_bar(),
            BAR_REGION,
            vec!["anchor/gate", "anchor/unlock"],
        ),
        (
            "grammar-causeway",
            causeway(),
            CAUSEWAY_REGION,
            vec!["anchor/causeway-head", "anchor/elite"],
        ),
        (
            "grammar-elite-ground",
            elite_ground(),
            ARENA_REGION,
            vec!["anchor/elite"],
        ),
    ] {
        let dir = library_dir(name);
        let export = export_prefab(&program, region, &ExpandOptions::seeded(4), name).unwrap();
        export.write_to_dir(&dir).unwrap();

        let registry = PrefabRegistry::load_dir(&dir).unwrap();
        assert!(
            registry.load_diagnostics().is_empty(),
            "{name}: the engine refused the exported metadata: {:#?}",
            registry.load_diagnostics()
        );
        let meta = registry.get(&format!("prefab/{name}")).expect("indexed");
        let names = registry
            .anchors_for(&PrefabId(format!("prefab/{name}")))
            .expect("a marked prefab is a known prefab");

        for want in expected {
            let anchor = meta
                .anchors
                .get(want)
                .unwrap_or_else(|| panic!("{name} lost {want}: {:#?}", meta.anchors));
            let pos = anchor.pos.expect("a staging anchor names a cell");
            assert!(
                (0..3).all(|i| pos[i] >= 0 && pos[i] < meta.structure.size[i]),
                "{name}/{want} sits at {pos:?}, outside the {:?} structure",
                meta.structure.size
            );
            assert!(anchor.facing.is_some(), "{name}/{want} has no facing");
            assert!(
                names.contains(want),
                "{name}: {want} not bindable: {names:?}"
            );
        }
        // ...and nothing else crept in: every exported anchor is one the rules
        // declared, under a name the DSL's `anchor/<kebab>` grammar accepts.
        assert_eq!(meta.anchors.len(), names.len());
        assert!(meta.anchors.keys().all(|k| k.starts_with("anchor/")));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// The exporter must not be able to hand the engine metadata the engine would
/// reject. A hand-written near-miss shows what that failure looks like — and
/// that the export above is not simply passing because the registry ignores
/// what it cannot parse.
#[test]
fn metadata_the_engine_cannot_read_is_a_load_diagnostic_not_a_silent_skip() {
    let dir = library_dir("bad");
    std::fs::write(
        dir.join("faked.json"),
        r#"{
  "prefab_id": "prefab/faked",
  "structure": { "file": "faked.nbt", "id": "faked", "size": [3, 3, 3],
                 "data_version": 4671, "generator": "crates/grammar" },
  "anchors": {},
  "lighting": { "profile": "unmeasured", "measured_min_light": 9 },
  "license": {}
}
"#,
    )
    .unwrap();

    let registry = PrefabRegistry::load_dir(&dir).unwrap();
    let diags = registry.load_diagnostics();
    assert_eq!(diags.len(), 1, "{diags:#?}");
    assert_eq!(diags[0].code, "DW0346");
    assert!(
        diags[0].message.contains("faked.json"),
        "{}",
        diags[0].message
    );
    assert!(registry.get("prefab/faked").is_none());

    std::fs::remove_dir_all(&dir).unwrap();
}
