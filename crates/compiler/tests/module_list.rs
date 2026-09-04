//! The crate doc's module list is the whole list.
//!
//! `lib.rs` opened with a list of modules that named twenty-four of sixty-two. It
//! reads as an index — "Modules:" followed by bullets — so a module absent from
//! it reads as a module that is not there, and a reader looking for where the
//! horizon becomes physical facts found nothing and went looking in `plan`.
//!
//! Nothing could have caught it: a doc comment compiles whatever it says, and a
//! list that is merely incomplete is invisible to `rustdoc`, `clippy` and every
//! gate in `tools/`. So the list is checked here, in both directions — a module
//! with no line, and a line naming no module — because a stale entry left behind
//! by a rename is the same defect wearing the other face.

use std::collections::BTreeSet;
use std::path::Path;

/// Every `pub mod <name>;` in `lib.rs`, and every `//! - [`<name>`]:` bullet
/// above them, as two sets that must be equal.
fn sets(src: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut declared = BTreeSet::new();
    let mut described = BTreeSet::new();
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("pub mod ")
            && let Some(name) = rest.strip_suffix(';')
        {
            declared.insert(name.to_string());
        }
        if let Some(rest) = line.strip_prefix("//! - [`")
            && let Some((name, _)) = rest.split_once("`]: ")
        {
            described.insert(name.to_string());
        }
    }
    (declared, described)
}

#[test]
fn the_module_list_names_every_module() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("the crate root is readable");
    let (declared, described) = sets(&src);

    // A zero on either side is this test's parse failing, not a crate with no
    // modules — and it would fail green, both sets empty and equal.
    assert!(
        declared.len() > 50,
        "only {} `pub mod` lines parsed out of lib.rs — the declaration shape moved",
        declared.len()
    );
    assert!(
        described.len() > 50,
        "only {} module bullets parsed out of the crate doc — the bullet shape moved",
        described.len()
    );

    let undocumented: Vec<_> = declared.difference(&described).collect();
    assert!(
        undocumented.is_empty(),
        "the crate doc's module list does not name {undocumented:?} — add one line each"
    );
    let phantom: Vec<_> = described.difference(&declared).collect();
    assert!(
        phantom.is_empty(),
        "the crate doc's module list names {phantom:?}, which `lib.rs` does not declare"
    );
}

/// The bullets are in the order the modules are declared, so the list can be read
/// against the declarations by eye rather than by search.
#[test]
fn the_module_list_is_in_declaration_order() {
    let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"))
        .expect("the crate root is readable");
    let declared: Vec<&str> = src
        .lines()
        .filter_map(|l| l.strip_prefix("pub mod ").and_then(|r| r.strip_suffix(';')))
        .collect();
    let described: Vec<&str> = src
        .lines()
        .filter_map(|l| l.strip_prefix("//! - [`"))
        .filter_map(|r| r.split_once("`]: ").map(|(n, _)| n))
        .collect();
    assert_eq!(declared, described);
}
