//! Author-declared views — a camera the author aims, in the language the shot
//! manifest already speaks.
//!
//! # Why this exists
//!
//! The planned set ([`crate::shots`]) answers two questions well and a third not
//! at all. The orbit cameras are corner-isometrics at pitch 30, a plan at pitch
//! 90 and per-socket/per-anchor obliques; the eye cameras stand inside the piece
//! at a body's eye height. **Nothing in that set is square-on at a face.** A
//! building whose identity is one elevation — a west front, a gatehouse, an
//! approach face — has no picture in its own review set, and the near workaround
//! makes it worse: a level eye camera with a 70° vertical field reaches only
//! ≈ 0.7 × distance above eye height, so framing a 20-block front needs ≈26
//! blocks of standoff, and a forecourt long enough to hold that standoff enters
//! the model bounds and shrinks the building in every orbit frame.
//!
//! # What a view is
//!
//! Nothing new. [`crate::shots::PieceShot`] is already a complete camera
//! description — a bearing, a pitch, a field of view, a framing, a cutaway — and
//! `<stem>-shots.json` already writes every one of those fields down per shot.
//! A view is that same description arriving as an *input*: an author states one,
//! the planner appends it to the same plan, and it is rendered, named and
//! recorded exactly like a planned shot. There is no second camera path.
//!
//! # What a view aims at
//!
//! A camera is a **bearing** plus a **subject**, and both are stated in terms the
//! *piece* owns rather than in terms of any campaign's fiction:
//!
//! - a bearing is a [`Face`] of the subject's own box (`face=north`, and `up` /
//!   `down` for the two horizontal ones) or a raw `yaw=`;
//! - a subject is a named box in the piece: the whole model, or a declared
//!   anchor's point/region ([`Subject`]).
//!
//! "The west front" is a design decision about cathedrals; "the −Z face of this
//! box" is a mechanism, and a creator building something else configures it to
//! their own fiction by naming their own box. When a scope can name a box of its
//! own, that box becomes another [`Subject`] variant and every other field here
//! is unchanged.

use crate::meta::PrefabMeta;
use crate::nbt::Structure;
use crate::occupancy::Facing;

/// A face of an axis-aligned box, named the way Minecraft names directions.
///
/// Six faces, because a box has six. `up`/`down` are the two an author reaches
/// for when they want a plan or a soffit; the four cardinals are the elevations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    North,
    South,
    East,
    West,
    Up,
    Down,
}

impl Face {
    /// Parse a face keyword; `None` for anything else.
    pub fn parse(s: &str) -> Option<Face> {
        match s {
            "north" => Some(Face::North),
            "south" => Some(Face::South),
            "east" => Some(Face::East),
            "west" => Some(Face::West),
            "up" => Some(Face::Up),
            "down" => Some(Face::Down),
            _ => None,
        }
    }

    /// The keyword, for the manifest.
    pub fn as_str(self) -> &'static str {
        match self {
            Face::North => "north",
            Face::South => "south",
            Face::East => "east",
            Face::West => "west",
            Face::Up => "up",
            Face::Down => "down",
        }
    }

    /// The cardinal this face points along, for the four vertical faces.
    pub fn facing(self) -> Option<Facing> {
        match self {
            Face::North => Some(Facing::North),
            Face::South => Some(Facing::South),
            Face::East => Some(Facing::East),
            Face::West => Some(Facing::West),
            Face::Up | Face::Down => None,
        }
    }

    /// Yaw and pitch of a camera standing off this face and looking square-on at
    /// it.
    ///
    /// The four cardinals reuse [`Facing::behind_yaw_deg`] — the one conversion
    /// that already means "stand on the far side of a thing with this facing and
    /// look back along it", which is exactly what photographing a face is. A
    /// second copy of that arithmetic here is how the doorway shots and the
    /// elevations would drift apart under a Nucleation convention change.
    pub fn camera(self) -> (f32, f32) {
        match self {
            Face::Up => (0.0, 90.0),
            Face::Down => (0.0, -90.0),
            other => (
                other
                    .facing()
                    .expect("the four cardinals have a facing")
                    .behind_yaw_deg(),
                0.0,
            ),
        }
    }
}

/// The box a view is aimed at — a named box the piece owns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Subject {
    /// The whole piece.
    Model,
    /// A declared anchor's point or region, by its full metadata name.
    Anchor(String),
}

impl Subject {
    /// The manifest tag.
    pub fn tag(&self) -> String {
        match self {
            Subject::Model => "model".to_string(),
            Subject::Anchor(name) => name.clone(),
        }
    }

    /// The subject's box, as inclusive-exclusive block corners.
    ///
    /// An unresolvable subject is an **error**, never a fallback to the model:
    /// a view that silently photographed the whole piece instead of the anchor
    /// the author named would be a correct-looking picture of the wrong thing,
    /// and nothing downstream could tell.
    pub fn box_of(
        &self,
        st: &Structure,
        meta: Option<&PrefabMeta>,
    ) -> Result<([f32; 3], [f32; 3]), String> {
        match self {
            Subject::Model => Ok((
                [0.0, 0.0, 0.0],
                [st.size[0] as f32, st.size[1] as f32, st.size[2] as f32],
            )),
            Subject::Anchor(name) => {
                let Some(meta) = meta else {
                    return Err(format!(
                        "view aims at `{name}`, but this piece has no metadata file, so it \
                         declares no anchors at all"
                    ));
                };
                let Some(a) = meta.anchors.get(name) else {
                    let mut declared: Vec<&str> = meta.anchors.keys().map(String::as_str).collect();
                    declared.sort_unstable();
                    let list = if declared.is_empty() {
                        "none".to_string()
                    } else {
                        declared.join(", ")
                    };
                    return Err(format!(
                        "view aims at `{name}`, which this piece does not declare. Declared \
                         anchors: {list}"
                    ));
                };
                if let Some(p) = a.pos {
                    return Ok((
                        [p[0] as f32, p[1] as f32, p[2] as f32],
                        [p[0] as f32 + 1.0, p[1] as f32 + 1.0, p[2] as f32 + 1.0],
                    ));
                }
                if let Some(r) = &a.region {
                    let lo = |i: usize| r.from[i].min(r.to[i]) as f32;
                    let hi = |i: usize| r.from[i].max(r.to[i]) as f32 + 1.0;
                    return Ok(([lo(0), lo(1), lo(2)], [hi(0), hi(1), hi(2)]));
                }
                Err(format!(
                    "view aims at `{name}`, which declares neither a position nor a region — \
                     there is no box to point a camera at"
                ))
            }
        }
    }

    /// The centre of the subject's box, in structure coordinates.
    pub fn centre(&self, st: &Structure, meta: Option<&PrefabMeta>) -> Result<[f32; 3], String> {
        let (lo, hi) = self.box_of(st, meta)?;
        Ok(midpoint(lo, hi))
    }
}

/// Centre of a box.
pub fn midpoint(lo: [f32; 3], hi: [f32; 3]) -> [f32; 3] {
    [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ]
}

/// Default field of view for a declared view: the orbit lens, so a declared
/// elevation is directly comparable with the planned exterior shots.
pub const DEFAULT_VIEW_FOV_DEG: f32 = crate::shots::ORBIT_FOV_DEG;

/// One author-declared camera.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    /// Shot name — the image is `<stem>-<name>.png`.
    pub name: String,
    /// The spec as written, recorded in the manifest so a frame can be re-asked
    /// for verbatim.
    pub spec: String,
    /// The face this view photographs, when it was stated as one.
    pub face: Option<Face>,
    /// Camera bearing (degrees, Nucleation convention).
    pub yaw_deg: f32,
    /// Camera pitch (degrees).
    pub pitch_deg: f32,
    /// Field of view (degrees).
    pub fov_deg: f32,
    /// Zoom on top of the fit: 1 frames the whole framed box, >1 pushes closer.
    pub zoom: f32,
    /// The box the camera is aimed at.
    pub subject: Subject,
    /// Strip the top Y layer before meshing.
    pub cutaway: bool,
}

impl View {
    /// The box this view frames — the thing the author asked to see.
    ///
    /// For a `yaw=` view that is the whole subject box. For a `face=` view it is
    /// the subject box **collapsed to that face**, and that difference is what
    /// separates a surface that works from one that needs a magic number per
    /// building. Fitting the whole box from the front of a 93-block-deep nave
    /// backs the camera off past the apse and leaves the west front occupying a
    /// third of the frame; the author's only recourse is then a hand-tuned
    /// `zoom=` rediscovered for every piece, which is exactly the downstream
    /// folklore a first-class surface exists to prevent. An elevation frames its
    /// elevation.
    pub fn framed_box(
        &self,
        st: &Structure,
        meta: Option<&PrefabMeta>,
    ) -> Result<([f32; 3], [f32; 3]), String> {
        let (mut lo, mut hi) = self.subject.box_of(st, meta)?;
        if let Some(f) = self.face {
            let (axis, at_max) = match f {
                Face::West => (0, false),
                Face::East => (0, true),
                Face::Down => (1, false),
                Face::Up => (1, true),
                Face::North => (2, false),
                Face::South => (2, true),
            };
            if at_max {
                lo[axis] = hi[axis];
            } else {
                hi[axis] = lo[axis];
            }
        }
        Ok((lo, hi))
    }
}

/// Every key a view spec accepts, in the order the help prints them.
const KEYS: &[&str] = &[
    "name", "face", "yaw", "pitch", "fov", "zoom", "of", "cutaway",
];

/// A view name must be a plain lowercase filename fragment: it becomes a file
/// stem, and a name carrying a path separator or an upper-case letter is a
/// portability bug waiting for the first case-insensitive filesystem.
fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("view `name=` is empty".to_string());
    }
    let ok = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !ok || name.starts_with('-') {
        return Err(format!(
            "view name `{name}` is not usable as a file stem — use lowercase letters, digits and \
             hyphens, not starting with a hyphen"
        ));
    }
    Ok(())
}

fn number(key: &str, value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("view `{key}={value}`: not a finite number"))
}

impl View {
    /// Parse one `--view` spec: comma-separated `key=value` pairs.
    ///
    /// ```text
    /// face=north                             the −Z elevation, square-on
    /// name=west-front,face=north             …under a name of the author's choosing
    /// face=east,of=anchor/crossing,zoom=2    …centred on a declared anchor, closer
    /// yaw=25,pitch=15,fov=60                 an arbitrary bearing
    /// ```
    ///
    /// Exactly one of `face` / `yaw` is required: a camera with no bearing is not
    /// a camera. Everything else defaults to the elevation case — pitch 0, the
    /// orbit lens, the fit distance, no cutaway, the whole model as subject.
    pub fn parse(spec: &str) -> Result<View, String> {
        let mut name: Option<String> = None;
        let mut face: Option<Face> = None;
        let mut yaw: Option<f32> = None;
        let mut pitch: Option<f32> = None;
        let mut fov = DEFAULT_VIEW_FOV_DEG;
        let mut zoom = 1.0f32;
        let mut subject = Subject::Model;
        let mut cutaway = false;

        for field in spec.split(',') {
            let field = field.trim();
            if field.is_empty() {
                continue;
            }
            let (key, value) = match field.split_once('=') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => {
                    return Err(format!(
                        "view field `{field}` is not `key=value` (keys: {})",
                        KEYS.join(", ")
                    ));
                }
            };
            match key {
                "name" => {
                    check_name(value)?;
                    name = Some(value.to_string());
                }
                "face" => {
                    face = Some(Face::parse(value).ok_or_else(|| {
                        format!(
                            "view `face={value}`: not a face — use north, south, east, west, up \
                             or down"
                        )
                    })?);
                }
                "yaw" => yaw = Some(number("yaw", value)?),
                "pitch" => pitch = Some(number("pitch", value)?),
                "fov" => {
                    let v = number("fov", value)?;
                    if v <= 0.0 || v >= 180.0 {
                        return Err(format!(
                            "view `fov={value}`: a field of view must be between 0 and 180 degrees"
                        ));
                    }
                    fov = v;
                }
                "zoom" => {
                    let v = number("zoom", value)?;
                    if v <= 0.0 {
                        return Err(format!("view `zoom={value}`: must be greater than 0"));
                    }
                    zoom = v;
                }
                "of" => {
                    subject = if value == "model" {
                        Subject::Model
                    } else {
                        Subject::Anchor(value.to_string())
                    };
                }
                "cutaway" => {
                    cutaway = match value {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(format!("view `cutaway={other}`: use true or false"));
                        }
                    }
                }
                other => {
                    return Err(format!(
                        "view field `{other}` is not a view key (keys: {})",
                        KEYS.join(", ")
                    ));
                }
            }
        }

        let (yaw_deg, default_pitch) = match (face, yaw) {
            (Some(_), Some(_)) => {
                return Err(
                    "view states both `face=` and `yaw=` — a camera has one bearing. Use `face=` \
                     for a square-on elevation, `yaw=` for any other angle"
                        .to_string(),
                );
            }
            (Some(f), None) => f.camera(),
            (None, Some(y)) => (y.rem_euclid(360.0), 0.0),
            (None, None) => {
                return Err(format!(
                    "view `{spec}` states no bearing — give `face=<north|south|east|west|up|down>` \
                     for a square-on elevation, or `yaw=<degrees>`"
                ));
            }
        };

        let name = match name {
            Some(n) => n,
            None => match face {
                Some(f) => format!("view-{}", f.as_str()),
                None => format!("view-yaw{}", strip_point_zero(yaw_deg)),
            },
        };

        Ok(View {
            name,
            spec: spec.to_string(),
            face,
            yaw_deg,
            pitch_deg: pitch.unwrap_or(default_pitch),
            fov_deg: fov,
            zoom,
            subject,
            cutaway,
        })
    }
}

/// `45.0` → `45`, `22.5` → `22.5`: a default view name should read like the
/// number the author typed.
fn strip_point_zero(v: f32) -> String {
    let s = format!("{v}");
    s.strip_suffix(".0").unwrap_or(&s).replace('.', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> PrefabMeta {
        serde_json::from_slice(
            br#"{"anchors": {
                    "anchor/altar": { "pos": [15,1,86], "facing": "north" },
                    "anchor/gate":  { "region": { "from": [2,1,4], "to": [4,3,6] } },
                    "anchor/bare":  { "facing": "north" }
                }}"#,
        )
        .unwrap()
    }

    fn structure() -> Structure {
        Structure {
            size: [31, 64, 93],
            palette: vec!["minecraft:air".into()],
            blocks: vec![],
        }
    }

    /// The whole point of the surface: naming a face gives a camera standing off
    /// that face, level, looking straight at it.
    #[test]
    fn a_face_is_a_square_on_camera() {
        for (face, yaw) in [
            ("north", 180.0),
            ("south", 0.0),
            ("east", 90.0),
            ("west", 270.0),
        ] {
            let v = View::parse(&format!("face={face}")).unwrap();
            assert_eq!(v.yaw_deg, yaw, "face {face}");
            assert_eq!(v.pitch_deg, 0.0, "face {face} must be level");
            assert_eq!(v.name, format!("view-{face}"));
        }
        assert_eq!(View::parse("face=up").unwrap().pitch_deg, 90.0);
        assert_eq!(View::parse("face=down").unwrap().pitch_deg, -90.0);
    }

    /// A camera standing off a face looks the OPPOSITE way from a body with that
    /// facing — and the two derive from one conversion, so they cannot drift.
    #[test]
    fn a_face_camera_looks_back_along_that_facing() {
        for (face, facing) in [
            (Face::North, Facing::North),
            (Face::South, Facing::South),
            (Face::East, Facing::East),
            (Face::West, Facing::West),
        ] {
            assert_eq!(face.camera().0, facing.behind_yaw_deg());
            assert_eq!(
                (face.camera().0 - facing.view_yaw_deg()).abs(),
                180.0,
                "{face:?}"
            );
        }
    }

    #[test]
    fn every_field_is_settable_and_defaults_are_the_elevation_case() {
        let v = View::parse("face=north").unwrap();
        assert_eq!(v.fov_deg, DEFAULT_VIEW_FOV_DEG);
        assert_eq!(v.zoom, 1.0);
        assert_eq!(v.subject, Subject::Model);
        assert!(!v.cutaway);

        let v = View::parse(
            "name=nave-section,face=east,of=anchor/altar,pitch=-5,fov=60,zoom=2.5,cutaway=true",
        )
        .unwrap();
        assert_eq!(v.name, "nave-section");
        assert_eq!(v.yaw_deg, 90.0);
        assert_eq!(v.pitch_deg, -5.0);
        assert_eq!(v.fov_deg, 60.0);
        assert_eq!(v.zoom, 2.5);
        assert_eq!(v.subject, Subject::Anchor("anchor/altar".into()));
        assert!(v.cutaway);
    }

    #[test]
    fn a_raw_yaw_is_a_bearing_too() {
        let v = View::parse("yaw=25,pitch=15").unwrap();
        assert_eq!(v.yaw_deg, 25.0);
        assert_eq!(v.pitch_deg, 15.0);
        assert_eq!(v.name, "view-yaw25");
        assert_eq!(View::parse("yaw=-90").unwrap().yaw_deg, 270.0);
        assert_eq!(View::parse("yaw=22.5").unwrap().name, "view-yaw22-5");
    }

    #[test]
    fn a_view_without_a_bearing_is_refused() {
        let e = View::parse("name=x,fov=60").unwrap_err();
        assert!(e.contains("no bearing"), "{e}");
    }

    #[test]
    fn two_bearings_are_refused() {
        let e = View::parse("face=north,yaw=10").unwrap_err();
        assert!(e.contains("one bearing"), "{e}");
    }

    #[test]
    fn bad_fields_are_named_not_ignored() {
        for (spec, needle) in [
            ("face=northeast", "not a face"),
            ("face=north,fov=200", "between 0 and 180"),
            ("face=north,zoom=0", "greater than 0"),
            ("face=north,yow=3", "not a view key"),
            ("face=north,cutaway", "not `key=value`"),
            ("face=north,cutaway=yes", "use true or false"),
            ("face=north,name=West Front", "not usable as a file stem"),
            ("face=north,pitch=nan", "not a finite number"),
        ] {
            let e = View::parse(spec).unwrap_err();
            assert!(e.contains(needle), "spec `{spec}` gave `{e}`");
        }
    }

    #[test]
    fn a_subject_resolves_to_the_centre_of_its_box() {
        let (st, m) = (structure(), meta());
        assert_eq!(
            Subject::Model.centre(&st, Some(&m)).unwrap(),
            [15.5, 32.0, 46.5]
        );
        assert_eq!(
            Subject::Anchor("anchor/altar".into())
                .centre(&st, Some(&m))
                .unwrap(),
            [15.5, 1.5, 86.5]
        );
        assert_eq!(
            Subject::Anchor("anchor/gate".into())
                .centre(&st, Some(&m))
                .unwrap(),
            [3.5, 2.5, 5.5]
        );
    }

    /// A bad aim never becomes a picture of something else: an anchor the piece
    /// does not declare is an error that names what it does declare.
    #[test]
    fn an_unknown_subject_is_an_error_that_lists_the_real_ones() {
        let (st, m) = (structure(), meta());
        let e = Subject::Anchor("anchor/west-front".into())
            .centre(&st, Some(&m))
            .unwrap_err();
        assert!(e.contains("does not declare"), "{e}");
        assert!(e.contains("anchor/altar"), "{e}");
        assert!(e.contains("anchor/gate"), "{e}");

        let e = Subject::Anchor("anchor/bare".into())
            .centre(&st, Some(&m))
            .unwrap_err();
        assert!(e.contains("neither a position nor a region"), "{e}");

        let e = Subject::Anchor("anchor/altar".into())
            .centre(&st, None)
            .unwrap_err();
        assert!(e.contains("no metadata file"), "{e}");
    }

    #[test]
    fn parsing_is_deterministic() {
        let spec = "name=west-front,face=north,zoom=1.5";
        assert_eq!(View::parse(spec).unwrap(), View::parse(spec).unwrap());
    }
}
