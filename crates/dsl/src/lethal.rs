//! Lethal volumes and the kinds of damage they deal.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Guard, LethalVolumeId, StealthZone};

#[cfg(doc)]
use crate::Verb;

/// The damage type of a [`Verb::DamagePlayers`] effect (DSL v0.6). A
/// **curated** subset of the vanilla 1.21.11 damage-type registry: every variant
/// respects the `keepInventory` death flow (a gamerule, so all deaths do) and does
/// **not** bypass a totem of undying — the totem-bypassing `out_of_world` /
/// `generic_kill` types are deliberately excluded, so a scripted consequence can
/// never silently void a player's held totem. Modelled as an enum (not a free
/// string) so an unknown type is a schema rejection (`DW0100`) and needs no separate
/// registry / diagnostic. `generic` is the default: command damage that respects
/// totems + absorption but ignores armor, so a scripted hit lands regardless of gear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DamageKind {
    /// `minecraft:generic` — armor-ignoring command damage (default).
    Generic,
    /// `minecraft:magic` — magical damage.
    Magic,
    /// `minecraft:wither` — the wither/withering effect's damage type.
    Wither,
    /// `minecraft:on_fire` — burning damage.
    Fire,
    /// `minecraft:drown` — drowning damage.
    Drown,
    /// `minecraft:freeze` — powder-snow freezing damage.
    Freeze,
    /// `minecraft:fall` — fall damage.
    Fall,
    /// `minecraft:lightning_bolt` — a lightning strike's damage type.
    LightningBolt,
    /// `minecraft:explosion` — a (non-player) explosion's damage type.
    Explosion,
}

impl DamageKind {
    /// The vanilla `minecraft:` damage-type id emitted to `/damage`.
    pub fn id(self) -> &'static str {
        match self {
            DamageKind::Generic => "minecraft:generic",
            DamageKind::Magic => "minecraft:magic",
            DamageKind::Wither => "minecraft:wither",
            DamageKind::Fire => "minecraft:on_fire",
            DamageKind::Drown => "minecraft:drown",
            DamageKind::Freeze => "minecraft:freeze",
            DamageKind::Fall => "minecraft:fall",
            DamageKind::LightningBolt => "minecraft:lightning_bolt",
            DamageKind::Explosion => "minecraft:explosion",
        }
    }
}

/// A stage-5 **lethal volume** (DSL v0.10, spec-0031): a declared box that kills
/// whatever enters it, and states — in the campaign's own words — what killed it.
///
/// # A mechanism, not a fiction
///
/// The commissioning case was a cliff whose fall must be fatal, but nothing here
/// knows what a cliff is: a lava pit, an acid pool, an out-of-bounds plane and the
/// bottom of a lift shaft are the same declaration, differently dressed and
/// differently worded. The alternative considered and **rejected** for the cliff
/// was making the world's horizon void so the fall kills anyway — that changes
/// approved art to obtain a behaviour, and it serves exactly one fiction.
///
/// # It is geometry, so the completability proof owns it
///
/// A volume that kills is, for a route, a volume that cannot be crossed. The
/// compiler models its cells as impassable in the same navigation world every
/// other reachability proof runs on, exactly as a `close-gate`'s sealed region is
/// modelled solid — so a forced path that has no way to an objective except
/// through a lethal volume is a build failure (`DW0510`) naming the volume, never
/// a shipped delve that kills the player on the critical path. A respawn seat
/// inside one is the death loop that failure mode ends in, and is its own
/// error (`DW0511`).
///
/// # It rides the death edge that already exists
///
/// The kill is a `/damage`, exactly like `damage-players`, a trap payload or a
/// timed gate's crush. Everything downstream — the vanilla `deathCount` edge
/// (`dw.deaths` / `dw.death_ack`), the checkpoint re-seat (`cp_respawn_check`),
/// `keep_inventory` — sees an ordinary death and needs no second detector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LethalVolume {
    /// Unique lethal-volume id (`lethal/<kebab>`).
    pub id: LethalVolumeId,
    /// The volume: an anchor-centred box (`anchor ± extent`).
    ///
    /// **Deliberately the existing zone type**, not a second struct with the same
    /// two fields. `StealthZone` is already the engine's anchor-centred box object
    /// class — `damage-players`'s `in` filter reuses it, and the compiler resolves
    /// every one of them through the single `Plan::zone_box`. A private twin here
    /// would be `tools/ci/check-capability-ownership.py` check C by construction, and
    /// would fork the resolution the very next time a box grew a capability.
    pub region: StealthZone,
    /// What this volume says when it kills — a player-visible line, inventoried
    /// under `lethal.<id>.message` and translated like every other one.
    ///
    /// Required, and deliberately so: a volume with no words is a player who dies
    /// with no idea why, and there is no compiler-owned default that could be
    /// right for a cliff, a lava pit and an acid pool at once.
    pub message: String,
    /// The damage type the kill is dealt with (default [`DamageKind::Generic`]).
    ///
    /// This is what words vanilla's own broadcast — `fall` says the party member
    /// fell from a high place, `on_fire` that they burnt to a crisp — while
    /// [`Self::message`] says what the *place* was. The curated enum is shared
    /// with `damage-players`, so a lethal volume can no more void a held totem
    /// than a scripted hit can.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_type: Option<DamageKind>,
    /// **The blocks that show a player this floor kills** (spec-0062 §3).
    ///
    /// A lava surface, a magma floor, a bed of spikes, a burning strip: the
    /// danger is level with the footing because the block *is* the signal. This
    /// is where a volume says so, and it is a claim about the assembled bytes
    /// rather than a word that switches a rule off — `DW0891` checks it per
    /// caught cell against the block under or in that cell, refuses a listed
    /// block no caught cell bears out, and refuses at validation any id vanilla
    /// does not hurt a body with.
    ///
    /// Empty is the ordinary case and means the ordinary thing: this volume
    /// catches no floor the party walks, because it sits at the bottom of a pit
    /// or a course under a lake's surface. It does **not** exempt a cell from
    /// the walk graph — a visible hazard is still a hazard, and the router
    /// refuses every cell of the keep-out either way.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shown_by: Vec<String>,
    /// **When this volume kills** (spec-0088): the one [`Guard`] every other
    /// gated object carries, verbatim. Absent, the volume is live from
    /// world-load to the end. Present, it is live while its gate holds —
    /// `requires_flags` is a pit that kills from a beat on, `forbids_flags` a
    /// shaft that kills until one, `requires_state` a chamber that kills while
    /// a party datum stands in range.
    ///
    /// A volume's liveness is a fact about the place, so the gate is a fact
    /// about the party: `when: {}` (a stage with no term) and a term on a
    /// `player`-scoped datum are refused at the document (`DW0953`). The
    /// navigation world holds the volume per quest configuration, and `DW0891`
    /// judges what shows it in every configuration a body can meet it in,
    /// including the last one before it goes live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Guard>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeSet;

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

crate::dw_code! {
    /// (spec-0031, DSL v0.10) A `lethal_volumes[]` entry's `message` is blank.
    ///
    /// The volume would still kill — and would kill in silence, which is the one
    /// thing the declaration exists to prevent. There is no compiler default that
    /// could be right for a cliff, a lava pit and an acid pool at once, so a blank
    /// wording is refused rather than papered over: a gate that reports green
    /// while the player learns nothing is exactly the vacuous pass CLAUDE.md names.
    pub const LETHAL_MESSAGE_BLANK: DwCode = DwCode::new("DW0512", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0062) **A killing volume and what shows it disagree.**
    ///
    /// One rule, three shapes, and every remedy each names is admitted by the
    /// others — which is why one code carries all of them (spec-0062 §4).
    ///
    /// * **Caught floor that shows nothing.** Some cell the party can walk to
    ///   lies in the volume's keep-out, and the block under or in it is not one
    ///   of the volume's `shown_by`. The player reads stone and dies on it. The
    ///   remedy is the geometry's: lower the volume so its keep-out's top course
    ///   lies under the floor, draw its `extent` in, or author a block vanilla
    ///   hurts with under those cells and declare it. Never mark walkable-looking
    ///   ground unwalkable — the compiler knows and the player does not.
    /// * **A declared signal the bytes do not hold.** A `shown_by` block under
    ///   or in no caught cell, whether the list is wrong or the volume catches
    ///   nothing at all. The `DW0887` shape, on a volume instead of a waterline.
    /// * **A `shown_by` naming a block vanilla does not hurt a body with.** The
    ///   document arm, and the only one answerable with nothing placed:
    ///   `minecraft:stone` over stone is borne out by the bytes and shows
    ///   nothing.
    ///
    /// The world arm is raised by `delvec::compiler::lethal` over the final
    /// assembled world (whether a cell is floor is a fact about the settled
    /// bytes, so nothing before assembly can answer it); the document arm here,
    /// by [`crate::validate`].
    pub const LETHAL_INVISIBLE: DwCode = DwCode::new("DW0891", ExitTier::Build);
}

/// Stage-5 lethal-volume structural checks (DSL v0.10, spec-0031): id syntax and
/// uniqueness, a resolvable region anchor, and a wording the player can actually
/// read (`DW0512`).
///
/// Everything geometric is deliberately absent here and lives in the compiler:
/// where the box lands, what it overlaps and whether the party can still finish
/// are questions about the *solved layout*, which the DSL crate does not have.
pub(crate) fn lethal_volume_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let volumes = &c.quests.content.lethal_volumes;
    if volumes.is_empty() {
        return;
    }
    // The same "is this anchor provided by some bound prefab" rule every stage-5
    // anchor reference uses; a pool area defers the answer to the compiler.
    let providers = AnchorProviders::build(c, anchors);
    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    for (i, v) in volumes.iter().enumerate() {
        if !v.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/lethal_volumes/{i}/id"),
                format!(
                    "malformed lethal-volume id `{}` — lethal-volume ids must be lowercase \
                     kebab-case with the `lethal/` prefix (e.g. `lethal/cliff-fall`)",
                    v.id
                ),
            ));
        }
        if !seen_id.insert(v.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/lethal_volumes/{i}/id"),
                format!("duplicate lethal-volume id `{}`", v.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            v.region.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a lethal volume's region centre",
            "quests",
            format!("/content/lethal_volumes/{i}/region/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(v.region.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/lethal_volumes/{i}/region/anchor"),
                format!(
                    "lethal-volume anchor `{}` is not provided by any prefab bound in this \
                     campaign — {}",
                    v.region.anchor,
                    providers.anchor_remedy(
                        "use an anchor the prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        // `DW0891`, document arm (spec-0062 §4): a `shown_by` id that is a known
        // block and not one vanilla hurts a body with. Nothing has to be placed
        // to know it, so it is refused here rather than three passes later, and
        // the message prints the set the author may choose from — a remedy that
        // named no candidates would be a remedy an author has to guess at.
        //
        // An id the pinned version does not have at all is `DW0193`, the code
        // every block id in the DSL validates under, and deliberately not this
        // rule's: a typo is a typo wherever it is written.
        for (j, block) in v.shown_by.iter().enumerate() {
            if crate::blocks::BlockRegistry::v1_21_11()
                .validate_state_string(block)
                .is_err()
            {
                d.push(Diagnostic::error(
                    codes::BLOCK_UNKNOWN,
                    "quests",
                    format!("/content/lethal_volumes/{i}/shown_by/{j}"),
                    format!(
                        "lethal volume `{}` declares `shown_by` block `{block}`, which is not a \
                         block state of Minecraft Java 1.21.11",
                        v.id
                    ),
                ));
                continue;
            }
            if crate::blockshape::hurts_body(block) {
                continue;
            }
            d.push(Diagnostic::error(
                LETHAL_INVISIBLE,
                "quests",
                format!("/content/lethal_volumes/{i}/shown_by/{j}"),
                format!(
                    "lethal volume `{}` declares `shown_by` block `{block}`, which vanilla does \
                     not hurt a body with — so it shows a player nothing, and floor made of it \
                     reads as safe however this volume is declared. `shown_by` says what the \
                     player SEES; it is not a word that switches the rule off. Name the block \
                     that shows the danger, from the set vanilla hurts a body with: {}.",
                    v.id,
                    crate::blockshape::HURTING_BLOCKS_1_21_11
                        .iter()
                        .map(|b| format!("`{b}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ));
        }
        if v.message.trim().is_empty() {
            d.push(Diagnostic::error(
                LETHAL_MESSAGE_BLANK,
                "quests",
                format!("/content/lethal_volumes/{i}/message"),
                format!(
                    "lethal volume `{}` declares a blank `message`, so it would kill in silence \
                     — the one thing this declaration exists to prevent. Write the line the \
                     player reads as they die (`The undertow takes you.`); there is no compiler \
                     default that could be right for a cliff, a lava pit and an acid pool at \
                     once.",
                    v.id
                ),
            ));
        }
    }
}

/// `DW0953`'s empty-gate shape (spec-0088 §3.2): a `when` with no term is not a
/// stage. The player-scoped shape is raised beside `DW0503` in
/// [`state_checks`], where every gate's `requires_state` is already read.
pub(crate) fn lethal_stage_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, v) in c.quests.content.lethal_volumes.iter().enumerate() {
        if v.when.is_some() && v.gate().is_empty() {
            d.push(Diagnostic::error(
                codes::LETHAL_STAGE_GATE,
                "quests",
                format!("/content/lethal_volumes/{i}/when"),
                format!(
                    "lethal volume `{}` declares `when: {{}}` — a stage with no term. An \
                     always-live volume is spelled by leaving `when` out; to stage it, name a \
                     flag (`requires_flags` / `forbids_flags`) or a `party`-scoped datum \
                     (`requires_state`)",
                    v.id.as_str()
                ),
            ));
        }
    }
}
