//! **The two answers to "is this block id a fluid" are the same answer.**
//!
//! This test belongs to neither branch that made it necessary, which is why it
//! lands with their integration (CLAUDE.md, *A clean auto-merge is not evidence
//! of semantic compatibility*).
//!
//! One side gave `delvec` a fluid model: `assembled::is_fluid` decides whether a
//! prefab cell or a runtime region write leaves a body swimming, and its own
//! documentation calls it *the one answer to "is this block id a fluid"*. The
//! other side gave the prefab pipeline a containment rule: `schem::fluid`
//! decides whether a body of fluid in a piece is walled, and carries its own
//! `FLUIDS` list to do it.
//!
//! **That duplication is over** (spec-0056). Its stated reason — `delvec` is
//! published and may not depend on `delvewright-schem`, so no edge can collapse
//! the two — was a true fact about the wrong edge: both crates already depend on
//! `delvewright-dsl`, and that is where the block-shape table lives now. Both
//! predicates delegate to `delvewright_dsl::blockshape::is_fluid`.
//!
//! So this file no longer asks whether two lists agree. It asks whether the
//! delegation is real, over the whole pinned registry, from the one crate that
//! can see both — because a re-privatised copy would pass every other test in
//! this workspace.
//!
//! **Binding**: every block id in the pinned 1.21.11 registry — the same file
//! both crates read. A run that examined zero ids is a failure, not a pass.

use std::collections::BTreeMap;

/// The pinned block registry, as its ids. Read straight out of the vendored
/// file (`crates/dsl/data/blocks-1.21.11.json`, `data/PROVENANCE.md`)
/// rather than through either crate's parser, so the corpus is not supplied by
/// one of the two things under test.
fn pinned_block_ids() -> Vec<String> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../dsl/data/blocks-1.21.11.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is not readable: {e}", path.display()));
    let map: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(&text).expect("the pinned registry is a JSON object");
    map.into_keys().collect()
}

/// **Every id in the pin gets the same verdict from both names.**
///
/// The input domain is every spelling either site can be handed: a namespaced
/// `.nbt` palette entry, a bare hand-written `fill-region` block, and — since
/// both now read one state-sensitive table — the same two with a state suffix
/// attached. Inside that domain the answers must be identical.
#[test]
fn both_fluid_predicates_agree_on_every_pinned_block_id() {
    let ids = pinned_block_ids();
    assert!(
        !ids.is_empty(),
        "the pinned registry produced no block ids, so this test examined nothing"
    );

    let mut disagreements: Vec<String> = Vec::new();
    let mut fluids: Vec<String> = Vec::new();
    let mut examined = 0usize;

    for id in &ids {
        // Every spelling reaches both sites: prefab palettes are namespaced, a
        // hand-written `fill-region` block is not, a DSL block may carry a state
        // suffix, and all of them are legal.
        let bare = id.trim_start_matches("minecraft:").to_string();
        for spelling in [
            id.clone(),
            bare.clone(),
            format!("{id}[level=3]"),
            format!("{bare}[level=3]"),
        ] {
            examined += 1;
            let compiler = delvewright_compiler::assembled::is_fluid(&spelling);
            let schem = delvewright_schem::fluid::is_fluid(&spelling);
            if compiler != schem {
                disagreements.push(format!(
                    "{spelling}: delvewright_compiler::assembled::is_fluid={compiler}, \
                     delvewright_schem::fluid::is_fluid={schem}"
                ));
            }
            if compiler && spelling == *id {
                fluids.push(id.clone());
            }
        }
    }

    println!(
        "fluid-predicate agreement: {examined} spelling(s) over {} pinned block id(s); \
         both call {} of them a fluid",
        ids.len(),
        fluids.len()
    );

    assert!(
        disagreements.is_empty(),
        "{} of {examined} spelling(s) get different fluid verdicts from the two crates:\n  {}",
        disagreements.len(),
        disagreements.join("\n  ")
    );

    // The verdict set itself, so the agreement is not agreement on "nothing is a
    // fluid". A cauldron holding one is a block, not a body of fluid, and is the
    // case a substring match would get wrong in both crates at once.
    assert_eq!(
        fluids,
        vec!["minecraft:lava".to_string(), "minecraft:water".to_string()],
        "the agreed fluid set is not the game's two fluids"
    );
}

/// **The place the two used to differ, now closed.**
///
/// `delvec` accepted an id with its state suffix attached
/// (`minecraft:water[level=3]`) because a DSL `fill-region` block is one
/// hand-written string classified whole; `delvewright-schem` saw only palette
/// entries, which never carry a suffix, and answered `false` for the same water.
/// One table, one parser, so both answer for the block rather than for the
/// spelling their own caller happened to hand them.
#[test]
fn a_suffixed_spelling_is_the_same_block_to_both() {
    for name in [
        "minecraft:water[level=3]",
        "water[level=0]",
        "minecraft:lava[level=1]",
    ] {
        assert!(delvewright_compiler::assembled::is_fluid(name), "{name}");
        assert!(delvewright_schem::fluid::is_fluid(name), "{name}");
    }

    // And neither is fooled by a block whose name merely contains a fluid's.
    for id in [
        "minecraft:water_cauldron",
        "minecraft:lava_cauldron",
        "minecraft:waterlogged",
    ] {
        assert!(!delvewright_compiler::assembled::is_fluid(id), "{id}");
        assert!(!delvewright_schem::fluid::is_fluid(id), "{id}");
    }
}
