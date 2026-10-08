//! Stage 5 — assemblies (spec-0082): a fixed thing that can be hit and hits back.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_zero3;
use crate::{AssemblyId, Facing, Mark, QuestEffect, RigId, StealthZone};

/// A fixed thing that can be hit and hits back (spec-0082): an object built of
/// display entities standing at a [`Mark`], moving through the clips of a
/// library [`rig`](crate::rig), struck in melee through an optional
/// `minecraft:interaction` hitbox, and striking a player who stands in its
/// arming region.
///
/// It is not a fight class and never dies: no health, equipment, traversal,
/// health bar or kill credit. A hit count is an ordinary `state` datum a
/// `strike-assembly` trigger adds to, and what happens at a count is an effect
/// behind the ordinary gate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Assembly {
    /// Unique assembly id (`assembly/<kebab>`).
    pub id: AssemblyId,
    /// The library rig (`rig/<name>`, resolved to `rigs/<name>/rig.json`
    /// beside the prefab library) whose parts and clips this assembly is.
    pub rig: RigId,
    /// Where the rig's origin stands: an anchor and an optional offset. The
    /// rig's origin is the mark cell's centre at its floor plane.
    pub at: Mark,
    /// Which way the rig's `+z` front faces (default `south`). Applied by the
    /// compiler to every frame, so the emitted entities stand at yaw 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// The clip playing from spawn. Absent: the parts stand in the rig's rest
    /// pose and no clip plays until a `play-clip`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
    /// The `minecraft:interaction` a player strikes. Absent: the assembly
    /// cannot be struck, and a `strike-assembly` on it is refused (`DW0936`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<AssemblyHitbox>,
    /// The blows it deals. Absent: it never strikes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikes: Option<AssemblyStrikes>,
}

impl Assembly {
    /// The declared facing, `south` when absent.
    pub fn facing(&self) -> Facing {
        self.facing.unwrap_or(Facing::South)
    }
}

/// An assembly's hitbox (spec-0082 §3.2): a `minecraft:interaction` of
/// `width × height` whose bottom centre is the mark's cell centre plus
/// `offset`. Melee only: an arrow passes through an interaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssemblyHitbox {
    /// Width in blocks (`0 < width <= 6`, `DW0936`).
    pub width: f64,
    /// Height in blocks (`0 < height <= 22`, `DW0936`).
    pub height: f64,
    /// Integer `[x, y, z]` block offset of the box's bottom centre from the
    /// mark (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// An assembly's strike pattern (spec-0082 §3.2).
///
/// The pattern runs, repeating from its first step, on every tick on which
/// some player's body is in `while_in`, and stops at the end of the step in
/// flight when nobody is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssemblyStrikes {
    /// The arming region: an anchor-centred box. Every landing box lies inside
    /// it (`DW0938`), so a player who never entered it is never struck.
    pub while_in: StealthZone,
    /// The steps, in order.
    pub pattern: Vec<StrikeStep>,
    /// Aim (spec-0082 §5.7). Absent: every blow lands where its `on_land`
    /// boxes say. Present: at the start of every wind-up the assembly turns to
    /// the one of its declared facings nearest the bearing of the nearest
    /// player in `while_in`, and every `damage-players` box in `on_land` turns
    /// with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aim: Option<StrikeAim>,
}

/// An aimed strike pattern's facings (spec-0082 §5.7): `facings` turns
/// evenly spaced round the vertical axis through the mark, the first being the
/// assembly's declared `facing`. Every `on_land` box is written for that first
/// facing; the compiler turns it to each of the others and proves every facing
/// a player in `while_in` can draw (`DW0938`, judged per facing). Which facing
/// a blow takes is chosen at run time among the proven ones; nothing about
/// where it lands is computed there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeAim {
    /// How many facings, evenly spaced: 4 is a quarter turn apart, 8 an eighth,
    /// 16 a sixteenth. At least 1 (1 is the declared facing alone).
    pub facings: std::num::NonZeroU32,
}

/// One step of a strike pattern: wind up, hold, strike, land.
///
/// How long the wind-up is and how hard the blow lands are the creator's
/// judgement, by spec-0016's standing ruling: no telegraph rule and no
/// one-shot rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeStep {
    /// The clip played first.
    pub windup: String,
    /// Ticks the windup's last frame is held before the strike clip starts.
    pub hold: u32,
    /// The clip the blow is.
    pub strike: String,
    /// The step's pace: the keyframe cadence, in ticks per frame (1–20, the
    /// rig's own bounds), its wind-up and strike clips play at. Absent: each
    /// clip's own. The wind-up lasts `1 + (frames - 1) × cadence` ticks, then
    /// `hold`; the blow lands one cadence after the strike's last frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks_per_frame: Option<u32>,
    /// A lock (spec-0094): at the start of the wind-up the step picks one
    /// player in `lock.within` by `lock.pick`, reads the cell their feet stand
    /// in, turns the assembly to it and strikes it with whichever of `strike`
    /// and `lock.reaches` the compiler proved comes down there. The blow's area
    /// is then the cells that clip comes down on, derived by the compiler: a
    /// `damage-players` in a locked step declares no `in` (`DW0969`). Absent:
    /// the blow lands where its `on_land` boxes say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<StrikeLock>,
    /// Effects run, with no acting player, on the tick a client has drawn the
    /// strike clip's last frame whole (one cadence after it is applied). A step
    /// with none is a feint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_land: Vec<QuestEffect>,
}

/// A strike step's lock (spec-0094 §3.1): the step strikes where one player
/// stands, chosen when its wind-up begins.
///
/// A display entity cannot bend live to a point: its pose is a keyframe a rig
/// precomputed. So a lock is two run-time choices among things the compiler
/// proved — a **turn** of the whole assembly about its mark (any yaw: a `tp` of
/// the root turns every riding part with it), and a **pose**, the first of
/// `strike` then `reaches` whose last frame, at that turn, comes down on the
/// locked cell with its whole blow inside `while_in`. Every standable cell of
/// `within` owes such a pose, or the build refuses naming the cells (`DW0968`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeLock {
    /// The region a target is chosen in: an anchor-centred box. The step winds
    /// up only while some player's body is in it, and every blow it can deal
    /// lies inside `while_in` (`DW0968`).
    pub within: StealthZone,
    /// Which player in `within` the step locks onto.
    pub pick: LockPick,
    /// Further strike clips, beyond the step's `strike`, the lock may choose
    /// among — a limb's blows at other reaches. Tried after `strike`, in the
    /// order written; the first that comes down on the locked cell is played.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaches: Vec<String>,
}

/// Which player a locked strike chooses (spec-0094 §3.1): vanilla's own
/// selector orders, measured from the assembly's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LockPick {
    /// The player nearest the mark (`sort=nearest`).
    Nearest,
    /// The player furthest from the mark (`sort=furthest`).
    Furthest,
    /// A player chosen at random (`sort=random`).
    Random,
}

impl LockPick {
    /// The `sort=` value of the selector that makes the choice.
    pub fn sort(self) -> &'static str {
        match self {
            LockPick::Nearest => "nearest",
            LockPick::Furthest => "furthest",
            LockPick::Random => "random",
        }
    }
}
