//! **`liveness_of` has exactly the production callers named here.**
//!
//! `compiler::nav::world::liveness_of` is the function `region_state_at`'s
//! derivation calls, and the only reader of a gate into a lethal set; its unit
//! tests live with it in `nav/world/tests.rs`. Its callers are enumerated from
//! the package's production text ([`common::source_scan`]), each named by the
//! function its call sits in, so a new reader reds this test until it is
//! added here.

mod common;

use common::source_scan;
use std::path::Path;

#[test]
fn region_state_at_derives_the_lethal_set_through_liveness_of() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut callers: Vec<String> = Vec::new();
    for (rel, text) in source_scan::production_sources(&root) {
        let mut current = String::new();
        for line in text.lines() {
            if let Some(name) = source_scan::fn_name(line) {
                current = name.to_string();
            }
            if line.contains("liveness_of(") && !line.contains("fn liveness_of") {
                callers.push(format!("{rel}::{current}"));
            }
        }
    }
    callers.sort();
    callers.dedup();
    assert_eq!(
        callers,
        vec![
            "compiler/lethal.rs::never_held_term",
            "compiler/nav/route/region.rs::loop_liveness",
            "compiler/nav/route/region.rs::staged_liveness"
        ],
        "liveness_of is read by staged_liveness (the lethal set), by loop_liveness \
         (the held loop slabs, spec-0086) and by DW0954's per-term wording, and by \
         nothing else"
    );
    let nav = std::fs::read_to_string(root.join("compiler/nav/route/region.rs")).unwrap();
    let body = &nav[nav.find("fn region_state_inner(").unwrap()..];
    let body = &body[..body.find("\n    }\n").unwrap()];
    assert!(
        body.contains(".staged_liveness(region_events, arrival, ancestor)"),
        "region_state_at's derivation calls staged_liveness"
    );
    assert!(
        body.contains(".loop_liveness(region_events, arrival, ancestor)"),
        "region_state_at's derivation calls loop_liveness"
    );
}
