//! Cutscenes: the camera shots a cutscene is made of and whether the party is shown.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_zero3;
use crate::{ActorId, Mark, NpcId};

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

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::{QuestEffect, Verb};
use std::collections::BTreeSet;

/// `cutscene` shape (`DW0199`): a cutscene is written either multi-shot
/// (`shots: [...]`, DSL v0.6) or single-shot (`path` + `seconds`, DSL v0.4) —
/// never both, never neither — and every resolved shot needs at least one camera
/// waypoint. The two spellings normalize to the same shot list
/// ([`QuestEffect::cutscene_shots`]), so this is the one place the shape is
/// policed; emission may then assume a non-empty, well-formed list.
pub(crate) fn check_cutscene_shape(eff: &QuestEffect, base_path: &str, d: &mut Vec<Diagnostic>) {
    let Verb::Cutscene {
        shots,
        path,
        seconds,
        ..
    } = &eff.verb
    else {
        return;
    };
    let single = !path.is_empty() || seconds.is_some();
    let err = |d: &mut Vec<Diagnostic>, field: &str, msg: String| {
        d.push(Diagnostic::error(
            codes::CUTSCENE_SHAPE,
            "quests",
            format!("{base_path}/{field}"),
            msg,
        ));
    };
    match (!shots.is_empty(), single) {
        (true, true) => err(
            d,
            "shots",
            "`cutscene` mixes the multi-shot `shots` list with the single-shot \
             `path`/`seconds` fields — use one form: move the single-shot fields into a `shots` \
             entry, or drop `shots`"
                .to_string(),
        ),
        (false, false) => err(
            d,
            "shots",
            "`cutscene` declares no shot — give a `shots` list of \
             `{path, seconds, look_at?}` (multi-shot), or a single-shot `path` + `seconds`"
                .to_string(),
        ),
        (false, true) if seconds.is_none() => err(
            d,
            "seconds",
            "single-shot `cutscene` is missing `seconds` — every shot needs a duration".to_string(),
        ),
        _ => {}
    }
    for (i, shot) in shots.iter().enumerate() {
        // A `shot_style` supplies both a default dolly and a default duration
        // (spec-0015), so `path`/`seconds` become optional overrides on a
        // styled shot; the style's own shape is policed by `DW0348`/`DW0349`.
        if shot.shot_style.is_some() {
            continue;
        }
        if shot.path.is_empty() {
            err(
                d,
                &format!("shots/{i}/path"),
                "`cutscene` shot has an empty camera `path` — give at least one waypoint (one \
                 waypoint is a static shot, two or more is a dolly), or use a `shot_style`"
                    .to_string(),
            );
        }
        if shot.seconds.is_none() {
            err(
                d,
                &format!("shots/{i}/seconds"),
                "`cutscene` shot is missing `seconds` — every unstyled shot needs an explicit \
                 duration (a `shot_style` would supply a default)"
                    .to_string(),
            );
        }
    }
    if shots.is_empty() && single && path.is_empty() {
        err(
            d,
            "path",
            "single-shot `cutscene` has an empty camera `path` — give at least one waypoint (one \
             waypoint is a static shot, two or more is a dolly)"
                .to_string(),
        );
    }
}

/// Shot-style semantics (DSL v0.6, spec-0015 shot grammar): `DW0348` for
/// invalid style/param combinations, `DW0112` for a subject referencing an
/// unknown npc/actor, and `DW0349` for a `side-track`/`low-follow` whose
/// subject provably cannot move.
///
/// The moving-subject scope mirrors the compiler's expansion resolution
/// exactly: a subject "moves" if a matching `move-npc`/`move-actor` runs in the
/// **same effect list**, or anywhere in the **same `sequence` timeline**
/// (including the list that launched the sequence). Nested reaction lists
/// (`on_arrive`/`on_caught`/`on_respawn`) start a fresh scope — their firing
/// time is unknowable statically, so motion outside them is never assumed.
pub(crate) fn cutscene_style_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::{CameraSubject, ShotStyle};
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let npc_ids = &npc_ids;
    let actor_ids: BTreeSet<&str> = quests.actors.iter().map(|a| a.id.as_str()).collect();

    /// The sibling moves visible to a cutscene: `(is_actor, id)`.
    fn moves_in(list: &[QuestEffect], scope: &mut Vec<(bool, String)>) {
        for e in list {
            match &e.verb {
                Verb::MoveNpc { npc, .. } => scope.push((false, npc.to_string())),
                Verb::MoveActor { actor, .. } => scope.push((true, actor.to_string())),
                Verb::Sequence { steps } => {
                    // A sequence launched from this list shares its timeline.
                    for st in steps {
                        moves_in(&st.effects, scope);
                    }
                }
                _ => {}
            }
        }
    }

    fn subject_check(
        sub: &CameraSubject,
        field: &str,
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let (unknown, kind, id) = match sub {
            CameraSubject::Anchor(_) => return,
            CameraSubject::Npc(s) => (!npc_ids.contains(s.npc.as_str()), "npc", s.npc.as_str()),
            CameraSubject::Actor(s) => (
                !actor_ids.contains(s.actor.as_str()),
                "actor",
                s.actor.as_str(),
            ),
        };
        if unknown {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("{path}/{field}"),
                format!(
                    "shot `{field}` references unknown {kind} `{id}` — declare it (stage 2 npcs / \
                     stage-5 `actors`) or correct the reference"
                ),
            ));
        }
    }

    fn check_shot_list(
        eff: &QuestEffect,
        scope: &[(bool, String)],
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let Verb::Cutscene { shots, .. } = &eff.verb else {
            return;
        };
        for (i, shot) in shots.iter().enumerate() {
            let spath = format!("{path}/shots/{i}");
            let err = |d: &mut Vec<Diagnostic>, field: &str, msg: String| {
                d.push(Diagnostic::error(
                    codes::SHOT_STYLE_INVALID,
                    "quests",
                    format!("{spath}/{field}"),
                    msg,
                ));
            };
            let Some(style) = shot.shot_style else {
                for (field, present) in [
                    ("subject", shot.subject.is_some()),
                    ("subject_b", shot.subject_b.is_some()),
                    ("dist", shot.dist.is_some()),
                    ("degrees", shot.degrees.is_some()),
                    ("bearing", shot.bearing.is_some()),
                ] {
                    if present {
                        err(
                            d,
                            field,
                            format!(
                                "`{field}` is a `shot_style` parameter but this shot declares no \
                                 `shot_style` — add one, or drop the field"
                            ),
                        );
                    }
                }
                continue;
            };
            let token = style.token();
            match &shot.subject {
                None => err(
                    d,
                    "subject",
                    format!(
                        "`shot_style: {token}` needs a `subject` — the anchor, npc, or actor the \
                         shot frames"
                    ),
                ),
                Some(sub) => subject_check(sub, "subject", &spath, npc_ids, actor_ids, d),
            }
            match (&shot.subject_b, style == ShotStyle::TwoShot) {
                (None, true) => err(
                    d,
                    "subject_b",
                    "`shot_style: two-shot` frames two subjects — give `subject_b`".to_string(),
                ),
                (Some(_), false) => err(
                    d,
                    "subject_b",
                    format!("`subject_b` is only meaningful on `two-shot`, not `{token}`"),
                ),
                (Some(sub), true) => subject_check(sub, "subject_b", &spath, npc_ids, actor_ids, d),
                (None, false) => {}
            }
            if let Some(g) = shot.degrees {
                if style != ShotStyle::OrbitArc {
                    err(
                        d,
                        "degrees",
                        format!("`degrees` is only meaningful on `orbit-arc`, not `{token}`"),
                    );
                } else if !(45.0..=120.0).contains(&g) {
                    err(
                        d,
                        "degrees",
                        format!(
                            "`orbit-arc` sweep `{g}` is outside `45..=120` degrees (the dossier's \
                             readable-orbit range)"
                        ),
                    );
                }
            }
            if let Some(dist) = shot.dist
                && !(1.0..=48.0).contains(&dist)
            {
                err(
                    d,
                    "dist",
                    format!("`dist` `{dist}` is outside the sane `1..=48` block range"),
                );
            }
            if let Some(b) = shot.bearing
                && !(-360.0..=360.0).contains(&b)
            {
                err(
                    d,
                    "bearing",
                    format!("`bearing` `{b}` is outside `-360..=360` degrees"),
                );
            }
            if style.needs_moving_subject() {
                let moved = match &shot.subject {
                    Some(CameraSubject::Npc(s)) => {
                        scope.iter().any(|(a, id)| !a && id == s.npc.as_str())
                    }
                    Some(CameraSubject::Actor(s)) => {
                        scope.iter().any(|(a, id)| *a && id == s.actor.as_str())
                    }
                    // An anchor can never move; a missing subject already got DW0348.
                    Some(CameraSubject::Anchor(_)) => false,
                    None => true,
                };
                if !moved {
                    d.push(Diagnostic::error(
                        codes::SHOT_SUBJECT_UNMOVED,
                        "quests",
                        format!("{spath}/subject"),
                        format!(
                            "`shot_style: {token}` dollies with a MOVING subject, but this \
                             subject has no matching `move-npc`/`move-actor` in the same effect \
                             group or sequence — add the move alongside the cutscene, or use a \
                             static style (`locked-off`, `push-in`)"
                        ),
                    ));
                }
            }
        }
    }

    /// Walk an effect list with its move scope; recurse into nested lists.
    fn walk_list(
        list: &[QuestEffect],
        outer_scope: &[(bool, String)],
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let mut scope = outer_scope.to_vec();
        moves_in(list, &mut scope);
        for (j, e) in list.iter().enumerate() {
            let epath = format!("{path}/{j}");
            check_shot_list(e, &scope, &epath, npc_ids, actor_ids, d);
            for (pseg, _kseg, inner) in e.nested_effect_lists_labeled() {
                // A sequence step shares this timeline's scope; reaction lists
                // (`on_arrive`/`on_caught`/`on_respawn`) fire at an unknowable
                // time and start fresh.
                let inherited: &[(bool, String)] = if matches!(&e.verb, Verb::Sequence { .. }) {
                    &scope
                } else {
                    &[]
                };
                walk_list(
                    inner,
                    inherited,
                    &format!("{epath}/{pseg}"),
                    npc_ids,
                    actor_ids,
                    d,
                );
            }
        }
    }

    for (i, q) in quests.quests.iter().enumerate() {
        for (key, effs) in &q.on_objective_complete {
            walk_list(
                effs,
                &[],
                &format!("/content/quests/{i}/on_objective_complete/{key}"),
                npc_ids,
                &actor_ids,
                d,
            );
        }
        walk_list(
            &q.on_complete,
            &[],
            &format!("/content/quests/{i}/on_complete"),
            npc_ids,
            &actor_ids,
            d,
        );
    }
    for (i, t) in quests.triggers.iter().enumerate() {
        walk_list(
            &t.effects,
            &[],
            &format!("/content/triggers/{i}/effects"),
            npc_ids,
            &actor_ids,
            d,
        );
    }
}
