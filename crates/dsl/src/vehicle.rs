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
    /// How many bodies of this vehicle stand (DSL v0.78, spec-0135 §13): a
    /// constant, or `{base, per_player}` — `base + per_player × (n − 1)` for
    /// the `n` players present when the story seats it (spec-0134 §4's
    /// count, the one shape). Absent = one body, byte-identical. A count
    /// that scales is measured at the seat, so it is seated `by-story`
    /// (`DW1115`).
    #[serde(default = "one_body", skip_serializing_if = "is_one_body")]
    pub count: crate::MobCount,
    /// Where each further body stands (DSL v0.78, spec-0135 §13): body `k`
    /// (from 0) is seated at `at` plus `k × spacing`, so a row of mounts at a
    /// starting line is one mark and one step. Required when more than one
    /// body can stand (`DW1115`); each body's seat is proven in its medium
    /// and inside `bounds` as the first one's is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<[i32; 3]>,
    /// The actor seated on this vehicle (DSL v0.78, spec-0135 §13): placed
    /// aboard by the pinned game's `ride <actor> mount <vehicle>` whenever
    /// both stand — when the vehicle is seated and when the actor is
    /// spawned, unleashed or stood up again — so a `move-actor` of it walks
    /// this body with the actor aboard. It takes one of the kind's seats. A
    /// vehicle of one body, standing on the actor's own mark (`DW1114`).
    /// Absent = a vehicle only players board, byte-identical.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rider: Option<crate::ActorId>,
}

fn one_body() -> crate::MobCount {
    crate::MobCount::Fixed(1)
}

fn is_one_body(c: &crate::MobCount) -> bool {
    *c == crate::MobCount::Fixed(1)
}

impl Vehicle {
    /// The seat offset of body `k` from the mark: `k × spacing`.
    pub fn body_offset(&self, k: u32) -> [i32; 3] {
        let s = self.spacing.unwrap_or([0, 0, 0]);
        let k = k as i32;
        [s[0] * k, s[1] * k, s[2] * k]
    }
}

/// **The vehicle actor `actor` is seated on** (spec-0135 §13): the first
/// vehicle whose `rider` names it, or `None` for an actor that stands on its
/// own feet. The one answer every reader of a rider asks (a second vehicle
/// naming the same actor is `DW1114`).
pub fn vehicle_of_rider<'c>(c: &'c crate::envelope::Campaign, actor: &str) -> Option<&'c Vehicle> {
    c.quests
        .content
        .vehicles
        .iter()
        .find(|v| v.rider.as_ref().is_some_and(|r| r.as_str() == actor))
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

crate::dw_code! {
    /// (spec-0135 §13) **A count of vehicle bodies the engine cannot seat**: a
    /// count of no body; more than one body with no `spacing` to stand the
    /// rest by (they would stand in one cell); or a count that scales with
    /// the party on a vehicle seated `at-load` — the party is measured when
    /// the story seats the vehicle, and no player is present when the world
    /// is built. Validation-tier (exit 1). Prescription: a count of one or
    /// more, a `spacing`, and `seated: by-story` for a count per player.
    pub const VEHICLE_COUNT_UNSEATABLE: DwCode = DwCode::new("DW1115", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0135 §13) **A seat an actor cannot take, or an act a seated
    /// actor cannot do**: a vehicle's `rider` on a vehicle of several bodies
    /// (one actor sits one body), an actor named the rider of two vehicles,
    /// or a rider whose own mark is not its vehicle's `at` (it is placed
    /// aboard where the vehicle stands, and every proof of the actor reads
    /// its mark); an `entrance` on a seated actor (the kind's own arrival is a
    /// lone body's); a `hop` of one (the hop moves its body by `tp`, and the
    /// pinned game's teleport of a rider stops its riding — `Entity.teleport`,
    /// `docs/reference/vehicles-animals-and-targets.md` §16); a `move-actor`
    /// of an actor seated on a boat or a minecart (the walk is proven on
    /// ground, and neither kind moves there); and a ride the party is
    /// declared to take on a vehicle whose every seat its actor fills.
    /// Validation-tier (exit 1). Prescription: one body per seated actor, the
    /// actor's mark written as the vehicle's `at`, `move-actor` to move a
    /// seated actor, or the act given to a body on its own feet.
    pub const RIDER_CANNOT: DwCode = DwCode::new("DW1114", ExitTier::Build);
}

/// Stage-5 rider checks (spec-0135 §13): the actor a vehicle seats is a
/// declared actor (`DW0112`) and one it can hold, and nothing asks it what a
/// seated body cannot do (`DW1114`).
fn rider_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let q = &c.quests.content;
    if q.vehicles.iter().all(|v| v.rider.is_none()) {
        return;
    }
    let mut seated: BTreeMap<&str, &str> = BTreeMap::new();
    for (i, v) in q.vehicles.iter().enumerate() {
        let Some(r) = &v.rider else {
            continue;
        };
        let at = format!("/content/vehicles/{i}/rider");
        let Some(a) = q.actors.iter().find(|a| a.id.as_str() == r.as_str()) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                at,
                format!(
                    "vehicle `{}` seats unknown actor `{r}` — declare it in the stage-5 `actors` \
                     list, or fix the reference",
                    v.id
                ),
            ));
            continue;
        };
        if let Some(first) = seated.insert(r.as_str(), v.id.as_str()) {
            seated.insert(r.as_str(), first);
            d.push(Diagnostic::error(
                RIDER_CANNOT,
                "quests",
                at.clone(),
                format!(
                    "actor `{r}` is the rider of vehicle `{first}` and of vehicle `{}`: a body \
                     sits one seat. Seat it on one of them",
                    v.id
                ),
            ));
        }
        if v.count.max() > 1 {
            d.push(Diagnostic::error(
                RIDER_CANNOT,
                "quests",
                at.clone(),
                format!(
                    "vehicle `{}` stands up to {} bodies and seats actor `{r}`: one actor sits \
                     one body. Declare a vehicle of one body for the actor — a vehicle each, \
                     where several actors ride",
                    v.id,
                    v.count.max()
                ),
            ));
        }
        let mark = crate::BodyRef::Actor(a).mark();
        if mark != v.at {
            d.push(Diagnostic::error(
                RIDER_CANNOT,
                "quests",
                at.clone(),
                format!(
                    "actor `{r}` stands at `{}` and its vehicle `{}` at `{}`: a seated actor is \
                     placed aboard where its vehicle stands, and every proof of the actor reads \
                     its own mark. Write the actor's `anchor` and `offset` as the vehicle's `at`",
                    mark.display(),
                    v.id,
                    v.at.display()
                ),
            ));
        }
        if let Some(ai) = q.actors.iter().position(|x| x.id == a.id)
            && a.entrance.is_some()
        {
            d.push(Diagnostic::error(
                RIDER_CANNOT,
                "quests",
                format!("/content/actors/{ai}/entrance"),
                format!(
                    "actor `{r}` is seated on vehicle `{}` and declares an `entrance`: an \
                     entrance is the kind's own arrival, a lone body's (a warden digging out of \
                     the ground), and a seated actor arrives aboard. Drop the `entrance`, or the \
                     `rider`",
                    v.id
                ),
            ));
        }
        if v.steering().is_some_and(|s| s.seats <= 1) {
            for (qi, quest) in q.quests.iter().enumerate() {
                for (oi, o) in quest.objectives.iter().enumerate() {
                    if o.ride().is_some_and(|id| id == &v.id) {
                        d.push(Diagnostic::error(
                            RIDER_CANNOT,
                            "quests",
                            format!("/content/quests/{qi}/objectives/{oi}/ride"),
                            format!(
                                "objective `{}` is ridden on vehicle `{}`, whose one seat actor \
                                 `{r}` sits: the party has no seat on it. Ride a vehicle with a \
                                 seat free, or take the `rider` off this one",
                                o.id(),
                                v.id
                            ),
                        ));
                    }
                }
            }
        }
    }
    crate::for_each_campaign_effect(c, &mut |path, _site, e| match &e.verb {
        crate::Verb::Hop { .. } => {
            for k in e.bodies() {
                if let crate::BodyKey::Actor(a) = &k
                    && let Some(v) = vehicle_of_rider(c, a.actor.as_str())
                {
                    d.push(Diagnostic::error(
                        RIDER_CANNOT,
                        "quests",
                        path.to_string(),
                        format!(
                            "`hop` lifts actor `{}`, which is seated on vehicle `{}`: a hop \
                             moves its body by teleport, and the pinned game's teleport of a \
                             rider sets it down off its seat. Walk the vehicle with \
                             `move-actor` (it carries the rider aboard), or give the hop to a \
                             body that stands on its own feet",
                            a.actor, v.id
                        ),
                    ));
                }
            }
        }
        crate::Verb::MoveActor { actor, .. } => {
            if let Some(v) = vehicle_of_rider(c, actor.as_str())
                && let Some(k) = v.kind()
                && k != VehicleKind::Mount
            {
                d.push(Diagnostic::error(
                    RIDER_CANNOT,
                    "quests",
                    path.to_string(),
                    format!(
                        "`move-actor` walks actor `{actor}`, which is seated on vehicle `{}`, a \
                         {}: a seated actor's walk carries its vehicle over ground, and a {} does \
                         not move there. Seat the actor on a mount to walk it, or leave it \
                         standing aboard",
                        v.id,
                        k.token(),
                        k.token()
                    ),
                ));
            }
        }
        _ => {}
    });
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
    // spec-0135 §13: the count is one the engine can seat.
    for (i, v) in vehicles.iter().enumerate() {
        let at = format!("/content/vehicles/{i}/count");
        if v.count.base() == 0 {
            d.push(Diagnostic::error(
                VEHICLE_COUNT_UNSEATABLE,
                "quests",
                at.clone(),
                format!(
                    "vehicle `{}` counts no body: a vehicle is at least one body. Give it a count \
                     of one or more, or remove it",
                    v.id
                ),
            ));
        }
        if v.count.max() > 1 && v.spacing.is_none_or(|s| s == [0, 0, 0]) {
            d.push(Diagnostic::error(
                VEHICLE_COUNT_UNSEATABLE,
                "quests",
                format!("/content/vehicles/{i}/spacing"),
                format!(
                    "vehicle `{}` stands up to {} bodies and declares no `spacing`: body k is \
                     seated at `at` plus k × `spacing`, and with none they would all stand in one \
                     cell. Declare `spacing` — the step from one body's seat to the next, e.g. \
                     `[2, 0, 0]` for a row two blocks apart",
                    v.id,
                    v.count.max()
                ),
            ));
        }
        if v.count.scales() && v.seated == VehicleSeated::AtLoad {
            d.push(Diagnostic::error(
                VEHICLE_COUNT_UNSEATABLE,
                "quests",
                at,
                format!(
                    "vehicle `{}` counts its bodies per player present and is seated at load: \
                     the party is counted when the story seats the vehicle, and no player is \
                     present while the world is built. Declare `seated: by-story` and seat it \
                     with `seat-vehicle` at the beat the party is there",
                    v.id
                ),
            ));
        }
    }
    // spec-0135 §13: the actor a vehicle seats.
    rider_checks(c, d);
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
