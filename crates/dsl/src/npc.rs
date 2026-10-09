//! Stage 2 — NPCs: who they are, what they look like and where they stand.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::{is_false, is_zero3};
use crate::{AnchorId, AreaId, BodyTraversal, NpcId};

#[cfg(doc)]
use crate::{EncounterTier, Mark, Verb};

/// Stage 2 payload: the campaign's NPCs (casting sheets).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcsContent {
    /// All NPCs in the campaign.
    pub npcs: Vec<Npc>,
}

/// A stationary NPC bound to an area anchor (a casting sheet, spec-0001 v0.2).
///
/// Stage 2 carries **no dialogue** — the structured [`Persona`] is the character
/// contract the stage-6 `dialogue` tree must honor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Npc {
    /// Unique NPC id.
    pub id: NpcId,
    /// Player-facing name.
    pub name: String,
    /// NPC role.
    pub role: Role,
    /// The area this NPC stands in (stage-1 ref).
    pub area: AreaId,
    /// The prefab anchor this NPC stands on.
    pub anchor: AnchorId,
    /// Integer `[x, y, z]` block offset from `anchor` (spec-0066, default
    /// `[0, 0, 0]`): the NPC stands at the [`Mark`] the two fields spell.
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
    /// The vanilla entity to re-dress, e.g. `minecraft:villager`.
    pub base_entity: String,
    /// The structured persona (character contract for stage 6).
    pub persona: Persona,
    /// Optional player-model skin (DSL v0.4, spec-0008 §6 / spec-0009). When set,
    /// the compiler emits a `minecraft:mannequin` body carrying this skin profile
    /// instead of re-dressing `base_entity`; the interaction hitbox is unchanged.
    /// Non-skinned NPCs are byte-identical to v0.3.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin: Option<NpcSkin>,
    /// Deferred entrance (DSL v0.6): when `true` the NPC is **not** summoned at
    /// world init — its body and interaction hitbox only appear when a
    /// [`Verb::SpawnNpc`] fires, at this same `anchor`. The dual of
    /// `despawn-npc`: a character with a scripted entrance must not stand at its
    /// mark as a statue from minute one. A deferred NPC that no `spawn-npc` ever
    /// spawns is unreachable content (`DW0197`). Default `false` = summoned at
    /// init, byte-identical to pre-0.6.
    #[serde(default, skip_serializing_if = "is_false")]
    pub deferred: bool,
    /// What this body can do when it moves (DSL v0.11, spec-0034). Absent = the
    /// class the compiler derives from `base_entity` (or from `minecraft:mannequin`
    /// when `skin` is set — the body that actually ships). See [`BodyTraversal`]:
    /// the declaration must change a verdict or it is `DW0454`, and it can never
    /// reach the error tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traversal: Option<BodyTraversal>,
}

/// A mannequin NPC's player-model skin (DSL v0.4). The skin PNG is sourced from
/// the campaign dir's `skins/<texture_id>.png` and ships in the per-delve resource
/// pack at `assets/delvewright/textures/npc/<campaign_id>/<texture_id>.png`, which
/// is what the mannequin's `profile.texture` resolves to. The delve's own
/// directory is stamped on at emission ([`crate::l10n::namespace_skin_textures`])
/// — a client merges every applied pack's textures into ONE space, so two delves
/// that both cast a `keeper` would otherwise wear each other's faces. Nothing a
/// creator writes or names on disk carries it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NpcSkin {
    /// Skin id: the PNG basename under `skins/`, and the last segment of the
    /// resource-pack texture path (a bare kebab token; validated by `DW0190`).
    pub texture_id: String,
    /// Player model. **Required** (spec-0009): an omitted model renders slim, so
    /// a wide skin on a slim model is distorted — the compiler always emits it.
    pub model: SkinModel,
    /// The overlay layers this mannequin does **not** draw (spec-0097 §5). A
    /// mannequin draws all seven of the player model's second-layer parts unless
    /// told otherwise, and the skin's paint on a hidden part is not shown. Absent
    /// or empty draws every layer and emits nothing; otherwise the list is
    /// emitted as the mannequin's own `hidden_layers` field, in this order. A
    /// layer named twice is `DW0980`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_layers: Vec<SkinLayer>,
}

/// One of the player model's second-layer parts, as the pinned client's
/// `PlayerModelPart` names it (spec-0097 §2.4). `left` and `right` are the
/// model's own, not the observer's. `crates/delvec/tests/skin_parts.rs` holds
/// these tokens equal to the layers `crates/delvec/data/model-parts-1.21.11.json`
/// read from the jar.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SkinLayer {
    /// The cape, where the profile carries one.
    Cape,
    /// The torso's overlay shell.
    Jacket,
    /// The left arm's overlay shell.
    LeftSleeve,
    /// The right arm's overlay shell.
    RightSleeve,
    /// The left leg's overlay shell.
    LeftPantsLeg,
    /// The right leg's overlay shell.
    RightPantsLeg,
    /// The head's overlay shell.
    Hat,
}

impl SkinLayer {
    /// Every layer, in the client's own order.
    pub const ALL: [SkinLayer; 7] = [
        SkinLayer::Cape,
        SkinLayer::Jacket,
        SkinLayer::LeftSleeve,
        SkinLayer::RightSleeve,
        SkinLayer::LeftPantsLeg,
        SkinLayer::RightPantsLeg,
        SkinLayer::Hat,
    ];

    /// The vanilla id a mannequin's `hidden_layers` list carries.
    pub fn token(self) -> &'static str {
        match self {
            SkinLayer::Cape => "cape",
            SkinLayer::Jacket => "jacket",
            SkinLayer::LeftSleeve => "left_sleeve",
            SkinLayer::RightSleeve => "right_sleeve",
            SkinLayer::LeftPantsLeg => "left_pants_leg",
            SkinLayer::RightPantsLeg => "right_pants_leg",
            SkinLayer::Hat => "hat",
        }
    }
}

/// Player-model shape for a mannequin skin (`wide` = classic/Steve, `slim` =
/// Alex). Emitted verbatim into the mannequin `profile.model`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SkinModel {
    /// Classic 4-pixel arms (Steve).
    Wide,
    /// Slim 3-pixel arms (Alex).
    Slim,
}

impl SkinModel {
    /// The vanilla `profile.model` token.
    pub fn token(self) -> &'static str {
        match self {
            SkinModel::Wide => "wide",
            SkinModel::Slim => "slim",
        }
    }
}

/// A structured casting sheet. Structure lives in the
/// keys; every value is free text. `archetype`, `speech_style` and `motivation`
/// are required; the rest are optional.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    /// One-line character archetype (required).
    pub archetype: String,
    /// How the NPC speaks — register, tics, formality (required).
    pub speech_style: String,
    /// Emotional bearing toward the player (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub demeanor: Option<String>,
    /// What the NPC wants (required).
    pub motivation: String,
    /// Something the NPC hides (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// Backstory colour (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backstory: Option<String>,
    /// Attitudes toward other same-stage NPCs (optional; refs validated).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<Relationship>,
}

/// One persona relationship: an attitude toward another same-stage NPC.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Relationship {
    /// The other NPC (stage-2 ref, validated within stage 2).
    pub npc: NpcId,
    /// Free-text attitude toward that NPC.
    pub attitude: String,
}

/// What a speaking part does. A schema enum offers what the engine accepts,
/// so there are two of them: how hard a fight is billed is [`EncounterTier`] on
/// the body that fights (a `waves[]` entry or a stage-5 actor), not a role here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Gives and advances quests.
    QuestGiver,
    /// Flavor only.
    Flavor,
}

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::ids::is_kebab;
use crate::{Objective, QuestEffect};

crate::dw_code! {
    /// (v0.4) A dialogue `talk-to` or `interact` objective targets an NPC after a
    /// `despawn-npc` removes it on a reachable path (spec-0008 §5).
    pub const NPC_DESPAWNED_REF: DwCode = DwCode::new("DW0195", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) A stage-2 NPC declares `deferred: true` but **no** `spawn-npc` effect
    /// anywhere in the campaign ever summons it — the NPC never enters the world,
    /// so its dialogue tree and any `talk-to` on it are unreachable content. The
    /// NPC-lifecycle dual of [`NPC_DESPAWNED_REF`] / `DW0195`.
    ///
    /// (0197/0198 were *reserved* by spec-0011's draft and released when that spec
    /// renumbered to `DW0340`/`DW0341`; they were never emitted by any code.)
    pub const NPC_NEVER_SPAWNED: DwCode = DwCode::new("DW0197", ExitTier::Build);
}

crate::dw_code! {
    /// (v0.6) A `talk-to` on a `deferred` NPC activates before the NPC can exist:
    /// every `spawn-npc` for it sits in a quest that is a strict *descendant* of the
    /// objective's quest on the stage-4 DAG (and none fires from a trigger or
    /// dialogue), so the objective provably activates on an empty anchor.
    pub const NPC_SPAWNED_LATE: DwCode = DwCode::new("DW0198", ExitTier::Build);
}

/// Collect every NPC `e` (or an effect nested inside it) despawns **on every
/// playthrough that runs `e` at all** — the only despawns `DW0195` may reason
/// about, because its model is quest-DAG order with no branch semantics.
///
/// Two things stop the descent, and each is a real branch rather than a
/// convenience:
///
/// - **A flag gate.** An effect carrying `requires_flags`/`forbids_flags` fires
///   only when campaign state says so. The island's Perimedes walks out through
///   the cave mouth and despawns *only* on the flee branch (`flag/flee`); the
///   `talk-to`s that follow live on the sealed-in branch. Counting that despawn
///   would reject a perfectly playable delve. Branch-conditional reachability is
///   the branch-coherent completability proof's job (`DW0204`), not this rule's.
/// - **A lifecycle reaction bundle.** `set-checkpoint`'s `on_respawn` runs only if
///   a player dies and `begin-stealth`'s `on_caught` only if one is caught, so
///   neither is guaranteed. A `sequence` step and a `move-*` `on_arrive` *are*
///   guaranteed once their parent runs, so the descent continues through them.
fn unconditional_despawns<'a>(e: &'a QuestEffect, out: &mut Vec<&'a crate::ids::NpcId>) {
    if !e.requires_flags().is_empty() || !e.forbids_flags().is_empty() {
        return;
    }
    if let Some(npc) = e.despawn_npc() {
        out.push(npc);
    }
    for (_pseg, kseg, list) in e.nested_effect_lists_labeled() {
        if kseg == "respawn" || kseg == "caught" {
            continue;
        }
        for inner in list {
            unconditional_despawns(inner, out);
        }
    }
}

/// DW0195: a `talk-to` targeting an NPC despawned by an effect that runs strictly
/// before it on the quest dependency graph. Conservative: quest-ancestor despawn
/// (via `on_complete`) or same-quest earlier-objective despawn (via
/// `on_objective_complete` on a prerequisite `after` objective).
pub(crate) fn despawned_ref_check(
    c: &Campaign,
    _npc_ids: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    // Quest transitive ancestors (a quest completes before its dependents start).
    let deps: BTreeMap<&str, &Vec<crate::ids::QuestId>> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), &q.depends_on))
        .collect();
    let ancestors = |q: &str| -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        let mut stack = vec![q];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = deps.get(cur) {
                for dep in ds.iter() {
                    if out.insert(dep.as_str()) {
                        stack.push(dep.as_str());
                    }
                }
            }
        }
        out
    };

    // Where each npc is despawned: quests that despawn it on completion. Deep, but
    // only through effects that are **certain to run** (see
    // [`unconditional_despawns`]) — a `despawn-npc` nested one level down in a
    // `sequence` step removes the NPC exactly as thoroughly as a top-level one, and
    // the shallow scan this replaces walked straight past it.
    let mut despawn_quest: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            let mut npcs = Vec::new();
            unconditional_despawns(e, &mut npcs);
            for npc in npcs {
                despawn_quest
                    .entry(npc.as_str())
                    .or_default()
                    .insert(q.id.as_str());
            }
        }
    }
    if despawn_quest.is_empty() {
        return;
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let anc = ancestors(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { npc, .. } = o
                && let Some(dq) = despawn_quest.get(npc.as_str())
                && dq.iter().any(|dqid| anc.contains(dqid))
            {
                d.push(Diagnostic::error(
                    NPC_DESPAWNED_REF,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/npc"),
                    format!(
                        "`talk-to` targets npc `{npc}`, which a prerequisite quest despawns — the \
                         npc is gone by the time this objective activates; talk to `{npc}` before \
                         the quest that despawns it, or drop the `despawn-npc`"
                    ),
                ));
            }
        }
    }
}

/// `deferred` NPC staging proofs (DSL v0.6), the dual of `despawned_ref_check`:
///
/// * `DW0112` — a dialogue `spawn-npc` naming an unknown NPC (the quest-effect form
///   is covered by `check_effect_v04`).
/// * `DW0197` — a `deferred: true` NPC that **no** `spawn-npc` anywhere summons: it
///   never enters the world, so its tree and any `talk-to` on it are dead content.
/// * `DW0198` — a `talk-to` on a deferred NPC that provably activates before the
///   NPC exists: every `spawn-npc` for it lives in a quest that is a strict DAG
///   *descendant* of the objective's quest. Conservative by construction — a spawn
///   from a trigger, from dialogue, or from the objective's own quest is not
///   DAG-ordered, so it suppresses the proof rather than risking a false positive.
pub(crate) fn deferred_npc_checks(c: &Campaign, npc_ids: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    use crate::DialogueEffect;
    let deferred: BTreeSet<&str> = c
        .npcs
        .content
        .npcs
        .iter()
        .filter(|n| n.deferred)
        .map(|n| n.id.as_str())
        .collect();

    // Spawn sites. `quest_spawns`: npc -> quests whose effects spawn it (DAG-ordered).
    // `loose_spawns`: npcs spawned from a trigger or a dialogue option — sources with
    // no position on the quest DAG.
    let mut quest_spawns: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut loose_spawns: BTreeSet<String> = BTreeSet::new();
    for q in &c.quests.content.quests {
        let qid = q.id.as_str().to_string();
        crate::validate::for_each_effect_deep(q, |_path, eff| {
            if let Some(npc) = eff.spawn_npc() {
                quest_spawns
                    .entry(npc.as_str().to_string())
                    .or_default()
                    .insert(qid.clone());
            }
        });
    }
    for t in &c.quests.content.triggers {
        crate::validate::for_each_trigger_effect_deep(t, |_path, eff| {
            if let Some(npc) = eff.spawn_npc() {
                loose_spawns.insert(npc.as_str().to_string());
            }
        });
    }
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (m, eff) in opt.effects.iter().enumerate() {
                    let DialogueEffect::SpawnNpc { npc } = eff else {
                        continue;
                    };
                    if !npc_ids.contains(npc.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/npc"),
                            format!(
                                "dialogue `spawn-npc` references unknown npc `{npc}` — declare it \
                                 in stage 2 or correct the reference"
                            ),
                        ));
                        continue;
                    }
                    loose_spawns.insert(npc.as_str().to_string());
                }
            }
        }
    }

    // DW0197: deferred but never spawned anywhere.
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        if !n.deferred {
            continue;
        }
        let id = n.id.as_str();
        if quest_spawns.contains_key(id) || loose_spawns.contains(id) {
            continue;
        }
        d.push(Diagnostic::error(
            NPC_NEVER_SPAWNED,
            "npcs",
            format!("/content/npcs/{i}/deferred"),
            format!(
                "npc `{id}` is `deferred: true` but no `spawn-npc` effect anywhere in the \
                 campaign summons it — it never enters the world, so its dialogue tree (and any \
                 `talk-to` on it) is unreachable content. Add a `spawn-npc {{ npc: \"{id}\" }}` \
                 effect at the beat where the character should walk in, or drop `deferred` so it \
                 stands at its anchor from world init. Do NOT delete the dialogue tree to silence \
                 this — every stage-2 npc needs one (`DW0152`)"
            ),
        ));
    }
    if deferred.is_empty() {
        return;
    }

    // DW0198: a `talk-to` on a deferred npc whose every spawn site is a strict DAG
    // descendant of the objective's quest.
    let ancestors = crate::validate::quest_ancestors(c);
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::TalkTo { npc, .. } = o else {
                continue;
            };
            let npc = npc.as_str();
            if !deferred.contains(npc) || loose_spawns.contains(npc) {
                continue;
            }
            let Some(sqs) = quest_spawns.get(npc) else {
                continue; // never spawned at all — already DW0197
            };
            let all_later = sqs.iter().all(|sq| {
                sq.as_str() != q.id.as_str()
                    && ancestors
                        .get(sq.as_str())
                        .is_some_and(|anc| anc.contains(q.id.as_str()))
            });
            if !all_later {
                continue;
            }
            let names: Vec<&str> = sqs.iter().map(|s| s.as_str()).collect();
            d.push(Diagnostic::error(
                NPC_SPAWNED_LATE,
                "quests",
                format!("/content/quests/{qi}/objectives/{oi}/npc"),
                format!(
                    "`talk-to` targets deferred npc `{npc}`, but every `spawn-npc` for it fires \
                     in a quest that depends on this one (`{}`) — the objective activates on an \
                     empty anchor and can never complete. Move the `spawn-npc` to this quest or \
                     one of its prerequisites, or move the `talk-to` after the entrance. Do NOT \
                     drop `deferred` just to pass this — that puts the character back on stage \
                     from minute one",
                    names.join("`, `")
                ),
            ));
        }
    }
}

/// spec-0009: a mannequin skin's `texture_id` is a bare kebab token (`DW0190`).
///
/// A `texture_id` names a FILE, and two bodies may wear one file: the bake
/// reads it once and both summons point at the one pack texture
/// (`read_skins`), and a skin's per-body choices (`model`, `hidden_layers`)
/// ride the body, not the file (spec-0097 §4.3). So only the id's shape is
/// refused here.
pub(crate) fn npc_skin_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(skin) = &npc.skin
            && !is_kebab(&skin.texture_id)
        {
            d.push(Diagnostic::error(
                codes::SKIN_INVALID,
                "npcs",
                format!("/content/npcs/{i}/skin/texture_id"),
                format!(
                    "skin `texture_id` `{}` is malformed — it must be a bare kebab token \
                     (e.g. `keeper-armor`), matching the `skins/<texture_id>.png` filename",
                    skin.texture_id
                ),
            ));
        }
    }
}

crate::dw_code! {
    /// (spec-0097 §5) A body's `skin.hidden_layers` names one layer twice. The
    /// list is the set of overlay layers the mannequin does not draw; a
    /// repeat says nothing a single entry does not, and is a mistake.
    pub const SKIN_LAYER_TWICE: DwCode = DwCode::new("DW0980", ExitTier::Build);
}

/// spec-0097 §5: a skinned body's `hidden_layers` names each layer at most once
/// (`DW0980`). Walked over every body that declares a skin, whatever its class.
pub(crate) fn skin_layer_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for site in crate::body::body_skin_sites(c) {
        let mut seen = BTreeSet::new();
        for (k, layer) in site.skin.hidden_layers.iter().enumerate() {
            if !seen.insert(*layer) {
                d.push(Diagnostic::error(
                    SKIN_LAYER_TWICE,
                    site.body.stage(),
                    format!("{}/hidden_layers/{k}", site.path),
                    format!(
                        "`{}` hides `{}` twice — `hidden_layers` is the set of overlay layers \
                         the mannequin does not draw, so name each layer once",
                        site.body.id(),
                        layer.token()
                    ),
                ));
            }
        }
    }
}

/// An NPC's station resolves in its area and is a `point` (`DW0142`, `DW0871`),
/// answered by the one anchor authority, [`crate::validate::AnchorProviders`].
pub(crate) fn npc_anchor_checks(
    c: &Campaign,
    providers: &crate::validate::AnchorProviders,
    d: &mut Vec<Diagnostic>,
) {
    // NPC anchors.
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(f) = crate::validate::station_kind_diag(
            providers,
            npc.anchor.as_str(),
            crate::layout::StationKind::Point,
            "an NPC's station",
            "npcs",
            format!("/content/npcs/{i}/anchor"),
        ) {
            d.push(f);
        } else if let Some(set) = providers.for_area(npc.area.as_str())
            && !set.contains(npc.anchor.as_str())
        {
            // The one prefab remedy in this file that names the anchor back, so
            // it is built before the call rather than passed as a literal.
            let prefab_remedy = format!(
                "use an anchor the prefab exposes, or bind a prefab/pool that carries `{}`. \
                 Anchor names come from prefab metadata; do NOT invent one",
                npc.anchor
            );
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "npcs",
                format!("/content/npcs/{i}/anchor"),
                format!(
                    "npc anchor `{}` is not provided by the prefab bound to area `{}` — {}",
                    npc.anchor,
                    npc.area,
                    providers.anchor_remedy(&prefab_remedy),
                ),
            ));
        }
    }
}

/// `DW0110` over the NPC ids.
pub(crate) fn npc_id_syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        crate::ids::id_syntax!(d, npc.id, "npcs", format!("/content/npcs/{i}/id"));
    }
}

/// `DW0111` over the NPC ids.
pub(crate) fn npc_id_uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::ids::dup_check(
        c.npcs
            .content
            .npcs
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), format!("/content/npcs/{i}/id"))),
        "npcs",
        "npc",
        d,
    );
}

/// `DW0112` over what an NPC names: its area, and the NPC each persona
/// relationship is with (a same-stage reference, validated within stage 2).
pub(crate) fn npc_dangling_refs(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::ids::dangling;
    let area_ids = crate::world::declared_area_ids(c);
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(npc.area.as_str()),
            "npcs",
            format!("/content/npcs/{i}/area"),
            format!(
                "npc references unknown area `{}` — {}",
                npc.area,
                crate::placement::Placement::of(c).area_remedy(),
            ),
        );
        // Persona relationships are same-stage NPC refs (validated within stage 2).
        for (k, rel) in npc.persona.relationships.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(rel.npc.as_str()),
                "npcs",
                format!("/content/npcs/{i}/persona/relationships/{k}/npc"),
                format!(
                    "persona relationship references unknown npc `{}` — declare that npc in \
                     stage 2 or correct the reference",
                    rel.npc
                ),
            );
        }
    }
}
