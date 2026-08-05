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
use delvewright_grammar::library::temple;
use delvewright_grammar::{Box3, ExpandOptions, export_prefab};

const REGION: Box3 = Box3::at_origin([13, 14, 21]);

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
