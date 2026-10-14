//! **Rotations, facings and the seeded stream** the compiler places templates
//! with (ADR-0006, ADR-0035).
//!
//! A template is placed at a position with a quarter turn about its origin,
//! exactly as vanilla's `/place template <pos> <rotation>` does (pivot = the
//! structure origin, no mirror):
//!
//! | rotation             | `transform(x, y, z)` |
//! | -------------------- | -------------------- |
//! | `none`               | `( x, y,  z)`        |
//! | `clockwise_90`       | `(-z, y,  x)`        |
//! | `180`                | `(-x, y, -z)`        |
//! | `counterclockwise_90`| `( z, y, -x)`        |
//!
//! The stream is splitmix64 over named sub-streams of the one campaign seed.

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
