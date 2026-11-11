//! **Rails as a medium** (spec-0135 §3): the rail cells of the assembled world
//! joined by their shapes, and the run a pushed minecart takes along them.
//!
//! A rail cell is read from its block state — `rail`, `powered_rail`,
//! `detector_rail`, `activator_rail` — with its `shape` and, for a powered
//! rail, its `powered` state as the assembled world holds it. The engine reads
//! power; it never infers it.

use std::collections::{BTreeMap, BTreeSet};

/// The plain rail a ridden cart coasts over without stopping, in rail cells
/// (`docs/reference/vehicles-animals-and-targets.md` §6, §11): a cart at 10.4
/// blocks a second held that speed over 45 cells, and a cart pushed once from
/// rest was still rolling 57 cells on — 45 is inside both.
pub const COAST_CELLS: usize = 45;

/// The rate a ride on a cart is priced and waited for at, in blocks a second:
/// the slowest whole run the record holds (§11: 57 cells of plain rail in
/// 23.5 s from one push). A floor — a line with powered rails runs faster.
pub const CART_RATE: f64 = 2.4;

/// The motion the engine gives a cart the tick a body boards it, in blocks a
/// tick along its `facing` — the push the record measured (§6, §11).
pub const CART_PUSH: f64 = 0.3;

/// One rail cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rail {
    /// Whether it is a `powered_rail`.
    pub booster: bool,
    /// For a `powered_rail`, its `powered` state.
    pub powered: bool,
    /// Its two exits: horizontal direction and the rise toward it (0 or +1).
    pub exits: [([i32; 2], i32); 2],
}

impl Rail {
    /// Whether a cart is driven forward here: a powered rail that is powered.
    pub fn drives(&self) -> bool {
        self.booster && self.powered
    }
}

/// The rail a block state is, if it is one.
pub fn rail_of(state: &str) -> Option<Rail> {
    let (id, props) = match state.split_once('[') {
        Some((id, rest)) => (id, rest.trim_end_matches(']')),
        None => (state, ""),
    };
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    if !matches!(
        id,
        "rail" | "powered_rail" | "detector_rail" | "activator_rail"
    ) {
        return None;
    }
    let prop = |k: &str| {
        props
            .split(',')
            .find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v))
    };
    const N: [i32; 2] = [0, -1];
    const S: [i32; 2] = [0, 1];
    const E: [i32; 2] = [1, 0];
    const W: [i32; 2] = [-1, 0];
    let exits = match prop("shape").unwrap_or("north_south") {
        "north_south" => [(N, 0), (S, 0)],
        "east_west" => [(E, 0), (W, 0)],
        "ascending_east" => [(E, 1), (W, 0)],
        "ascending_west" => [(W, 1), (E, 0)],
        "ascending_north" => [(N, 1), (S, 0)],
        "ascending_south" => [(S, 1), (N, 0)],
        "south_east" => [(S, 0), (E, 0)],
        "south_west" => [(S, 0), (W, 0)],
        "north_west" => [(N, 0), (W, 0)],
        "north_east" => [(N, 0), (E, 0)],
        _ => return None,
    };
    Some(Rail {
        booster: id == "powered_rail",
        powered: prop("powered") == Some("true"),
        exits,
    })
}

/// The rail cells next to `c` along its shape that join back to it.
pub fn rail_neighbours(rails: &BTreeMap<[i32; 3], Rail>, c: [i32; 3]) -> Vec<[i32; 3]> {
    let Some(r) = rails.get(&c) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (d, rise) in r.exits {
        // Up the slope, level, or down onto a rail that climbs toward us.
        for dy in [rise, 0, -1] {
            let n = [c[0] + d[0], c[1] + dy, c[2] + d[1]];
            if out.contains(&n) {
                continue;
            }
            let Some(nr) = rails.get(&n) else { continue };
            let back = [-d[0], -d[1]];
            if nr.exits.iter().any(|(e, _)| *e == back) {
                out.push(n);
                break;
            }
        }
    }
    out
}

/// **Whether a cart with nobody in it stays at `seat`** (the record §11): a
/// level rail that does not launch it — a powered rail that is powered, with
/// a solid block (`solid`) at one end, pushes a cart at rest away from the
/// block whether or not anybody sits in it.
pub fn rests(
    rails: &BTreeMap<[i32; 3], Rail>,
    solid: &dyn Fn([i32; 3]) -> bool,
    seat: [i32; 3],
) -> bool {
    let Some(r) = rails.get(&seat) else {
        return false;
    };
    if r.exits.iter().any(|(_, rise)| *rise != 0) {
        return false;
    }
    !(r.drives()
        && r.exits
            .iter()
            .any(|(d, _)| solid([seat[0] + d[0], seat[1], seat[2] + d[1]])))
}

/// **The run a cart pushed from `seat` toward `facing` takes** (spec-0135 §3):
/// along the rails from the seat, through every stretch of rail that does not
/// drive it no longer than [`COAST_CELLS`], to where the rail ends, a brake —
/// an unpowered powered rail — stops it, or such a stretch outruns the coast.
/// `None` when the seat is no rail or its rail does not run toward `facing`.
/// The run is in order, the seat first.
pub fn driven_run(
    rails: &BTreeMap<[i32; 3], Rail>,
    seat: [i32; 3],
    facing: [i32; 2],
    within: &dyn Fn([i32; 3]) -> bool,
) -> Option<Vec<[i32; 3]>> {
    let r = rails.get(&seat)?;
    if !r.exits.iter().any(|(d, _)| *d == facing) {
        return None;
    }
    let mut run = vec![seat];
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::from([seat]);
    let mut prev = seat;
    let mut coast = 0usize;
    let mut next = rail_neighbours(rails, seat)
        .into_iter()
        .find(|n| n[0] == seat[0] + facing[0] && n[2] == seat[2] + facing[1]);
    while let Some(c) = next {
        if !within(c) || !seen.insert(c) {
            break;
        }
        let rail = rails[&c];
        if rail.booster && !rail.powered {
            run.push(c); // a brake stops the cart on it
            break;
        }
        coast = if rail.drives() { 0 } else { coast + 1 };
        if coast > COAST_CELLS {
            break;
        }
        run.push(c);
        next = rail_neighbours(rails, c)
            .into_iter()
            .find(|n| *n != prev && !seen.contains(n));
        prev = c;
    }
    Some(run)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: i32, powered_upto: i32) -> BTreeMap<[i32; 3], Rail> {
        (0..n)
            .map(|x| {
                let s = if x < powered_upto {
                    "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]"
                } else {
                    "minecraft:rail[shape=east_west,waterlogged=false]"
                };
                ([x, 0, 0], rail_of(s).unwrap())
            })
            .collect()
    }

    #[test]
    fn a_pushed_cart_runs_its_line_to_the_end() {
        let rails = line(20, 0);
        let run = driven_run(&rails, [0, 0, 0], [1, 0], &|_| true).unwrap();
        assert_eq!(run.len(), 20);
        assert_eq!(*run.last().unwrap(), [19, 0, 0]);
    }

    #[test]
    fn the_coast_cuts_the_run_and_a_powered_rail_renews_it() {
        let rails = line(60, 0);
        let run = driven_run(&rails, [0, 0, 0], [1, 0], &|_| true).unwrap();
        assert_eq!(run.len(), 1 + COAST_CELLS);
        let mut renewed = line(60, 0);
        renewed.insert(
            [30, 0, 0],
            rail_of("minecraft:powered_rail[powered=true,shape=east_west]").unwrap(),
        );
        let run = driven_run(&renewed, [0, 0, 0], [1, 0], &|_| true).unwrap();
        assert_eq!(run.len(), 60);
    }

    #[test]
    fn a_brake_stops_the_cart_on_it() {
        let mut rails = line(20, 0);
        rails.insert(
            [8, 0, 0],
            rail_of("minecraft:powered_rail[powered=false,shape=east_west]").unwrap(),
        );
        let run = driven_run(&rails, [0, 0, 0], [1, 0], &|_| true).unwrap();
        assert_eq!(*run.last().unwrap(), [8, 0, 0]);
    }

    #[test]
    fn a_cart_facing_across_its_rail_runs_nowhere() {
        let rails = line(10, 0);
        assert!(driven_run(&rails, [0, 0, 0], [0, 1], &|_| true).is_none());
        // Facing the rail's dead end, the run is the seat alone.
        assert_eq!(
            driven_run(&rails, [0, 0, 0], [-1, 0], &|_| true).unwrap(),
            vec![[0, 0, 0]]
        );
    }

    #[test]
    fn a_cart_rests_on_level_rail_and_not_on_a_launcher_or_a_slope() {
        let plain = line(10, 0);
        assert!(rests(&plain, &|c| c == [-1, 0, 0], [0, 0, 0]));
        let launcher = line(10, 3);
        assert!(!rests(&launcher, &|c| c == [-1, 0, 0], [0, 0, 0]));
        // A powered rail with nothing solid at either end launches nothing.
        assert!(rests(&launcher, &|_| false, [1, 0, 0]));
        let mut slope = BTreeMap::new();
        slope.insert(
            [0, 0, 0],
            rail_of("minecraft:rail[shape=ascending_east]").unwrap(),
        );
        assert!(!rests(&slope, &|_| false, [0, 0, 0]));
        assert!(!rests(&plain, &|_| false, [0, 5, 0]));
    }

    #[test]
    fn a_slope_joins_its_neighbours() {
        let mut rails = BTreeMap::new();
        rails.insert(
            [0, 0, 0],
            rail_of("minecraft:rail[shape=ascending_east]").unwrap(),
        );
        rails.insert(
            [1, 1, 0],
            rail_of("minecraft:rail[shape=east_west]").unwrap(),
        );
        rails.insert(
            [-1, 0, 0],
            rail_of("minecraft:rail[shape=east_west]").unwrap(),
        );
        assert_eq!(
            rail_neighbours(&rails, [0, 0, 0]),
            vec![[1, 1, 0], [-1, 0, 0]]
        );
        assert_eq!(rail_neighbours(&rails, [1, 1, 0]), vec![[0, 0, 0]]);
    }
}
