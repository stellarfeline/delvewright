//! **What solid the viewer is standing inside** — the cutaway, as a property of
//! the shot.
//!
//! A cutaway is a set of axis-aligned **half-space clips** over the model's own
//! bounding box. Each clip names a face and how far in from that face the
//! material is taken away; a cell is meshed unless some clip removes it. The
//! kept set is therefore the intersection of the complements — an axis-aligned
//! box — which is what makes "is anything left to look at" an exact question
//! rather than a scan ([`Cutaway::kept_box`]).
//!
//! # Why this shape
//!
//! The renderer used to carry a single `strip_ceiling: bool`: take away the top
//! Y layer. That is a dollhouse roof-off, and it is right for a small roofed
//! room — but it is *one configuration* of the question, hard-coded as the only
//! one. In a zone ten to fourteen courses tall carved out of solid mass the
//! layer under the roof is more rock, so the picture showed the bounding box and
//! never the massing (measured, PR #372: interior-wall changes moved 0–0.3% of
//! the frame).
//!
//! So the boolean is not accompanied by a second knob, it is **replaced**:
//! `strip_ceiling == true` is exactly `Cutaway` = `y-max:1`, the degenerate case
//! of the general form. The six faces and the two depth units span the shots a
//! reviewer actually asks for:
//!
//! | intent | spec |
//! |---|---|
//! | dollhouse: take the roof off | `y-max:1` |
//! | plan section: cut the mass at mid height and look down | `y-max:50%` |
//! | elevation section: halve the building and look at the cut face | `z-min:50%` |
//! | corner dollhouse | `x-min:50%+y-max:2` |
//! | show what is under a floor | `y-min:4` |
//!
//! Nothing here knows about roofs, wards, bells or Minecraft courses: a creator
//! building a spaceship, an office block or an anthill asks the same question of
//! the same surface and answers it in their own fiction.
//!
//! # What was deliberately not built
//!
//! - **Oblique (non-axis-aligned) clip planes.** The models are axis-aligned
//!   voxel boxes and every planned camera is axis-oriented; an oblique plane
//!   buys no view these cannot express and costs a float comparison per cell.
//! - **"Hide whatever occludes this anchor."** That is a *policy for choosing*
//!   clips, not a fourth mechanism — a planner that knows the anchor computes a
//!   [`Cutaway`] for it, which is exactly what [`crate::shots`] does for anchor
//!   and doorway shots. Built as its own primitive it would have been a private
//!   re-implementation of this one.

use std::fmt;
use std::str::FromStr;

/// A face of the model's bounding box: the side a clip is measured in from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Face {
    /// The low-X face.
    XMin,
    /// The high-X face.
    XMax,
    /// The low-Y face (the floor).
    YMin,
    /// The high-Y face (the ceiling).
    YMax,
    /// The low-Z face.
    ZMin,
    /// The high-Z face.
    ZMax,
}

impl Face {
    /// The axis this face is perpendicular to (0 = X, 1 = Y, 2 = Z).
    pub fn axis(self) -> usize {
        match self {
            Face::XMin | Face::XMax => 0,
            Face::YMin | Face::YMax => 1,
            Face::ZMin | Face::ZMax => 2,
        }
    }

    /// True for the `Max` side of its axis.
    pub fn is_max(self) -> bool {
        matches!(self, Face::XMax | Face::YMax | Face::ZMax)
    }

    /// Spec-string name (`x-min`, `y-max`, …).
    pub fn as_str(self) -> &'static str {
        match self {
            Face::XMin => "x-min",
            Face::XMax => "x-max",
            Face::YMin => "y-min",
            Face::YMax => "y-max",
            Face::ZMin => "z-min",
            Face::ZMax => "z-max",
        }
    }
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Face {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "x-min" => Face::XMin,
            "x-max" => Face::XMax,
            "y-min" => Face::YMin,
            "y-max" => Face::YMax,
            "z-min" => Face::ZMin,
            "z-max" => Face::ZMax,
            other => {
                return Err(format!(
                    "unknown cutaway face `{other}` (expected one of \
                     x-min, x-max, y-min, y-max, z-min, z-max)"
                ));
            }
        })
    }
}

/// How deep a clip cuts, measured inward from its face.
///
/// Two units, because a shot has two genuinely different intents: "take the
/// roof off" is a fixed number of layers and must not change with the model,
/// while "halve it" is a proportion and must, so that one shot plan is
/// meaningful across a library of pieces of different sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// A fixed count of block layers.
    Layers(u32),
    /// A percentage of the model's extent along the clip's axis, floored
    /// (integer arithmetic — no float, per ADR-0006).
    Percent(u32),
}

impl Depth {
    /// This depth in layers, against an axis `extent`. Never negative; a
    /// percentage over 100 saturates at the extent.
    pub fn layers(self, extent: i32) -> i32 {
        let extent = extent.max(0);
        match self {
            Depth::Layers(n) => i32::try_from(n).unwrap_or(i32::MAX).min(extent),
            Depth::Percent(p) => {
                let p = i64::from(p.min(100));
                ((i64::from(extent) * p) / 100) as i32
            }
        }
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Depth::Layers(n) => write!(f, "{n}"),
            Depth::Percent(p) => write!(f, "{p}%"),
        }
    }
}

/// One half-space clip: take `depth` away, measured in from `face`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    /// The face the depth is measured from.
    pub face: Face,
    /// How far in the material is taken away.
    pub depth: Depth,
}

impl Clip {
    /// A clip of `depth` layers in from `face`.
    pub fn layers(face: Face, n: u32) -> Clip {
        Clip {
            face,
            depth: Depth::Layers(n),
        }
    }

    /// A clip of `p` percent of the axis extent, in from `face`.
    pub fn percent(face: Face, p: u32) -> Clip {
        Clip {
            face,
            depth: Depth::Percent(p),
        }
    }
}

impl fmt::Display for Clip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.face, self.depth)
    }
}

impl FromStr for Clip {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (face, depth) = s.split_once(':').ok_or_else(|| {
            format!("cutaway clip `{s}` is not `<face>:<depth>` (e.g. `y-max:1`, `z-min:50%`)")
        })?;
        let face: Face = face.trim().parse()?;
        let depth = depth.trim();
        let depth = if let Some(p) = depth.strip_suffix('%') {
            Depth::Percent(
                p.trim()
                    .parse::<u32>()
                    .map_err(|_| format!("cutaway depth `{depth}` is not a percentage"))?,
            )
        } else {
            Depth::Layers(
                depth
                    .parse::<u32>()
                    .map_err(|_| format!("cutaway depth `{depth}` is not a layer count"))?,
            )
        };
        Ok(Clip { face, depth })
    }
}

/// The solid the viewer is outside of: an ordered set of [`Clip`]s. A cell is
/// meshed unless some clip removes it.
///
/// The empty cutaway removes nothing — the full model, which is what an
/// exterior shot wants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cutaway {
    clips: Vec<Clip>,
}

/// The cell interval a [`Cutaway`] leaves along one axis, as `[lo, hi)`.
type Span = (i32, i32);

impl Cutaway {
    /// The cutaway that removes nothing.
    pub fn none() -> Cutaway {
        Cutaway { clips: Vec::new() }
    }

    /// A cutaway of exactly these clips, in this order.
    pub fn new(clips: Vec<Clip>) -> Cutaway {
        Cutaway { clips }
    }

    /// A single clip.
    pub fn of(clip: Clip) -> Cutaway {
        Cutaway { clips: vec![clip] }
    }

    /// The dollhouse: take off the top Y layer. This is the whole of the
    /// renderer's pre-cutaway behaviour, kept as a named constructor because it
    /// is a genuinely common intent — never because it is special.
    pub fn top_layer() -> Cutaway {
        Cutaway::of(Clip::layers(Face::YMax, 1))
    }

    /// The clips, in author order.
    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    /// True when nothing is removed.
    pub fn is_empty(&self) -> bool {
        self.clips.is_empty()
    }

    /// The `[lo, hi)` cell interval left on `axis` for a model of `size`.
    fn span(&self, axis: usize, size: [i32; 3]) -> Span {
        let extent = size[axis].max(0);
        let mut lo = 0;
        let mut hi = extent;
        for c in &self.clips {
            if c.face.axis() != axis {
                continue;
            }
            let d = c.depth.layers(extent);
            if c.face.is_max() {
                hi = hi.min(extent - d);
            } else {
                lo = lo.max(d);
            }
        }
        (lo, hi)
    }

    /// The box of cells this cutaway keeps, as `([from], [to_exclusive])`, or
    /// `None` when it keeps nothing at all.
    ///
    /// Exact, not sampled: the kept set of an intersection of axis-aligned
    /// half-spaces *is* a box, so "would this shot render an empty frame" is
    /// arithmetic.
    pub fn kept_box(&self, size: [i32; 3]) -> Option<([i32; 3], [i32; 3])> {
        let mut from = [0i32; 3];
        let mut to = [0i32; 3];
        for axis in 0..3 {
            let (lo, hi) = self.span(axis, size);
            if lo >= hi {
                return None;
            }
            from[axis] = lo;
            to[axis] = hi;
        }
        Some((from, to))
    }

    /// True when this cutaway takes `pos` (a model-local cell) away.
    pub fn removes(&self, pos: [i32; 3], size: [i32; 3]) -> bool {
        pos.iter().enumerate().any(|(axis, p)| {
            let (lo, hi) = self.span(axis, size);
            *p < lo || *p >= hi
        })
    }

    /// The face a section is *read from*: the one whose clip takes the most
    /// away, so the camera stands where the removed material was and looks at
    /// the cut face. Ties keep author order. `None` for the empty cutaway.
    ///
    /// This is the single derivation of "where do I stand to see this cut" —
    /// the planner and the `--cutaway` CLI flag both go through it, so a shot
    /// asked for on the command line is framed exactly like a planned one.
    pub fn view_face(&self, size: [i32; 3]) -> Option<Face> {
        let mut best: Option<(i32, Face)> = None;
        for c in &self.clips {
            let d = c.depth.layers(size[c.face.axis()]);
            if d <= 0 {
                continue;
            }
            if best.is_none_or(|(bd, _)| d > bd) {
                best = Some((d, c.face));
            }
        }
        best.map(|(_, f)| f)
    }

    /// Orbit `(yaw_deg, pitch_deg)` for looking at this cut: the camera stands
    /// on the side the material was taken from. `None` for the empty cutaway
    /// (there is no cut to read, so the caller keeps its own framing).
    ///
    /// Yaw follows Nucleation's convention (see [`crate::shots::PieceShot`]):
    /// camera at +Z is yaw 0, +X is 90, −Z is 180, −X is 270. A horizontal
    /// section is read from a little above eye level so the floors read as
    /// floors; a plan section is read straight down.
    pub fn viewpoint(&self, size: [i32; 3]) -> Option<(f32, f32)> {
        Some(match self.view_face(size)? {
            Face::ZMax => (0.0, SECTION_PITCH),
            Face::XMax => (90.0, SECTION_PITCH),
            Face::ZMin => (180.0, SECTION_PITCH),
            Face::XMin => (270.0, SECTION_PITCH),
            Face::YMax => (0.0, 90.0),
            Face::YMin => (0.0, -90.0),
        })
    }
}

/// How far above the horizon a vertical section is read from. Not zero: a dead
/// elevation hides the floors, which are half of what a massing review is
/// looking at.
const SECTION_PITCH: f32 = 15.0;

impl fmt::Display for Cutaway {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.clips.is_empty() {
            return f.write_str("none");
        }
        let parts: Vec<String> = self.clips.iter().map(|c| c.to_string()).collect();
        f.write_str(&parts.join("+"))
    }
}

impl FromStr for Cutaway {
    type Err = String;

    /// `none` / the empty string → [`Cutaway::none`]; otherwise `+`-joined
    /// clips, e.g. `y-max:50%`, `x-min:50%+y-max:2`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() || s == "none" {
            return Ok(Cutaway::none());
        }
        let mut clips = Vec::new();
        for part in s.split('+') {
            let part = part.trim();
            if part.is_empty() {
                return Err(format!("empty clip in cutaway `{s}`"));
            }
            clips.push(part.parse::<Clip>()?);
        }
        Ok(Cutaway::new(clips))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: [i32; 3] = [20, 10, 84];

    #[test]
    fn the_old_boolean_is_the_degenerate_case() {
        // `strip_ceiling == true` was exactly this, and nothing else.
        let c = Cutaway::top_layer();
        assert_eq!(c.to_string(), "y-max:1");
        let size = [7, 5, 9];
        assert!(c.removes([3, 4, 4], size), "the top layer is gone");
        assert!(!c.removes([3, 3, 4], size), "the layer below it is not");
        // …and its camera derivation reproduces the historical `top` shot
        // (yaw 0, pitch 90) rather than restating it.
        assert_eq!(c.viewpoint(size), Some((0.0, 90.0)));
    }

    #[test]
    fn round_trips_through_its_spec_string() {
        for s in [
            "none",
            "y-max:1",
            "y-max:50%",
            "z-min:50%",
            "x-min:50%+y-max:2",
            "y-min:4",
        ] {
            let c: Cutaway = s.parse().expect(s);
            assert_eq!(c.to_string(), s, "round trip {s}");
        }
    }

    #[test]
    fn percent_is_integer_arithmetic_of_the_axis_extent() {
        let c: Cutaway = "z-min:50%".parse().unwrap();
        // 84 / 2 = 42 removed from the low-Z end.
        assert!(c.removes([0, 0, 41], SIZE));
        assert!(!c.removes([0, 0, 42], SIZE));
        // An odd extent floors rather than rounding — deterministic on every
        // machine, no float in the path.
        let odd: Cutaway = "z-min:50%".parse().unwrap();
        assert!(!odd.removes([0, 0, 3], [1, 1, 7]));
        assert!(odd.removes([0, 0, 2], [1, 1, 7]));
    }

    #[test]
    fn clips_compose_as_an_intersection_of_half_spaces() {
        let c: Cutaway = "x-min:50%+y-max:2".parse().unwrap();
        let (from, to) = c.kept_box(SIZE).expect("keeps something");
        assert_eq!(from, [10, 0, 0]);
        assert_eq!(to, [20, 8, 84]);
        assert!(c.removes([9, 0, 0], SIZE));
        assert!(c.removes([10, 8, 0], SIZE));
        assert!(!c.removes([10, 7, 0], SIZE));
    }

    #[test]
    fn a_cutaway_that_keeps_nothing_is_arithmetic_not_a_scan() {
        assert!(Cutaway::none().kept_box(SIZE).is_some());
        // One clip as deep as the model.
        let all: Cutaway = "y-max:100%".parse().unwrap();
        assert_eq!(all.kept_box(SIZE), None);
        // Two clips that meet in the middle: neither alone empties the model.
        let meet: Cutaway = "x-min:50%+x-max:50%".parse().unwrap();
        assert!(
            "x-min:50%"
                .parse::<Cutaway>()
                .unwrap()
                .kept_box(SIZE)
                .is_some()
        );
        assert!(
            "x-max:50%"
                .parse::<Cutaway>()
                .unwrap()
                .kept_box(SIZE)
                .is_some()
        );
        assert_eq!(meet.kept_box(SIZE), None);
        // Layers saturate at the extent rather than wrapping.
        let over: Cutaway = "y-max:9999".parse().unwrap();
        assert_eq!(over.kept_box(SIZE), None);
    }

    #[test]
    fn the_viewpoint_stands_where_the_material_was_removed() {
        let expect = [
            ("z-min:50%", (180.0, SECTION_PITCH)),
            ("z-max:50%", (0.0, SECTION_PITCH)),
            ("x-min:50%", (270.0, SECTION_PITCH)),
            ("x-max:50%", (90.0, SECTION_PITCH)),
            ("y-max:50%", (0.0, 90.0)),
            ("y-min:2", (0.0, -90.0)),
        ];
        for (spec, want) in expect {
            let c: Cutaway = spec.parse().unwrap();
            assert_eq!(c.viewpoint(SIZE), Some(want), "{spec}");
        }
        assert_eq!(Cutaway::none().viewpoint(SIZE), None);
        // The deepest cut is the one that is read; ties keep author order.
        let mixed: Cutaway = "y-max:1+z-min:50%".parse().unwrap();
        assert_eq!(mixed.view_face(SIZE), Some(Face::ZMin));
        let tie: Cutaway = "x-min:2+z-min:2".parse().unwrap();
        assert_eq!(tie.view_face(SIZE), Some(Face::XMin));
    }

    #[test]
    fn bad_specs_are_refused_by_name() {
        for bad in ["y-mid:1", "y-max", "y-max:one", "y-max:50%%", "y-max:1+"] {
            assert!(bad.parse::<Cutaway>().is_err(), "{bad} should not parse");
        }
    }

    #[test]
    fn a_zero_depth_clip_removes_nothing_and_reads_as_no_cut() {
        let c: Cutaway = "y-max:0".parse().unwrap();
        assert!(!c.removes([0, 9, 0], SIZE));
        assert_eq!(c.view_face(SIZE), None);
        // 0% of any extent is 0 layers.
        let p: Cutaway = "z-min:0%".parse().unwrap();
        assert!(!p.removes([0, 0, 0], SIZE));
    }
}
