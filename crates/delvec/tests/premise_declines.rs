//! **Every production decline of the campaign's premises, enumerated.**
//!
//! `Premises::of_plan` makes forgetting a premise impossible — you cannot
//! state a subset — but `Premises::geometry_only` is still a way to decline
//! the whole set, and a decline nobody has to write down is the original
//! defect with a better name. So the production call sites are listed here
//! with their counts, and a new one reds this test until it is added.
//!
//! The point is not the numbers. It is that adding a decline is an edit to
//! this list, which a reviewer reads, rather than an absence, which nobody
//! can see. Each entry's reason lives at its call site, in a comment beside
//! the call; this test only insists that the entry exists.
//!
//! The population is the package's sources (`crates/delvec/src`, the
//! compiler and the binary that mounts it — `delvec snapshot` stands a
//! camera up in `main.rs`), each read as its production text
//! ([`common::source_scan`]): a synthetic world in a unit test has no
//! campaign behind it and nothing to state.

mod common;

#[test]
fn premise_declines_are_enumerated() {
    // (file under `src/`, how many production call sites) — the path, not
    // the file name: `mod.rs` names more than one file.
    const EXPECTED: &[(&str, usize)] = &[
        // `blockout.rs`: the stage-5 battery's `open` and `sealed` worlds,
        // which carry their own sealing authority; and the stairwell pass,
        // which asks only whether one step between two courses of a stair
        // is a body move over the mass as laid — a question about geometry
        // inside the stair's own box, with no campaign premise to consult.
        ("compiler/blockout.rs", 3),
        // `edit.rs`: a `relight` verb's own darkness survey.
        ("compiler/edit.rs", 1),
        // `light.rs`: the relight pass's darkness survey.
        ("compiler/light.rs", 1),
        // `compiler/nav/world/mod.rs`: the synthetic constructors' own door
        // (`from_solid_and_flooded`), which every unit-test world goes
        // through.
        ("compiler/nav/world/mod.rs", 1),
        // `main.rs`: `delvec snapshot`, where a camera is stood up against
        // blocks.
        ("main.rs", 1),
        // `sculpt/mod.rs`: the pocket proof over a sculpted piece ALONE,
        // before any campaign exists to state a premise (spec-0087 §3.4) — no
        // horizon, no volume, no gate; the piece's own blocks are the question.
        ("sculpt/mod.rs", 1),
    ];

    let src = common::repo_root().join("crates/delvec/src");
    let files = common::source_scan::production_sources(&src);
    assert!(
        files.len() > 20,
        "the population is the crate's sources, not a handful: {}",
        files.len()
    );

    let mut found: Vec<(String, usize)> = Vec::new();
    for (rel, prod) in &files {
        // The definition itself is `pub fn geometry_only`, never a call.
        let n = prod.matches("Premises::geometry_only()").count();
        if n > 0 {
            found.push((rel.clone(), n));
        }
    }
    // Order by path, as EXPECTED is.
    found.sort();
    let expected: Vec<(String, usize)> = EXPECTED
        .iter()
        .map(|(f, n)| ((*f).to_string(), *n))
        .collect();
    assert_eq!(
        found,
        expected,
        "a production world declines the campaign's premises somewhere this \
         list does not name. That is legitimate — say WHY at the call site, \
         then add it here. It is not legitimate to leave it unlisted: the \
         whole reason `Premises` exists is that a premise nobody has to \
         mention is a premise that goes missing. ({} source file(s) examined)",
        files.len()
    );
}

/// The rule the scan reads production through removes the item a column-zero
/// `#[cfg(test)]` applies to and nothing after it — the defect it replaces
/// stopped at the first such attribute and dropped every production item below
/// a test-only helper.
#[test]
fn a_test_only_item_removes_itself_and_nothing_after_it() {
    use common::source_scan::production;
    let text = "fn a() {}\n#[cfg(test)]\nfn helper() -> &'static str {\n    \"} not the end\"\n}\nfn b() { let _ = '}'; }\n#[cfg(test)]\nmod tests;\nconst C: [u8; 2] = [1, 2];\n#[cfg(test)]\nmod inline {\n    fn t() {}\n}\nfn d() {}\n";
    let (prod, modules) = production(text);
    assert_eq!(
        prod,
        "fn a() {}\n\nfn b() { let _ = '}'; }\n\nconst C: [u8; 2] = [1, 2];\n\nfn d() {}\n"
    );
    assert_eq!(modules, vec!["tests".to_string()]);
}

/// A test module written as its own file is test code exactly as an inline one
/// is: the scan of the package's own tree lists none of them.
#[test]
fn an_out_of_line_test_module_is_not_production() {
    let src = common::repo_root().join("crates/delvec/src");
    let files = common::source_scan::production_sources(&src);
    let listed: Vec<&str> = files.iter().map(|(r, _)| r.as_str()).collect();
    for test_file in [
        "compiler/nav/world/tests.rs",
        "compiler/nav/route/tests.rs",
        "compiler/nav/testkit.rs",
    ] {
        assert!(
            src.join(test_file).is_file(),
            "{test_file} exists, so this assertion binds"
        );
        assert!(
            !listed.contains(&test_file),
            "{test_file} is a `#[cfg(test)]` module, not production"
        );
    }
    assert!(listed.contains(&"compiler/nav/world/mod.rs"));
}
