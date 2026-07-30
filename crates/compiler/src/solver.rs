//! The jigsaw layout solver (ADR-0004 amendment, M2 task #9).
//!
//! **The compiler is the jigsaw.** Rather than issue `/place jigsaw` at runtime
//! and try to predict Mojang's placement, the compiler solves the piece layout
//! itself from the campaign seed and emits per-piece `/place template <piece>
//! <pos> <rotation>` calls (ADR-0004 §"Verification" fallback, promoted to
//! primary). This keeps the shipped delve plain vanilla (ADR-0003), makes
//! determinism trivial (all randomness from the one campaign seed through this
//! module's PRNG, ADR-0006), and gives the compiler full layout knowledge for
//! anchors, the critical path, and global constraints.
//!
//! ## Geometry (matches vanilla `/place template <pos> <rotation>`)
//!
//! `/place template` places a structure's local `(0,0,0)` at `pos`, then rotates
//! about that point (rotation pivot = the structure origin, no mirror). A local
//! block `l` lands at `pos + transform(l, rotation)` where, with pivot at the
//! origin (mirroring Minecraft's `StructureTemplate::transform`):
//!
//! | rotation             | `transform(x, y, z)` |
//! | -------------------- | -------------------- |
//! | `none`               | `( x, y,  z)`        |
//! | `clockwise_90`       | `(-z, y,  x)`        |
//! | `180`                | `(-x, y, -z)`        |
//! | `counterclockwise_90`| `( z, y, -x)`        |
//!
//! ## Socket mating (keep-socket-v1)
//!
//! Each doorway is a socket at its wall cell facing outward. Two sockets connect
//! when the child socket sits **one block beyond** the parent socket, facing the
//! **opposite** direction (`final_state = air` leaves a clean 3×3 passage). Given
//! a parent socket at world `ws` facing `ds`, a child piece with a chosen
//! connector `(Lc, Fc)` is placed with the unique rotation `r` s.t.
//! `rotate_facing(Fc, r) == opposite(ds)`, at `pos = ws + unit(ds) −
//! transform(Lc, r)`. Its other connectors become the new open frontier.
//!
//! ## Growth strategy
//!
//! Two modes, chosen by how many dead-end terminals the campaign requires:
//!
//! - **Single terminal (`grow_spine`)** — a straight-line spine: grow from the
//!   `entry` along its exit with straight-preferring `connector` fillers, thread
//!   the through-rooms inline, and end at the one dead-end terminal (`boss-hall`
//!   last, farthest from the entry). This is the pre-pathfinder behaviour, kept
//!   **byte-identical** so single-terminal pools (`keep-crawl`) are unchanged.
//! - **Two or more terminals (`grow_branching`)** — a branching tree: extend the
//!   trunk, then fork with `tee`/`cross` branch pieces (`m2-gameplay-verbs`,
//!   lifting the old `DW0304` "one terminal max" limit — the harness now
//!   pathfinds, so branches/turns are walkable), and cap each terminal on a
//!   distinct branch socket (e.g. shrine **and** boss-hall).
//!
//! The mating/AABB machinery below is fully general (all four cardinal rotations,
//! overlap rejection — unit-tested); both modes preserve the guarantees
//! (connected; exactly one entry; each required anchor-bearing piece placed once;
//! piece count in `[min,max]`). Every unmated socket is sealed with wall material;
//! every mated socket's jigsaw block is cleared to air.

use crate::registry::{Connector, PoolMember, PrefabRegistry};

// ---------------------------------------------------------------------------
// Deterministic PRNG (splitmix64) — ADR-0006 named streams
// ---------------------------------------------------------------------------

/// A tiny deterministic PRNG (splitmix64). Hand-rolled to stay within the
/// dependency budget (spec-0002); it is only used for layout choices, all seeded
/// from the one campaign seed (ADR-0006). Not cryptographic.
#[derive(Debug, Clone)]
pub struct Splitmix64 {
    state: u64,
}

impl Splitmix64 {
    /// Seed a stream. Callers derive a distinct seed per named stream (see
    /// [`stream_seed`]) so per-area layouts are independent yet reproducible.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next 64-bit value (advances the state).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A weighted choice among `(index, weight)` pairs; returns the chosen index.
    /// Deterministic for a given state and weight list. Empty / all-zero weights
    /// return `None`.
    pub fn weighted(&mut self, weights: &[u32]) -> Option<usize> {
        let total: u64 = weights.iter().map(|&w| w as u64).sum();
        if total == 0 {
            return None;
        }
        let mut pick = self.next_u64() % total;
        for (i, &w) in weights.iter().enumerate() {
            let w = w as u64;
            if pick < w {
                return Some(i);
            }
            pick -= w;
        }
        None
    }
}

/// Derive a named PRNG stream seed from the campaign seed and a stream name
/// (ADR-0006: "all randomness derives from stage-1 `seed` via a named PRNG").
/// The name is folded in with an FNV-1a hash, then run through one splitmix step
/// so adjacent names give well-separated streams.
pub fn stream_seed(campaign_seed: u64, name: &str) -> u64 {
    let mut h: u64 = 0xCBF2_9CE4_8422_2325; // FNV-1a offset basis
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    Splitmix64::new(campaign_seed ^ h).next_u64()
}

// ---------------------------------------------------------------------------
// Rotation geometry
// ---------------------------------------------------------------------------

/// A cardinal yaw rotation, matching vanilla's `template_rotation` argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// No rotation.
    None,
    /// 90° clockwise (viewed from above).
    Cw90,
    /// 180°.
    Cw180,
    /// 90° counter-clockwise.
    Ccw90,
}

impl Rotation {
    /// All four rotations, in a fixed order.
    pub const ALL: [Rotation; 4] = [
        Rotation::None,
        Rotation::Cw90,
        Rotation::Cw180,
        Rotation::Ccw90,
    ];

    /// Transform a local offset about the origin pivot (see module docs).
    pub fn transform(self, l: [i32; 3]) -> [i32; 3] {
        let [x, y, z] = l;
        match self {
            Rotation::None => [x, y, z],
            Rotation::Cw90 => [-z, y, x],
            Rotation::Cw180 => [-x, y, -z],
            Rotation::Ccw90 => [z, y, -x],
        }
    }

    /// The vanilla `template_rotation` token, or `None` for the identity (which
    /// is emitted by omitting the argument, keeping single-piece output — and the
    /// hello-world regression — byte-identical).
    pub fn token(self) -> Option<&'static str> {
        match self {
            Rotation::None => None,
            Rotation::Cw90 => Some("clockwise_90"),
            Rotation::Cw180 => Some("180"),
            Rotation::Ccw90 => Some("counterclockwise_90"),
        }
    }

    /// The world AABB `(min, max)` (inclusive block cells) of a `size` structure
    /// placed at `pos` with this rotation.
    pub fn bbox(self, pos: [i32; 3], size: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let (sx, sy, sz) = (size[0], size[1], size[2]);
        // Transform the four horizontal extreme corners; y is unrotated.
        let corners = [
            [0, 0, 0],
            [sx - 1, 0, 0],
            [0, 0, sz - 1],
            [sx - 1, 0, sz - 1],
        ];
        let mut min = [i32::MAX, pos[1], i32::MAX];
        let mut max = [i32::MIN, pos[1] + sy - 1, i32::MIN];
        for c in corners {
            let t = self.transform(c);
            for axis in [0usize, 2] {
                let w = pos[axis] + t[axis];
                min[axis] = min[axis].min(w);
                max[axis] = max[axis].max(w);
            }
        }
        (min, max)
    }
}

/// A cardinal facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    /// −z.
    North,
    /// +z.
    South,
    /// +x.
    East,
    /// −x.
    West,
}

impl Facing {
    /// Parse a facing keyword (`north`/`south`/`east`/`west`).
    pub fn parse(s: &str) -> Option<Facing> {
        match s {
            "north" => Some(Facing::North),
            "south" => Some(Facing::South),
            "east" => Some(Facing::East),
            "west" => Some(Facing::West),
            _ => None,
        }
    }

    /// The unit direction vector (MC axes: north = −z, east = +x).
    pub fn unit(self) -> [i32; 3] {
        match self {
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::East => [1, 0, 0],
            Facing::West => [-1, 0, 0],
        }
    }

    /// The opposite facing.
    pub fn opposite(self) -> Facing {
        match self {
            Facing::North => Facing::South,
            Facing::South => Facing::North,
            Facing::East => Facing::West,
            Facing::West => Facing::East,
        }
    }

    /// This facing rotated by `r` (yaw).
    pub fn rotate(self, r: Rotation) -> Facing {
        // Cw90: N→E→S→W→N.
        let cw = |f: Facing| match f {
            Facing::North => Facing::East,
            Facing::East => Facing::South,
            Facing::South => Facing::West,
            Facing::West => Facing::North,
        };
        match r {
            Rotation::None => self,
            Rotation::Cw90 => cw(self),
            Rotation::Cw180 => cw(cw(self)),
            Rotation::Ccw90 => cw(cw(cw(self))),
        }
    }
}

/// AABB overlap test (inclusive block cells). Flush (face-touching) pieces do
/// **not** overlap.
fn aabb_overlap(a: (&[i32; 3], &[i32; 3]), b: (&[i32; 3], &[i32; 3])) -> bool {
    (0..3).all(|i| a.0[i] <= b.1[i] && b.0[i] <= a.1[i])
}

// ---------------------------------------------------------------------------
// Placed pieces & the solved layout
// ---------------------------------------------------------------------------

/// A placed piece in the solved layout (world coordinates).
#[derive(Debug, Clone)]
pub struct PlacedPiece {
    /// Prefab id (`prefab/<name>`).
    pub prefab_id: String,
    /// The `/place template` position (where local `(0,0,0)` lands).
    pub pos: [i32; 3],
    /// Placement rotation.
    pub rotation: Rotation,
    /// Inclusive world AABB min corner.
    pub bbox_min: [i32; 3],
    /// Inclusive world AABB max corner.
    pub bbox_max: [i32; 3],
    /// Per-connector mated flag (index-aligned with the prefab's `connectors`).
    pub mated: Vec<bool>,
}

/// A wall-fill or air-clear at a socket (emitted into the setup bootstrap).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealFill {
    /// Inclusive min corner (world).
    pub from: [i32; 3],
    /// Inclusive max corner (world).
    pub to: [i32; 3],
    /// The block to fill with (`minecraft:stone_bricks` to seal, `minecraft:air`
    /// to clear a mated jigsaw block).
    pub block: String,
}

/// The solved layout of one pool area.
#[derive(Debug, Clone)]
pub struct AreaLayout {
    /// The placed pieces, in placement order (entry first).
    pub pieces: Vec<PlacedPiece>,
    /// Socket seal/clear fills for every connector of every piece.
    pub seals: Vec<SealFill>,
}

/// A solver failure (maps to a `DW03xx` build diagnostic, exit 3).
#[derive(Debug)]
pub struct SolveError {
    /// The stable diagnostic code (`DW03xx`).
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
}

impl SolveError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        SolveError {
            code,
            message: message.into(),
        }
    }
}

/// `DW0301`: a pool declares no `entry`-role piece (nothing to seed the layout).
pub const DW_NO_ENTRY: &str = "DW0301";
/// `DW0302`: a campaign-referenced anchor is provided by no member of the pool
/// (unsatisfiable required anchor).
pub const DW_UNSATISFIABLE_ANCHOR: &str = "DW0302";
/// `DW0303`: the `pieces {min,max}` range is too small to fit the entry plus the
/// required anchor-bearing pieces.
pub const DW_RANGE_TOO_SMALL: &str = "DW0303";
/// `DW0304`: the solver could not place a required piece without an overlap, or a
/// branching layout has no branch piece (tee/cross) to fork its terminals
/// (layout infeasible for this pool / seed).
pub const DW_INFEASIBLE: &str = "DW0304";

/// An open socket on the growth frontier: which placed piece, which of its
/// connectors, and the socket's resolved world pose.
#[derive(Debug, Clone)]
struct OpenSocket {
    piece: usize,
    connector: usize,
    world_pos: [i32; 3],
    facing: Facing,
}

/// A candidate piece to place: its prefab, size, and connectors.
struct Candidate<'a> {
    prefab_id: &'a str,
    size: [i32; 3],
    connectors: &'a [Connector],
}

/// Solve the layout of one pool area.
///
/// - `pool_id` names the pool (member pieces come from `registry.pool`).
/// - `required_anchors` are the anchor names the campaign references in this area
///   (NPC stands, `reach-anchor` targets, `open-gate` anchors). The solver places
///   exactly one piece per required anchor.
/// - `pieces_min`/`pieces_max` bound the total piece count (DSL `pieces`).
/// - `origin` is the area origin (`/place` pos of the entry, rotation `none`).
/// - `stream` is this area's named PRNG stream.
#[allow(clippy::too_many_arguments)]
pub fn solve_area(
    registry: &PrefabRegistry,
    pool_id: &str,
    required_anchors: &[String],
    pieces_min: u32,
    pieces_max: u32,
    origin: [i32; 3],
    stream: &mut Splitmix64,
) -> Result<AreaLayout, SolveError> {
    let members: &[PoolMember] = registry
        .pool(pool_id)
        .ok_or_else(|| SolveError::new(DW_NO_ENTRY, format!("pool `{pool_id}` is not declared")))?;

    // Entry piece (role `entry`). Exactly one is expected; the first wins.
    let entry_prefab = members
        .iter()
        .find(|m| m.role == "entry")
        .map(|m| m.prefab.clone())
        .ok_or_else(|| {
            SolveError::new(
                DW_NO_ENTRY,
                format!("pool `{pool_id}` declares no `entry`-role piece"),
            )
        })?;

    // Map each required anchor to the pool piece that carries it (dedup: one
    // piece may carry several required anchors, e.g. gate-room's gate + stand).
    let mut required_prefabs: Vec<String> = Vec::new();
    for anchor in required_anchors {
        let carriers = registry.pool_prefabs_with_anchor(pool_id, anchor);
        let Some(prefab) = carriers.into_iter().next() else {
            return Err(SolveError::new(
                DW_UNSATISFIABLE_ANCHOR,
                format!("pool `{pool_id}` has no piece providing required anchor `{anchor}`"),
            ));
        };
        if !required_prefabs.contains(&prefab) {
            required_prefabs.push(prefab);
        }
    }

    // Order required pieces so single-socket dead-ends come last, boss-hall
    // absolutely last (farthest terminal). Through-rooms (≥2 sockets) go first so
    // the spine can continue past them.
    required_prefabs.sort_by_key(|p| {
        let sockets = socket_count(registry, p);
        let dead_end = sockets <= 1;
        let boss = p.contains("boss");
        (dead_end, boss)
    });

    // Split required pieces into through-rooms (≥2 sockets, keep the spine going)
    // and dead-end terminals (≤1 socket, cap a branch). A single terminal grows a
    // straight walkable spine (the pre-pathfinder behaviour, kept byte-identical);
    // two or more terminals grow a **branching tree** (M2 `m2-gameplay-verbs`,
    // lifts the old `DW0304` "one terminal max" limit — the harness now
    // pathfinds, so branches/turns are walkable).
    let through: Vec<&String> = required_prefabs
        .iter()
        .filter(|p| socket_count(registry, p) >= 2)
        .collect();
    let terminals: Vec<&String> = required_prefabs
        .iter()
        .filter(|p| socket_count(registry, p) <= 1)
        .collect();
    let n_terminals = terminals.len();
    // Two or more terminals need (n_terminals − 1) branch pieces (tees/crosses) to
    // create the extra branch sockets (a 1-socket entry + 2-socket through-rooms
    // otherwise expose only one open socket at a time).
    let branch_needed = (n_terminals as u32).saturating_sub(1);

    // Total piece count: entry + required + fillers. Choose N in [min,max].
    let min_needed = 1 + required_prefabs.len() as u32;
    let min_with_branch = min_needed + branch_needed;
    if pieces_max < min_with_branch {
        return Err(SolveError::new(
            DW_RANGE_TOO_SMALL,
            format!(
                "pool `{pool_id}` needs at least {min_with_branch} pieces (entry + {} required \
                 anchor-bearing{}) but `pieces.max` is {pieces_max}",
                required_prefabs.len(),
                if branch_needed > 0 {
                    format!(" + {branch_needed} branch")
                } else {
                    String::new()
                }
            ),
        ));
    }
    let lo = pieces_min.max(min_with_branch);
    let n = if pieces_max > lo {
        lo + (stream.next_u64() % (pieces_max - lo + 1) as u64) as u32
    } else {
        lo
    };
    let filler_count = n - min_needed;

    // Connector pools. `connector_members` (straight-preferring) drives the
    // single-terminal spine, unchanged; branching uses `all_connectors` for
    // extensions and `branchers` (≥3 sockets: tee/cross) for branch points.
    let all_connectors: Vec<&PoolMember> =
        members.iter().filter(|m| m.role == "connector").collect();
    let straight: Vec<&PoolMember> = all_connectors
        .iter()
        .copied()
        .filter(|m| is_straight_through(registry, &m.prefab))
        .collect();
    let connector_members: Vec<&PoolMember> = if straight.is_empty() {
        all_connectors.clone()
    } else {
        straight
    };
    if connector_members.is_empty() && filler_count > 0 {
        return Err(SolveError::new(
            DW_NO_ENTRY,
            format!("pool `{pool_id}` declares no `connector`-role filler pieces"),
        ));
    }

    // --- grow ---
    let mut pieces: Vec<PlacedPiece> = Vec::new();
    let mut frontier: Vec<OpenSocket> = Vec::new();

    // Place the entry at the origin, rotation none.
    place_piece(
        registry,
        &entry_prefab,
        origin,
        Rotation::None,
        &mut pieces,
        &mut frontier,
    )?;

    if n_terminals >= 2 {
        grow_branching(
            registry,
            pool_id,
            &through,
            &terminals,
            &all_connectors,
            filler_count,
            branch_needed,
            &mut pieces,
            &mut frontier,
            stream,
        )?;
    } else {
        grow_spine(
            registry,
            &through,
            terminals.first().copied(),
            &connector_members,
            filler_count,
            &mut pieces,
            &mut frontier,
            stream,
        )?;
    }

    // --- seal ---
    let seals = seal_layout(registry, &pieces);

    Ok(AreaLayout { pieces, seals })
}

/// The number of connector sockets a prefab declares (0 if unknown).
fn socket_count(registry: &PrefabRegistry, prefab_id: &str) -> usize {
    registry
        .get(prefab_id)
        .map(|m| m.connectors.len())
        .unwrap_or(0)
}

/// Grow a straight walkable spine (entry already placed): straight-preferring
/// fillers threaded around the through-rooms, ending at the single dead-end
/// terminal (if any). This is the pre-pathfinder behaviour, preserved
/// bit-for-bit so single-terminal pools (`keep-crawl`) stay byte-identical.
#[allow(clippy::too_many_arguments)]
fn grow_spine(
    registry: &PrefabRegistry,
    through: &[&String],
    terminal: Option<&String>,
    connector_members: &[&PoolMember],
    filler_count: u32,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
    stream: &mut Splitmix64,
) -> Result<(), SolveError> {
    // Split fillers into gaps: before each through waypoint, plus one trailing gap
    // before the terminal / at the end. Deterministic even split.
    let gaps = through.len() + 1;
    let mut gap_fillers = vec![0u32; gaps];
    for i in 0..filler_count {
        gap_fillers[(i as usize) % gaps] += 1;
    }

    let place_fillers = |count: u32,
                         pieces: &mut Vec<PlacedPiece>,
                         frontier: &mut Vec<OpenSocket>,
                         stream: &mut Splitmix64|
     -> Result<(), SolveError> {
        for _ in 0..count {
            let weights: Vec<u32> = connector_members.iter().map(|m| m.weight).collect();
            let choice = stream.weighted(&weights).unwrap_or(0);
            attach_piece(
                registry,
                &connector_members[choice].prefab,
                pieces,
                frontier,
            )?;
        }
        Ok(())
    };

    for (gi, waypoint) in through.iter().enumerate() {
        place_fillers(gap_fillers[gi], pieces, frontier, stream)?;
        attach_piece(registry, waypoint, pieces, frontier)?;
    }
    place_fillers(gap_fillers[gaps - 1], pieces, frontier, stream)?;
    if let Some(term) = terminal {
        attach_piece(registry, term, pieces, frontier)?;
    }
    Ok(())
}

/// Grow a branching tree (entry already placed): the through-rooms extend the
/// trunk, `brancher` fillers (tee/cross) open the extra branch sockets needed for
/// two or more terminals, remaining fillers extend branches, and each terminal
/// caps a distinct open socket. Guarantees: connected (every piece mates to an
/// open socket), each required piece placed exactly once, open sockets sealed.
#[allow(clippy::too_many_arguments)]
fn grow_branching(
    registry: &PrefabRegistry,
    pool_id: &str,
    through: &[&String],
    terminals: &[&String],
    all_connectors: &[&PoolMember],
    filler_count: u32,
    branch_needed: u32,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
    stream: &mut Splitmix64,
) -> Result<(), SolveError> {
    // Branch-capable fillers: ≥3 sockets (tee = +1 open, cross = +2).
    let branchers: Vec<&PoolMember> = all_connectors
        .iter()
        .copied()
        .filter(|m| socket_count(registry, &m.prefab) >= 3)
        .collect();
    if branchers.is_empty() && branch_needed > 0 {
        return Err(SolveError::new(
            DW_INFEASIBLE,
            format!(
                "pool `{pool_id}` needs a branch piece (tee/cross, ≥3 sockets) to host \
                 {} terminals, but declares none",
                terminals.len()
            ),
        ));
    }

    // Straight-preferring extension set (keeps the trunk from wrapping back on
    // itself, so large terminals still fit); falls back to the full set.
    let straight: Vec<&PoolMember> = all_connectors
        .iter()
        .copied()
        .filter(|m| is_straight_through(registry, &m.prefab))
        .collect();
    let extensions: &[&PoolMember] = if straight.is_empty() {
        all_connectors
    } else {
        &straight
    };

    // Through-rooms extend the trunk.
    for wp in through {
        attach_piece(registry, wp, pieces, frontier)?;
    }

    // Extend the trunk with the non-branch fillers FIRST, before forking — so the
    // terminals cap fresh branch sockets at the far end of the trunk, where space
    // is uncrowded (greedy tree growth has no backtracking).
    let extension_count = filler_count.saturating_sub(branch_needed);
    for _ in 0..extension_count {
        let weights: Vec<u32> = extensions.iter().map(|m| m.weight).collect();
        let choice = stream.weighted(&weights).unwrap_or(0);
        attach_piece(registry, &extensions[choice].prefab, pieces, frontier)?;
    }

    // Open enough branch sockets for every terminal.
    let mut branch_budget = filler_count - extension_count;
    while frontier.len() < terminals.len() {
        if branch_budget == 0 {
            return Err(SolveError::new(
                DW_INFEASIBLE,
                format!(
                    "pool `{pool_id}` ran out of filler budget opening branches for \
                     {} terminals",
                    terminals.len()
                ),
            ));
        }
        let weights: Vec<u32> = branchers.iter().map(|m| m.weight).collect();
        let choice = stream.weighted(&weights).unwrap_or(0);
        attach_piece(registry, &branchers[choice].prefab, pieces, frontier)?;
        branch_budget -= 1;
    }

    // Cap each terminal on a distinct open socket (most-recent first, so the two
    // freshest branch sockets take the two terminals).
    for term in terminals {
        attach_piece(registry, term, pieces, frontier)?;
    }
    Ok(())
}

/// Whether a prefab is a "straight through" connector: exactly two sockets with
/// opposite facings (so mating in and continuing out keeps a straight line).
fn is_straight_through(registry: &PrefabRegistry, prefab_id: &str) -> bool {
    let Some(meta) = registry.get(prefab_id) else {
        return false;
    };
    if meta.connectors.len() != 2 {
        return false;
    }
    match (
        Facing::parse(&meta.connectors[0].facing),
        Facing::parse(&meta.connectors[1].facing),
    ) {
        (Some(a), Some(b)) => a.opposite() == b,
        _ => false,
    }
}

/// Place the entry piece at a fixed pose (no mating).
fn place_piece(
    registry: &PrefabRegistry,
    prefab_id: &str,
    pos: [i32; 3],
    rotation: Rotation,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
) -> Result<(), SolveError> {
    let meta = registry.get(prefab_id).ok_or_else(|| {
        SolveError::new(
            DW_INFEASIBLE,
            format!("prefab `{prefab_id}` metadata missing"),
        )
    })?;
    let (bbox_min, bbox_max) = rotation.bbox(pos, meta.structure.size);
    let idx = pieces.len();
    let mated = vec![false; meta.connectors.len()];
    pieces.push(PlacedPiece {
        prefab_id: prefab_id.to_string(),
        pos,
        rotation,
        bbox_min,
        bbox_max,
        mated,
    });
    for (ci, c) in meta.connectors.iter().enumerate() {
        let (wp, f) = socket_world(pos, rotation, c)?;
        frontier.push(OpenSocket {
            piece: idx,
            connector: ci,
            world_pos: wp,
            facing: f,
        });
    }
    Ok(())
}

/// Attach `prefab_id` to some open frontier socket, mating one of its connectors.
/// Tries frontier sockets (most recent first, for a spine) and, for each, the
/// prefab's connectors; the mating rule fixes the rotation. Rejects overlaps.
fn attach_piece(
    registry: &PrefabRegistry,
    prefab_id: &str,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
) -> Result<(), SolveError> {
    let meta = registry.get(prefab_id).ok_or_else(|| {
        SolveError::new(
            DW_INFEASIBLE,
            format!("prefab `{prefab_id}` metadata missing"),
        )
    })?;
    let cand = Candidate {
        prefab_id,
        size: meta.structure.size,
        connectors: &meta.connectors,
    };

    // Most-recently-added open socket first → a straight, extending spine.
    for fi in (0..frontier.len()).rev() {
        let socket = frontier[fi].clone();
        for (ci, conn) in cand.connectors.iter().enumerate() {
            let Some(conn_facing) = Facing::parse(&conn.facing) else {
                continue;
            };
            // Rotation so the child socket faces opposite the parent socket.
            let want = socket.facing.opposite();
            let Some(rot) = Rotation::ALL
                .iter()
                .copied()
                .find(|&r| conn_facing.rotate(r) == want)
            else {
                continue;
            };
            // pos = ws + unit(ds) − transform(Lc, r).
            let ds = socket.facing.unit();
            let tl = rot.transform(conn.local_pos);
            let pos = [
                socket.world_pos[0] + ds[0] - tl[0],
                socket.world_pos[1] + ds[1] - tl[1],
                socket.world_pos[2] + ds[2] - tl[2],
            ];
            let (bmin, bmax) = rot.bbox(pos, cand.size);
            // Reject overlap with any placed piece except a shared face with the
            // parent (flush faces do not overlap by the inclusive test).
            if pieces
                .iter()
                .any(|p| aabb_overlap((&bmin, &bmax), (&p.bbox_min, &p.bbox_max)))
            {
                continue;
            }
            // Accept: place, mark both sockets mated, refresh the frontier.
            let idx = pieces.len();
            pieces.push(PlacedPiece {
                prefab_id: cand.prefab_id.to_string(),
                pos,
                rotation: rot,
                bbox_min: bmin,
                bbox_max: bmax,
                mated: vec![false; cand.connectors.len()],
            });
            pieces[idx].mated[ci] = true;
            pieces[socket.piece].mated[socket.connector] = true;
            // Remove the consumed parent socket; add the child's other sockets.
            frontier.remove(fi);
            for (cj, c2) in cand.connectors.iter().enumerate() {
                if cj == ci {
                    continue;
                }
                let (wp, f) = socket_world(pos, rot, c2)?;
                frontier.push(OpenSocket {
                    piece: idx,
                    connector: cj,
                    world_pos: wp,
                    facing: f,
                });
            }
            return Ok(());
        }
    }
    Err(SolveError::new(
        DW_INFEASIBLE,
        format!("could not place `{prefab_id}` at any open socket without overlap"),
    ))
}

/// The world pose (socket cell + facing) of a connector on a placed piece.
fn socket_world(
    pos: [i32; 3],
    rot: Rotation,
    conn: &Connector,
) -> Result<([i32; 3], Facing), SolveError> {
    let f = Facing::parse(&conn.facing).ok_or_else(|| {
        SolveError::new(
            DW_INFEASIBLE,
            format!("connector has invalid facing `{}`", conn.facing),
        )
    })?;
    let t = rot.transform(conn.local_pos);
    let wp = [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]];
    Ok((wp, f.rotate(rot)))
}

/// Build the seal/clear fills for every connector of every placed piece.
fn seal_layout(registry: &PrefabRegistry, pieces: &[PlacedPiece]) -> Vec<SealFill> {
    let mut seals = Vec::new();
    for piece in pieces {
        let Some(meta) = registry.get(&piece.prefab_id) else {
            continue;
        };
        for (ci, conn) in meta.connectors.iter().enumerate() {
            let Ok((wp, facing)) = socket_world(piece.pos, piece.rotation, conn) else {
                continue;
            };
            let (from, to) = opening_region(wp, facing, conn.opening);
            let block = if piece.mated.get(ci).copied().unwrap_or(false) {
                // Mated: clear the jigsaw block so the 3×3 passage is open.
                "minecraft:air"
            } else {
                // Open/unmated: seal the doorway with wall material.
                "minecraft:stone_bricks"
            };
            seals.push(SealFill {
                from,
                to,
                block: block.to_string(),
            });
        }
    }
    seals
}

/// The world region of a socket's `[w,h]` opening: `w` wide across the facing's
/// perpendicular horizontal axis, `h` tall in +y, one block deep at the wall
/// plane. `wp` is the bottom-centre wall cell.
fn opening_region(wp: [i32; 3], facing: Facing, opening: [i32; 2]) -> ([i32; 3], [i32; 3]) {
    let half = (opening[0] - 1) / 2;
    let top = opening[1] - 1;
    match facing {
        Facing::North | Facing::South => (
            [wp[0] - half, wp[1], wp[2]],
            [wp[0] + half, wp[1] + top, wp[2]],
        ),
        Facing::East | Facing::West => (
            [wp[0], wp[1], wp[2] - half],
            [wp[0], wp[1] + top, wp[2] + half],
        ),
    }
}

// ---------------------------------------------------------------------------
// Anchor transform (consumed by plan.rs)
// ---------------------------------------------------------------------------

/// Transform a local point through a piece's placement (pos + rotation) to world.
pub fn transform_point(piece: &PlacedPiece, local: [i32; 3]) -> [i32; 3] {
    let t = piece.rotation.transform(local);
    [
        piece.pos[0] + t[0],
        piece.pos[1] + t[1],
        piece.pos[2] + t[2],
    ]
}

/// Rotate a facing keyword through a piece's rotation (returns the world facing
/// keyword), or `None` if unset/unparseable.
pub fn transform_facing(piece: &PlacedPiece, facing: Option<&str>) -> Option<String> {
    let f = Facing::parse(facing?)?;
    Some(
        match f.rotate(piece.rotation) {
            Facing::North => "north",
            Facing::South => "south",
            Facing::East => "east",
            Facing::West => "west",
        }
        .to_string(),
    )
}

/// Which placed piece (index) carries `anchor_name`, if any (the first match).
pub fn piece_with_anchor(
    registry: &PrefabRegistry,
    pieces: &[PlacedPiece],
    anchor_name: &str,
) -> Option<usize> {
    pieces.iter().position(|p| {
        registry
            .get(&p.prefab_id)
            .is_some_and(|m| m.anchors.contains_key(anchor_name))
    })
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_transform_matches_vanilla() {
        // Local (1,0,0): +x. Under cw90 → (0,0,1) i.e. +z (south). Under ccw90 →
        // (0,0,-1) i.e. -z (north). Under 180 → (-1,0,0).
        assert_eq!(Rotation::Cw90.transform([1, 0, 0]), [0, 0, 1]);
        assert_eq!(Rotation::Ccw90.transform([1, 0, 0]), [0, 0, -1]);
        assert_eq!(Rotation::Cw180.transform([1, 0, 0]), [-1, 0, 0]);
        // Local (0,0,1): +z. cw90 → (-1,0,0).
        assert_eq!(Rotation::Cw90.transform([0, 0, 1]), [-1, 0, 0]);
    }

    #[test]
    fn facing_rotate_is_consistent_with_transform() {
        // A north-facing socket (dir -z) rotated cw90 should face east (+x), and
        // its unit vector should match transforming the direction.
        for f in [Facing::North, Facing::South, Facing::East, Facing::West] {
            for r in Rotation::ALL {
                let rotated = f.rotate(r);
                let expect = r.transform(f.unit());
                assert_eq!(rotated.unit(), expect, "facing {f:?} rot {r:?}");
            }
        }
    }

    #[test]
    fn bbox_none_is_origin_to_size() {
        let (min, max) = Rotation::None.bbox([10, 64, 20], [9, 5, 7]);
        assert_eq!(min, [10, 64, 20]);
        assert_eq!(max, [18, 68, 26]);
    }

    #[test]
    fn bbox_cw90_shifts_into_negative_x() {
        // cw90 maps x→z, z→-x, so a [9,5,7] piece at origin spans x[-6..0], z[0..8].
        let (min, max) = Rotation::Cw90.bbox([0, 64, 0], [9, 5, 7]);
        assert_eq!(min, [-6, 64, 0]);
        assert_eq!(max, [0, 68, 8]);
    }

    #[test]
    fn aabb_flush_faces_do_not_overlap() {
        let a = ([0, 64, 0], [8, 68, 8]);
        let b = ([0, 64, 9], [4, 68, 15]); // z starts one past a's max
        assert!(!aabb_overlap((&a.0, &a.1), (&b.0, &b.1)));
        let c = ([0, 64, 8], [4, 68, 14]); // z overlaps at 8
        assert!(aabb_overlap((&a.0, &a.1), (&c.0, &c.1)));
    }

    #[test]
    fn splitmix_is_deterministic() {
        let mut a = Splitmix64::new(42);
        let mut b = Splitmix64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(stream_seed(1, "area/keep"), stream_seed(1, "area/crypt"));
    }

    /// A mating between two synthetic straight (N/S) pieces lands flush and
    /// clears/exposes the expected sockets — the core geometry the real tileset
    /// relies on.
    #[test]
    fn straight_mate_is_flush() {
        // Parent south socket at world [4,65,8] facing south; child straight piece
        // with a north socket at local [2,1,0].
        let parent = OpenSocket {
            piece: 0,
            connector: 0,
            world_pos: [4, 65, 8],
            facing: Facing::South,
        };
        let conn = Connector {
            name: "keep:socket".into(),
            target: "keep:socket".into(),
            local_pos: [2, 1, 0],
            facing: "north".into(),
            opening: [3, 3],
            joint: "aligned".into(),
        };
        let cf = Facing::parse(&conn.facing).unwrap();
        let want = parent.facing.opposite();
        let rot = Rotation::ALL
            .iter()
            .copied()
            .find(|&r| cf.rotate(r) == want)
            .unwrap();
        assert_eq!(rot, Rotation::None);
        let ds = parent.facing.unit();
        let tl = rot.transform(conn.local_pos);
        let pos = [
            parent.world_pos[0] + ds[0] - tl[0],
            parent.world_pos[1] + ds[1] - tl[1],
            parent.world_pos[2] + ds[2] - tl[2],
        ];
        assert_eq!(pos, [2, 64, 9]);
        // The child's mated socket world pos == parent + unit(ds).
        let (wp, _) = socket_world(pos, rot, &conn).unwrap();
        assert_eq!(wp, [4, 65, 9]);
    }

    #[test]
    fn opening_region_north_is_3_wide_3_tall() {
        let (from, to) = opening_region([4, 65, 8], Facing::South, [3, 3]);
        assert_eq!(from, [3, 65, 8]);
        assert_eq!(to, [5, 67, 8]);
        // East-facing spans z instead of x.
        let (from, to) = opening_region([6, 65, 3], Facing::East, [3, 3]);
        assert_eq!(from, [6, 65, 2]);
        assert_eq!(to, [6, 67, 4]);
    }
}
