//! Stage 5 — assemblies (spec-0082): a fixed thing that can be hit and hits back.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::serde_fields::is_zero3;
use crate::{AssemblyId, Facing, Mark, QuestEffect, RigId, StealthZone};

/// A fixed thing that can be hit and hits back (spec-0082): an object built of
/// display entities standing at a [`Mark`], moving through the clips of a
/// library [`rig`](crate::rig), struck in melee through an optional
/// `minecraft:interaction` hitbox, and striking a player who stands in its
/// arming region.
///
/// It is not a fight class and never dies: no health, equipment, traversal,
/// health bar or kill credit. A hit count is an ordinary `state` datum a
/// `strike-assembly` trigger adds to, and what happens at a count is an effect
/// behind the ordinary gate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Assembly {
    /// Unique assembly id (`assembly/<kebab>`).
    pub id: AssemblyId,
    /// The library rig (`rig/<name>`, resolved to `rigs/<name>/rig.json`
    /// beside the prefab library) whose parts and clips this assembly is.
    pub rig: RigId,
    /// Where the rig's origin stands: an anchor and an optional offset. The
    /// rig's origin is the mark cell's centre at its floor plane.
    pub at: Mark,
    /// Which way the rig's `+z` front faces (default `south`). Applied by the
    /// compiler to every frame, so the emitted entities stand at yaw 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<Facing>,
    /// The clip playing from spawn. Absent: the parts stand in the rig's rest
    /// pose and no clip plays until a `play-clip`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial: Option<String>,
    /// The `minecraft:interaction` a player strikes. Absent: the assembly
    /// cannot be struck, and a `strike-assembly` on it is refused (`DW0936`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hitbox: Option<AssemblyHitbox>,
    /// The blows it deals. Absent: it never strikes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strikes: Option<AssemblyStrikes>,
}

impl Assembly {
    /// The declared facing, `south` when absent.
    pub fn facing(&self) -> Facing {
        self.facing.unwrap_or(Facing::South)
    }
}

/// An assembly's hitbox (spec-0082 §3.2): a `minecraft:interaction` of
/// `width × height` whose bottom centre is the mark's cell centre plus
/// `offset`. Melee only: an arrow passes through an interaction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssemblyHitbox {
    /// Width in blocks (`0 < width <= 6`, `DW0936`).
    pub width: f64,
    /// Height in blocks (`0 < height <= 22`, `DW0936`).
    pub height: f64,
    /// Integer `[x, y, z]` block offset of the box's bottom centre from the
    /// mark (default `[0, 0, 0]`).
    #[serde(default, skip_serializing_if = "is_zero3")]
    pub offset: [i32; 3],
}

/// An assembly's strike pattern (spec-0082 §3.2).
///
/// The pattern runs, repeating from its first step, on every tick on which
/// some player's body is in `while_in`, and stops at the end of the step in
/// flight when nobody is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssemblyStrikes {
    /// The arming region: an anchor-centred box. Every landing box lies inside
    /// it (`DW0938`), so a player who never entered it is never struck.
    pub while_in: StealthZone,
    /// The steps, in order.
    pub pattern: Vec<StrikeStep>,
    /// Aim (spec-0082 §5.7). Absent: every blow lands where its `on_land`
    /// boxes say. Present: at the start of every wind-up the assembly turns to
    /// the one of its declared facings nearest the bearing of the nearest
    /// player in `while_in`, and every `damage-players` box in `on_land` turns
    /// with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aim: Option<StrikeAim>,
}

/// An aimed strike pattern's facings (spec-0082 §5.7): `facings` turns
/// evenly spaced round the vertical axis through the mark, the first being the
/// assembly's declared `facing`. Every `on_land` box is written for that first
/// facing; the compiler turns it to each of the others and proves every facing
/// a player in `while_in` can draw (`DW0938`, judged per facing). Which facing
/// a blow takes is chosen at run time among the proven ones; nothing about
/// where it lands is computed there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeAim {
    /// How many facings, evenly spaced: 4 is a quarter turn apart, 8 an eighth,
    /// 16 a sixteenth. At least 1 (1 is the declared facing alone).
    pub facings: std::num::NonZeroU32,
}

/// One step of a strike pattern: wind up, hold, strike, land.
///
/// How long the wind-up is and how hard the blow lands are the creator's
/// judgement, by spec-0016's standing ruling: no telegraph rule and no
/// one-shot rule.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeStep {
    /// The clip played first.
    pub windup: String,
    /// Ticks the windup's last frame is held before the strike clip starts.
    pub hold: u32,
    /// The clip the blow is.
    pub strike: String,
    /// The step's pace: the keyframe cadence, in ticks per frame (1–20, the
    /// rig's own bounds), its wind-up and strike clips play at. Absent: each
    /// clip's own. The wind-up lasts `1 + (frames - 1) × cadence` ticks, then
    /// `hold`; the blow lands one cadence after the strike's last frame.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticks_per_frame: Option<u32>,
    /// A lock (spec-0094): at the start of the wind-up the step picks one
    /// player in `lock.within` by `lock.pick`, reads the cell their feet stand
    /// in, turns the assembly to it and strikes it with whichever of `strike`
    /// and `lock.reaches` the compiler proved comes down there. The blow's area
    /// is then the cells that clip comes down on, derived by the compiler: a
    /// `damage-players` in a locked step declares no `in` (`DW0969`). Absent:
    /// the blow lands where its `on_land` boxes say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock: Option<StrikeLock>,
    /// Effects run, with no acting player, on the tick a client has drawn the
    /// strike clip's last frame whole (one cadence after it is applied). A step
    /// with none is a feint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_land: Vec<QuestEffect>,
}

/// A strike step's lock (spec-0094 §3.1): the step strikes where one player
/// stands, chosen when its wind-up begins.
///
/// A display entity cannot bend live to a point: its pose is a keyframe a rig
/// precomputed. So a lock is two run-time choices among things the compiler
/// proved — a **turn** of the whole assembly about its mark (any yaw: a `tp` of
/// the root turns every riding part with it), and a **pose**, the first of
/// `strike` then `reaches` whose last frame, at that turn, comes down on the
/// locked cell with its whole blow inside `while_in`. Every standable cell of
/// `within` owes such a pose, or the build refuses naming the cells (`DW0968`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StrikeLock {
    /// The region a target is chosen in: an anchor-centred box. The step winds
    /// up only while some player's body is in it, and every blow it can deal
    /// lies inside `while_in` (`DW0968`).
    pub within: StealthZone,
    /// Which player in `within` the step locks onto.
    pub pick: LockPick,
    /// Further strike clips, beyond the step's `strike`, the lock may choose
    /// among — a limb's blows at other reaches. Tried after `strike`, in the
    /// order written; the first that comes down on the locked cell is played.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reaches: Vec<String>,
}

/// Which player a locked strike chooses (spec-0094 §3.1): vanilla's own
/// selector orders, measured from the assembly's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LockPick {
    /// The player nearest the mark (`sort=nearest`).
    Nearest,
    /// The player furthest from the mark (`sort=furthest`).
    Furthest,
    /// A player chosen at random (`sort=random`).
    Random,
}

impl LockPick {
    /// The `sort=` value of the selector that makes the choice.
    pub fn sort(self) -> &'static str {
        match self {
            LockPick::Nearest => "nearest",
            LockPick::Furthest => "furthest",
            LockPick::Random => "random",
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::BTreeMap;

use crate::Verb;
use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

crate::dw_code! {
    /// (spec-0082 §5.1, §5.5) **An assembly's rig cannot be emitted as
    /// declared.** The library holds no `rigs/<name>/rig.json` for the
    /// assembly's `rig`, or the file does not parse, or it breaks a structural
    /// rule (no part, an unknown block, a clip with no frame, a frame short a
    /// part, a cadence outside `1..=20`, a non-finite transform, a zero scale);
    /// or an `initial`, a strike step's `windup`/`strike`, or a `play-clip`
    /// names a clip the rig lacks — the message lists the rig's clips.
    /// Validation-tier (exit 1). Prescription: regenerate the rig with its
    /// generator, or name a clip the rig declares (`delvec rig describe`
    /// prints them).
    pub const ASSEMBLY_RIG: DwCode = DwCode::new("DW0935", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0094 §5.2) **A locked strike's blow is declared where the lock
    /// derives it.** A locked step's blow lands on the cells its chosen
    /// clip comes down on at the locked turn, so a `damage-players` at the
    /// top of its `on_land` that declares an `in` box, any `damage-players`
    /// nested inside another effect's list there (it cannot be moved with
    /// the lock), and a locked step in a pattern that also declares `aim`
    /// (two rules choosing one turn) are refused, each naming the field.
    /// Validation-tier (exit 1). Prescription: drop the `in`, lift the
    /// `damage-players` to the top of `on_land`, or drop `aim` or `lock`.
    pub const ASSEMBLY_LOCK_SHAPE: DwCode = DwCode::new("DW0969", ExitTier::Build);
}

crate::dw_code! {
    /// (spec-0094 §3.3) **An `arm-strikes` names an assembly that never
    /// strikes.** The assembly declares no `strikes`, so there is no pattern
    /// to re-arm and the beat does nothing. Validation-tier (exit 1).
    /// Prescription: give the assembly a `strikes` pattern, or drop the
    /// effect.
    pub const ASSEMBLY_ARM_NOTHING: DwCode = DwCode::new("DW0970", ExitTier::Build);
}

/// spec-0082: **assemblies, their rigs, and every reference to one.**
///
/// * Each assembly's `rig` resolves in the library and passes the rig's
///   structural rules ([`crate::rig::check`]); its `initial` and every strike
///   step's `windup`/`strike` name clips the rig declares (`DW0935`). A
///   registry that is not the whole library answers
///   [`crate::rig::RigLookup::Unknown`] and nothing is refused on its word.
/// * Its mark's anchor, and its arming region's, are provided by some area
///   (`DW0142`), and the mark is a point station.
/// * Every `spawn-assembly` / `despawn-assembly` / `play-clip`, at every depth
///   of every effect root, names a declared assembly (`DW0112`), and a
///   `play-clip` names a clip its rig declares (`DW0935`).
/// * Every `strike-assembly` trigger names a declared assembly (`DW0112`).
///
/// The hitbox's bounds, its reach and where a blow lands are judged at build
/// time, where cells exist (`DW0936`–`DW0938`, `compiler::assembly`).
pub(crate) fn assembly_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    use crate::rig::RigLookup;
    let quests = &c.quests.content;
    if quests.assemblies.is_empty()
        && !quests
            .triggers
            .iter()
            .any(|t| t.on.assembly_target().is_some())
    {
        // Still walk the effects: a verb naming an assembly in a campaign that
        // declares none is a dangling reference.
        let mut any = false;
        crate::for_each_campaign_effect(c, &mut |_, _, e| {
            any |= assembly_verb(e).is_some();
        });
        if !any {
            return;
        }
    }
    let providers = AnchorProviders::build(c, anchors);
    // The rig each declared assembly resolved to, for the clip checks below.
    let mut rigs: BTreeMap<&str, Option<&crate::rig::Rig>> = BTreeMap::new();
    let clip_list = |r: &crate::rig::Rig| -> String {
        let names = r.clip_names();
        if names.is_empty() {
            "none".to_string()
        } else {
            names
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    for (i, a) in quests.assemblies.iter().enumerate() {
        let at = format!("/content/assemblies/{i}");
        let resolved = match anchors.rig(&a.rig) {
            RigLookup::Unknown => None,
            RigLookup::Missing => {
                d.push(Diagnostic::error(
                    ASSEMBLY_RIG,
                    "quests",
                    format!("{at}/rig"),
                    format!(
                        "assembly `{}` names rig `{}`, and the library holds no `{}/{}/{}` — a \
                         rig is a file a generator writes beside the prefab library, never \
                         campaign JSON. Run the generator that writes it, or name a rig the \
                         library holds",
                        a.id,
                        a.rig,
                        crate::rig::RIGS_DIR,
                        crate::l10n::local_id(a.rig.as_str()),
                        crate::rig::RIG_FILE,
                    ),
                ));
                None
            }
            RigLookup::Malformed(e) => {
                d.push(Diagnostic::error(
                    ASSEMBLY_RIG,
                    "quests",
                    format!("{at}/rig"),
                    format!(
                        "assembly `{}` names rig `{}`, whose `{}` does not parse as a rig \
                         document: {e}. Regenerate it with the generator that wrote it",
                        a.id,
                        a.rig,
                        crate::rig::RIG_FILE,
                    ),
                ));
                None
            }
            RigLookup::Found(r) => {
                let issues = crate::rig::check(r);
                for issue in &issues {
                    d.push(Diagnostic::error(
                        ASSEMBLY_RIG,
                        "quests",
                        format!("{at}/rig"),
                        format!(
                            "assembly `{}` names rig `{}`, which breaks a rig rule at `{}`: {}. \
                             Regenerate the rig with its generator",
                            a.id, a.rig, issue.field, issue.message
                        ),
                    ));
                }
                if issues.is_empty() { Some(r) } else { None }
            }
        };
        rigs.insert(a.id.as_str(), resolved);
        if let Some(r) = resolved {
            let mut need = |clip: &str, path: String, role: &str| {
                if r.clips.contains_key(clip) {
                    return;
                }
                d.push(Diagnostic::error(
                    ASSEMBLY_RIG,
                    "quests",
                    path,
                    format!(
                        "assembly `{}` asks for clip `{clip}` as its {role}, and rig `{}` declares \
                         no such clip. Its clips are: {}",
                        a.id,
                        a.rig,
                        clip_list(r)
                    ),
                ));
            };
            if let Some(initial) = &a.initial {
                need(initial, format!("{at}/initial"), "`initial`");
            }
            let mut paced: Vec<Diagnostic> = Vec::new();
            if let Some(s) = &a.strikes {
                for (j, step) in s.pattern.iter().enumerate() {
                    need(
                        &step.windup,
                        format!("{at}/strikes/pattern/{j}/windup"),
                        "strike step's `windup`",
                    );
                    need(
                        &step.strike,
                        format!("{at}/strikes/pattern/{j}/strike"),
                        "strike step's `strike`",
                    );
                    if let Some(lock) = &step.lock {
                        for (k, reach) in lock.reaches.iter().enumerate() {
                            need(
                                reach,
                                format!("{at}/strikes/pattern/{j}/lock/reaches/{k}"),
                                "locked strike step's `reaches`",
                            );
                        }
                    }
                    if let Some(t) = step.ticks_per_frame
                        && !(crate::rig::MIN_TICKS_PER_FRAME..=crate::rig::MAX_TICKS_PER_FRAME)
                            .contains(&t)
                    {
                        paced.push(Diagnostic::error(
                            ASSEMBLY_RIG,
                            "quests",
                            format!("{at}/strikes/pattern/{j}/ticks_per_frame"),
                            format!(
                                "assembly `{}`'s strike step {j} plays its clips at {t} tick(s) per \
                                 frame. A keyframe cadence is {} to {} — the bounds every rig clip \
                                 is held to. Choose a cadence in that range, or drop \
                                 `ticks_per_frame` to play each clip at its own",
                                a.id,
                                crate::rig::MIN_TICKS_PER_FRAME,
                                crate::rig::MAX_TICKS_PER_FRAME
                            ),
                        ));
                    }
                }
            }
            d.extend(paced);
        }
        if let Some(f) = station_kind_diag(
            &providers,
            a.at.anchor.as_str(),
            crate::layout::StationKind::Point,
            "an assembly's mark",
            "quests",
            format!("{at}/at/anchor"),
        ) {
            d.push(f);
        } else if !providers.resolvable(a.at.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("{at}/at/anchor"),
                format!(
                    "assembly `{}` stands at anchor `{}`, which no area's prefab provides — {}",
                    a.id,
                    a.at.anchor,
                    providers.anchor_remedy(
                        "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                    ),
                ),
            ));
        }
        if let Some(s) = &a.strikes {
            lock_shape_checks(a, i, s, d);
            for (j, step) in s.pattern.iter().enumerate() {
                let Some(lock) = &step.lock else { continue };
                if providers.resolvable(lock.within.anchor.as_str()) {
                    continue;
                }
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("{at}/strikes/pattern/{j}/lock/within/anchor"),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player in a region centred \
                         on anchor `{}`, which no area's prefab provides — {}",
                        a.id,
                        lock.within.anchor,
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                        ),
                    ),
                ));
            }
        }
        if let Some(s) = &a.strikes
            && !providers.resolvable(s.while_in.anchor.as_str())
        {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("{at}/strikes/while_in/anchor"),
                format!(
                    "assembly `{}`'s arming region is centred on anchor `{}`, which no area's \
                     prefab provides — {}",
                    a.id,
                    s.while_in.anchor,
                    providers.anchor_remedy(
                        "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                    ),
                ),
            ));
        }
    }
    // Which declared assemblies strike, for `arm-strikes` (`DW0970`).
    let strikes: BTreeMap<&str, bool> = quests
        .assemblies
        .iter()
        .map(|a| (a.id.as_str(), a.strikes.is_some()))
        .collect();
    // Every verb that names an assembly, at every depth of every root.
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        let Some((assembly, clip)) = assembly_verb(e) else {
            return;
        };
        if matches!(e.verb, Verb::ArmStrikes { .. }) && strikes.get(assembly) == Some(&false) {
            d.push(Diagnostic::error(
                ASSEMBLY_ARM_NOTHING,
                "quests",
                format!("{path}/assembly"),
                format!(
                    "`arm-strikes` re-arms assembly `{assembly}`'s strike pattern, and the \
                     assembly declares no `strikes` — there is no pattern to re-arm, so the beat \
                     does nothing. Give the assembly a `strikes` pattern, or drop the effect"
                ),
            ));
        }
        let Some(resolved) = rigs.get(assembly) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names assembly `{assembly}`, which the stage-5 `assemblies` list does \
                     not declare — declare it, or fix the reference",
                    e.verb.tag()
                ),
            ));
            return;
        };
        if let (Some(clip), Some(r)) = (clip, resolved)
            && !r.clips.contains_key(clip)
        {
            d.push(Diagnostic::error(
                ASSEMBLY_RIG,
                "quests",
                format!("{path}/clip"),
                format!(
                    "`play-clip` asks assembly `{assembly}` for clip `{clip}`, and its rig \
                     declares no such clip. Its clips are: {}",
                    clip_list(r)
                ),
            ));
        }
    });
    for (i, t) in quests.triggers.iter().enumerate() {
        if let Some(m) = t.on.assembly_target()
            && !rigs.contains_key(m.as_str())
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("/content/triggers/{i}/on/assembly"),
                format!(
                    "`strike-assembly` trigger `{}` targets assembly `{m}`, which the stage-5 \
                     `assemblies` list does not declare — use a declared assembly id",
                    t.id
                ),
            ));
        }
    }
}

/// The assembly a verb names, with the clip a `play-clip` asks for.
fn assembly_verb(e: &QuestEffect) -> Option<(&str, Option<&str>)> {
    match &e.verb {
        Verb::SpawnAssembly { assembly } | Verb::DespawnAssembly { assembly } => {
            Some((assembly.as_str(), None))
        }
        Verb::PlayClip { assembly, clip } => Some((assembly.as_str(), Some(clip.as_str()))),
        Verb::ArmStrikes { assembly } => Some((assembly.as_str(), None)),
        _ => None,
    }
}

/// spec-0094 §5.2 (`DW0969`): **a locked step's blow is the lock's to place.**
/// A locked step lands on the cells its chosen clip comes down on at the turn
/// it locked to, so a box an author writes cannot be where the blow lands: a
/// top-level `damage-players` in its `on_land` declares no `in`, no
/// `damage-players` stands inside another effect's list there, and a pattern
/// that turns by `aim` has no locked step.
fn lock_shape_checks(
    a: &crate::Assembly,
    i: usize,
    s: &crate::AssemblyStrikes,
    d: &mut Vec<Diagnostic>,
) {
    fn nested_damage(effs: &[QuestEffect], path: &str, out: &mut Vec<String>) {
        for (k, e) in effs.iter().enumerate() {
            let here = format!("{path}/{k}");
            if matches!(e.verb, Verb::DamagePlayers { .. }) {
                out.push(here.clone());
            }
            for (seg, _, list) in e.nested_effect_lists_labeled() {
                nested_damage(list, &format!("{here}/{seg}"), out);
            }
        }
    }
    let at = format!("/content/assemblies/{i}/strikes");
    for (j, step) in s.pattern.iter().enumerate() {
        if step.lock.is_none() {
            continue;
        }
        let here = format!("{at}/pattern/{j}");
        if s.aim.is_some() {
            d.push(Diagnostic::error(
                ASSEMBLY_LOCK_SHAPE,
                "quests",
                format!("{here}/lock"),
                format!(
                    "assembly `{}`'s strike pattern turns by `aim`, and its step {j} declares a \
                     `lock` — two rules choosing the one turn the assembly strikes from. A locked \
                     step turns to the cell it locks onto; an aimed pattern turns to one of its \
                     facings. Drop `aim` from the pattern, or drop `lock` from the step",
                    a.id
                ),
            ));
        }
        for (k, e) in step.on_land.iter().enumerate() {
            let p = format!("{here}/on_land/{k}");
            if matches!(e.verb, Verb::DamagePlayers { .. }) && e.damage_within().is_some() {
                d.push(Diagnostic::error(
                    ASSEMBLY_LOCK_SHAPE,
                    "quests",
                    format!("{p}/in"),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player, and its \
                         `damage-players` ({p}) declares an `in` box. A locked blow lands on the \
                         cells its clip comes down on at the turn it locked to — the compiler \
                         derives that area for every cell it can lock, so a written box is a \
                         second, fixed answer that is wrong at every other cell. Drop the `in`",
                        a.id
                    ),
                ));
            }
            let mut deep = Vec::new();
            for (seg, _, list) in e.nested_effect_lists_labeled() {
                nested_damage(list, &format!("{p}/{seg}"), &mut deep);
            }
            for q in deep {
                d.push(Diagnostic::error(
                    ASSEMBLY_LOCK_SHAPE,
                    "quests",
                    q.clone(),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player, and a \
                         `damage-players` ({q}) stands inside another effect's list. Only a blow \
                         at the top of a locked step's `on_land` is moved to the cells the clip \
                         comes down on; this one would land nowhere the lock chose. Move it to the \
                         top of `on_land` (a `when` on it is kept)",
                        a.id
                    ),
                ));
            }
        }
    }
}

/// `DW0110` over each assembly's id and the rig it names.
pub(crate) fn assembly_id_syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.quests.content.assemblies.iter().enumerate() {
        crate::ids::id_syntax!(d, a.id, "quests", format!("/content/assemblies/{i}/id"));
        crate::ids::id_syntax!(d, a.rig, "quests", format!("/content/assemblies/{i}/rig"));
    }
}

/// `DW0111` over the assembly ids: unique within the stage-5 assemblies
/// namespace (spec-0082).
pub(crate) fn assembly_id_uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::ids::dup_check(
        c.quests
            .content
            .assemblies
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/assemblies/{i}/id"))),
        "quests",
        "assembly",
        d,
    );
}
