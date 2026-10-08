//! How a body the compiler removes leaves the scene, and the unseen sweep.

use super::*;

/// How a body the compiler removes leaves the scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Exit {
    /// The body leaves where no player can see it go: no death animation, no
    /// red flash, no death particles in the room. Every removal the story makes
    /// that is not a death on screen — a `despawn-npc`, a `despawn-actor`
    /// `vanish`, the puppet an `unleash` replaces, a bonfire's re-seat.
    Unseen,
    /// The body dies where it stands, with vanilla's death animation: a
    /// `despawn-actor` the author wrote as `style: kill`, and nothing else.
    OnScreen,
}

/// The tag an [`Exit::Unseen`] body carries, alone, from the moment it leaves
/// until [`UNSEEN_SWEEP_FN`] removes it.
pub(super) const UNSEEN_TAG: &str = "dw_unseen";

/// The function that removes every body waiting under [`UNSEEN_TAG`].
pub(super) const UNSEEN_SWEEP_FN: &str = "unseen_sweep";

/// The Y an [`Exit::Unseen`] body is moved to, straight down its own column: the
/// overworld's floor is `-64`, and vanilla's void damage starts below `-128`, so
/// a body frozen here is under the world and takes no damage.
pub(super) const UNSEEN_Y: i32 = -128;

/// Ticks between an [`Exit::Unseen`] body's departure and its removal. The
/// server tells a client that a tracked entity moved within the entity type's
/// update interval (at most 3 ticks for a living entity); the death that follows
/// is sent only after that, so no client renders it where the body stood.
pub(super) const UNSEEN_DELAY_TICKS: u32 = 5;

/// **The one way the compiler removes a body it placed.** Every removal of an
/// NPC's body, an actor's puppet or twin, or a wave's mobs is built here, so a
/// site cannot choose a removal that plays a death the story did not write, nor
/// forget the strip in front of it.
///
/// [`Exit::OnScreen`] is vanilla `/kill` in place. [`Exit::Unseen`] composes
/// intended primitives only: any passenger is set down (a rider is never carried
/// out of the world), the body is moved straight down its own column to
/// [`UNSEEN_Y`] and frozen there with every tag replaced by [`UNSEEN_TAG`] — so
/// from that command on no selector the datapack writes can find it — and
/// [`UNSEEN_SWEEP_FN`] kills it [`UNSEEN_DELAY_TICKS`] later, under the world.
/// The sweep is scheduled with `replace`, and only when a body is really
/// leaving, so every body waits at least the full delay. The move runs
/// `execute as … at @s` because every path reaching a removal runs from the
/// server source, where a bare `tp <targets> ~ Y ~` resolves `~ ~` at world
/// spawn rather than down each body's own column.
///
/// `declares_drops` is the body's own declaration ([`wave_declares_drops`],
/// [`actor_declares_drops`]) and puts [`strip_drops_line`] first: a declared
/// drop is what a player's kill yields, never a removal's, and the sweep's
/// `/kill` under the world is still an ordinary death.
pub(super) fn removal_lines(ns: &str, tag: &str, declares_drops: bool, exit: Exit) -> Vec<String> {
    let mut out = Vec::new();
    if declares_drops {
        out.push(strip_drops_line(tag));
    }
    match exit {
        Exit::OnScreen => out.push(format!("kill @e[tag={tag}]")),
        Exit::Unseen => {
            out.push(format!(
                "execute if entity @e[tag={tag}] run schedule function {ns}:{UNSEEN_SWEEP_FN} {UNSEEN_DELAY_TICKS}t replace"
            ));
            out.push(format!(
                "execute as @e[tag={tag}] on passengers run ride @s dismount"
            ));
            out.push(format!(
                "execute as @e[tag={tag}] at @s run tp @s ~ {UNSEEN_Y} ~"
            ));
            out.push(format!(
                "execute as @e[tag={tag}] run data merge entity @s {{Tags:[\"{UNSEEN_TAG}\"],NoGravity:1b,NoAI:1b,Silent:1b}}"
            ));
        }
    }
    out
}

/// **The one way a generated PackTest runs the unseen sweep in the tick it needs
/// a removal's death**: a `kill` of the bodies waiting under [`UNSEEN_TAG`] in the
/// column of the template's own dummy `sel`, at [`UNSEEN_Y`], and of no other.
///
/// The suite shares one world, and [`UNSEEN_SWEEP_FN`] kills every waiting body in
/// it: a template that ran it would kill each body a sibling's removal parked in
/// the same batch, inside the [`UNSEEN_DELAY_TICKS`] the removal promises it —
/// the delay `v04_despawn_<npc>` watches, and which a sibling's direct sweep
/// failed on the pinned server. A template that needs the death drags the bodies
/// onto its dummy before the removal, so the removal parks them in the dummy's
/// column, where this reaches them. The dummies of a batch stand apart, so no two
/// templates' columns meet.
pub(super) fn unseen_sweep_under(sel: &str) -> String {
    format!("execute at {sel} positioned ~ {UNSEEN_Y} ~ run kill @e[tag={UNSEEN_TAG},distance=..1]")
}

/// The [`UNSEEN_SWEEP_FN`] function, emitted exactly when some function
/// schedules it.
pub(super) fn unseen_sweep_fn(fns: &[(String, String)], ns: &str) -> Option<(String, String)> {
    let call = format!("schedule function {ns}:{UNSEEN_SWEEP_FN} ");
    fns.iter().any(|(_, body)| body.contains(&call)).then(|| {
        (
            UNSEEN_SWEEP_FN.to_string(),
            lines(&[format!("kill @e[tag={UNSEEN_TAG}]")]),
        )
    })
}
