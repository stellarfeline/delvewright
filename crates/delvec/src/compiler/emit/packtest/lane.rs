use super::*;

/// spec-0016 §6 PackTests: four templates, one per live-verified claim of the TD
/// lane mechanism. Emits nothing for a campaign with neither a lane nor an
/// aggro-edge wave.
///
/// * `souls_td_patrol_nbt` — **the codec trap, as a test and not a comment.**
///   1.21.11's strict codec silently DROPS the legacy `PatrolTarget:{X,Y,Z}`
///   compound; only the snake_case `patrol_target:[I;x,y,z]` int-array survives.
///   The failure mode is a squad that patrols to vanilla-rolled random points —
///   working-but-drunk, invisible to every other proof. This asserts the array
///   reads back off the summoned squad, that exactly one mob is the
///   `PatrolLeader`, and that the whole squad spawns `Patrolling:1b`.
/// * `souls_td_lane_march` — the lane advances in **march order**: arriving at
///   the current waypoint steps the index by exactly one, and standing at a
///   LATER waypoint while the index still points at an earlier one does not skip
///   ahead (the lane is walked, not teleported through).
/// * `souls_td_lane_release` — routing hands over to native AI at aggro range:
///   with no player inside `aggro_radius` the whole squad is re-asserted onto
///   the lane; with a player inside it, every mob is `Patrolling:0b`.
/// * `souls_td_aggro_edge` — a `summon: aggro-edge` wave really materializes on
///   its perception ring around the defended anchor: full authored count, every
///   mob at its own `follow_range` from the defended point, measured from the
///   same snapped centre the compiler placed them around.
pub(super) fn emit_td_lane_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    lane_routes: &crate::compiler::nav::LaneRoutes,
    wave_rings: &WaveRings,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let waves = &plan.campaign.quests.content.waves;
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    if let Some((w, lane, wps)) = waves.iter().find_map(|w| {
        let lane = w.lane.as_ref()?;
        let wps = lane_routes.get(w.id.as_str())?;
        Some((w, lane, wps))
    }) {
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let lead = lane_leader_tag(w.id.as_str());
        let idx = lane_index_holder(w.id.as_str());
        let total = plan::wave_total(w);
        let r = lane.aggro_radius;
        let t0 = wps[0];
        // A cell 200 blocks above the lane: no player in the batch is anywhere
        // near it, so `unless entity @a[distance=..R]` is decidable from the
        // compiler's chair — the re-assert half of the clock can be asserted
        // without knowing where a sibling template parked its dummy.
        let high = |c: [i32; 3]| {
            let p = ent_xyz(c);
            format!("{} 200.0 {}", p[0], p[2])
        };

        let mut b = packtest_header(&format!(
            "{title}: lane `{}` spawns as a patrol squad, snake_case `patrol_target` and all \
             (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdnbt dw.sys if entity @e[tag={tag}]"
        ));
        b.push(format!("assert score #n_tdnbt dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #l_tdnbt dw.sys if entity \
             @e[tag={lead},nbt={{PatrolLeader:1b}}]"
        ));
        b.push("assert score #l_tdnbt dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #p_tdnbt dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #p_tdnbt dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #t_tdnbt dw.sys if entity \
             @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
            t0[0], t0[1], t0[2]
        ));
        b.push(format!("assert score #t_tdnbt dw.sys matches {total}"));
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_patrol_nbt", b);

        let mut b = packtest_header(&format!(
            "{title}: lane `{}` advances in march order, one waypoint at a time (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("scoreboard players set {idx} dw.sys 0"));
        if wps.len() > 1 {
            // Standing at a LATER waypoint does not skip the lane forward: the
            // index only ever advances off the waypoint it currently names.
            b.push(format!("tp @e[tag={tag}] {}", high(wps[1])));
            b.push(format!("function {ns}:lane_tick_{safe}"));
            b.push(format!("assert score {idx} dw.sys matches 0"));
        }
        b.push(format!("tp @e[tag={tag}] {}", ent_xyz(wps[0]).join(" ")));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!("assert score {idx} dw.sys matches 1"));
        if wps.len() > 1 {
            // …and the squad is really re-pointed at the next waypoint (high
            // above the lane, so no player is inside the release radius).
            b.push(format!("tp @e[tag={tag}] {}", high(wps[1])));
            b.push(format!("function {ns}:lane_tick_{safe}"));
            b.push(format!(
                "execute store result score #m_tdmar dw.sys if entity \
                 @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
                wps[1][0], wps[1][1], wps[1][2]
            ));
            b.push(format!("assert score #m_tdmar dw.sys matches {total}"));
        }
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_lane_march", b);

        let (pin, sel) = pin_dummy("dw_pt_tdrel");
        let mut b = packtest_header(&format!(
            "{title}: lane `{}` marches while distant and is released to native AI at aggro \
             range (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("scoreboard players set {idx} dw.sys 0"));
        b.push(format!("tp @e[tag={tag}] {}", high(wps[0])));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #d_tdrel dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #d_tdrel dw.sys matches {total}"));
        // Bring the squad onto the pinned dummy rather than moving the player:
        // the test's own dummy stays exactly where the batch put it, so nothing
        // it leaves behind can perturb a sibling. `{r}` blocks is the release
        // radius; 0 is inside it by any measure.
        b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #a_tdrel dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push("assert score #a_tdrel dw.sys matches 0".to_string());
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("tag {sel} remove dw_pt_tdrel"));
        write("souls_td_lane_release", b);

        // --- the re-summon re-stations the squad ---
        //
        // A wave re-seat is `kill` + the wave's own `spawn_<wave>`, and the whole
        // stationed-re-seat ruling rests on that second half putting the squad
        // back on the lane exactly as the first summon did. This drives the same
        // two commands from the far side of the mechanism's worst state — the
        // squad hauled onto the party, released to native AI by the real clock,
        // its march clock at the END of the lane — and demands the fresh squad is
        // routed from waypoint 0 again with the release gone. Emitted for every
        // lane wave, bonfire or not, so a campaign that ships lanes proves it
        // without having to ship a rest point next to one.
        let (pin, sel) = pin_dummy("dw_pt_tdrst");
        let mut b = packtest_header(&format!(
            "{title}: re-summoning lane `{}` re-stations it — the feral release does not survive \
             (spec-0016 §1/§6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #f_tdrst dw.sys if entity @e[tag={tag},nbt={{Patrolling:0b}}]"
        ));
        b.push(format!("assert score #f_tdrst dw.sys matches {total}"));
        b.push(format!(
            "scoreboard players set {idx} dw.sys {}",
            wps.len().saturating_sub(1)
        ));
        // The re-seat body, verbatim.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdrst dw.sys if entity @e[tag={tag},nbt={{Patrolling:1b}}]"
        ));
        b.push(format!("assert score #n_tdrst dw.sys matches {total}"));
        b.push(format!(
            "execute store result score #t_tdrst dw.sys if entity \
             @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
            t0[0], t0[1], t0[2]
        ));
        b.push(format!("assert score #t_tdrst dw.sys matches {total}"));
        b.push(format!("assert score {idx} dw.sys matches 0"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("tag {sel} remove dw_pt_tdrst"));
        write("souls_td_lane_reseat", b);
        let _ = r;
    }

    if let Some((w, centre)) = waves.iter().find_map(|w| {
        (w.summon == Some(delvewright_dsl::WaveSummon::AggroEdge))
            .then(|| wave_rings.get(w.id.as_str()).map(|c| (w, *c)))
            .flatten()
    }) {
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let total = plan::wave_total(w);
        let mut b = packtest_header(&format!(
            "{title}: aggro-edge wave `{}` materializes on its perception ring, never on the \
             defended point (spec-0016 §6)",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!(
            "execute store result score #n_tdedg dw.sys if entity @e[tag={tag}]"
        ));
        b.push(format!("assert score #n_tdedg dw.sys matches {total}"));
        // One assertion per (species, follow_range) group: each group's mobs must
        // ALL sit in its own ring band. The band is the compiler's annulus
        // tolerance plus 0.1 for the float compare — mobs and the ring centre are
        // both addressed at cell centres, so no rounding slack is needed.
        let mut groups: BTreeMap<(String, String), i64> = BTreeMap::new();
        for m in &w.mobs {
            let Some(radius) = m.attributes.and_then(|a| a.follow_range) else {
                continue;
            };
            *groups
                .entry((m.entity.clone(), fmt_f64(radius)))
                .or_default() += i64::from(m.count);
        }
        let c = ent_xyz(centre);
        for (i, ((entity, radius), count)) in groups.iter().enumerate() {
            let radius: f64 = radius.parse().unwrap_or_default();
            let lo = fmt_f64((radius - AGGRO_RING_TOLERANCE - 0.1).max(0.0));
            let hi = fmt_f64(radius + 0.1);
            b.push(format!(
                "execute positioned {} {} {} store result score #r{i}_tdedg dw.sys if entity \
                 @e[tag={tag},type={entity},distance={lo}..{hi}]",
                c[0], c[1], c[2]
            ));
            b.push(format!("assert score #r{i}_tdedg dw.sys matches {count}"));
        }
        b.push(format!("kill @e[tag={tag}]"));
        write("souls_td_aggro_edge", b);
    }
}
