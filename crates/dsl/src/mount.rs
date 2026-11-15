//! A body that rides another (spec-0135 §13): the mount a wave mob or an
//! actor is summoned on.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Actor, MobAttributes, MobEquipment, Wave, WaveMob};

/// **The body a body rides** (spec-0135 §13): summoned with the rider as its
/// passenger, through the pinned game's `Passengers` entity data — the shape
/// vanilla itself spawns a mounted body in (a zombie horse's
/// `finalizeSpawn` seats an iron-spear zombie on it;
/// `docs/reference/vehicles-animals-and-targets.md` §16).
///
/// A field of the class that summons the rider — a wave mob stack and an
/// actor — because the mount is how that body arrives, not a body of its own:
/// it is never counted, censused or mustered as the rider's class (a wave's
/// kill count and census count riders), it leaves the world with its rider's
/// removal, and a mount a rider has fallen from stays where it is, as in
/// vanilla.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    /// The vanilla entity ridden, validated against the pinned registry
    /// (`DW0173`), e.g. `minecraft:zombie_horse`, `minecraft:pig`.
    pub entity: String,
    /// What the mount wears, in the wave mob's shape (`DW0898` holds each
    /// piece to what the body shows); drop chances zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// The mount's attribute overrides, in the wave mob's shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
}

/// The rider a [`Mount`] carries.
#[derive(Clone, Copy, Debug)]
pub enum MountRider<'a> {
    /// A wave's mob stack: every body of the stack rides one mount of its own.
    WaveMob {
        /// The wave.
        wave: &'a Wave,
        /// The stack's index in `mobs`.
        index: usize,
        /// The stack.
        mob: &'a WaveMob,
    },
    /// An actor: its puppet and its twin each ride one.
    Actor(&'a Actor),
}

/// One declared mount, where it is written and who rides it.
#[derive(Clone, Debug)]
pub struct MountSite<'a> {
    /// The JSON pointer of the `mount` object in stage 5.
    pub path: String,
    /// The rider.
    pub rider: MountRider<'a>,
    /// The mount.
    pub mount: &'a Mount,
}

/// **Every mount the campaign declares**, in declaration order: wave mob
/// stacks, then actors. The one enumeration every reader of the class walks.
pub fn mount_sites(c: &crate::envelope::Campaign) -> Vec<MountSite<'_>> {
    let q = &c.quests.content;
    let mut out = Vec::new();
    for (wi, w) in q.waves.iter().enumerate() {
        for (index, mob) in w.mobs.iter().enumerate() {
            if let Some(mount) = &mob.mount {
                out.push(MountSite {
                    path: format!("/content/waves/{wi}/mobs/{index}/mount"),
                    rider: MountRider::WaveMob {
                        wave: w,
                        index,
                        mob,
                    },
                    mount,
                });
            }
        }
    }
    for (ai, a) in q.actors.iter().enumerate() {
        if let Some(mount) = &a.mount {
            out.push(MountSite {
                path: format!("/content/actors/{ai}/mount"),
                rider: MountRider::Actor(a),
                mount,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::registry::EntityRegistry;
use crate::{BodyKey, Verb};

crate::dw_code! {
    /// (spec-0135 §13) **A body that rides, asked to do what a rider cannot**:
    /// a `hop` of an actor that rides a mount — the hop moves its body by
    /// `tp`, and the pinned game's teleport of a rider stops its riding
    /// (`Entity.teleport` calls `stopRiding` unless the move is a passenger's,
    /// `docs/reference/vehicles-animals-and-targets.md` §16); or an actor that
    /// rides and declares an `entrance` — the kind's own arrival (a warden
    /// digging out of the ground) is a lone body's. Validation-tier (exit 1).
    /// Prescription: walk the mount (`move-actor` carries the rider aboard),
    /// or drop the mount for the beat that needs the lone body.
    pub const RIDER_CANNOT: DwCode = DwCode::new("DW1114", ExitTier::Build);
}

/// Stage-5 mount checks (spec-0135 §13): a known entity (`DW0173`) and the
/// acts a rider cannot do (`DW1114`).
pub(crate) fn mount_checks(
    c: &crate::envelope::Campaign,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let sites = mount_sites(c);
    if sites.is_empty() {
        return;
    }
    for s in &sites {
        if !entities.contains(&s.mount.entity) {
            d.push(Diagnostic::error(
                codes::ENTITY_UNKNOWN,
                "quests",
                format!("{}/entity", s.path),
                format!(
                    "mount entity `{}` is not a known 1.21.11 entity id — use a valid namespaced \
                     entity id (e.g. `minecraft:zombie_horse`)",
                    s.mount.entity
                ),
            ));
        }
        if let MountRider::Actor(a) = s.rider
            && a.entrance.is_some()
        {
            d.push(Diagnostic::error(
                RIDER_CANNOT,
                "quests",
                s.path.clone(),
                format!(
                    "actor `{}` rides a `{}` and declares an `entrance`: an entrance is the \
                     kind's own arrival, a lone body's (a warden digging out of the ground), and \
                     a body that rides arrives aboard its mount. Drop the `entrance`, or the \
                     `mount`",
                    a.id, s.mount.entity
                ),
            ));
        }
    }
    let riders: std::collections::BTreeSet<&str> = sites
        .iter()
        .filter_map(|s| match s.rider {
            MountRider::Actor(a) => Some(a.id.as_str()),
            MountRider::WaveMob { .. } => None,
        })
        .collect();
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        if !matches!(e.verb, Verb::Hop { .. }) {
            return;
        }
        for k in e.bodies() {
            if let BodyKey::Actor(a) = &k
                && riders.contains(a.actor.as_str())
            {
                d.push(Diagnostic::error(
                    RIDER_CANNOT,
                    "quests",
                    path.to_string(),
                    format!(
                        "`hop` lifts actor `{}`, which rides a mount: a hop moves its body by \
                         teleport, and the pinned game's teleport of a rider sets it down off \
                         its mount. Walk the mount with `move-actor` (it carries the rider \
                         aboard), or give the hop to a body that stands on its own feet",
                        a.actor
                    ),
                ));
            }
        }
    });
}
