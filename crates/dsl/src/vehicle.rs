//! Vehicles: a body that carries another (spec-0135).

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Facing, Mark, MobAttributes, MobEquipment, StealthZone, VehicleId};

/// A stage-5 **vehicle** (spec-0135 §2): a boat, the rider's minecart or a
/// saddled mount the campaign places, a player boards and leaves, and a route
/// may ride.
///
/// One class, whose **kind** is derived from the entity through the pinned
/// steering table ([`steering`]), never declared: what the engine must know of
/// every kind is where it is placed, how a body gets on and off, where the
/// body is while on, and the medium the kind moves in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Vehicle {
    /// Unique vehicle id (`vehicle/<kebab>`).
    pub id: VehicleId,
    /// The vanilla entity: a boat or raft, `minecraft:minecart`, or a mount the
    /// pinned game steers under a rider (`entity-steering-1.21.11.json`). Any
    /// other entity is `DW1096`; an unknown id is `DW0173`.
    pub entity: String,
    /// The seat: the anchor's cell plus `offset` (spec-0066, `DW0897`). The
    /// body is placed here at load, by `seat-vehicle`, and by the re-seat.
    pub at: Mark,
    /// Initial facing (default `south`). For a minecart, also the way along
    /// its rail the engine pushes it when a body boards: a cart facing across
    /// its rail is `DW1097`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// The region the vehicle lives in — an anchor-centred box. It cuts the
    /// medium the vehicle moves through: a ride never leaves it. Required. A
    /// mark outside it is `DW1099`. (A riderless body away from its mark is
    /// re-seated there wherever it stands.)
    pub bounds: StealthZone,
    /// A name tag over the body (`DW0983`: a vehicle is one body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What a mount wears, in the wave mob's shape; the saddle is the engine's
    /// unless a piece is declared there. A piece the body does not show is
    /// `DW0898`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equipment: Option<MobEquipment>,
    /// A mount's attribute overrides, in the wave mob's shape; absent, the
    /// registry's defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MobAttributes>,
    /// When the body is first placed (spec-0135 §2): `at-load` (absent) seats
    /// it in `setup_finish`; `by-story` places nothing until a `seat-vehicle`
    /// fires — a ride the story hands out at a beat.
    #[serde(default, skip_serializing_if = "VehicleSeated::is_at_load")]
    pub seated: VehicleSeated,
    /// Where a riderless body is once a rider has left it (spec-0135 §3):
    /// `at-its-mark` (absent) re-seats it at its mark within 100 ticks, where
    /// every proof of a ride starts; `where-left` keeps it where it was left —
    /// a mount waits there without its AI — until the story seats it again.
    #[serde(default, skip_serializing_if = "VehicleLeft::is_at_its_mark")]
    pub left: VehicleLeft,
}

/// When a vehicle's body is first placed (spec-0135 §2).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum VehicleSeated {
    /// In `setup_finish`, with the re-seat armed.
    #[default]
    AtLoad,
    /// Only by a `seat-vehicle`; nothing stands at the mark before it.
    ByStory,
}

impl VehicleSeated {
    /// Whether this is the default.
    pub fn is_at_load(&self) -> bool {
        *self == VehicleSeated::AtLoad
    }
}

/// Where a riderless vehicle is once its rider has left it (spec-0135 §3).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum VehicleLeft {
    /// Re-seated at its mark within 100 ticks.
    #[default]
    AtItsMark,
    /// Where its rider left it, until a `seat-vehicle` seats it at its mark.
    WhereLeft,
}

impl VehicleLeft {
    /// Whether this is the default.
    pub fn is_at_its_mark(&self) -> bool {
        *self == VehicleLeft::AtItsMark
    }
}

/// What kind of vehicle an entity is (spec-0135 §1, §3).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum VehicleKind {
    /// A boat or a raft: floats, driven by its rider's client.
    Boat,
    /// The rider's minecart: driven by its rails.
    Minecart,
    /// A saddled mob the game steers under a rider: driven by its rider's
    /// client and, where the kind needs one, the steering item.
    Mount,
}

impl VehicleKind {
    /// The kebab token.
    pub fn token(self) -> &'static str {
        match self {
            VehicleKind::Boat => "boat",
            VehicleKind::Minecart => "minecart",
            VehicleKind::Mount => "mount",
        }
    }
}

/// The medium a vehicle moves through (spec-0135 §3).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum VehicleMedium {
    /// Surface water.
    Water,
    /// Rail cells joined by their shapes.
    Rails,
    /// Standable ground at the body's footprint.
    Ground,
}

/// One row of the steering table: what the pinned game says of an entity
/// that carries a rider.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steering {
    /// The vehicle kind.
    pub kind: VehicleKind,
    /// The medium the kind moves in.
    pub medium: VehicleMedium,
    /// Riders it seats.
    pub seats: u32,
    /// Whether it is summoned `Tame` (the horse family throws an untamed
    /// rider).
    #[serde(default)]
    pub tame: bool,
    /// The item a rider holds to steer it, where the kind needs one.
    #[serde(default)]
    pub steering_item: Option<String>,
}

/// The pinned steering table (`crates/dsl/data/entity-steering-1.21.11.json`,
/// regenerated per MC pin by `tools/maintenance/extract-entity-steering.py`;
/// `crates/delvec/data/PROVENANCE.md`), keyed by namespaced entity id.
pub fn steering_table() -> &'static BTreeMap<String, Steering> {
    static TABLE: std::sync::LazyLock<BTreeMap<String, Steering>> =
        std::sync::LazyLock::new(|| {
            serde_json::from_str(include_str!("../data/entity-steering-1.21.11.json"))
                .expect("vendored entity-steering table is valid JSON")
        });
    &TABLE
}

/// What the pinned game says of `entity` as a vehicle — `None` for an entity
/// that carries no rider. Accepts a bare or namespaced id.
pub fn steering(entity: &str) -> Option<&'static Steering> {
    steering_table().get(&crate::namespaced_entity(entity))
}

impl Vehicle {
    /// The steering row, when the entity carries a rider.
    pub fn steering(&self) -> Option<&'static Steering> {
        steering(&self.entity)
    }

    /// The kind, when the entity carries a rider.
    pub fn kind(&self) -> Option<VehicleKind> {
        self.steering().map(|s| s.kind)
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::{AnchorRegistry, EntityRegistry};
use crate::validate::{AnchorProviders, station_kind_diag};

crate::dw_code! {
    /// (spec-0135 §8) **An entity that carries no rider** in `vehicles[]`: not
    /// a boat, the rider's minecart or a mount the pinned game steers on
    /// ground (a chest boat, a hopper cart, a llama, a zombie; a nautilus is
    /// steered in water, which the engine does not model); or a mount whose
    /// steering item no path puts in a hand before a ride (raised by the
    /// compiler, which reads the path). Validation-tier (exit 1).
    /// Prescription: an entity of the steering table, or the steering item
    /// in a kit or a `give-item` before the ride.
    pub const VEHICLE_CARRIES_NO_RIDER: DwCode = DwCode::new("DW1096", ExitTier::Build);
}

/// The entities that carry a rider, by kind, for a diagnostic.
fn riders_by_kind() -> String {
    let mut by: BTreeMap<VehicleKind, Vec<&str>> = BTreeMap::new();
    for (id, s) in steering_table() {
        if s.medium != VehicleMedium::Water || s.kind == VehicleKind::Boat {
            by.entry(s.kind).or_default().push(id.as_str());
        }
    }
    by.iter()
        .map(|(k, ids)| format!("{}: {}", k.token(), ids.join(", ")))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Stage-5 vehicle checks (spec-0135 §2, §8): id syntax and uniqueness, a
/// known entity that carries a rider (`DW1096`), and the seat and bounds
/// anchors resolvable. The mark's piece is `DW0897`, the seat's medium
/// `DW1097` and the bounds `DW1099` — compiler-tier, they need the world.
pub(crate) fn vehicle_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let vehicles = &c.quests.content.vehicles;
    let declared: BTreeSet<&str> = vehicles.iter().map(|v| v.id.as_str()).collect();
    for (i, v) in vehicles.iter().enumerate() {
        crate::ids::id_syntax!(d, v.id, "quests", format!("/content/vehicles/{i}/id"));
    }
    crate::ids::dup_check(
        vehicles
            .iter()
            .enumerate()
            .map(|(i, v)| (v.id.as_str(), format!("/content/vehicles/{i}/id"))),
        "quests",
        "vehicle",
        d,
    );
    let providers = AnchorProviders::build(c, anchors);
    for (i, v) in vehicles.iter().enumerate() {
        let at = |tail: &str| format!("/content/vehicles/{i}{tail}");
        if !entities.contains(&v.entity) {
            d.push(Diagnostic::error(
                codes::ENTITY_UNKNOWN,
                "quests",
                at("/entity"),
                format!(
                    "vehicle entity `{}` is not a known 1.21.11 entity id — use a valid \
                     namespaced entity id (e.g. `minecraft:oak_boat`)",
                    v.entity
                ),
            ));
        } else {
            match v.steering() {
                None => d.push(Diagnostic::error(
                    VEHICLE_CARRIES_NO_RIDER,
                    "quests",
                    at("/entity"),
                    format!(
                        "vehicle `{}` is a `{}`, which carries no rider in the pinned game — it \
                         is not a boat, the rider's minecart or a mount the game steers under a \
                         rider (a chest boat and a chest, hopper or furnace cart carry cargo; a \
                         llama is not steered). The entities that carry a rider are {}. Choose \
                         one of them, or declare this body as an actor",
                        v.id,
                        v.entity,
                        riders_by_kind()
                    ),
                )),
                Some(s) if s.kind == VehicleKind::Mount && s.medium == VehicleMedium::Water => {
                    d.push(Diagnostic::error(
                        VEHICLE_CARRIES_NO_RIDER,
                        "quests",
                        at("/entity"),
                        format!(
                            "vehicle `{}` is a `{}`, a mount the pinned game steers in water; \
                             the engine models a mount on ground and a boat on water, and \
                             proves no ride it cannot model. Choose a ground mount or a boat \
                             ({})",
                            v.id,
                            v.entity,
                            riders_by_kind()
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
        for (field, anchor, what) in [
            ("/at/anchor", v.at.anchor.as_str(), "a vehicle's seat"),
            (
                "/bounds/anchor",
                v.bounds.anchor.as_str(),
                "a vehicle's bounds centre",
            ),
        ] {
            if let Some(f) = station_kind_diag(
                &providers,
                anchor,
                crate::layout::StationKind::Point,
                what,
                "quests",
                at(field),
            ) {
                d.push(f);
            }
            if !providers.resolvable(anchor) {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    at(field),
                    format!(
                        "vehicle `{}` names anchor `{anchor}` ({what}), which no prefab bound in \
                         this campaign provides — {}",
                        v.id,
                        providers.anchor_remedy(
                            "use an anchor the prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
    }
    // Every objective's `ride` names a declared vehicle.
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Some(id) = o.ride()
                && !declared.contains(id.as_str())
            {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/ride"),
                    format!(
                        "objective `{}` rides unknown vehicle `{id}` — declare it in the stage-5 \
                         `vehicles` list, or fix the reference",
                        o.id()
                    ),
                ));
            }
        }
    }
    // Every `seat-vehicle` / `despawn-vehicle` names a declared vehicle.
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        if let Some(id) = e.vehicle_ref()
            && !declared.contains(id.as_str())
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names unknown vehicle `{id}` — declare it in the stage-5 `vehicles` \
                     list, or fix the reference",
                    e.verb.tag()
                ),
            ));
        }
    });
}
