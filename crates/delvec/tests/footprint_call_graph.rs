//! spec-0082 acceptance criterion 8 — **one footprint symbol**.
//!
//! The cells a rig frame's parts occupy are the number a creator reads off
//! `delvec rig describe` and declares a strike's landing box from, and the
//! number `DW0938` refuses that box against. Two implementations would be two
//! answers to one question, and the first time they disagreed a landing box
//! declared from the printed cells would be refused by the check — or worse,
//! pass it while the blow landed somewhere else.
//!
//! So the call graph is asserted from the source, in the family
//! `call_graph_integrity.rs` belongs to: both callers reach
//! `delvewright_dsl::rig::last_frame_footprint` / `frame_footprint`, the dsl
//! defines each exactly once, and neither caller defines a footprint of its
//! own. Both reach the one per-part cell set, `Transform::cells`, through
//! `frame_footprint_turned`.

mod common;

use common::source_scan;
use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap();
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The body of `fn <name>` in `src`: from its signature to the nearest
/// following top-level function, at any visibility (`fn`, `pub fn`,
/// `pub(crate) fn`, `pub(in …) fn`), or `#[cfg(test)]`.
fn body<'a>(src: &'a str, name: &str) -> &'a str {
    let sig = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("no `fn {name}`"));
    let rest = &src[sig..];
    let starts_item = |line: &str| {
        (!line.starts_with(char::is_whitespace) && source_scan::fn_name(line).is_some())
            || line.starts_with("#[cfg(test)]")
    };
    let end = rest
        .match_indices('\n')
        .map(|(i, _)| i + 1)
        .find(|&i| starts_item(&rest[i..]))
        .unwrap_or(rest.len());
    &rest[..end]
}

#[test]
fn rig_describe_and_the_strike_check_read_one_footprint() {
    let rig = read("crates/dsl/src/rig.rs");
    for f in ["frame_footprint", "last_frame_footprint"] {
        assert_eq!(
            rig.matches(&format!("pub fn {f}(")).count(),
            1,
            "`{f}` is defined exactly once"
        );
    }
    // The printed footprint.
    assert!(body(&rig, "describe").contains("last_frame_footprint(clip, facing)"));
    let main = read("crates/delvec/src/main.rs");
    let describe = body(&main, "run_rig_describe");
    assert!(
        describe.contains("rig::describe(") && describe.contains("rig::last_frame_footprint("),
        "`delvec rig describe` prints through the dsl's footprint"
    );
    // Every footprint is the per-part cell set, turned: `frame_footprint` is
    // `frame_footprint_turned` at a quarter turn, which unions
    // `Transform::turned(..).cells()`.
    assert!(
        body(&rig, "frame_footprint")
            .contains("frame_footprint_turned(frame, facing_angle(facing))")
    );
    assert!(body(&rig, "frame_footprint_turned").contains("t.turned(a).cells()"));
    // The judged footprint: the hitbox rule through the dsl's footprint at
    // every facing, and the strike rule through the same per-part cells,
    // each part then met against a standing body's box.
    let asm = read("crates/delvec/src/compiler/assembly.rs");
    let judge = body(&asm, "judge");
    assert!(
        judge.contains("rig::frame_footprint_turned(") && judge.contains("struck_cells("),
        "`compiler::assembly::judge` judges by the dsl's footprint"
    );
    let struck = body(&asm, "struck_cells");
    assert!(
        struck.contains("t.turned(turn)")
            && struck.contains("t.cells()")
            && struck.contains("t.meets_box("),
        "the strike rule reads the dsl's per-part cells: {struck}"
    );
    // Neither caller has a footprint of its own.
    for (file, src) in [("main.rs", &main), ("assembly.rs", &asm)] {
        assert!(
            !src.lines()
                .any(|l| source_scan::fn_name(l).is_some() && l.contains("footprint")),
            "{file} defines a footprint function of its own"
        );
    }
}
