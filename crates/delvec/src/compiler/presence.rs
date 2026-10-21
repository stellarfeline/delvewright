//! **One named character is one body** (`DW1026`): no story state puts two
//! bodies of one character in the world at once, anywhere.
//!
//! A character's identity is its declared name as the player reads it — an
//! NPC's `name` and an actor's `name`, each with its markup dropped
//! (`textstyle::visible`), since a name tag is the only thing over a body that
//! says who it is. Two declarations that carry one name are one character to the
//! player, however many ids the documents give it: a character the story moves
//! between places by despawning one declaration and spawning the next (one body
//! per scene) is legal, and a story state in which both stand somewhere is a
//! person seen twice. One declaration is one body by construction — every
//! `spawn-npc` and `spawn-actor` is guarded on the body's own tag — so the
//! question is only ever about two declarations sharing a name. An assembly
//! declares no name, and a cutscene stand-in wears the player's own profile and
//! lives only while that player is in spectator (`DW0971`), so neither can be a
//! second body of a named character; both are stated in the binding.
//!
//! # The story states
//!
//! Per configuration — the exported critical path and every realised branch's
//! play order in its story world ([`Flow::playthrough`], [`Flow::story_playthrough`]) — the replay's own
//! journal ([`Flow::journal`]) says which lifecycle lines each step fires, in
//! order ([`crate::compiler::flow::JournalStep::fired_in_order`]). World init
//! puts every non-deferred NPC on stage and no actor. Each step then applies the
//! taken dialogue option's `spawn-npc` lines, then its fired lines **one list at
//! a time**: the lines of one list run as one function in one tick, so a
//! `despawn-npc` and a `spawn-npc` side by side never show both bodies, and the
//! state after the list is what the party can see. A nested list (a `sequence`
//! step, an `on_arrive`) runs later, so it is its own state. Every state is
//! judged, the init state included.
//!
//! A `spawn-npc` or `spawn-actor` the replay cannot date — in an environment
//! trigger, a trap payload, a reaction bundle, an `on_kill`, a shop, or on a
//! dialogue option the path does not take — can fire at any state, so its body
//! is counted as possibly present in every state. A `despawn-*` the replay
//! cannot date may never fire, so it removes nothing. A body killed in a fight
//! has not been despawned by the story and is counted present until a
//! `despawn-*` line removes it.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{
    Campaign, Diagnostic, DialogueEffect, DwCode, EffectRootOwner, ExitTier, QuestEffect, Verb,
};

use crate::compiler::flow::Flow;

delvewright_dsl::dw_code! {
    /// `DW1026`: **one named character, two bodies at once.** Two
    /// declarations — NPCs, actors, or one of each — carry one name, and some
    /// story state of some configuration leaves both of them in the world.
    /// Names the character, the bodies, the configuration and the beat, and
    /// the two ways out: despawn the body that leaves in the list that brings
    /// the next one in, or give each its own name.
    pub const DW_ONE_CHARACTER_TWO_BODIES: DwCode = DwCode::new("DW1026", ExitTier::Build);
}

/// What the presence check examined.
#[derive(Clone, Debug, Default)]
pub struct PresenceBinding {
    /// NPC declarations (every one carries a name).
    pub npcs: usize,
    /// Actor declarations.
    pub actors: usize,
    /// Actors that carry a name.
    pub named_actors: usize,
    /// Assemblies (no name; never a character's second body).
    pub assemblies: usize,
    /// Names two or more declarations carry.
    pub shared_names: usize,
    /// Declarations carrying a shared name.
    pub sharing_bodies: usize,
    /// Configurations walked.
    pub configurations: usize,
    /// Story states judged, over every configuration.
    pub states: usize,
    /// Bodies an undatable `spawn-*` can bring in at any state.
    pub undatable: usize,
    /// Characters refused.
    pub refused: usize,
}

impl PresenceBinding {
    /// The line every run prints, zeroes included.
    pub fn line(&self) -> String {
        format!(
            "presence binding (DW1026): {} npc(s) and {} actor(s) ({} named) declared, {} \
             assembl(ies) with no name; {} name(s) carried by more than one declaration, over {} \
             body(ies); {} story state(s) judged (world init, and every state a spawn changes) \
             over {} configuration(s), {} body(ies) an undatable spawn can bring in; {} \
             character(s) refused",
            self.npcs,
            self.actors,
            self.named_actors,
            self.assemblies,
            self.shared_names,
            self.sharing_bodies,
            self.states,
            self.configurations,
            self.undatable,
            self.refused
        )
    }
}

/// A body by its declared id (`npc/…` or `actor/…`; the prefixes are distinct).
type Body = String;

/// Every list of a lifecycle line's root, walked with pointers.
fn walk_lists<'a>(
    list: &'a [QuestEffect],
    base: &str,
    reaction: bool,
    out: &mut Vec<(String, &'a QuestEffect, bool)>,
) {
    for (i, e) in list.iter().enumerate() {
        let path = format!("{base}/{i}");
        out.push((path.clone(), e, reaction));
        for (seg, _, nested) in e.nested_effect_lists_labeled() {
            let r = reaction || matches!(seg.as_str(), "on_respawn" | "on_rest" | "on_caught");
            walk_lists(nested, &format!("{path}/{seg}"), r, out);
        }
    }
}

/// The list a pointer's line belongs to: the pointer without its last index.
fn list_of(pointer: &str) -> &str {
    pointer.rsplit_once('/').map_or("", |(l, _)| l)
}

/// `DW1026` over every story state of every configuration.
pub fn check(c: &Campaign) -> (Vec<Diagnostic>, PresenceBinding) {
    let q = &c.quests.content;
    let npcs = &c.npcs.content.npcs;
    let shown = |n: &str| delvewright_dsl::textstyle::visible(n).into_owned();
    let mut bind = PresenceBinding {
        npcs: npcs.len(),
        actors: q.actors.len(),
        named_actors: q.actors.iter().filter(|a| a.name.is_some()).count(),
        assemblies: q.assemblies.len(),
        ..PresenceBinding::default()
    };
    // name -> the declarations that carry it, in declaration order.
    let mut carriers: BTreeMap<String, Vec<Body>> = BTreeMap::new();
    for n in npcs {
        carriers
            .entry(shown(&n.name))
            .or_default()
            .push(n.id.as_str().to_string());
    }
    for a in &q.actors {
        if let Some(n) = &a.name {
            carriers
                .entry(shown(n))
                .or_default()
                .push(a.id.as_str().to_string());
        }
    }
    carriers.retain(|_, v| v.len() > 1);
    bind.shared_names = carriers.len();
    bind.sharing_bodies = carriers.values().map(Vec::len).sum();
    let name_of: BTreeMap<&str, &str> = carriers
        .iter()
        .flat_map(|(n, bs)| bs.iter().map(move |b| (b.as_str(), n.as_str())))
        .collect();

    // Every lifecycle line, by pointer, and the bodies an undatable one spawns.
    let mut lines: BTreeMap<String, &QuestEffect> = BTreeMap::new();
    let mut anytime: BTreeSet<Body> = BTreeSet::new();
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        let datable_root = matches!(
            site.owner,
            EffectRootOwner::ObjectiveComplete { .. } | EffectRootOwner::QuestComplete { .. }
        );
        let mut all = Vec::new();
        walk_lists(list, &site.path, false, &mut all);
        for (path, e, reaction) in all {
            if datable_root && !reaction {
                lines.insert(path, e);
                continue;
            }
            match &e.verb {
                Verb::SpawnNpc { npc, .. } => {
                    anytime.insert(npc.as_str().to_string());
                }
                Verb::SpawnActor { actor, .. } => {
                    anytime.insert(actor.as_str().to_string());
                }
                _ => {}
            }
        }
    });
    // Dialogue options, flattened as `plan::OptionPlan::n` numbers them:
    // (npc, n) -> (the objectives it completes, the npcs it spawns).
    type Opt = (Vec<String>, Vec<String>, String);
    let mut options: BTreeMap<(String, usize), Opt> = BTreeMap::new();
    for (ti, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let mut n = 0usize;
        for (ni, node) in tree.nodes.iter().enumerate() {
            for (oi, opt) in node.options.iter().enumerate() {
                n += 1;
                let mut completes = Vec::new();
                let mut spawns = Vec::new();
                for e in &opt.effects {
                    match e {
                        DialogueEffect::CompleteObjective { objective } => {
                            completes.push(objective.as_str().to_string());
                        }
                        DialogueEffect::SpawnNpc { npc } => spawns.push(npc.as_str().to_string()),
                        _ => {}
                    }
                }
                let at = format!("/content/dialogues/{ti}/nodes/{ni}/options/{oi}");
                options.insert((tree.npc.as_str().to_string(), n), (completes, spawns, at));
            }
        }
    }

    let flow = Flow::new(c);
    let main = flow.playthrough();
    if main.degenerate || main.cyclic {
        return (Vec::new(), bind); // DW0201/DW0130 own a campaign with no play order
    }
    let mut walks = vec![("the critical path".to_string(), main)];
    for r in crate::compiler::branch::realize(c) {
        if let Some(w) = r.world {
            walks.push((
                format!("branch `{}`", r.branch.id),
                flow.story_playthrough(w),
            ));
        }
    }
    bind.configurations = walks.len();
    // Options taken on some path are dated there; every other option's spawn
    // can happen whenever the dialogue is open.
    let mut taken: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut journals = Vec::new();
    for (label, p) in &walks {
        let journal = flow.journal(p);
        for js in &journal {
            if let Some(n) = js.talk_option
                && let Some((key, _)) = options
                    .iter()
                    .find(|((_, k), (done, _, _))| *k == n && done.contains(&js.objective))
            {
                taken.insert(key.clone());
            }
        }
        journals.push((label.clone(), journal));
    }
    for (key, (_, spawns, _)) in &options {
        if !taken.contains(key) {
            anytime.extend(spawns.iter().cloned());
        }
    }
    bind.undatable = anytime
        .iter()
        .filter(|b| name_of.contains_key(b.as_str()))
        .count();

    let mut refused: BTreeMap<String, Diagnostic> = BTreeMap::new();
    let npc_index: BTreeMap<&str, usize> = npcs
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    // `at` is the line that brought the second body in; `None` is world init,
    // where the declaration of the second body on stage is what put it there.
    let judge = |present: &BTreeSet<Body>,
                 config: &str,
                 beat: &str,
                 at: Option<(&str, String)>,
                 refused: &mut BTreeMap<String, Diagnostic>| {
        for (name, bodies) in &carriers {
            if refused.contains_key(name) {
                continue;
            }
            let on: Vec<&Body> = bodies
                .iter()
                .filter(|b| present.contains(*b) || anytime.contains(*b))
                .collect();
            if on.len() < 2 {
                continue;
            }
            let list = on
                .iter()
                .map(|b| {
                    if anytime.contains(*b) && !present.contains(*b) {
                        format!("`{b}` (spawned by a line the play order cannot date)")
                    } else {
                        format!("`{b}`")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let (stage, path) = at.clone().unwrap_or_else(|| {
                let i = on
                    .iter()
                    .filter_map(|b| npc_index.get(b.as_str()))
                    .nth(1)
                    .copied()
                    .unwrap_or(0);
                ("npcs", format!("/content/npcs/{i}/name"))
            });
            refused.insert(
                name.clone(),
                Diagnostic::error(
                    DW_ONE_CHARACTER_TWO_BODIES,
                    stage,
                    path,
                    format!(
                        "`{name}` is the name over {} declarations, and {list} are all in the \
                         world at once on {config}, {beat}: the party can see one person twice. \
                         If they are one character, despawn the body that is leaving in the same \
                         effect list that brings the next one in (`despawn-npc` / \
                         `despawn-actor` beside the `spawn-npc` / `spawn-actor`); if they are \
                         different people, give each its own `name`",
                        bodies.len()
                    ),
                ),
            );
        }
    };

    for (config, journal) in &journals {
        let mut present: BTreeSet<Body> = npcs
            .iter()
            .filter(|n| !n.deferred)
            .map(|n| n.id.as_str().to_string())
            .collect();
        bind.states += 1;
        judge(&present, config, "from world init", None, &mut refused);
        for js in journal {
            // The taken option's own spawns, in the click that completes the step.
            if let Some(n) = js.talk_option
                && let Some((_, (_, spawns, at))) = options
                    .iter()
                    .find(|((_, k), (done, _, _))| *k == n && done.contains(&js.objective))
                && !spawns.is_empty()
            {
                present.extend(spawns.iter().cloned());
                bind.states += 1;
                judge(
                    &present,
                    config,
                    &format!(
                        "when the dialogue option completing `{}` is taken",
                        js.objective
                    ),
                    Some(("dialogue", at.clone())),
                    &mut refused,
                );
            }
            // The fired lines, one list (one tick) at a time: a parent list
            // before the lists nested in it, siblings in firing order.
            let mut order: Vec<&str> = Vec::new();
            for p in &js.fired_in_order {
                let l = list_of(p);
                if !order.contains(&l) {
                    order.push(l);
                }
            }
            order.sort_by_key(|l| l.matches('/').count());
            for l in order {
                let mut changed: Option<String> = None;
                for p in js.fired_in_order.iter().filter(|p| list_of(p) == l) {
                    let Some(e) = lines.get(p) else { continue };
                    match &e.verb {
                        Verb::SpawnNpc { npc, .. } => {
                            present.insert(npc.as_str().to_string());
                            changed.get_or_insert_with(|| p.clone());
                        }
                        Verb::SpawnActor { actor, .. } => {
                            present.insert(actor.as_str().to_string());
                            changed.get_or_insert_with(|| p.clone());
                        }
                        Verb::DespawnNpc { npc, .. } => {
                            present.remove(npc.as_str());
                        }
                        Verb::DespawnActor { actor, .. } => {
                            present.remove(actor.as_str());
                        }
                        _ => {}
                    }
                }
                let Some(at) = changed else { continue };
                bind.states += 1;
                judge(
                    &present,
                    config,
                    &format!("once `{}` completes ({l})", js.objective),
                    Some(("quests", at)),
                    &mut refused,
                );
            }
        }
    }
    bind.refused = refused.len();
    (refused.into_values().collect(), bind)
}
