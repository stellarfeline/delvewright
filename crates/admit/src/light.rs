//! A **static block-light probe** for a converted piece.
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
//! It measures the **minimum** block light over walkable floor cells (an air cell
//! with a solid block beneath and headroom above). Doorway openings are treated as
//! the structure edge (no external light enters), so the value is conservative —
//! a connected neighbour can only add light, exactly like the generator's
//! sealed-piece probe.

use std::collections::VecDeque;

use crate::structure::Structure;

/// Default block-light threshold below which a floor is `dark` (aligns with the
/// compiler's `DW0210` "floor light < 3" rule). Configurable — FLAGGED for owner
/// review (the lit/dark cutoff is policy, not mechanism).
pub const DEFAULT_DARK_THRESHOLD: i32 = 3;

/// The probe result.
#[derive(Debug, Clone)]
pub struct LightProbe {
    /// Minimum block light over walkable floor cells; `None` if the piece has no
    /// walkable floor (nothing to stand on / measure).
    pub measured_min_light: Option<i32>,
    /// How many walkable floor cells were measured.
    pub floor_cells: usize,
    /// `"lit"` / `"dark"` / `"unknown"` (unknown when there is no floor).
    pub profile: &'static str,
    /// The threshold used.
    pub dark_threshold: i32,
}

impl LightProbe {
    pub fn is_dark(&self) -> bool {
        self.profile == "dark"
    }
}

/// Run the static block-light BFS over `s`.
pub fn probe(s: &Structure, dark_threshold: i32) -> LightProbe {
    let [sx, sy, sz] = s.size;
    let idx = |x: i32, y: i32, z: i32| ((x * sy + y) * sz + z) as usize;
    let n = (sx * sy * sz).max(0) as usize;

    // name grid (air where a cell is absent — dense templates fill every cell).
    let mut names: Vec<&str> = vec!["minecraft:air"; n];
    for b in &s.blocks {
        let [x, y, z] = b.pos;
        if x >= 0 && x < sx && y >= 0 && y < sy && z >= 0 && z < sz {
            names[idx(x, y, z)] = s.palette[b.state as usize].name.as_str();
        }
    }

    // block-light BFS.
    let mut light = vec![0i32; n];
    let mut q: VecDeque<(i32, i32, i32)> = VecDeque::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let e = emitter_level(names[idx(x, y, z)]);
                if e > 0 {
                    let i = idx(x, y, z);
                    if e > light[i] {
                        light[i] = e;
                        q.push_back((x, y, z));
                    }
                }
            }
        }
    }
    while let Some((x, y, z)) = q.pop_front() {
        let l = light[idx(x, y, z)];
        let nl = l - 1;
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
            if !is_transparent(names[ni]) {
                continue;
            }
            if nl > light[ni] {
                light[ni] = nl;
                q.push_back((nx, ny, nz));
            }
        }
    }

    // walkable floor cells: standable air with a solid floor and headroom.
    let mut min_light: Option<i32> = None;
    let mut floor_cells = 0usize;
    for x in 0..sx {
        for y in 1..sy {
            for z in 0..sz {
                if !is_standable(names[idx(x, y, z)]) {
                    continue;
                }
                if !is_floor_solid(names[idx(x, y - 1, z)]) {
                    continue;
                }
                // headroom: cell above is standable air (skip when y is the top).
                if y + 1 < sy && !is_standable(names[idx(x, y + 1, z)]) {
                    continue;
                }
                floor_cells += 1;
                let l = light[idx(x, y, z)];
                min_light = Some(min_light.map_or(l, |m| m.min(l)));
            }
        }
    }

    let profile = match min_light {
        None => "unknown",
        Some(m) if m < dark_threshold => "dark",
        Some(_) => "lit",
    };
    LightProbe {
        measured_min_light: min_light,
        floor_cells,
        profile,
        dark_threshold,
    }
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
    if is_standable(name) {
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
            | "vine"
            | "glow_lichen"
            | "cobweb"
            | "torch"
            | "wall_torch"
            | "soul_torch"
            | "redstone_torch"
            | "lantern"
            | "soul_lantern"
            | "end_rod"
            | "lightning_rod"
            | "rail"
            | "water"
            | "flower_pot"
            | "bell"
            | "campfire"
            | "soul_campfire"
            | "structure_void"
            | "barrier"
            | "light"
            | "snow"
            | "amethyst_cluster"
    )
}

/// An empty, standable cell (a player can occupy it).
fn is_standable(name: &str) -> bool {
    matches!(strip(name), "air" | "cave_air" | "void_air")
}

/// A block a player can stand on top of (anything not empty / not a decoration a
/// player falls through). Conservative floor test: any non-air, non-passable block.
fn is_floor_solid(name: &str) -> bool {
    let s = strip(name);
    if is_standable(name) {
        return false;
    }
    !matches!(
        s,
        "torch"
            | "wall_torch"
            | "soul_torch"
            | "redstone_torch"
            | "water"
            | "lava"
            | "vine"
            | "glow_lichen"
            | "rail"
            | "light"
            | "structure_void"
            | "barrier"
    )
}

fn strip(name: &str) -> &str {
    name.split_once(':').map(|(_, p)| p).unwrap_or(name)
}
