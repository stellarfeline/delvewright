//! **What the engine says of a voxel model an author draws** — `delvec grammar
//! survey` and `delvec grammar blocks`.
//!
//! A place generator draws its building as blocks in piece-local cells and then
//! has to claim every cell: the floor a body walks into the space it stands in,
//! the floor no body reaches as out of walk, an edge between two spaces only
//! where a body crosses. Each of those is a question the engine already answers
//! when it judges the finished piece, so the generator asks it here rather than
//! keeping a second answer of its own:
//!
//! - where a body can stand: [`nav::standable_cells`];
//! - what a body reaches from the ways in: the reach graph's own flood
//!   ([`nav::body_world`], [`crate::compiler::nav::World::body_flood`]);
//! - which air no body and no sky reaches: [`contract::passable_component`],
//!   the object the out-of-walk gate's `sealed` kind quantifies over;
//! - which two spaces a walk joins: [`contract::walk_joined`], the edge
//!   proof's own answer for a `walk`;
//! - which blocks the pinned server drops: [`delvewright_dsl::support`], the
//!   `canSurvive` table `DW1002` judges the shipped world by;
//! - which blocks the admission audit refuses (`DW0730`):
//!   [`Allowlist::default_building`], the list `delvec detail` audits with;
//! - which blocks the holder census counts as containers (`DW1018`,
//!   `DW1024`): [`delvewright_dsl::blocks::container_kind`].
//!
//! The request and the answer are JSON; nothing here writes a file.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::admit::allowlist::{self, Allowlist};
use crate::grammar::block::BlockState;
use crate::grammar::contract;
use crate::grammar::geom::Box3;
use crate::grammar::model::VoxelModel;
use crate::grammar::nav;

/// What a generator asks: its model, where a body comes in, and the spaces it
/// means to declare.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// The model's extent, piece-local, `[x, y, z]`.
    pub size: [u32; 3],
    /// Every non-air cell, `[x, y, z, "minecraft:block[state]"]`.
    pub blocks: Vec<(i32, i32, i32, String)>,
    /// The inclusive box a body is kept inside while it is flooded
    /// (`[[x0, y0, z0], [x1, y1, z1]]`); the whole model when absent.
    #[serde(default)]
    pub keep: Option<[[i32; 3]; 2]>,
    /// Where a body comes in: every standable cell of these is a seed.
    #[serde(default)]
    pub from: Vec<[i32; 3]>,
    /// The spaces the generator means to declare, each as its cells; every
    /// pair a walk joins both ways comes back in `joined`.
    #[serde(default)]
    pub spaces: BTreeMap<String, Vec<[i32; 3]>>,
    /// Every block the program may write beyond `blocks` (a weighted role's
    /// other variants): judged for `refused` and `containers` as `blocks` are.
    #[serde(default)]
    pub palette: Vec<String>,
}

/// A block the pinned server drops where the model puts it.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Unsupported {
    /// Its cell.
    pub cell: [i32; 3],
    /// The block, as written.
    pub block: String,
    /// What would hold it, by the measured rule.
    pub needs: String,
}

/// A block the admission audit refuses (`DW0730`).
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Refused {
    /// The block id.
    pub block: String,
    /// The DSL surface the block is had through instead, if there is one.
    pub route: Option<&'static str>,
}

/// The engine's answers.
#[derive(Debug, Serialize)]
pub struct Answer {
    /// Every cell a body can stand in.
    pub standable: Vec<[i32; 3]>,
    /// Every rest state a body reaches from the standable cells of `from`,
    /// kept inside `keep`.
    pub reached: Vec<[i32; 3]>,
    /// Every passable cell of an air pocket that holds an unreached standable
    /// cell inside `keep` and that neither the model's outer layer nor any
    /// reached cell touches: what the out-of-walk gate calls sealed.
    pub sealed: Vec<[i32; 3]>,
    /// Every pair of `spaces` (in name order) a walk joins both ways over
    /// their reached standable cells, as the edge proof proves a `walk` edge.
    pub joined: Vec<[String; 2]>,
    /// Every block the pinned server drops.
    pub unsupported: Vec<Unsupported>,
    /// Every distinct block the admission audit refuses.
    pub refused: Vec<Refused>,
    /// Every distinct block the holder census counts as a container.
    pub containers: Vec<String>,
}

/// The block id of a state: `minecraft:lantern[hanging=true]` → `minecraft:lantern`.
fn base_id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

/// Build the model a request names, refusing a block that does not parse or a
/// cell outside the extent.
fn model_of(req: &Request) -> Result<VoxelModel, String> {
    let region = Box3::at_origin(req.size);
    let mut model = VoxelModel::new(region);
    let max = region.maximum();
    for (x, y, z, state) in &req.blocks {
        let pos = [*x, *y, *z];
        if (0..3).any(|a| pos[a] < 0 || pos[a] >= max[a]) {
            return Err(format!(
                "the block `{state}` at {pos:?} lies outside the model's extent {:?}",
                req.size
            ));
        }
        let block: BlockState = state
            .parse()
            .map_err(|e| format!("the block at {pos:?} does not parse: {e}"))?;
        model.set(pos, &block).map_err(|_| {
            "the model holds more distinct block states than a palette can".to_string()
        })?;
    }
    Ok(model)
}

/// Answer a request.
pub fn survey(req: &Request) -> Result<Answer, String> {
    let model = model_of(req)?;
    let region = model.region();
    let max = region.maximum();
    let keep = req
        .keep
        .unwrap_or([[0, 0, 0], [max[0] - 1, max[1] - 1, max[2] - 1]]);
    let inside = |c: [i32; 3]| (0..3).all(|a| keep[0][a] <= c[a] && c[a] <= keep[1][a]);

    // A `structure_void` cell is not the piece's: no body stands in it on
    // this piece's account, as the contract's own index reads it.
    let standable: BTreeSet<[i32; 3]> = nav::standable_cells(&model)
        .into_iter()
        .filter(|c| {
            !model
                .get(*c)
                .is_some_and(|b| b.name == "minecraft:structure_void")
        })
        .collect();
    let seeds: Vec<[i32; 3]> = req
        .from
        .iter()
        .filter(|c| standable.contains(*c) && inside(**c))
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let world = nav::body_world(&model);
    let reached = world.body_flood(
        &seeds,
        &crate::compiler::nav::Confinement::cells(&|next| inside(next)),
    );

    // A pocket is searched from an unreached floor cell's air; it is sealed
    // when it touches neither the end of the model nor anywhere a body goes.
    let mut sealed: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut searched: BTreeSet<[i32; 3]> = BTreeSet::new();
    for cell in standable
        .iter()
        .filter(|c| inside(**c) && !reached.contains(*c))
    {
        if searched.contains(cell) {
            continue;
        }
        let pocket = contract::passable_component(&model, *cell);
        searched.extend(pocket.iter().copied());
        let open = pocket
            .iter()
            .any(|c| contract::on_outer_layer(&model, *c) || reached.contains(c));
        if !open {
            sealed.extend(pocket);
        }
    }

    // A space's floor is the standable cells of it a body reaches (all of
    // them when the request names no way in).
    let floor_of = |cells: &Vec<[i32; 3]>| -> BTreeSet<[i32; 3]> {
        cells
            .iter()
            .filter(|c| standable.contains(*c) && (req.from.is_empty() || reached.contains(*c)))
            .copied()
            .collect()
    };
    let spaces: Vec<(&String, BTreeSet<[i32; 3]>)> =
        req.spaces.iter().map(|(n, c)| (n, floor_of(c))).collect();
    let mut joined = Vec::new();
    for (i, (a, ca)) in spaces.iter().enumerate() {
        for (b, cb) in spaces.iter().skip(i + 1) {
            if ca.is_empty() || cb.is_empty() {
                continue;
            }
            let graph: BTreeSet<[i32; 3]> = ca.union(cb).copied().collect();
            if contract::walk_joined(&model, &graph, ca, cb) {
                joined.push([(*a).clone(), (*b).clone()]);
            }
        }
    }

    let mut unsupported = Vec::new();
    for pos in region.positions() {
        let Some(block) = model.get(pos) else {
            continue;
        };
        if block.is_air() {
            continue;
        }
        let state = block.to_string();
        let Some(delvewright_dsl::support::Needs::Support(rules)) =
            delvewright_dsl::support::needs(&state)
        else {
            continue;
        };
        let at = |f: delvewright_dsl::blockshape::Face| -> String {
            let o = f.offset();
            model
                .get([pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]])
                .map_or_else(|| "minecraft:air".to_string(), |b| b.to_string())
        };
        if rules.iter().any(|r| r.holds(&at(r.cell))) {
            continue;
        }
        let needs = rules
            .iter()
            .map(|r| {
                format!(
                    "the block {} to {} (there is `{}`)",
                    crate::compiler::attached::beside(r.cell),
                    r.describe(),
                    at(r.cell)
                )
            })
            .collect::<Vec<_>>()
            .join("; or ");
        unsupported.push(Unsupported {
            cell: pos,
            block: state,
            needs,
        });
    }

    let ids: BTreeSet<String> = req
        .blocks
        .iter()
        .map(|b| b.3.as_str())
        .chain(req.palette.iter().map(String::as_str))
        .map(|s| base_id(s).to_string())
        .filter(|id| id != "minecraft:air")
        .collect();
    let allow = Allowlist::default_building();
    let refused = ids
        .iter()
        .filter(|id| !allow.permits(id))
        .map(|id| Refused {
            block: id.clone(),
            route: allowlist::route_for(id),
        })
        .collect();
    let containers = ids
        .iter()
        .filter(|id| delvewright_dsl::blocks::container_kind(id).is_some())
        .cloned()
        .collect();

    Ok(Answer {
        standable: standable.into_iter().collect(),
        reached: reached.into_iter().collect(),
        sealed: sealed.into_iter().collect(),
        joined,
        unsupported,
        refused,
        containers,
    })
}

/// The pinned registry's blocks, as the two lists a place author plans with.
#[derive(Debug, Serialize)]
pub struct Blocks {
    /// Every block of the pin the admission audit admits.
    pub admitted: Vec<String>,
    /// Every block of the pin it refuses (`DW0730`), with the surface it is
    /// had through instead where one exists.
    pub refused: Vec<Refused>,
    /// Every block the holder census counts as a container (`DW1018`,
    /// `DW1024`).
    pub containers: Vec<String>,
}

/// [`Blocks`], read off the pinned registry, the default allowlist and the
/// container table.
pub fn blocks() -> Blocks {
    let allow = Allowlist::default_building();
    let mut admitted = Vec::new();
    let mut refused = Vec::new();
    let mut containers = Vec::new();
    let reg = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let ids: BTreeSet<&str> = reg.ids().collect();
    for id in ids {
        if id == "minecraft:air" {
            continue;
        }
        if allow.permits(id) {
            admitted.push(id.to_string());
        } else {
            refused.push(Refused {
                block: id.to_string(),
                route: allowlist::route_for(id),
            });
        }
        if delvewright_dsl::blocks::container_kind(id).is_some() {
            containers.push(id.to_string());
        }
    }
    Blocks {
        admitted,
        refused,
        containers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(size: [u32; 3]) -> Vec<(i32, i32, i32, String)> {
        let mut out = Vec::new();
        for x in 0..size[0] as i32 {
            for z in 0..size[2] as i32 {
                out.push((x, 0, z, "minecraft:stone".to_string()));
            }
        }
        out
    }

    /// A floor split by a wall: the far half is unreached, a walk joins
    /// nothing across the wall, and opening a door in it joins the two.
    #[test]
    fn a_wall_parts_the_reach_and_a_door_joins_two_spaces() {
        let size = [7, 5, 3];
        let mut blocks = floor(size);
        for y in 1..4 {
            for z in 0..3 {
                blocks.push((3, y, z, "minecraft:stone".to_string()));
            }
        }
        let west: Vec<[i32; 3]> = (0..3)
            .flat_map(|x| (0..3).map(move |z| [x, 1, z]))
            .collect();
        let east: Vec<[i32; 3]> = (4..7)
            .flat_map(|x| (0..3).map(move |z| [x, 1, z]))
            .collect();
        let mut spaces = BTreeMap::new();
        spaces.insert("west".to_string(), west.clone());
        spaces.insert("east".to_string(), east.clone());
        let req = Request {
            size,
            blocks: blocks.clone(),
            keep: None,
            from: vec![[0, 1, 1]],
            spaces: spaces.clone(),
            palette: vec![],
        };
        let a = survey(&req).unwrap();
        assert!(a.reached.contains(&[2, 1, 1]));
        assert!(!a.reached.contains(&[5, 1, 1]));
        assert!(a.joined.is_empty());
        blocks.retain(|b| !(b.0 == 3 && b.2 == 1 && (b.1 == 1 || b.1 == 2)));
        // The doorway's floor is the west room's: every cell a body crosses
        // belongs to one of the spaces the walk is proved over.
        spaces.get_mut("west").unwrap().push([3, 1, 1]);
        let open = Request {
            size,
            blocks,
            keep: None,
            from: vec![[0, 1, 1]],
            spaces,
            palette: vec![],
        };
        let b = survey(&open).unwrap();
        assert!(b.reached.contains(&[5, 1, 1]));
        assert_eq!(b.joined, vec![["east".to_string(), "west".to_string()]]);
    }

    /// A lantern hung under air is dropped; one under stone is kept. A fence
    /// top is no floor. A candle is refused and a barrel is a container.
    #[test]
    fn support_admission_and_containers_are_the_engines() {
        let size = [3, 5, 3];
        let mut blocks = floor(size);
        blocks.push((
            0,
            3,
            0,
            "minecraft:lantern[hanging=true,waterlogged=false]".to_string(),
        ));
        blocks.push((2, 4, 2, "minecraft:stone".to_string()));
        blocks.push((
            2,
            3,
            2,
            "minecraft:lantern[hanging=true,waterlogged=false]".to_string(),
        ));
        blocks.push((
            1,
            1,
            1,
            "minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]"
                .to_string(),
        ));
        blocks.push((
            0,
            1,
            2,
            "minecraft:barrel[facing=up,open=false]".to_string(),
        ));
        let req = Request {
            size,
            blocks,
            keep: None,
            from: vec![],
            spaces: BTreeMap::new(),
            palette: vec!["minecraft:candle[candles=1,lit=false,waterlogged=false]".to_string()],
        };
        let a = survey(&req).unwrap();
        assert_eq!(
            a.unsupported.iter().map(|u| u.cell).collect::<Vec<_>>(),
            vec![[0, 3, 0]]
        );
        assert!(!a.standable.contains(&[1, 2, 1]), "a fence top is no floor");
        assert!(a.refused.iter().any(|r| r.block == "minecraft:candle"));
        assert_eq!(a.containers, vec!["minecraft:barrel".to_string()]);
    }

    #[test]
    fn the_block_lists_partition_the_pin() {
        let b = blocks();
        assert!(b.admitted.iter().any(|id| id == "minecraft:stone_bricks"));
        assert!(
            b.refused
                .iter()
                .any(|r| r.block == "minecraft:blast_furnace")
        );
        assert!(b.containers.iter().any(|id| id == "minecraft:smoker"));
        let n = delvewright_dsl::blocks::BlockRegistry::v1_21_11()
            .ids()
            .count();
        assert_eq!(
            b.admitted.len() + b.refused.len() + 1,
            n,
            "air aside, every block is in one list"
        );
    }
}
