//! A **static block-light probe** over the space a player can actually stand in.
//!
//! The generator records `measured_min_light` from a **live 1.21.11 server probe**
//! (the gold standard). For admission machinery that must run in CI without a
//! server, this module estimates the same quantity with a deterministic
//! block-light **BFS** — a faithful model of vanilla block light, which is a
//! 6-neighbour flood that decrements 1 per step through non-opaque cells.
//!
//! HONESTY: the emitted metadata records `method` as a *static estimate*, never a
//! live probe, so an admitted piece is never mistaken for a server-measured one.
//! A live re-probe (the generator's method) remains the gold standard for
//! borderline pieces — FLAGGED for owner review.
//!
//! # What the minimum is taken over, and why it is not the region box
//!
//! The probe measures the minimum block light over the **roofed floor cells a
//! body can walk to from outside**, and it states how many cells that was.
//!
//! Taking it over every walkable cell of the region box instead is a gate that
//! nobody can pass and therefore nobody reads. A free-standing building sits in
//! a box with ground around it: the apron outside the walls is walkable floor
//! under open sky, its block light is zero because nothing lights the outdoors
//! at night, and so **every free-standing building reports `dark` at any
//! lighting design whatsoever**. The same box also holds the sealed voids
//! between a vault and its roof, which no lantern reaches and no player ever
//! stands in. A measure that cannot come out any other way is not a measure.
//!
//! Three filters, each removing cells for a reason a player would recognise:
//!
//! * **standable** — two courses of clearance over something to stand on;
//! * **roofed** ([`nav::sheltered`]) — the one thing geometry can say about
//!   whether a floor is indoors; this is what removes the apron and the parapets;
//! * **reachable on foot** from a ground-level entrance ([`nav::ground_entry`]) —
//!   this is what removes the sealed voids.
//!
//! And the binding is reported, because a filter chain that removes everything
//! would otherwise read as "nothing was dark". A zero binding is a **finding**
//! (`DW0752`), never a pass: a sealed pitch-black crypt binds zero cells, and
//! the one thing this probe must never do is call that lit.
//!
//! The walk and the standability predicate are [`delvewright_schem::nav`]'s, not
//! this module's: they are the same question the grammar back end asks of an
//! expansion, and the seventh private copy of them was here.

use std::collections::{BTreeSet, VecDeque};

use delvewright_schem::nav::{self, Voxels};
use delvewright_schem::split::TilePart;

use crate::structure::Structure;

/// Default block-light threshold below which a floor is `dark` (aligns with the
/// compiler's `DW0210` "floor light < 3" rule). Configurable — FLAGGED for owner
/// review (the lit/dark cutoff is policy, not mechanism).
pub const DEFAULT_DARK_THRESHOLD: i32 = 3;

/// A whole zone's blocks as one grid of block names, however many files they
/// arrived in.
///
/// A tiled zone is one building; the tiling is packaging, and light crosses a
/// packaging plane exactly as it crosses any other cell. Probing tile by tile
/// would report darkness at every cut.
pub struct Zone<'a> {
    size: [i32; 3],
    names: Vec<&'a str>,
}

impl<'a> Zone<'a> {
    /// One structure template, probed on its own.
    pub fn single(s: &'a Structure) -> Zone<'a> {
        Zone::assemble(s.size, [([0, 0, 0], s)])
    }

    /// A zone reassembled from its tiles, each translated by the zone-local
    /// offset its manifest declares.
    pub fn from_tiles(size: [i32; 3], tiles: &'a [(TilePart, Structure)]) -> Zone<'a> {
        Zone::assemble(size, tiles.iter().map(|(p, s)| (p.offset, s)))
    }

    fn assemble(
        size: [i32; 3],
        tiles: impl IntoIterator<Item = ([i32; 3], &'a Structure)>,
    ) -> Zone<'a> {
        let [sx, sy, sz] = size;
        let n = (sx.max(0) as usize) * (sy.max(0) as usize) * (sz.max(0) as usize);
        // Absent cells are air: a dense template fills every cell, and a sparse
        // one means air wherever it says nothing.
        let mut names: Vec<&str> = vec!["minecraft:air"; n];
        for (offset, s) in tiles {
            for b in &s.blocks {
                let p = [
                    b.pos[0] + offset[0],
                    b.pos[1] + offset[1],
                    b.pos[2] + offset[2],
                ];
                if (0..3).all(|a| p[a] >= 0 && p[a] < size[a]) {
                    names[((p[0] * sy + p[1]) * sz + p[2]) as usize] =
                        s.palette[b.state as usize].name.as_str();
                }
            }
        }
        Zone { size, names }
    }

    /// The zone's extent.
    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    fn name(&self, pos: [i32; 3]) -> Option<&str> {
        if !(0..3).all(|a| pos[a] >= 0 && pos[a] < self.size[a]) {
            return None;
        }
        let [_, sy, sz] = self.size;
        Some(self.names[((pos[0] * sy + pos[1]) * sz + pos[2]) as usize])
    }
}

impl Voxels for Zone<'_> {
    fn origin(&self) -> [i32; 3] {
        [0, 0, 0]
    }

    fn size(&self) -> [i32; 3] {
        self.size
    }

    fn passable(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(is_passable)
    }

    fn floor(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(is_floor)
    }
}

/// The probe result — a measurement, and the binding it was taken over.
#[derive(Debug, Clone)]
pub struct LightProbe {
    /// Minimum block light over the measured cells; `None` when nothing bound.
    pub measured_min_light: Option<i32>,
    /// The darkest measured cell, in zone coordinates — where to put a light.
    pub darkest_cell: Option<[i32; 3]>,
    /// `"lit"` / `"dark"` / `"unbound"`.
    pub profile: &'static str,
    /// The threshold used.
    pub dark_threshold: i32,
    /// Standable cells anywhere in the region box (before any filter).
    pub standable_cells: usize,
    /// Ground-level entry cells found on the box's vertical faces.
    pub entry_cells: usize,
    /// Standable cells a body can walk to from an entry cell.
    pub reachable_cells: usize,
    /// **The binding**: reachable cells that are also roofed — the cells the
    /// minimum was actually taken over.
    pub measured_cells: usize,
}

impl LightProbe {
    pub fn is_dark(&self) -> bool {
        self.profile == "dark"
    }

    /// Did the probe bind to nothing? A zero binding is a finding, not a pass.
    pub fn is_unbound(&self) -> bool {
        self.profile == "unbound"
    }

    /// Why the probe bound to nothing, in the words an author can act on.
    pub fn unbound_reason(&self) -> String {
        if self.standable_cells == 0 {
            "no cell in the piece has two courses of clearance over a floor — there is nowhere \
             to stand in it at all"
                .to_string()
        } else if self.entry_cells == 0 {
            format!(
                "{} standable cell(s), but no ground-level entrance on any of the four vertical \
                 faces: nothing can be walked into. A piece whose way in is a jigsaw socket must \
                 be socketed before it is probed",
                self.standable_cells
            )
        } else if self.reachable_cells == 0 {
            format!(
                "{} standable cell(s) and {} entry cell(s), but nothing is reachable on foot from \
                 an entrance",
                self.standable_cells, self.entry_cells
            )
        } else {
            format!(
                "{} cell(s) are reachable on foot but none of them is roofed — every one is under \
                 open sky, so this piece has no interior to measure",
                self.reachable_cells
            )
        }
    }
}

/// Run the static block-light BFS over `zone` and take the minimum over player
/// space.
pub fn probe(zone: &Zone, dark_threshold: i32) -> LightProbe {
    let light = block_light(zone);

    let standable = nav::standable_cells(zone);
    let entry = nav::ground_entry(zone);
    let reachable = nav::reachable_from(&standable, &entry);
    let measured: BTreeSet<[i32; 3]> = reachable
        .iter()
        .copied()
        .filter(|&c| nav::sheltered(zone, c))
        .collect();

    let [_, sy, sz] = zone.size;
    let mut min_light: Option<i32> = None;
    let mut darkest: Option<[i32; 3]> = None;
    for &c in &measured {
        let l = light[((c[0] * sy + c[1]) * sz + c[2]) as usize];
        if min_light.is_none_or(|m| l < m) {
            min_light = Some(l);
            darkest = Some(c);
        }
    }

    let profile = match min_light {
        None => "unbound",
        Some(m) if m < dark_threshold => "dark",
        Some(_) => "lit",
    };
    LightProbe {
        measured_min_light: min_light,
        darkest_cell: darkest,
        profile,
        dark_threshold,
        standable_cells: standable.len(),
        entry_cells: entry.len(),
        reachable_cells: reachable.len(),
        measured_cells: measured.len(),
    }
}

/// Vanilla's block-light flood: every emitter seeds its level, and light spreads
/// to the 6 neighbours it can pass into, losing one level per step.
fn block_light(zone: &Zone) -> Vec<i32> {
    let [sx, sy, sz] = zone.size;
    let idx = |x: i32, y: i32, z: i32| ((x * sy + y) * sz + z) as usize;
    let mut light = vec![0i32; zone.names.len()];
    let mut q: VecDeque<(i32, i32, i32)> = VecDeque::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let e = emitter_level(zone.names[idx(x, y, z)]);
                if e > 0 {
                    light[idx(x, y, z)] = e;
                    q.push_back((x, y, z));
                }
            }
        }
    }
    while let Some((x, y, z)) = q.pop_front() {
        let nl = light[idx(x, y, z)] - 1;
        if nl <= 0 {
            continue;
        }
        for (dx, dy, dz) in [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if nx < 0 || nx >= sx || ny < 0 || ny >= sy || nz < 0 || nz >= sz {
                continue;
            }
            let ni = idx(nx, ny, nz);
            // light only enters a cell it can pass into (transparent to light).
            if !is_transparent(zone.names[ni]) {
                continue;
            }
            if nl > light[ni] {
                light[ni] = nl;
                q.push_back((nx, ny, nz));
            }
        }
    }
    light
}

/// Block-light emission level of a block, or 0. Covers the common decorative light
/// sources; unknown blocks emit nothing (conservative).
fn emitter_level(name: &str) -> i32 {
    match strip(name) {
        "glowstone"
        | "sea_lantern"
        | "shroomlight"
        | "lantern"
        | "jack_o_lantern"
        | "froglight"
        | "ochre_froglight"
        | "verdant_froglight"
        | "pearlescent_froglight"
        | "lava"
        | "fire"
        | "campfire"
        | "beacon"
        | "conduit"
        | "redstone_lamp_lit" => 15,
        "end_rod" | "torch" | "wall_torch" => 14,
        "soul_lantern" | "soul_torch" | "soul_campfire" | "soul_fire" | "crying_obsidian" => 10,
        "redstone_torch" | "glow_lichen" | "amethyst_cluster" => 7,
        "brewing_stand" | "brown_mushroom" => 1,
        "magma_block" => 3,
        _ => 0,
    }
}

/// A block light can propagate **into** (non-opaque). Conservative: opaque unless
/// explicitly known transparent — under-reporting light is the safe direction for
/// a minimum-light probe.
fn is_transparent(name: &str) -> bool {
    let s = strip(name);
    if is_passable(name) {
        return true;
    }
    if s.ends_with("_glass")
        || s.ends_with("_glass_pane")
        || s.ends_with("_bars")
        || s.ends_with("_fence")
        || s.ends_with("_fence_gate")
        || s.ends_with("_door")
        || s.ends_with("_trapdoor")
        || s.ends_with("_wall")
        || s.ends_with("_carpet")
        || s.ends_with("_torch")
        || s.ends_with("_candle")
        || s.ends_with("_sign")
        || s.ends_with("_slab")
        || s.ends_with("_stairs")
    {
        return true;
    }
    matches!(
        s,
        "glass"
            | "tinted_glass"
            | "iron_bars"
            | "chain"
            | "iron_chain"
            | "ladder"
            | "scaffolding"
            | "cobweb"
            | "lantern"
            | "soul_lantern"
            | "end_rod"
            | "lightning_rod"
            | "flower_pot"
            | "bell"
            | "campfire"
            | "soul_campfire"
            | "barrier"
            | "snow"
            | "amethyst_cluster"
    )
}

/// An empty, standable cell (a player can occupy it).
fn is_standable(name: &str) -> bool {
    matches!(strip(name), "air" | "cave_air" | "void_air")
}

/// A cell a body's own volume passes through: empty air, and the decorations
/// vanilla gives no collision box.
fn is_passable(name: &str) -> bool {
    if is_standable(name) {
        return true;
    }
    matches!(
        strip(name),
        "torch"
            | "wall_torch"
            | "soul_torch"
            | "redstone_torch"
            | "water"
            | "vine"
            | "glow_lichen"
            | "rail"
            | "light"
            | "structure_void"
    )
}

/// A block a player can stand on top of: anything with a collision box that is
/// not a fluid a body sinks through. Passable decorations are excluded by
/// construction, so a cell above a wall torch is not floor.
fn is_floor(name: &str) -> bool {
    !is_passable(name) && !matches!(strip(name), "lava" | "barrier")
}

fn strip(name: &str) -> &str {
    name.split_once(':').map(|(_, p)| p).unwrap_or(name)
}
