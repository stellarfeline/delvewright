//! **Every block the build writes is one the server keeps** (`DW1002`).
//!
//! A wall torch hung on a glass pane, a lantern hung under air, a flower on
//! stone, a rail on a fence: the build writes each of them and the pinned
//! server drops it the first time a shape update reaches it, because its
//! `canSurvive` rule asks a neighbour for something the neighbour does not
//! give. Until this check the only thing that noticed was the staging-time
//! comparison of the written world with a server save (`DW0955`), an hour of
//! the ladder after the build said yes.
//!
//! The rule is the pinned jar's, measured ([`delvewright_dsl::support`]); the
//! world judged is the one `DW0955` compares:
//!
//! * **at load** — [`crate::compiler::view::beat::load_blocks`] over
//!   [`crate::compiler::view::beat::picture_base`]: the assembled pieces, the
//!   world edits, the world-load seals, the relight fixtures and the trigger
//!   props, in the critical path's configuration at its first step. Every
//!   block that needs support is judged;
//! * **after each runtime write the model lays** — every distinct
//!   configuration the critical path passes ([`crate::compiler::nav::path_configurations`]),
//!   its forced writes laid as the emitted commands write them. Each written
//!   cell and its six neighbours are judged again there. An unforced write is
//!   not laid by the model, and a branch's configurations are not walked:
//!   neither is judged.
//!
//! A cell no block was written to is air, except under the `ocean` horizon,
//! where the generator's stone and water stand outside every placed piece's
//! box. A block whose rule the table cannot read from single neighbours
//! (`unjudged`: a crop's light, a vine's far reads) is counted by block and
//! not judged.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::blockshape::Face;
use delvewright_dsl::support::{Needs, Rule, needs};
use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::nav::{Ambient, BuiltPiece, World};
use crate::compiler::plan::Plan;

delvewright_dsl::dw_code! {
    /// `DW1002`: a block the build writes is one the pinned server drops — its
    /// `canSurvive` rule, measured from the jar
    /// (`crates/dsl/data/support-1.21.11.tsv`), is met by none of its
    /// neighbours in the world at load, or in a configuration the critical path
    /// passes. The message names the block, the cell, each neighbour that could
    /// hold it, what that neighbour must give, and what stands there.
    pub const DW_BLOCK_DROPPED: DwCode = DwCode::new("DW1002", ExitTier::Build);
}

/// How many dropped cells a refusal names.
const LIST_LIMIT: usize = 8;

const FACES: [Face; 6] = [
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
    Face::West,
    Face::East,
];

/// What the generator puts where nothing was written.
struct Surround {
    ambient: Ambient,
    built: Vec<BuiltPiece>,
}

impl Surround {
    fn of(plan: &Plan) -> Surround {
        Surround {
            ambient: Ambient::of_plan(plan),
            built: crate::compiler::nav::built_volume(plan),
        }
    }

    fn inside(&self, c: [i32; 3]) -> Option<&str> {
        self.built
            .iter()
            .find(|(_, (lo, hi))| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]))
            .map(|(id, _)| id.as_str())
    }

    /// The block at an unwritten cell: air inside a piece's box (`/place
    /// template` writes the whole box) and under the void horizon; the ocean
    /// superflat's layers outside every box.
    fn unwritten(&self, c: [i32; 3]) -> &'static str {
        match &self.ambient {
            Ambient::Void => "minecraft:air",
            Ambient::Ocean(sea) => {
                if self.inside(c).is_some() || c[1] > sea.level {
                    "minecraft:air"
                } else if c[1] > sea.floor_top {
                    "minecraft:water"
                } else if c[1] == crate::compiler::view::world::MIN_SECTION * 16 {
                    "minecraft:bedrock"
                } else if c[1] < crate::compiler::view::world::MIN_SECTION * 16 {
                    "minecraft:air"
                } else {
                    "minecraft:stone"
                }
            }
        }
    }
}

/// What one rule asks and what stands there: `(rule, neighbour cell,
/// neighbour block)`.
pub type Want = (Rule, [i32; 3], String);

/// One dropped block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    /// Where.
    pub cell: [i32; 3],
    /// The block state written there.
    pub block: String,
    /// What each rule asks and what stands there: `(rule, neighbour cell,
    /// neighbour block)`.
    pub wants: Vec<Want>,
    /// The piece whose box holds the cell, if any.
    pub piece: Option<String>,
    /// `None` at load; the critical step a runtime configuration arrives at.
    pub after_step: Option<usize>,
}

/// What the judgement bound to, and what it found.
#[derive(Debug, Default)]
pub struct Binding {
    /// Non-air cells of the load world.
    pub cells: usize,
    /// Of them, blocks that need support (a `support` row).
    pub attached: usize,
    /// Of those, held.
    pub held: usize,
    /// Cells whose rule is unjudged, by block id.
    pub unjudged: BTreeMap<String, usize>,
    /// Cells holding a block the table does not hold, by block id.
    pub unknown: BTreeMap<String, usize>,
    /// Runtime configurations judged, and the cells re-judged across them.
    pub configurations: usize,
    pub rejudged: usize,
    /// Every dropped block, load first, then by configuration.
    pub dropped: Vec<Dropped>,
}

impl Binding {
    /// The binding line, printed on every build.
    pub fn line(&self) -> String {
        let list = |m: &BTreeMap<String, usize>| {
            if m.is_empty() {
                "none".to_string()
            } else {
                m.iter()
                    .map(|(b, n)| format!("{b} x{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };
        format!(
            "attached blocks (DW1002): {} of {} non-air cell(s) at load need a neighbour to stay — \
             {} held, {} dropped; {} configuration(s) the critical path passes re-judged {} cell(s) \
             around their writes, {} dropped there; unjudged by the measured rule: {} cell(s) ({}); \
             not in the table: {} cell(s) ({})",
            self.attached,
            self.cells,
            self.held,
            self.dropped
                .iter()
                .filter(|d| d.after_step.is_none())
                .count(),
            self.configurations,
            self.rejudged,
            self.dropped
                .iter()
                .filter(|d| d.after_step.is_some())
                .count(),
            self.unjudged.values().sum::<usize>(),
            list(&self.unjudged),
            self.unknown.values().sum::<usize>(),
            list(&self.unknown),
        )
    }

    /// The refusal, when anything was dropped.
    pub fn refusal(&self) -> Option<String> {
        self.dropped.first()?;
        let mut msg = format!(
            "{} block(s) the build writes are dropped by the pinned server: each one's \
             `canSurvive` rule (measured from the jar, `crates/dsl/data/support-1.21.11.tsv`) is \
             met by none of its neighbours, so the server removes it the first time a shape \
             update reaches it, and the world the party walks is not the world the build wrote \
             (the staging comparison would find it as `DW0955`).",
            self.dropped.len()
        );
        for d in self.dropped.iter().take(LIST_LIMIT) {
            msg.push_str(&format!("\n  - {}", describe(d)));
        }
        if self.dropped.len() > LIST_LIMIT {
            msg.push_str(&format!(
                "\n  … and {} more.",
                self.dropped.len() - LIST_LIMIT
            ));
        }
        msg.push_str(
            "\nRepair it where the block is authored (the piece, or the edit that writes it): \
             give it the neighbour it asks for, hang it on a face that holds it, or choose a \
             block that needs nothing there.",
        );
        Some(msg)
    }
}

fn describe(d: &Dropped) -> String {
    let [x, y, z] = d.cell;
    let when = match d.after_step {
        None => "at load".to_string(),
        Some(s) => format!("once the critical path reaches step {s}"),
    };
    let piece = d
        .piece
        .as_ref()
        .map(|p| format!(" (in the box of piece `{p}`)"))
        .unwrap_or_default();
    let wants = d
        .wants
        .iter()
        .map(|(rule, [nx, ny, nz], there)| {
            format!(
                "the block {} at [{nx}, {ny}, {nz}] to {}, and there is `{there}`",
                beside(rule.cell),
                rule.describe()
            )
        })
        .collect::<Vec<_>>()
        .join("; or ");
    format!(
        "`{}` at [{x}, {y}, {z}]{piece} {when} needs {wants}",
        d.block
    )
}

/// Where the neighbour across `f` stands, in words.
pub(crate) fn beside(f: Face) -> String {
    match f {
        Face::Down => "below it".to_string(),
        Face::Up => "above it".to_string(),
        other => format!("{} of it", other.name()),
    }
}

fn step(c: [i32; 3], f: Face) -> [i32; 3] {
    let o = f.offset();
    [c[0] + o[0], c[1] + o[1], c[2] + o[2]]
}

/// Judge one cell whose block is `name`, with `at` answering any cell.
fn judge_cell<'a>(
    cell: [i32; 3],
    name: &str,
    at: &dyn Fn([i32; 3]) -> &'a str,
) -> Option<Result<(), Vec<Want>>> {
    match needs(name)? {
        Needs::Free | Needs::Unjudged(_) => None,
        Needs::Support(rules) => {
            if rules.iter().any(|r| r.holds(at(step(cell, r.cell)))) {
                Some(Ok(()))
            } else {
                Some(Err(rules
                    .iter()
                    .map(|r| {
                        let n = step(cell, r.cell);
                        (r.clone(), n, at(n).to_string())
                    })
                    .collect()))
            }
        }
    }
}

/// **Judge the world the build writes** (module header): the load world, then
/// every configuration the critical path passes around its laid writes.
pub fn judge(plan: &Plan, world: &World, base: &BlockMap) -> Binding {
    let surround = Surround::of(plan);
    let load = crate::compiler::view::beat::load_blocks(plan, world, base);
    let mut b = judge_load(&load, &surround);
    let load_dropped: BTreeSet<[i32; 3]> = b.dropped.iter().map(|d| d.cell).collect();

    let (configs, _) = crate::compiler::nav::path_configurations(plan, world);
    for config in &configs {
        let over = config.laid();
        if over.is_empty() {
            continue;
        }
        b.configurations += 1;
        let at = |c: [i32; 3]| -> &str {
            match over.get(&c) {
                Some(Some(s)) => s,
                Some(None) => "minecraft:air",
                None => base
                    .get(&c)
                    .map(|s| &**s)
                    .unwrap_or_else(|| surround.unwritten(c)),
            }
        };
        let mut around: BTreeSet<[i32; 3]> = BTreeSet::new();
        for c in over.keys() {
            around.insert(*c);
            around.extend(FACES.iter().map(|f| step(*c, *f)));
        }
        for cell in around {
            let name = at(cell);
            if delvewright_dsl::blockshape::is_air(name) || load_dropped.contains(&cell) {
                continue;
            }
            b.rejudged += 1;
            if let Some(Err(wants)) = judge_cell(cell, name, &at)
                && !b.dropped.iter().any(|d| d.cell == cell && d.block == name)
            {
                b.dropped.push(Dropped {
                    cell,
                    block: name.to_string(),
                    wants,
                    piece: surround.inside(cell).map(str::to_string),
                    after_step: Some(config.step),
                });
            }
        }
    }
    b
}

/// The load half of [`judge`]: every non-air cell of `load`, its unwritten
/// neighbours answered by `surround`.
fn judge_load(load: &BlockMap, surround: &Surround) -> Binding {
    let mut b = Binding::default();
    let at_load = |c: [i32; 3]| -> &str {
        load.get(&c)
            .map(|s| &**s)
            .unwrap_or_else(|| surround.unwritten(c))
    };
    for (cell, state) in load {
        let name: &str = state;
        if delvewright_dsl::blockshape::is_air(name) {
            continue;
        }
        b.cells += 1;
        match needs(name) {
            None => *b.unknown.entry(base_name(name)).or_default() += 1,
            Some(Needs::Unjudged(_)) => *b.unjudged.entry(base_name(name)).or_default() += 1,
            Some(Needs::Free) => {}
            Some(Needs::Support(_)) => {
                b.attached += 1;
                match judge_cell(*cell, name, &at_load) {
                    Some(Ok(())) => b.held += 1,
                    Some(Err(wants)) => b.dropped.push(Dropped {
                        cell: *cell,
                        block: name.to_string(),
                        wants,
                        piece: surround.inside(*cell).map(str::to_string),
                        after_step: None,
                    }),
                    None => {}
                }
            }
        }
    }
    b
}

fn base_name(name: &str) -> String {
    delvewright_dsl::blockshape::base_id(name).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::blockstate::BlockState;

    fn void() -> Surround {
        Surround {
            ambient: Ambient::Void,
            built: Vec::new(),
        }
    }

    fn map(cells: &[([i32; 3], &str)]) -> BlockMap {
        cells
            .iter()
            .map(|(c, b)| (*c, BlockState::new(b)))
            .collect()
    }

    /// The finding (demo The Still Ones): a wall torch hung on a glass pane is
    /// dropped, and the refusal names the block, its cell, the face it needs
    /// and what stands there. `wall_torch[facing=north]` hangs on the block
    /// south of it.
    #[test]
    fn a_wall_torch_on_a_glass_pane_is_dw1002_naming_the_cell() {
        let b = judge_load(
            &map(&[
                ([4, 65, 7], "minecraft:wall_torch[facing=north]"),
                ([4, 65, 8], "minecraft:glass_pane"),
            ]),
            &void(),
        );
        assert_eq!((b.attached, b.held, b.dropped.len()), (1, 0, 1));
        let why = b.refusal().expect("refused");
        assert!(
            why.contains("`minecraft:wall_torch[facing=north]` at [4, 65, 7]"),
            "{why}"
        );
        assert!(why.contains("the block south of it at [4, 65, 8]"), "{why}");
        assert!(why.contains("a full (sturdy) north face"), "{why}");
        assert!(why.contains("there is `minecraft:glass_pane`"), "{why}");
        assert!(b.line().contains("1 of 2 non-air cell(s)"), "{}", b.line());
    }

    /// The same torch on stone is held, and nothing is refused.
    #[test]
    fn a_wall_torch_on_stone_is_held() {
        let b = judge_load(
            &map(&[
                ([4, 65, 7], "minecraft:wall_torch[facing=north]"),
                ([4, 65, 8], "minecraft:stone"),
            ]),
            &void(),
        );
        assert_eq!((b.attached, b.held), (1, 1));
        assert!(b.refusal().is_none());
    }

    /// A hanging lantern under air is dropped; under planks it is held.
    #[test]
    fn a_hanging_lantern_under_air_is_dw1002() {
        let lantern = "minecraft:lantern[hanging=true]";
        let b = judge_load(&map(&[([0, 70, 0], lantern)]), &void());
        let why = b.refusal().expect("refused");
        assert!(why.contains("the block above it at [0, 71, 0]"), "{why}");
        assert!(why.contains("there is `minecraft:air`"), "{why}");
        let b = judge_load(
            &map(&[([0, 70, 0], lantern), ([0, 71, 0], "minecraft:oak_planks")]),
            &void(),
        );
        assert!(b.refusal().is_none());
    }

    /// A torch on a slab, as the jar says: kept on a top slab, dropped on a
    /// bottom one (its top face is half a block below the cell's).
    #[test]
    fn a_torch_on_a_slab_is_judged_as_the_jar_judges_it() {
        let on = |slab: &str| {
            judge_load(
                &map(&[([0, 65, 0], "minecraft:torch"), ([0, 64, 0], slab)]),
                &void(),
            )
            .dropped
            .len()
        };
        assert_eq!(on("minecraft:oak_slab[type=top]"), 0);
        assert_eq!(on("minecraft:oak_slab[type=double]"), 0);
        assert_eq!(on("minecraft:oak_slab[type=bottom]"), 1);
    }

    /// Under the ocean horizon the generator's water and stone stand outside
    /// every piece: a lily pad on the open sea is held, one inside a piece's
    /// box (where `/place template` wrote air) is not.
    #[test]
    fn the_ocean_surround_holds_what_the_generator_puts_there() {
        let ocean = Surround {
            ambient: Ambient::Ocean(crate::compiler::nav::Sea {
                level: 62,
                floor_top: 54,
            }),
            built: vec![("prefab/box".to_string(), ([10, 50, 10], [20, 70, 20]))],
        };
        let pad = "minecraft:lily_pad";
        assert!(
            judge_load(&map(&[([0, 63, 0], pad)]), &ocean)
                .refusal()
                .is_none()
        );
        assert!(
            judge_load(&map(&[([15, 63, 15], pad)]), &ocean)
                .refusal()
                .is_some()
        );
    }

    /// A block the rule cannot judge is counted by block, never refused.
    #[test]
    fn an_unjudged_block_is_counted_not_refused() {
        let b = judge_load(&map(&[([0, 64, 0], "minecraft:wheat")]), &void());
        assert!(b.refusal().is_none());
        assert_eq!(b.unjudged.get("minecraft:wheat"), Some(&1));
        assert!(b.line().contains("minecraft:wheat x1"), "{}", b.line());
    }
}
