//! **Where an area is used from** — an `interact` pressed over a `region`
//! instead of one block (`DW1143`).
//!
//! An area's body is the tiling of its box
//! ([`crate::compiler::clickbox::ClickBox::tiling`]): a use whose ray reaches
//! anything inside the box lands on a tile. What the build still owes is a
//! place the party can make that use from, and an area whose declared top face
//! the world leaves open to it:
//!
//! * **a stance** — a standable cell of the walk region the party walks to on
//!   cue (rooted at every placed anchor), outside the area's own columns, from whose eye a cell of the
//!   area's top face is within the entity reach
//!   ([`crate::compiler::crosshair::INTERACTION_REACH`]) along a line no solid
//!   block stops ([`eye_point_sees`], the one sight rule a press is judged
//!   by). Of the stances, the one nearest the objective's `anchor` is chosen
//!   (ties in cell order), with the nearest such top cell from its eye: the
//!   route proof's leg to the step ends there ([`super::fix_stances`]), and
//!   the export hands it to the bot as `stand`, with the cell and the point it
//!   clicks;
//! * **an uncovered top face** — no solid block stands directly on a cell of
//!   the box's top face. A block there is a lid over that part of the area: a
//!   use from above lands on the lid, outside the area, and the declared
//!   surface is not the one a player can use.
//!
//! A body standing inside the area is `DW0359`'s, judged over the tiles as
//! over every press body ([`crate::compiler::eclipse`]).

use std::collections::{BTreeMap, BTreeSet};

use super::{BodyBox, World, eye_point_sees, eye_point_within, interaction_stances};
use crate::compiler::failure::Failure;
use crate::compiler::plan::Plan;

use super::DW_AREA_UNCLICKABLE;

/// An area's box, as its inclusive corners.
pub type AreaBox = ([i32; 3], [i32; 3]);

/// Where an area `interact` is used from, as the build proved it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AreaClick {
    /// The feet cell the party stands in.
    pub stance: [i32; 3],
    /// The cell of the area's top face the use aims at.
    pub cell: [i32; 3],
    /// The point aimed at: the centre of that cell's top face.
    pub aim: [f64; 3],
}

/// The area's whole box, in blocks.
fn area_box((lo, hi): ([i32; 3], [i32; 3])) -> BodyBox {
    BodyBox {
        lo: lo.map(f64::from),
        hi: [
            f64::from(hi[0]) + 1.0,
            f64::from(hi[1]) + 1.0,
            f64::from(hi[2]) + 1.0,
        ],
    }
}

/// Whether a body standing in `c` stands inside the area's own columns —
/// in the pond, on the field it is using: never a stance.
fn in_columns(c: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])) -> bool {
    (lo[0]..=hi[0]).contains(&c[0])
        && (lo[2]..=hi[2]).contains(&c[2])
        && (lo[1] - 1..=hi[1] + 2).contains(&c[1])
}

/// The cells of the box's top face, in cell order.
fn top_cells((lo, hi): ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let mut v = Vec::new();
    for x in lo[0]..=hi[0] {
        for z in lo[2]..=hi[2] {
            v.push([x, hi[1], z]);
        }
    }
    v
}

/// The cells of the top face a solid block stands directly on, in cell order.
pub fn covered(world: &World, region: ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    top_cells(region)
        .into_iter()
        .filter(|c| world.solid_at([c[0], c[1] + 1, c[2]]))
        .collect()
}

/// The eye of a body standing in `c`.
fn eye(world: &World, c: [i32; 3]) -> [f64; 3] {
    [
        f64::from(c[0]) + 0.5,
        world.feet_y(c) + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
        f64::from(c[2]) + 0.5,
    ]
}

/// **Where the area over `region` is used from** (see the module docs): the
/// stance nearest `near` among the walk region's cells (`walk`), with the
/// nearest top-face cell its eye reaches and sees. `None` when no cell of the
/// walk region clicks the area.
pub fn area_click(
    world: &World,
    walk: &BTreeSet<[i32; 3]>,
    region: ([i32; 3], [i32; 3]),
    near: [i32; 3],
) -> Option<AreaClick> {
    let reach = crate::compiler::crosshair::INTERACTION_REACH;
    let tops = top_cells(region);
    let d2 =
        |a: [i32; 3], b: [i32; 3]| -> i64 { (0..3).map(|i| i64::from(a[i] - b[i]).pow(2)).sum() };
    let mut best: Option<(i64, AreaClick)> = None;
    for stance in interaction_stances(world, &area_box(region), walk) {
        if in_columns(stance, region) {
            continue;
        }
        let e = eye(world, stance);
        let gap = |c: &[i32; 3]| {
            (0..3)
                .map(|i| (f64::from(c[i]) + 0.5 - e[i]).powi(2))
                .sum::<f64>()
        };
        let Some(cell) = tops
            .iter()
            .filter(|c| {
                let b = BodyBox::block(**c);
                eye_point_within(e, &b, reach) && eye_point_sees(world, e, &b)
            })
            .min_by(|a, b| gap(a).total_cmp(&gap(b)).then(a.cmp(b)))
            .copied()
        else {
            continue;
        };
        let click = AreaClick {
            stance,
            cell,
            aim: [
                f64::from(cell[0]) + 0.5,
                f64::from(cell[1]) + 1.0,
                f64::from(cell[2]) + 0.5,
            ],
        };
        let key = d2(stance, near);
        // `interaction_stances` yields cells in order, so the first at the
        // least distance is the least cell among them.
        if best.as_ref().is_none_or(|(k, _)| key < *k) {
            best = Some((key, click));
        }
    }
    best.map(|(_, c)| c)
}

/// The area `interact`s this build places, as `(objective id, region, the
/// cell its anchor resolves to)`, in [`Plan::presses`] order.
pub fn areas(plan: &Plan) -> Vec<(String, AreaBox, [i32; 3])> {
    use crate::compiler::pressable::Owner;
    plan.presses()
        .iter()
        .filter_map(|p| {
            let a = p.area.as_ref()?;
            let Owner::Interact { id, .. } = &p.owner else {
                return None;
            };
            let near = plan.point_any(&p.anchor).unwrap_or(p.cell);
            Some((id.clone(), a.region, near))
        })
        .collect()
}

/// The cells a stance is chosen among: the walk region rooted at every
/// placed anchor ([`World::walk_region_rooted`]), the cells the party walks to
/// on cue in whichever area it has been carried to — the relation a holder's
/// stance is chosen in (`DW1037`). Empty for a campaign with no area.
pub(super) fn area_walk(plan: &Plan, world: &World) -> BTreeSet<[i32; 3]> {
    if areas(plan).is_empty() {
        return BTreeSet::new();
    }
    world.walk_region_rooted(&crate::compiler::edit::anchor_starts(plan))
}

/// Every area's click over `world`, by objective id; an area with none is
/// left out, for [`check_areas`] to name.
pub(super) fn clicks(plan: &Plan, world: &World) -> BTreeMap<String, AreaClick> {
    let walk = area_walk(plan, world);
    areas(plan)
        .into_iter()
        .filter_map(|(id, region, near)| area_click(world, &walk, region, near).map(|c| (id, c)))
        .collect()
}

/// **Every area's click, fixed once per plan**: the first proof over a world
/// that asks (the route proof's legs through [`super::fix_stances`], the area
/// proof) chooses each, and every later reader — the legs, the export, the
/// binding — reads the same cells.
pub fn fix_area_clicks<'p>(plan: &'p Plan, world: &World) -> &'p BTreeMap<String, AreaClick> {
    plan.area_clicks.get_or_init(|| clicks(plan, world))
}

/// What the area proof examined, stated whichever way it went.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AreaBinding {
    /// `(objective id, region, its click)`, in [`Plan::presses`] order.
    pub areas: Vec<(String, AreaBox, AreaClick)>,
}

impl AreaBinding {
    /// The binding line, zeroes included.
    pub fn line(&self) -> String {
        let rows: Vec<String> = self
            .areas
            .iter()
            .map(|(id, (lo, hi), c)| {
                format!(
                    "`{id}` over {lo:?}..{hi:?} from {:?}, clicking {:?}",
                    c.stance, c.cell
                )
            })
            .collect();
        format!(
            "area binding (DW1143): {} area interact(s) judged{}{}",
            self.areas.len(),
            if rows.is_empty() { "" } else { ": " },
            rows.join("; ")
        )
    }
}

/// **The area proof** over the build's final world: every area `interact`
/// has an uncovered top face and a stance (see the module docs), else
/// `DW1143` naming the area, the covered cells or the nearest walkable cell.
pub fn check_areas(plan: &Plan, world: &World) -> Result<AreaBinding, Failure> {
    let fixed = fix_area_clicks(plan, world);
    let mut b = AreaBinding::default();
    for (id, region, near) in areas(plan) {
        let lid = covered(world, region);
        if !lid.is_empty() {
            return Err(Failure {
                code: DW_AREA_UNCLICKABLE,
                message: format!(
                    "the area of `interact` objective `{id}`, {:?}..{:?}, has a block standing \
                     directly on its top face at {} of its {} top cell(s) ({}). A use from above \
                     lands on that block, outside the area, so part of the surface the objective \
                     declares is one nobody can use. Prescription: lower the area's top \
                     (`extent` or the `region` anchor) to the surface the party sees — the water \
                     of the pond, the soil of the field — or move the block off it",
                    region.0,
                    region.1,
                    lid.len(),
                    top_cells(region).len(),
                    lid.iter()
                        .map(|c| format!("{c:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }
        let Some(click) = fixed.get(&id) else {
            let walk = area_walk(plan, world);
            let nearest = walk
                .iter()
                .min_by_key(|c| {
                    (0..3)
                        .map(|i| i64::from(c[i] - near[i]).pow(2))
                        .sum::<i64>()
                })
                .map_or_else(|| "none".to_string(), |c| format!("{c:?}"));
            return Err(Failure {
                code: DW_AREA_UNCLICKABLE,
                message: format!(
                    "the area of `interact` objective `{id}`, {:?}..{:?}, can be used from \
                     nowhere: no standable cell the party can walk to, outside the area's own \
                     columns, has its eye within {} blocks (the entity reach) of a cell of the \
                     area's top face along a line no block stops. The walkable cell nearest the \
                     objective's anchor is {nearest}. Prescription: give the area an edge the \
                     party can stand at — a rim, a bank, a path beside it — within reach of \
                     its surface",
                    region.0,
                    region.1,
                    crate::compiler::crosshair::INTERACTION_REACH
                ),
            });
        };
        b.areas.push((id, region, *click));
    }
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::floored;

    /// A 9 x 9 floor at y=65 (ceiling at 67), with a pond cut 3 x 3 into it:
    /// its cells at y=64 hold no block (the water a pick passes), its bed at 63.
    fn pond() -> (World, ([i32; 3], [i32; 3])) {
        let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
        for x in 0..9 {
            for z in 0..9 {
                let pond = (3..=5).contains(&x) && (3..=5).contains(&z);
                solid.insert(if pond { [x, 63, z] } else { [x, 64, z] });
                solid.insert([x, 67, z]);
            }
        }
        (World::from_solid_cells(solid), ([3, 64, 3], [5, 64, 5]))
    }

    #[test]
    fn a_pond_is_used_from_its_bank_at_the_cell_nearest_the_eye() {
        let (w, region) = pond();
        let walk = w.walk_region(&[[0, 65, 0]]);
        let c = area_click(&w, &walk, region, [4, 64, 4]).expect("the bank reaches the water");
        assert!(
            !in_columns(c.stance, region),
            "{c:?}: a stance is never in the pond"
        );
        assert!(walk.contains(&c.stance));
        assert!(top_cells(region).contains(&c.cell));
        let e = eye(&w, c.stance);
        assert!(eye_point_within(e, &BodyBox::block(c.cell), 3.0), "{c:?}");
        // The bank cell beside the pond's middle row is as near the anchor as
        // any; the least in cell order of those is taken, and its nearest top
        // cell is the pond's own edge in front of it.
        assert_eq!(c.stance, [2, 65, 4]);
        assert_eq!(c.cell, [3, 64, 4]);
        assert_eq!(c.aim, [3.5, 65.0, 4.5]);
        assert!(covered(&w, region).is_empty());
    }

    #[test]
    fn an_area_out_of_every_stance_reach_has_no_click() {
        let (w, _) = pond();
        let walk = w.walk_region(&[[0, 65, 0]]);
        let far = ([30, 64, 30], [32, 64, 32]);
        assert_eq!(area_click(&w, &walk, far, [31, 64, 31]), None);
    }

    #[test]
    fn a_wall_between_the_bank_and_the_pond_takes_every_click() {
        // A ring of wall two high round the pond, its top over every eye.
        let (w, region) = pond();
        let mut solid: BTreeSet<[i32; 3]> = BTreeSet::new();
        for x in 0..9 {
            for z in 0..9 {
                for y in 63..=67 {
                    if w.solid_at([x, y, z]) {
                        solid.insert([x, y, z]);
                    }
                }
                let ring = (2..=6).contains(&x)
                    && (2..=6).contains(&z)
                    && !((3..=5).contains(&x) && (3..=5).contains(&z));
                if ring {
                    for y in 65..=66 {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        let walled = World::from_solid_cells(solid);
        let walk = walled.walk_region(&[[0, 65, 0]]);
        assert_eq!(area_click(&walled, &walk, region, [4, 64, 4]), None);
    }

    #[test]
    fn a_block_on_the_top_face_covers_the_cell_under_it() {
        let w = floored(9, 9, 65, &[[4, 66, 4]]);
        // The floor-level layer, a block standing on its middle cell.
        assert_eq!(covered(&w, ([3, 65, 3], [5, 65, 5])), vec![[4, 65, 4]]);
        // The ceiling at 67 lies on the top face of the layer under it.
        assert_eq!(covered(&w, ([3, 66, 3], [5, 66, 5])).len(), 9);
        assert!(covered(&w, ([0, 65, 0], [2, 65, 2])).is_empty());
    }
}
