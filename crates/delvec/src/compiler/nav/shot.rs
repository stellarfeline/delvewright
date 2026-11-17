//! **Where a target block is shot from** (spec-0134 §2).
//!
//! A `shot` trigger fires when a projectile the player launched lands on the
//! `minecraft:target` its prop places. The static proof follows the arrow: a
//! reachable standing cell from whose eye (`metrics::PLAYER_EYE_HEIGHT` over
//! its feet) a full-drawn arrow's **flight** — the low arc through the
//! block's centre, tick by tick, as the pinned jar flies it
//! ([`delvewright_dsl::metrics::arrow::flight`], the solver the bot aims by)
//! — meets nothing before the target. The compiler finds, among the cells a
//! body stands in, one between [`SHOT_REACH_MIN`] and [`SHOT_REACH_MAX`]
//! blocks of the target with a [`clear_arc`], nearest the target first; none
//! is `DW1091`. The cell is exported on the trigger's step as `stand`, where
//! the bot draws, and it is where the route proof's leg to the step ends
//! ([`fix_stances`]): the cells it is chosen among are the walk region from
//! where the party begins ([`World::walk_region`]), every one of which a
//! leg's walk reaches, never a ledge only a jump of the launch set lands on.
//!
//! A straight line is not the flight: an arrow rises over its chord and
//! falls onto it, so a lintel on the line can sit under the arc and a ceiling
//! over the line can sit across it. The arc is the one an unscattered arrow
//! flies; the server's spread is the bot's to loose again for.
//!
//! "Meets nothing" is stricter than the sight rule a reader's eye takes
//! (`sight-through-blocks.md`): an arrow is stopped by anything a body is
//! stopped by, glass and bars included, and slowed by water, so every
//! occupied cell the flight's segments pass strictly before the target
//! blocks it.

use std::collections::BTreeSet;

use super::World;
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

use super::DW_SHOT_UNSEEN;

/// The nearest a stance may stand to the target, in blocks: closer, a drawn
/// bow's arrow leaves the player's own hitbox at the target.
pub const SHOT_REACH_MIN: i32 = 4;

/// The farthest a stance may stand from the target, in blocks: well inside a
/// full-drawn arrow's reach in every direction
/// (`vehicles-animals-and-targets.md` §14).
pub const SHOT_REACH_MAX: i32 = 40;

/// A stance: the feet cell, and its eye's distance to the target's centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotStance {
    /// The feet cell the bot stands in.
    pub cell: [i32; 3],
    /// From the eye to the target block's centre, in blocks.
    pub distance: f64,
}

/// The eye of a body standing in `cell`.
pub fn eye_of(cell: [i32; 3]) -> [f64; 3] {
    [
        f64::from(cell[0]) + 0.5,
        f64::from(cell[1]) + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
        f64::from(cell[2]) + 0.5,
    ]
}

/// Whether a full-drawn arrow loosed from `eye` on the low arc through
/// `target`'s centre meets nothing before `target`'s cell: every cell each
/// tick's segment of its [`flight`](delvewright_dsl::metrics::arrow::flight)
/// passes, up to the target, is one a body is not stopped by. No arc reaching
/// the centre is no clear arc.
pub fn clear_arc(world: &World, eye: [f64; 3], target: [i32; 3]) -> bool {
    let centre = [
        f64::from(target[0]) + 0.5,
        f64::from(target[1]) + 0.5,
        f64::from(target[2]) + 0.5,
    ];
    let Some(path) = delvewright_dsl::metrics::arrow::flight(eye, centre) else {
        return false;
    };
    for seg in path.windows(2) {
        match super::walk_cells(seg[0], seg[1], |c| c == target || world.is_occupied(c)) {
            Some(c) => return c == target,
            None => continue,
        }
    }
    false
}

/// **The stance**: among `walk` (the cells a body stands in on the way), the
/// cell between [`SHOT_REACH_MIN`] and [`SHOT_REACH_MAX`] blocks of `target`
/// whose eye has a [`clear_arc`] to it, nearest first, ties by cell order
/// (ADR-0006). `None` is `DW1091`'s.
pub fn shot_stance(
    world: &World,
    walk: &BTreeSet<[i32; 3]>,
    target: [i32; 3],
) -> Option<ShotStance> {
    let (lo, hi) = (f64::from(SHOT_REACH_MIN), f64::from(SHOT_REACH_MAX));
    let centre = [
        f64::from(target[0]) + 0.5,
        f64::from(target[1]) + 0.5,
        f64::from(target[2]) + 0.5,
    ];
    // Every cell in reach, nearest first (distances in thousandths, so the
    // order is exact, ADR-0006), ties by cell order; the first whose arc is
    // clear is the stance, and no farther cell's flight is solved.
    let mut near: Vec<(u64, [i32; 3], f64)> = walk
        .iter()
        .filter_map(|&cell| {
            let eye = eye_of(cell);
            let d = ((eye[0] - centre[0]).powi(2)
                + (eye[1] - centre[1]).powi(2)
                + (eye[2] - centre[2]).powi(2))
            .sqrt();
            (lo..=hi)
                .contains(&d)
                .then(|| ((d * 1000.0).round() as u64, cell, d))
        })
        .collect();
    near.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    near.into_iter()
        .find(|&(_, cell, _)| clear_arc(world, eye_of(cell), target))
        .map(|(_, cell, distance)| ShotStance { cell, distance })
}

/// The stance of `shot` trigger `id` at `cell`, or its `DW1091`.
pub fn stance_or_unseen(
    world: &World,
    walk: &BTreeSet<[i32; 3]>,
    id: &str,
    cell: [i32; 3],
) -> Result<ShotStance, Failure> {
    shot_stance(world, walk, cell).ok_or_else(|| Failure {
        code: DW_SHOT_UNSEEN,
        message: format!(
            "`shot` trigger `{id}`'s target at {cell:?} is reached by no full-drawn arrow's \
             arc from a cell a body stands in between {SHOT_REACH_MIN} and {SHOT_REACH_MAX} \
             blocks of it ({} walk cell(s) examined): every flight through its centre meets a \
             block first. Clear the arc from the walk — an arrow rises over the straight line \
             and falls onto it — or move the target where one reaches it",
            walk.len()
        ),
    })
}

/// What the shot proof examined, zeroes included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShotBinding {
    /// `shot` triggers judged, each with its stance and distance.
    pub stances: Vec<(String, ShotStance)>,
}

impl ShotBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        let rows: Vec<String> = self
            .stances
            .iter()
            .map(|(t, s)| format!("`{t}` from {:?} at {:.1} blocks", s.cell, s.distance))
            .collect();
        format!(
            "shot binding (DW1090/DW1091): {} shot trigger(s) judged{}{}",
            self.stances.len(),
            if rows.is_empty() { "" } else { ": " },
            rows.join("; ")
        )
    }
}

/// The bows and crossbows a shot is loosed from.
const LAUNCHERS: &[&str] = &["bow", "crossbow"];
/// The arrows they loose.
const ARROWS: &[&str] = &["arrow", "spectral_arrow", "tipped_arrow"];

/// **The shot proof** over the build's final world (spec-0134 §2): every
/// `shot` trigger owes a bow or a crossbow and arrows a body holds by the
/// critical-path step that performs it (the last step when the path does not)
/// — read off `plan.movement`, the walk `DW1043` makes over every item the
/// campaign puts in a hand — else `DW1090`; and a [`shot_stance`] in the walk
/// region (the reach graph's standing cells), else `DW1091`. The stances are fixed on the plan for the emitter.
pub fn check_shots(plan: &Plan, world: &World) -> Result<ShotBinding, Failure> {
    let mut b = ShotBinding::default();
    let targets = plan.shot_targets();
    let walk = stance_walk(plan, world, &targets);
    let fixed = fix_stances(plan, world);
    for (id, cell, _) in &targets {
        let step = plan
            .critical_path
            .iter()
            .position(|s| s.trigger() == Some(id.as_str()))
            .unwrap_or(plan.critical_path.len());
        let held = |items: &[&str]| plan.movement.first_held(items).is_some_and(|s| s <= step);
        if !(held(LAUNCHERS) && held(ARROWS)) {
            return Err(Failure {
                code: delvewright_dsl::SHOT_UNTARGETED,
                message: format!(
                    "`shot` trigger `{id}` is performed at critical-path step {step}, and no \
                     source the campaign gives puts {} in a hand by then — a target nobody can \
                     shoot. Give a bow (or a crossbow) and arrows before it: a class kit, a \
                     `give-item` on an earlier beat, a container on the way",
                    if held(LAUNCHERS) {
                        "arrows"
                    } else if held(ARROWS) {
                        "a bow or a crossbow"
                    } else {
                        "a bow or a crossbow and arrows"
                    }
                ),
            });
        }
        // The stance the route proof's leg ended at, fixed the first time a
        // proof asked; a target with none is `DW1091`'s.
        let stance = match fixed.get(id) {
            Some(&c) => ShotStance {
                cell: c,
                distance: distance(c, *cell),
            },
            None => stance_or_unseen(world, &walk, id, *cell)?,
        };
        b.stances.push((id.clone(), stance));
    }
    Ok(b)
}

/// The cells a stance is chosen among: the walk region from where the party
/// begins ([`Plan::campaign_start`], [`World::walk_region`]), every cell of
/// which a leg's walk reaches from the party's own ground — never a floor only
/// an anchor of its own seats (a belfry's, beside its target), and never a
/// ledge only a jump of the launch set lands on. A campaign with no start
/// takes the walk region from its anchors. Empty for a campaign with no shot.
fn stance_walk(
    plan: &Plan,
    world: &World,
    targets: &[(String, [i32; 3], String)],
) -> BTreeSet<[i32; 3]> {
    if targets.is_empty() {
        return BTreeSet::new();
    }
    match plan.campaign_start() {
        Some((_, pos)) => world.walk_region(&[pos]),
        None => world.walk_region_rooted(&crate::compiler::edit::anchor_starts(plan)),
    }
}

/// From a body's eye in `cell` to `target`'s centre, in blocks.
fn distance(cell: [i32; 3], target: [i32; 3]) -> f64 {
    let eye = eye_of(cell);
    (0..3)
        .map(|i| (eye[i] - (f64::from(target[i]) + 0.5)).powi(2))
        .sum::<f64>()
        .sqrt()
}

/// **Every shot's stance, fixed once per plan** (spec-0134 §2): the first
/// proof over a world that asks — the route proof's legs, the relight pass's
/// required cells, the shot proof — chooses each `shot` trigger's
/// [`shot_stance`] over that world's [`stance_walk`], and every later reader
/// (the legs, the export's `stand`, the binding) reads the same cells. A
/// target with no stance is left out, for `DW1091` to name.
pub fn fix_stances<'p>(
    plan: &'p Plan,
    world: &World,
) -> &'p std::collections::BTreeMap<String, [i32; 3]> {
    plan.shot_stances.get_or_init(|| {
        let targets = plan.shot_targets();
        let walk = stance_walk(plan, world, &targets);
        targets
            .iter()
            .filter_map(|(id, cell, _)| {
                shot_stance(world, &walk, *cell).map(|s| (id.clone(), s.cell))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::floored;

    /// A hall 50 long on x, a target block in the far wall at `[49, 66, 1]`.
    fn hall(walls: &[[i32; 3]]) -> (World, BTreeSet<[i32; 3]>) {
        let w = floored(50, 3, 65, walls);
        let walk = (0..48)
            .flat_map(|x| (0..3).map(move |z| [x, 65, z]))
            .filter(|c| w.is_standable(*c))
            .collect();
        (w, walk)
    }

    #[test]
    fn the_nearest_clear_cell_past_the_minimum_is_the_stance() {
        let (w, walk) = hall(&[[49, 65, 1], [49, 66, 1]]);
        let s = shot_stance(&w, &walk, [49, 66, 1]).expect("a clear hall has a stance");
        assert!(s.distance >= f64::from(SHOT_REACH_MIN));
        assert!(clear_arc(&w, eye_of(s.cell), [49, 66, 1]));
        // Nearest: the cell one closer stands under the minimum.
        assert!(s.distance < f64::from(SHOT_REACH_MIN) + 1.0, "{s:?}");
    }

    #[test]
    fn a_hall_walled_across_has_no_stance() {
        let wall: Vec<[i32; 3]> = (0..3).flat_map(|z| [[44, 65, z], [44, 66, z]]).collect();
        let (w, walk) = hall(&wall);
        let walk: BTreeSet<[i32; 3]> = walk.into_iter().filter(|c| c[0] < 44).collect();
        assert!(shot_stance(&w, &walk, [49, 66, 1]).is_none());
        let f = stance_or_unseen(&w, &walk, "trigger/t", [49, 66, 1]).unwrap_err();
        assert_eq!(f.code, "DW1091");
    }

    /// An open field 46 long on x, floored at `y` 64, a target block at
    /// `[40, 66, 1]`, and `extra` solid; the walk is the cells 32 to 40
    /// blocks off, whose arcs climb over a block above the straight line.
    fn field(extra: &[[i32; 3]]) -> (World, BTreeSet<[i32; 3]>) {
        let mut solid: BTreeSet<[i32; 3]> = (0..46)
            .flat_map(|x| (0..3).map(move |z| [x, 64, z]))
            .collect();
        solid.insert([40, 66, 1]);
        solid.extend(extra.iter().copied());
        let w = World::from_solid_cells(solid);
        let walk = (0..=8).map(|x| [x, 65, 1]).collect();
        (w, walk)
    }

    #[test]
    fn a_ceiling_over_the_line_that_the_arc_meets_has_no_stance() {
        // A roof one block over the head of every body, from x 18 to 24: the
        // straight line from each eye to the centre passes under it; every arc
        // rises into it.
        let roof: Vec<[i32; 3]> = (18..=24)
            .flat_map(|x| (0..3).map(move |z| [x, 67, z]))
            .collect();
        let (w, walk) = field(&roof);
        let target = [40, 66, 1];
        for c in &walk {
            let eye = eye_of(*c);
            let centre = [40.5, 66.5, 1.5];
            let line =
                crate::compiler::nav::walk_cells(eye, centre, |q| q != target && w.is_occupied(q));
            assert_eq!(line, None, "{c:?}: the straight line is clear");
        }
        assert!(shot_stance(&w, &walk, target).is_none());
        assert_eq!(
            stance_or_unseen(&w, &walk, "trigger/t", target)
                .unwrap_err()
                .code,
            "DW1091"
        );
    }

    #[test]
    fn a_block_on_the_line_under_the_arc_leaves_a_stance() {
        // One block on the straight line at x 20, which every arc from the
        // walk passes over.
        let (w, walk) = field(&[[20, 66, 1]]);
        let target = [40, 66, 1];
        for c in &walk {
            let line = crate::compiler::nav::walk_cells(eye_of(*c), [40.5, 66.5, 1.5], |q| {
                q != target && w.is_occupied(q)
            });
            assert_eq!(
                line,
                Some([20, 66, 1]),
                "{c:?}: the straight line is blocked"
            );
        }
        let s = shot_stance(&w, &walk, target).expect("the arc clears the block");
        assert_eq!(s.cell, [8, 65, 1], "nearest first");
    }

    #[test]
    fn a_target_past_the_maximum_has_no_stance() {
        let (w, walk) = hall(&[]);
        let walk: BTreeSet<[i32; 3]> = walk.into_iter().filter(|c| c[0] < 5).collect();
        assert!(shot_stance(&w, &walk, [49, 66, 1]).is_none());
    }
}
