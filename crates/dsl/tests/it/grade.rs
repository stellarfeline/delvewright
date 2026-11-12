//! spec-0138: an open side meets the ground in steps.
//!
//! Every expected height here is computed from the spec's definition by a
//! second route — an explicit nearest-edge search per column, the step count
//! found by counting runs, never by the engine's distance transform — so the
//! engine's arithmetic is not asked to agree with itself.

use delvewright_dsl::ids::NodeId;
use delvewright_dsl::siteplan::grade::{Grade, Meets, settle};
use delvewright_dsl::siteplan::{
    Base, Face, Ground, PlacedBox, SideKind, Sides, Site, Slope, Stratum,
};
use serde_json::{Value, json};

const SEED: u64 = 0x5EED_0138;

/// A sky-open ground place on `foot`, its floor at `floor`.
fn place(node: &str, foot: [i64; 4], floor: i64, sides: [Option<SideKind>; 4]) -> PlacedBox {
    PlacedBox {
        node: NodeId(format!("node/{node}")),
        foot,
        floor,
        clearance: 4,
        open: true,
        base: Base::Ground,
        roof: None,
        sides: Sides {
            east: sides[0],
            west: sides[1],
            south: sides[2],
            north: sides[3],
        },
        slope: Slope::default(),
    }
}

/// A square region `0..n` over a natural surface `t(x, z)`.
fn natural(n: i64, t: impl Fn(i64, i64) -> i64) -> Vec<i64> {
    let mut out = Vec::new();
    for z in 0..n {
        for x in 0..n {
            out.push(t(x, z));
        }
    }
    out
}

fn grade(boxes: &[PlacedBox], n: i64, nat: &[i64]) -> Grade {
    Grade::of(boxes, [0, 0], n, n, Some(nat), SEED)
}

/// The definition of §4 by a second route: the ring columns of `b` whose
/// every side is graded and that no other place's shell covers.
fn edge_columns(b: &PlacedBox, others: &[&PlacedBox]) -> Vec<[i64; 2]> {
    let [x0, x1, z0, z1] = b.foot;
    let mut out = Vec::new();
    for x in x0 - 1..=x1 + 1 {
        for z in z0 - 1..=z1 + 1 {
            let mut faces = Vec::new();
            if x == x1 + 1 {
                faces.push(Face::East);
            }
            if x == x0 - 1 {
                faces.push(Face::West);
            }
            if z == z1 + 1 {
                faces.push(Face::South);
            }
            if z == z0 - 1 {
                faces.push(Face::North);
            }
            if faces.is_empty() {
                continue;
            }
            let graded = faces
                .iter()
                .all(|f| !matches!(side(b, *f), Some(SideKind::Wall | SideKind::Sheer)));
            let covered = others.iter().any(|o| {
                x >= o.foot[0] - 1 && x <= o.foot[1] + 1 && z >= o.foot[2] - 1 && z <= o.foot[3] + 1
            });
            if graded && !covered {
                out.push([x, z]);
            }
        }
    }
    out
}

fn side(b: &PlacedBox, f: Face) -> Option<SideKind> {
    match f {
        Face::East => b.sides.east,
        Face::West => b.sides.west,
        Face::South => b.sides.south,
        Face::North => b.sides.north,
        _ => None,
    }
}

/// The step count by counting runs: the least `k` whose distance `1 + run·k`
/// is at least the exact Euclidean distance `√q`, compared in squares.
fn k_by_counting(q: i64, run: i64) -> i64 {
    let mut k = 0;
    while (1 + run * k) * (1 + run * k) < q {
        k += 1;
    }
    k
}

/// A place's allowance at `c` by the second route: rise × runs counted.
fn allowance_at(p: &PlacedBox, edges: &[[i64; 2]], c: [i64; 2]) -> Option<i64> {
    let q = edges
        .iter()
        .map(|e| (e[0] - c[0]).pow(2) + (e[1] - c[1]).pow(2))
        .min()?;
    Some(p.slope.rise() * k_by_counting(q, p.slope.run()))
}

/// Each place's bank by the second route: a flood from the columns beside
/// its edge columns through whole columns (none of `boxes`' shells) whose
/// natural height its own bound excludes, over an `n × n` region.
fn banks(
    places: &[(&PlacedBox, Vec<[i64; 2]>)],
    boxes: &[&PlacedBox],
    n: i64,
    t: &dyn Fn(i64, i64) -> i64,
) -> Vec<std::collections::BTreeSet<[i64; 2]>> {
    let whole = |c: [i64; 2]| {
        c[0] >= 0
            && c[1] >= 0
            && c[0] < n
            && c[1] < n
            && !boxes.iter().any(|b| in_shell(b, c[0], c[1]))
    };
    places
        .iter()
        .map(|(p, edges)| {
            let s = p.floor - 1;
            let mut bank = std::collections::BTreeSet::new();
            let mut seen = std::collections::BTreeSet::new();
            let mut todo: Vec<[i64; 2]> = Vec::new();
            for e in edges {
                for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    todo.push([e[0] + d[0], e[1] + d[1]]);
                }
            }
            while let Some(c) = todo.pop() {
                if !whole(c) || !seen.insert(c) {
                    continue;
                }
                let a = allowance_at(p, edges, c).expect("edges");
                let nat = t(c[0], c[1]);
                if nat >= s - a && nat <= s + a {
                    continue;
                }
                bank.insert(c);
                for d in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                    todo.push([c[0] + d[0], c[1] + d[1]]);
                }
            }
            bank
        })
        .collect()
}

/// `[lo, hi]` at column `c` over the places whose bank reaches it.
fn bound(
    places: &[(&PlacedBox, Vec<[i64; 2]>)],
    banks: &[std::collections::BTreeSet<[i64; 2]>],
    c: [i64; 2],
) -> Option<(i64, i64)> {
    let mut b: Option<(i64, i64)> = None;
    for ((p, edges), bank) in places.iter().zip(banks) {
        if !bank.contains(&c) {
            continue;
        }
        let a = allowance_at(p, edges, c)?;
        let s = p.floor - 1;
        b = Some(b.map_or((s - a, s + a), |(l, h)| (l.max(s - a), h.min(s + a))));
    }
    b
}

fn in_shell(b: &PlacedBox, x: i64, z: i64) -> bool {
    x >= b.foot[0] - 1 && x <= b.foot[1] + 1 && z >= b.foot[2] - 1 && z <= b.foot[3] + 1
}

// ---------------------------------------------------------------- criterion 1

/// `sheer` is a third unwalled value: open, not declared open, and not graded;
/// `grade` is a slope inside its ranges, refused outside them.
#[test]
fn a_side_may_be_sheer() {
    let s: Sides =
        serde_json::from_value(json!({"east": "sheer", "north": "wall"})).expect("`sheer` parses");
    assert_eq!(s.east, Some(SideKind::Sheer));
    assert!(s.is_open(Face::East), "a sheer side stands no wall");
    assert!(!s.is_declared_open(Face::East), "a sheer side is not a way");
    assert!(!s.is_graded(Face::East), "a sheer side keeps its drop");
    assert!(s.is_graded(Face::West), "an absent side is graded");
    assert!(!s.is_graded(Face::North), "a wall side has no bank");
    assert_eq!(
        serde_json::to_value(s).expect("serialises"),
        json!({"east": "sheer", "north": "wall"}),
        "round-trips in its own spelling"
    );
    // The slope: in range parses, out of range is refused.
    let ok: Slope = serde_json::from_value(json!({"rise": 5, "run": 1})).expect("steepest");
    assert_eq!((ok.rise(), ok.run()), (5, 1));
    assert_eq!(Slope::default(), Slope::new(1, 2).expect("the default"));
    for bad in [
        json!({"rise": 0, "run": 1}),
        json!({"rise": 6, "run": 1}),
        json!({"rise": 1, "run": 0}),
        json!({"rise": 1, "run": 5}),
        json!({"rise": 1}),
    ] {
        assert!(
            serde_json::from_value::<Slope>(bad.clone()).is_err(),
            "{bad} is refused"
        );
    }
    // The schema exports all three values at each of the four keys, and the
    // slope's ranges.
    let schema = serde_json::to_string(&delvewright_dsl::stage_schema(
        delvewright_dsl::Stage::SitePlan,
    ))
    .expect("the schema serialises");
    for v in ["\"wall\"", "\"open\"", "\"sheer\""] {
        assert!(schema.contains(v), "the schema names {v}");
    }
    let schema: Value = serde_json::from_str(&schema).expect("parses");
    let sides = find_def(&schema, "Sides").expect("the schema defines Sides");
    let keys = sides["properties"]
        .as_object()
        .expect("Sides has properties");
    assert_eq!(keys.len(), 4, "four keys of `boxes[].sides`");
    assert!(
        find_def(&schema, "Slope").is_some(),
        "the schema defines Slope"
    );
}

fn find_def<'a>(schema: &'a Value, name: &str) -> Option<&'a Value> {
    schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(|d| d.get(name))
}

// ---------------------------------------------------------------- criteria 2, 3

/// The slopes every straight-side test is run over: the default, the
/// steepest, the gentlest and one between.
const SLOPES: [(u8, u8); 4] = [(1, 2), (5, 1), (1, 4), (3, 2)];

/// One place over flat terrain at `drop` (`P − t`), its east side read along
/// one straight row: the shoulder at `P`; each column held between the bound
/// `P ∓ rise·⌈(d − 1)/run⌉` and one rise back toward `P`; the natural height
/// from where the bound reaches the terrain; and `rise·…` regraded columns
/// per row.
fn straight_side(drop: i64, slope: (u8, u8)) -> (usize, usize) {
    let s = Slope::new(slope.0, slope.1).expect("a slope in range");
    let (rise, run) = (s.rise(), s.run());
    let t = 63;
    let p = t + drop;
    let n = 140;
    let mut b = place("terrace", [10, 19, 60, 69], p + 1, [None, None, None, None]);
    b.slope = s;
    let nat = natural(n, |_, _| t);
    let g = grade(std::slice::from_ref(&b), n, &nat);
    let (mut examined, mut moved) = (0usize, 0usize);
    let needed = (drop.abs() + rise - 1) / rise; // runs until the bound meets t
    let last = 1 + run * needed; // the first column standing at t, or past it
    for z in 62..=67 {
        for d in 1..=last + 2 {
            let x = 19 + 1 + d;
            examined += 1;
            let top = g.top(x, z).expect("inside the region");
            let a = rise * ((d - 1 + run - 1) / run);
            if d == 1 {
                assert_eq!(top, p, "the shoulder stands at the place's height (d 1)");
            } else if a < drop.abs() {
                // Held to the bound, and carried at most one rise back toward
                // the place, never past it.
                let (mn, mx) = if drop > 0 {
                    (p - a, (p - a + rise).min(p))
                } else {
                    ((p + a - rise).max(p), p + a)
                };
                assert!(
                    (mn..=mx).contains(&top),
                    "drop {drop} slope {rise}/{run}: column {d} stands at {top}, outside {mn}..={mx}"
                );
            } else {
                assert_eq!(
                    top, t,
                    "drop {drop} slope {rise}/{run}: column {d} is natural"
                );
            }
            if top != t {
                moved += 1;
            }
        }
    }
    let rows = 6;
    let per_row = (1..=last + 2)
        .filter(|d| *d == 1 || rise * ((d - 1 + run - 1) / run) < drop.abs())
        .count();
    assert_eq!(
        moved,
        rows * per_row,
        "regraded per edge column, drop {drop}"
    );
    if slope == (1, 2) {
        assert_eq!(per_row as i64, 2 * drop.abs() - 1, "2H − 1 per edge column");
    }
    (examined, moved)
}

#[test]
fn a_raised_side_steps_down_to_the_ground() {
    let mut total = 0;
    for slope in SLOPES {
        for h in 1..=6 {
            let (examined, _) = straight_side(h, slope);
            let s = Slope::new(slope.0, slope.1).expect("in range");
            let needed = (h + s.rise() - 1) / s.rise();
            assert_eq!(examined as i64, 6 * (1 + s.run() * needed + 2));
            total += examined;
        }
    }
    // The castle on the mountain: thirty blocks at five a column.
    let (_, moved) = straight_side(30, (5, 1));
    assert_eq!(moved, 6 * 6, "six columns drop thirty blocks, per row");
    assert_eq!(total, 1338, "columns examined over every slope and drop");
}

#[test]
fn a_lowered_side_steps_up_to_the_ground() {
    let mut total = 0;
    for slope in SLOPES {
        for h in 1..=6 {
            let (examined, _) = straight_side(-h, slope);
            total += examined;
        }
    }
    assert_eq!(total, 1338, "columns examined over every slope and drop");
}

/// A terrain that crosses the place's height along its east side: each
/// column takes the branch it needs, and a column already inside the bound is
/// byte for byte what it was.
#[test]
fn ground_that_already_meets_the_side_is_left() {
    let n = 80;
    let p = 63;
    let b = place("terrace", [10, 19, 20, 59], p + 1, [None, None, None, None]);
    // Along `z` the terrain runs from P − 6 to P + 6 and back.
    let t = |_: i64, z: i64| p - 6 + (z - 20).clamp(0, 12) - (z - 40).clamp(0, 12);
    let nat = natural(n, t);
    let g = grade(std::slice::from_ref(&b), n, &nat);
    let edges = vec![(&b, edge_columns(&b, &[]))];
    let bk = banks(&edges, &[&b], n, &t);
    let (mut raised, mut lowered, mut kept) = (0, 0, 0);
    for x in 21..60 {
        for z in 21..59 {
            let top = g.top(x, z).expect("inside");
            let nat = t(x, z);
            let Some((lo, hi)) = bound(&edges, &bk, [x, z]) else {
                assert_eq!(top, nat, "a column no bank reaches is left at [{x}, {z}]");
                kept += 1;
                continue;
            };
            if nat < lo {
                assert!(
                    top >= lo && top <= hi,
                    "raised into the bound at [{x}, {z}]"
                );
                raised += 1;
            } else if nat > hi {
                assert!(
                    top >= lo && top <= hi,
                    "lowered into the bound at [{x}, {z}]"
                );
                lowered += 1;
            } else {
                assert_eq!(top, nat, "a column inside the bound is left at [{x}, {z}]");
                kept += 1;
            }
        }
    }
    assert!(
        raised > 0 && lowered > 0 && kept > 0,
        "{raised} {lowered} {kept}"
    );
    assert_eq!(raised + lowered + kept, 39 * 38);
}

// ---------------------------------------------------------------- criterion 4

const VALUES: [Option<SideKind>; 4] = [
    None,
    Some(SideKind::Open),
    Some(SideKind::Sheer),
    Some(SideKind::Wall),
];

/// Over every combination of the four values on four sides and drops
/// −4..=4: the bound equals the definition computed by the second route; each
/// column settles by [`settle`] with the noise; no two 4-adjacent columns'
/// bounds differ by more than the rise; a corner between a graded and an
/// ungraded side is not an edge column.
#[test]
fn a_corner_is_rounded_and_a_wall_end_is_turned() {
    let n = 36;
    let t = 63;
    let nat = natural(n, |_, _| t);
    let (mut plans, mut compared) = (0usize, 0usize);
    for combo in 0..256usize {
        let sides = [
            VALUES[combo % 4],
            VALUES[(combo / 4) % 4],
            VALUES[(combo / 16) % 4],
            VALUES[(combo / 64) % 4],
        ];
        for drop in -4..=4 {
            plans += 1;
            let b = place("p", [14, 21, 14, 21], t + drop + 1, sides);
            let g = grade(std::slice::from_ref(&b), n, &nat);
            let edges = edge_columns(&b, &[]);
            assert_eq!(g.edge_columns, edges.len(), "edge columns, combo {combo}");
            let ring = vec![(&b, edges)];
            let tf = |_: i64, _: i64| t;
            let bk = banks(&ring, &[&b], n, &tf);
            let mut bounds = vec![None; (n * n) as usize];
            for x in 0..n {
                for z in 0..n {
                    if in_shell(&b, x, z) {
                        continue;
                    }
                    let top = g.top(x, z).expect("inside");
                    let want = match bound(&ring, &bk, [x, z]) {
                        Some((lo, hi)) => {
                            bounds[(z * n + x) as usize] = Some(lo);
                            // The default slope's rise is the noise's amplitude.
                            settle(t, lo, hi, Ground::bank_noise(SEED, x, z, 1))
                        }
                        None => t,
                    };
                    assert_eq!(top, want, "combo {combo} drop {drop} column [{x}, {z}]");
                    compared += 1;
                }
            }
            for x in 0..n - 1 {
                for z in 0..n - 1 {
                    let here = bounds[(z * n + x) as usize];
                    for o in [((z * n) + x + 1) as usize, ((z + 1) * n + x) as usize] {
                        if let (Some(a), Some(b)) = (here, bounds[o]) {
                            assert!((a - b).abs() <= 1, "adjacent bounds differ by one at most");
                        }
                    }
                }
            }
        }
    }
    assert_eq!(plans, 2304);
    assert_eq!(compared, 2304 * (36 * 36 - 100));
}

// ---------------------------------------------------------------- criterion 5

/// The grade writes no cell any place owns: ownership, the claim and the
/// fixed ground are equal graded and with every side rewritten `sheer`; a
/// lone wall side, sheer side or aloft place regrades nothing.
#[test]
fn the_grade_writes_no_cell_of_any_place() {
    let n = 48;
    let nat = natural(n, |x, z| 60 + (x + z) / 12);
    let skin = || {
        (
            "minecraft:grass_block",
            "minecraft:dirt",
            Vec::<Stratum>::new(),
        )
    };
    let sheer = |b: &PlacedBox| {
        let mut b = b.clone();
        for f in [
            &mut b.sides.east,
            &mut b.sides.west,
            &mut b.sides.south,
            &mut b.sides.north,
        ] {
            if *f != Some(SideKind::Wall) {
                *f = Some(SideKind::Sheer);
            }
        }
        b
    };
    let mut cells = 0usize;
    let mut plans: Vec<Vec<PlacedBox>> = Vec::new();
    for combo in (0..256usize).step_by(5) {
        let sides = [
            VALUES[combo % 4],
            VALUES[(combo / 4) % 4],
            VALUES[(combo / 16) % 4],
            VALUES[(combo / 64) % 4],
        ];
        plans.push(vec![place("p", [18, 27, 18, 27], 67, sides)]);
    }
    plans.push(vec![
        place("a", [6, 15, 6, 15], 68, [None, None, None, None]),
        place(
            "b",
            [30, 39, 28, 37],
            58,
            [Some(SideKind::Open), None, None, Some(SideKind::Wall)],
        ),
    ]);
    for boxes in &plans {
        let (s1, s2, ns) = skin();
        let graded = Ground::open_over(
            [0, 0, 0],
            [n - 1, 200, n - 1],
            nat.clone(),
            boxes,
            SEED,
            (s1, s2, ns),
        );
        let flat: Vec<PlacedBox> = boxes.iter().map(sheer).collect();
        let (s1, s2, ns) = skin();
        let left = Ground::open_over(
            [0, 0, 0],
            [n - 1, 200, n - 1],
            nat.clone(),
            &flat,
            SEED,
            (s1, s2, ns),
        );
        assert!(
            left.grade().expect("graded").regraded.is_empty(),
            "all sheer moves nothing"
        );
        let a = Site::new(boxes, &[], &graded);
        let b = Site::new(&flat, &[], &left);
        for i in 0..boxes.len() {
            assert_eq!(a.claim_cells(i), b.claim_cells(i), "the claim");
            assert_eq!(a.fixed_cells(i), b.fixed_cells(i), "the fixed ground");
            for c in a.claim_cells(i) {
                assert_eq!(a.owner(c), b.owner(c), "the owner of {c:?}");
                cells += 1;
            }
        }
    }
    assert!(cells > 50_000, "claim cells compared: {cells}");
    // One place alone: a wall side, a sheer side, an aloft place.
    let flat = natural(n, |_, _| 50);
    let wall = place("w", [18, 27, 18, 27], 67, [Some(SideKind::Wall); 4]);
    let shr = place("s", [18, 27, 18, 27], 67, [Some(SideKind::Sheer); 4]);
    let mut aloft = place("a", [18, 27, 18, 27], 67, [None; 4]);
    aloft.base = Base::Aloft(0);
    for b in [wall, shr, aloft] {
        let g = grade(std::slice::from_ref(&b), n, &flat);
        assert_eq!(g.regraded.len(), 0, "{} regrades nothing", b.node);
    }
}

// ---------------------------------------------------------------- criterion 6

/// A regraded column takes the blocks of the first stratum its graded height
/// meets, steepness read over the graded surface, and is solid to its top.
#[test]
fn a_regraded_column_is_skinned_as_terrain() {
    let n = 48;
    let strata: Vec<Stratum> = serde_json::from_value(json!([
        {"when": {"steep": 3}, "surface": "minecraft:stone", "below": "minecraft:stone"},
        {"when": {"y_max": 60}, "surface": "minecraft:sand", "below": "minecraft:sandstone"},
        {"when": {"y_min": 61}, "surface": "minecraft:grass_block", "below": "minecraft:dirt"}
    ]))
    .expect("strata parse");
    let mut steep = place("castle", [18, 27, 18, 27], 74, [None; 4]);
    steep.slope = Slope::new(5, 1).expect("in range");
    let gentle = place("meadow", [4, 9, 36, 43], 62, [None; 4]);
    let boxes = vec![steep, gentle];
    let nat = natural(n, |_, _| 56);
    let g = Ground::open_over(
        [0, 0, 0],
        [n - 1, 120, n - 1],
        nat,
        &boxes,
        SEED,
        ("minecraft:grass_block", "minecraft:dirt", strata.clone()),
    );
    let grade = g.grade().expect("graded");
    assert!(!grade.regraded.is_empty());
    let mut examined = 0;
    for r in &grade.regraded {
        let y = g.top(r.x, r.z).expect("a top");
        assert_eq!(y, r.graded);
        let st = [(1, 0), (-1, 0), (0, 1), (0, -1)]
            .iter()
            .map(|(dx, dz)| {
                let (px, pz) = ((r.x + dx).clamp(0, n - 1), (r.z + dz).clamp(0, n - 1));
                (g.top(px, pz).expect("a top") - y).abs()
            })
            .max()
            .unwrap_or(0);
        let want = if st >= 3 {
            ("minecraft:stone", "minecraft:stone")
        } else if y <= 60 {
            ("minecraft:sand", "minecraft:sandstone")
        } else {
            ("minecraft:grass_block", "minecraft:dirt")
        };
        assert_eq!(
            g.skin(r.x, r.z),
            Some(want),
            "column [{}, {}] at {y}",
            r.x,
            r.z
        );
        assert_eq!(g.fill_block([r.x, y, r.z]), Some(want.0));
        for yy in 0..y {
            assert!(
                g.fill_block([r.x, yy, r.z]).is_some(),
                "solid under the surface"
            );
        }
        assert_eq!(g.fill_block([r.x, y + 1, r.z]), None, "air over it");
        examined += 1;
    }
    assert_eq!(examined, grade.regraded.len());
    assert!(examined > 200, "regraded columns examined: {examined}");
}

// ---------------------------------------------------------------- criterion 11

/// The counts add up, and the ledger's text moves with a pixel inside a bank
/// and not with one outside every bound.
#[test]
fn the_ledger_counts_what_was_regraded() {
    let n = 64;
    let boxes = vec![
        place(
            "terrace",
            [10, 19, 10, 19],
            67,
            [
                Some(SideKind::Open),
                None,
                Some(SideKind::Sheer),
                Some(SideKind::Wall),
            ],
        ),
        place("hollow", [40, 49, 40, 49], 61, [None; 4]),
    ];
    let t = 63;
    let nat = natural(n, |_, _| t);
    let g = grade(&boxes, n, &nat);
    let (u, s, h) = g.side_counts();
    assert_eq!((u, s, h), (7, 6, 1));
    assert_eq!(u, s + h);
    let (r, l) = g.raised_lowered();
    assert_eq!(r + l, g.regraded.len());
    assert_eq!(g.sides.iter().map(|s| s.raised).sum::<usize>(), r);
    assert_eq!(g.sides.iter().map(|s| s.lowered).sum::<usize>(), l);
    assert!(g.sides.iter().any(|s| s.meets == Meets::Sheer));
    let rows = g.terrain_rows();
    assert_eq!(rows.lines().count(), g.regraded.len());
    // A pixel inside the terrace's east bank.
    let mut nat2 = nat.clone();
    nat2[(15 * n + 23) as usize] = 66;
    let g2 = grade(&boxes, n, &nat2);
    assert_ne!(g2.terrain_rows(), rows, "a pixel in a bank moves the rows");
    let east = |g: &Grade| {
        g.sides
            .iter()
            .find(|s| s.node.0 == "node/terrace" && s.face == Face::East)
            .map(|s| (s.raised, s.lowered, s.drop))
    };
    assert_ne!(east(&g2), east(&g), "and the east side's row");
    // A pixel outside every bound.
    let mut nat3 = nat.clone();
    nat3[(60 * n + 2) as usize] = t + 1;
    let g3 = grade(&boxes, n, &nat3);
    assert_eq!(
        g3.terrain_rows(),
        rows,
        "a pixel outside every bank moves nothing"
    );
    assert_eq!(east(&g3), east(&g));
}

// ---------------------------------------------------------------- criterion 8

/// `DW1108`: five refused shapes, six passing.
#[test]
fn dw1108_a_grade_has_no_room_to_land() {
    use crate::v14_site_plan::{grade_plan_codes as codes, has_grade};
    // The columns a refusal says to move the places apart by.
    let stated = |codes: &[String]| -> i64 {
        let m = codes
            .iter()
            .find(|c| c.starts_with("DW1108"))
            .expect("a DW1108");
        let at = m.find("apart by ").expect("names the columns") + 9;
        m[at..]
            .split(' ')
            .next()
            .expect("a number")
            .parse()
            .expect("a number")
    };
    let open = json!("open");
    let wall = json!("wall");
    let sheer = json!("sheer");
    let b = |node: &str, min: [i64; 2], ext: [i64; 2], y: i64, sides: Value| {
        json!({"node": node, "min": min, "extent": ext, "floor": {"y": y},
               "ceiling": {"open": 4}, "base": "ground", "sides": sides})
    };
    let water = |min: [i64; 3], ext: [i64; 3]| {
        json!({"id": "volume/pond", "role": "massif", "block": "minecraft:water",
               "region": {"min": min, "extent": ext}})
    };
    // 1. Two raised places whose facing graded sides stand two whole columns
    //    apart, walled north and south: a difference of 3 needs 6.
    let ns = |more: Value| {
        let mut v = json!({"north": "wall", "south": "wall"});
        if let Some(o) = more.as_object() {
            for (k, x) in o {
                v[k] = x.clone();
            }
        }
        v
    };
    let near = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, ns(json!({}))),
            b("node/b", [14, 46], [6, 6], 65, ns(json!({}))),
        ],
        vec![],
    );
    assert!(has_grade(&near), "two banks with no room: {near:?}");
    let more = stated(&near);
    let apart = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, ns(json!({}))),
            b("node/b", [14 + more, 46], [6, 6], 65, ns(json!({}))),
        ],
        vec![],
    );
    assert!(!has_grade(&apart), "the stated columns added: {apart:?}");
    let one_sheer = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, ns(json!({"east": sheer}))),
            b("node/b", [14, 46], [6, 6], 65, ns(json!({}))),
        ],
        vec![],
    );
    assert!(
        !has_grade(&one_sheer),
        "the pair with one side sheer: {one_sheer:?}"
    );
    // 2. A bank that moves another place's shoulder: one whole column
    //    between a place four over the ground and one standing at it.
    let shoulder = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, json!({})),
            b("node/b", [13, 46], [6, 6], 64, json!({})),
        ],
        vec![],
    );
    assert!(
        has_grade(&shoulder),
        "a bank at another's edge: {shoulder:?}"
    );
    let more = stated(&shoulder);
    let shoulder_apart = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, json!({})),
            b("node/b", [13 + more, 46], [6, 6], 64, json!({})),
        ],
        vec![],
    );
    assert!(
        !has_grade(&shoulder_apart),
        "moved apart: {shoulder_apart:?}"
    );
    // Two places that meet draw their join: no refusal (settled ruling).
    let meet = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, json!({})),
            b("node/b", [12, 46], [6, 6], 64, json!({})),
        ],
        vec![],
    );
    assert!(!has_grade(&meet), "two places that meet: {meet:?}");
    // 3. A bank burying another place's wall foot.
    let foot = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, json!({})),
            b(
                "node/b",
                [14, 46],
                [6, 6],
                64,
                json!({"west": wall, "east": wall, "south": wall, "north": wall}),
            ),
        ],
        vec![],
    );
    assert!(has_grade(&foot), "a bank at another's wall foot: {foot:?}");
    let more = stated(&foot);
    let foot_apart = codes(
        vec![
            b("node/a", [4, 46], [6, 6], 68, json!({})),
            b(
                "node/b",
                [14 + more, 46],
                [6, 6],
                64,
                json!({"west": wall, "east": wall, "south": wall, "north": wall}),
            ),
        ],
        vec![],
    );
    assert!(!has_grade(&foot_apart), "moved apart: {foot_apart:?}");
    // A place's bank against its own wall passes.
    let own = codes(
        vec![b(
            "node/a",
            [4, 46],
            [6, 6],
            68,
            json!({"north": wall, "east": open}),
        )],
        vec![],
    );
    assert!(!has_grade(&own), "its own wall: {own:?}");
    // 4. A cut beside water, and 5. under it.
    let hollow = || vec![b("node/a", [20, 46], [8, 8], 61, json!({}))];
    let beside = codes(hollow(), vec![water([31, 62, 46], [3, 2, 8])]);
    assert!(has_grade(&beside), "a cut beside water: {beside:?}");
    let under = codes(hollow(), vec![water([29, 63, 46], [1, 1, 8])]);
    assert!(has_grade(&under), "a cut under water: {under:?}");
    // A bank raised into the water volume passes; so does a `ground` volume
    // inside a bank.
    let raised = codes(
        vec![b("node/a", [20, 46], [8, 8], 66, json!({}))],
        vec![water([29, 62, 46], [3, 2, 8])],
    );
    assert!(!has_grade(&raised), "a bank raised into water: {raised:?}");
    let inside = codes(
        vec![b("node/a", [20, 46], [8, 8], 66, json!({}))],
        vec![
            json!({"id": "volume/knoll", "role": "ground", "region": {"min": [29, 50, 46], "extent": [2, 14, 4]}}),
        ],
    );
    assert!(
        !has_grade(&inside),
        "a ground volume inside a bank: {inside:?}"
    );
}
