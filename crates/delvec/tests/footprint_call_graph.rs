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
//! own.

use std::path::Path;

fn read(rel: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap();
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The body of `fn <name>` in `src`: from its signature to the next top-level
/// item.
fn body<'a>(src: &'a str, name: &str) -> &'a str {
    let sig = src
        .find(&format!("fn {name}("))
        .unwrap_or_else(|| panic!("no `fn {name}`"));
    let rest = &src[sig..];
    let end = rest[1..]
        .find("\nfn ")
        .or_else(|| rest[1..].find("\npub fn "))
        .or_else(|| rest[1..].find("\n#[cfg(test)]"))
        .map(|i| i + 1)
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
    // The judged footprint.
    let asm = read("crates/delvec/src/compiler/assembly.rs");
    let judge = body(&asm, "judge");
    assert!(
        judge.contains("rig::last_frame_footprint(") && judge.contains("rig::frame_footprint("),
        "`compiler::assembly::judge` judges by the dsl's footprint"
    );
    // Neither caller has a footprint of its own.
    for (file, src) in [("main.rs", &main), ("assembly.rs", &asm)] {
        assert!(
            !src.lines().any(|l| {
                let t = l.trim_start();
                (t.starts_with("fn ")
                    || t.starts_with("pub fn ")
                    || t.starts_with("pub(crate) fn "))
                    && t.contains("footprint")
            }),
            "{file} defines a footprint function of its own"
        );
    }
}
