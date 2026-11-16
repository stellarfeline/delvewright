//! A body that rides another (spec-0135 §13): the mount a wave mob is
//! summoned on.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{MobAttributes, MobEquipment, Wave, WaveMob};

/// **The body a body rides** (spec-0135 §13): summoned with the rider as its
/// passenger, through the pinned game's `Passengers` entity data — the shape
/// vanilla itself spawns a mounted body in (a zombie horse's
/// `finalizeSpawn` seats an iron-spear zombie on it;
/// `docs/reference/vehicles-animals-and-targets.md` §16).
///
/// A field of the wave mob stack, the class that summons its rider, because
/// the mount is how that body arrives, not a body of its own:
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
    /// The body's vanilla variant (DSL v0.83, [`crate::BodyVariant`]): its
    /// coat, colour, collar, kind or trade, by the pinned game's own property
    /// names and values. Absent = the type's default (a summon skips the
    /// random pick), byte-identical. A property the body does not carry is
    /// `DW1131`; a value it does not take is `DW1132`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<crate::BodyVariant>,
    /// What the mount wears, in the wave mob's shape (`DW0898` holds each
    /// piece to what the body shows); drop chances zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// The mount's attribute overrides, in the wave mob's shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
}

/// One declared mount: where it is written and the wave stack that rides it.
#[derive(Clone, Debug)]
pub struct MountSite<'a> {
    /// The JSON pointer of the `mount` object in stage 5.
    pub path: String,
    /// The wave.
    pub wave: &'a Wave,
    /// The stack's index in `mobs`.
    pub index: usize,
    /// The stack: every body of it rides one mount of its own.
    pub mob: &'a WaveMob,
    /// The mount.
    pub mount: &'a Mount,
}

/// **Every mount the campaign declares**, in declaration order. The one
/// enumeration every reader of the class walks. An actor rides a vehicle of
/// `vehicles[]` (its `rider`), never a mount.
pub fn mount_sites(c: &crate::envelope::Campaign) -> Vec<MountSite<'_>> {
    let q = &c.quests.content;
    let mut out = Vec::new();
    for (wi, w) in q.waves.iter().enumerate() {
        for (index, mob) in w.mobs.iter().enumerate() {
            if let Some(mount) = &mob.mount {
                out.push(MountSite {
                    path: format!("/content/waves/{wi}/mobs/{index}/mount"),
                    wave: w,
                    index,
                    mob,
                    mount,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, codes};
use crate::registry::EntityRegistry;

/// Stage-5 mount checks (spec-0135 §13): a known entity (`DW0173`).
pub(crate) fn mount_checks(
    c: &crate::envelope::Campaign,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for s in &mount_sites(c) {
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
    }
}
