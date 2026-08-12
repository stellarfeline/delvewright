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

use std::collections::BTreeSet;

use crate::registry::{Connector, PoolMember, PrefabRegistry};
use delvewright_dsl::DwCode;

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

    /// The blockstate keyword for this facing (`north`/`south`/`east`/`west`) —
    /// the inverse of [`Facing::parse`].
    pub fn token(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::East => "east",
            Facing::West => "west",
        }
    }

    /// The facing of the horizontal step `from` → `to`, or `None` when the two
    /// cells are not exactly one cardinal step apart horizontally (the vertical
    /// component is ignored).
    pub fn between(from: [i32; 3], to: [i32; 3]) -> Option<Facing> {
        match (to[0] - from[0], to[2] - from[2]) {
            (1, 0) => Some(Facing::East),
            (-1, 0) => Some(Facing::West),
            (0, 1) => Some(Facing::South),
            (0, -1) => Some(Facing::North),
            _ => None,
        }
    }

    /// The two facings perpendicular to this one (the lateral axis of a stair
    /// run: the width a staircase is built across).
    pub fn perpendicular(self) -> [Facing; 2] {
        match self {
            Facing::North | Facing::South => [Facing::East, Facing::West],
            Facing::East | Facing::West => [Facing::North, Facing::South],
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
pub(crate) fn aabb_overlap(a: (&[i32; 3], &[i32; 3]), b: (&[i32; 3], &[i32; 3])) -> bool {
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
    pub code: DwCode,
    /// Human-readable explanation.
    pub message: String,
    /// The prefab ids the draw had already seated when the failure was raised,
    /// in placement order — empty for failures raised before growth.
    ///
    /// Carried so the caller can still explain the layout that produced the
    /// error: a `DW0305` ambiguous anchor is very often the downstream symptom
    /// of a pool that seated one anchor-bearing prefab twice, and the
    /// pool-level `DW0498` ([`crate::pool`]) is only useful if it is printed
    /// *with* the hard failure rather than instead of it.
    pub placed: Vec<String>,
}

impl SolveError {
    fn new(code: DwCode, message: impl Into<String>) -> Self {
        SolveError {
            code,
            message: message.into(),
            placed: Vec::new(),
        }
    }

    /// The same error, carrying the draw that was on the table when it fired.
    fn with_placed(mut self, pieces: &[PlacedPiece]) -> Self {
        self.placed = pieces.iter().map(|p| p.prefab_id.clone()).collect();
        self
    }
}

/// `DW0301`: a pool declares no `entry`-role piece (nothing to seed the layout).
pub const DW_NO_ENTRY: DwCode = DwCode::every_version("DW0301");
/// `DW0302`: a campaign-referenced anchor is provided by no member of the pool
/// (unsatisfiable required anchor).
pub const DW_UNSATISFIABLE_ANCHOR: DwCode = DwCode::every_version("DW0302");
/// `DW0303`: the `pieces {min,max}` range is too small to fit the entry plus the
/// required anchor-bearing pieces.
pub const DW_RANGE_TOO_SMALL: DwCode = DwCode::every_version("DW0303");
/// `DW0304`: the solver could not place a required piece without an overlap, or a
/// branching layout has no branch piece (tee/cross) to fork its terminals
/// (layout infeasible for this pool / seed).
pub const DW_INFEASIBLE: DwCode = DwCode::every_version("DW0304");
/// `DW0305`: a campaign-referenced anchor is defined by **more than one** placed
/// piece, so resolving it would be silent + arbitrary (ambiguous anchor). Also the
/// role-aware failure when the only carrier of a required anchor is the entry
/// piece and the entry does not already provide it.
pub const DW_AMBIGUOUS_ANCHOR: DwCode = DwCode::every_version("DW0305");

/// The maximum number of deterministically-reordered branching attempts before a
/// layout is declared infeasible (item 2: large-terminal placement robustness).
/// Attempt 0 reproduces the pre-M2 greedy growth byte-for-byte; later attempts
/// reorder terminal capping (largest footprint first) and draw fresh filler/branch
/// choices so a big terminal (e.g. `keep-boss-hall`, 11×13) finds open space.
const MAX_BRANCH_ATTEMPTS: u32 = 32;

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
    let members: &[PoolMember] = registry.pool(pool_id).ok_or_else(|| {
        SolveError::new(
            DW_NO_ENTRY,
            format!(
                "prefab pool `{pool_id}` is not declared in the prefab metadata — bind a pool \
                     that exists in the prefab library, or add `{pool_id}` to it (prefab-library \
                     issue, not quest logic)"
            ),
        )
    })?;

    // Entry piece (role `entry`). Exactly one is expected; the first wins.
    let entry_prefab = members
        .iter()
        .find(|m| m.role == "entry")
        .map(|m| m.prefab.clone())
        .ok_or_else(|| {
            SolveError::new(
                DW_NO_ENTRY,
                format!(
                    "prefab pool `{pool_id}` declares no `entry`-role piece to seed the layout — \
                     add an `entry`-role member to the pool's metadata (prefab-library issue, not \
                     quest logic)"
                ),
            )
        })?;

    // Anchors the (already-fixed) entry piece provides. Role-aware capping never
    // re-adds the entry as a required/cap piece: it is placed exactly once, at the
    // origin, and already resolves its own anchors (e.g. spawn-hall's `spawn` +
    // `anchor/exit`). Without this, an NPC anchored to `anchor/exit` used to force
    // a *second* spawn-hall (hollow-vigil's duplicate-spawn bug).
    let entry_anchors: BTreeSet<String> = registry
        .get(&entry_prefab)
        .map(|m| m.anchors.keys().cloned().collect())
        .unwrap_or_default();

    // Map each required anchor to the pool piece that carries it. Iterate anchors
    // in sorted order for determinism (plan.rs already passes a sorted set; sorting
    // a local copy makes direct solver calls deterministic too, and matches the
    // former byte output). Coverage-reuse: if an already-selected required piece
    // already carries this anchor, do not add a second piece — this makes
    // hollow-vigil's `anchor/objective` resolve to the boss-hall that `anchor/boss`
    // already forces, instead of pulling in a redundant shrine that would also
    // define `anchor/objective` (→ ambiguity). Entry-role carriers are excluded.
    let mut sorted_anchors: Vec<String> = required_anchors.to_vec();
    sorted_anchors.sort();
    sorted_anchors.dedup();
    let mut required_prefabs: Vec<String> = Vec::new();
    for anchor in &sorted_anchors {
        if entry_anchors.contains(anchor) {
            continue;
        }
        if required_prefabs
            .iter()
            .any(|p| piece_defines(registry, p, anchor))
        {
            continue;
        }
        let prefab = registry
            .pool_prefabs_with_anchor(pool_id, anchor)
            .into_iter()
            .find(|p| !is_entry_role(members, p));
        let Some(prefab) = prefab else {
            return Err(SolveError::new(
                DW_UNSATISFIABLE_ANCHOR,
                format!(
                    "prefab pool `{pool_id}` has no non-entry piece providing required anchor \
                     `{anchor}` — either the campaign references an anchor the pool cannot supply \
                     (use one a pool piece carries), or the pool is missing a piece that defines \
                     `{anchor}` (add it to the pool metadata)"
                ),
            ));
        };
        required_prefabs.push(prefab);
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
                 anchor-bearing{}) but the area's `pieces.max` is {pieces_max} — raise \
                 `pieces.max` to at least {min_with_branch}, or reduce the anchors this area must \
                 provide",
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
            format!(
                "prefab pool `{pool_id}` needs `connector`-role filler pieces to span its \
                 `pieces` budget but declares none — add a `connector`-role member to the pool \
                 metadata, or lower the area's `pieces.min` (prefab-library issue)"
            ),
        ));
    }

    // Stair connectors (keep-socket-v1 pieces whose two sockets sit at different
    // local y — a vertical rise). A pool with no stair behaves exactly as before
    // (existing fixtures stay byte-identical); when a pool has stairs and there is
    // filler budget, growth forces at least one so the layout spans ≥2 elevations.
    let stairs: Vec<&PoolMember> = all_connectors
        .iter()
        .copied()
        .filter(|m| is_stair(registry, &m.prefab))
        .collect();

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
            &stairs,
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
            &stairs,
            filler_count,
            &mut pieces,
            &mut frontier,
            stream,
        )?;
    }

    // --- ambiguous-anchor check (DW0305) ---
    // Every campaign-referenced anchor must resolve to exactly one placed piece;
    // two placed carriers would resolve silently + arbitrarily (hollow-vigil's
    // `anchor/objective` landing on the shrine rather than the referenced boss-hall).
    for anchor in &sorted_anchors {
        let carriers: Vec<&str> = pieces
            .iter()
            .filter(|p| piece_defines(registry, &p.prefab_id, anchor))
            .map(|p| p.prefab_id.as_str())
            .collect();
        if carriers.len() > 1 {
            return Err(SolveError::new(
                DW_AMBIGUOUS_ANCHOR,
                format!(
                    "campaign-referenced anchor `{anchor}` is defined by {} placed pieces \
                     ({}); resolution would be arbitrary — reference a piece-unique anchor",
                    carriers.len(),
                    carriers.join(", ")
                ),
            )
            // The draw travels with the error so the caller can add the
            // pool-level `DW0498` explanation (task #187).
            .with_placed(&pieces));
        }
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
    stairs: &[&PoolMember],
    filler_count: u32,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
    stream: &mut Splitmix64,
) -> Result<(), SolveError> {
    // Vertical: force one stair (deterministic weighted pick) at the head of the
    // spine when the pool has stairs and there is filler budget — every piece past
    // it sits one elevation higher, so the spine (and its terminal finale) spans ≥2
    // levels. No stairs / no budget → this whole block is skipped and the spine is
    // byte-identical to the pre-M2 behaviour.
    let mut filler_count = filler_count;
    if !stairs.is_empty() && filler_count > 0 {
        let weights: Vec<u32> = stairs.iter().map(|m| m.weight).collect();
        let choice = stream.weighted(&weights).unwrap_or(0);
        attach_piece(registry, &stairs[choice].prefab, pieces, frontier)?;
        filler_count -= 1;
    }

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
///
/// **Robustness (item 2).** A single greedy pass often cannot fit a large terminal
/// (`keep-boss-hall`, 11×13) once smaller branches crowd the space. This wraps the
/// greedy pass in a bounded, deterministic retry: attempt 0 reproduces the pre-M2
/// growth byte-for-byte (existing fixtures unchanged); each later attempt reorders
/// terminal capping (largest footprint first, so the big terminal grabs open space
/// before the small ones) and — because the shared PRNG has advanced through the
/// failed attempts — draws fresh filler/branch choices, exploring different
/// layouts. The first attempt that places everything without overlap wins.
#[allow(clippy::too_many_arguments)]
fn grow_branching(
    registry: &PrefabRegistry,
    pool_id: &str,
    through: &[&String],
    terminals: &[&String],
    all_connectors: &[&PoolMember],
    stairs: &[&PoolMember],
    filler_count: u32,
    branch_needed: u32,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
    stream: &mut Splitmix64,
) -> Result<(), SolveError> {
    // Branch-capable fillers: ≥3 sockets (tee = +1 open, cross = +2). A structural
    // (seed-independent) failure, so it is checked once, before any retry.
    let branchers: Vec<&PoolMember> = all_connectors
        .iter()
        .copied()
        .filter(|m| socket_count(registry, &m.prefab) >= 3)
        .collect();
    if branchers.is_empty() && branch_needed > 0 {
        return Err(SolveError::new(
            DW_INFEASIBLE,
            format!(
                "prefab pool `{pool_id}` needs a branch piece (tee/cross, ≥3 sockets) to host \
                 {} dead-end terminals, but declares none — add a ≥3-socket branch member to the \
                 pool metadata, or reduce the number of anchor-bearing dead-end rooms this area \
                 requires (prefab-library issue)",
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

    // Snapshot the entry-only state so each attempt starts fresh; the PRNG stream
    // is NOT reset, so retries draw different choices (determinism is preserved:
    // same seed → same attempt sequence → same result).
    let base_pieces = pieces.clone();
    let base_frontier = frontier.clone();
    let mut last_err: Option<SolveError> = None;
    for attempt in 0..MAX_BRANCH_ATTEMPTS {
        let mut p = base_pieces.clone();
        let mut f = base_frontier.clone();
        match try_branching(
            attempt,
            registry,
            pool_id,
            through,
            terminals,
            extensions,
            &branchers,
            stairs,
            filler_count,
            branch_needed,
            &mut p,
            &mut f,
            stream,
        ) {
            Ok(()) => {
                *pieces = p;
                *frontier = f;
                return Ok(());
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        SolveError::new(
            DW_INFEASIBLE,
            format!(
                "prefab pool `{pool_id}` could not place all terminal rooms without overlap after \
                 every deterministic retry — the layout is infeasible for this pool. Give the \
                 pool larger `pieces` budget, more/smaller connector pieces, or fewer required \
                 dead-end anchors. Do NOT reroll the `seed` to dodge this (ADR-0006): the seed is \
                 fixed and a different one would not make the pool geometrically fit"
            ),
        )
    }))
}

/// One branching-growth attempt on fresh piece/frontier copies. `attempt == 0`
/// reproduces the pre-M2 greedy pass exactly (byte-identity); later attempts cap
/// terminals largest-first.
#[allow(clippy::too_many_arguments)]
fn try_branching(
    attempt: u32,
    registry: &PrefabRegistry,
    pool_id: &str,
    through: &[&String],
    terminals: &[&String],
    extensions: &[&PoolMember],
    branchers: &[&PoolMember],
    stairs: &[&PoolMember],
    filler_count: u32,
    branch_needed: u32,
    pieces: &mut Vec<PlacedPiece>,
    frontier: &mut Vec<OpenSocket>,
    stream: &mut Splitmix64,
) -> Result<(), SolveError> {
    // Through-rooms extend the trunk.
    for wp in through {
        attach_piece(registry, wp, pieces, frontier)?;
    }

    // Vertical: force one stair into the trunk (deterministic weighted pick) when
    // the pool has stairs and there is extension budget, so a branching layout also
    // spans ≥2 elevations. Skipped (byte-identical) for stair-free pools.
    let mut extension_count = filler_count.saturating_sub(branch_needed);
    if !stairs.is_empty() && extension_count > 0 {
        let weights: Vec<u32> = stairs.iter().map(|m| m.weight).collect();
        let choice = stream.weighted(&weights).unwrap_or(0);
        attach_piece(registry, &stairs[choice].prefab, pieces, frontier)?;
        extension_count -= 1;
    }

    // Extend the trunk with the non-branch fillers FIRST, before forking — so the
    // terminals cap fresh branch sockets at the far end of the trunk, where space
    // is uncrowded.
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
                    "prefab pool `{pool_id}` ran out of `pieces` filler budget while opening \
                     branches for {} terminal rooms — raise the area's `pieces.max` so there is \
                     budget for a branch piece per extra terminal, or require fewer dead-end \
                     anchor rooms",
                    terminals.len()
                ),
            ));
        }
        let weights: Vec<u32> = branchers.iter().map(|m| m.weight).collect();
        let choice = stream.weighted(&weights).unwrap_or(0);
        attach_piece(registry, &branchers[choice].prefab, pieces, frontier)?;
        branch_budget -= 1;
    }

    // Cap each terminal on a distinct open socket. Attempt 0 keeps the given order
    // (byte-identity); later attempts cap the largest-footprint terminal first so
    // it claims open space before the small ones crowd it. `attach_piece` already
    // scans every open socket, so this is where the retry pays off.
    let mut order: Vec<&String> = terminals.to_vec();
    if attempt > 0 {
        order.sort_by(|a, b| {
            footprint_area(registry, b)
                .cmp(&footprint_area(registry, a))
                .then_with(|| a.cmp(b))
        });
    }
    for term in order {
        attach_piece(registry, term, pieces, frontier)?;
    }
    Ok(())
}

/// Whether a prefab defines `anchor_name` in its metadata.
fn piece_defines(registry: &PrefabRegistry, prefab_id: &str, anchor_name: &str) -> bool {
    registry
        .get(prefab_id)
        .is_some_and(|m| m.anchors.contains_key(anchor_name))
}

/// Whether `prefab_id` is the pool's `entry`-role piece.
fn is_entry_role(members: &[PoolMember], prefab_id: &str) -> bool {
    members
        .iter()
        .any(|m| m.prefab == prefab_id && m.role == "entry")
}

/// Whether a prefab is a stair connector: ≥2 sockets whose local `y` differs (a
/// vertical rise between the two doorways). Both up and down are the same piece,
/// distinguished only by which socket mates to the parent.
fn is_stair(registry: &PrefabRegistry, prefab_id: &str) -> bool {
    let Some(meta) = registry.get(prefab_id) else {
        return false;
    };
    if meta.connectors.len() < 2 {
        return false;
    }
    let y0 = meta.connectors[0].local_pos[1];
    meta.connectors.iter().any(|c| c.local_pos[1] != y0)
}

/// A prefab's horizontal footprint (x·z), used to cap the largest terminal first.
fn footprint_area(registry: &PrefabRegistry, prefab_id: &str) -> i32 {
    registry
        .get(prefab_id)
        .map(|m| m.structure.size[0] * m.structure.size[2])
        .unwrap_or(0)
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
            format!(
                "internal invariant violation: the solver selected prefab `{prefab_id}` but its \
                 metadata is missing from the registry — pool membership and the metadata registry \
                 disagree. This is a compiler/prefab-library bug; stop and escalate"
            ),
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
            format!(
                "internal invariant violation: the solver selected prefab `{prefab_id}` but its \
                 metadata is missing from the registry — pool membership and the metadata registry \
                 disagree. This is a compiler/prefab-library bug; stop and escalate"
            ),
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
        format!(
            "could not place prefab `{prefab_id}` at any open socket without overlap — the \
             partial layout leaves no non-colliding socket for this piece. Enlarge the pool's \
             connector variety or `pieces` budget, or shrink the piece footprints. Do NOT reroll \
             the `seed` to dodge this (ADR-0006) — the seed is fixed"
        ),
    ))
}

/// The world pose (socket cell + facing) of a connector on a placed piece.
pub(crate) fn socket_world(
    pos: [i32; 3],
    rot: Rotation,
    conn: &Connector,
) -> Result<([i32; 3], Facing), SolveError> {
    let f = Facing::parse(&conn.facing).ok_or_else(|| {
        SolveError::new(
            DW_INFEASIBLE,
            format!(
                "prefab connector declares invalid facing `{}` — a connector's `facing` must be \
                 one of north/south/east/west/up/down. Fix the prefab's socket metadata \
                 (prefab-library defect, not a campaign error)",
                conn.facing
            ),
        )
    })?;
    let t = rot.transform(conn.local_pos);
    let wp = [pos[0] + t[0], pos[1] + t[1], pos[2] + t[2]];
    Ok((wp, f.rotate(rot)))
}

/// Build the seal/clear fills for every connector of every placed piece.
pub(crate) fn seal_layout(registry: &PrefabRegistry, pieces: &[PlacedPiece]) -> Vec<SealFill> {
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
pub(crate) fn opening_region(
    wp: [i32; 3],
    facing: Facing,
    opening: [i32; 2],
) -> ([i32; 3], [i32; 3]) {
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
