//! TD lanes (spec-0016 §6).

use super::*;

/// The lane clock period, in ticks. The spike measured 20–40 ticks as the working
/// band (2 commands per mob per cycle, 0.5–1.0 ms MSPT for a four-mob squad) and
/// ran 30t (1.5 s) live: fast enough that the re-assert defeats vanilla's
/// arrival re-roll and the lone-patroller self-cancel, slow enough to cost
/// nothing.
pub(super) const LANE_PERIOD_TICKS: u32 = 30;

/// How close a squad member must get to the current waypoint for the lane to
/// advance, in blocks. Measured: with the 1.5 s re-assert, an advance radius of 8
/// produced zero stalls over six waypoints.
pub(super) const LANE_ADVANCE_RADIUS: u32 = 8;

/// The tag on a lane squad's `PatrolLeader`. The leader is the wave's first
/// summoned mob (deterministic), and the tag exists so the runtime — and the
/// generated PackTest — can address the one mob vanilla treats specially.
pub(super) fn lane_leader_tag(wave_id: &str) -> String {
    format!("dw_lead_{}", plan::safe_local(wave_id))
}

/// The fake-player holder carrying a lane's current waypoint index on `dw.sys`.
/// One index for the whole squad: the lane is a thing the warband walks, not a
/// per-mob itinerary, which is also what keeps the re-assert to one command per
/// mob per cycle.
pub(super) fn lane_index_holder(wave_id: &str) -> String {
    format!("#lane_{}", plan::safe_local(wave_id))
}

/// A lane mob's attributes with `follow_range` forced to the lane's
/// `aggro_radius` (spec-0016 §6). Perception radius and release radius are the
/// same number by construction: a patrolling raider that targets a player outside
/// its engagement range holds ground instead of marching, so any daylight between
/// the two stalls the squad mid-lane. `DW0381` rejects a contradicting authored
/// override rather than silently overwriting it.
pub(super) fn lane_attributes(
    base: Option<delvewright_dsl::MobAttributes>,
    aggro_radius: u32,
) -> delvewright_dsl::MobAttributes {
    let mut a = base.unwrap_or(delvewright_dsl::MobAttributes {
        max_health: None,
        attack_damage: None,
        movement_speed: None,
        follow_range: None,
    });
    a.follow_range = Some(f64::from(aggro_radius));
    a
}

/// The per-wave lane clock (spec-0016 §6), implementing the spike's verdict
/// verbatim:
///
/// 1. **advance** — when any squad member is within [`LANE_ADVANCE_RADIUS`] of
///    the current waypoint, the shared index steps forward. Emitted in
///    DESCENDING index order so one cycle can advance at most one waypoint (an
///    ascending emission would cascade the whole lane in a single tick), and
///    driven by any squad member rather than the leader alone so a dead leader
///    cannot strand the warband on a waypoint forever.
/// 2. **release** — a mob with a player inside `aggro_radius` gets
///    `Patrolling:0b` and is thereafter a plain native hostile. Vanilla's patrol
///    goal is hard-gated on having no target, so combat-preempts-routing is
///    engine semantics; this line just makes the handover explicit and permanent
///    for as long as the player stays close.
/// 3. **re-assert** — a mob with no player inside `aggro_radius` is put back on
///    the lane. This is what defeats vanilla's random re-roll on arrival and the
///    lone-patroller self-cancel; it is inert during combat because the goal
///    cannot restart while the mob has a target.
/// 4. **re-arm** — reschedule while any squad member lives, so the clock stops
///    on its own when the wave is cleared.
pub(super) fn lane_tick_fn(
    ns: &str,
    w: &delvewright_dsl::Wave,
    lane: &delvewright_dsl::WaveLane,
    wps: &[[i32; 3]],
    guard: &str,
) -> (String, String) {
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let idx = lane_index_holder(w.id.as_str());
    let r = lane.aggro_radius;
    let adv = LANE_ADVANCE_RADIUS;
    let mut body: Vec<String> = Vec::new();
    for i in (0..wps.len().saturating_sub(1)).rev() {
        let c = ent_xyz(wps[i]);
        body.push(format!(
            "execute if score {idx} dw.sys matches {i} positioned {} {} {} if entity \
             @e[tag={tag},distance=..{adv}] run scoreboard players set {idx} dw.sys {}",
            c[0],
            c[1],
            c[2],
            i + 1
        ));
    }
    body.push(format!(
        "execute as @e[tag={tag}] at @s if entity @a[distance=..{r}{guard}] run data merge entity @s \
         {{Patrolling:0b}}"
    ));
    for (i, t) in wps.iter().enumerate() {
        body.push(format!(
            "execute if score {idx} dw.sys matches {i} as @e[tag={tag}] at @s unless entity \
             @a[distance=..{r}{guard}] run data merge entity @s {{Patrolling:1b,patrol_target:[I;{},{},{}]}}",
            t[0], t[1], t[2]
        ));
    }
    body.push(format!(
        "execute if entity @e[tag={tag}] run schedule function {ns}:lane_tick_{safe} \
         {LANE_PERIOD_TICKS}t"
    ));
    (format!("lane_tick_{safe}"), lines(&body))
}
