//! The ported rule libraries expand, and expand into the buildings they claim.

use delvewright_grammar::block::BlockState;
use delvewright_grammar::ir::{Paint, Program, WeightedBlock};
use delvewright_grammar::library;
use delvewright_grammar::library::{castle, church, temple};
use delvewright_grammar::{Box3, ExpandOptions, expand};

/// The regions the individual claims below are made at. Every SWEEP reads its
/// region off `library::PROGRAMS`; what is left here is the handful a single
/// named test measures one building against.
const TEMPLE_REGION: Box3 = Box3::at_origin([13, 14, 21]);
const CHURCH_REGION: Box3 = Box3::at_origin([15, 16, 30]);

/// **Every program the registry carries, at the expansion it declares.**
///
/// Read off `library::PROGRAMS`, never restated. This used to be a hand-written
/// list of twenty-two of the thirty-three, with the other ten in
/// `tests/idioms.rs` and `negated-guard` in neither, so no sweep here ever saw
/// the whole corpus. Now a program cannot enter the registry without its
/// region, and every sweep below is bound to all of it by construction.
fn programs() -> Vec<(Program, Box3)> {
    library::PROGRAMS
        .iter()
        .map(|p| ((p.build)(), Box3::at_origin(p.region)))
        .collect()
}

#[test]
fn every_library_program_is_structurally_valid() {
    for (program, _) in programs() {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
    }
}

/// Every program that builds a PIECE — the vocabulary, not the teaching set.
///
/// The class comes off the registry entry, never off the id: `negated-guard` is
/// a language example and carries no `idiom-` prefix, and a filter keyed on the
/// prefix asserted "that is a cube, not a building" against a program whose
/// whole job is to fill its box. It matters for exactly one claim below — a
/// teaching program's job is to show one construct, and `idiom-mirror` filling
/// its 15 x 11 x 2 slab solid is the demonstration working.
fn pieces() -> Vec<(Program, Box3)> {
    library::PROGRAMS
        .iter()
        .filter(|p| p.kind == library::Kind::Piece)
        .map(|p| ((p.build)(), Box3::at_origin(p.region)))
        .collect()
}

#[test]
fn every_library_piece_builds_something_inside_its_box() {
    let mut judged = 0usize;
    for (program, region) in pieces() {
        let out = expand(&program, region, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
        let filled = out.model.filled_cells();
        let volume = region.volume() as usize;
        assert!(
            filled > volume / 50,
            "{} filled only {filled} of {volume} cells",
            program.name
        );
        assert!(
            filled < volume,
            "{} filled the whole box — that is a cube, not a building",
            program.name
        );
        assert_eq!(out.model.region(), region);
        judged += 1;
    }
    assert_eq!(
        judged, 22,
        "the vocabulary this swept — 33 less the 11 language examples"
    );
}

/// **Every program in the registry gives the verdict the record says it gives,
/// at the expansion it declares** — the sweep `delvec grammar audit --library`
/// runs, run here so `cargo test` carries it too.
///
/// One program is recorded red: `causeway`'s flood is not contained, which is a
/// missing `nav` capability rather than a defect a session could fix
/// ([`UNCONTAINED_LIBRARY_RULES`], and `.github/zone-audit-exclusions.json`,
/// which is what the CLI sweep reads). The assertion is INVERTED for it rather
/// than dropped: a recorded program that starts passing reds this test, and so
/// does any other program that fails anything. That is the same discrimination
/// in both directions the exclusions record gives the CLI, and it is why this
/// test does not say "every gate passes" — it would be false, and the way to
/// make it true would be to stop judging the one piece that fails.
///
/// This replaces a `rules_applied > 5` assertion that used to sit in the sweep
/// above. That number measured how MUCH derivation happened rather than how far
/// it reached, and it was tuned against the twenty-two building-scale programs
/// the sweep saw when it was hand-written; registry-driven, the sweep also
/// reaches the teaching set, where `idiom-erosion` covers its whole box with one
/// rule. It was not lowered — it was a proxy that is false as a general claim,
/// and what stands in its place is strictly stronger: the full gate report,
/// every gate, every program, with the binding counts asserted.
#[test]
fn every_library_program_gives_the_recorded_verdict_at_its_declared_expansion() {
    use delvewright_grammar::gates;
    let mut totals: std::collections::BTreeMap<&str, usize> = Default::default();
    for entry in library::PROGRAMS {
        let program = (entry.build)();
        let out = expand(
            &program,
            Box3::at_origin(entry.region),
            &ExpandOptions::seeded(entry.seed),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", entry.id));
        let report = gates::judge(&out, entry.gates);
        for gate in &report.gates {
            // The one inversion, and it is exact in both directions: this gate
            // on this program MUST be red, everything else MUST be green.
            let recorded_red =
                gate.id == "fluid-contained" && UNCONTAINED_LIBRARY_RULES.contains(&entry.id);
            assert_eq!(
                gate.passed(),
                !recorded_red,
                "{}: `{}` — {}{}",
                entry.id,
                gate.id,
                gate.detail,
                if recorded_red {
                    ". This program is RECORDED red on this gate. It passing means the capability \
                     gap closed, and this record plus its note in \
                     `.github/zone-audit-exclusions.json` must go with it"
                } else {
                    ""
                }
            );
            assert!(
                gate.bound > 0,
                "{}: gate `{}` examined zero objects",
                entry.id,
                gate.id
            );
            *totals.entry(gate.id).or_default() += gate.bound;
        }
    }
    // Binding counts, so a corpus that quietly stopped reaching these gates is
    // a red rather than a shorter green list (CLAUDE.md's first vacuity mode).
    assert_eq!(library::PROGRAMS.len(), 36, "the corpus this swept");
    for (id, floor) in [
        ("blocks-exist", 100usize),
        ("shape-complete", 100),
        ("states-complete", 100),
        ("oriented-fills", 600),
        ("non-empty", 40000),
        // `decorated-room` is the corpus's only `reachable_floor` claim, so
        // this floor is the whole of that gate's binding: it went from zero —
        // green because nothing asked it anything — to one program's roofed
        // floor.
        ("reachable-floor", 80),
    ] {
        let bound = *totals.get(id).unwrap_or(&0);
        assert!(
            bound >= floor,
            "gate `{id}` bound {bound}, expected >= {floor}"
        );
    }
    // The route claim binds to something: a corpus in which no entry claims a
    // route would run this gate zero times and read as a pass.
    assert!(
        *totals.get("traversable").unwrap_or(&0) > 0,
        "no library entry claims a route, so the walk gate examined nothing"
    );
}

/// Count the distinct column runs along the temple's front colonnade.
fn temple_columns(program: &Program, depth: u32) -> usize {
    // The peristyle is a gap/column rhythm along Z, so a deeper box gets more
    // columns. Count the distinct solid runs along the front row.
    let region = Box3::at_origin([13, 14, depth]);
    let out = expand(program, region, &ExpandOptions::seeded(1)).unwrap();
    let mut runs = 0;
    let mut prev_solid = false;
    for z in 0..depth as i32 {
        let solid = !out.model.get([1, 4, z]).unwrap().is_air();
        if solid && !prev_solid {
            runs += 1;
        }
        prev_solid = solid;
    }
    runs
}

#[test]
fn the_temple_has_a_colonnade_that_follows_the_box() {
    let program = temple();
    let shallow = temple_columns(&program, 15);
    let deep = temple_columns(&program, 29);
    assert!(
        deep > shallow && shallow >= 3,
        "colonnade did not follow the box: {shallow} vs {deep}"
    );
}

/// `library/temple.rs` claims the port diverges from upstream — which fixes the
/// colonnade at four columns — *without* losing upstream's building: "four
/// columns across a nine-deep box reproduces upstream exactly". That claim was
/// prose, so it could rot silently the next time the `columns` rule is touched.
/// Here it is arithmetic: a nine-deep box, at the default `column_size` of 1,
/// gives exactly the tetrastyle the paper's own figure shows.
#[test]
fn a_nine_deep_box_reproduces_upstreams_four_columns() {
    let mut program = temple();
    assert_eq!(program.params["column_size"], 1, "upstream's column width");
    assert_eq!(
        temple_columns(&program, 9),
        4,
        "the divergence note promises upstream's tetrastyle at depth 9"
    );
    // And the rhythm is genuinely `column_size`-driven, not a coincidence of 9:
    // doubling the thickness halves what fits.
    program.set_param("column_size", 2).unwrap();
    assert_eq!(temple_columns(&program, 9), 3);
}

#[test]
fn the_church_lays_directional_stairs_and_a_two_half_door() {
    let out = expand(&church(), CHURCH_REGION, &ExpandOptions::seeded(1)).unwrap();
    let names: Vec<String> = out.model.palette().iter().map(|b| b.to_string()).collect();
    // Block states, not bare ids: the roof needs facings and the door needs
    // halves. This is the assertion behind "block states from day one".
    let stairs: Vec<_> = names.iter().filter(|n| n.contains("oak_stairs")).collect();
    assert!(
        stairs.iter().any(|n| n.contains("facing=")),
        "roof stairs lost their facing: {names:?}"
    );
    let door_halves: Vec<_> = names.iter().filter(|n| n.contains("oak_door")).collect();
    assert_eq!(
        door_halves.len(),
        2,
        "a door is two halves, got {door_halves:?}"
    );
    assert!(names.iter().any(|n| n.contains("glass")), "{names:?}");
}

#[test]
fn undersized_regions_are_loud() {
    // Upstream prints a warning and writes blocks outside the box, or silently
    // voids the scope. Both are failures we would only find by looking.
    let err = expand(
        &temple(),
        Box3::at_origin([13, 6, 21]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("too small"),
        "expected a sizing diagnostic, got: {err}"
    );

    let err = expand(
        &castle(),
        Box3::at_origin([12, 14, 12]),
        &ExpandOptions::seeded(1),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected an unsatisfied-guard diagnostic, got: {err}"
    );
}

#[test]
fn the_documented_minimum_regions_are_the_real_ones() {
    // `docs/reference/grammar.md` and each library module state the smallest box
    // their program expands in. Documented numbers drift; these hold them to the
    // code from both sides — the minimum works, one block less does not.
    fn check(name: &str, program: &Program, smallest: [u32; 3], too_small: &[[u32; 3]]) {
        expand(
            program,
            Box3::at_origin(smallest),
            &ExpandOptions::seeded(1),
        )
        .unwrap_or_else(|e| panic!("{name} should expand at its documented minimum: {e}"));
        for &size in too_small {
            assert!(
                expand(program, Box3::at_origin(size), &ExpandOptions::seeded(1)).is_err(),
                "{name} expanded at {size:?}, below its documented minimum {smallest:?}"
            );
        }
    }

    // temple: 6 + 2*column_size in X, 1 + column_height + 5 in Y, 7 in Z.
    check(
        "temple",
        &temple(),
        [8, 14, 7],
        &[[7, 14, 7], [8, 13, 7], [8, 14, 6]],
    );
    // castle: both horizontal extents 2*large_tower + 2, tower_height + 1 in Y.
    check(
        "castle",
        &castle(),
        [20, 9, 20],
        &[[19, 9, 20], [20, 8, 20], [20, 9, 19]],
    );

    // church: no fixed minimum — the roof's height has to follow the nave's
    // width, because it steps in two blocks per course.
    for (width, min_height) in [(9u32, 9u32), (15, 12), (21, 18)] {
        let ok = Box3::at_origin([width, min_height, 30]);
        expand(&church(), ok, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("church {width}x{min_height}: {e}"));
        let squat = Box3::at_origin([width, min_height - 1, 30]);
        assert!(
            expand(&church(), squat, &ExpandOptions::seeded(1)).is_err(),
            "church expanded at {width} wide and only {} tall",
            min_height - 1
        );
    }
}

#[test]
fn programs_round_trip_through_json() {
    for (program, region) in programs() {
        let json = serde_json::to_string_pretty(&program).unwrap();
        let back: Program = serde_json::from_str(&json).unwrap();
        assert_eq!(back, program, "{} did not survive JSON", program.name);
        // and the round trip is stable, not merely lossless
        assert_eq!(serde_json::to_string_pretty(&back).unwrap(), json);
        // ...and the deserialised program expands to the same blocks.
        let a = expand(&program, region, &ExpandOptions::seeded(3)).unwrap();
        let b = expand(&back, region, &ExpandOptions::seeded(3)).unwrap();
        assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    }
}

#[test]
fn the_json_form_is_the_one_an_author_would_write() {
    let json = serde_json::to_value(temple()).unwrap();
    assert_eq!(json["palette"]["marble"], "minecraft:quartz_block");
    assert_eq!(json["params"]["column_height"], 8);
    let back_wall = &json["rules"]["back_wall"][0]["body"];
    assert_eq!(back_wall["op"], "split");
    assert_eq!(back_wall["axis"], "z");
    assert_eq!(
        back_wall["sizes"][0],
        serde_json::json!({"size": "absolute", "blocks": {"expr": "int", "value": 1}})
    );
    assert_eq!(back_wall["children"][0]["op"], "void");
}

#[test]
fn a_palette_swap_restyles_without_touching_a_rule() {
    let mut sandstone = temple();
    sandstone
        .set_role(
            "marble",
            Paint::block(BlockState::simple("smooth_sandstone")),
        )
        .unwrap();
    let marble = expand(&temple(), TEMPLE_REGION, &ExpandOptions::seeded(1)).unwrap();
    let sand = expand(&sandstone, TEMPLE_REGION, &ExpandOptions::seeded(1)).unwrap();
    assert_eq!(
        marble.model.filled_cells(),
        sand.model.filled_cells(),
        "a restyle must not move a single block"
    );
    assert!(
        sand.model
            .palette()
            .iter()
            .any(|b| b.name == "minecraft:smooth_sandstone")
    );
    assert_ne!(marble.model.canonical_bytes(), sand.model.canonical_bytes());
}

#[test]
fn a_weathered_palette_mixes_per_cell_under_the_seed() {
    let mut weathered = temple();
    weathered
        .set_role(
            "marble",
            Paint::mix(vec![
                WeightedBlock {
                    weight: 6,
                    block: BlockState::simple("quartz_block"),
                },
                WeightedBlock {
                    weight: 1,
                    block: BlockState::simple("cracked_stone_bricks"),
                },
            ]),
        )
        .unwrap();
    let a = expand(&weathered, TEMPLE_REGION, &ExpandOptions::seeded(11)).unwrap();
    let b = expand(&weathered, TEMPLE_REGION, &ExpandOptions::seeded(12)).unwrap();
    assert!(
        a.model
            .palette()
            .iter()
            .any(|x| x.name == "minecraft:cracked_stone_bricks")
    );
    assert_ne!(
        a.model.canonical_bytes(),
        b.model.canonical_bytes(),
        "a per-cell mix must follow the seed"
    );
    assert_eq!(
        a.model.filled_cells(),
        b.model.filled_cells(),
        "...but only the blocks change, never the geometry"
    );
}

#[test]
fn unknown_knobs_are_refused_rather_than_ignored() {
    let mut program = temple();
    assert!(program.set_param("colum_height", 12).is_err());
    assert!(
        program
            .set_role("marbel", Paint::block(BlockState::air()))
            .is_err()
    );
    assert_eq!(program, temple(), "a refused override changes nothing");
}

/// **Every block every library program paints is a real 1.21.11 block.**
///
/// The gate the export enforces (`ExportError::UnknownBlocks`), asserted over
/// the whole library rather than over one program, because the defect it
/// catches is silent: a structure template loads an unknown id as AIR, so a
/// mistyped or renamed block costs the piece and reports nothing. Eight cells
/// of `minecraft:chain` — renamed `iron_chain` in 1.21.11 — shipped inside
/// `tk-bell-tower.nbt` for exactly that reason.
///
/// The binding count is asserted, not just the emptiness of the failure list: a
/// green that examined zero block states would be vacuous (CLAUDE.md).
#[test]
fn every_library_program_paints_only_blocks_that_exist() {
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    let mut examined = 0usize;
    let mut bad: Vec<String> = Vec::new();
    for (program, region) in programs() {
        let out = expand(&program, region, &ExpandOptions::seeded(4)).unwrap();
        for state in out.model.palette() {
            examined += 1;
            if let Err(e) = registry.validate(&state.name, &state.properties) {
                bad.push(format!("{}: {e}", program.name));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
    assert!(
        examined >= 60,
        "the gate examined only {examined} block states — it is bound to almost nothing"
    );
}

/// **The one library rule whose body of water is not contained** — a live
/// finding of `DW0800`, pinned here rather than hidden by it.
///
/// `causeway` is a flooded ward with a 1-wide raised spine through it, and the
/// flood is water from the ward floor to the ceiling on both flanks of that
/// spine. In the game the water runs sideways into the spine's air column at
/// every level and forward into the guard station's open interior: 134 ways
/// out of a body of 252 sources, so the causeway a player would walk is
/// flooded and so is the post that watches it. Nothing upstream of a server
/// could see it — a render draws still water — which is why the rule this
/// fixture belongs to exists.
///
/// It is NOT repaired here, and the reason is a capability rather than effort.
/// The piece fills its flood to the ceiling on purpose: `crate::nav`'s
/// `standable` treats any non-air cell as floor, so water with a body-height
/// air pocket over it reads as a walkable surface, and the ward's whole claim
/// ("off the spline there is nothing to stand on") would go green while being
/// false. Lowering the waterline — the only repair that keeps the design —
/// needs `nav` to know that a body cannot stand on water, which is spec-0038
/// §2.1's rule one layer over and its own round of work with its own
/// playtest gate.
///
/// The pin is exact and falsifiable in BOTH directions: a second library rule
/// that leaks reds this test, and so does repairing this one. It cannot
/// quietly absorb a new defect and it cannot outlive the fix.
const UNCONTAINED_LIBRARY_RULES: [&str; 1] = ["causeway"];

/// **Both settling gates over the whole corpus** (`DW0801` stair shapes,
/// `DW0800` bodies of fluid), at each program's documented region.
///
/// This is the invocation that keeps them from being UNRUN over the library
/// (CLAUDE.md's fourth vacuity mode), and it states what they BIND here rather
/// than implying it: stairs are bound by the church's roof courses, and fluid
/// by the causeway's flood. Every zero binding is asserted to be NAMED in its
/// own report — the vacuity rule enforced over the corpus instead of trusted.
#[test]
fn every_library_program_passes_the_settling_gates() {
    use delvewright_grammar::gates;
    let (mut stairs_bound, mut fluid_bound, mut zero_bindings) = (0usize, 0usize, 0usize);
    let corpus = programs();
    let swept = corpus.len();
    let mut uncontained: Vec<String> = Vec::new();
    for (program, region) in corpus {
        let out = expand(&program, region, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
        let report = gates::judge(&out, gates::Options::default());
        stairs_bound += report.measurements.stairs;
        fluid_bound += report.measurements.fluid_cells;
        for (id, present) in [
            ("stair-shape", report.measurements.stairs),
            ("fluid-contained", report.measurements.fluid_cells),
        ] {
            match (report.gates.iter().find(|g| g.id == id), present) {
                // Nothing to judge: the rule must claim NO verdict rather than a
                // green one, and its count stands as a measurement.
                (None, 0) => zero_bindings += 1,
                (None, n) => panic!("{}: `{id}` said nothing about {n} object(s)", program.name),
                (Some(_), 0) => {
                    panic!("{}: `{id}` claimed a verdict over nothing", program.name)
                }
                (Some(g), n) => {
                    assert_eq!(g.bound, n, "{}: `{id}` binding", program.name);
                    if !g.passed() {
                        assert_ne!(id, "stair-shape", "{}: {}", program.name, g.detail);
                        uncontained.push(program.name.clone());
                    }
                }
            }
        }
    }
    assert_eq!(
        uncontained, UNCONTAINED_LIBRARY_RULES,
        "the set of library rules whose fluid runs is pinned. A NEW name here is a new leak; a \
         MISSING one means the finding was repaired and this pin plus its note must go with it"
    );
    assert!(
        stairs_bound > 0,
        "the stair gate examined ZERO stairs across {swept} library program(s)"
    );
    assert!(
        fluid_bound > 0,
        "the fluid gate examined ZERO fluid cells across {swept} library program(s)"
    );
    assert!(
        zero_bindings > 0,
        "not one program in {swept} lacks a stair or a drop of fluid — the zero-binding half of \
         this test bound to nothing"
    );
}

/// **The two doors give the same verdict, over the whole corpus.**
///
/// `expand` and `audit` used to disagree about what a binding of zero means:
/// the gate report raised one as a finding and passed, while the corpus audit
/// folded every `bound == 0` into its red set — so the door a creator runs on
/// their own machine was the WEAKER of the two, which is the worse direction
/// for two authorities to disagree in. The audit's own filter now reads nothing
/// but `Gate::failed()`, which is sound exactly while this holds: **no report
/// carries a gate that is green over nothing.**
///
/// Asserted over every program at its declared expansion rather than on a
/// fixture, because the defect it guards is a gate someone adds later without
/// deciding what its emptiness means.
///
/// **What this test does and does not demonstrate, stated because the
/// difference is the whole subject.** Its per-gate predicate has NO live
/// instance in this corpus: no library program binds a gate to zero today, so
/// disabling the rule it guards leaves this sweep green. That is an unbound
/// predicate — a gate bound to plenty of objects whose *test* bit on none of
/// them — and naming it is the obligation rather than an excuse. The live
/// red→green demonstrations are the fixture pair in `tests/contract_check.rs`,
/// which do go red when the rule is removed; this sweep is the regression guard
/// that stops a NEW gate or program introducing a green-over-nothing silently.
///
/// So both counts are pinned rather than merely asserted non-zero. A zero-bound
/// gate appearing in this corpus, or a gate starting to be withheld in it, is
/// something to come and look at — not something to discover later.
#[test]
fn no_library_program_reports_a_gate_that_is_green_over_nothing() {
    use delvewright_grammar::gates;
    let (mut programs_swept, mut gates_seen) = (0usize, 0usize);
    let (mut zero_bound, mut withheld) = (0usize, 0usize);
    for entry in library::PROGRAMS {
        let program = (entry.build)();
        let out = expand(
            &program,
            Box3::at_origin(entry.region),
            &ExpandOptions::seeded(entry.seed),
        )
        .unwrap_or_else(|e| panic!("{}: {e}", entry.id));
        let report = gates::judge(&out, entry.gates);
        programs_swept += 1;
        for gate in &report.gates {
            gates_seen += 1;
            zero_bound += usize::from(gate.bound == 0);
            assert!(
                !(gate.bound == 0 && gate.passed()),
                "{}: gate `{}` is green over a binding of ZERO. Either its emptiness is honest, \
                 in which case it computes an `empty_ok` and is withheld, or it is not, in which \
                 case it reds — a green over nothing is what made the corpus audit disagree with \
                 the creator's own door. {}",
                entry.id,
                gate.id,
                gate.detail
            );
        }
        // A withheld gate never vanishes quietly: whatever was struck from the
        // verdict list says so in the enumeration, which is a list a reviewer
        // reads rather than a count a script can satisfy.
        for line in &report.enumeration {
            if line.contains("is not emitted") {
                withheld += 1;
                assert!(
                    line.contains("gate `"),
                    "{}: a withheld gate must name itself: {line}",
                    entry.id
                );
            }
        }
    }
    assert!(
        programs_swept >= 30 && gates_seen >= 150,
        "the sweep saw {programs_swept} program(s) and {gates_seen} gate(s) — it bound to almost \
         nothing, which is the rule it is asserting failing on itself"
    );
    assert_eq!(
        (zero_bound, withheld),
        (0, 0),
        "pinned: no library program binds a gate to zero, and none has a gate withheld. A change \
         here is not a failure — it is the first live instance in this corpus of the rule this \
         test guards, and it wants reading before the pin is moved"
    );
}

/// The other two members of the blockstate family (`DW0735`, `DW0736`),
/// asserted over the whole corpus the same way: every library program judges
/// green on `shape-complete` and `oriented-fills` at its documented region.
///
/// This is the invocation that keeps the two gates from being UNRUN over the
/// corpus (CLAUDE.md's fourth vacuity mode): a new library piece that fills a
/// bare wall/fence/pane, or an oriented state without its `orientation` guard,
/// reds here — which is exactly how `broken_grate`'s bare `iron_bars` (an
/// isolated-post row, shipped since the piece landed) was caught and fixed.
/// Binding counts are summed and asserted, per the vacuity rule.
#[test]
fn every_library_program_passes_the_shape_and_orientation_gates() {
    use delvewright_grammar::gates;
    let (mut states_bound, mut fills_bound, mut carrying) = (0usize, 0usize, 0u64);
    for (program, region) in programs() {
        let out = expand(&program, region, &ExpandOptions::seeded(1))
            .unwrap_or_else(|e| panic!("{}: {e}", program.name));
        carrying += out.oriented.carrying;
        let report = gates::judge(&out, gates::Options::default());
        for id in ["shape-complete", "oriented-fills"] {
            let gate = report
                .gates
                .iter()
                .find(|g| g.id == id)
                .unwrap_or_else(|| panic!("{}: no `{id}` gate", program.name));
            assert!(gate.passed(), "{}: {}", program.name, gate.detail);
            assert!(
                gate.bound > 0,
                "{}: gate `{id}` examined zero objects",
                program.name
            );
            match id {
                "shape-complete" => states_bound += gate.bound,
                _ => fills_bound += gate.bound,
            }
        }
    }
    assert!(
        states_bound >= 60 && fills_bound >= 100,
        "the sweep bound {states_bound} placed states / {fills_bound} fills — almost nothing"
    );
    assert!(
        carrying >= 3,
        "only {carrying} fills in the whole corpus carried block-state properties — the \
         oriented predicate had almost nothing to bite on"
    );
}
