//! **A gated connection is built shut** (spec-0107).
//!
//! A connection the layout graph gates is shut in blocks from world load: the
//! derivation stands the bar where §6 names it for the connection's kind, and
//! the proofs read the bytes. Each test here is half of a red→green pair over
//! one declaration: the cells are asserted from numbers read off the
//! allocation (`delvec allocation`, the frame's `world_min` plus the handed
//! piece-local cells), never from the function that places the bar.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use delvec::compiler::blockout;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::Campaign;

/// The bar of a wall, the hatch of a floor, the cover over a ladder's hole
/// (spec-0107 §6): the three blocks a shut seam is built of.
const BAR: &str = delvewright_dsl::siteplan::SEAM_BAR;
const HATCH: &str = delvewright_dsl::siteplan::SEAM_HATCH;
const COVER: &str = delvewright_dsl::siteplan::SEAM_COVER;

/// The final block map a derivation writes, replayed in order (air removes).
fn mass_map(c: &Campaign) -> BTreeMap<[i64; 3], String> {
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let (placement, _) = blockout::derive(c, &mut reads).expect("a site-plan campaign derives");
    let mut out = BTreeMap::new();
    for f in &placement.mass {
        let lo = [
            i64::from(f.from[0]),
            i64::from(f.from[1]),
            i64::from(f.from[2]),
        ];
        let hi = [i64::from(f.to[0]), i64::from(f.to[1]), i64::from(f.to[2])];
        for cell in blockout::cells_of(lo, hi) {
            if f.block == "minecraft:air" {
                out.remove(&cell);
            } else {
                out.insert(cell, f.block.clone());
            }
        }
    }
    out
}

fn derivation_line(c: &Campaign) -> String {
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let (_, b) = blockout::derive(c, &mut reads).expect("a site-plan campaign derives");
    b.binding.line()
}

/// The gallery's site-plan point with every place a stand-in, so every bar is
/// the derivation's to write. `tag` names the scratch copy.
fn gallery_stand_ins(tag: &str) -> Campaign {
    let dir = std::env::temp_dir().join(format!("dw-gated-connection-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::gallery_site_plan_at(&dir);
    let mut c = common::campaign_at(&dir);
    c.detail_plan = None;
    c
}

fn ungate(c: &Campaign, edge: &str) -> Campaign {
    let mut c = c.clone();
    let g = &mut c.layout_graph.as_mut().expect("a graph").content;
    for e in &mut g.edges {
        if e.id().0 != edge {
            continue;
        }
        match e {
            delvewright_dsl::layout::Edge::Walk { gating, .. }
            | delvewright_dsl::layout::Edge::Stair { gating, .. }
            | delvewright_dsl::layout::Edge::Climb { gating, .. }
            | delvewright_dsl::layout::Edge::Drop { gating, .. } => *gating = None,
            _ => panic!("`{edge}` is a traversal connection"),
        }
    }
    c
}

fn cells(lo: [i64; 3], hi: [i64; 3]) -> BTreeSet<[i64; 3]> {
    blockout::cells_of(lo, hi).collect()
}

/// A connection, the cells its bar stands in, the opening the plan allocated,
/// and the block the bar is built of.
type GatedCells = (
    &'static str,
    BTreeSet<[i64; 3]>,
    BTreeSet<[i64; 3]>,
    &'static str,
);

/// The four gated connections of spec-0107 §1, each with the cells §6 names for
/// its kind and the opening the plan allocated — read off `delvec allocation`
/// over the materialised point (world = `world_min` + piece-local).
fn the_four() -> Vec<GatedCells> {
    vec![
        // A walk in a wall: the arch's own cells.
        (
            "edge/annex-chute",
            cells([20, 64, 22], [20, 66, 23]),
            cells([20, 64, 22], [20, 66, 23]),
            BAR,
        ),
        // A stair's head in a wall: the passage's cells.
        (
            "edge/loft-stair",
            cells([41, 69, 20], [43, 71, 20]),
            cells([41, 69, 20], [43, 71, 20]),
            BAR,
        ),
        // A drop through a floor: the hole's own cells, the hatch.
        (
            "edge/undercroft-shaft",
            cells([25, 63, 16], [25, 63, 17]),
            cells([25, 63, 16], [25, 63, 17]),
            HATCH,
        ),
        // A climb through a floor: the course ABOVE the hole, over its
        // footprint; the hole keeps its rung.
        (
            "edge/undercroft-crawl",
            cells([28, 64, 16], [30, 64, 18]),
            cells([28, 63, 16], [30, 63, 18]),
            COVER,
        ),
    ]
}

/// **Criterion 2.** For each of the four, the derived bytes hold the bar at
/// exactly §6's cells and nowhere else in the opening; ungating the connection
/// moves those bytes to air and nothing else; the derivation's binding line
/// counts every shut seam, and a plan with none states `0 shut`.
#[test]
fn each_gated_connection_is_built_shut_at_its_kinds_cells() {
    let c = gallery_stand_ins("built-shut");
    let shut = mass_map(&c);
    for (edge, bar, opening, block) in the_four() {
        for cell in &bar {
            assert_eq!(
                shut.get(cell).map(String::as_str),
                Some(block),
                "`{edge}`: the bar stands at {cell:?}"
            );
        }
        for cell in opening.difference(&bar) {
            assert_ne!(
                shut.get(cell).map(String::as_str),
                Some(block),
                "`{edge}`: {cell:?} is the opening, not the bar"
            );
        }
        let open = mass_map(&ungate(&c, edge));
        let moved: BTreeSet<[i64; 3]> = shut
            .keys()
            .chain(open.keys())
            .filter(|k| shut.get(*k) != open.get(*k))
            .copied()
            .collect();
        assert_eq!(moved, bar, "`{edge}`: ungated, exactly its bar moves");
        for cell in &bar {
            assert_eq!(open.get(cell), None, "`{edge}`: ungated, {cell:?} is air");
        }
    }
    // The floor climb's hole keeps its rung under the grating.
    assert!(
        shut.get(&[28, 63, 16])
            .is_some_and(|b| b.starts_with("minecraft:ladder")),
        "the crawl's hole holds the rung: {:?}",
        shut.get(&[28, 63, 16])
    );

    // Seven gated connections: the three doors and the four above.
    let line = derivation_line(&c);
    assert!(line.contains(", 7 shut)"), "{line}");
    let mut none = c.clone();
    for e in [
        "edge/west-door",
        "edge/centre-door",
        "edge/east-door",
        "edge/annex-chute",
        "edge/loft-stair",
        "edge/undercroft-shaft",
        "edge/undercroft-crawl",
    ] {
        none = ungate(&none, e);
    }
    let line = derivation_line(&none);
    assert!(line.contains(", 0 shut)"), "{line}");
}

/// **Criterion 5, the seed half.** No bar is a function of the seed: the
/// derivation reads no seed at all, so a world seed of another value moves no
/// cell of the mass.
#[test]
fn changing_the_seed_moves_no_bar() {
    let c = gallery_stand_ins("seed");
    let mut other = c.clone();
    other.world.content.seed = other.world.content.seed.wrapping_add(1);
    assert_eq!(mass_map(&c), mass_map(&other));
}

// ---------------------------------------------------------------------------
// Criterion 4: the climb's rung
// ---------------------------------------------------------------------------

/// The blockout fixture with its cell–undercroft link a gated `climb`: the
/// treads taken out, the class a climb, gated on the flag the keeper's beat
/// sets, and that beat opening the grating beside the hall's bars.
fn gated_climb_fixture(tag: &str) -> Campaign {
    let src = common::repo_root().join("crates/delvec/tests/fixtures/blockout");
    let dir = std::env::temp_dir().join(format!("dw-gated-climb-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&src, &dir);
    common::patch_file(&dir.join("site-plan.json"), |v| {
        for s in v["content"]["seams"].as_array_mut().unwrap() {
            if s["edge"] == "edge/cell-undercroft" {
                s.as_object_mut().unwrap().remove("stair_in");
            }
        }
    });
    common::patch_file(&dir.join("layout-graph.json"), |v| {
        for e in v["content"]["edges"].as_array_mut().unwrap() {
            if e["id"] == "edge/cell-undercroft" {
                e["class"] = serde_json::json!("climb");
                e["gating"] = serde_json::json!({ "flags": ["flag/keeper-yields"] });
            }
        }
    });
    let text = std::fs::read_to_string(dir.join("quests.json")).unwrap();
    assert_eq!(text.matches("\"anchor/seam-hall-cell\"").count(), 1);
    std::fs::write(
        dir.join("quests.json"),
        common::patch_doc(&text, |v| {
            fn add(v: &mut serde_json::Value) -> bool {
                match v {
                    serde_json::Value::Array(xs) => {
                        let at = xs
                            .iter()
                            .position(|x| x["anchor"] == "anchor/seam-hall-cell");
                        if let Some(i) = at {
                            let mut opener = xs[i].clone();
                            opener["anchor"] = serde_json::json!("anchor/seam-cell-undercroft");
                            xs.push(opener);
                            return true;
                        }
                        xs.iter_mut().any(add)
                    }
                    serde_json::Value::Object(m) => m.values_mut().any(add),
                    _ => false,
                }
            }
            assert!(add(v), "the keeper's beat opens the hall's bars");
        }),
    )
    .unwrap();
    common::campaign_at(&dir)
}

/// Every cell a body can get to from `roots` by walking, falling, jumping,
/// swimming or climbing.
fn closure(w: &delvec::compiler::nav::World, roots: &[[i32; 3]]) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = roots.iter().copied().collect();
    let mut queue: Vec<[i32; 3]> = seen.iter().copied().collect();
    while let Some(cur) = queue.pop() {
        for n in w.body_moves(cur) {
            if seen.insert(n) {
                queue.push(n);
            }
        }
    }
    seen
}

/// **Criterion 4.** A floor climb with `gating`: the hole cell holds the
/// ladder and the course above it holds the bar; the climb from the lower
/// place reaches the upper floor **only** once the opener has cleared it.
/// The opener's effect is the clear `open-gate` emits, applied to the gate
/// region the derivation names, so the "after" world is the bytes the beat
/// leaves.
#[test]
fn a_gated_floor_climb_keeps_its_rung_and_opens_only_with_its_beat() {
    let c = gated_climb_fixture("rung");
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let mut d = Vec::new();
    delvewright_dsl::siteplan::check(&c, &mut reads, &mut d);
    let errors: Vec<&str> = d
        .iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .map(|x| x.code.as_str())
        .collect();
    assert!(errors.is_empty(), "the gated climb plans clean: {errors:?}");

    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("the library loads");
    let plan = Plan::build(&c, &reg).expect("the gated climb plans");
    let b = plan.blockout.as_ref().expect("a blockout");
    let s = b
        .seams
        .iter()
        .find(|s| s.edge.0 == "edge/cell-undercroft")
        .expect("the climb's seam");
    let (hlo, hhi) = s.opening;
    assert_eq!(s.normal_axis, 1, "the climb goes through a floor");
    let mass = mass_map(&c);
    assert!(
        mass.get(&hlo)
            .is_some_and(|x| x.starts_with("minecraft:ladder")),
        "the hole's first cell holds the rung: {:?}",
        mass.get(&hlo)
    );
    for cell in blockout::cells_of([hlo[0], hlo[1] + 1, hlo[2]], [hhi[0], hhi[1] + 1, hhi[2]]) {
        assert_eq!(mass.get(&cell).map(String::as_str), Some(COVER), "{cell:?}");
    }

    let structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
    let world_of = |blocks: &delvec::compiler::blockstate::BlockMap| {
        delvec::compiler::nav::World::from_occupancy(
            delvec::compiler::assembled::occupancy_over(blocks, &BTreeSet::new()),
            delvec::compiler::nav::Premises::geometry_only(),
        )
    };
    // The assembled bytes leave every gate region to its world-load seal
    // (`World::from_plan` models the gate open and the route proofs shut it per
    // configuration), so the two states are built here from those bytes: shut,
    // with the derivation's own bar written back over the region — the bytes
    // the world loads with, read off the mass above — and open, as the
    // opener's `fill … air` leaves them.
    let grating = blockout::cells_of([hlo[0], hlo[1] + 1, hlo[2]], [hhi[0], hhi[1] + 1, hhi[2]]);
    let mut with_bar = blocks.clone();
    for cell in grating {
        assert_eq!(mass.get(&cell).map(String::as_str), Some(COVER));
        with_bar.insert(
            [cell[0] as i32, cell[1] as i32, cell[2] as i32],
            delvec::compiler::blockstate::BlockState::new(COVER),
        );
    }
    let shut = world_of(&with_bar);
    let open = world_of(&blocks);

    let lower = b
        .boxes
        .iter()
        .find(|x| x.node.0 == "node/undercroft")
        .unwrap();
    let upper = b.boxes.iter().find(|x| x.node.0 == "node/cell").unwrap();
    let roots: Vec<[i32; 3]> = {
        let (lo, hi) = lower.space();
        blockout::cells_of([lo[0], lower.floor, lo[2]], [hi[0], lower.floor, hi[2]])
            .map(|c| [c[0] as i32, c[1] as i32, c[2] as i32])
            .filter(|c| shut.is_standable(*c))
            .collect()
    };
    assert!(!roots.is_empty(), "the undercroft has floor to stand on");
    let on_upper = |reach: &BTreeSet<[i32; 3]>| {
        let (lo, hi) = upper.space();
        reach.iter().any(|c| {
            i64::from(c[1]) == upper.floor
                && (0..3).all(|i| i64::from(c[i]) >= lo[i] && i64::from(c[i]) <= hi[i])
        })
    };
    assert!(
        !on_upper(&closure(&shut, &roots)),
        "shut, the grating keeps the body on the ladder below the upper floor"
    );
    assert!(
        on_upper(&closure(&open, &roots)),
        "opened, the climb reaches the upper floor"
    );
}

// ---------------------------------------------------------------------------
// The bar is the last write to its own cells
// ---------------------------------------------------------------------------

/// `c` with the stair `edge` gated on the terms the connection `like` carries.
fn gate_like(c: &Campaign, edge: &str, like: &str) -> Campaign {
    let mut c = c.clone();
    let g = &mut c.layout_graph.as_mut().expect("a graph").content;
    let gating_of = |e: &delvewright_dsl::layout::Edge| match e {
        delvewright_dsl::layout::Edge::Walk { gating, .. }
        | delvewright_dsl::layout::Edge::Stair { gating, .. }
        | delvewright_dsl::layout::Edge::Climb { gating, .. }
        | delvewright_dsl::layout::Edge::Drop { gating, .. } => gating.clone(),
        _ => None,
    };
    let terms = g
        .edges
        .iter()
        .find(|e| e.id().0 == like)
        .and_then(gating_of)
        .unwrap_or_else(|| panic!("`{like}` is gated"));
    for e in &mut g.edges {
        if e.id().0 != edge {
            continue;
        }
        match e {
            delvewright_dsl::layout::Edge::Stair { gating, .. } => *gating = Some(terms.clone()),
            _ => panic!("`{edge}` is a stair"),
        }
    }
    c
}

/// **A gated stair through a floor, its treads hosted by a stand-in, is built
/// shut.** The stairwell the derivation cuts over a through-floor run reaches
/// the hole's own cells, so a bar laid before it is cut back to air and the
/// world loads with the way open. The bar is the last write to its cells: every
/// cell of the hole holds it, and the run under it still has its stairwell.
#[test]
fn a_gated_stair_through_a_stand_ins_floor_keeps_its_hatch() {
    let open = gallery_stand_ins("stairwell-hatch");
    let line = derivation_line(&open);
    assert!(
        !line.contains(" 0 stairwell cell(s)"),
        "the ungated stair's run is cut a stairwell, so the cut can reach the hole: {line}"
    );
    // The gallery's stair down through the far hall's floor, its treads in the
    // undercroft, gated on the flag the shaft beside it waits on.
    let shut = gate_like(&open, "edge/undercroft-stair", "edge/undercroft-shaft");

    let reg = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("the library loads");
    let plan = Plan::build(&shut, &reg).expect("the gated stair plans");
    let b = plan.blockout.as_ref().expect("a blockout");
    let s = b
        .seams
        .iter()
        .find(|s| s.edge.0 == "edge/undercroft-stair")
        .expect("the stair's seam");
    assert_eq!(s.normal_axis, 1, "the stair goes through a floor");
    let hole = cells(s.opening.0, s.opening.1);
    assert!(!hole.is_empty());

    let mass = mass_map(&shut);
    let missing: Vec<[i64; 3]> = hole
        .iter()
        .filter(|c| mass.get(*c).map(String::as_str) != Some(HATCH))
        .copied()
        .collect();
    assert!(
        missing.is_empty(),
        "every cell of the hole holds the hatch; {} of {} do not: {missing:?}",
        missing.len(),
        hole.len()
    );
    let line = derivation_line(&shut);
    assert!(
        !line.contains(" 0 stairwell cell(s)"),
        "gated, the run still has its stairwell: {line}"
    );
}
