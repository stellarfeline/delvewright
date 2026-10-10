//! Body traversal — the declaration every body that moves carries (DSL v0.11,
//! spec-0034), and the one enumeration of the campaign's bodies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Actor, AreaId, ClassId, Mark, Npc, NpcSkin};

#[cfg(doc)]
use crate::WaveMob;

/// How a body gets around.
///
/// The compiler DERIVES this from the entity id for every body (spiders climb,
/// ghasts fly, `#minecraft:aquatic` swims, and everything else — including every
/// id the table has never heard of — is [`Locomotion::Ground`], the checked
/// class). [`BodyTraversal`] is the author's side of the same vocabulary: one
/// enum, so a declaration and a derivation can never mean different things.
///
/// The vocabulary lives in this crate rather than in the compiler because it is
/// now DSL surface; the compiler re-exports it and owns the derivation table
/// (`compiler::traversal`).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Locomotion {
    /// Walks, steps and jumps — the default and the CHECKED class.
    Ground,
    /// Climbs sheer vertical surfaces (vanilla's `Spider` class).
    Climber,
    /// Leaves the ground under its own power.
    Flier,
    /// A member of vanilla's `#minecraft:aquatic` tag. A ledger classification
    /// that exempts nothing, which is why it may not be **declared**
    /// (`DW0455`) — see [`BodyTraversal`].
    Aquatic,
}

impl Locomotion {
    /// The stable kebab token this class is written and reported under.
    pub fn token(self) -> &'static str {
        match self {
            Locomotion::Ground => "ground",
            Locomotion::Climber => "climber",
            Locomotion::Flier => "flier",
            Locomotion::Aquatic => "aquatic",
        }
    }

    /// Every class, in ledger order — so a report can never silently drop a row
    /// when a class is added.
    pub const ALL: [Locomotion; 4] = [
        Locomotion::Ground,
        Locomotion::Climber,
        Locomotion::Flier,
        Locomotion::Aquatic,
    ];
}

/// What a body can do when it moves, **declared by the author** (DSL v0.11,
/// spec-0034).
///
/// Carried by every object class in the DSL that has a body and a position and
/// is walked by a compiler-emitted route — the stage-2 [`Npc`] and the stage-5
/// [`Actor`]. It is deliberately one shared type on both rather than a field
/// per consumer: traversal is a property of a body that moves, not of the verb
/// that first needed it (CLAUDE.md), and a second bespoke field would be the
/// defect rather than the fix.
///
/// **A declaration is a claim the build holds you to, never an opt-out.** The
/// compiler compares the verdicts this body earns under the declared class
/// against the ones it earns under its species' derived class; a declaration
/// that changes no verdict is inert and is `DW0454`. So declaring `climber` on
/// a sheep is only accepted where that sheep's route really does go over a
/// barrier line — the exception is authored and proven, instead of happening by
/// accident and merely rendering.
///
/// **What is deliberately NOT here: `opens_gates`.** Passing a closed fence gate
/// is a right-click, a scripted walk is a compiler-emitted `tp` polyline whose
/// puppet performs no interaction at all, and no runtime verb changes a fence
/// gate's block state. Declaring it would not make it true, so the error tier
/// (`DW0452`) has no authorable exemption and a declaration can never reach it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyTraversal {
    /// How this body gets around, overriding what its entity id implies.
    pub locomotion: Locomotion,
}

/// **A still body that turns to face a player** (spec-0101) — the declaration
/// both body classes carry, as one type, because facing belongs to the body and
/// not to the verb that first wanted it turned.
///
/// The run-time turn is `rotate <body> facing entity <player> eyes`, re-issued
/// from the root `tick` by one generated function (`watch_tick`): every tick a
/// player [`Self::who`] names stands within [`Self::within`] blocks of the
/// body's feet, the body faces that player's eyes with its own, body and head
/// together; with nobody in reach it keeps its last facing; while a `move-npc` /
/// `move-actor` walks it, the walk owns the yaw and the watch resumes on the tick
/// after the arrival.
///
/// Both fields are required: whom the figure watches and how far it sees are the
/// creator's, so neither has a default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BodyWatch {
    /// Whom the body faces: `"nearest"` (the nearest player), or
    /// `{ "class": "class/<id>" }` (the nearest player wearing that class).
    pub who: WatchWho,
    /// How far the body sees, in whole blocks `>= 1`, measured as the game
    /// measures an entity selector's `distance`: from the body's feet to the
    /// player's.
    pub within: std::num::NonZeroU32,
}

/// Whom a watching body faces ([`BodyWatch::who`]): the bare keyword
/// `"nearest"`, or a class filter. Untagged, and each variant's payload is its
/// own type, so a mistyped key fails the schema instead of matching the wrong
/// arm (see `CameraSubject`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum WatchWho {
    /// `"nearest"`: the nearest player, whoever they are.
    Nearest(WatchNearest),
    /// `{ "class": "class/<id>" }`: the nearest player who took that class.
    Class(WatchClass),
}

impl WatchWho {
    /// The class this watch is filtered to, if any.
    pub fn class(&self) -> Option<&ClassId> {
        match self {
            WatchWho::Nearest(_) => None,
            WatchWho::Class(c) => Some(&c.class),
        }
    }

    /// The stable token a report writes this under: `nearest`, or the class id.
    pub fn token(&self) -> String {
        match self {
            WatchWho::Nearest(_) => "nearest".to_string(),
            WatchWho::Class(c) => c.class.as_str().to_string(),
        }
    }
}

/// The keyword form of [`WatchWho`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WatchNearest {
    /// The nearest player.
    Nearest,
}

/// The class-filter form of [`WatchWho`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WatchClass {
    /// The class (stage-3 ref) whose players this body watches.
    pub class: ClassId,
}

/// One object class that has a **body the compiler stages**: a declared
/// position, a declared species, and a compiler-emitted route.
///
/// A sum type rather than a flattened tuple, so **adding a body class is a
/// compile error at every consumer** until each one says what it does with it.
/// The alternative — each rule walking the classes it happens to remember — is
/// the defect class CLAUDE.md names: a hand-rolled walk that
/// enumerated three of five effect roots.
///
/// **Keyed to the object, not to the verb that first needed it.** This type was
/// introduced for [`BodyTraversal`] and read, for a while, as "one consumer of
/// `BodyTraversal`" — which is why its only enumeration
/// ([`body_traversal_sites`]) was filtered to bodies declaring a traversal, and
/// why the second property a body carries, its [`NpcSkin`], had no enumeration
/// at all: the bake walked the stage-2 npc list by hand, and an actor's skin was
/// emitted into the summon, never baked into the pack, and never refused.
/// A body's properties belong to the body. [`body_sites`] is the unfiltered
/// walk; a per-property enumeration is a *filter* over it, never a second walk.
///
/// Deliberately NOT a member: [`WaveMob`]. A wave mob has a body and a position,
/// but it is driven by **native vanilla AI**, never by a compiler-emitted route,
/// so the compiler makes no claim about the moves it makes and a locomotion
/// declaration on it could change no verdict. It declares no `skin` either — the
/// exclusion holds for both properties, and the schema says so
/// (`crates/dsl/tests/body_skin_sites.rs`). It becomes a member the day the lane
/// proof reasons about how its bodies move — and it joins here, through this
/// same type, rather than through a field of its own.
#[derive(Clone, Copy, Debug)]
pub enum BodyRef<'a> {
    /// A stage-2 NPC, walked by `move-npc`.
    Npc(&'a Npc),
    /// A stage-5 scripted actor, walked by `move-actor`.
    Actor(&'a Actor),
}

impl<'a> BodyRef<'a> {
    /// The declaring stage's wire name (`npcs` / `quests`) — also the stage
    /// whose `dsl_version` fences this body's declaration.
    pub fn stage(self) -> &'static str {
        match self {
            BodyRef::Npc(_) => "npcs",
            BodyRef::Actor(_) => "quests",
        }
    }

    /// The body's declared id.
    pub fn id(self) -> &'a str {
        match self {
            BodyRef::Npc(n) => n.id.as_str(),
            BodyRef::Actor(a) => a.id.as_str(),
        }
    }

    /// The entity id written on the body. **Not necessarily the body that
    /// ships**: a `skin` re-dresses it as a `minecraft:mannequin` — see
    /// [`Self::worn_entity`].
    pub fn declared_entity(self) -> &'a str {
        match self {
            BodyRef::Npc(n) => n.base_entity.as_str(),
            BodyRef::Actor(a) => a.entity.as_str(),
        }
    }

    /// **The entity id the body ships as**: `minecraft:mannequin` when it
    /// declares a `skin`, else the declared entity. The one authority for that
    /// rule — the compiler's geometric proofs (`nav::npc_body_entity`,
    /// `nav::actor_body_entity`) and the equipment fit rule (`DW0898`) all read
    /// it.
    pub fn worn_entity(self) -> &'a str {
        match self.skin() {
            Some(_) => "minecraft:mannequin",
            None => self.declared_entity(),
        }
    }

    /// **The mark this body is placed on** — the anchor and offset the engine
    /// summons it at, for every class alike (spec-0066).
    ///
    /// A body's placement is a property of the body, not of the stage list that
    /// happens to declare it: a mark is a cell, and a cell holds one body. The
    /// rule that reads this ([`crate::compiler`]'s `DW0896`, via
    /// [`body_sites`]) therefore quantifies over npcs and actors in one pass
    /// rather than over `actors[]`, which is where the seven-men-one-anchor
    /// muster came from.
    pub fn mark(self) -> Mark {
        let (anchor, offset) = match self {
            BodyRef::Npc(n) => (&n.anchor, n.offset),
            BodyRef::Actor(a) => (&a.anchor, a.offset),
        };
        Mark {
            anchor: anchor.clone(),
            offset,
        }
    }

    /// The area whose anchor table resolves [`Self::mark`]'s anchor first, when this
    /// class declares one.
    ///
    /// A stage-2 npc names its area and is resolved inside it; a stage-5 actor
    /// names none and is resolved across every placed piece, exactly as an
    /// `open-gate` / `move-actor` destination is. Stated here so the resolution
    /// rule is one rule over both classes and not a per-call-site habit.
    pub fn area(self) -> Option<&'a AreaId> {
        match self {
            BodyRef::Npc(n) => Some(&n.area),
            BodyRef::Actor(_) => None,
        }
    }

    /// Whether this body stands on its mark from **world init**, with no effect
    /// having to fire.
    ///
    /// A stage-2 npc does unless it is `deferred`; a stage-5 actor never does —
    /// a puppet exists only from the `spawn-actor` that summons it, which is why
    /// an actor no `spawn-actor` names never exists at all.
    pub fn at_world_init(self) -> bool {
        match self {
            BodyRef::Npc(n) => !n.deferred,
            BodyRef::Actor(_) => false,
        }
    }

    /// Whether a **player** can end this body's life.
    ///
    /// An npc body is emitted `Invulnerable:1b` unconditionally, so nothing a
    /// player does removes it; an actor's puppet is `Invulnerable` unless it
    /// declares [`Actor::vulnerable`]. A body a player can kill is one whose
    /// lifetime the compiler cannot bound, which is the whole of what this
    /// answers.
    pub fn killable_by_players(self) -> bool {
        match self {
            BodyRef::Npc(_) => false,
            BodyRef::Actor(a) => a.vulnerable,
        }
    }

    /// This body's traversal declaration, if it carries one.
    pub fn traversal(self) -> Option<&'a BodyTraversal> {
        match self {
            BodyRef::Npc(n) => n.traversal.as_ref(),
            BodyRef::Actor(a) => a.traversal.as_ref(),
        }
    }

    /// This body's skin declaration, if it carries one.
    ///
    /// A skinned body of **either** class ships as a `minecraft:mannequin`
    /// whose `profile.texture` resolves to `delvewright:npc/<campaign_id>/<texture_id>`,
    /// so either one owes the same `skins/<texture_id>.png` under the same refusal
    /// (`DW0309`). Answering it here is what stops the bake from being a
    /// property of one class.
    pub fn skin(self) -> Option<&'a NpcSkin> {
        match self {
            BodyRef::Npc(n) => n.skin.as_ref(),
            BodyRef::Actor(a) => a.skin.as_ref(),
        }
    }

    /// This body's watch declaration (spec-0101), if it carries one.
    pub fn watch(self) -> Option<&'a BodyWatch> {
        match self {
            BodyRef::Npc(n) => n.watch.as_ref(),
            BodyRef::Actor(a) => a.watch.as_ref(),
        }
    }

    /// This class's name in the JSON Schema export (`delvec schema --stage all`).
    ///
    /// The join between the closed Rust set and the schema, which is the only
    /// authority on *which object classes declare what*. `body_skin_sites.rs`
    /// compares the two.
    pub fn class(self) -> &'static str {
        match self {
            BodyRef::Npc(_) => "Npc",
            BodyRef::Actor(_) => "Actor",
        }
    }

    /// Every body class, by schema name. The closed set, stated once.
    pub const ALL_CLASSES: [&'static str; 2] = ["Npc", "Actor"];
}

/// A staged body declaration, with the JSON pointer at the declaration itself.
///
/// The pointer is at the OBJECT (`/content/npcs/3`), not at any one of its
/// fields: a per-property site appends its own field name. A pointer built per
/// property is how two walks of one population start disagreeing about where a
/// thing was declared.
#[derive(Clone, Debug)]
pub struct BodySite<'a> {
    /// Which object class declared it, and the object itself.
    pub body: BodyRef<'a>,
    /// JSON pointer at the declaration, for a diagnostic path.
    pub path: String,
}

/// **Every** body the campaign declares, in stage order: stage-2 npcs in
/// declaration order, then stage-5 actors in declaration order.
///
/// The one walk of the campaign's bodies. A rule about a property a body carries
/// is a *filter* over this ([`body_traversal_sites`], [`body_skin_sites`]) — never
/// a second traversal, and never a hand-written loop over one stage's list,
/// which is exactly how an actor's skin came to be emitted but never baked.
pub fn body_sites(c: &crate::envelope::Campaign) -> Vec<BodySite<'_>> {
    let mut out: Vec<BodySite<'_>> = Vec::new();
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        out.push(BodySite {
            body: BodyRef::Npc(n),
            path: format!("/content/npcs/{i}"),
        });
    }
    for (i, a) in c.quests.content.actors.iter().enumerate() {
        out.push(BodySite {
            body: BodyRef::Actor(a),
            path: format!("/content/actors/{i}"),
        });
    }
    out
}

/// A body that carries a [`BodyTraversal`] declaration, with the JSON pointer at
/// it.
#[derive(Clone, Debug)]
pub struct BodyTraversalSite<'a> {
    /// Which object class declared it, and the object itself.
    pub body: BodyRef<'a>,
    /// JSON pointer at the `traversal` field, for a diagnostic path.
    pub path: String,
    /// The declaration.
    pub traversal: &'a BodyTraversal,
}

/// Every body in the campaign that DECLARES a traversal, in stage order.
///
/// The one enumeration of the declaration's consumers, shared by the DSL's value
/// check (`DW0455`) and by the compiler's proof (`DW0454`), so "which object
/// classes carry this" is answered in exactly one place.
pub fn body_traversal_sites(c: &crate::envelope::Campaign) -> Vec<BodyTraversalSite<'_>> {
    body_sites(c)
        .into_iter()
        .filter_map(|s| {
            s.body.traversal().map(|t| BodyTraversalSite {
                body: s.body,
                path: format!("{}/traversal", s.path),
                traversal: t,
            })
        })
        .collect()
}

/// A body that carries an [`NpcSkin`] declaration, with the JSON pointer at it.
#[derive(Clone, Debug)]
pub struct BodySkinSite<'a> {
    /// Which object class declared it, and the object itself.
    pub body: BodyRef<'a>,
    /// JSON pointer at the `skin` field, for a diagnostic path.
    pub path: String,
    /// The declaration.
    pub skin: &'a NpcSkin,
}

/// Every body in the campaign that DECLARES a skin, in stage order.
///
/// The one enumeration of what the resource-pack bake must serve and what
/// `DW0309` must refuse, for every class alike — a skinned npc and a skinned
/// actor are the same fact about two bodies.
///
/// **Declarations, not textures.** Two bodies may name one `texture_id` (an npc
/// and the puppet that plays it), and the caller decides what that means; the
/// bake reads each file once.
pub fn body_skin_sites(c: &crate::envelope::Campaign) -> Vec<BodySkinSite<'_>> {
    body_sites(c)
        .into_iter()
        .filter_map(|s| {
            s.body.skin().map(|k| BodySkinSite {
                body: s.body,
                path: format!("{}/skin", s.path),
                skin: k,
            })
        })
        .collect()
}

/// A body that carries a [`BodyWatch`] declaration, with the JSON pointer at it.
#[derive(Clone, Debug)]
pub struct BodyWatchSite<'a> {
    /// Which object class declared it, and the object itself.
    pub body: BodyRef<'a>,
    /// JSON pointer at the `watch` field, for a diagnostic path.
    pub path: String,
    /// The declaration.
    pub watch: &'a BodyWatch,
}

/// Every body in the campaign that DECLARES a watch, in stage order (spec-0101).
///
/// The one enumeration of the watchers, shared by the class refusal
/// (`DW0996`), the drawability proof (`DW0997`), the emitter (`watch_tick`, the
/// summon tag, the walk-driver yield), the generated PackTests and the bot's
/// record — a filter over [`body_sites`], so a third body class is a compile
/// error at every consumer until it says what it does with a watch.
pub fn body_watch_sites(c: &crate::envelope::Campaign) -> Vec<BodyWatchSite<'_>> {
    body_sites(c)
        .into_iter()
        .filter_map(|s| {
            s.body.watch().map(|w| BodyWatchSite {
                body: s.body,
                path: format!("{}/watch", s.path),
                watch: w,
            })
        })
        .collect()
}

/// The **mutable mirror** of [`body_skin_sites`]: every skin declaration in the
/// campaign, in the identical order, exposed mutably so one pass can rewrite what
/// every emitter will read ([`crate::l10n::namespace_skin_textures`]).
///
/// It carries no [`BodyRef`] and no pointer, because a rewrite needs neither and a
/// borrow of the whole body would forbid the field it is there to change. What it
/// does owe is the **same population**: a body class that declares a skin and is
/// missing here would keep an un-namespaced texture and collide with every other
/// delve, silently. `body_skin_sites_mut_is_the_same_walk`
/// (`crates/dsl/tests/body_skin_sites.rs`) pins that over a campaign carrying one
/// body of every class in [`BodyRef::ALL_CLASSES`] — the closed set the schema
/// export is compared against in the same file, so a new body class turns that
/// coverage red and both walks are visited together.
pub fn body_skins_mut(c: &mut crate::envelope::Campaign) -> Vec<&mut NpcSkin> {
    let mut out: Vec<&mut NpcSkin> = Vec::new();
    out.extend(
        c.npcs
            .content
            .npcs
            .iter_mut()
            .filter_map(|n| n.skin.as_mut()),
    );
    out.extend(
        c.quests
            .content
            .actors
            .iter_mut()
            .filter_map(|a| a.skin.as_mut()),
    );
    out
}

// ---------------------------------------------------------------------------
// Validation — the checks `dsl::validate` runs over this object (ADR-0031)
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;

crate::dw_code! {
    /// (v0.11, spec-0034) **A declared locomotion the engine cannot hold the
    /// body to** — today exactly one value, `aquatic`.
    ///
    /// The declaration surface exists so an author can claim a capability and
    /// have the claim PROVEN. `aquatic` is the one
    /// class that carries no exemption and governs no rule: it is a ledger
    /// label the compiler derives from vanilla's own `#minecraft:aquatic` tag.
    /// Declaring it could therefore never change a verdict, so it would always
    /// land in `DW0454` — and a value whose only possible outcome is another
    /// diagnostic is a trap, not a surface.
    ///
    /// The gap it names, stated rather than left to folklore (CLAUDE.md's
    /// no-hack rule): the compiler routes **every** body on standable ground,
    /// and `flooded` cells are impassable and never floor for every body. There
    /// is no water-traversal model for a declaration to feed, so there is
    /// nothing to hold an aquatic claim to. When routing grows one, this
    /// refusal is what has to be deleted to enable the value.
    ///
    /// Error tier, raised in `validate_campaign_with`, so the run ends at the
    /// validation tier (exit 1). Prescription: remove the declaration — a body whose
    /// route crosses water is governed by the flooded-cell rules already, and
    /// the derived aquatic class still reaches the binding ledger.
    pub const TRAVERSAL_UNPROVABLE: DwCode = DwCode::new("DW0455", ExitTier::Build);
}

/// DSL v0.11 (spec-0034): a declared locomotion the engine cannot hold the body
/// to is refused at declaration time (`DW0455`).
///
/// Today that is exactly `aquatic`, and the reason is structural rather than a
/// taste call: `aquatic` carries no exemption and governs no rule, so declaring
/// it could never change a verdict — it would land in `DW0454` every time. A
/// value whose only outcome is another diagnostic is a trap, so it is refused
/// here with the gap named.
pub(crate) fn body_traversal_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for site in body_traversal_sites(c) {
        if site.traversal.locomotion != Locomotion::Aquatic {
            continue;
        }
        let (stage, path, id) = (site.body.stage(), &site.path, site.body.id());
        d.push(Diagnostic::error(
            TRAVERSAL_UNPROVABLE,
            stage,
            format!("{path}/locomotion"),
            format!(
                "`{id}` declares `locomotion: aquatic`, which the compiler cannot hold it to. \
                 `aquatic` is the one class that carries no exemption and governs no rule — it is \
                 a ledger label derived from vanilla's own `#minecraft:aquatic` tag — so the \
                 declaration could never change a verdict and would be reported inert (`DW0454`). \
                 The gap, stated rather than left to folklore: routing has ONE reachability model, \
                 standable ground, and water-flooded cells are impassable and never floor for \
                 EVERY body, so there is nothing for an aquatic claim to feed. Prescription: \
                 remove the declaration — a route that crosses water is already governed by the \
                 flooded-cell rules, and a body vanilla itself calls aquatic still reaches the \
                 traversal proof's binding ledger under its derived class."
            ),
        ));
    }
}

crate::dw_code! {
    /// (spec-0101) **A watch for a class nobody plays**: a body's
    /// `watch.who.class` names a class stage 3 does not declare.
    ///
    /// The watch line filters its player by the class tag the class apply puts
    /// on whoever takes it (`dw_class_<c>`). A class stage 3 never declares is a
    /// class nobody can take, so no player can ever wear its tag and the body
    /// could never turn. Judged over the whole campaign — a stage-2 body may
    /// name a class the creator writes at stage 3, exactly as `DW0197` judges a
    /// stage-2 body against stage 5. Prescription: name a declared class, or
    /// watch `"nearest"`.
    pub const WATCH_CLASS_UNDECLARED: DwCode = DwCode::new("DW0996", ExitTier::Build);
}

/// spec-0101: a watch that names a class stage 3 does not declare is refused
/// (`DW0996`), naming the body and the class. Empty for a campaign whose bodies
/// declare no class watch.
pub(crate) fn body_watch_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let declared: std::collections::BTreeSet<&str> = c
        .classes
        .content
        .classes
        .iter()
        .map(|k| k.id.as_str())
        .collect();
    for site in body_watch_sites(c) {
        let Some(class) = site.watch.who.class() else {
            continue;
        };
        if declared.contains(class.as_str()) {
            continue;
        }
        let (stage, path, id) = (site.body.stage(), &site.path, site.body.id());
        let known = if declared.is_empty() {
            "none".to_string()
        } else {
            format!(
                "`{}`",
                declared.iter().copied().collect::<Vec<_>>().join("`, `")
            )
        };
        d.push(Diagnostic::error(
            WATCH_CLASS_UNDECLARED,
            stage,
            format!("{path}/who/class"),
            format!(
                "`{id}` watches the nearest player of `{class}`, a class stage 3 does not \
                 declare — nobody can take it, so no player ever wears its tag and the body \
                 could never turn. The declared classes: {known}. Name one of them, or \
                 watch `\"nearest\"`."
            ),
        ));
    }
}
