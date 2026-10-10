//! **What a block needs beside it to stay** — the pinned game's
//! `BlockState.canSurvive`, as a table.
//!
//! A wall torch on a glass pane, a lantern hung under air, a flower on stone, a
//! rail on a fence: each is a block a structure or a `setblock` writes and the
//! server drops the first time a shape update reaches it, because its
//! `canSurvive` rule asks a neighbour for something the neighbour does not give.
//! This module answers that rule for every blockstate of the pin, read from two
//! tables measured inside the pinned server jar by
//! `tools/maintenance/dump-support.py` (provenance in
//! `crates/delvec/data/PROVENANCE.md`):
//!
//! * `crates/dsl/data/support-1.21.11.tsv` — per state, `free` (it survives in
//!   air), `support` (the neighbours any one of which keeps it, each a base set
//!   the game names plus its exact differences), or `unjudged` (the
//!   single-neighbour reading cannot be its whole rule, with the reason);
//! * `crates/dsl/data/support-bases-1.21.11.tsv` — per state, the base sets'
//!   own questions: CENTER- and RIGID-sturdy faces, air, solid, still water.
//!   The FULL-sturdy faces are [`crate::blockshape::face_is_sturdy`]'s table.
//!
//! Nothing here is classified by hand: a block is "attached" because the jar
//! says it does not survive in air, and what holds it is the set of neighbours
//! the jar said yes to. The compiler's `DW1002` judges every block of the world
//! a build writes with [`Needs::kept`].

use crate::blockshape::{
    Face, PinnedRows, face_is_sturdy, namespaced_id, pinned_row, pinned_rows, state_has,
};

/// The support table: `crates/dsl/data/support-1.21.11.tsv`.
const SUPPORT_TSV: &str = include_str!("../data/support-1.21.11.tsv");
/// The base table: `crates/dsl/data/support-bases-1.21.11.tsv`.
const BASES_TSV: &str = include_str!("../data/support-bases-1.21.11.tsv");

/// The faces in the order the tables spell them (`d u n s w e`).
const FACES: [Face; 6] = [
    Face::Down,
    Face::Up,
    Face::North,
    Face::South,
    Face::West,
    Face::East,
];

/// A base set of neighbours, as the game names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    /// No neighbour: only the rule's added states hold.
    None,
    /// The neighbour's face toward the block is FULL-sturdy
    /// (`isFaceSturdy(…, SupportType.FULL)`): its support shape covers the
    /// whole square.
    Full,
    /// CENTER-sturdy: the support shape covers the face's centre.
    Center,
    /// RIGID-sturdy: the support shape covers the face but for a rim.
    Rigid,
    /// Any block that is not air (`!isAir()`).
    NonAir,
    /// A solid block (`isSolid()`).
    Solid,
    /// A block whose fluid is still water.
    Water,
}

impl Base {
    fn parse(s: &str) -> Base {
        match s {
            "none" => Base::None,
            "full" => Base::Full,
            "center" => Base::Center,
            "rigid" => Base::Rigid,
            "nonair" => Base::NonAir,
            "solid" => Base::Solid,
            "water" => Base::Water,
            other => panic!("support table: unknown base {other:?}"),
        }
    }

    /// What the base asks of a neighbour, for a message: `face` is the
    /// neighbour's face toward the block.
    pub fn describe(self, face: Face) -> String {
        match self {
            Base::None => "nothing in general".to_string(),
            Base::Full => format!("a full (sturdy) {} face", face.name()),
            Base::Center => format!("a {} face sturdy at its centre", face.name()),
            Base::Rigid => format!("a rigid {} face", face.name()),
            Base::NonAir => "any block but air".to_string(),
            Base::Solid => "a solid block".to_string(),
            Base::Water => "still water".to_string(),
        }
    }

    /// Whether `neighbour`, across its `face` toward the block, is in this base.
    fn holds(self, neighbour: &str, face: Face) -> bool {
        let b = bases();
        match self {
            Base::None => false,
            Base::Full => face_is_sturdy(neighbour, face),
            Base::Center => b.face(&b.center, neighbour, face, "center"),
            Base::Rigid => b.face(&b.rigid, neighbour, face, "rigid"),
            Base::NonAir => !b.flag(neighbour, 'a'),
            Base::Solid => b.flag(neighbour, 's'),
            Base::Water => b.flag(neighbour, 'w'),
        }
    }
}

/// One neighbour that keeps a block: the neighbour across `cell` is in `base`,
/// or is one of `added`, and is not one of `removed`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The direction from the block to the neighbour this rule reads.
    pub cell: Face,
    /// The base set.
    pub base: Base,
    /// States the jar keeps the block beside beyond the base, as table rows
    /// (`minecraft:block` or `minecraft:block[k=v,…]`).
    pub added: Vec<String>,
    /// States of the base the jar does not keep it beside.
    pub removed: Vec<String>,
}

impl Rule {
    /// Whether `neighbour`, the block across [`Rule::cell`], keeps the block.
    pub fn holds(&self, neighbour: &str) -> bool {
        let face = self.cell.opposite();
        if self.removed.iter().any(|p| pattern_has(p, neighbour)) {
            return false;
        }
        self.base.holds(neighbour, face) || self.added.iter().any(|p| pattern_has(p, neighbour))
    }

    /// What the rule asks of its neighbour, for a message: the base, and the
    /// states it adds (listed when there are few, counted when many).
    pub fn describe(&self) -> String {
        let face = self.cell.opposite();
        let added = match self.added.len() {
            0 => String::new(),
            n if n <= 6 => format!(" or one of {}", self.added.join(", ")),
            n => format!(
                " or one of {n} listed states ({}, …)",
                self.added[..3].join(", ")
            ),
        };
        match (self.base, added.is_empty()) {
            (Base::None, false) => added.trim_start_matches(" or ").to_string(),
            _ => format!("{}{added}", self.base.describe(face)),
        }
    }
}

/// What one block state needs beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Needs {
    /// It survives in air: nothing.
    Free,
    /// Any one of these neighbours keeps it.
    Support(Vec<Rule>),
    /// The single-neighbour reading cannot be its whole rule; the reason, as
    /// the table states it. Nothing is judged.
    Unjudged(String),
}

impl Needs {
    /// Whether the block survives with `neighbour(cell)` beside it, by the
    /// measured rule: `Some(true)` for a free block or one a rule holds,
    /// `Some(false)` when no rule holds, `None` when the rule is unjudged.
    pub fn kept<'a>(&self, neighbour: impl Fn(Face) -> &'a str) -> Option<bool> {
        match self {
            Needs::Free => Some(true),
            Needs::Support(rules) => Some(rules.iter().any(|r| r.holds(neighbour(r.cell)))),
            Needs::Unjudged(_) => None,
        }
    }
}

/// **What a block state needs beside it to survive**, by the pinned jar, or
/// `None` for a block the table does not hold (a non-`minecraft:` id, or one
/// the pin lacks).
pub fn needs(name: &str) -> Option<&'static Needs> {
    pinned_row(support_rows(), name)
}

/// Whether a block state is one a table row names: the row's block, carrying
/// every property the row lists ([`crate::blockshape`]'s one comparison).
fn pattern_has(pattern: &str, name: &str) -> bool {
    let (id, props) = split_pattern(pattern);
    namespaced_id(name) == id && state_has(name, &props)
}

fn split_pattern(pattern: &str) -> (&str, Vec<(String, String)>) {
    match pattern.find('[') {
        Some(open) => (
            &pattern[..open],
            pattern[open + 1..pattern.len() - 1]
                .split(',')
                .filter_map(|kv| kv.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        ),
        None => (pattern, Vec::new()),
    }
}

/// `<cell>:<base>[+<rows>][-<rows>]`, rows `|`-separated, each beginning
/// `minecraft:` (which is how the added and removed lists are told apart:
/// no vanilla id or property value holds `-minecraft:`).
fn parse_rule(text: &str) -> Rule {
    let (cell, rest) = text
        .split_once(':')
        .unwrap_or_else(|| panic!("support rule has no cell: {text:?}"));
    let cell = FACES
        .into_iter()
        .find(|f| f.name().starts_with(cell) && cell.len() == 1)
        .unwrap_or_else(|| panic!("support rule names no face: {text:?}"));
    let base_end = rest.find(['+', '-']).unwrap_or(rest.len());
    let base = Base::parse(&rest[..base_end]);
    let tail = &rest[base_end..];
    let (plus, minus) = match tail.find("-minecraft:") {
        Some(i) => (&tail[..i], &tail[i + 1..]),
        None => (tail, ""),
    };
    let list = |s: &str| -> Vec<String> {
        s.trim_start_matches('+')
            .split('|')
            .filter(|r| !r.is_empty())
            .map(str::to_string)
            .collect()
    };
    Rule {
        cell,
        base,
        added: list(plus),
        removed: list(minus),
    }
}

fn support_rows() -> &'static PinnedRows<Needs> {
    static ROWS: std::sync::OnceLock<PinnedRows<Needs>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        pinned_rows(SUPPORT_TSV, "support", |kind, rule| match kind {
            "free" => Needs::Free,
            "support" => Needs::Support(rule.split(' ').map(parse_rule).collect()),
            "unjudged" => Needs::Unjudged(rule.to_string()),
            other => panic!("support table: unknown kind {other:?}"),
        })
    })
}

/// The base table, one [`PinnedRows`] per column, and the faces each face
/// column carries.
struct Bases {
    center: PinnedRows<u8>,
    rigid: PinnedRows<u8>,
    flags: PinnedRows<String>,
    center_faces: u8,
    rigid_faces: u8,
}

impl Bases {
    fn face(&self, rows: &PinnedRows<u8>, name: &str, face: Face, what: &str) -> bool {
        let carried = if what == "center" {
            self.center_faces
        } else {
            self.rigid_faces
        };
        assert!(
            carried & bit(face) != 0,
            "support-bases table carries no {what} {} face — a rule asked one the \
             extractor did not see asked",
            face.name()
        );
        pinned_row(rows, name).is_some_and(|m| m & bit(face) != 0)
    }

    fn flag(&self, name: &str, flag: char) -> bool {
        pinned_row(&self.flags, name).is_some_and(|f| f.contains(flag))
    }
}

fn bit(face: Face) -> u8 {
    1 << FACES.iter().position(|f| *f == face).expect("a face")
}

fn mask(letters: &str) -> u8 {
    FACES
        .iter()
        .filter(|f| letters.contains(f.name().chars().next().expect("named")))
        .fold(0, |m, f| m | bit(*f))
}

fn bases() -> &'static Bases {
    static B: std::sync::OnceLock<Bases> = std::sync::OnceLock::new();
    B.get_or_init(|| {
        let all = pinned_rows(BASES_TSV, "support-bases", |col, v| {
            (col.to_string(), v.to_string())
        });
        let column = |which: &str| -> PinnedRows<String> {
            all.iter()
                .map(|(id, rows)| {
                    (
                        id.clone(),
                        rows.iter()
                            .filter(|(_, (c, _))| c == which)
                            .map(|(p, (_, v))| (p.clone(), v.clone()))
                            .collect(),
                    )
                })
                .collect()
        };
        let faces = |which: &str| -> PinnedRows<u8> {
            column(which)
                .into_iter()
                .map(|(id, rows)| {
                    let rows = rows
                        .into_iter()
                        .map(|(p, v)| (p, if v == "-" { 0 } else { mask(&v) }))
                        .collect();
                    (id, rows)
                })
                .collect()
        };
        let carried = |what: &str| -> u8 {
            let key = format!("#   {what} faces carried: ");
            BASES_TSV
                .lines()
                .find_map(|l| l.strip_prefix(key.as_str()))
                .map(mask)
                .unwrap_or_else(|| panic!("support-bases table states no carried {what} faces"))
        };
        Bases {
            center: faces("center"),
            rigid: faces("rigid"),
            flags: column("flags"),
            center_faces: carried("center"),
            rigid_faces: carried("rigid"),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The neighbourhood of a block: `beside` at the one cell named, air at
    /// every other.
    fn alone<'a>(cell: Face, beside: &'a str) -> impl Fn(Face) -> &'a str {
        move |f| if f == cell { beside } else { "minecraft:air" }
    }

    fn kept(block: &str, cell: Face, beside: &str) -> Option<bool> {
        needs(block)
            .unwrap_or_else(|| panic!("{block} is in the table"))
            .kept(alone(cell, beside))
    }

    /// The finding: a wall torch hung on a glass pane is dropped (a pane's side
    /// is not a sturdy face), and kept on stone. `wall_torch[facing=north]`
    /// hangs on the block SOUTH of it.
    #[test]
    fn a_wall_torch_needs_a_sturdy_face_behind_it() {
        let t = "minecraft:wall_torch[facing=north]";
        assert_eq!(kept(t, Face::South, "minecraft:glass_pane"), Some(false));
        assert_eq!(kept(t, Face::South, "minecraft:stone"), Some(true));
        // Only the block behind it holds it.
        assert_eq!(kept(t, Face::North, "minecraft:stone"), Some(false));
        assert_eq!(kept(t, Face::Down, "minecraft:stone"), Some(false));
    }

    /// A hanging lantern hangs on the block above it, and air holds nothing.
    #[test]
    fn a_hanging_lantern_needs_the_block_above() {
        let l = "minecraft:lantern[hanging=true]";
        assert_eq!(kept(l, Face::Up, "minecraft:air"), Some(false));
        assert_eq!(kept(l, Face::Up, "minecraft:oak_planks"), Some(true));
        // A chain's bottom is sturdy at its centre: a lantern hangs from one.
        assert_eq!(kept(l, Face::Up, "minecraft:iron_chain"), Some(true));
        // Standing, it needs the block under it instead.
        let s = "minecraft:lantern[hanging=false]";
        assert_eq!(kept(s, Face::Up, "minecraft:oak_planks"), Some(false));
        assert_eq!(kept(s, Face::Down, "minecraft:oak_planks"), Some(true));
    }

    /// A torch stands on a face sturdy at its centre (`canSupportCenter`): the
    /// top of a top slab is one, the top of a bottom slab is not (it is half a
    /// block below the cell's floor), and a glass pane's post is.
    #[test]
    fn a_torch_on_a_slab_is_kept_as_the_jar_keeps_it() {
        let t = "minecraft:torch";
        assert_eq!(
            kept(t, Face::Down, "minecraft:stone_slab[type=top]"),
            Some(true)
        );
        assert_eq!(
            kept(t, Face::Down, "minecraft:stone_slab[type=double]"),
            Some(true)
        );
        assert_eq!(
            kept(t, Face::Down, "minecraft:stone_slab[type=bottom]"),
            Some(false)
        );
        // A bare slab is its default: the bottom half.
        assert_eq!(kept(t, Face::Down, "minecraft:stone_slab"), Some(false));
        assert_eq!(kept(t, Face::Down, "minecraft:glass_pane"), Some(true));
    }

    /// A flower roots in the blocks the jar's `#dirt`-and-friends rule names,
    /// and nowhere else; a carpet sits on anything but air.
    #[test]
    fn soil_and_carpet() {
        assert_eq!(
            kept("minecraft:poppy", Face::Down, "minecraft:grass_block"),
            Some(true)
        );
        assert_eq!(
            kept("minecraft:poppy", Face::Down, "minecraft:stone"),
            Some(false)
        );
        assert_eq!(
            kept("minecraft:white_carpet", Face::Down, "minecraft:glass_pane"),
            Some(true)
        );
        assert_eq!(
            kept("minecraft:white_carpet", Face::Down, "minecraft:air"),
            Some(false)
        );
    }

    /// The jar's rule, not a guess: a flower pot and a campfire need nothing.
    #[test]
    fn a_flower_pot_and_a_campfire_stand_on_nothing() {
        assert_eq!(needs("minecraft:flower_pot"), Some(&Needs::Free));
        assert_eq!(needs("minecraft:campfire"), Some(&Needs::Free));
        assert_eq!(needs("minecraft:stone"), Some(&Needs::Free));
        assert!(needs("example:not_a_block").is_none());
    }

    /// A door's upper half stands on its own lower half.
    #[test]
    fn a_door_half_stands_on_its_other_half() {
        let up = "minecraft:oak_door[half=upper]";
        assert_eq!(
            kept(up, Face::Down, "minecraft:oak_door[half=lower]"),
            Some(true)
        );
        assert_eq!(kept(up, Face::Down, "minecraft:stone"), Some(false));
    }

    /// Every rule parses, every face a centre or rigid rule asks is carried, and
    /// every listed state is one the registry holds.
    #[test]
    fn every_rule_parses_and_asks_a_carried_face() {
        let mut rules = 0usize;
        for rows in support_rows().values() {
            for (_, n) in rows {
                if let Needs::Support(rs) = n {
                    for r in rs {
                        rules += 1;
                        let face = r.cell.opposite();
                        // Asking the base of air panics if the face is not carried.
                        r.base.holds("minecraft:air", face);
                        for p in r.added.iter().chain(&r.removed) {
                            assert!(
                                crate::blocks::BlockRegistry::v1_21_11()
                                    .validate_state_string(p)
                                    .is_ok(),
                                "{p} is not a state the registry holds"
                            );
                        }
                    }
                }
            }
        }
        assert!(rules > 600, "only {rules} support rules parsed");
    }
}
