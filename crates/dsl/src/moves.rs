//! A body the story moves (spec-0136): the moves a staged body makes — a hop,
//! a pose, a turn, a held item, a size — the bodies an effect names, and the
//! cadence an effect keeps.
//!
//! Three things, each on the object it belongs to:
//!
//! * **the bodies** — [`BodyKey`], one staged body by class and id, the wire
//!   form of a [`BodyRef`]. An effect's `audience` names one (`{"npc": id}`,
//!   `{"actor": id}`) or a cast of them (`{"bodies": […]}`), and a `particle`'s
//!   or a `play-sound`'s `at` names where it happens the same way;
//! * **the moves** — [`Verb::Hop`], [`Verb::Pose`], [`Verb::Turn`],
//!   [`Verb::Face`], [`Verb::Hold`], [`Verb::Resize`], each one vanilla
//!   primitive measured on the pinned game (`docs/reference/npc-moves.md`);
//! * **the cadence** — [`Repeat`] and `stagger_ticks` on the effect envelope,
//!   for the verbs [`Verb::keeps_cadence`] names.
//!
//! # A hop never releases the body
//!
//! The arc is the game's own jump ([`hop_arc`]: the impulse, gravity and drag
//! of `LivingEntity`), walked by one relative teleport per tick while the body
//! stays `immovable`. Every height of the arc is a whole number of
//! [`HOP_UNIT`]ths of a block, so every step is a binary fraction the game
//! adds without rounding and the steps sum to exactly zero: the body ends on
//! the height it left.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::codes;
use crate::envelope::Campaign;
use crate::{
    ActorId, BodyRef, Diagnostic, DwCode, EffectAudience, ExitTier, ItemRegistry, Mark, NpcId,
    ParticleAt, QuestEffect, SoundAt, Verb,
};

crate::dw_code! {
    /// (spec-0136 §5) **A move its body cannot make.** A `pose` addressed to a
    /// body that does not ship as a `minecraft:mannequin` (only a skinned body
    /// has the five poses), or a `face` `to: "watch"` addressed to a body that
    /// has no watch to return to.
    ///
    /// Error tier, validation (exit 1).
    pub const MOVE_BODY_CANNOT: DwCode = DwCode::new("DW1101", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0136 §6) **A cadence the effect cannot keep.** `repeat` or
    /// `stagger_ticks` on a verb that happens once
    /// ([`Verb::keeps_cadence`] answers `false`); `stagger_ticks` on an effect
    /// that names fewer than two bodies; a cadence beside `audience: actor`
    /// (a repeated effect runs from the scheduler, which has no acting
    /// player); or `every_ticks` shorter than the move itself — a hop's arc, or
    /// the two ticks a size is held before a client is told.
    ///
    /// Error tier, validation (exit 1).
    pub const CADENCE_NOT_KEPT: DwCode = DwCode::new("DW1102", ExitTier::Build);
}

/// One staged body, by class and id — the document's spelling of a
/// [`BodyRef`]: `{"npc": "npc/<id>"}` or `{"actor": "actor/<id>"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BodyKey {
    /// A stage-2 NPC.
    Npc(NpcKey),
    /// A stage-5 actor.
    Actor(ActorKey),
}

/// `{"npc": id}` — an NPC's body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcKey {
    /// The NPC (stage-2 ref).
    pub npc: NpcId,
}

/// `{"actor": id}` — an actor's body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActorKey {
    /// The actor (stage-5 `actors` ref).
    pub actor: ActorId,
}

impl BodyKey {
    /// An NPC's key.
    pub fn npc(id: NpcId) -> Self {
        BodyKey::Npc(NpcKey { npc: id })
    }

    /// An actor's key.
    pub fn actor(id: ActorId) -> Self {
        BodyKey::Actor(ActorKey { actor: id })
    }

    /// The class word the document spells (`npc` / `actor`).
    pub fn class(&self) -> &'static str {
        match self {
            BodyKey::Npc(_) => "npc",
            BodyKey::Actor(_) => "actor",
        }
    }

    /// The declared id.
    pub fn id(&self) -> &str {
        match self {
            BodyKey::Npc(k) => k.npc.as_str(),
            BodyKey::Actor(k) => k.actor.as_str(),
        }
    }

    /// The body this key names, when the campaign declares it.
    pub fn resolve<'a>(&self, c: &'a Campaign) -> Option<BodyRef<'a>> {
        match self {
            BodyKey::Npc(k) => c
                .npcs
                .content
                .npcs
                .iter()
                .find(|n| n.id == k.npc)
                .map(BodyRef::Npc),
            BodyKey::Actor(k) => c
                .quests
                .content
                .actors
                .iter()
                .find(|a| a.id == k.actor)
                .map(BodyRef::Actor),
        }
    }
}

/// A pose a mannequin holds (spec-0136 §3.2) — the five the pinned game's
/// `Mannequin.VALID_POSES` admits. Any other name is accepted by the game's
/// command and silently becomes `standing`, which is why the set is closed
/// here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BodyPose {
    /// Upright.
    Standing,
    /// The sneak crouch.
    Crouching,
    /// Prone, as a swimmer.
    Swimming,
    /// Prone, as an elytra flier.
    FallFlying,
    /// Lying down.
    Sleeping,
}

impl BodyPose {
    /// Every pose, in declaration order.
    pub const ALL: [BodyPose; 5] = [
        BodyPose::Standing,
        BodyPose::Crouching,
        BodyPose::Swimming,
        BodyPose::FallFlying,
        BodyPose::Sleeping,
    ];

    /// The name the game's `pose` field takes.
    pub fn nbt(self) -> &'static str {
        match self {
            BodyPose::Standing => "standing",
            BodyPose::Crouching => "crouching",
            BodyPose::Swimming => "swimming",
            BodyPose::FallFlying => "fall_flying",
            BodyPose::Sleeping => "sleeping",
        }
    }
}

/// Where a [`Verb::Face`] turns its body: a yaw in degrees, a mark to look
/// at, or `"watch"` — back to the body's own watch (spec-0101).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FaceTo {
    /// A yaw in degrees, as the game measures it: 0 is south (+z), 90 west,
    /// ±180 north, −90 east. The pitch is levelled.
    Yaw(f64),
    /// A mark: the body looks from its eyes at the centre of the mark's cell,
    /// head and body together, pitch included.
    Mark(Mark),
    /// `"watch"` — the body's declared or default watch takes its facing back.
    Watch(WatchKeyword),
}

/// The literal `watch`, the one keyword [`FaceTo`] admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchKeyword {
    /// Back to the body's watch.
    Watch,
}

/// **An effect that happens more than once** (spec-0136 §4): `times` firings,
/// `every_ticks` apart, the first at the moment the effect fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Repeat {
    /// How many times the effect happens, the first included.
    #[schemars(range(min = 1))]
    pub times: u32,
    /// Ticks from one firing to the next.
    #[schemars(range(min = 1))]
    pub every_ticks: u32,
}

/// The least `scale` the pinned game keeps (`attribute … minecraft:scale`
/// clamps below it).
pub const MIN_SCALE: f64 = 0.0625;
/// The greatest `scale` the pinned game keeps.
pub const MAX_SCALE: f64 = 16.0;
/// Ticks a size must be held for a client to be told it: the mannequin's
/// attributes are sent on its sync tick, every second server tick.
pub const SCALE_HOLD_TICKS: u32 = 2;
/// The least peak a hop may declare, in blocks.
pub const MIN_HOP_HEIGHT: f64 = 0.25;
/// The greatest peak a hop may declare, in blocks.
pub const MAX_HOP_HEIGHT: f64 = 4.0;

/// The denominator of every height of a hop's arc: a height is a whole number
/// of `1 / HOP_UNIT` blocks, a binary fraction, so the game adds each step to
/// the body's `y` without rounding.
pub const HOP_UNIT: i32 = 1024;
/// The game's jump impulse (`LivingEntity.getJumpPower`, the `jump_strength`
/// attribute's default), in blocks per tick.
pub const JUMP_IMPULSE: f64 = 0.42;
/// The game's gravity (`minecraft:gravity`'s default), in blocks per tick².
pub const GRAVITY: f64 = 0.08;
/// The game's vertical drag, applied each tick after gravity.
pub const DRAG: f64 = 0.98;

/// The heights of the game's jump from an upward `impulse`, tick by tick,
/// until the body is back at or under its start.
fn jump_heights(impulse: f64) -> Vec<f64> {
    let mut out = Vec::new();
    let (mut y, mut v) = (0.0_f64, impulse);
    loop {
        y += v;
        v = (v - GRAVITY) * DRAG;
        if y <= 0.0 {
            return out;
        }
        out.push(y);
    }
}

/// **A hop's arc** (spec-0136 §3.1): the body's height above where it stood
/// on each tick of the hop, in [`HOP_UNIT`]ths of a block; the last entry is
/// always `0`.
///
/// With no `height` it is the game's own jump — the heights a released body
/// was measured at on the pinned server, rounded to the unit: peak 1.2522
/// blocks on tick 6, down on tick 12. With a `height` it is the same motion
/// from the impulse whose peak is that height (found by bisection, the same
/// arithmetic on every host).
pub fn hop_arc(height: Option<f64>) -> Vec<i32> {
    let impulse = match height {
        None => JUMP_IMPULSE,
        Some(h) => {
            let peak = |v: f64| jump_heights(v).into_iter().fold(0.0_f64, f64::max);
            let (mut lo, mut hi) = (0.0_f64, 2.0_f64);
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if peak(mid) < h {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            hi
        }
    };
    let mut arc: Vec<i32> = jump_heights(impulse)
        .into_iter()
        .map(|y| (y * f64::from(HOP_UNIT)).round() as i32)
        .collect();
    arc.push(0);
    arc
}

/// How many ticks a hop of this `height` lasts.
pub fn hop_ticks(height: Option<f64>) -> u32 {
    hop_arc(height).len() as u32
}

/// A whole number of [`HOP_UNIT`]ths of a block as the exact decimal a
/// relative coordinate takes (`431` → `0.4208984375`).
pub fn hop_decimal(units: i32) -> String {
    // 1 / 1024 = 0.0009765625 exactly: ten decimal places.
    let scaled = i64::from(units.unsigned_abs()) * 9_765_625;
    let (whole, frac) = (scaled / 10_000_000_000, scaled % 10_000_000_000);
    let sign = if units < 0 { "-" } else { "" };
    let frac = format!("{frac:010}");
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{frac}")
    }
}

impl Verb {
    /// **Is this verb a move a staged body makes?** (spec-0136) — it is
    /// addressed to bodies through the envelope's `audience` and to nothing
    /// else.
    pub fn moves_a_body(&self) -> bool {
        matches!(
            self,
            Verb::Hop { .. }
                | Verb::Pose { .. }
                | Verb::Turn { .. }
                | Verb::Face { .. }
                | Verb::Hold { .. }
                | Verb::Resize { .. }
        )
    }

    /// **May this verb keep a cadence?** (spec-0136 §4) — `repeat` and
    /// `stagger_ticks` on the envelope. `true` for a body's move and for the
    /// two point effects that go with one, a particle and a sound: none
    /// leaves anything a proof reads, so happening again changes no verdict.
    /// Every other verb happens once (`DW1102`).
    pub fn keeps_cadence(&self) -> bool {
        self.moves_a_body() || matches!(self, Verb::Particle { .. } | Verb::PlaySound { .. })
    }

    /// The ticks one firing of this verb lasts before it may be fired again
    /// on the same body: a hop's arc, the hold of a size; `1` otherwise.
    pub fn move_ticks(&self) -> u32 {
        match self {
            Verb::Hop { height } => hop_ticks(*height),
            Verb::Resize { .. } => SCALE_HOLD_TICKS,
            _ => 1,
        }
    }
}

impl QuestEffect {
    /// **The bodies this effect names**, in the order the document lists them
    /// — the audience's when it names staged bodies, else those a `particle`
    /// or a `play-sound` happens `at`. The order is the stagger's.
    pub fn bodies(&self) -> Vec<BodyKey> {
        match &self.audience {
            Some(EffectAudience::Body(a)) => return vec![BodyKey::actor(a.clone())],
            Some(EffectAudience::Npc(n)) => return vec![BodyKey::npc(n.clone())],
            Some(EffectAudience::Bodies(b)) => return b.clone(),
            _ => {}
        }
        match &self.verb {
            Verb::Particle {
                at: ParticleAt::Bodies(at),
                ..
            } => at.bodies.clone(),
            Verb::PlaySound {
                at: Some(SoundAt::Bodies { bodies }),
                ..
            } => bodies.clone(),
            _ => Vec::new(),
        }
    }

    /// Whether the effect declares a cadence.
    pub fn has_cadence(&self) -> bool {
        self.repeat.is_some() || self.stagger_ticks.is_some()
    }
}

/// **The moves, the bodies and the cadence, at every effect root**
/// (spec-0136): `DW0112` for a body no stage declares, `DW0100` for a number
/// outside its schema bound, `DW0143` and `DW0898` for a held item,
/// `DW0942` for a move with no body to make it, `DW1101` and `DW1102`.
pub(crate) fn moves_checks(c: &Campaign, items: &dyn ItemRegistry, d: &mut Vec<Diagnostic>) {
    fn descend(
        c: &Campaign,
        items: &dyn ItemRegistry,
        stage: &'static str,
        path: String,
        eff: &QuestEffect,
        d: &mut Vec<Diagnostic>,
    ) {
        moves_one(c, items, stage, &path, eff, d);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(c, items, stage, format!("{path}/{pseg}/{j}"), inner, d);
            }
        }
    }
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            descend(c, items, site.stage, format!("{}/{i}", site.path), eff, d);
        }
    });
}

/// Where in the effect its body list is written.
fn bodies_pointer(eff: &QuestEffect) -> &'static str {
    match &eff.audience {
        Some(EffectAudience::Bodies(_)) => "audience/bodies",
        Some(EffectAudience::Npc(_) | EffectAudience::Body(_)) => "audience",
        _ => "at/bodies",
    }
}

fn moves_one(
    c: &Campaign,
    items: &dyn ItemRegistry,
    stage: &'static str,
    path: &str,
    eff: &QuestEffect,
    d: &mut Vec<Diagnostic>,
) {
    let tag = eff.verb.tag();
    let keys = eff.bodies();
    let at = bodies_pointer(eff);
    // An `{"actor": id}` audience is resolved by the perception bundle
    // (`DW0112`); every other body named here is resolved here.
    let listed = !matches!(eff.audience, Some(EffectAudience::Body(_)));
    let mut bodies: Vec<BodyRef<'_>> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        match k.resolve(c) {
            Some(b) => bodies.push(b),
            None if listed => {
                let pointer = if at == "audience" {
                    format!("{path}/audience/{}", k.class())
                } else {
                    format!("{path}/{at}/{i}/{}", k.class())
                };
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    stage,
                    pointer,
                    format!(
                        "the effect names {} `{}`, which stage {} does not declare.",
                        k.class(),
                        k.id(),
                        if k.class() == "npc" { 2 } else { 5 }
                    ),
                ));
            }
            None => {}
        }
    }
    if listed && keys.is_empty() && !matches!(at, "audience") && body_list_declared(eff) {
        d.push(Diagnostic::error(
            codes::SCHEMA,
            stage,
            format!("{path}/{at}"),
            format!(
                "`{tag}` names an empty list of bodies. The schema's minimum is one; name the \
                 bodies as `{{\"npc\": …}}` or `{{\"actor\": …}}`"
            ),
        ));
    }
    for (i, k) in keys.iter().enumerate() {
        if keys[..i].contains(k) {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/{at}/{i}"),
                format!(
                    "`{tag}` names {} `{}` twice. A body is listed once: its place in the list \
                     is its place in the stagger",
                    k.class(),
                    k.id()
                ),
            ));
        }
    }

    // A move is made by a body, so it names one.
    if eff.verb.moves_a_body() && !eff.audience.as_ref().is_some_and(EffectAudience::is_staged) {
        d.push(Diagnostic::error(
            crate::perception::PERCEPTION_AUDIENCE_ON_A_PARTY_FACT,
            stage,
            format!("{path}/audience"),
            format!(
                "`{tag}` {}, and a `{tag}` is a move a staged body makes — an NPC or an actor, \
                 standing where the story put it. This one names none. Name the body — \
                 `\"audience\": {{\"npc\": \"npc/<id>\"}}`, `{{\"actor\": \"actor/<id>\"}}`, or a \
                 cast, `{{\"bodies\": [{{\"npc\": …}}, {{\"actor\": …}}]}}`",
                match &eff.audience {
                    None => "states no `audience`".to_string(),
                    Some(a) => format!("declares `audience` `{}`", a.token()),
                }
            ),
        ));
    }

    match &eff.verb {
        Verb::Hop { height: Some(h) } if !(MIN_HOP_HEIGHT..=MAX_HOP_HEIGHT).contains(h) => {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/height"),
                format!(
                    "`hop` `height` is {h}. A hop peaks between {MIN_HOP_HEIGHT} and \
                     {MAX_HOP_HEIGHT} blocks above where the body stands (absent is the game's \
                     own jump, 1.25). Write a height in that range"
                ),
            ));
        }
        Verb::Turn { by } if *by == 0.0 || !(-180.0..=180.0).contains(by) => {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/by"),
                format!(
                    "`turn` `by` is {by}. A turn is a number of degrees other than 0 between \
                     -180 and 180, clockwise seen from above when positive; a spin is a `turn` \
                     that repeats"
                ),
            ));
        }
        Verb::Face {
            to: crate::FaceTo::Yaw(y),
        } if !(-180.0..=180.0).contains(y) => {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/to"),
                format!(
                    "`face` `to` is the yaw {y}. A yaw is between -180 and 180 degrees: 0 faces \
                     south (+z), 90 west, 180 north, -90 east"
                ),
            ));
        }
        Verb::Resize { scale } if !(MIN_SCALE..=MAX_SCALE).contains(scale) => {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/scale"),
                format!(
                    "`resize` `scale` is {scale}. The pinned game keeps a scale between \
                     {MIN_SCALE} and {MAX_SCALE} and clamps anything else to the nearer end, so \
                     the body would not be the size written. Write a scale in that range"
                ),
            ));
        }
        Verb::Hold {
            slot,
            item: Some(item),
        } => {
            if !items.contains(item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    stage,
                    format!("{path}/item"),
                    format!(
                        "`hold` names item `{item}`, which is not an item of the pinned 1.21.11 \
                         game."
                    ),
                ));
            } else {
                for b in &bodies {
                    let entity = b.worn_entity();
                    let Some(m) = crate::equipment::judge(entity, *slot, item, items) else {
                        continue;
                    };
                    if !m.any() {
                        continue;
                    }
                    let kind = items
                        .equippable(item)
                        .kind()
                        .expect("judged, so the kind is known");
                    d.push(Diagnostic::error(
                        codes::EQUIPMENT_UNSHOWN,
                        stage,
                        format!("{path}/slot"),
                        crate::equipment::misfit_message(
                            &format!("`hold` on {} `{}`", b.class().to_lowercase(), b.id()),
                            entity,
                            *slot,
                            item,
                            kind,
                            &m,
                        ),
                    ));
                }
            }
        }
        Verb::Pose { pose } => {
            for b in &bodies {
                if b.skin().is_none() {
                    d.push(Diagnostic::error(
                        MOVE_BODY_CANNOT,
                        stage,
                        format!("{path}/pose"),
                        format!(
                            "`pose` `{}` is addressed to {} `{}`, which ships as `{}`. Only a \
                             skinned body — a `minecraft:mannequin` — has the five poses \
                             (standing, crouching, swimming, fall-flying, sleeping); on any \
                             other body the game has no `pose` field to write. Give the body a \
                             `skin`, or leave it out of this `pose`",
                            pose.nbt().replace('_', "-"),
                            b.class().to_lowercase(),
                            b.id(),
                            b.worn_entity()
                        ),
                    ));
                }
            }
        }
        Verb::Face {
            to: crate::FaceTo::Watch(_),
        } => {
            let watchers = crate::body_watch_sites(c);
            for b in &bodies {
                if !watchers.iter().any(|w| w.body.id() == b.id()) {
                    d.push(Diagnostic::error(
                        MOVE_BODY_CANNOT,
                        stage,
                        format!("{path}/to"),
                        format!(
                            "`face` `to: \"watch\"` is addressed to {} `{}`, which watches \
                             nobody: it declares no `watch`{}. There is no watch to hand its \
                             facing back to. Declare the body's `watch`, or face it `to` a yaw \
                             or a mark",
                            b.class().to_lowercase(),
                            b.id(),
                            if b.holds_its_facing() {
                                " (`\"watch\": \"none\"`)"
                            } else {
                                " and is not a body a player talks to"
                            }
                        ),
                    ));
                }
            }
        }
        _ => {}
    }

    cadence_one(stage, path, eff, keys.len(), d);
}

/// Whether the document wrote a list of bodies on this effect at all.
fn body_list_declared(eff: &QuestEffect) -> bool {
    matches!(eff.audience, Some(EffectAudience::Bodies(_)))
        || matches!(
            &eff.verb,
            Verb::Particle {
                at: ParticleAt::Bodies(_),
                ..
            } | Verb::PlaySound {
                at: Some(SoundAt::Bodies { .. }),
                ..
            }
        )
}

/// The cadence rules of one effect (`DW0100` for a bound, `DW1102`).
fn cadence_one(
    stage: &'static str,
    path: &str,
    eff: &QuestEffect,
    bodies: usize,
    d: &mut Vec<Diagnostic>,
) {
    if !eff.has_cadence() {
        return;
    }
    let tag = eff.verb.tag();
    if !eff.verb.keeps_cadence() {
        let field = if eff.repeat.is_some() {
            "repeat"
        } else {
            "stagger_ticks"
        };
        d.push(Diagnostic::error(
            CADENCE_NOT_KEPT,
            stage,
            format!("{path}/{field}"),
            format!(
                "`{tag}` declares `{field}`, and a `{tag}` happens once: what it leaves behind \
                 is read by the build's proofs as one event. A cadence is kept by a body's \
                 move (`hop`, `pose`, `turn`, `face`, `hold`, `resize`), a `particle` and a \
                 `play-sound`. Repeat this beat as steps of a `sequence`"
            ),
        ));
        return;
    }
    if let Some(r) = eff.repeat {
        for (field, v) in [("times", r.times), ("every_ticks", r.every_ticks)] {
            if v == 0 {
                d.push(Diagnostic::error(
                    codes::SCHEMA,
                    stage,
                    format!("{path}/repeat/{field}"),
                    format!(
                        "`{tag}` `repeat` `{field}` is 0, and the schema's minimum is 1. Write \
                         how many times the effect happens and the ticks between two firings"
                    ),
                ));
            }
        }
        let needs = eff.verb.move_ticks();
        if r.times > 1 && r.every_ticks >= 1 && r.every_ticks < needs {
            d.push(Diagnostic::error(
                CADENCE_NOT_KEPT,
                stage,
                format!("{path}/repeat/every_ticks"),
                match &eff.verb {
                    Verb::Hop { .. } => format!(
                        "`hop` repeats every {} ticks, and this hop is in the air for {needs}: \
                         a body asked to hop before it has landed does not hop, so fewer hops \
                         would happen than are declared. Write `every_ticks` of at least \
                         {needs}",
                        r.every_ticks
                    ),
                    _ => format!(
                        "`{tag}` repeats every {} tick(s), and a size reaches a client on the \
                         body's sync tick, every second tick: a size held for one tick is never \
                         seen. Write `every_ticks` of at least {needs}",
                        r.every_ticks
                    ),
                },
            ));
        }
    }
    if let Some(s) = eff.stagger_ticks {
        if s == 0 {
            d.push(Diagnostic::error(
                codes::SCHEMA,
                stage,
                format!("{path}/stagger_ticks"),
                format!(
                    "`{tag}` `stagger_ticks` is 0, and the schema's minimum is 1. A cast that \
                     moves together declares no stagger"
                ),
            ));
        } else if bodies < 2 {
            d.push(Diagnostic::error(
                CADENCE_NOT_KEPT,
                stage,
                format!("{path}/stagger_ticks"),
                format!(
                    "`{tag}` declares `stagger_ticks` and names {bodies} bod{}: a stagger is \
                     the delay between one body of a cast and the next, in the order they are \
                     listed, so it needs at least two. List the cast (`{{\"bodies\": […]}}`), \
                     or remove `stagger_ticks`",
                    if bodies == 1 { "y" } else { "ies" }
                ),
            ));
        }
    }
    if matches!(eff.audience, Some(EffectAudience::Actor)) {
        d.push(Diagnostic::error(
            CADENCE_NOT_KEPT,
            stage,
            format!("{path}/audience"),
            format!(
                "`{tag}` declares a cadence and `audience: actor`. A repeated or staggered \
                 effect runs from the scheduler, which has no acting player to address. \
                 Address the party, or fire the effect once"
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The heights a released body was measured at on the pinned server
    /// (`docs/reference/npc-moves.md` §1), tick by tick.
    const MEASURED: [f64; 12] = [
        0.42, 0.7532, 1.0013, 1.1661, 1.2492, 1.2522, 1.1768, 1.0244, 0.7967, 0.4952, 0.1213, 0.0,
    ];

    #[test]
    fn the_default_arc_is_the_measured_jump() {
        let arc = hop_arc(None);
        assert_eq!(arc.len(), MEASURED.len());
        for (units, measured) in arc.iter().zip(MEASURED) {
            let h = f64::from(*units) / f64::from(HOP_UNIT);
            assert!(
                (h - measured).abs() < 0.001,
                "{h} is not the measured {measured}"
            );
        }
    }

    #[test]
    fn every_arc_ends_where_it_began_and_never_dips() {
        for h in [None, Some(0.25), Some(0.6), Some(1.0), Some(2.5), Some(4.0)] {
            let arc = hop_arc(h);
            assert_eq!(arc.last(), Some(&0), "{h:?}");
            assert!(arc[..arc.len() - 1].iter().all(|u| *u > 0), "{h:?}");
            let mut prev = 0;
            let mut sum = 0;
            for u in &arc {
                sum += u - prev;
                prev = *u;
            }
            assert_eq!(sum, 0, "{h:?}: the steps sum to zero");
        }
    }

    #[test]
    fn a_declared_height_is_the_peak() {
        for h in [0.25, 0.6, 1.0, 2.5, 4.0] {
            let peak = hop_arc(Some(h)).into_iter().max().unwrap();
            assert!(
                (f64::from(peak) / f64::from(HOP_UNIT) - h).abs() <= 1.0 / f64::from(HOP_UNIT),
                "{h}"
            );
        }
    }

    #[test]
    fn a_step_is_an_exact_binary_fraction() {
        assert_eq!(hop_decimal(0), "0");
        assert_eq!(hop_decimal(1024), "1");
        assert_eq!(hop_decimal(512), "0.5");
        assert_eq!(hop_decimal(-1), "-0.0009765625");
        assert_eq!(hop_decimal(430), "0.419921875");
        // Every step parses back to the very number of units it was written
        // from, and adding it to a height in the same unit is exact.
        for units in -5000..5000 {
            let v: f64 = hop_decimal(units).parse().unwrap();
            assert_eq!(v * f64::from(HOP_UNIT), f64::from(units));
            assert_eq!((64.5 + v - 64.5) * f64::from(HOP_UNIT), f64::from(units));
        }
    }
}
