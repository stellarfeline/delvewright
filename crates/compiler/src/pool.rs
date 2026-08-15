//! `DW0498` — a pool draw that seats the same **anchored** prefab twice, stated
//! once at the pool/area declaration.
//!
//! # What goes wrong without this
//!
//! An anchor name belongs to a *prefab*, not to a placement. When the layout
//! solver seats the same prefab more than once, every anchor that prefab
//! declares is suddenly carried by two (or more) placed pieces, and the name no
//! longer picks out a place in the world.
//!
//! The compiler already refuses the sharp end of that: [`crate::solver`]'s
//! `DW0305` fails the build when a **campaign-referenced** anchor — an NPC
//! stand, a `reach-anchor`/`collect`/`interact` target, an `open-gate` /
//! `close-gate` / `set-block` / `move-npc` anchor, a wave spawn, a lane
//! waypoint, a cutscene subject — resolves to more than one placed piece. But
//! `DW0305` fires per anchor, at the **use** site, and only for anchors in that
//! guaranteed set. The pool that made them ambiguous says nothing at all. On
//! the island, `pool/island` draws `prefab/island-greenfield` twice, which makes
//! every one of its nine anchors (`anchor/fold` … `anchor/meadow`) ambiguous —
//! a constraint the campaign author discovered one blocked placement at a time.
//!
//! # Severity: advisory, deliberately
//!
//! A repeated draw with no ambiguous-anchor *use* is legal and shipping content
//! relies on it, so `DW0498` is a [`Severity::Warning`]: it never turns a green
//! campaign red. When an ambiguous anchor IS referenced, `DW0305` still fails
//! the build at the use site — and this warning is the pool-level explanation
//! printed alongside it.
//!
//! # Anchorless fillers are not a finding
//!
//! Repeating an anchorless connector is how a jigsaw pool spans its `pieces`
//! budget — `pool/stone-keep`'s corridors exist to be drawn over and over. A
//! prefab that declares no anchors can make no anchor ambiguous, so it is
//! filtered out before anything is reported. Warning on every campaign that
//! uses fillers would be noise, not information.
//!
//! # What the diagnostic asserts
//!
//! Facts about **this build's assembled draw** — the pieces the pinned seed
//! actually seated (ADR-0006), read after stage-7 massing so what is reported is
//! the layout the player gets. It never claims a pool "always" repeats: with a
//! different member set or piece budget the same pool may not. That is why the
//! prescription is to change the pool, never to reroll the seed.

use std::collections::BTreeMap;

use delvewright_dsl::Diagnostic;

use crate::registry::PrefabRegistry;
use delvewright_dsl::DwCode;

/// `DW0498`: the assembled draw seats one anchor-bearing prefab more than once,
/// so every anchor that prefab declares has more than one carrier. Advisory.
pub const DW_POOL_DOUBLE_DRAW: DwCode = DwCode::every_version("DW0498");

/// One prefab the draw seated more than once, with every anchor it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoubleDraw {
    /// The repeated prefab id (`prefab/<name>`).
    pub prefab_id: String,
    /// How many placed pieces are copies of it (≥ 2).
    pub count: usize,
    /// Its declared anchor names, sorted — every one of them now ambiguous.
    pub anchors: Vec<String>,
}

/// The anchor-bearing prefabs an assembled draw seated more than once, in prefab
/// id order (deterministic: a `BTreeMap` tally over the placed ids).
///
/// Anchorless prefabs are excluded — see the module docs.
pub fn scan<'a>(
    registry: &PrefabRegistry,
    placed: impl IntoIterator<Item = &'a str>,
) -> Vec<DoubleDraw> {
    let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
    for id in placed {
        *tally.entry(id).or_insert(0) += 1;
    }
    tally
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .filter_map(|(prefab_id, count)| {
            let anchors: Vec<String> = registry
                .get(prefab_id)
                .map(|m| m.anchors.keys().cloned().collect())
                .unwrap_or_default();
            if anchors.is_empty() {
                return None;
            }
            Some(DoubleDraw {
                prefab_id: prefab_id.to_string(),
                count,
                anchors,
            })
        })
        .collect()
}

/// The pool-area declaration a `DW0498` is reported against.
#[derive(Debug, Clone, Copy)]
pub struct PoolArea<'a> {
    /// The stage-1 area id.
    pub area_id: &'a str,
    /// Its index in `world.content.areas` — the diagnostic path.
    pub area_index: usize,
    /// The bound pool id.
    pub pool_id: &'a str,
    /// The area's declared `pieces.min`.
    pub pieces_min: u32,
    /// The area's declared `pieces.max`.
    pub pieces_max: u32,
}

/// The single `DW0498` for one pool area's assembled draw, or `None` when the
/// draw repeats nothing that carries anchors.
pub fn check<'a>(
    registry: &PrefabRegistry,
    area: &PoolArea<'_>,
    placed: impl IntoIterator<Item = &'a str>,
) -> Option<Diagnostic> {
    let placed: Vec<&str> = placed.into_iter().collect();
    diagnostic(registry, area, placed.len(), &scan(registry, placed))
}

/// [`check`]'s reporting half: format the one diagnostic for an already-scanned
/// draw. Split out so the facts and their wording are separately readable.
pub fn diagnostic(
    registry: &PrefabRegistry,
    area: &PoolArea<'_>,
    placed_total: usize,
    draws: &[DoubleDraw],
) -> Option<Diagnostic> {
    if draws.is_empty() {
        return None;
    }
    let PoolArea {
        area_id,
        area_index,
        pool_id,
        pieces_min,
        pieces_max,
    } = *area;

    let members = registry.pool(pool_id).unwrap_or(&[]);
    let mut lines = String::new();
    for d in draws {
        let role = members
            .iter()
            .find(|m| m.prefab == d.prefab_id)
            .map(|m| m.role.as_str())
            .unwrap_or("connector");
        let n_role = members.iter().filter(|m| m.role == role).count();
        lines.push_str(&format!(
            "\n  `{}` is seated {} times (pool role `{role}`, one of {n_role} distinct `{role}` \
             member(s)); the anchors it declares, now carried by {} placed pieces each: {}",
            d.prefab_id,
            d.count,
            d.count,
            d.anchors.join(", "),
        ));
    }

    Some(Diagnostic::warning(
        DW_POOL_DOUBLE_DRAW,
        "world",
        format!("/content/areas/{area_index}"),
        format!(
            "area `{area_id}` assembles {placed_total} piece(s) from prefab pool `{pool_id}` \
             (`pieces` {pieces_min}..{pieces_max}, {} pool member(s)), and the draw seats the \
             same anchor-bearing prefab more than once:{lines}\n\nAn anchor with more than one \
             carrier does not name a place. Anchor resolution takes the FIRST carrier in \
             placement order, so anything hung on one of these names — a `spawn-actor`, a \
             `move-actor` destination, a light or block edit — lands on one copy and leaves the \
             other(s) empty. And the moment the campaign references one of them from the set the \
             layout solver has to guarantee (an NPC stand, a `reach-anchor`/`collect`/`interact` \
             target, an `open-gate`/`close-gate`/`set-block`/`move-npc` anchor, a wave spawn, a \
             lane waypoint, a cutscene subject), the build fails hard with `DW0305` at that use \
             site — one use site at a time. This warning is that failure stated once, at the \
             declaration that causes it.\n\nAdvisory, not an error: a pool that repeats an \
             anchored piece is legal and shipping campaigns rely on it. Prescription — either \
             give `{pool_id}` more DISTINCT variant members in the repeated role (same sockets, \
             different prefab), so a draw of this size never has to reuse one piece, or accept \
             these anchors as unusable and keep every placement off them. Do NOT reroll the seed \
             to change the draw: the seed is pinned (ADR-0006) and the pool is what has to \
             change.",
            members.len(),
        ),
    ))
}
