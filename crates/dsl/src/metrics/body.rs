//! **How a vanilla body moves, one tick at a time** (spec-0113 §1): the engine's
//! own simulator of the pinned game's body under the pinned physics, over any
//! collision world a caller hands it.
//!
//! The rules are recorded, with their read sites, in
//! `docs/reference/body-movement.md`; this module carries the arithmetic and the
//! constants, and the reference page carries no number a test here holds.
//!
//! # Two rule sets, one simulator
//!
//! The tick is written once. It runs under one of two [`Rules`]:
//!
//! * [`Rules::Bot`] is the client physics the harness bot runs —
//!   prismarine-physics 1.11.1 (MIT) at the harness lockfile's pin, as it
//!   configures itself for 1.21. `the_simulator_matches_the_bot_physics` holds
//!   this module to that library tick for tick over a committed case file the
//!   library itself wrote (`crates/dsl/tests/fixtures/movement/`).
//! * [`Rules::Game`] is the pinned jar's own rules where
//!   `body-movement.md` cites one and the bot's arithmetic differs from it: the
//!   movement-axis order (`Direction.axisStepOrder`), the ground acceleration
//!   and friction in the jar's `float` constants (`getFrictionInfluencedSpeed`,
//!   `travelInAir`), the jump (`max(power, vy)`, the honey jump factor `0.5`),
//!   the block speed factor of soul sand and honey (`getBlockSpeedFactor`), the
//!   slime step (`SlimeBlock.stepOn`), the sprint's forward-impulse condition
//!   (`canStartSprinting`), the small-velocity cut (`aiStep`), the water drag
//!   while sprinting (`travelInWater`) and its sink (`getFluidFallingAdjustedMovement`),
//!   the `Mth.sin` table, and the pose (`updatePlayerPose`). Everything else is
//!   the bot's arithmetic, which the record found the jar's on every rig of its
//!   §9. The ballistic half of these rules is held to the pinned server by the
//!   PackTest `movement_ballistics`.
//!
//! The reach graph runs [`Rules::Game`]: it asks what a player can do. A declared
//! jump is proven under both, because the bot that drives it runs the other.
//!
//! # What the simulator does not take
//!
//! Elytra, riptide, levitation, firework boosts, bubble columns, scaffolding,
//! the honey slide, powder snow, depth strider and every effect: a campaign that
//! hands a player any of them is refused (`DW1043`) rather than simulated
//! wrongly. A sprint ended by a horizontal collision is not ended here — the
//! body keeps the faster speed, which is the over-approximating direction.

/// An axis-aligned box, in blocks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// The least corner.
    pub min: [f64; 3],
    /// The greatest corner.
    pub max: [f64; 3],
}

impl Aabb {
    /// The box `[x0, y0, z0] .. [x1, y1, z1]`.
    #[must_use]
    pub const fn new(x0: f64, y0: f64, z0: f64, x1: f64, y1: f64, z1: f64) -> Aabb {
        Aabb {
            min: [x0, y0, z0],
            max: [x1, y1, z1],
        }
    }

    /// This box moved by `d`.
    #[must_use]
    pub fn offset(mut self, d: [f64; 3]) -> Aabb {
        for (i, v) in d.iter().enumerate() {
            self.min[i] += v;
            self.max[i] += v;
        }
        self
    }

    /// This box grown toward `d` on each axis (its far face moves, its near one
    /// stays) — the region a move by `d` sweeps.
    #[must_use]
    pub fn extend(mut self, d: [f64; 3]) -> Aabb {
        for (i, v) in d.iter().enumerate() {
            if *v < 0.0 {
                self.min[i] += v;
            } else {
                self.max[i] += v;
            }
        }
        self
    }

    /// This box shrunk by `d` on every face.
    #[must_use]
    pub fn contract(mut self, d: [f64; 3]) -> Aabb {
        for (i, v) in d.iter().enumerate() {
            self.min[i] += v;
            self.max[i] -= v;
        }
        self
    }

    /// Whether the two boxes overlap (strictly; touching faces do not).
    #[must_use]
    pub fn intersects(&self, o: &Aabb) -> bool {
        (0..3).all(|i| self.min[i] < o.max[i] && self.max[i] > o.min[i])
    }

    /// How far `other` may move by `off` along `axis` before it meets this box.
    fn offset_along(&self, other: &Aabb, axis: usize, off: f64) -> f64 {
        let (a, b) = match axis {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        if other.max[a] > self.min[a]
            && other.min[a] < self.max[a]
            && other.max[b] > self.min[b]
            && other.min[b] < self.max[b]
        {
            if off > 0.0 && other.max[axis] <= self.min[axis] {
                return (self.min[axis] - other.max[axis]).min(off);
            } else if off < 0.0 && other.min[axis] >= self.max[axis] {
                return (self.max[axis] - other.min[axis]).max(off);
            }
        }
        off
    }
}

/// What a block is to a moving body beyond its collision shape: the blocks
/// whose friction, speed, jump or bounce differs from the default, and the
/// non-colliding blocks a body is held or slowed by (`body-movement.md` §3.1, §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Material {
    /// Any block with the default friction `0.6`, speed and jump factor `1.0`.
    #[default]
    Plain,
    /// `ice`, `packed_ice`, `frosted_ice` — friction `0.98`.
    Ice,
    /// `blue_ice` — friction `0.989`.
    BlueIce,
    /// `slime_block` — friction `0.8`, bounce, no fall damage.
    Slime,
    /// `honey_block` — speed factor `0.4`, jump factor `0.5` (the bot: `0.4`),
    /// fall damage `× 0.2`.
    Honey,
    /// `soul_sand` — speed factor `0.4`.
    SoulSand,
    /// `hay_block` — fall damage `× 0.2`.
    Hay,
    /// Every bed — fall damage `× 0.5`.
    Bed,
    /// `cobweb` — the next move scaled by `(0.25, 0.05, 0.25)`.
    Cobweb,
    /// A ladder or a vine the world keeps (spec-0099).
    Climbable,
}

impl Material {
    /// The block friction (`BlockBehaviour.Properties.friction`).
    #[must_use]
    pub fn friction(self) -> f64 {
        match self {
            Material::Ice => 0.98,
            Material::BlueIce => 0.989,
            Material::Slime => 0.8,
            _ => 0.6,
        }
    }

    /// The fall-damage multiplier a landing on this block applies
    /// (`Block.fallOn` and its overrides).
    #[must_use]
    pub fn fall_damage_multiplier(self) -> f64 {
        match self {
            Material::Slime => 0.0,
            Material::Honey | Material::Hay => 0.2,
            Material::Bed => 0.5,
            _ => 1.0,
        }
    }
}

/// A fluid cell, as the simulator reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fluid {
    /// Lava rather than water.
    pub lava: bool,
    /// The rendered depth: `0` for a source (and for a waterlogged block or a
    /// water-filled plant), `1..=7` for flowing water.
    pub depth: u8,
    /// A falling column (`level >= 8`).
    pub falling: bool,
}

impl Fluid {
    /// Still source water — what every fluid cell of an assembled world is
    /// read as by the reach model, which carries no fluid levels.
    pub const SOURCE_WATER: Fluid = Fluid {
        lava: false,
        depth: 0,
        falling: false,
    };
}

/// **The world a body moves through**: per cell, its collision boxes, its
/// [`Material`] and its fluid. The simulator asks nothing else, so anything
/// that can answer these three questions — an assembled campaign world, a test
/// rig, a case file another implementation wrote — is a world it moves in.
pub trait CollisionWorld {
    /// Append the collision boxes of the block at `c` to `out`, in world
    /// coordinates.
    fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>);
    /// The block's material.
    fn material(&self, c: [i32; 3]) -> Material;
    /// The fluid in the cell, if any.
    fn fluid(&self, c: [i32; 3]) -> Option<Fluid>;
}

/// The body's attribute set (`body-movement.md` §8; spec-0113 §4.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Attributes {
    /// Box width.
    pub width: f64,
    /// Box height standing.
    pub height: f64,
    /// Box height crouched.
    pub crouched_height: f64,
    /// Box height swimming or crawling.
    pub crawling_height: f64,
    /// `movement_speed`.
    pub movement_speed: f64,
    /// The sprint's `movement_speed` modifier (`ADD_MULTIPLIED_TOTAL`).
    pub sprint_multiplier: f64,
    /// `step_height`.
    pub step_height: f64,
    /// `gravity`.
    pub gravity: f64,
    /// `jump_strength`.
    pub jump_strength: f64,
    /// `safe_fall_distance`.
    pub safe_fall_distance: f64,
    /// `fall_damage_multiplier`.
    pub fall_damage_multiplier: f64,
    /// `max_health`.
    pub max_health: f64,
    /// `water_movement_efficiency`.
    pub water_movement_efficiency: f64,
    /// `sneaking_speed`.
    pub sneaking_speed: f64,
}

impl Attributes {
    /// **The vanilla player body** — `Player.createAttributes` over
    /// `LivingEntity.createLivingAttributes`, the poses of `Avatar.POSES`, and
    /// the sprint modifier of `LivingEntity.setSprinting` (`body-movement.md`
    /// §2, §3, §8).
    pub const PLAYER: Attributes = Attributes {
        width: super::PLAYER_WIDTH,
        height: super::PLAYER_HEIGHT,
        crouched_height: super::PLAYER_CROUCHED_HEIGHT,
        crawling_height: 0.6,
        movement_speed: 0.1,
        sprint_multiplier: 0.3,
        step_height: 0.6,
        gravity: super::GRAVITY_BLOCKS_PER_TICK2,
        jump_strength: 0.42,
        safe_fall_distance: super::FALL_DAMAGE_ONSET_BLOCKS,
        fall_damage_multiplier: 1.0,
        max_health: super::PLAYER_MAX_HEALTH,
        water_movement_efficiency: 0.0,
        sneaking_speed: 0.3,
    };

    /// A body with this one's attributes and another box — a mob's, for the
    /// ballistic instrument (`movement_ballistics`).
    #[must_use]
    pub fn with_box(mut self, width: f64, height: f64) -> Attributes {
        self.width = width;
        self.height = height;
        self.crouched_height = height;
        self.crawling_height = height;
        self
    }

    /// The damage a fall of `distance` onto a block of `material` deals
    /// (**cited**, `LivingEntity.calculateFallDamage` / `calculateFallPower`:
    /// `floor((d + 1e-6 − safe_fall_distance) · multiplier ·
    /// fall_damage_multiplier)`).
    #[must_use]
    pub fn fall_damage(&self, distance: f64, material: Material) -> f64 {
        let power = distance + 1e-6 - self.safe_fall_distance;
        (power * material.fall_damage_multiplier() * self.fall_damage_multiplier)
            .floor()
            .max(0.0)
    }
}

/// Which arithmetic a tick runs (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    /// prismarine-physics 1.11.1 as it configures itself for 1.21.
    Bot,
    /// The pinned jar's rules where the record cites them.
    Game,
}

/// One tick's controls: the keys held and where the body faces.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Input {
    /// `+1` forward, `-1` back, `0` neither.
    pub forward: i8,
    /// `+1` right, `-1` left, `0` neither.
    pub strafe: i8,
    /// The jump key.
    pub jump: bool,
    /// The sprint key.
    pub sprint: bool,
    /// The sneak key.
    pub sneak: bool,
    /// Facing, in the game's degrees: `0` south (`+z`), `-90` east (`+x`).
    pub yaw: f64,
    /// The input vector's magnitude: `1.0` for the keys; a smaller value scales
    /// the move vector as an analogue input would (the declared-jump window's
    /// `0.98`, spec-0113 §4.4). Ignored by [`Rules::Bot`], whose bot has keys.
    pub magnitude: f64,
}

impl Input {
    /// Forward, facing `yaw`, nothing else held.
    #[must_use]
    pub fn forward(yaw: f64) -> Input {
        Input {
            forward: 1,
            yaw,
            magnitude: 1.0,
            ..Input::default()
        }
    }
}

/// The body's state between ticks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    /// Feet centre.
    pub pos: [f64; 3],
    /// Velocity.
    pub vel: [f64; 3],
    /// Whether the last move was stopped moving down.
    pub on_ground: bool,
    /// In water at the start of the last tick.
    pub in_water: bool,
    /// In lava at the start of the last tick.
    pub in_lava: bool,
    /// Caught in a cobweb: the next move is scaled.
    pub in_web: bool,
    /// The last move was stopped horizontally.
    pub collided_h: bool,
    /// The last move was stopped vertically.
    pub collided_v: bool,
    /// The jump cooldown.
    pub jump_ticks: u32,
    /// The fall distance accumulated since the body last stood, swam or held on
    /// (`Entity.checkFallDamage`).
    pub fall: f64,
    /// The damage the last landing dealt.
    pub landing_damage: f64,
    /// Whether the body is sprinting ([`Rules::Game`]; the bot sprints whenever
    /// its key is held).
    pub sprinting: bool,
    /// The box height this tick's pose gave the body.
    pub height: f64,
}

impl State {
    /// A body standing at rest with its feet at `pos`.
    #[must_use]
    pub fn standing(pos: [f64; 3], body: &Attributes) -> State {
        State {
            pos,
            vel: [0.0; 3],
            on_ground: true,
            in_water: false,
            in_lava: false,
            in_web: false,
            collided_h: false,
            collided_v: false,
            jump_ticks: 0,
            fall: 0.0,
            landing_damage: 0.0,
            sprinting: false,
            height: body.height,
        }
    }

    /// The body's box at its position, at the height its pose gives it.
    #[must_use]
    pub fn aabb(&self, body: &Attributes) -> Aabb {
        player_box(self.pos, body.width, self.height)
    }
}

fn player_box(p: [f64; 3], width: f64, height: f64) -> Aabb {
    let w = width / 2.0;
    Aabb::new(-w, 0.0, -w, w, height, w).offset(p)
}

/// The `Mth.sin` table (**cited**, `Mth`: 65536 `float`s,
/// `SIN[i] = (float)Math.sin(i · 2π / 65536)`).
fn sin_table() -> &'static [f32] {
    static TABLE: std::sync::OnceLock<Vec<f32>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        (0..65536)
            .map(|i| (f64::from(i) * std::f64::consts::PI * 2.0 / 65536.0).sin() as f32)
            .collect()
    })
}

/// `Mth.sin(x)` for `x` in `float` radians.
fn mth_sin(x: f32) -> f32 {
    sin_table()[((f64::from(x) * 10430.378350470453) as i64 & 65535) as usize]
}

/// `Mth.cos(x)`.
fn mth_cos(x: f32) -> f32 {
    sin_table()[((f64::from(x) * 10430.378350470453 + 16384.0) as i64 & 65535) as usize]
}

/// `(sin, cos)` of the facing `yaw` (game degrees) as each rule set computes it.
fn facing(rules: Rules, yaw: f64) -> (f64, f64) {
    match rules {
        // mineflayer stores `π − yaw`; prismarine turns it back with `π − ·`.
        Rules::Bot => {
            let entity_yaw = std::f64::consts::PI - yaw * std::f64::consts::PI / 180.0;
            let y = std::f64::consts::PI - entity_yaw;
            (y.sin(), y.cos())
        }
        Rules::Game => {
            let r = (yaw as f32) * (std::f32::consts::PI / 180.0);
            (f64::from(mth_sin(r)), f64::from(mth_cos(r)))
        }
    }
}

/// The simulator's scratch: the boxes a query gathered.
struct Scratch {
    boxes: Vec<Aabb>,
}

fn floor(v: f64) -> i32 {
    v.floor() as i32
}

/// Every collision box of every cell a query box touches, the cell under it
/// included (`getSurroundingBBs`: `y` from `floor(minY) − 1`).
fn surrounding<W: CollisionWorld + ?Sized>(world: &W, q: &Aabb, out: &mut Vec<Aabb>) {
    out.clear();
    for y in (floor(q.min[1]) - 1)..=floor(q.max[1]) {
        for z in floor(q.min[2])..=floor(q.max[2]) {
            for x in floor(q.min[0])..=floor(q.max[0]) {
                world.shapes([x, y, z], out);
            }
        }
    }
}

fn block_at<W: CollisionWorld + ?Sized>(world: &W, p: [f64; 3]) -> [i32; 3] {
    let _ = world;
    [floor(p[0]), floor(p[1]), floor(p[2])]
}

/// The water cells a box is in (`getWaterInBB`), as `(cell, fluid)`.
fn water_in<W: CollisionWorld + ?Sized>(world: &W, bb: &Aabb) -> Vec<([i32; 3], Fluid)> {
    let mut out = Vec::new();
    for y in floor(bb.min[1])..=floor(bb.max[1]) {
        for z in floor(bb.min[2])..=floor(bb.max[2]) {
            for x in floor(bb.min[0])..=floor(bb.max[0]) {
                if let Some(f) = world.fluid([x, y, z])
                    && !f.lava
                {
                    let level = f64::from(y) + 1.0 - (f64::from(f.depth) + 1.0) / 9.0;
                    if bb.max[1].ceil() >= level {
                        out.push(([x, y, z], f));
                    }
                }
            }
        }
    }
    out
}

fn lava_in<W: CollisionWorld + ?Sized>(world: &W, bb: &Aabb) -> bool {
    for y in floor(bb.min[1])..=floor(bb.max[1]) {
        for z in floor(bb.min[2])..=floor(bb.max[2]) {
            for x in floor(bb.min[0])..=floor(bb.max[0]) {
                if world.fluid([x, y, z]).is_some_and(|f| f.lava) {
                    return true;
                }
            }
        }
    }
    false
}

fn has_shape<W: CollisionWorld + ?Sized>(world: &W, c: [i32; 3], scratch: &mut Vec<Aabb>) -> bool {
    scratch.clear();
    world.shapes(c, scratch);
    !scratch.is_empty()
}

/// The flow vector of one water cell (`getFlow`), normalised.
fn flow<W: CollisionWorld + ?Sized>(world: &W, c: [i32; 3], f: Fluid) -> [f64; 3] {
    let depth = |p: [i32; 3]| -> i32 {
        match world.fluid(p) {
            Some(f) if !f.lava => i32::from(f.depth),
            _ => -1,
        }
    };
    let mut scratch = Vec::new();
    let cur = i32::from(f.depth);
    let mut v = [0.0f64; 3];
    for (dx, dz) in [(0, 1), (-1, 0), (0, -1), (1, 0)] {
        let adj = [c[0] + dx, c[1], c[2] + dz];
        let level = depth(adj);
        if level < 0 {
            if has_shape(world, adj, &mut scratch) {
                let below = depth([adj[0], adj[1] - 1, adj[2]]);
                if below >= 0 {
                    let k = f64::from(below - (cur - 8));
                    v[0] += f64::from(dx) * k;
                    v[2] += f64::from(dz) * k;
                }
            }
        } else {
            let k = f64::from(level - cur);
            v[0] += f64::from(dx) * k;
            v[2] += f64::from(dz) * k;
        }
    }
    if f.falling {
        for (dx, dz) in [(0, 1), (-1, 0), (0, -1), (1, 0)] {
            let adj = [c[0] + dx, c[1], c[2] + dz];
            let up = [adj[0], adj[1] + 1, adj[2]];
            if has_shape(world, adj, &mut scratch) || has_shape(world, up, &mut scratch) {
                v = normalize(v);
                v[1] -= 6.0;
            }
        }
    }
    normalize(v)
}

fn normalize(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n == 0.0 {
        v
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}

/// One tick of the body (`LivingEntity.aiStep` with `LocalPlayer.applyInput`,
/// prismarine's `simulatePlayer`).
pub fn tick<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    rules: Rules,
    st: &mut State,
    input: &Input,
) {
    let mut scratch = Scratch { boxes: Vec::new() };
    if rules == Rules::Game {
        st.height = pose_height(world, body, st, input, &mut scratch.boxes);
    } else {
        st.height = body.height;
    }
    let bb = st.aabb(body);
    let water_bb = bb.contract([0.001, 0.401, 0.001]);
    let lava_bb = bb.contract([0.1, 0.4, 0.1]);
    let waters = water_in(world, &water_bb);
    st.in_water = !waters.is_empty();
    let mut acc = [0.0f64; 3];
    for (c, f) in &waters {
        let fl = flow(world, *c, *f);
        for i in 0..3 {
            acc[i] += fl[i];
        }
    }
    let len = (acc[0] * acc[0] + acc[1] * acc[1] + acc[2] * acc[2]).sqrt();
    if len > 0.0 {
        for i in 0..3 {
            st.vel[i] += acc[i] / len * 0.014;
        }
    }
    st.in_lava = lava_in(world, &lava_bb);
    if st.in_water || st.in_lava {
        st.fall = 0.0;
    }

    // Small velocities are zeroed.
    match rules {
        Rules::Bot => {
            for v in &mut st.vel {
                if v.abs() < 0.003 {
                    *v = 0.0;
                }
            }
        }
        Rules::Game => {
            if st.vel[0] * st.vel[0] + st.vel[2] * st.vel[2] < 9.0e-6 {
                st.vel[0] = 0.0;
                st.vel[2] = 0.0;
            }
            if st.vel[1].abs() < 0.003 {
                st.vel[1] = 0.0;
            }
        }
    }

    // The sprint.
    st.sprinting = match rules {
        Rules::Bot => input.sprint,
        Rules::Game => input.sprint && input.forward > 0 && !input.sneak,
    };

    let (sin, cos) = facing(rules, input.yaw);
    // The jump.
    if input.jump {
        if st.jump_ticks > 0 {
            st.jump_ticks -= 1;
        }
        if st.in_water || st.in_lava {
            st.vel[1] += 0.04;
        } else if st.on_ground && st.jump_ticks == 0 {
            let below = block_at(
                world,
                [
                    st.pos[0].floor(),
                    st.pos[1].floor() - 0.5,
                    st.pos[2].floor(),
                ],
            );
            let power = match rules {
                Rules::Bot => {
                    let honey = world.material(below) == Material::Honey;
                    f64::from(body.jump_strength as f32) * if honey { 0.4 } else { 1.0 }
                }
                Rules::Game => {
                    let feet = block_at(world, st.pos);
                    let factor = |m: Material| if m == Material::Honey { 0.5 } else { 1.0 };
                    let f = match factor(world.material(feet)) {
                        1.0 => factor(world.material(game_below(st.pos))),
                        f => f,
                    };
                    (body.jump_strength * f).max(st.vel[1])
                }
            };
            st.vel[1] = power;
            if st.sprinting {
                st.vel[0] -= sin * 0.2;
                st.vel[2] += cos * 0.2;
            }
            st.jump_ticks = 10;
        }
    } else {
        st.jump_ticks = 0;
    }

    let mut strafe = f64::from(input.strafe) * 0.98;
    let mut forward = f64::from(input.forward) * 0.98;
    if rules == Rules::Game && input.magnitude != 1.0 {
        strafe *= input.magnitude;
        forward *= input.magnitude;
    }
    if input.sneak {
        strafe *= body.sneaking_speed;
        forward *= body.sneaking_speed;
    }
    travel(
        world,
        body,
        rules,
        st,
        input,
        strafe,
        forward,
        sin,
        cos,
        &mut scratch,
    );
}

/// The block below that affects the body's movement under [`Rules::Game`]
/// (`getBlockPosBelowThatAffectsMyMovement` = `getOnPos(0.500001)`).
fn game_below(p: [f64; 3]) -> [i32; 3] {
    [floor(p[0]), floor(p[1] - 0.500_001), floor(p[2])]
}

/// The pose's box height (`Player.updatePlayerPose`): the wanted pose if the
/// body fits it, else crouching, else crawling.
fn pose_height<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    st: &State,
    input: &Input,
    scratch: &mut Vec<Aabb>,
) -> f64 {
    let fits = |h: f64, scratch: &mut Vec<Aabb>| {
        let bb = player_box(st.pos, body.width, h).contract([1.0e-7, 1.0e-7, 1.0e-7]);
        surrounding(world, &bb, scratch);
        !scratch.iter().any(|b| b.intersects(&bb))
    };
    let wanted = if input.sneak {
        body.crouched_height
    } else {
        body.height
    };
    if fits(wanted, scratch) {
        wanted
    } else if fits(body.crouched_height, scratch) {
        body.crouched_height
    } else {
        body.crawling_height
    }
}

fn apply_heading(st: &mut State, strafe: f64, forward: f64, multiplier: f64, sin: f64, cos: f64) {
    let mut speed = (strafe * strafe + forward * forward).sqrt();
    if speed < 0.01 {
        return;
    }
    speed = multiplier / speed.max(1.0);
    let (s, f) = (strafe * speed, forward * speed);
    st.vel[0] -= s * cos + f * sin;
    st.vel[2] += f * cos - s * sin;
}

#[allow(clippy::too_many_arguments)]
fn travel<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    rules: Rules,
    st: &mut State,
    input: &Input,
    strafe: f64,
    forward: f64,
    sin: f64,
    cos: f64,
    scratch: &mut Scratch,
) {
    if st.in_water || st.in_lava {
        let last_y = st.pos[1];
        let water = st.in_water;
        let inertia = if water { 0.8 } else { 0.5 };
        let horizontal = match rules {
            Rules::Game if water && st.sprinting => 0.9,
            _ => inertia,
        };
        let accel = if water && body.water_movement_efficiency > 0.0 {
            0.02 + (body.movement_speed - 0.02) * body.water_movement_efficiency
        } else {
            0.02
        };
        apply_heading(st, strafe, forward, accel, sin, cos);
        let v = st.vel;
        move_entity(world, body, rules, st, input, v, scratch);
        st.vel[1] *= inertia;
        let sink = body.gravity / if water { 16.0 } else { 4.0 };
        match rules {
            Rules::Game if water && st.sprinting => {}
            Rules::Game if water => {
                let y = st.vel[1];
                st.vel[1] = if y <= 0.0 && (y - 0.005).abs() >= 0.003 && (y - sink).abs() < 0.003 {
                    -0.003
                } else {
                    y - sink
                };
            }
            _ => st.vel[1] -= sink,
        }
        st.vel[0] *= horizontal;
        st.vel[2] *= horizontal;
        if st.collided_h {
            let p = [
                st.pos[0] + st.vel[0],
                st.pos[1] + (st.vel[1] + 0.6 - st.pos[1] + last_y),
                st.pos[2] + st.vel[2],
            ];
            if does_not_collide(world, body, st.height, p, &mut scratch.boxes) {
                st.vel[1] = 0.3;
            }
        }
        return;
    }
    let under = block_at(world, [st.pos[0], st.pos[1] - 1.0, st.pos[2]]);
    let (accel, h_inertia, v_drag) = if st.on_ground {
        match rules {
            Rules::Bot => {
                // prismarine adds the sprint modifier as `y += y · amount`.
                let base = body.movement_speed;
                let speed = if st.sprinting {
                    base + base * body.sprint_multiplier
                } else {
                    base
                };
                let inertia = world.material(under).friction() * 0.91;
                let a = speed * (0.1627714 / (inertia * inertia * inertia));
                (a.max(0.0), inertia, f64::from(0.98f32))
            }
            Rules::Game => {
                // `AttributeInstance`: `ADD_MULTIPLIED_TOTAL` is `value · (1 + amount)`.
                let speed = body.movement_speed
                    * if st.sprinting {
                        1.0 + body.sprint_multiplier
                    } else {
                        1.0
                    };
                let f = world.material(game_below(st.pos)).friction() as f32;
                let a = (speed as f32) * (0.216_000_02_f32 / (f * f * f));
                (f64::from(a), f64::from(f * 0.91f32), f64::from(0.98f32))
            }
        }
    } else {
        match rules {
            Rules::Bot => {
                let a = 0.02 + if st.sprinting { 0.02 * 0.3 } else { 0.0 };
                (a, 0.91, f64::from(0.98f32))
            }
            Rules::Game => {
                let a = if st.sprinting {
                    0.025_999_999_f32
                } else {
                    0.02f32
                };
                (f64::from(a), f64::from(0.91f32), f64::from(0.98f32))
            }
        }
    };
    apply_heading(st, strafe, forward, accel, sin, cos);
    let climb_here = world.material(block_at(world, st.pos)) == Material::Climbable;
    if climb_here {
        st.vel[0] = st.vel[0].clamp(-0.15, 0.15);
        st.vel[2] = st.vel[2].clamp(-0.15, 0.15);
        st.vel[1] = st.vel[1].max(if input.sneak { 0.0 } else { -0.15 });
        st.fall = 0.0;
    }
    let v = st.vel;
    move_entity(world, body, rules, st, input, v, scratch);
    let climb_after = world.material(block_at(world, st.pos)) == Material::Climbable;
    if climb_after && (st.collided_h || input.jump) {
        st.vel[1] = 0.2;
    }
    st.vel[1] -= body.gravity;
    st.vel[1] *= v_drag;
    st.vel[0] *= h_inertia;
    st.vel[2] *= h_inertia;
}

fn does_not_collide<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    height: f64,
    p: [f64; 3],
    scratch: &mut Vec<Aabb>,
) -> bool {
    let bb = player_box(p, body.width, height);
    surrounding(world, &bb, scratch);
    !scratch.iter().any(|b| bb.intersects(b)) && water_in(world, &bb).is_empty()
}

/// The order the movement's axes are resolved in: `y` first, then `x` and
/// `z` — the bot always `x` then `z`, the jar the larger first
/// (`Direction.axisStepOrder`).
fn horizontal_order(rules: Rules, d: [f64; 3]) -> [usize; 2] {
    match rules {
        Rules::Game if d[0].abs() < d[2].abs() => [2, 0],
        _ => [0, 2],
    }
}

fn collide_axes(boxes: &[Aabb], bb: &mut Aabb, d: &mut [f64; 3], order: [usize; 2]) {
    for b in boxes {
        d[1] = b.offset_along(bb, 1, d[1]);
    }
    *bb = bb.offset([0.0, d[1], 0.0]);
    for axis in order {
        for b in boxes {
            d[axis] = b.offset_along(bb, axis, d[axis]);
        }
        let mut m = [0.0; 3];
        m[axis] = d[axis];
        *bb = bb.offset(m);
    }
}

/// `Entity.move` (prismarine's `moveEntity`): collision, the step-up, the
/// ground flag, the bounce and the blocks the body ends up in.
fn move_entity<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    rules: Rules,
    st: &mut State,
    input: &Input,
    mv: [f64; 3],
    scratch: &mut Scratch,
) {
    let [mut dx, mut dy, mut dz] = mv;
    if st.in_web {
        dx *= 0.25;
        dy *= 0.05;
        dz *= 0.25;
        st.vel = [0.0; 3];
        st.in_web = false;
        st.fall = 0.0;
    }
    let mut old_x = dx;
    let old_y = dy;
    let mut old_z = dz;
    let height = st.height;
    if input.sneak && st.on_ground {
        let step = 0.05;
        let clear = |dx: f64, dz: f64, scratch: &mut Vec<Aabb>| {
            surrounding(
                world,
                &player_box(st.pos, body.width, height).offset([dx, 0.0, dz]),
                scratch,
            );
            scratch.is_empty()
        };
        while dx != 0.0 && clear(dx, 0.0, &mut scratch.boxes) {
            if dx < step && dx >= -step {
                dx = 0.0;
            } else if dx > 0.0 {
                dx -= step;
            } else {
                dx += step;
            }
            old_x = dx;
        }
        while dz != 0.0 && clear(0.0, dz, &mut scratch.boxes) {
            if dz < step && dz >= -step {
                dz = 0.0;
            } else if dz > 0.0 {
                dz -= step;
            } else {
                dz += step;
            }
            old_z = dz;
        }
        while dx != 0.0 && dz != 0.0 && clear(dx, dz, &mut scratch.boxes) {
            if dx < step && dx >= -step {
                dx = 0.0;
            } else if dx > 0.0 {
                dx -= step;
            } else {
                dx += step;
            }
            if dz < step && dz >= -step {
                dz = 0.0;
            } else if dz > 0.0 {
                dz -= step;
            } else {
                dz += step;
            }
            old_x = dx;
            old_z = dz;
        }
    }
    let order = horizontal_order(rules, [dx, dy, dz]);
    let start = player_box(st.pos, body.width, height);
    let mut bb = start;
    let query = start.extend([dx, dy, dz]);
    surrounding(world, &query, &mut scratch.boxes);
    let mut d = [dx, dy, dz];
    collide_axes(&scratch.boxes, &mut bb, &mut d, order);
    [dx, dy, dz] = d;

    if body.step_height > 0.0
        && (st.on_ground || (dy != old_y && old_y < 0.0))
        && (dx != old_x || dz != old_z)
    {
        let (col_x, col_y, col_z, col_bb) = (dx, dy, dz, bb);
        let up = body.step_height;
        let q = start.extend([old_x, up, old_z]);
        let mut boxes = Vec::new();
        surrounding(world, &q, &mut boxes);
        let mut b1 = start;
        let mut b2 = start;
        let bxz = b1.extend([dx, 0.0, dz]);
        let (mut dy1, mut dy2) = (up, up);
        for b in &boxes {
            dy1 = b.offset_along(&bxz, 1, dy1);
            dy2 = b.offset_along(&b2, 1, dy2);
        }
        b1 = b1.offset([0.0, dy1, 0.0]);
        b2 = b2.offset([0.0, dy2, 0.0]);
        let mut d1 = [old_x, 0.0, old_z];
        let mut d2 = [old_x, 0.0, old_z];
        for axis in order {
            for b in &boxes {
                d1[axis] = b.offset_along(&b1, axis, d1[axis]);
                d2[axis] = b.offset_along(&b2, axis, d2[axis]);
            }
            let mut m1 = [0.0; 3];
            m1[axis] = d1[axis];
            b1 = b1.offset(m1);
            let mut m2 = [0.0; 3];
            m2[axis] = d2[axis];
            b2 = b2.offset(m2);
        }
        let n1 = d1[0] * d1[0] + d1[2] * d1[2];
        let n2 = d2[0] * d2[0] + d2[2] * d2[2];
        let (nx, ny, nz, mut nbb) = if n1 > n2 {
            (d1[0], -dy1, d1[2], b1)
        } else {
            (d2[0], -dy2, d2[2], b2)
        };
        let mut ny = ny;
        for b in &boxes {
            ny = b.offset_along(&nbb, 1, ny);
        }
        nbb = nbb.offset([0.0, ny, 0.0]);
        if col_x * col_x + col_z * col_z >= nx * nx + nz * nz {
            dx = col_x;
            dy = col_y;
            dz = col_z;
            bb = col_bb;
        } else {
            dx = nx;
            dy = ny;
            dz = nz;
            bb = nbb;
        }
    }

    let w = body.width / 2.0;
    st.pos = [bb.min[0] + w, bb.min[1], bb.min[2] + w];
    st.collided_h = dx != old_x || dz != old_z;
    st.collided_v = dy != old_y;
    let was_on_ground = st.on_ground;
    st.on_ground = st.collided_v && old_y < 0.0;
    let _ = was_on_ground;

    // The fall distance (`checkFallDamage`), spent on landing.
    st.landing_damage = 0.0;
    if st.on_ground {
        if st.fall > 0.0 {
            let material = world.material(block_at(world, [st.pos[0], st.pos[1] - 0.2, st.pos[2]]));
            let sneak_on_slime = material == Material::Slime && input.sneak;
            let m = if sneak_on_slime {
                Material::Plain
            } else {
                material
            };
            st.landing_damage = body.fall_damage(st.fall, m);
        }
        st.fall = 0.0;
    } else if dy < 0.0 {
        st.fall -= dy;
    }

    let feet = block_at(world, [st.pos[0], st.pos[1] - 0.2, st.pos[2]]);
    if dx != old_x {
        st.vel[0] = 0.0;
    }
    if dz != old_z {
        st.vel[2] = 0.0;
    }
    if dy != old_y {
        if world.material(feet) == Material::Slime && !input.sneak {
            st.vel[1] = -st.vel[1];
        } else {
            st.vel[1] = 0.0;
        }
    }
    if rules == Rules::Game {
        // The slime step (`SlimeBlock.stepOn`) and the block speed factor
        // (`Entity.getBlockSpeedFactor`), both applied after the move.
        let on = game_below(st.pos);
        if st.on_ground
            && world.material(on) == Material::Slime
            && st.vel[1].abs() < 0.1
            && !input.sneak
        {
            let k = 0.4 + st.vel[1].abs() * 0.2;
            st.vel[0] *= k;
            st.vel[2] *= k;
        }
        let speed = |m: Material| match m {
            Material::SoulSand | Material::Honey => 0.4,
            _ => 1.0,
        };
        let here = block_at(world, st.pos);
        let k = if world.fluid(here).is_none() && speed(world.material(here)) != 1.0 {
            speed(world.material(here))
        } else {
            speed(world.material(on))
        };
        st.vel[0] *= k;
        st.vel[2] *= k;
    }

    let inside = bb.contract([0.001, 0.001, 0.001]);
    for y in floor(inside.min[1])..=floor(inside.max[1]) {
        for z in floor(inside.min[2])..=floor(inside.max[2]) {
            for x in floor(inside.min[0])..=floor(inside.max[0]) {
                if world.material([x, y, z]) == Material::Cobweb {
                    st.in_web = true;
                }
            }
        }
    }
}

/// Run the body from `start`, one tick per input `input` yields, until it
/// yields `None`; every tick's resulting state, in order.
pub fn simulate<W: CollisionWorld + ?Sized>(
    world: &W,
    body: &Attributes,
    rules: Rules,
    start: State,
    mut input: impl FnMut(usize, &State) -> Option<Input>,
) -> Vec<State> {
    let mut st = start;
    let mut out = Vec::new();
    let mut t = 0usize;
    while let Some(i) = input(t, &st) {
        tick(world, body, rules, &mut st, &i);
        out.push(st);
        t += 1;
    }
    out
}

/// The steady velocity a body holding `input` reaches on an endless floor of
/// `material` — the "saturated" run-up of spec-0113 §4.3, taken from the
/// simulator's own approach rather than assumed: the body is run on a flat
/// floor until its velocity stops changing.
#[must_use]
pub fn saturated_velocity(
    body: &Attributes,
    rules: Rules,
    material: Material,
    input: &Input,
) -> [f64; 3] {
    struct Floor(Material);
    impl CollisionWorld for Floor {
        fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>) {
            if c[1] == -1 {
                out.push(Aabb::new(
                    f64::from(c[0]),
                    -1.0,
                    f64::from(c[2]),
                    f64::from(c[0]) + 1.0,
                    0.0,
                    f64::from(c[2]) + 1.0,
                ));
            }
        }
        fn material(&self, c: [i32; 3]) -> Material {
            if c[1] == -1 { self.0 } else { Material::Plain }
        }
        fn fluid(&self, _: [i32; 3]) -> Option<Fluid> {
            None
        }
    }
    let world = Floor(material);
    let mut st = State::standing([0.5, 0.0, 0.5], body);
    let mut held = *input;
    held.jump = false;
    let mut last = [f64::NAN; 3];
    for _ in 0..400 {
        tick(&world, body, rules, &mut st, &held);
        if st.vel == last {
            break;
        }
        last = st.vel;
    }
    st.vel
}

#[cfg(test)]
mod tests;
