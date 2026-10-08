//! Cutscenes: the camera shots a cutscene is made of and whether the party is shown.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_zero3;
use crate::{ActorId, Mark, NpcId};

#[cfg(doc)]
use crate::Verb;

/// Whether a `cutscene` shows the party's bodies (spec-0095).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CutsceneParty {
    /// Each player in play is shown by a stand-in where they stood: a
    /// `minecraft:mannequin` wearing their own profile (skin) and a copy of
    /// their armour and held items, removed when the cutscene ends.
    #[default]
    Present,
    /// No stand-ins: the bodies leave the scene for the cutscene's length.
    Absent,
}

impl CutsceneParty {
    /// The kebab token (`present` / `absent`).
    pub fn token(self) -> &'static str {
        match self {
            CutsceneParty::Present => "present",
            CutsceneParty::Absent => "absent",
        }
    }
}

/// One shot of a [`Verb::Cutscene`] (DSL v0.6): a camera dolly with its
/// own duration and optional subject. A cutscene plays its shots back-to-back —
/// a hard cut between them — inside a single gamemode/position save-restore
/// bracket, so a wide establishing move can be followed by an interior close-up
/// without the players ever leaving the cinematic.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CameraShot {
    /// Ordered camera waypoints (straight-line lerp between them). A one-waypoint
    /// path is a static shot. Required without `shot_style`; with one, optional —
    /// an explicit `path` always overrides the style's expanded dolly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Mark>,
    /// This shot's duration in seconds. Required without `shot_style`; with one,
    /// optional — the style's default duration applies (see
    /// [`ShotStyle::default_seconds`]), and an explicit value always overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<u32>,
    /// Optional subject the camera keeps framed for this shot. Absent = face
    /// along the direction of travel (or, under a `shot_style`, the style's own
    /// aim at its `subject`). An explicit `look_at` always overrides a style's
    /// aim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub look_at: Option<Mark>,
    /// Shot-style preset (DSL v0.6, spec-0015 shot-grammar library): the
    /// compiler expands the style deterministically into a camera dolly +
    /// per-keyframe aim from the `subject`'s resolved geometry. Requires
    /// `subject`; `path`/`look_at`/`seconds` remain legal and always override
    /// the corresponding expanded part (`DW0348` polices the combinations).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shot_style: Option<ShotStyle>,
    /// The styled shot's subject — what the shot is *about*. Required with
    /// `shot_style`, rejected without one (`DW0348`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<CameraSubject>,
    /// `two-shot` only: the second framed subject (`DW0348` elsewhere).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_b: Option<CameraSubject>,
    /// Styled shots only: the style's characteristic camera distance in blocks
    /// (its *start* distance for the dolly styles). Default per style — see the
    /// `shot_style` table in `docs/reference/compiler.md`. Clamped range 1..=48
    /// (`DW0348`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dist: Option<f64>,
    /// `orbit-arc` only: the sweep in degrees, 45..=120 (dossier range),
    /// default 90 (`DW0348` elsewhere or out of range).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degrees: Option<f64>,
    /// Styled shots only: placement bearing in degrees — where the camera sits
    /// (or starts) relative to the subject, measured like a Minecraft yaw
    /// *from* the subject: `0` puts the camera south of the subject (+Z), `90`
    /// west (−X), `-90` east (+X), `180` north (−Z). Default `0`. For
    /// `side-track` the bearing picks which side of the subject's travel the
    /// camera runs abeam (`0` = right of travel, `180` = left).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearing: Option<f64>,
}

/// A `shot_style` preset (DSL v0.6): the dossier's 9-template library
/// (`docs/notes/camera-dossier.md` §2), each expanded deterministically by the
/// compiler into a dolly + aim from the subject's geometry. Camera "lens feel"
/// is **distance only** — vanilla has no in-game FOV control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ShotStyle {
    /// Fully static close framing of a prop/detail. The beat that always looks
    /// right.
    Insert,
    /// Static position; only the aim turns as the subject (ideally moving)
    /// passes. Rockstar's roadside "ground view".
    LockedOff,
    /// Straight dolly toward the subject along the view axis; medium → close.
    PushIn,
    /// Reverse of `push-in`: close → wide, revealing context.
    PullBackReveal,
    /// High and far, descending and closing on the subject. First sight of an
    /// area.
    EstablishingCrane,
    /// Constant-radius, constant-height arc around the subject.
    OrbitArc,
    /// Parallel dolly abeam a **moving** subject at constant offset — needs a
    /// compiler-known subject path (`DW0349`).
    SideTrack,
    /// Static placement solved so **both** subjects land on opposite thirds
    /// (Toric-space construction). Needs `subject_b`.
    TwoShot,
    /// Low, close, trailing a **moving** subject near ground level — needs a
    /// compiler-known subject path (`DW0349`).
    LowFollow,
}

impl ShotStyle {
    /// The kebab token (diagnostics, digests).
    pub fn token(self) -> &'static str {
        match self {
            ShotStyle::Insert => "insert",
            ShotStyle::LockedOff => "locked-off",
            ShotStyle::PushIn => "push-in",
            ShotStyle::PullBackReveal => "pull-back-reveal",
            ShotStyle::EstablishingCrane => "establishing-crane",
            ShotStyle::OrbitArc => "orbit-arc",
            ShotStyle::SideTrack => "side-track",
            ShotStyle::TwoShot => "two-shot",
            ShotStyle::LowFollow => "low-follow",
        }
    }

    /// Default shot duration in seconds, from the dossier's per-style duration
    /// ranges (§2, anchored on the film-editing ASL literature) — the value an
    /// omitted `seconds` resolves to.
    pub fn default_seconds(self) -> u32 {
        match self {
            ShotStyle::Insert => 2,
            ShotStyle::LockedOff => 6,
            ShotStyle::PushIn => 4,
            ShotStyle::PullBackReveal => 6,
            ShotStyle::EstablishingCrane => 8,
            ShotStyle::OrbitArc => 8,
            ShotStyle::SideTrack => 8,
            ShotStyle::TwoShot => 5,
            ShotStyle::LowFollow => 5,
        }
    }

    /// `true` for the styles whose subject must be *moving* on a compiler-known
    /// path (`move-npc` / `move-actor` in the same effect group or sequence) —
    /// `side-track` and `low-follow` (`DW0349` otherwise).
    pub fn needs_moving_subject(self) -> bool {
        matches!(self, ShotStyle::SideTrack | ShotStyle::LowFollow)
    }
}

/// A styled shot's subject (DSL v0.6): the world thing the shot frames — a
/// prefab anchor point, a stage-2 NPC, or a stage-5 actor — plus an integer
/// block offset. For `npc`/`actor` subjects the aim point is the entity's cell
/// **plus one block up** (torso height, so a close shot does not frame feet)
/// before `offset` is applied; an `anchor` subject aims at the block centre
/// exactly like a [`CameraTarget`].
/// Each variant's payload is a **named struct** carrying `deny_unknown_fields`
/// — serde has no variant-level `deny_unknown_fields`, so an untagged
/// enum with inline struct variants silently *ignores* any key it does not
/// recognise: `{"npc": …, "ofset": [0,1,0]}` would deserialize happily with the
/// offset dropped, and `{"anchor": …, "npc": …}` would quietly match `Anchor`
/// and discard the NPC. Lifting each variant into its own type keeps the repo-wide
/// deny-unknown rule for both serde and the published JSON Schema
/// (`additionalProperties: false`): a mistyped shot subject fails the schema
/// instead of rendering a shot pointed somewhere the author never asked for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum CameraSubject {
    /// A fixed world point: prefab anchor + offset.
    Anchor(Mark),
    /// A stage-2 NPC — moving if a `move-npc` for it runs in the same effect
    /// group / sequence, else static at its declared (or spawn) anchor.
    Npc(NpcSubject),
    /// A stage-5 actor — moving if a `move-actor` for it runs in the same
    /// effect group / sequence, else static at its declared anchor.
    Actor(ActorSubject),
}

/// A [`CameraSubject::Npc`] payload: a stage-2 NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcSubject {
    /// The NPC (stage-2 ref).
    pub npc: NpcId,
    /// Integer `[x, y, z]` block offset (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// A [`CameraSubject::Actor`] payload: a stage-5 actor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorSubject {
    /// The actor (stage-5 `actors` ref).
    pub actor: ActorId,
    /// Integer `[x, y, z]` block offset (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

impl CameraSubject {
    /// The subject's integer offset, whichever variant.
    pub fn offset(&self) -> [i32; 3] {
        match self {
            CameraSubject::Anchor(s) => s.offset,
            CameraSubject::Npc(s) => s.offset,
            CameraSubject::Actor(s) => s.offset,
        }
    }

    /// A short canonical rendering for digests/diagnostics.
    pub fn canon(&self) -> String {
        let (kind, id, o) = match self {
            CameraSubject::Anchor(s) => ("a", s.anchor.as_str(), &s.offset),
            CameraSubject::Npc(s) => ("n", s.npc.as_str(), &s.offset),
            CameraSubject::Actor(s) => ("c", s.actor.as_str(), &s.offset),
        };
        format!("{kind}:{id}@{},{},{}", o[0], o[1], o[2])
    }
}

/// `Debug` is hand-written because it is a **stable content-key rendering**:
/// the compiler's `payload_verb_key` (FNV over a verb's own `{:?}`) names the
/// generated `volley_`, `collapse_` and `teleport_` functions from it, so a shot
/// that uses none of the v0.6 style fields must render byte-identically to the
/// pre-style struct (`seconds` prints its inner value; absent style fields print
/// nothing) — otherwise a purely additive schema change would silently churn the
/// content key of every payload that carries a cutscene.
impl std::fmt::Debug for CameraShot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("CameraShot");
        d.field("path", &self.path);
        match self.seconds {
            Some(v) => d.field("seconds", &v),
            None => d.field("seconds", &self.seconds),
        };
        d.field("look_at", &self.look_at);
        if self.shot_style.is_some() {
            d.field("shot_style", &self.shot_style);
        }
        if self.subject.is_some() {
            d.field("subject", &self.subject);
        }
        if self.subject_b.is_some() {
            d.field("subject_b", &self.subject_b);
        }
        if self.dist.is_some() {
            d.field("dist", &self.dist);
        }
        if self.degrees.is_some() {
            d.field("degrees", &self.degrees);
        }
        if self.bearing.is_some() {
            d.field("bearing", &self.bearing);
        }
        d.finish()
    }
}

impl CameraShot {
    /// The shot's resolved duration in seconds: explicit `seconds`, else the
    /// style default, else `1` (a shape-invalid shot — `DW0199` reports it; the
    /// fallback only keeps downstream passes total).
    pub fn resolved_seconds(&self) -> u32 {
        self.seconds
            .or(self.shot_style.map(ShotStyle::default_seconds))
            .unwrap_or(1)
    }
}
