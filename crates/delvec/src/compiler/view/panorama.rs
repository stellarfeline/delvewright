//! The storybook's exterior shot of the built place (`delvec panorama`).
//!
//! Every content release ships an oblique view of the delve's built place. The
//! camera for it is *computed from the layout*, not authored: `render-plan.json`
//! states the layout AABB and the world-generator horizon, which is everything
//! the framing needs — so the shot is a first-class emission rather than a
//! hand-edited scene JSON.
//!
//! ## Why its own subcommand, not another shot in `scene`
//!
//! `scene` emits exactly one Chunky scene per shot in the compiler's plan, and
//! `index` pairs each of those scenes with the plan's machine `expect` lines for
//! the vision reviewer. The panorama has no `expect` pair and no plan shot: it is
//! a release artifact, not a review frame. Folding it into `scene` would break
//! the shot↔scene correspondence both commands rest on and push a release
//! decision (bearing, pitch, frame size, sample budget) into the review path.
//! Those are a creator's choices at render time, which are CLI flags, not
//! compiled constants.
//!
//! ## The subject is the placed areas; the horizon is loaded, never framed
//!
//! The camera frames the **layout AABB** — the union of the placed areas, which
//! is what the campaign declares it built — on every horizon. The ground a
//! `valley` builds around it is loaded (`scene::loaded_extent` feeds the chunk
//! list and the Y clip) so the place stands in its landform, but that landform is
//! not part of the subject. Framing the union instead put a castle a fifth of the
//! frame wide inside its mountains: on `doune-castle-tour` the valley's box is
//! 260×300 blocks around a 104×120 layout, and the frame was mostly slopes and
//! the void beyond them. On an ocean or void horizon the two boxes are the same,
//! so the rule means one thing everywhere.
//!
//! ## Framing
//!
//! The camera looks down at `pitch` from one of eight [`Bearing`]s. Its distance
//! is solved exactly, not guessed: all eight corners of the subject box are
//! projected into the camera basis and the camera is pushed back until every one
//! of them is inside the frustum — the vertical [`FOV_DEG`] and the horizontal
//! angle the frame's aspect gives it — with [`MARGIN`] of breathing room — and
//! slid in the image plane until the projected box is centred, so the slack is
//! shared by opposite edges instead of piling up on one side of a perspective
//! view. A narrow FOV keeps the perspective close to the oblique "model on a
//! table" look and away from wide-angle distortion.
//!
//! Chunky's `fov` is the **vertical** field of view: the pinned core's
//! `PathTracingRenderer` maps a pixel to `x ∈ [−w/2h, w/2h]`, `y ∈ [−½, ½]`, and
//! `PinholeProjector` scales both by `2·tan(fov/2)` (read from the pinned core's
//! bytecode). So the horizontal half-extent is `aspect · tan(fov/2)`.
//!
//! How much of the frame the subject covers is measured from the emitted camera
//! and returned ([`SubjectSpan`]); a corner bearing puts any subject's box over
//! at least a third of the default frame, while a cardinal bearing faces one
//! side of it, which a long strip of rooms cannot fill.
//!
//! ## What is behind the subject
//!
//! Every ray that leaves the loaded world samples the sky. The simulated sky of
//! the pinned core (`PreethamSky.calcIncidentLight`) clamps a downward ray's
//! elevation to the horizon, so below the horizon it returns one bright,
//! desaturated haze colour. At any pitch steeper than half the vertical FOV no
//! ray in the frame points above the horizon at all: the flat grey around a
//! whole-map frame was never sky, it was the void outside the loaded chunks.
//! Framing the subject is what keeps it out of the picture; nothing here paints
//! it.
//!
//! ## The sun is the campaign's hour, and the bearing is the creator's choice
//!
//! The sun is `scene::sun_at` of the plan's declared hour — the same one every
//! review scene gets. So the bearing is purely a choice of which side is in shot,
//! and choosing it is the creator's job: at a given hour the sun stands east or
//! west, and a bearing looking into it is a backlit frame. A delve declared at
//! `night` or `midnight` renders dark, which is what a night delve looks like;
//! nothing here brightens a scene to make a picture come out.

use crate::compiler::view::camera::{self, Frame};
use crate::compiler::view::diag::{DW_INPUT, Diagnostic};
use crate::compiler::view::scene;

/// Default camera pitch, degrees below horizontal — the 45° oblique.
pub const DEFAULT_PITCH_DEG: u8 = 45;

/// The pitches a panorama accepts, degrees. Above 85° the view is a plan and its
/// "right" axis degenerates; below 5° the camera is level with what it frames.
pub const PITCH_RANGE: std::ops::RangeInclusive<u8> = 5..=85;

/// Default vertical field of view, degrees. Narrower than the 70° first-person
/// default: a view of a built place wants low distortion, not reach.
pub const FOV_DEG: f64 = 40.0;

/// The vertical fields of view a panorama accepts, degrees: from a long lens far
/// back to a wide one close in.
pub const FOV_RANGE: std::ops::RangeInclusive<u8> = 10..=90;

/// Framing margin: the solved distance leaves this much slack around the
/// subject's tightest fit (1.0 = corners exactly on the frame edge).
pub const MARGIN: f64 = 1.12;

/// Halvings of the interval the centring offset is searched in. Fixed, not
/// "until converged", so the bytes cannot depend on a tolerance test; a hundred
/// halvings of a box-sized interval are below any float's resolution.
const CENTRING_STEPS: usize = 100;

/// Never place the camera closer than this to the aim point, so a degenerate
/// (single-block) AABB still yields a usable scene.
const MIN_DISTANCE: f64 = 16.0;

/// Default sample target for a final frame (`--spp` overrides). The tiered
/// doctrine: ~64 for a draft look, ~300 for release art.
pub const DEFAULT_SPP: u32 = 300;

/// Default frame width, pixels. A storybook or repository README shows an image
/// at its column's width — about 800 CSS pixels — so 1600 is that column at 2×
/// device pixels.
pub const DEFAULT_WIDTH: u32 = 1600;

/// Default frame height, pixels: 16:9 with [`DEFAULT_WIDTH`], a landscape frame
/// for a place that lies along the ground.
pub const DEFAULT_HEIGHT: u32 = 900;

/// Which side of the subject the camera stands on: a corner or a face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[clap(rename_all = "lower")]
pub enum Bearing {
    /// South-east (the default): camera at +X/+Z, looking north-west.
    Se,
    /// South-west: camera at −X/+Z, looking north-east.
    Sw,
    /// North-east: camera at +X/−Z, looking south-west.
    Ne,
    /// North-west: camera at −X/−Z, looking south-east.
    Nw,
    /// North: camera at −Z, looking south at the north face.
    N,
    /// East: camera at +X, looking west at the east face.
    E,
    /// South: camera at +Z, looking north at the south face.
    S,
    /// West: camera at −X, looking east at the west face.
    W,
}

impl Bearing {
    /// Every bearing, in declaration order.
    pub const ALL: [Bearing; 8] = [
        Bearing::Se,
        Bearing::Sw,
        Bearing::Ne,
        Bearing::Nw,
        Bearing::N,
        Bearing::E,
        Bearing::S,
        Bearing::W,
    ];

    /// Lowercase name, used in the emitted file and scene names.
    pub fn name(self) -> &'static str {
        match self {
            Bearing::Se => "se",
            Bearing::Sw => "sw",
            Bearing::Ne => "ne",
            Bearing::Nw => "nw",
            Bearing::N => "n",
            Bearing::E => "e",
            Bearing::S => "s",
            Bearing::W => "w",
        }
    }

    /// Whether the camera stands over a corner (looking along a diagonal) rather
    /// than square to a face.
    pub fn is_corner(self) -> bool {
        matches!(self, Bearing::Se | Bearing::Sw | Bearing::Ne | Bearing::Nw)
    }

    /// Horizontal unit vector from the subject **toward** the camera.
    /// Minecraft axes: +X east, +Z south.
    fn toward_camera(self) -> [f64; 3] {
        const D: f64 = std::f64::consts::FRAC_1_SQRT_2;
        match self {
            Bearing::Se => [D, 0.0, D],
            Bearing::Sw => [-D, 0.0, D],
            Bearing::Ne => [D, 0.0, -D],
            Bearing::Nw => [-D, 0.0, -D],
            Bearing::N => [0.0, 0.0, -1.0],
            Bearing::E => [1.0, 0.0, 0.0],
            Bearing::S => [0.0, 0.0, 1.0],
            Bearing::W => [-1.0, 0.0, 0.0],
        }
    }
}

/// A solved camera, in the render-plan convention (`yaw = atan2(-dz,dx)`,
/// positive `pitch` looks down).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanoramaCamera {
    pub pos: [f64; 3],
    pub yaw_deg: f64,
    pub pitch_deg: f64,
    pub fov_deg: f64,
}

/// How much of the frame the subject box occupies, measured from the emitted
/// camera: the projected box's bounding rectangle as a fraction of the frame's
/// width and of its height (each clipped to the frame). [`SubjectSpan::fill`] is
/// their product — the share of the frame's area the subject's extent covers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubjectSpan {
    pub width: f64,
    pub height: f64,
}

impl SubjectSpan {
    /// The share of the frame's area inside the subject's bounding rectangle.
    pub fn fill(self) -> f64 {
        self.width * self.height
    }
}

/// Round to 6 decimals: enough precision for a camera, few enough digits that
/// libm ulp differences between platforms cannot move the emitted bytes
/// (ADR-0006).
fn round6(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    // Never emit `-0.0`; it serializes differently from `0.0`.
    if r == 0.0 { 0.0 } else { r }
}

/// The geometric box of an inclusive block AABB: blocks span `[c, c+1)`, so the
/// far corner is `max + 1`.
fn box_corners(min: [i32; 3], max: [i32; 3]) -> ([f64; 3], [[f64; 3]; 8]) {
    let lo = [min[0] as f64, min[1] as f64, min[2] as f64];
    let hi = [
        max[0] as f64 + 1.0,
        max[1] as f64 + 1.0,
        max[2] as f64 + 1.0,
    ];
    let centre = [
        (lo[0] + hi[0]) / 2.0,
        (lo[1] + hi[1]) / 2.0,
        (lo[2] + hi[2]) / 2.0,
    ];
    let mut corners = [[0.0; 3]; 8];
    for (i, c) in corners.iter_mut().enumerate() {
        *c = [
            if i & 1 == 0 { lo[0] } else { hi[0] },
            if i & 2 == 0 { lo[1] } else { hi[1] },
            if i & 4 == 0 { lo[2] } else { hi[2] },
        ];
    }
    (centre, corners)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The camera basis for a bearing and pitch: view direction, right, up.
fn basis(bearing: Bearing, pitch_deg: u8) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let h = bearing.toward_camera();
    let p = f64::from(pitch_deg).to_radians();
    // View direction: horizontally toward the subject, tilted down by `p`.
    let f = [-h[0] * p.cos(), -p.sin(), -h[2] * p.cos()];
    // `right` is horizontal (the frame is never rolled).
    let right = {
        let v = [-f[2], 0.0, f[0]];
        let n = (v[0] * v[0] + v[2] * v[2]).sqrt();
        [v[0] / n, 0.0, v[2] / n]
    };
    let up = cross(right, f);
    (f, right, up)
}

/// The projected subject's extent seen from `pos`, in tangent units:
/// `(min_x, max_x, min_y, max_y)`.
fn projected_extent(
    corners: &[[f64; 3]; 8],
    pos: [f64; 3],
    (f, right, up): ([f64; 3], [f64; 3], [f64; 3]),
) -> (f64, f64, f64, f64) {
    let mut e = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for c in corners {
        let rel = [c[0] - pos[0], c[1] - pos[1], c[2] - pos[2]];
        let depth = dot(rel, f);
        let x = dot(rel, right) / depth;
        let y = dot(rel, up) / depth;
        e = (e.0.min(x), e.1.max(x), e.2.min(y), e.3.max(y));
    }
    e
}

/// Solve the camera for an inclusive subject AABB.
///
/// The camera looks from `bearing` at `pitch_deg` below horizontal. With the
/// direction fixed, write its position as `centre + a·right + b·up − t·forward`.
/// A corner at `(R, U, F)` in that basis lands inside a frame of vertical
/// [`FOV_DEG`] and `aspect` (width / height), with [`MARGIN`] to spare, iff
/// `|R − a| ≤ kₕ·(F + t)` and `|U − b| ≤ kᵥ·(F + t)`, where `kᵥ = tan(fov/2)/MARGIN`
/// and `kₕ = aspect·kᵥ`. Each axis is then a one-dimensional problem: the least
/// `t` over `a` is `(A + B)/2` with `A = max(R/kₕ − F)` and `B = max(−R/kₕ − F)`
/// — exact, no search. The camera stands at the larger of the two axes' `t`, and
/// each offset is the one that centres its axis: the projected coordinates
/// `(R − a)/(F + t)` span symmetrically about zero, which is also the offset
/// that minimises the worst corner, so a feasible `t` stays feasible.
pub fn frame(
    min: [i32; 3],
    max: [i32; 3],
    bearing: Bearing,
    pitch_deg: u8,
    fov_deg: f64,
    aspect: f64,
) -> (PanoramaCamera, SubjectSpan) {
    let (centre, corners) = box_corners(min, max);
    let view = basis(bearing, pitch_deg);
    let (f, right, up) = view;

    let tan_v = (fov_deg.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    let rel: Vec<[f64; 3]> = corners
        .iter()
        .map(|c| [c[0] - centre[0], c[1] - centre[1], c[2] - centre[2]])
        .collect();
    let along = |axis: [f64; 3]| -> Vec<(f64, f64)> {
        rel.iter().map(|c| (dot(*c, axis), dot(*c, f))).collect()
    };
    let (horizontal, vertical) = (along(right), along(up));
    let least_t = |pts: &[(f64, f64)], k: f64| -> f64 {
        let a = pts.iter().map(|&(r, d)| r / k - d).fold(f64::MIN, f64::max);
        let b = pts
            .iter()
            .map(|&(r, d)| -r / k - d)
            .fold(f64::MIN, f64::max);
        (a + b) / 2.0
    };
    let t = least_t(&horizontal, tan_h / MARGIN)
        .max(least_t(&vertical, tan_v / MARGIN))
        .max(MIN_DISTANCE);
    // The offset whose projected span is symmetric: `max x + min x` falls
    // monotonically as the offset grows, so the root is bisected.
    let centred = |pts: &[(f64, f64)]| -> f64 {
        let imbalance = |o: f64| {
            let xs = pts.iter().map(|&(r, d)| (r - o) / (d + t));
            let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
            lo + hi
        };
        let (mut lo, mut hi) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &(r, _)| {
            (lo.min(r), hi.max(r))
        });
        for _ in 0..CENTRING_STEPS {
            let mid = (lo + hi) / 2.0;
            if imbalance(mid) > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    };
    let (a, b) = (centred(&horizontal), centred(&vertical));
    let pos = [
        centre[0] + a * right[0] + b * up[0] - t * f[0],
        centre[1] + a * right[1] + b * up[1] - t * f[1],
        centre[2] + a * right[2] + b * up[2] - t * f[2],
    ];

    let camera = PanoramaCamera {
        pos: [round6(pos[0]), round6(pos[1]), round6(pos[2])],
        // Re-derived from the view vector in the render-plan convention rather
        // than restated, so the two can never drift.
        yaw_deg: round6((-f[2]).atan2(f[0]).to_degrees()),
        pitch_deg: round6(
            (-f[1])
                .atan2((f[0] * f[0] + f[2] * f[2]).sqrt())
                .to_degrees(),
        ),
        fov_deg,
    };
    // Measured from the camera as emitted (rounded), so it describes the bytes.
    let (x0, x1, y0, y1) = projected_extent(&corners, camera.pos, view);
    let span = SubjectSpan {
        width: ((x1.min(tan_h) - x0.max(-tan_h)) / (2.0 * tan_h)).clamp(0.0, 1.0),
        height: ((y1.min(tan_v) - y0.max(-tan_v)) / (2.0 * tan_v)).clamp(0.0, 1.0),
    };
    (camera, span)
}

/// Options for panorama emission.
#[derive(Debug, Clone)]
pub struct PanoramaOptions {
    /// Path Chunky should load the delve world from. Chunky resolves it against
    /// the rendering process's working directory, so the CLI hands an absolute
    /// path it has checked holds a world save.
    pub world_path: String,
    pub width: u32,
    pub height: u32,
    /// Path-tracing sample target ([`DEFAULT_SPP`] for release art).
    pub spp_target: u32,
    pub bearing: Bearing,
    /// Degrees below horizontal, within [`PITCH_RANGE`].
    pub pitch_deg: u8,
    /// Vertical field of view, degrees, within [`FOV_RANGE`].
    pub fov_deg: u8,
    /// What the camera is fitted to.
    pub subject: Subject,
}

/// What a panorama frames.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Subject {
    /// The placed areas: the layout AABB.
    #[default]
    Layout,
    /// The building the named anchors stand in: their cells' horizontal span, at
    /// the layout's full height.
    Anchors(Vec<String>),
}

impl Subject {
    /// Parse `layout` or `anchor/<a>[,anchor/<b>…]`.
    pub fn parse(spec: &str) -> Result<Subject, String> {
        if spec == "layout" {
            return Ok(Subject::Layout);
        }
        let names: Vec<String> = spec
            .split(',')
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();
        if names.is_empty() || names.iter().any(|n| !n.starts_with("anchor/")) {
            return Err(format!(
                "`{spec}` is not a subject: `layout`, or anchors by their full names \
                 (`anchor/stop-gate,anchor/stop-kitchen`)"
            ));
        }
        Ok(Subject::Anchors(names))
    }
}

/// The anchors a build resolved, from `creator-datapack/layout.json`: full name →
/// world cell.
pub fn resolved_anchors(layout_json: &[u8]) -> Result<Vec<(String, [i32; 3])>, Diagnostic> {
    #[derive(serde::Deserialize)]
    struct Layout {
        anchors: Vec<Anchor>,
    }
    #[derive(serde::Deserialize)]
    struct Anchor {
        id: String,
        pos: [i32; 3],
    }
    let layout: Layout = serde_json::from_slice(layout_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse layout.json: {e}")))?;
    Ok(layout.anchors.into_iter().map(|a| (a.id, a.pos)).collect())
}

/// The box a subject names, over a plan's layout and a build's resolved anchors.
///
/// Anchors name *where on the plan* the building is — a body stands in every
/// room of it — and never how tall it is, because nobody stands on a roof ridge.
/// So the box is the anchors' horizontal span, from the layout's floor to its top.
/// The walls outside the outermost anchors are the frame's margin to hold.
pub fn subject_box(
    subject: &Subject,
    layout: ([i32; 3], [i32; 3]),
    anchors: &[(String, [i32; 3])],
) -> Result<([i32; 3], [i32; 3]), Diagnostic> {
    let names = match subject {
        Subject::Layout => return Ok(layout),
        Subject::Anchors(names) => names,
    };
    let mut lo = [i32::MAX, layout.0[1], i32::MAX];
    let mut hi = [i32::MIN, layout.1[1], i32::MIN];
    for name in names {
        let (_, pos) = anchors.iter().find(|(id, _)| id == name).ok_or_else(|| {
            Diagnostic::error(
                DW_INPUT,
                format!(
                    "the subject names `{name}`, which this build resolved no anchor for. \
                     Resolved: {}",
                    if anchors.is_empty() {
                        "none".to_string()
                    } else {
                        anchors
                            .iter()
                            .map(|(id, _)| id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    }
                ),
            )
        })?;
        for a in [0, 2] {
            lo[a] = lo[a].min(pos[a]);
            hi[a] = hi[a].max(pos[a]);
        }
    }
    Ok((lo, hi))
}

impl Default for PanoramaOptions {
    fn default() -> Self {
        PanoramaOptions {
            world_path: "world".to_string(),
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            spp_target: DEFAULT_SPP,
            bearing: Bearing::Se,
            pitch_deg: DEFAULT_PITCH_DEG,
            fov_deg: FOV_DEG as u8,
            subject: Subject::Layout,
        }
    }
}

/// One emitted panorama scene.
#[derive(Debug, Clone, PartialEq)]
pub struct Panorama {
    /// `<stem>.json`; the stem is also the Chunky scene `name`.
    pub file_name: String,
    pub bytes: Vec<u8>,
    /// The subject box the camera was solved for.
    pub subject: ([i32; 3], [i32; 3]),
    /// How much of the frame that box occupies from the emitted camera.
    pub span: SubjectSpan,
    /// The solved camera, in the showcase record's own format
    /// ([`crate::compiler::view::camera::Camera`]) — so a creator keeps or refines
    /// this frame by copying it into `design/cameras.json`.
    pub record: camera::Camera,
}

/// Emit the panorama scene for a `render-plan.json`. `anchors` is the build's
/// resolved anchors ([`resolved_anchors`]), read only by an anchor subject.
/// Byte-deterministic like [`crate::compiler::view::scene`].
pub fn panorama_from_plan(
    plan_json: &[u8],
    anchors: &[(String, [i32; 3])],
    opts: &PanoramaOptions,
) -> Result<Panorama, Diagnostic> {
    let plan = scene::parse_plan(plan_json)?;
    scene::plan_sky(&plan)?;
    if opts.width == 0 || opts.height == 0 {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "a {}×{} frame has no pixels; give the panorama a width and a height",
                opts.width, opts.height
            ),
        ));
    }
    if !PITCH_RANGE.contains(&opts.pitch_deg) {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "pitch {}° is outside {}..={}°",
                opts.pitch_deg,
                PITCH_RANGE.start(),
                PITCH_RANGE.end()
            ),
        ));
    }
    if !FOV_RANGE.contains(&opts.fov_deg) {
        return Err(Diagnostic::error(
            DW_INPUT,
            format!(
                "field of view {}° is outside {}..={}°",
                opts.fov_deg,
                FOV_RANGE.start(),
                FOV_RANGE.end()
            ),
        ));
    }
    // The subject is what the creator names — the placed areas by default. The
    // ground a horizon built around them is loaded (chunk list, Y clip) but never
    // framed — see the module docs.
    let subject = subject_box(
        &opts.subject,
        (plan.layout_aabb.min, plan.layout_aabb.max),
        anchors,
    )?;
    let aspect = f64::from(opts.width) / f64::from(opts.height);
    let (cam, span) = frame(
        subject.0,
        subject.1,
        opts.bearing,
        opts.pitch_deg,
        f64::from(opts.fov_deg),
        aspect,
    );
    // File name == scene `name`, so Chunky's own save lands back on this file and
    // its caches share the stem (see `scene::scene_file_stem`). Every choice that
    // makes a different picture is in the stem: the bearing, the pitch, and — when
    // not the default — the field of view and a named subject.
    let mut shot = format!("panorama_{}_{}", opts.bearing.name(), opts.pitch_deg);
    if f64::from(opts.fov_deg) != FOV_DEG {
        shot.push_str(&format!("_fov{}", opts.fov_deg));
    }
    if opts.subject != Subject::Layout {
        shot.push_str("_anchors");
    }
    let stem = scene::scene_file_stem(&plan.campaign_id, &shot);

    let frame = Frame {
        pos: cam.pos,
        plan_yaw_deg: cam.yaw_deg,
        pitch_deg: cam.pitch_deg,
        fov_deg: cam.fov_deg,
        // A panorama is the world as the path tracer measures it.
        exposure: 1.0,
        width: opts.width,
        height: opts.height,
        spp: opts.spp_target,
    };
    let scene = camera::world_scene(&plan, &stem, &frame, &opts.world_path)?;

    Ok(Panorama {
        file_name: format!("{stem}.json"),
        bytes: scene.to_bytes()?,
        subject,
        span,
        record: camera::Camera {
            answers: String::new(),
            exposure: 1.0,
            fov: cam.fov_deg,
            height: opts.height,
            name: shot.replace('_', "-"),
            pitch: cam.pitch_deg,
            pos: cam.pos,
            source: camera::Source::Estimated,
            spp: opts.spp_target,
            width: opts.width,
            yaw: scene::round6(camera::minecraft_yaw(cam.yaw_deg)),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 64×16×64 layout sitting on y=60, centred on (32, 68, 32).
    const MIN: [i32; 3] = [0, 60, 0];
    const MAX: [i32; 3] = [63, 75, 63];
    const WIDE: f64 = 16.0 / 9.0;

    /// The camera basis re-derived from the emitted yaw/pitch, independently of
    /// [`basis`].
    fn view_of(c: &PanoramaCamera) -> ([f64; 3], [f64; 3], [f64; 3]) {
        let p = c.pitch_deg.to_radians();
        let yaw = c.yaw_deg.to_radians();
        let f = [yaw.cos() * p.cos(), -p.sin(), -yaw.sin() * p.cos()];
        let right = {
            let n = (f[2] * f[2] + f[0] * f[0]).sqrt();
            [-f[2] / n, 0.0, f[0] / n]
        };
        (f, right, cross(right, f))
    }

    #[test]
    fn se_camera_stands_over_the_south_east_corner_looking_down_at_45() {
        let (c, _) = frame(MIN, MAX, Bearing::Se, 45, FOV_DEG, 1.0);
        assert_eq!(c.pitch_deg, 45.0);
        assert_eq!(c.yaw_deg, 135.0, "SE camera looks north-west");
        assert_eq!(c.fov_deg, FOV_DEG);
        assert!(c.pos[0] > 32.0 && c.pos[2] > 32.0, "{c:?}");
        assert!(c.pos[1] > 75.0, "camera above the layout: {c:?}");
        // Equal offsets on both horizontal axes — a true corner bearing.
        assert!((c.pos[0] - c.pos[2]).abs() < 1e-6, "{c:?}");
    }

    #[test]
    fn every_bearing_looks_back_at_the_layout() {
        let mut seen = 0;
        for (b, yaw, sx, sz) in [
            (Bearing::Se, 135.0, 1.0, 1.0),
            (Bearing::Sw, 45.0, -1.0, 1.0),
            (Bearing::Ne, -135.0, 1.0, -1.0),
            (Bearing::Nw, -45.0, -1.0, -1.0),
            (Bearing::N, -90.0, 0.0, -1.0),
            (Bearing::E, 180.0, 1.0, 0.0),
            (Bearing::S, 90.0, 0.0, 1.0),
            (Bearing::W, 0.0, -1.0, 0.0),
        ] {
            let (c, _) = frame(MIN, MAX, b, 30, FOV_DEG, WIDE);
            assert_eq!(c.yaw_deg.abs(), f64::abs(yaw), "{b:?}: {c:?}");
            assert_eq!(c.pitch_deg, 30.0, "{b:?}");
            if sx != 0.0 {
                assert!((c.pos[0] - 32.0) * sx > 0.0, "{b:?} x side: {c:?}");
            }
            if sz != 0.0 {
                assert!((c.pos[2] - 32.0) * sz > 0.0, "{b:?} z side: {c:?}");
            }
            assert_eq!(b.is_corner(), sx != 0.0 && sz != 0.0, "{b:?}");
            seen += 1;
        }
        assert_eq!(seen, Bearing::ALL.len(), "every bearing was examined");
    }

    /// The framing promise, at every bearing, several pitches and both a square
    /// and a wide frame: every corner of the subject lands inside the frame; the
    /// tightest corner sits exactly at the declared margin on the axis that
    /// binds; and the slack is shared — the projected box is centred.
    #[test]
    fn the_subject_fits_the_frame_with_the_declared_margin_and_is_centred() {
        let mut judged = 0;
        for b in Bearing::ALL {
            for pitch in [5u8, 15, 30, 45, 70, 85] {
                for aspect in [1.0, WIDE, 0.5] {
                    let (c, span) = frame(MIN, MAX, b, pitch, FOV_DEG, aspect);
                    let (_, corners) = box_corners(MIN, MAX);
                    let (f, right, up) = view_of(&c);
                    let tan_v = (c.fov_deg.to_radians() / 2.0).tan();
                    let tan_h = tan_v * aspect;
                    let mut worst = 0.0f64;
                    let (mut x0, mut x1, mut y0, mut y1) = (1e9f64, -1e9f64, 1e9f64, -1e9f64);
                    for corner in corners {
                        let rel = [
                            corner[0] - c.pos[0],
                            corner[1] - c.pos[1],
                            corner[2] - c.pos[2],
                        ];
                        let depth = dot(rel, f);
                        assert!(depth > 0.0, "corner behind the camera: {corner:?}");
                        let x = dot(rel, right) / depth;
                        let y = dot(rel, up) / depth;
                        worst = worst.max((x.abs() / tan_h).max(y.abs() / tan_v));
                        (x0, x1, y0, y1) = (x0.min(x), x1.max(x), y0.min(y), y1.max(y));
                    }
                    let ctx = format!("{b:?} pitch {pitch} aspect {aspect}");
                    assert!(worst < 1.0, "{ctx}: subject overflows the frame: {worst}");
                    assert!(
                        (worst - 1.0 / MARGIN).abs() < 1e-4,
                        "{ctx}: framing is not the declared margin: {worst}"
                    );
                    assert!(
                        ((x0 + x1) / 2.0).abs() / tan_h < 1e-4
                            && ((y0 + y1) / 2.0).abs() / tan_v < 1e-4,
                        "{ctx}: projected subject is off-centre: x[{x0},{x1}] y[{y0},{y1}]"
                    );
                    // The reported span is the same measurement.
                    assert!(
                        (span.width - (x1 - x0) / (2.0 * tan_h)).abs() < 1e-6,
                        "{ctx}"
                    );
                    assert!(
                        (span.height - (y1 - y0) / (2.0 * tan_v)).abs() < 1e-6,
                        "{ctx}"
                    );
                    judged += 1;
                }
            }
        }
        assert_eq!(judged, Bearing::ALL.len() * 6 * 3);
    }

    /// The subject boxes of the places this rule has to mean the same thing on —
    /// the ocean island, the valley castle, and the gallery's point on each
    /// horizon — each cover at least a third of the default frame from every
    /// corner bearing at the default pitch. A third is what the island's accepted
    /// release art measures: its subject's bounding rectangle over its frame.
    #[test]
    fn a_built_place_covers_a_third_of_the_default_frame_from_every_corner() {
        let aspect = f64::from(DEFAULT_WIDTH) / f64::from(DEFAULT_HEIGHT);
        let subjects: [(&str, [i32; 3], [i32; 3]); 5] = [
            ("nobodys-cave-island", [-8, 60, -72], [264, 87, 43]),
            ("doune-castle-tour", [0, 60, 0], [103, 115, 119]),
            ("gallery ocean-horizon", [0, 60, 0], [519, 69, 30]),
            ("gallery site-plan (valley)", [3, 48, 3], [55, 87, 62]),
            ("gallery void-horizon", [0, 64, 0], [262, 73, 30]),
        ];
        let mut judged = 0;
        for (what, min, max) in subjects {
            for b in Bearing::ALL.into_iter().filter(|b| b.is_corner()) {
                let (_, span) = frame(min, max, b, DEFAULT_PITCH_DEG, FOV_DEG, aspect);
                assert!(
                    span.fill() >= 1.0 / 3.0,
                    "{what} from {b:?}: the subject covers {:.3} of the frame ({:.3} × {:.3})",
                    span.fill(),
                    span.width,
                    span.height
                );
                // One axis always binds at the margin.
                assert!(
                    (span.width.max(span.height) - 1.0 / MARGIN).abs() < 1e-4,
                    "{what} from {b:?}: no axis binds: {span:?}"
                );
                judged += 1;
            }
        }
        assert_eq!(judged, 5 * 4);
    }

    #[test]
    fn a_degenerate_layout_still_gets_a_usable_camera() {
        let (c, _) = frame([0, 64, 0], [0, 64, 0], Bearing::Se, 45, FOV_DEG, 1.0);
        let d = ((c.pos[0] - 0.5).powi(2) + (c.pos[1] - 64.5).powi(2) + (c.pos[2] - 0.5).powi(2))
            .sqrt();
        assert!(d >= MIN_DISTANCE - 1e-6, "camera inside the block: {c:?}");
    }

    const MINI: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-mini.json");
    const OCEAN: &[u8] = include_bytes!("../../../tests/fixtures/view/render-plan-ocean.json");

    /// A valley plan: a small layout inside a landform several times its size.
    const VALLEY: &[u8] =
        br#"{"campaign_id":"vale","layout_aabb":{"min":[0,60,0],"max":[31,79,31]},
          "horizon":{"kind":"valley","gap_floor_y":63,"rim_height":40,
                     "extent":{"min":[-80,59,-80],"max":[111,110,111]}},
          "sky":{"time":"day","daytime_ticks":1000},"shots":[]}"#;

    /// The subject is the placed areas on a valley too: the camera is the one
    /// solved for the layout box alone, while the chunk list still loads every
    /// column of the landform so the place does not stand in a void.
    #[test]
    fn a_valley_frames_the_layout_and_loads_the_landform() {
        let p = panorama_from_plan(VALLEY, &[], &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
        let aspect = f64::from(DEFAULT_WIDTH) / f64::from(DEFAULT_HEIGHT);
        let (want, span) = frame([0, 60, 0], [31, 79, 31], Bearing::Se, 45, FOV_DEG, aspect);
        assert_eq!(v["camera"]["position"]["x"], serde_json::json!(want.pos[0]));
        assert_eq!(v["camera"]["position"]["y"], serde_json::json!(want.pos[1]));
        assert_eq!(v["camera"]["position"]["z"], serde_json::json!(want.pos[2]));
        assert_eq!(p.span, span);
        assert_eq!(p.subject, ([0, 60, 0], [31, 79, 31]));
        let chunks: Vec<[i64; 2]> = v["chunkList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| [c[0].as_i64().unwrap(), c[1].as_i64().unwrap()])
            .collect();
        let mut columns = 0;
        for cx in (-80i64).div_euclid(16)..=111i64.div_euclid(16) {
            for cz in (-80i64).div_euclid(16)..=111i64.div_euclid(16) {
                assert!(
                    chunks.contains(&[cx, cz]),
                    "landform chunk [{cx},{cz}] not loaded"
                );
                columns += 1;
            }
        }
        assert_eq!(columns, 12 * 12);
    }

    /// The sun is the campaign's hour and nothing else — the same bytes `scene`
    /// writes for the same plan, whatever bearing the frame is shot from.
    #[test]
    fn the_sun_is_the_declared_hour_and_the_bearing_never_moves_it() {
        let expected = scene::sun_at(12000); // the mini fixture declares dusk
        let mut seen = 0;
        for b in Bearing::ALL {
            let opts = PanoramaOptions {
                bearing: b,
                ..Default::default()
            };
            let p = panorama_from_plan(MINI, &[], &opts).unwrap();
            let v: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
            assert_eq!(v["sun"]["altitude"], serde_json::json!(expected.altitude));
            assert_eq!(v["sun"]["azimuth"], serde_json::json!(expected.azimuth));
            seen += 1;
        }
        assert_eq!(seen, Bearing::ALL.len(), "every bearing was examined");
    }

    /// A plan with no hour is refused rather than rendered under Chunky's own
    /// midday default — the panorama half of the rule `scene` enforces.
    #[test]
    fn a_panorama_of_a_plan_with_no_hour_is_refused() {
        let no_sky = br#"{"campaign_id":"c","layout_aabb":{"min":[0,64,0],"max":[1,65,1]},
          "shots":[]}"#;
        let err = panorama_from_plan(no_sky, &[], &PanoramaOptions::default()).unwrap_err();
        assert_eq!(err.code, DW_INPUT, "{err:?}");
    }

    #[test]
    fn a_frame_with_no_pixels_or_a_pitch_out_of_range_is_refused() {
        let refused = [
            PanoramaOptions {
                width: 0,
                ..Default::default()
            },
            PanoramaOptions {
                height: 0,
                ..Default::default()
            },
            PanoramaOptions {
                pitch_deg: 4,
                ..Default::default()
            },
            PanoramaOptions {
                pitch_deg: 86,
                ..Default::default()
            },
        ];
        for opts in &refused {
            let err = panorama_from_plan(MINI, &[], opts).unwrap_err();
            assert_eq!(err.code, DW_INPUT, "{opts:?}");
        }
        for pitch in [5u8, 85] {
            let opts = PanoramaOptions {
                pitch_deg: pitch,
                ..Default::default()
            };
            assert!(
                panorama_from_plan(MINI, &[], &opts).is_ok(),
                "pitch {pitch}"
            );
        }
    }

    #[test]
    fn the_frame_size_is_the_one_asked_for() {
        let p = panorama_from_plan(MINI, &[], &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
        assert_eq!(v["width"], serde_json::json!(DEFAULT_WIDTH));
        assert_eq!(v["height"], serde_json::json!(DEFAULT_HEIGHT));
        let opts = PanoramaOptions {
            width: 1024,
            height: 1024,
            ..Default::default()
        };
        let p = panorama_from_plan(MINI, &[], &opts).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
        assert_eq!(v["width"], serde_json::json!(1024));
        assert_eq!(v["height"], serde_json::json!(1024));
    }

    #[test]
    fn the_water_plane_is_raised_only_for_an_ocean_horizon() {
        let ocean = panorama_from_plan(OCEAN, &[], &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&ocean.bytes).unwrap();
        assert_eq!(v["waterWorldEnabled"], serde_json::json!(true));
        assert_eq!(v["waterWorldHeight"], serde_json::json!(62.875));
        assert_eq!(v["waterWorldHeightOffsetEnabled"], serde_json::json!(false));
        assert_eq!(v["waterWorldClipEnabled"], serde_json::json!(true));

        let void = panorama_from_plan(MINI, &[], &PanoramaOptions::default()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&void.bytes).unwrap();
        assert!(v.get("waterWorldEnabled").is_none());
        assert!(v.get("waterWorldHeight").is_none());
    }

    #[test]
    fn bearing_and_pitch_name_the_file_and_the_scene_identically() {
        let mut seen = 0;
        for b in Bearing::ALL {
            for pitch in [30u8, 45] {
                let opts = PanoramaOptions {
                    bearing: b,
                    pitch_deg: pitch,
                    ..Default::default()
                };
                let p = panorama_from_plan(OCEAN, &[], &opts).unwrap();
                // One stem for the file, the Chunky scene `name`, its caches and
                // the rendered PNG.
                assert_eq!(
                    p.file_name,
                    format!("isle_panorama_{}_{pitch}.json", b.name())
                );
                let v: serde_json::Value = serde_json::from_slice(&p.bytes).unwrap();
                assert_eq!(v["name"], format!("isle_panorama_{}_{pitch}", b.name()));
                seen += 1;
            }
        }
        assert_eq!(seen, Bearing::ALL.len() * 2);
    }

    #[test]
    fn emission_is_byte_deterministic() {
        let a = panorama_from_plan(OCEAN, &[], &PanoramaOptions::default()).unwrap();
        let b = panorama_from_plan(OCEAN, &[], &PanoramaOptions::default()).unwrap();
        assert_eq!(a, b);
        assert!(a.bytes.ends_with(b"\n"));
    }

    #[test]
    fn malformed_plan_json_is_dw0721() {
        let err = panorama_from_plan(b"not json", &[], &PanoramaOptions::default()).unwrap_err();
        assert_eq!(err.code, DW_INPUT, "expected DW0721: {err:?}");
    }
}
