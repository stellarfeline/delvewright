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
        // which carry their own sealing authority, and `DW0838`'s world with
        // every allocated opening walled, which is `open` with the plan's
        // ways deleted (all three take the body's premises,
        // `Premises::with_body_of`); and the stairwell pass,
        // which asks only whether one step between two courses of a stair
        // is a body move over the mass as laid — a question about geometry
        // inside the stair's own box, with no campaign premise to consult;
        // and `DW0838`'s world without the commons, which is the opening-walled
        // world with the commons' floor walled too (also `with_body_of`).
        ("compiler/blockout.rs", 5),
        // `edit.rs`: a `relight` verb's own darkness survey.
        ("compiler/edit.rs", 1),
        // `light.rs`: the relight pass's darkness survey.
        ("compiler/light.rs", 1),
        // `compiler/nav/world/mod.rs`: the synthetic constructors' own door
        // (`from_solid_and_flooded`), which every unit-test world goes
        // through; and `World::of_piece`, ONE piece's own blocks judged before
        // any campaign places it — no horizon, no volume, no gate; the piece's
        // blocks are the question. Every per-piece reader goes through it: the
        // grammar's gates and edge proofs (`grammar::nav::body_world`), the
        // sculpt's pocket proof and entry walk, the light probe and the
        // enclosure survey.
        ("compiler/nav/world/mod.rs", 2),
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
/// The question a reader that keeps off the reach graph answers instead —
/// each one a question that is not "can the player's body reach this cell".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Question {
    /// The route the bot is driven along on cue: a leg the critical path
    /// performs, or one the harness walks (§4.5).
    BotRoute,
    /// Whether one move is the walk relation's, or the relation's own
    /// definition and a mob's moves over it (§4.6).
    StepLegality,
}

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
    const EXPECTED: &[(&str, &str, usize, Question)] = &[
        // The bot's own route. `DW0937`: the critical path performs the
        // strike, so the question is whether the party walks on cue to a cell
        // it strikes from (§4.5); and the bot's spared stand beside it, a cell
        // the bot is walked to on cue.
        (
            "compiler/emit/proofs.rs",
            ".walk_region(",
            2,
            Question::BotRoute,
        ),
        // The bot's own route. `DW0920`: the fight is a `kill` the critical
        // path performs, so the question is whether the party, walked on cue
        // to where the wave is seated, gets its feet wet there.
        ("compiler/engage.rs", ".walk_region(", 1, Question::BotRoute),
        // The bot's own route. `holders::walk_world`: the stance a holder
        // `collect` is taken from, where the critical path's leg ends and the
        // bot takes the item (spec-0111 §5.2).
        (
            "compiler/holders.rs",
            ".walk_region_rooted(",
            1,
            Question::BotRoute,
        ),
        // A single step's legality. `DW1041`'s `not a jump`: whether the
        // route relation already holds the edge a declared jump adds
        // (§5.2), which the launch set never answers.
        (
            "compiler/nav/jump.rs",
            ".walk_region(",
            1,
            Question::StepLegality,
        ),
        // The bot's own route. The recovery stake's way home
        // (`reachable_under_every_quest_state` and `seat_reach`, whose one
        // reader is `compiler/stake.rs`): the route the party walks back on
        // cue to its stake.
        (
            "compiler/nav/route/region.rs",
            ".walk_region(",
            2,
            Question::BotRoute,
        ),
        // The bot's own route. `DW0327`: the party is walked to cover on the
        // stealth beat's sneak leg, and `DW0355` times that same walk (§4.5),
        // so the pair reads one relation.
        (
            "compiler/nav/stealth.rs",
            ".walk_region(",
            1,
            Question::BotRoute,
        ),
        // A single step's legality: the cell relation's definition, and a
        // mob's movement over it (§4.6).
        (
            "compiler/nav/world/body.rs",
            "moves_of(",
            2,
            Question::StepLegality,
        ),
        // A single step's legality: the reach graph's own cheap edges, taken
        // before it flies.
        (
            "compiler/nav/world/reach.rs",
            "moves_of(",
            1,
            Question::StepLegality,
        ),
        // The bot's own route. A stake's way home, from a seat that is not the
        // entry: the route the party walks back on cue, the domain read by the
        // same relation.
        ("compiler/stake.rs", ".walk_region(", 1, Question::BotRoute),
    ];
    let expected: Vec<(String, &str, usize)> = EXPECTED
        .iter()
        .map(|(f, p, n, _)| ((*f).to_string(), *p, *n))
        .collect();
    let of = |q: Question| {
        EXPECTED
            .iter()
            .filter(|e| e.3 == q)
            .map(|e| e.2)
            .sum::<usize>()
    };
    eprintln!(
        "graph declines: {} production site(s) off the reach graph, {} on the bot's own route, \
         {} a single step's legality",
        EXPECTED.iter().map(|e| e.2).sum::<usize>(),
        of(Question::BotRoute),
        of(Question::StepLegality)
    );
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
