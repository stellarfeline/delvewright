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
//! Neither crate can read the other's answer. `delvec` is published to
//! crates.io and deliberately does not depend on `delvewright-schem`; the
//! dependency edge that would collapse the two predicates into one is exactly
//! the edge publication forbids. So the duplication stays, and what binds it is
//! this: a divergence would let a piece the containment gate calls sealed be a
//! piece the compiler routes a party across, or the reverse, with both green.
//!
//! `delve-admit` is the one crate that depends on both, so the assertion lives
//! here.
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

/// **Every id in the pin gets the same verdict from both predicates.**
///
/// The shared input domain is a bare id — namespaced or not. That is what a
/// parsed `.nbt` palette entry carries (name and properties are separate
/// fields), and it is what a DSL-authored block id carries before its optional
/// state suffix. Inside that domain the two must never disagree.
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
        // Both spellings reach both sites: prefab palettes are namespaced, a
        // hand-written `fill-region` block is not, and both are legal.
        for spelling in [id.clone(), id.trim_start_matches("minecraft:").to_string()] {
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

/// **The one place the two are allowed to differ, stated rather than
/// discovered.**
///
/// `delvec` also accepts an id with its state suffix attached
/// (`minecraft:water[level=3]`), because a DSL `fill-region` block is one
/// hand-written string and the compiler classifies it whole. A palette entry
/// never carries a suffix, so `delvewright-schem` never sees one and does not
/// model it. That is an input-shape difference, not a disagreement about the
/// game — and it is asserted here so that a future change which "fixes" one
/// side to match the other has to come past this comment first.
#[test]
fn only_the_compiler_classifies_a_suffixed_spelling() {
    assert!(delvewright_compiler::assembled::is_fluid(
        "minecraft:water[level=3]"
    ));
    assert!(delvewright_compiler::assembled::is_fluid("water[level=0]"));
    assert!(!delvewright_schem::fluid::is_fluid(
        "minecraft:water[level=3]"
    ));

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
