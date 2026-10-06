//! **The rig: the parts and clips of an assembly** (spec-0082 §3.1).
//!
//! An assembly (`assemblies[]`, stage 5) is a thing built of display entities
//! that moves through authored clips. Its keyframes are thousands of numbers —
//! a procedural derivation, never a creative judgement — so they do not live in
//! campaign JSON. They live in a **rig file** beside the prefab library,
//! `<library>/rigs/<name>/rig.json`, written by a deterministic generator the
//! way a tileset generator writes `.nbt`, and referenced from the campaign as
//! `rig/<name>` exactly as a piece is referenced as `prefab/<name>`.
//!
//! This module is the one reader of that file and the one statement of what it
//! means:
//!
//! * [`Rig`] is the document, parsed with unknown keys refused.
//! * [`check`] is every structural rule (`DW0935`): part count, block ids
//!   against the pinned block registry (the `DW0193` rule), every clip
//!   non-empty with one transform per part per frame, a cadence in
//!   `1..=20`, a finite transform whose scale is not zero on any axis.
//! * [`frame_footprint`] / [`clip_footprint`] are **the** footprint arithmetic
//!   (§5.2): the cells a frame's parts occupy, as the compiler computes them
//!   from the transforms. `delvec rig describe` prints from it and the strike
//!   check judges from it — one symbol, so the number a creator is handed is
//!   the number the engine refuses against.
//! * [`Facing`]-rotation of a transform ([`Transform::faced`]) is applied here,
//!   once, for the emitter and the footprint alike.
//!
//! Determinism (ADR-0006): clips are a `BTreeMap`, footprints are `BTreeSet`s,
//! and nothing here reads a clock or a hash order.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::stages::Facing;

/// The only rig document version this engine reads.
pub const RIG_VERSION: u32 = 1;

/// The directory under a prefab library that holds rigs.
pub const RIGS_DIR: &str = "rigs";

/// The file inside `rigs/<name>/` that is the rig document.
pub const RIG_FILE: &str = "rig.json";

/// The slowest keyframe cadence a clip may declare, in ticks per frame.
///
/// A frame held a whole second is no longer one movement the client
/// interpolates; it is a pose. (Authored, spec-0082 §3.1.)
pub const MAX_TICKS_PER_FRAME: u32 = 20;

/// The fastest: one keyframe every tick.
pub const MIN_TICKS_PER_FRAME: u32 = 1;

/// One rig document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rig {
    /// The document version; [`RIG_VERSION`].
    pub rig_version: u32,
    /// One display entity each, in emission order.
    pub parts: Vec<RigPart>,
    /// Named clips. A frame is one transform per part, in [`Self::parts`]
    /// order, in the rig's own frame: origin at the assembly's mark (the cell's
    /// centre at the mark's floor plane), `+z` the rig's front.
    pub clips: BTreeMap<String, Clip>,
    /// Where the rig came from (ADR-0013's record, as a piece's metadata
    /// carries it).
    pub provenance: RigProvenance,
}

/// One part: one display entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigPart {
    /// The part's name, unique within the rig. Read by people and diagnostics.
    pub id: String,
    /// What kind of display the part is.
    pub kind: PartKind,
    /// The block state a `block` part shows (`minecraft:sculk`,
    /// `minecraft:oak_log[axis=x]`), checked against the pinned block registry.
    pub block: String,
    /// The part's rest pose, the transform it takes when no clip plays. Absent:
    /// the first frame of the rig's first clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<Transform>,
}

/// What kind of display entity a part is. `block` today; `item` is the next
/// kind and is not written (spec-0082 §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PartKind {
    /// A `minecraft:block_display` showing [`RigPart::block`].
    Block,
}

/// One clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    /// The keyframe cadence, and the `interpolation_duration` the compiler
    /// writes on every part (`1..=20`).
    pub ticks_per_frame: u32,
    /// Whether the clip starts over after its last frame. A clip that does not
    /// loop holds its last frame.
    #[serde(rename = "loop")]
    pub looping: bool,
    /// The keyframes: one transform per part per frame.
    pub frames: Vec<Vec<Transform>>,
}

impl Clip {
    /// How long the clip runs in ticks, from the tick its first frame is
    /// applied to the tick its last frame is applied: `(frames - 1) ×
    /// ticks_per_frame`, plus the one tick between a switch and its first frame.
    /// The number `delvec rig describe` prints and a `sequence` after the clip
    /// is timed by.
    pub fn length_ticks(&self) -> u32 {
        1 + (self.frames.len().saturating_sub(1) as u32) * self.ticks_per_frame
    }

    /// How long from the switch until a client has drawn the clip's last
    /// frame whole: [`Self::length_ticks`] plus one cadence, because each
    /// keyframe is drawn over `ticks_per_frame` ticks after it is applied. A
    /// strike's blow lands on this tick, so the limb the player sees is the
    /// last frame the strike check judges (spec-0082 §5.4).
    pub fn landing_ticks(&self) -> u32 {
        self.length_ticks() + self.ticks_per_frame
    }
}

/// A display entity's transformation (`Display` entity data, Minecraft Wiki):
/// the rendered model is `translation · left_rotation · scale · right_rotation`
/// applied to the unit cube.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    /// `[x, y, z]` translation, in blocks.
    pub translation: [f64; 3],
    /// `[x, y, z, w]` quaternion applied after scale.
    pub left_rotation: [f64; 4],
    /// `[x, y, z]` scale.
    pub scale: [f64; 3],
    /// `[x, y, z, w]` quaternion applied before scale.
    pub right_rotation: [f64; 4],
}

/// The rig's provenance record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigProvenance {
    /// The program that wrote the rig (`prefabs/rig-generator`), with its
    /// revision where it states one.
    pub generator: String,
    /// `original`, `cc0`, … — the source class ADR-0013 admits.
    pub source: String,
    /// The SPDX licence the rig is published under.
    pub spdx: String,
}

/// One structural refusal of a rig: the field it names and what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RigIssue {
    /// A JSON pointer into the rig document.
    pub field: String,
    /// What is wrong, in a sentence.
    pub message: String,
}

/// Parse a rig document, refusing unknown keys.
pub fn parse(raw: &str) -> Result<Rig, String> {
    serde_json::from_str::<Rig>(raw).map_err(|e| e.to_string())
}

/// Whether a clip name is one the engine can address: lowercase kebab, the
/// shape every id segment in the DSL takes.
pub fn is_clip_name(name: &str) -> bool {
    crate::ids::is_kebab(name)
}

/// **Every structural rule a rig obeys** (`DW0935`, spec-0082 §5.1), each
/// refusal naming its field. Empty for a rig the engine can emit.
pub fn check(rig: &Rig) -> Vec<RigIssue> {
    let mut out = Vec::new();
    let mut push = |field: String, message: String| out.push(RigIssue { field, message });
    if rig.rig_version != RIG_VERSION {
        push(
            "/rig_version".into(),
            format!(
                "`rig_version` is {}; this engine reads version {RIG_VERSION}",
                rig.rig_version
            ),
        );
    }
    if rig.parts.is_empty() {
        push(
            "/parts".into(),
            "the rig declares no part, so the assembly would show nothing".into(),
        );
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, p) in rig.parts.iter().enumerate() {
        if p.id.is_empty() {
            push(format!("/parts/{i}/id"), "a part's `id` is empty".into());
        } else if !seen.insert(p.id.as_str()) {
            push(
                format!("/parts/{i}/id"),
                format!("part id `{}` is declared twice", p.id),
            );
        }
        if let Err(e) = crate::blocks::BlockRegistry::v1_21_11().validate_state_string(&p.block) {
            push(
                format!("/parts/{i}/block"),
                format!(
                    "part `{}` shows `{}`, which is not a block state of Minecraft Java \
                     1.21.11 ({e:?})",
                    p.id, p.block
                ),
            );
        }
        if let Some(t) = &p.rest {
            for m in transform_issues(t) {
                push(format!("/parts/{i}/rest"), m);
            }
        }
    }
    if rig.clips.is_empty() {
        push(
            "/clips".into(),
            "the rig declares no clip, so the assembly has nothing to play and no pose to \
             stand in"
                .into(),
        );
    }
    for (name, clip) in &rig.clips {
        let at = format!("/clips/{name}");
        if !is_clip_name(name) {
            push(
                at.clone(),
                format!(
                    "clip name `{name}` is not lowercase kebab-case (`[a-z0-9]` segments joined by `-`)"
                ),
            );
        }
        if !(MIN_TICKS_PER_FRAME..=MAX_TICKS_PER_FRAME).contains(&clip.ticks_per_frame) {
            push(
                format!("{at}/ticks_per_frame"),
                format!(
                    "clip `{name}` declares `ticks_per_frame` {}; the cadence is \
                     {MIN_TICKS_PER_FRAME}..={MAX_TICKS_PER_FRAME} ticks per frame",
                    clip.ticks_per_frame
                ),
            );
        }
        if clip.frames.is_empty() {
            push(
                format!("{at}/frames"),
                format!("clip `{name}` has no frame"),
            );
        }
        for (f, frame) in clip.frames.iter().enumerate() {
            if frame.len() != rig.parts.len() {
                push(
                    format!("{at}/frames/{f}"),
                    format!(
                        "clip `{name}` frame {f} carries {} transform(s) and the rig has {} \
                         part(s); a frame is one transform per part",
                        frame.len(),
                        rig.parts.len()
                    ),
                );
            }
            for (p, t) in frame.iter().enumerate() {
                for m in transform_issues(t) {
                    push(format!("{at}/frames/{f}/{p}"), m);
                }
            }
        }
    }
    out
}

/// What is wrong with one transform: a non-finite number, a scale of zero on
/// an axis (the part would vanish into a plane), or a quaternion of zero
/// length (no rotation at all, not the identity).
fn transform_issues(t: &Transform) -> Vec<String> {
    let mut out = Vec::new();
    let all = t
        .translation
        .iter()
        .chain(t.left_rotation.iter())
        .chain(t.scale.iter())
        .chain(t.right_rotation.iter());
    if all.clone().any(|v| !v.is_finite()) {
        out.push("a transform carries a value that is not a finite number".to_string());
        return out;
    }
    for (axis, v) in ["x", "y", "z"].iter().zip(t.scale) {
        if v == 0.0 {
            out.push(format!(
                "`scale` is 0 on {axis}: the part collapses to a plane and shows nothing"
            ));
        }
    }
    for (name, q) in [
        ("left_rotation", t.left_rotation),
        ("right_rotation", t.right_rotation),
    ] {
        if q.iter().map(|v| v * v).sum::<f64>() < 1e-12 {
            out.push(format!("`{name}` is a quaternion of zero length"));
        }
    }
    out
}

impl Rig {
    /// The clip names, in the order the compiler numbers them.
    pub fn clip_names(&self) -> Vec<&str> {
        self.clips.keys().map(String::as_str).collect()
    }

    /// The compiler's index for a clip: its position in name order.
    pub fn clip_index(&self, name: &str) -> Option<usize> {
        self.clips.keys().position(|k| k == name)
    }

    /// The rest pose: each part's `rest`, else the first frame of the first
    /// clip. `None` only for a rig [`check`] refuses.
    pub fn rest_pose(&self) -> Option<Vec<Transform>> {
        let first = self.clips.values().next().and_then(|c| c.frames.first());
        self.parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                p.rest
                    .clone()
                    .or_else(|| first.and_then(|f| f.get(i).cloned()))
            })
            .collect()
    }

    /// Total frames across every clip — the count of frame functions the
    /// compiler emits for this rig.
    pub fn frame_count(&self) -> usize {
        self.clips.values().map(|c| c.frames.len()).sum()
    }
}

impl RigPart {
    /// The part's block state as `block_display` NBT: `{Name:"…"}` or
    /// `{Name:"…",Properties:{k:"v",…}}`.
    pub fn block_state_snbt(&self) -> String {
        let (name, props) = crate::blocks::parse_state(&self.block);
        let name = if name.contains(':') {
            name.to_string()
        } else {
            format!("minecraft:{name}")
        };
        if props.is_empty() {
            format!("{{Name:\"{name}\"}}")
        } else {
            let p: Vec<String> = props.iter().map(|(k, v)| format!("{k}:\"{v}\"")).collect();
            format!("{{Name:\"{name}\",Properties:{{{}}}}}", p.join(","))
        }
    }
}

// ---------------------------------------------------------------------------
// Facing and the transform arithmetic
// ---------------------------------------------------------------------------

/// The yaw a facing turns the rig's `+z` front to, as the angle of a rotation
/// about `+y` (right-handed: `+z` turns toward `+x`). `south` is the identity.
pub fn facing_angle(f: Facing) -> f64 {
    match f {
        Facing::South => 0.0,
        Facing::East => std::f64::consts::FRAC_PI_2,
        Facing::North => std::f64::consts::PI,
        Facing::West => -std::f64::consts::FRAC_PI_2,
    }
}

/// Hamilton product `a · b` of `[x, y, z, w]` quaternions.
fn qmul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// Rotate `v` by the unit quaternion `q`.
fn qrot(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let n = q.iter().map(|c| c * c).sum::<f64>().sqrt();
    let q = [q[0] / n, q[1] / n, q[2] / n, q[3] / n];
    let p = qmul(
        qmul(q, [v[0], v[1], v[2], 0.0]),
        [-q[0], -q[1], -q[2], q[3]],
    );
    [p[0], p[1], p[2]]
}

/// Snap a value within a billionth of an integer to it, and `-0.0` to `0.0`,
/// so a quarter turn of an exact number stays exact and two machines format it
/// identically.
fn tidy(v: f64) -> f64 {
    let r = v.round();
    let v = if (v - r).abs() < 1e-9 { r } else { v };
    if v == 0.0 { 0.0 } else { v }
}

impl Transform {
    /// This transform turned by `facing` about the vertical axis through the
    /// mark: the translation rotated, the yaw composed before the left
    /// rotation. The model is `Ryaw · T · L · S · R`, which is
    /// `T(Ryaw·t) · (Ryaw·L) · S · R` — so the emitted entity stands at yaw 0
    /// and no client fact about how a display's own yaw composes with its
    /// transformation is relied on (spec-0082 §3.2).
    pub fn faced(&self, facing: Facing) -> Transform {
        self.turned(facing_angle(facing))
    }

    /// This transform turned by `a` radians about the vertical axis through the
    /// mark (right-handed about `+y`: `+z` turns toward `+x`) — what
    /// [`Self::faced`] does for a quarter turn, for any angle. An aimed
    /// assembly's facings are turns of this kind (spec-0082 §5.5).
    pub fn turned(&self, a: f64) -> Transform {
        if a == 0.0 {
            return self.clone();
        }
        let yaw = [0.0, (a / 2.0).sin(), 0.0, (a / 2.0).cos()];
        let t = qrot(yaw, self.translation);
        let l = qmul(yaw, self.left_rotation);
        Transform {
            translation: t.map(tidy),
            left_rotation: l.map(tidy),
            scale: self.scale,
            right_rotation: self.right_rotation,
        }
    }

    /// The model's eight corners, relative to the entity's position.
    fn corners(&self) -> [[f64; 3]; 8] {
        let mut out = [[0.0; 3]; 8];
        for (k, c) in out.iter_mut().enumerate() {
            let unit = [(k & 1) as f64, ((k >> 1) & 1) as f64, ((k >> 2) & 1) as f64];
            let r = qrot(self.right_rotation, unit);
            let s = [
                r[0] * self.scale[0],
                r[1] * self.scale[1],
                r[2] * self.scale[2],
            ];
            let l = qrot(self.left_rotation, s);
            *c = [
                l[0] + self.translation[0],
                l[1] + self.translation[1],
                l[2] + self.translation[2],
            ];
        }
        out
    }

    /// The cells this part's box meets, relative to the mark's cell: every
    /// cell the transformed unit cube overlaps with positive volume, judged
    /// exactly (a separating-axis test of the oriented box against the cell),
    /// not by the box's axis-aligned hull — a part laid on a diagonal meets the
    /// cells along it, never the empty corners of its hull. The entity stands
    /// at the mark cell's centre (`x + 0.5`, `z + 0.5`) on its floor (`y`).
    pub fn cells(&self) -> BTreeSet<[i32; 3]> {
        // The entity's own position inside the mark cell.
        let origin = [0.5, 0.0, 0.5];
        let corners = self
            .corners()
            .map(|c| [c[0] + origin[0], c[1] + origin[1], c[2] + origin[2]]);
        let mut lo = [f64::INFINITY; 3];
        let mut hi = [f64::NEG_INFINITY; 3];
        for c in corners {
            for i in 0..3 {
                lo[i] = lo[i].min(c[i]);
                hi[i] = hi[i].max(c[i]);
            }
        }
        let span = |i: usize| -> (i32, i32) {
            let from = (lo[i] + CELL_EPS).floor() as i32;
            let to = (hi[i] - CELL_EPS).ceil() as i32 - 1;
            (from, to.max(from))
        };
        let (x0, x1) = span(0);
        let (y0, y1) = span(1);
        let (z0, z1) = span(2);
        let axes = separating_axes(&corners);
        let mut out = BTreeSet::new();
        for x in x0..=x1 {
            for y in y0..=y1 {
                for z in z0..=z1 {
                    if box_meets_cell(&corners, &axes, [x, y, z]) {
                        out.insert([x, y, z]);
                    }
                }
            }
        }
        out
    }
}

/// How far a box must reach into a cell to meet it: a billionth of a block,
/// so a face lying on a cell boundary meets neither side, and two machines whose
/// sines differ in the last bit decide every cell the same way.
const CELL_EPS: f64 = 1e-9;

/// The candidate separating axes of a parallelepiped against an axis-aligned
/// cell: the three world axes, its three edge directions and their nine cross
/// products.
fn separating_axes(c: &[[f64; 3]; 8]) -> Vec<[f64; 3]> {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let edges = [sub(c[1], c[0]), sub(c[2], c[0]), sub(c[4], c[0])];
    let world = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut axes: Vec<[f64; 3]> = world.to_vec();
    let mut push = |v: [f64; 3]| {
        let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if n > 1e-12 {
            axes.push([v[0] / n, v[1] / n, v[2] / n]);
        }
    };
    for e in edges {
        push(e);
    }
    for a in world {
        for e in edges {
            push([
                a[1] * e[2] - a[2] * e[1],
                a[2] * e[0] - a[0] * e[2],
                a[0] * e[1] - a[1] * e[0],
            ]);
        }
    }
    axes
}

/// Whether the parallelepiped `c` overlaps the unit cell at `cell` with
/// positive volume: no candidate axis separates them, an overlap thinner than
/// [`CELL_EPS`] counting as none.
fn box_meets_cell(c: &[[f64; 3]; 8], axes: &[[f64; 3]], cell: [i32; 3]) -> bool {
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    axes.iter().all(|a| {
        let (mut p0, mut p1) = (f64::INFINITY, f64::NEG_INFINITY);
        for v in c {
            let d = dot(*v, *a);
            p0 = p0.min(d);
            p1 = p1.max(d);
        }
        let (mut q0, mut q1) = (f64::INFINITY, f64::NEG_INFINITY);
        for k in 0..8 {
            let v = [
                f64::from(cell[0] + (k & 1)),
                f64::from(cell[1] + ((k >> 1) & 1)),
                f64::from(cell[2] + ((k >> 2) & 1)),
            ];
            let d = dot(v, *a);
            q0 = q0.min(d);
            q1 = q1.max(d);
        }
        p1 > q0 + CELL_EPS && q1 > p0 + CELL_EPS
    })
}

/// **The frame footprint** (spec-0082 §5.2): the union over parts of the cells
/// each part's box meets, relative to the mark's cell, after `facing`.
///
/// The one footprint function. `delvec rig describe` prints it and the strike
/// check (`DW0938`) judges by it, so the region a creator declares from the
/// printed cells is the region the engine measures.
pub fn frame_footprint(frame: &[Transform], facing: Facing) -> BTreeSet<[i32; 3]> {
    frame_footprint_turned(frame, facing_angle(facing))
}

/// The frame footprint with the rig turned `a` radians about the mark's
/// vertical axis ([`Transform::turned`]) — the one footprint function;
/// [`frame_footprint`] is it at a quarter turn.
pub fn frame_footprint_turned(frame: &[Transform], a: f64) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for t in frame {
        out.extend(t.turned(a).cells());
    }
    out
}

/// The clip footprint: the union over frames of [`frame_footprint`].
pub fn clip_footprint(clip: &Clip, facing: Facing) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for f in &clip.frames {
        out.extend(frame_footprint(f, facing));
    }
    out
}

/// The footprint of a clip's **last** frame — the pose a non-looping clip
/// holds, and for a strike clip the pose the blow lands in.
pub fn last_frame_footprint(clip: &Clip, facing: Facing) -> BTreeSet<[i32; 3]> {
    clip.frames
        .last()
        .map(|f| frame_footprint(f, facing))
        .unwrap_or_default()
}

/// A footprint as one line of cells, `[x, y, z]` relative to the mark, in
/// cell order — what `delvec rig describe` prints and a refusal quotes.
pub fn cells_line(cells: &BTreeSet<[i32; 3]>) -> String {
    cells
        .iter()
        .map(|c| format!("[{}, {}, {}]", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(" ")
}

/// **What `delvec rig describe` prints** (spec-0082 §3.1): the part count,
/// every clip with its length in ticks, and per clip the footprint of its last
/// frame relative to the mark at `facing`. Deterministic: two runs over one
/// rig are byte-identical.
pub fn describe(id: &str, rig: &Rig, facing: Facing) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "rig {id}: {} part(s), {} clip(s), {} frame(s) in all; facing {}; provenance: {} ({}, {})\n",
        rig.parts.len(),
        rig.clips.len(),
        rig.frame_count(),
        facing.token(),
        rig.provenance.generator,
        rig.provenance.source,
        rig.provenance.spdx,
    ));
    for (name, clip) in &rig.clips {
        let last = last_frame_footprint(clip, facing);
        out.push_str(&format!(
            "clip {name}: {} frame(s) every {} tick(s), {} tick(s) from switch to last frame, {}\n",
            clip.frames.len(),
            clip.ticks_per_frame,
            clip.length_ticks(),
            if clip.looping {
                "loops"
            } else {
                "holds its last frame"
            },
        ));
        out.push_str(&format!(
            "  last-frame footprint, {} cell(s) relative to the mark: {}\n",
            last.len(),
            cells_line(&last)
        ));
    }
    out
}

/// What a library knows about one rig id.
#[derive(Clone, Copy, Debug)]
pub enum RigLookup<'a> {
    /// The registry asked is not the whole library and cannot vouch either way
    /// (a test double, the DSL's vendored registry). Nothing is refused on its
    /// word.
    Unknown,
    /// The library holds no `rigs/<name>/rig.json`.
    Missing,
    /// The file is there and does not parse; the parse error.
    Malformed(&'a str),
    /// The rig.
    Found(&'a Rig),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> Transform {
        Transform {
            translation: [0.0, 0.0, 0.0],
            left_rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 1.0, 1.0],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        }
    }

    fn rig(frames: Vec<Vec<Transform>>, tpf: u32) -> Rig {
        let mut clips = BTreeMap::new();
        clips.insert(
            "idle".to_string(),
            Clip {
                ticks_per_frame: tpf,
                looping: true,
                frames,
            },
        );
        Rig {
            rig_version: RIG_VERSION,
            parts: vec![RigPart {
                id: "a".into(),
                kind: PartKind::Block,
                block: "minecraft:stone".into(),
                rest: None,
            }],
            clips,
            provenance: RigProvenance {
                generator: "test".into(),
                source: "original".into(),
                spdx: "GPL-3.0-or-later".into(),
            },
        }
    }

    /// The unit cube placed at the entity's position (the mark cell's centre)
    /// spans half of the mark cell and half of the cells beside it on x and z.
    #[test]
    fn the_unit_cube_at_the_origin_meets_four_cells() {
        let cells = unit().cells();
        assert_eq!(
            cells,
            [[0, 0, 0], [0, 0, 1], [1, 0, 0], [1, 0, 1]]
                .into_iter()
                .collect()
        );
    }

    /// Translated back by half a block on x and z, the unit cube is exactly
    /// the mark cell.
    #[test]
    fn a_centred_cube_is_the_mark_cell() {
        let mut t = unit();
        t.translation = [-0.5, 0.0, -0.5];
        assert_eq!(t.cells(), [[0, 0, 0]].into_iter().collect());
    }

    /// Scaled to three blocks tall and turned a quarter about z, a centred
    /// column lies along -x: its cells are the three cells west of the mark
    /// at floor height (the rotation lays it down, the scale lengthens it).
    #[test]
    fn a_diagonal_part_meets_the_cells_along_it_not_its_hull() {
        // A bar 0.2 thick and 4.24 long, turned 45 degrees about y, laid from
        // the mark cell's centre toward +x +z: its hull spans a 4 x 4 square of
        // columns; the bar itself crosses only the cells along the diagonal and
        // the ones its edge clips beside them.
        let a = std::f64::consts::FRAC_PI_4;
        let t = Transform {
            translation: [0.0, 0.0, 0.0],
            left_rotation: [0.0, (a / 2.0).sin(), 0.0, (a / 2.0).cos()],
            scale: [0.2, 1.0, 4.24],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let cells = t.cells();
        assert!(
            cells.contains(&[0, 0, 0]) && cells.contains(&[2, 0, 2]),
            "{cells:?}"
        );
        assert!(
            !cells.contains(&[0, 0, 3]) && !cells.contains(&[3, 0, 0]),
            "{cells:?}"
        );
        assert!(cells.len() < 16, "{} cells: {cells:?}", cells.len());
    }

    #[test]
    fn rotation_and_scale_move_the_cell_set() {
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let t = Transform {
            translation: [-0.5, 0.0, -0.5],
            // +90° about z: +y turns toward -x.
            left_rotation: [0.0, 0.0, s, s],
            scale: [1.0, 3.0, 1.0],
            right_rotation: [0.0, 0.0, 0.0, 1.0],
        };
        assert_eq!(
            t.cells(),
            [[-3, 0, 0], [-2, 0, 0], [-1, 0, 0]].into_iter().collect()
        );
    }

    /// Facing north turns a part standing two cells in front of the mark
    /// (+z) to two cells behind it (-z); east turns it to +x.
    #[test]
    fn facing_turns_the_footprint_about_the_mark() {
        let mut t = unit();
        t.translation = [-0.5, 0.0, 1.5];
        assert_eq!(
            frame_footprint(std::slice::from_ref(&t), Facing::South),
            [[0, 0, 2]].into_iter().collect()
        );
        assert_eq!(
            frame_footprint(std::slice::from_ref(&t), Facing::North),
            [[0, 0, -2]].into_iter().collect()
        );
        assert_eq!(
            frame_footprint(std::slice::from_ref(&t), Facing::East),
            [[2, 0, 0]].into_iter().collect()
        );
        assert_eq!(
            frame_footprint(std::slice::from_ref(&t), Facing::West),
            [[-2, 0, 0]].into_iter().collect()
        );
    }

    #[test]
    fn a_well_formed_rig_has_no_issue() {
        assert!(check(&rig(vec![vec![unit()]], 5)).is_empty());
    }

    #[test]
    fn each_structural_defect_names_its_field() {
        let short = rig(vec![vec![]], 5);
        assert!(
            check(&short)
                .iter()
                .any(|i| i.field == "/clips/idle/frames/0")
        );
        for tpf in [0, 21] {
            let r = rig(vec![vec![unit()]], tpf);
            assert!(
                check(&r)
                    .iter()
                    .any(|i| i.field == "/clips/idle/ticks_per_frame"),
                "{tpf}"
            );
        }
        let mut z = unit();
        z.scale = [1.0, 0.0, 1.0];
        assert!(
            check(&rig(vec![vec![z]], 5))
                .iter()
                .any(|i| i.field == "/clips/idle/frames/0/0" && i.message.contains("`scale` is 0"))
        );
        let mut bad = rig(vec![vec![unit()]], 5);
        bad.parts[0].block = "minecraft:no_such_block".into();
        assert!(check(&bad).iter().any(|i| i.field == "/parts/0/block"));
    }

    #[test]
    fn block_state_snbt_carries_properties() {
        let mut r = rig(vec![vec![unit()]], 5);
        assert_eq!(r.parts[0].block_state_snbt(), "{Name:\"minecraft:stone\"}");
        r.parts[0].block = "minecraft:oak_log[axis=x]".into();
        assert_eq!(
            r.parts[0].block_state_snbt(),
            "{Name:\"minecraft:oak_log\",Properties:{axis:\"x\"}}"
        );
    }

    #[test]
    fn describe_is_deterministic() {
        let r = rig(vec![vec![unit()], vec![unit()]], 5);
        assert_eq!(
            describe("rig/x", &r, Facing::South),
            describe("rig/x", &r, Facing::South)
        );
        assert!(describe("rig/x", &r, Facing::South).contains("6 tick(s)"));
    }
}
