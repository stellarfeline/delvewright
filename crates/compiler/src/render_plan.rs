//! `render-plan.json` emission (spec-0003 visual tier / spec-0007 rendering infra).
//!
//! The compiler knows every world coordinate, so the visual-tier camera plan is
//! **computed, not guessed**: one deterministic shot list derived from the layout,
//! each shot carrying a camera (pos + orientation) and a machine-generated
//! `expect` checklist derived from the DSL. `delvec scene` turns this into
//! Chunky scenes; the generation-time vision agent reviews each render against its
//! `expect` strings (findings are DSL-addressable, same shape as playtest notes).
//!
//! ## Camera convention (shared with `delve-render`)
//!
//! `pos`/`look_at` are world coordinates (block-centre floats). `yaw`/`pitch` are
//! **degrees**, aimed from `pos` at `look_at`:
//! - `yaw = atan2(-dz, dx)`: `0` faces +X (east), `90` faces −Z (north), `180`
//!   faces −X (west), `-90`/`270` faces +Z (south).
//! - `pitch = atan2(-dy, hypot(dx, dz))`: positive looks **down**, `0` is level.
//!
//! This matches the camera axes the spike-render-fidelity spike verified for
//! Chunky (`yaw ≈ π/2` faces −Z, positive pitch = down); `delvec scene`
//! converts these degrees to Chunky's radians directly.
//!
//! ## Determinism
//!
//! Shots are emitted in a fixed order (spawn → per-area interiors+seams → NPCs →
//! interacts → gates → **player-POV**), every list is plan-ordered or sorted, and
//! there is no RNG or wall-clock input — so `render-plan.json` rides the ADR-0006
//! double-build byte-identity gate like every other output.
//!
//! ## Every camera's eye cell is proven clear (`DW0724`)
//!
//! A camera whose eye cell holds a block renders the inside of that block, and a
//! picture of the inside of a block is indistinguishable from a picture of a
//! featureless room. That is a fact about **a camera**, so it holds for every kind
//! here and not just for the one that needed it first — `Shots::push` is the
//! single door a shot enters through, and [`render_plan`] is the only constructor
//! of a plan document, so it takes the assembled world and proves every eye
//! before there is a document at all.
//!
//! Six of the seven kinds place their camera at a fixed stand-off from the thing
//! they frame, and a fixed offset into authored geometry lands inside a block
//! about a quarter of the time. Measured over every campaign and fixture that
//! builds: **204 of 752 cameras** — 144 seam, 38 gate, 16 NPC, 6 interact — stood
//! inside a block, and every one of the 27 campaigns had at least one. So the
//! stand-off is a *preference*, not a position: a camera whose own cell holds a
//! block stands instead at the furthest clear point on its own sight line
//! ([`crate::camera::stand_in_open_air`], the mechanism `delvec snapshot --at`
//! already used and kept to itself), and says so (`camera.requested_pos` +
//! `camera.standoff`), because a displaced camera is invisible in its own frame.
//! It yields to that one fact and nothing else — an interior shot's eye is
//! deliberately outside the room and is not pulled through the roof it is
//! looking past.
//!
//! `pov` is the exception, and the object decides it rather than the author: that
//! eye IS the player's, 1.62 over a DW0314-proven-standable waypoint, so moving
//! it would stop it being the player's view. It is never moved, and a violation
//! there stays a hard `DW0724` against the derivation.
//!
//! ## Player-POV shots (the player's own eye)
//!
//! The other kinds are overhead/orbit cameras — useful for assembly review but not
//! for judging what the *player* sees, the owner's standing concern. The `pov` kind
//! closes that gap: a first-person camera at eye height ([`EYE_HEIGHT`] = 1.62) on
//! every corner-thinned critical-path waypoint (the same [`crate::waypoints::thin`]
//! list the harness replays), looking along the walk toward the next waypoint and,
//! at each leg's final waypoint, toward the objective anchor it arrives at. Each
//! shot carries its leg index, the objective it serves, and a one-sentence machine
//! `expect` line composed from campaign data (area name + objective/anchor names +
//! hint) — the (image ↔ expect) pair a vision model reviews. Every POV eye sits at
//! the eye-height of a DW0314-proven-standable waypoint, so the camera is provably
//! in open air; the DW0724 self-check ([`crate::nav::verify_camera_eyes`]) enforces
//! it structurally, for this kind and every other.
//!
//! ## `horizon` (the render layer is told, never guesses)
//!
//! A campaign with `horizon: ocean` (spec-0013) ships a world save holding only
//! the chunks its layout occupies — the sea around the island is the level
//! generator's, and a renderer loading that save sees void past the shoreline.
//! The plan therefore states the generator fact ([`horizon_fact`]:
//! `{"kind": "ocean", "sea_level": 62}`) so `delve-render` can raise Chunky's
//! ambient water plane at exactly the compiler's datum. `horizon: void` (the
//! default) emits no key, keeping every existing plan byte-identical.
//!
//! ## `lighting` stamp (declared-dark areas stay reviewable)
//!
//! POV and interior shots carry a `lighting` stamp derived **purely from the
//! shot's area's stage-1 declarations** ([`area_lighting_stamp`]): `lighting`
//! declared (the relight pass guarantees `min_light`) → `{"profile": "lit"}`;
//! only `mitigation: "night-vision"` declared → `{"profile": "dark",
//! "mitigation": "night-vision"}`; both → lit profile plus the mitigation;
//! neither → **no stamp key at all**, so campaigns without lighting declarations
//! stay byte-identical. The stamp is pure metadata for the render layer: a
//! declared-dark scene renders pure black in an honest path tracer (the first
//! Chunky run proved exposure boosts cannot reveal a sealed cave — no light means
//! amplified noise), so `delvec scene` uses the stamp to apply its
//! documented night-vision review emulation to exactly those shots and no others.

use delvewright_dsl::{
    AreaMitigation, Campaign, Diagnostic, HorizonBase, LightingProfile, Objective,
};
use serde_json::{Value, json};

use crate::failure::Failure;
use crate::nav::{CameraEye, LegRoute, World};
use crate::plan::{Plan, ResolvedAnchor, Step};
use crate::registry::PrefabRegistry;

/// Player eye height above the standing cell's floor (vanilla: 1.62 blocks). The
/// first-person camera sits here so a render frames exactly what the player sees.
///
/// The metrics table (spec-0049 §2) is the one definition. A camera that framed
/// a different eye from the one the navigation model proves standability for
/// would be photographing a body that is not the player's, and before the table
/// there was nothing that could have gone red about it.
pub use delvewright_dsl::metrics::PLAYER_EYE_HEIGHT as EYE_HEIGHT;

/// First-person field of view, degrees (vanilla default ~70°).
pub const POV_FOV_DEG: f64 = 70.0;

/// Unit facing vector for a Minecraft facing keyword (north = −Z).
fn facing_vec(facing: Option<&str>) -> [f64; 3] {
    match facing {
        Some("north") => [0.0, 0.0, -1.0],
        Some("south") => [0.0, 0.0, 1.0],
        Some("east") => [1.0, 0.0, 0.0],
        Some("west") => [-1.0, 0.0, 0.0],
        _ => [0.0, 0.0, 1.0], // default south, matching the summon yaw default
    }
}

/// Aim a camera at `look_at` from `pos`; returns `(yaw_deg, pitch_deg)` in the
/// documented convention.
fn aim(pos: [f64; 3], look_at: [f64; 3]) -> (f64, f64) {
    let dx = look_at[0] - pos[0];
    let dy = look_at[1] - pos[1];
    let dz = look_at[2] - pos[2];
    let yaw = (-dz).atan2(dx).to_degrees();
    let horiz = (dx * dx + dz * dz).sqrt();
    let pitch = (-dy).atan2(horiz).to_degrees();
    (round3(yaw), round3(pitch))
}

/// Round to 3 decimals so float formatting is stable across platforms (the
/// determinism gate compares bytes; a repeatable rounding avoids libm ulp drift).
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn centre(pos: [i32; 3]) -> [f64; 3] {
    [pos[0] as f64 + 0.5, pos[1] as f64, pos[2] as f64 + 0.5]
}

fn camera(pos: [f64; 3], look_at: [f64; 3]) -> Value {
    let (yaw, pitch) = aim(pos, look_at);
    json!({
        "pos": [round3(pos[0]), round3(pos[1]), round3(pos[2])],
        "yaw": yaw,
        "pitch": pitch,
        "look_at": [round3(look_at[0]), round3(look_at[1]), round3(look_at[2])],
    })
}

/// The thin (wall-normal) axis of an inclusive `[from,to]` region, or `None` when
/// it is not planar (no axis with `from == to`); prefers the first such axis.
fn thin_axis(from: [i32; 3], to: [i32; 3]) -> Option<usize> {
    (0..3).find(|&a| from[a] == to[a])
}

fn region_centre(from: [i32; 3], to: [i32; 3]) -> [f64; 3] {
    [
        (from[0] + to[0]) as f64 / 2.0 + 0.5,
        (from[1] + to[1]) as f64 / 2.0 + 1.0,
        (from[2] + to[2]) as f64 / 2.0 + 0.5,
    ]
}

/// Resolve the campaign spawn as `(area, pos, facing)` — the first area to
/// resolve an entry anchor, through the compiler's own resolver
/// (`Plan::entry_point_facing`) rather than a literal name. What an entry
/// anchor is called is not this planner's question, and framing the cell the
/// party arrives at is the whole job here.
fn spawn_of(plan: &Plan) -> Option<(String, [i32; 3], Option<String>)> {
    for area in &plan.areas {
        if let Some((pos, facing)) = plan.entry_point_facing(&area.area_id) {
            return Some((area.area_id.clone(), pos, facing));
        }
    }
    None
}

// ---- player-POV shots (spec-0003 visual tier: the player's own eye) ----------

/// One planned first-person player-POV shot: a camera at eye height standing on a
/// proven critical-path waypoint, looking the way the player walks. This is the
/// tier that lets a vision review judge the scene as the player actually sees it —
/// not from the overhead/orbit cameras of the other shot kinds.
#[derive(Debug, Clone, PartialEq)]
pub struct PovShot {
    /// Stable shot id (`pov/leg{L}/wp{W}`).
    pub id: String,
    /// Destination area id of the leg.
    pub area: String,
    /// Critical-path leg index (order the player walks the legs).
    pub leg: usize,
    /// Waypoint index within the leg (0 = leg start).
    pub wp: usize,
    /// Objective id this leg walks toward, if resolvable.
    pub objective: Option<String>,
    /// The standing (feet) cell — a DW0314-proven-standable waypoint.
    pub standing_cell: [i32; 3],
    /// Camera eye world position (feet + [`EYE_HEIGHT`], block-centred).
    pub eye: [f64; 3],
    /// World point the camera looks at (next waypoint's eye, or — at the leg's
    /// final waypoint — the objective anchor it arrives at).
    pub look_at: [f64; 3],
    /// One-sentence machine description of what this frame should show, composed
    /// from campaign data (area name + objective/anchor names + hint).
    pub expect_line: String,
    /// Structural expect checks appended after the description.
    pub extra_expect: Vec<String>,
}

impl PovShot {
    /// The integer block the eye sits in — the cell the DW0724 clear-eye self-check
    /// verifies is unoccupied.
    pub fn eye_cell(&self) -> [i32; 3] {
        eye_cell(self.eye)
    }
}

/// The integer block a camera eye sits in. One derivation, shared by every shot
/// kind, so "which cell is this camera in" cannot mean two things.
pub fn eye_cell(eye: [f64; 3]) -> [i32; 3] {
    [
        eye[0].floor() as i32,
        eye[1].floor() as i32,
        eye[2].floor() as i32,
    ]
}

/// Plan the deterministic first-person POV shot list from the DW0311-proven
/// critical-path routes: one camera at eye height on every corner-thinned waypoint
/// (the same thinning the harness replays — turns + endpoints, never every cell),
/// oriented along the walk toward the next waypoint, and — at each leg's final
/// waypoint — toward the objective anchor it arrives at. Pure and deterministic
/// (route order → waypoint order; no RNG/clock), so it rides the ADR-0006 gate.
pub fn pov_shots(plan: &Plan, routes: &[LegRoute]) -> Vec<PovShot> {
    let mut shots = Vec::new();
    for (leg, route) in routes.iter().enumerate() {
        let wps = crate::waypoints::thin(&route.cells, &route.use_gates);
        if wps.is_empty() {
            continue;
        }
        let objective = objective_at_step(plan, route.to_step);
        let ctx = leg_context(plan, route, objective.as_deref());
        let last = wps.len() - 1;
        for (wp, &cell) in wps.iter().enumerate() {
            let eye = [
                cell[0] as f64 + 0.5,
                cell[1] as f64 + EYE_HEIGHT,
                cell[2] as f64 + 0.5,
            ];
            let (look_at, arriving) = if wp < last {
                let n = wps[wp + 1];
                (
                    [
                        n[0] as f64 + 0.5,
                        n[1] as f64 + EYE_HEIGHT,
                        n[2] as f64 + 0.5,
                    ],
                    false,
                )
            } else {
                // Leg's final waypoint: frame the objective anchor (raw `to`) at
                // body height. The snapped standing waypoint often sits directly on
                // or above the anchor, so aiming straight at it looks vertically at
                // the floor; when the anchor is within ~2 blocks horizontally, keep
                // the approach heading (previous → last waypoint) and aim a few
                // blocks ahead at the anchor's height — a forward arrival frame with
                // the objective in view, never a straight-down floor shot.
                let t = route.to;
                let anchor = [t[0] as f64 + 0.5, t[1] as f64 + 1.0, t[2] as f64 + 0.5];
                let horiz = ((anchor[0] - eye[0]).powi(2) + (anchor[2] - eye[2]).powi(2)).sqrt();
                if horiz >= 2.0 {
                    (anchor, true)
                } else {
                    let dir = approach_heading(&wps, anchor, eye);
                    (
                        [eye[0] + dir[0] * 4.0, anchor[1], eye[2] + dir[1] * 4.0],
                        true,
                    )
                }
            };
            let compass = compass_toward(eye, look_at);
            let expect_line = compose_pov_expect(&ctx, compass, arriving);
            shots.push(PovShot {
                id: format!("pov/leg{leg}/wp{wp}"),
                area: ctx.area_id.clone(),
                leg,
                wp,
                objective: objective.clone(),
                standing_cell: cell,
                eye,
                look_at,
                expect_line,
                extra_expect: vec![
                    "camera at standing eye height (1.62) — a first-person view, not overhead or \
                     orbit"
                        .to_string(),
                    "the way ahead is open — no wall or block clipping the near camera".to_string(),
                ],
            });
        }
    }
    shots
}

/// The horizontal unit heading `(dx, dz)` the player faces on arrival: the last
/// walked segment (previous → final waypoint), falling back to eye→anchor, then to
/// east — never a zero vector, so the arrival camera always faces a definite way.
fn approach_heading(wps: &[[i32; 3]], anchor: [f64; 3], eye: [f64; 3]) -> [f64; 2] {
    let candidates = [
        wps.len()
            .checked_sub(2)
            .map(|p| {
                let a = wps[p];
                let b = wps[wps.len() - 1];
                [(b[0] - a[0]) as f64, (b[2] - a[2]) as f64]
            })
            .unwrap_or([0.0, 0.0]),
        [anchor[0] - eye[0], anchor[2] - eye[2]],
        [1.0, 0.0],
    ];
    for c in candidates {
        let len = (c[0] * c[0] + c[1] * c[1]).sqrt();
        if len > 1e-6 {
            return [c[0] / len, c[1] / len];
        }
    }
    [1.0, 0.0]
}

/// The objective id whose critical-path step is `step` (inverse of
/// `plan.objective_steps`), or `None` for a non-objective step.
fn objective_at_step(plan: &Plan, step: usize) -> Option<String> {
    plan.objective_steps
        .iter()
        .find(|(_, v)| **v == step)
        .map(|(o, _)| o.clone())
}

/// Resolved leg-destination context for POV expect composition.
struct LegContext {
    area_id: String,
    area_name: String,
    /// Human phrase for what the player walks toward ("Perimedes", "the door").
    target: String,
    /// The objective's one-line hint, if any (trimmed to one clause).
    hint: Option<String>,
}

/// Resolve the destination area name, target phrase, and hint for a leg.
fn leg_context(plan: &Plan, route: &LegRoute, objective: Option<&str>) -> LegContext {
    let c = plan.campaign;
    let step = plan.critical_path.get(route.to_step);
    let (target, area_id) = match step {
        Some(Step::TalkTo { npc_id, .. }) => (npc_name(c, npc_id), plan.npc_area(npc_id)),
        Some(Step::Reach { anchor_id, .. }) => {
            (anchor_phrase(anchor_id), anchor_area(plan, anchor_id))
        }
        Some(Step::Interact { anchor_id, .. }) => {
            (anchor_phrase(anchor_id), anchor_area(plan, anchor_id))
        }
        Some(Step::Collect { .. }) => ("the cache".to_string(), None),
        Some(Step::Kill { .. }) => ("the fight".to_string(), None),
        _ => ("the objective".to_string(), None),
    };
    // Prefer the objective's quest area (authoritative) when known.
    let area_id = objective
        .and_then(|o| objective_area(plan, o))
        .or_else(|| area_id.map(str::to_string))
        .unwrap_or_default();
    let area_name = area_name_of(c, &area_id);
    let hint = objective
        .and_then(|o| find_objective(c, o))
        .and_then(|o| o.hint())
        .map(first_clause);
    LegContext {
        area_id,
        area_name,
        target,
        hint,
    }
}

/// Compose the one-sentence POV description.
fn compose_pov_expect(ctx: &LegContext, compass: &str, arriving: bool) -> String {
    let place = if ctx.area_name.is_empty() {
        String::new()
    } else {
        format!(" in {}", ctx.area_name)
    };
    if arriving {
        let hint = ctx
            .hint
            .as_ref()
            .map(|h| format!(" ({h})"))
            .unwrap_or_default();
        format!(
            "First-person view arriving at {}{place}{hint} — the objective should be ahead in frame.",
            ctx.target
        )
    } else {
        format!(
            "First-person view walking {compass}{place} toward {} — the path ahead should be open.",
            ctx.target
        )
    }
}

/// The dominant cardinal direction from `eye` toward `look_at` (Minecraft: −Z is
/// north). Ties break toward the X axis for determinism.
fn compass_toward(eye: [f64; 3], look_at: [f64; 3]) -> &'static str {
    let dx = look_at[0] - eye[0];
    let dz = look_at[2] - eye[2];
    if dx.abs() >= dz.abs() {
        if dx >= 0.0 { "east" } else { "west" }
    } else if dz >= 0.0 {
        "south"
    } else {
        "north"
    }
}

/// The display name of an NPC, or its id's local part. The render plan is a
/// reviewer artifact, never a text component and never rendered by a client, so
/// authored names appear here as their English source (`l10n::plain`) — a named
/// exclusion under spec-0029, listed in `docs/reference/compiler.md`.
fn npc_name(c: &Campaign, npc_id: &str) -> String {
    c.npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| delvewright_dsl::l10n_plain(&n.name).to_string())
        .unwrap_or_else(|| local_of(npc_id))
}

/// Humanize an anchor id into a phrase ("anchor/door" → "the door").
fn anchor_phrase(anchor_id: &str) -> String {
    let local = local_of(anchor_id).replace(['_', '-'], " ");
    format!("the {local}")
}

/// The area id of a point/gate anchor (first area declaring it), if any.
fn anchor_area<'p>(plan: &'p Plan, anchor_id: &str) -> Option<&'p str> {
    plan.anchors
        .keys()
        .find(|(_, name)| name == anchor_id)
        .map(|(area, _)| area.as_str())
}

/// The quest area id of an objective, via its quest.
fn objective_area(plan: &Plan, obj_id: &str) -> Option<String> {
    let quest_id = plan
        .campaign
        .quests
        .content
        .quests
        .iter()
        .find(|q| q.objectives.iter().any(|o| o.id().as_str() == obj_id))
        .map(|q| q.id.as_str())?;
    plan.quest_area(quest_id).map(str::to_string)
}

/// Look up an objective by id in the stage-5 quests.
fn find_objective<'a>(c: &'a Campaign, obj_id: &str) -> Option<&'a Objective> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find(|o| o.id().as_str() == obj_id)
}

/// The display name of an area, or empty. Reviewer artifact — English source, per
/// [`npc_name`].
fn area_name_of(c: &Campaign, area_id: &str) -> String {
    c.world
        .content
        .areas
        .iter()
        .find(|a| a.id.as_str() == area_id)
        .map(|a| delvewright_dsl::l10n_plain(&a.name).to_string())
        .unwrap_or_default()
}

/// The first clause of a hint (up to the first sentence end), trimmed — keeps the
/// expect line to one sentence. Reviewer artifact, so the hint is read as its
/// English source (`l10n::plain`) — a spec-0029 named exclusion.
fn first_clause(hint: &str) -> String {
    let hint = delvewright_dsl::l10n_plain(hint);
    let end = hint.find(['.', ';', '—']).unwrap_or(hint.len());
    hint[..end].trim().to_string()
}

/// The local (post-`/`) part of an id.
fn local_of(id: &str) -> String {
    id.rsplit('/').next().unwrap_or(id).to_string()
}

// ---- one door into the shot list ------------------------------------------

/// One shot on its way into the plan, as [`Shots::push`] takes it.
///
/// Every kind fills the same fields, so a kind cannot quietly acquire a camera
/// nobody else's rules apply to. In particular there is no way to state a camera
/// position here without the eye cell being recorded for the clear-eye proof:
/// both come from this one `eye`.
struct Shot {
    /// Stable shot id (`spawn`, `seam/keep/0`, `pov/leg0/wp3`, …).
    id: String,
    /// The shot `kind` string, and the kind the diagnostic names.
    kind: &'static str,
    /// The area the shot belongs to (`""` when the plan cannot name one).
    area: String,
    /// The camera eye, in world coordinates.
    eye: [f64; 3],
    /// The point the camera is aimed at.
    look_at: [f64; 3],
    /// An explicit field of view, for the kinds that frame like a player.
    fov: Option<f64>,
    /// Top-level keys this kind carries beyond the common ones.
    extra: Vec<(&'static str, Value)>,
    /// The machine `expect` checklist a vision review reads against the frame.
    expect: Vec<Value>,
    /// Whether the area's stage-1 `lighting` declaration is stamped on
    /// ([`area_lighting_stamp`]).
    stamped: bool,
}

/// The shot list under construction, the eye cells the clear-eye proof owes a
/// verdict on, and the world both are judged against.
///
/// [`Shots::push`] is the only way a shot enters the plan, and it does three
/// things no kind may skip: it stands the camera up in open air, it writes the
/// resulting eye into the shot's `camera`, and it records that same eye for the
/// `DW0724` proof. A kind added later gets all three whether or not its author
/// thought about it — which is the whole finding this shape exists to answer,
/// since both the standing-up and the proof used to belong to one caller each.
struct Shots<'c> {
    campaign: &'c Campaign,
    world: &'c World,
    shots: Vec<Value>,
    eyes: Vec<CameraEye>,
    /// How many cameras could not stand at their requested stand-off.
    pulled_in: usize,
}

impl<'c> Shots<'c> {
    fn new(campaign: &'c Campaign, world: &'c World) -> Self {
        Shots {
            campaign,
            world,
            shots: Vec::new(),
            eyes: Vec::new(),
            pulled_in: 0,
        }
    }

    fn push(&mut self, shot: Shot) {
        // A `pov` camera is the PLAYER's eye: it is already 1.62 above a
        // DW0314-proven-standable waypoint, and moving it would stop it being the
        // player's view — the one thing that kind exists to show. Every other kind
        // states a stand-off from a subject it frames, which is a preference and
        // not a position. The distinction is a property of the kind, so it is read
        // off the kind here rather than chosen per shot.
        let requested = shot.eye;
        let eye = if shot.kind == "pov" || self.world.is_clear(eye_cell(requested)) {
            // The requested stand-off is honoured wherever it can be: an interior
            // shot's dollhouse eye ABOVE the piece is deliberately outside the
            // room (the renderer strips the ceiling), and pulling it down the
            // sight line just because a roof is in the way would replace the
            // framing this kind exists for. The stand-off yields to exactly one
            // fact — that the cell it names holds a block.
            requested
        } else {
            crate::camera::stand_in_open_air(
                |cell| !self.world.is_clear(cell),
                shot.look_at,
                requested,
            )
            .unwrap_or(requested)
        };
        let moved = eye != requested;
        if moved {
            self.pulled_in += 1;
        }
        let mut cam = camera(eye, shot.look_at);
        let cam_obj = cam.as_object_mut().expect("camera is a JSON object");
        if let Some(fov) = shot.fov {
            cam_obj.insert("fov".to_string(), json!(round3(fov)));
        }
        if moved {
            // A displaced camera is invisible in its own frame, so the plan says
            // where it was asked to stand and why it does not.
            cam_obj.insert(
                "requested_pos".to_string(),
                json!([
                    round3(requested[0]),
                    round3(requested[1]),
                    round3(requested[2])
                ]),
            );
            cam_obj.insert(
                "standoff".to_string(),
                json!(
                    "pulled-in — the requested stand-off is inside geometry; the camera stands \
                       at the furthest clear point on its own sight line"
                ),
            );
        }
        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), json!(shot.id));
        obj.insert("kind".to_string(), json!(shot.kind));
        obj.insert("area".to_string(), json!(shot.area));
        for (k, v) in shot.extra {
            obj.insert(k.to_string(), v);
        }
        obj.insert("camera".to_string(), cam);
        obj.insert("expect".to_string(), Value::Array(shot.expect));
        if shot.stamped
            && let Some(stamp) = area_lighting_stamp(self.campaign, &shot.area)
        {
            obj.insert("lighting".to_string(), stamp);
        }
        self.eyes.push(CameraEye {
            shot_id: shot.id,
            kind: shot.kind,
            cell: eye_cell(eye),
        });
        self.shots.push(Value::Object(obj));
    }
}

/// Build the `render-plan.json` value for a compiled plan (spec-0003), proving
/// every camera's eye cell clear in `world` — the final assembled model the shots
/// will be rendered from — before there is a document at all.
///
/// There is no way to a render-plan value that skips the proof: this function is
/// the only constructor and it takes the world. Returns `DW0724` naming the first
/// camera that could not be stood up, and — alongside the document — any
/// advisory the proof owes, including the zero-binding one.
///
/// The document states its own binding counts (`camera_eye_proof`: how many
/// cameras were examined, how many had to be pulled in), because a proof that
/// examined nothing must not read like a proof that examined everything and found
/// nothing.
///
/// Pure and deterministic: shot order is spawn → per-area interiors + seams →
/// NPCs → interacts → gates → player-POV, every list plan-ordered or sorted, no
/// RNG and no clock, so the result rides the ADR-0006 double-build gate.
/// The world y a body stands on outside the pieces, for a horizon that BUILT
/// ground, or `None` for one that did not (`void`, `ocean` — nothing outside a
/// piece but air or the level generator's own sea).
fn ground_plane(plan: &Plan) -> Option<f64> {
    plan.surround
        .as_ref()
        .map(|s| f64::from(s.valley.floor_top_y + 1))
}

pub fn render_plan(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    pov: &[PovShot],
    world: &World,
) -> Result<(Value, Vec<Diagnostic>), Failure> {
    let c = plan.campaign;
    let mut out = Shots::new(c, world);

    // --- spawn -------------------------------------------------------------
    if let Some((area, pos, facing)) = spawn_of(plan) {
        let f = facing_vec(facing.as_deref());
        let base = centre(pos);
        // Camera behind the player (opposite the spawn facing), raised, looking
        // the way the player faces.
        let eye = [base[0] - f[0] * 5.0, base[1] + 3.0, base[2] - f[2] * 5.0];
        let look = [base[0] + f[0] * 2.0, base[1] + 1.0, base[2] + f[2] * 2.0];
        out.push(Shot {
            id: "spawn".to_string(),
            kind: "spawn",
            area,
            eye,
            look_at: look,
            fov: None,
            extra: Vec::new(),
            expect: vec![
                Value::String(
                    "spawn point clear — player can stand, not suffocating or falling".into(),
                ),
                Value::String("area entry framed — no void gap at the floor".into()),
            ],
            stamped: false,
        });
    }

    // --- per-area interiors + seams ---------------------------------------
    for area in &plan.areas {
        for (pi, piece) in area.pieces.iter().enumerate() {
            let (min, max) = piece.bbox();
            let cx = (min[0] + max[0]) as f64 / 2.0 + 0.5;
            let cy = min[1] as f64 + 1.0;
            let cz = (min[2] + max[2]) as f64 / 2.0 + 0.5;
            // Dollhouse overview: eye above a corner, aimed down at the interior
            // centre. (Renderer strips the ceiling for the matching per-piece
            // interior shot; the Chunky path places a true in-room camera.)
            //
            // **Above the GROUND, not merely above the piece.** The offset is
            // from the piece and always was, and what changed under it is that
            // the world outside a piece is no longer guaranteed to be nothing:
            // a horizon that builds a landform puts real ground at its own walk
            // plane, and a piece whose top is below that plane is a sunken
            // storey with terrain over it. At a piece-relative offset the eye
            // then sits INSIDE the ground and `DW0724` refuses the shot — which
            // it is right to do, and which no campaign could ever satisfy,
            // because the occupying block is the world rather than anything an
            // author placed. Clearing the walk plane is not nudging a camera to
            // make a picture come out; it is an overview standing over the
            // ground it is an overview of.
            let over = ground_plane(plan).map_or(max[1] as f64, |g| (max[1] as f64).max(g));
            let eye = [min[0] as f64 - 1.5, over + 3.0, min[2] as f64 - 1.5];
            let look = [cx, cy, cz];
            let lit = piece_is_lit(prefabs, &piece.prefab_id);
            let mut expect = vec![
                Value::String("room interior assembled — walls, floor and ceiling intact".into()),
                Value::String("no floating or clipped blocks at piece bounds".into()),
            ];
            expect.push(Value::String(match lit {
                Some(true) => "room declared lit — no dark frame".into(),
                Some(false) => {
                    "room declared dark — mitigation expected (night-vision/placed light)".into()
                }
                None => "room lighting unmeasured — verify readability".into(),
            }));
            out.push(Shot {
                id: format!("interior/{}/{pi}", short(&area.area_id)),
                kind: "interior",
                area: area.area_id.clone(),
                eye,
                look_at: look,
                fov: None,
                extra: vec![("prefab", json!(piece.prefab_id))],
                expect,
                stamped: true,
            });
        }
        // Seam (doorway) shots: air-clear seals are cut openings between mated
        // pieces; wall-fill seals seal dead-end sockets (skipped — nothing to see).
        for (si, seal) in area.seals.iter().enumerate() {
            if seal.block != "minecraft:air" {
                continue;
            }
            let ctr = region_centre(seal.from, seal.to);
            let axis = thin_axis(seal.from, seal.to).unwrap_or(2);
            let mut n = [0.0, 0.0, 0.0];
            n[axis] = 1.0;
            let eye = [ctr[0] + n[0] * 4.0, ctr[1] + 1.5, ctr[2] + n[2] * 4.0];
            out.push(Shot {
                id: format!("seam/{}/{si}", short(&area.area_id)),
                kind: "seam",
                area: area.area_id.clone(),
                eye,
                look_at: ctr,
                fov: None,
                extra: Vec::new(),
                expect: vec![
                    Value::String("seam between pieces shows no floating or clipped blocks".into()),
                    Value::String("doorway opening is clear — passage traversable".into()),
                ],
                stamped: false,
            });
        }
    }

    // --- NPCs --------------------------------------------------------------
    for npc in &plan.npcs {
        let area = plan.npc_area(&npc.npc_id).unwrap_or("").to_string();
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| n.anchor.as_str())
            .unwrap_or("");
        let name = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| delvewright_dsl::l10n_plain(&n.name))
            .unwrap_or("NPC");
        let Some(ResolvedAnchor::Point { pos, facing }) =
            plan.anchors.get(&(area.clone(), anchor.to_string()))
        else {
            continue;
        };
        let f = facing_vec(facing.as_deref());
        let base = centre(*pos);
        // The player approaches from the direction the NPC faces (NPCs are summoned
        // facing the player), so the camera stands there and looks back at the NPC.
        let eye = [base[0] + f[0] * 4.0, base[1] + 1.6, base[2] + f[2] * 4.0];
        let look = [base[0], base[1] + 1.0, base[2]];
        out.push(Shot {
            id: format!("npc/{}", short(&npc.npc_id)),
            kind: "npc",
            area,
            eye,
            look_at: look,
            fov: None,
            extra: vec![("npc", json!(npc.npc_id))],
            expect: vec![
                Value::String(format!("NPC named \"{name}\" faces the camera")),
                Value::String(
                    "NPC name tag renders as readable text — not literal JSON/SNBT".into(),
                ),
                Value::String(
                    "NPC stands on the floor — not floating, sunk, or clipping a wall".into(),
                ),
            ],
            stamped: false,
        });
    }

    // --- interact anchors --------------------------------------------------
    for (obj_id, anchor, area, hint) in interact_anchors(c) {
        let Some(pos) = plan.point(&area, &anchor) else {
            continue;
        };
        let base = centre(pos);
        let eye = [base[0] + 2.5, base[1] + 3.0, base[2] + 2.5];
        let look = [base[0], base[1] + 1.0, base[2]];
        let mut expect = vec![
            Value::String("glowing lantern marker visible at the interact anchor".into()),
            Value::String("interaction hitbox present — objective is completable here".into()),
        ];
        if let Some(h) = hint {
            expect.push(Value::String(format!(
                "matches objective hint: {}",
                delvewright_dsl::l10n_plain(&h)
            )));
        }
        out.push(Shot {
            id: format!("interact/{}", short(&obj_id)),
            kind: "interact",
            area,
            eye,
            look_at: look,
            fov: None,
            extra: vec![("objective", json!(obj_id))],
            expect,
            stamped: false,
        });
    }

    // --- gates (both sides) -----------------------------------------------
    for ((area, anchor), resolved) in &plan.anchors {
        let ResolvedAnchor::Gate { from, to, block } = resolved else {
            continue;
        };
        let ctr = region_centre(*from, *to);
        let axis = thin_axis(*from, *to).unwrap_or(2);
        for (side, sign) in [("a", 1.0f64), ("b", -1.0f64)] {
            let mut off = [0.0, 0.0, 0.0];
            off[axis] = sign * 4.0;
            let eye = [ctr[0] + off[0], ctr[1] + 1.0, ctr[2] + off[2]];
            out.push(Shot {
                id: format!("gate/{}/{}/{side}", short(area), short(anchor)),
                kind: "gate",
                area: area.clone(),
                eye,
                look_at: ctr,
                fov: None,
                extra: Vec::new(),
                expect: vec![
                    Value::String(format!(
                        "gate of {block} spans the opening — no gaps when closed"
                    )),
                    Value::String(
                        "gate approach clear from this side — passage blocked until opened".into(),
                    ),
                ],
                stamped: false,
            });
        }
    }

    // --- player-POV shots (first-person, along the walked critical path) ---
    // Appended after the overhead/orbit kinds so the deterministic prefix (spawn
    // first, …) that existing consumers assert stays stable.
    for shot in pov {
        let mut expect: Vec<Value> = vec![Value::String(shot.expect_line.clone())];
        expect.extend(shot.extra_expect.iter().cloned().map(Value::String));
        out.push(Shot {
            id: shot.id.clone(),
            kind: "pov",
            area: shot.area.clone(),
            eye: shot.eye,
            look_at: shot.look_at,
            fov: Some(POV_FOV_DEG),
            extra: vec![
                ("leg", json!(shot.leg)),
                ("waypoint", json!(shot.wp)),
                ("objective", json!(shot.objective)),
                ("standing_cell", json!(shot.standing_cell)),
            ],
            expect,
            stamped: true,
        });
    }

    crate::nav::verify_camera_eyes(world, &out.eyes)?;
    let mut warnings = Vec::new();
    if out.eyes.is_empty() {
        warnings.push(Diagnostic::warning(
            crate::nav::DW_CAMERA_EYE_OCCLUDED,
            "world",
            "/shots",
            "the visual tier's clear-eye proof examined ZERO cameras: this campaign's render plan \
             holds no shots at all, so nothing in it is proven and nothing in it can be reviewed. \
             A zero binding is a finding, not a pass.",
        ));
    }

    let (amin, amax) = layout_aabb(plan);
    let mut root = json!({
        "version": c.world.dsl_version,
        "campaign_id": plan.namespace,
        "layout_aabb": { "min": amin, "max": amax },
        "camera_convention": "yaw/pitch degrees; yaw=atan2(-dz,dx) (0=+X,90=-Z); pitch=atan2(-dy,horiz) (+down)",
        "camera_eye_proof": { "cameras": out.eyes.len(), "pulled_in": out.pulled_in },
        "shots": out.shots,
    });
    if let Some(h) = horizon_fact(c, plan) {
        root.as_object_mut()
            .expect("render plan root is a JSON object")
            .insert("horizon".to_string(), h);
    }
    Ok((root, warnings))
}

/// The world-generator horizon (spec-0013) as the render layer needs it, or
/// `None` for `horizon: void`.
///
/// The renderer cannot see the level generator: the shipped world save only
/// holds the chunks the layout occupies, so an ocean-horizon delve renders as an
/// island floating in nothing unless Chunky is told to put its own ambient water
/// plane under the frame — at exactly the compiler's sea-level datum, or the
/// plane and the authored block water meet in a visible two-tone seam. That is a
/// *fact of the campaign*, so the compiler states it (`{"kind": "ocean",
/// "sea_level": 62}`) rather than leaving `delve-render` to infer it from
/// blocks.
///
/// A void horizon emits **no key at all** (not `null`), so every campaign that
/// declares nothing keeps a byte-identical `render-plan.json`.
fn horizon_fact(c: &Campaign, plan: &Plan) -> Option<Value> {
    let r = delvewright_dsl::resolved_horizon(&c.world.content.horizon);
    let extent = plan.surround.as_ref().map(|s| {
        let (p, d) = (s.piece.pos, s.piece.size);
        json!({
            "min": [p[0], p[1], p[2]],
            "max": [p[0] + d[0] - 1, p[1] + d[1] - 1, p[2] + d[2] - 1],
        })
    });
    match r.base {
        HorizonBase::Ocean => Some(json!({
            "kind": "ocean",
            "sea_level": crate::plan::SEA_LEVEL,
        })),
        // A valley's fact is its rim, because that is what a renderer has to
        // frame: the gap floor is ordinary ground, and the crest is the line
        // the sky starts above.
        HorizonBase::Valley => Some(json!({
            "kind": "valley",
            "gap_floor_y": crate::horizon::VALLEY_GAP_FLOOR_TOP_Y,
            "rim_height": r.rim_height,
            // The landform's own world AABB. Stated rather than left to be
            // inferred, for the same reason the ocean's sea level is: the
            // render layer cannot see the derivation, and every whole-map frame
            // has to know how big the ground is. A `valley` whose extent were
            // absent would frame the pieces and load the pieces' chunks, which
            // is a delve standing in a void it is surrounded by terrain in.
            "extent": extent,
        })),
        HorizonBase::Void => None,
    }
}

/// The last `/`-segment of an id, sanitized to `[a-z0-9_]`, for stable shot ids.
fn short(id: &str) -> String {
    let local = id.rsplit('/').next().unwrap_or(id);
    local.replace(['-', '.', ':'], "_")
}

/// The `lighting` stamp for a shot in `area_id`, derived **purely from the
/// area's stage-1 declarations** (never from measurement — the measured model
/// already gates via `DW0210`/`DW0211`; the stamp only tells the render layer
/// what the declaration *intends*):
///
/// - `lighting` declared → `{"profile": "lit"}` (the relight pass guarantees
///   `min_light` on every reachable walkable cell, so the assembled scene has
///   real fixtures for a path tracer to see);
/// - only `mitigation` declared → `{"profile": "dark", "mitigation":
///   "night-vision"}` (the area is *meant* to be dark and the players are
///   equipped — an honest render of it is black, so `delvec scene`
///   applies its documented night-vision review emulation);
/// - both declared → lit profile plus the mitigation (fixtures light the scene;
///   no emulation needed or applied);
/// - neither → `None`: **no stamp key is emitted**, keeping campaigns without
///   lighting declarations byte-identical.
fn area_lighting_stamp(c: &Campaign, area_id: &str) -> Option<Value> {
    let area = c
        .world
        .content
        .areas
        .iter()
        .find(|a| a.id.as_str() == area_id)?;
    let profile = if area.lighting.is_some() {
        "lit"
    } else if area.mitigation.is_some() {
        "dark"
    } else {
        return None;
    };
    let mut stamp = serde_json::Map::new();
    stamp.insert("profile".to_string(), json!(profile));
    if let Some(m) = area.mitigation {
        // Exhaustive on purpose: a new mitigation variant must decide its
        // stamp spelling here (kept in lockstep with the DSL's kebab-case
        // serde name).
        let name = match m {
            AreaMitigation::NightVision => "night-vision",
        };
        stamp.insert("mitigation".to_string(), json!(name));
    }
    Some(Value::Object(stamp))
}

/// Whether a placed prefab's **measured** lighting profile is `lit`
/// (`Some(true)`), `dim`/`dark` (`Some(false)`), or unknown (`None`).
///
/// A profile of `unmeasured` — what a generated prefab declares until the live
/// probe runs — is unknown, not dark: the reviewer instruction that follows from
/// `Some(false)` is "mitigation expected", and asking a reviewer to look for a
/// mitigation that was never specified is worse than telling them the truth,
/// which is the `None` branch's "verify readability".
fn piece_is_lit(prefabs: &PrefabRegistry, prefab_id: &str) -> Option<bool> {
    prefabs.get(prefab_id).and_then(|m| {
        let profile = m.lighting.as_ref()?.profile;
        profile
            .is_measurement()
            .then_some(matches!(profile, LightingProfile::Lit))
    })
}

/// `(objective_id, anchor, area, hint)` for every `interact` objective, in
/// declared order.
fn interact_anchors(c: &Campaign) -> Vec<(String, String, String, Option<String>)> {
    let quest_area: std::collections::BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        let area = quest_area.get(q.id.as_str()).copied().unwrap_or("");
        for o in &q.objectives {
            if let Objective::Interact {
                id, anchor, hint, ..
            } = o
            {
                out.push((
                    id.as_str().to_string(),
                    anchor.as_str().to_string(),
                    area.to_string(),
                    hint.clone(),
                ));
            }
        }
    }
    out
}

/// Union world AABB across every placed area (for the Chunky `chunkList`).
fn layout_aabb(plan: &Plan) -> ([i32; 3], [i32; 3]) {
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for area in &plan.areas {
        let (amin, amax) = area.bounds();
        for a in 0..3 {
            min[a] = min[a].min(amin[a]);
            max[a] = max[a].max(amax[a]);
        }
    }
    if min[0] == i32::MAX {
        // No pieces (should not happen for a valid campaign); degrade to origin.
        return ([0, 0, 0], [0, 0, 0]);
    }
    (min, max)
}

#[cfg(test)]
mod lighting_tests {
    use super::*;

    /// Every call gets its own directory, and the discriminator is a counter
    /// rather than anything derived from `lighting`.
    ///
    /// It used to be `lighting.len()`, which is not a discriminator at all when
    /// two callers pass the same string: `an_unmeasured_piece_is_unknown_not_dark`
    /// and `an_unknown_prefab_has_no_verdict` both pass a byte-identical
    /// `{ "profile": "unmeasured" }`, so under one process id they named ONE
    /// directory — and each begins by deleting it. Cargo runs a crate's tests on
    /// parallel threads, so the two collided by construction: whichever reached
    /// `remove_dir_all` second deleted the other's fixture out from under it,
    /// and the loser failed to load a prefab it had just written. That surfaced
    /// as a rare `--workspace` red (one in ~30) and read as flakiness, which is
    /// exactly the shape CLAUDE.md forbids re-running instead of root-causing.
    fn registry_with(lighting: &str) -> (std::path::PathBuf, PrefabRegistry) {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "dw-piece-is-lit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("p.json"),
            format!(
                r#"{{ "prefab_id": "prefab/p",
                      "structure": {{ "file": "p.nbt", "id": "p", "size": [3,3,3],
                                      "data_version": 4671 }},
                      "anchors": {{}}, "lighting": {lighting} }}"#
            ),
        )
        .unwrap();
        let registry = PrefabRegistry::load_dir(&dir).unwrap();
        assert!(registry.load_diagnostics().is_empty(), "{lighting}");
        (dir, registry)
    }

    /// The interior shot's reviewer instruction is chosen by this verdict, so
    /// the three states must stay three: lit, measured-not-lit ("mitigation
    /// expected"), and unknown ("verify readability"). A never-probed piece
    /// belongs in the third, never the second — it was not *declared* dark, it
    /// was not declared at all.
    #[test]
    fn an_unmeasured_piece_is_unknown_not_dark() {
        for (lighting, expected) in [
            (
                r#"{ "profile": "lit", "measured_min_light": 9, "measured": "2026-07-30" }"#,
                Some(true),
            ),
            (
                r#"{ "profile": "dark", "measured_min_light": 1, "measured": "2026-07-30" }"#,
                Some(false),
            ),
            (r#"{ "profile": "unmeasured" }"#, None),
        ] {
            let (dir, registry) = registry_with(lighting);
            assert_eq!(piece_is_lit(&registry, "prefab/p"), expected, "{lighting}");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }

    /// The fixture helper's own invariant, and the reason the discriminator is a
    /// counter: two callers passing the SAME lighting string must still get two
    /// directories. Under the old `lighting.len()` naming this assertion fails
    /// outright — the two paths are equal — which is the deterministic form of
    /// the race that made `an_unmeasured_piece_is_unknown_not_dark` and
    /// `an_unknown_prefab_has_no_verdict` delete each other's fixture.
    #[test]
    fn two_registries_never_share_a_directory() {
        let same = r#"{ "profile": "unmeasured" }"#;
        let (a, _ra) = registry_with(same);
        let (b, _rb) = registry_with(same);
        assert_ne!(
            a, b,
            "two fixtures built from an identical lighting string shared one directory"
        );
        assert!(a.join("p.json").is_file() && b.join("p.json").is_file());
        std::fs::remove_dir_all(&a).unwrap();
        std::fs::remove_dir_all(&b).unwrap();
    }

    /// An unknown prefab stays unknown — the verdict never invents a piece.
    #[test]
    fn an_unknown_prefab_has_no_verdict() {
        let (dir, registry) = registry_with(r#"{ "profile": "unmeasured" }"#);
        assert_eq!(piece_is_lit(&registry, "prefab/nope"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod pov_tests {
    use super::*;

    #[test]
    fn compass_is_dominant_horizontal_axis() {
        // Minecraft: −Z is north, +Z south, +X east, −X west.
        assert_eq!(compass_toward([0.0, 0.0, 0.0], [5.0, 0.0, 1.0]), "east");
        assert_eq!(compass_toward([0.0, 0.0, 0.0], [-5.0, 0.0, 1.0]), "west");
        assert_eq!(compass_toward([0.0, 0.0, 0.0], [1.0, 0.0, 5.0]), "south");
        assert_eq!(compass_toward([0.0, 0.0, 0.0], [1.0, 0.0, -5.0]), "north");
        // Ignores the vertical component (a staircase still reads by its heading).
        assert_eq!(compass_toward([0.0, 5.0, 0.0], [0.0, 0.0, -3.0]), "north");
    }

    #[test]
    fn first_clause_stops_at_the_first_sentence() {
        assert_eq!(
            first_clause("He is on the sand beside you, and he will not let go. More."),
            "He is on the sand beside you, and he will not let go"
        );
        assert_eq!(first_clause("no punctuation here"), "no punctuation here");
    }

    #[test]
    fn approach_heading_uses_the_last_segment_then_falls_back() {
        // Last segment runs +X (east); heading is the unit +X.
        let wps = [[0, 65, 0], [3, 65, 0], [5, 65, 0]];
        let h = approach_heading(&wps, [6.5, 66.0, 0.5], [5.5, 66.62, 0.5]);
        assert!(
            (h[0] - 1.0).abs() < 1e-9 && h[1].abs() < 1e-9,
            "heading east: {h:?}"
        );
        // A single-waypoint leg has no segment → fall back to eye→anchor.
        let one = [[5, 65, 0]];
        let h2 = approach_heading(&one, [5.5, 66.0, 9.5], [5.5, 66.62, 5.5]);
        assert!(h2[1] > 0.9, "falls back to eye→anchor (south): {h2:?}");
    }

    #[test]
    fn eye_cell_floors_the_eye_position() {
        let s = PovShot {
            id: "pov/leg0/wp0".into(),
            area: "area/keep".into(),
            leg: 0,
            wp: 0,
            objective: None,
            standing_cell: [5, 65, 4],
            eye: [5.5, 66.62, 4.5],
            look_at: [5.5, 66.62, 8.5],
            expect_line: "x".into(),
            extra_expect: vec![],
        };
        // Eye at y=66.62 sits in block Y=66 — the head cell above the standing cell,
        // which standability already proves clear (so DW0724 cannot fire here).
        assert_eq!(s.eye_cell(), [5, 66, 4]);
    }
}
