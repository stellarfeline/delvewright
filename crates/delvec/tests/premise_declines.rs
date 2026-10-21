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
//! compiler and the binary that mounts it), each read as its production text
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
        // `grammar/contract.rs`: the climb edge's proof over ONE piece's own
        // blocks (`body_world`), judged before any campaign places it — no
        // horizon, no volume, no gate; the piece's ladder is the question.
        ("grammar/contract.rs", 1),
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

/// spec-0113 A9: **every reader that floods a private relation instead of the
/// reach graph, enumerated** — every production call of the cell relation
/// `moves_of` and of the walk relation's floods (`walk_region`,
/// `walk_region_rooted`). A route proof walks on cue and reads the walk
/// relation (§4.5); a mob's moves are not a player's (§4.6); every other
/// "where can a body be" question reads the graph. A new call reds this test
/// until it is listed here, with its reason at the call site.
#[test]
fn the_graph_declines_are_enumerated() {
    let src = common::repo_root().join("crates/delvec/src");
    let files = common::source_scan::production_sources(&src);
    let mut found: Vec<(String, &str, usize)> = Vec::new();
    for (rel, prod) in &files {
        for pat in ["moves_of(", ".walk_region(", ".walk_region_rooted("] {
            let n = prod.matches(pat).count();
            if n > 0 {
                found.push((rel.clone(), pat, n));
            }
        }
    }
    found.sort();
    // (file under `src/`, the call, how many production sites) — each with
    // the question that keeps it off the graph.
    const EXPECTED: &[(&str, &str, usize)] = &[
        // The stage-5 battery's "two places joined where no seam was
        // allocated" (`DW0838`) reads cells of a box, not an assembled world:
        // spec-0113 §5.1's third wave, not moved until the graph is built
        // over a box.
        ("compiler/blockout.rs", ".walk_region(", 1),
        // `DW0937`: the critical path performs the strike, so the question is
        // whether the party walks on cue to a cell it strikes from (§4.5);
        // and the bot's spared stand beside it, a cell it walks to on cue.
        ("compiler/emit/proofs.rs", ".walk_region(", 2),
        // `holders::check_stance`: the stance a holder `collect` is taken
        // from, where the critical path's leg ends (spec-0111 §5.2).
        ("compiler/emit/mod.rs", ".walk_region_rooted(", 1),
        // `DW0920`: where the party is walked to engage a fight in water.
        ("compiler/engage.rs", ".walk_region(", 1),
        // `DW0315`/`DW0316`: checkpoints and respawns (§4.5).
        ("compiler/nav/checkpoint.rs", ".walk_region(", 1),
        // A timed hazard's pre-commitment ground: where the party walks
        // before it commits to the span, on cue.
        ("compiler/nav/hazard.rs", ".walk_region(", 1),
        // `DW1041`'s `not a jump`: whether the walk already joins the two
        // cells of a declared jump.
        ("compiler/nav/jump.rs", ".walk_region(", 1),
        // `reach_into_volumes`' player arm, the vacuity half of `DW0891`
        // ("a volume no modelled body reaches"): the cell relation, not yet
        // the graph — recorded debt. Its direction: it can only under-count
        // the volumes a body reaches, which adds a zero-binding warning and
        // never lets a hazard ship unjudged.
        ("compiler/nav/lethal.rs", "moves_of(", 1),
        // The route proofs' own region model (`DW0311` and family, §4.5).
        ("compiler/nav/route/region.rs", ".walk_region(", 2),
        // `DW0327`/`DW0355`: the stealth zone and its onset sprint (§4.5).
        ("compiler/nav/stealth.rs", ".walk_region(", 1),
        // The cell relation's definition, and the mob relation over it
        // (§4.6).
        ("compiler/nav/world/body.rs", "moves_of(", 2),
        // The reach graph's own cheap edges, taken before it flies.
        ("compiler/nav/world/reach.rs", "moves_of(", 1),
        // A stake's way home: the route the party walks back on cue.
        ("compiler/stake.rs", ".walk_region(", 1),
    ];
    let expected: Vec<(String, &str, usize)> = EXPECTED
        .iter()
        .map(|(f, p, n)| ((*f).to_string(), *p, *n))
        .collect();
    assert_eq!(
        found,
        expected,
        "a production reader floods the cell or walk relation somewhere this list does not \
         name. If its question is where a body can be, it reads the reach graph \
         (`World::body_reach`); if it walks on cue or moves a mob, say so at the call site and \
         add it here ({} source file(s) examined)",
        files.len()
    );
}
