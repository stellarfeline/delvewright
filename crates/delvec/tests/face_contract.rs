//! **A prefab's face contract survives the round trip into the engine**
//! (spec-0036 §2.8).
//!
//! The grammar exporter writes the contract block, and the compiler's metadata
//! reader refuses fields it does not know, so a contract-carrying prefab that
//! did not load would fail months later inside a campaign build. The artifact is
//! real: the prefab is exported by `crates/delvec/src/grammar` from the corpus
//! program that declares a contract and loaded back through the engine's own
//! `PrefabRegistry`.

use delvec::compiler::registry::PrefabRegistry;
use delvec::grammar::library::spatial_contract::spatial_contract;
use delvec::grammar::{Box3, ExpandOptions, export_prefab};

/// The region the corpus program is documented at.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

/// Export the contract-carrying corpus program into a throwaway prefab library.
fn library(tag: &str) -> (std::path::PathBuf, PrefabRegistry) {
    let dir = std::env::temp_dir().join(format!("dw-face-contract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let exported = export_prefab(
        &spatial_contract(),
        PIECE,
        &ExpandOptions::seeded(1),
        "twin-room",
    )
    .expect("the corpus program exports");
    exported.write_to_dir(&dir).unwrap();
    let registry =
        PrefabRegistry::load_dir(&dir).expect("the engine loads a contract-carrying prefab");
    (dir, registry)
}

/// **The engine reads a contract-carrying prefab at all.**
#[test]
fn a_prefab_carrying_a_spatial_contract_loads_and_exports_its_faces() {
    let (_dir, registry) = library("load");
    let meta = registry.get("prefab/twin-room").expect("it loaded");
    let contract = meta
        .spatial_contract
        .as_ref()
        .expect("the contract block survived the round trip");
    assert_eq!(contract.faces.len(), 2, "one way in at each end");
    let sides: Vec<&str> = contract.faces.iter().map(|f| f.dir.as_str()).collect();
    assert_eq!(sides, vec!["north", "south"]);
    assert!(contract.faces.iter().all(|f| f.class == "walk"));
}
