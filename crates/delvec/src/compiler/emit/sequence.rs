//! Sequences and their sites.

use super::*;

/// **Every `sequence` the campaign declares, with the function name it is emitted
/// under** — `seq_<root>_<n>`, where `<root>` is the effect root's own key with
/// its `fx.` prefix dropped (so an `on_objective_complete` bundle reads
/// `rescue_oc_find_the_bell`) and `<n>` is the timeline's index within that
/// root's deep walk.
///
/// Positional rather than content-addressed. A name derived from a hash of the
/// steps told a reader of the emitted pack nothing about where the timeline came
/// from, and it made the `Debug` rendering of every effect part of the pack's
/// bytes — a field added to any verb moved function names that had nothing to do
/// with it. A position moves under exactly the edits a reader expects it to: the
/// bundle it sits in being re-ordered, and nothing else.
///
/// Keyed by the timeline itself (`Verb::Sequence`, which is the whole of what the
/// generated body depends on — the guard wraps the CALL, never the body), and the
/// first declaration wins, so two identical timelines share one function exactly
/// as they did under the content key. Identity is by value and not by address
/// because emission reads a trap's payload from a clone, not from the campaign's
/// own allocation.
///
/// This is **one** enumeration, read by both consumers — `sequence_fns`, which
/// generates the functions, and [`sequence_fn`], which emits the call — so the
/// generator and the caller cannot disagree about a name. It is built from
/// `plan::for_each_effect_root`, the single root walk, so a timeline in any root
/// (a `shortcuts[].on_unlock`, a dialogue `on_respawn`) is named by the same rule.
///
/// Determinism (ADR-0006): the root walk's order is contractual and the deep walk
/// is declaration order; no hashing, no address, no wall clock.
pub(super) fn sequence_sites(c: &delvewright_dsl::Campaign) -> Vec<(&Verb, Audience, String)> {
    let mut out: Vec<(&Verb, Audience, String)> = Vec::new();
    plan::for_each_effect_root(c, &mut |site, effs| {
        let root = fn_safe(site.key.strip_prefix("fx.").unwrap_or(&site.key));
        let aud = site_audience(&site.root);
        let mut here: Vec<(&QuestEffect, Audience)> = Vec::new();
        for e in effs {
            push_effect_deep_audience(e, aud, &mut here);
        }
        let mut n = 0usize;
        for (e, a) in here {
            if !matches!(e.verb, Verb::Sequence { .. }) {
                continue;
            }
            if out.iter().any(|(v, x, _)| **v == e.verb && *x == a) {
                continue;
            }
            out.push((&e.verb, a, format!("seq_{root}_{n}")));
            n += 1;
        }
    });
    let mut names: Vec<&str> = out.iter().map(|(_, _, n)| n.as_str()).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(
        names.len(),
        before,
        "two timelines were given one function name — a positional name must be unique"
    );
    out
}

/// **The audience a root's bundle is emitted under**, per site: a trigger's own
/// declaration ([`trigger_audience`]), else the root class's ([`root_audience`]).
pub(super) fn site_audience(root: &plan::EffectRoot<'_>) -> Audience {
    use delvewright_dsl::EffectRootKind as K;
    use plan::EffectRoot as R;
    match root {
        R::Trigger(t) => trigger_audience(t),
        R::ObjectiveComplete { .. } => root_audience(K::ObjectiveComplete),
        R::QuestComplete(_) => root_audience(K::QuestComplete),
        R::TrapPayload(_) => root_audience(K::TrapPayload),
        R::DialogueRespawn => root_audience(K::DialogueRespawn),
        R::ShortcutUnlock => root_audience(K::ShortcutUnlock),
        R::OnDeath => root_audience(K::OnDeath),
        R::ShopOffer => root_audience(K::ShopOffer),
        R::OnKill(_) => root_audience(K::OnKill),
        R::AssemblyLand(_) => root_audience(K::AssemblyLand),
        R::LoopCross(_) => root_audience(K::LoopCross),
    }
}

/// A campaign id fragment as a datapack function-name segment: ids are
/// `[a-z0-9]+(-[a-z0-9]+)*` and a root key joins them with `.`, so both
/// separators become `_` and nothing else can appear.
pub(super) fn fn_safe(s: &str) -> String {
    s.replace(['-', '.', '/'], "_")
}

/// The generated start-function name for one `sequence` effect started under
/// `aud`: its positional name from [`sequence_sites`].
///
/// # Panics
///
/// If the timeline is not one the campaign declares under that audience.
/// Emission synthesizes effects (the scheduled-probe `set-flag`, a chrome
/// `narrate`) but never a timeline, and a synthesized one would emit a call to a
/// function `sequence_fns` never generated — the dangling-call failure `DW0497`
/// exists for, asserted here at the seam that would create it rather than found
/// downstream.
pub(super) fn sequence_fn(plan: &Plan, eff: &QuestEffect, aud: Audience) -> String {
    sequence_sites(plan.campaign)
        .into_iter()
        .find(|(v, a, _)| **v == eff.verb && *a == aud)
        .map(|(_, _, name)| name)
        .expect("every `sequence` emission lowers is one the campaign declares")
}

/// **Whether a timeline started under `aud` uses its actor at all** — whether
/// any step's body, emitted under `aud`, says something it would not say from
/// the server command source (an `@s` an `actor` audience or a solo root
/// addresses), or any step effect hands a `carrier: "one"` prop or reads or
/// writes a `player`-scoped datum, whose holder is `@s` under every audience.
///
/// A timeline that does not is emitted in the untagged form under every root:
/// its steps then run from the scheduler whether or not the player who started
/// it is still on the server, so a timeline of world facts — a spawn, a flag, a
/// gate — never waits on one player's connection.
pub(super) fn timeline_needs_actor(
    plan: &Plan,
    steps: &[delvewright_dsl::SequenceStep],
    aud: Audience,
) -> bool {
    let player_state = |e: &QuestEffect| {
        let is_player =
            |id: &StateId| state_decl(plan, id).is_some_and(|d| d.scope == StateScope::Player);
        e.writes_state().is_some_and(|(id, _)| is_player(id))
            || e.requires_state().iter().any(|c| is_player(&c.state))
    };
    steps.iter().any(|st| {
        emit_effect_bundle(plan, &st.effects, aud)
            != emit_effect_bundle(plan, &st.effects, Audience::Scheduled)
            || st
                .effects
                .iter()
                .any(|e| e.gives_to_one() || player_state(e))
    })
}

/// **The tag a timeline carries its actor by** (spec-0085 §3.2): `dw_` and the
/// timeline's own function name, so two timelines never share one.
pub(super) fn sequence_tag(base: &str) -> String {
    format!("dw_{base}")
}

/// `sequence` timeline functions (spec-0014): one start function that schedules each
/// step's effect-group at its exact `at_ticks` offset, plus one function per step.
/// Named by position ([`sequence_sites`]) — one function per declared timeline and
/// audience, so a reader of the pack can see which bundle each came from. Empty for
/// a campaign with no sequences.
///
/// **A timeline keeps its actor** (spec-0085 §3.2). Started where there is an
/// acting player ([`Audience::Party`] or [`Audience::Solo`]), the start function
/// tags `@s` with [`sequence_tag`]; every step is dispatched `execute as
/// @a[tag=…] at @s run function …`, so inside it `@s` is the actor — standing
/// where the actor stands, so a listener-relative `~ ~ ~` resolves at them — and
/// its body is emitted under the audience the timeline was started under; the
/// last step removes the tag. `schedule` is replace-mode, so a second start
/// before the first ends re-times the chain for every tagged player: the
/// timeline stays global, with a tag on it. Started from the server command
/// source ([`Audience::Scheduled`]) there is nobody to carry, and the timeline
/// is emitted exactly as before: steps called and scheduled directly, bodies
/// addressing the party.
pub(super) fn sequence_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for (verb, aud, base) in sequence_sites(plan.campaign) {
        let Verb::Sequence { steps } = verb else {
            unreachable!("sequence_sites yields only `sequence` timelines");
        };
        let carries = aud.has_actor() && timeline_needs_actor(plan, steps, aud);
        let tag = sequence_tag(&base);
        let last = steps
            .iter()
            .enumerate()
            .max_by_key(|(i, s)| (s.at_ticks, *i))
            .map(|(i, _)| i);
        let mut start: Vec<String> = Vec::new();
        if carries {
            start.push(format!("tag @s add {tag}"));
        }
        for (i, step) in steps.iter().enumerate() {
            // The step itself, or — carrying an actor — the dispatch that runs it
            // as every tagged player.
            let target = if carries {
                format!("{base}_{i}_as")
            } else {
                format!("{base}_{i}")
            };
            if step.at_ticks == 0 {
                start.push(format!("function {ns}:{target}"));
            } else {
                start.push(format!(
                    "schedule function {ns}:{target} {}t",
                    step.at_ticks
                ));
            }
        }
        out.push((base.clone(), lines(&start)));
        for (i, step) in steps.iter().enumerate() {
            if carries {
                // The scheduler re-invokes with the server source; this puts the
                // actor back as `@s`, at the actor.
                out.push((
                    format!("{base}_{i}_as"),
                    lines(&[format!(
                        "execute as @a[tag={tag}] at @s run function {ns}:{base}_{i}"
                    )]),
                ));
            }
            // EVERY step under one audience, not just the scheduled ones: a
            // timeline whose `at_ticks: 0` step behaved differently from its
            // `at_ticks: 20` step would be a trap.
            let mut b = emit_effect_bundle(plan, &step.effects, aud);
            if carries && Some(i) == last {
                b.push(format!("tag @s remove {tag}"));
            }
            out.push((format!("{base}_{i}"), lines(&b)));
        }
    }
    out
}
