//! Waves: spawn planning, holders, the census, and the wave machinery.

use super::*;

/// Validated spawn cells per wave: wave id → one standable cell per mob, in
/// summon order. Only waves whose spawn anchor resolves have an entry.
pub(super) type WavePlacements = BTreeMap<String, Vec<[i32; 3]>>;

/// The **defended point** each `summon: aggro-edge` wave's perception ring is
/// measured from (spec-0016 §6): its `anchor`, snapped to standable footing. The
/// generated PackTest asserts ring distance against exactly this cell, so the
/// runtime check and the compile-time placement share one origin.
pub(super) type WaveRings = BTreeMap<String, [i32; 3]>;

/// Where every wave actually IS in the assembled world: the three products of
/// wave planning, which always travel together into the generated PackTests.
/// Bundled because a template that asks "did the re-seat put them back?" needs
/// all three — the seated cells, the lane polyline, and the aggro-edge ring
/// centre — to say where "back" is.
pub(super) struct WaveGeometry<'a> {
    /// DW0312-proven seated spawn cells, per wave.
    pub(super) placements: &'a WavePlacements,
    /// DW0386-proven lane polylines, per lane wave.
    pub(super) lanes: &'a crate::compiler::nav::LaneRoutes,
    /// The snapped ring centre of each `summon: aggro-edge` wave.
    pub(super) rings: &'a WaveRings,
}

/// Seat every wave's mobs on compiler-validated standable cells near the wave
/// anchor, confined to the anchor's own assembled piece so the flock never strings
/// across a socket seam into a neighbouring room — an unconfined flock spreads
/// `+x` off its anchor across the nearest seam toward void, some bodies
/// ending inside blocks or outside the room. Cells are chosen by ascending BFS
/// distance from the anchor with a fixed `(y, z, x)` tie-break — deterministic
/// (ADR-0006). A wave that needs more standable footing than its room offers fails
/// the build with [`DW_WAVE_NO_ROOM`] (`DW0312`). A wave whose spawn anchor resolves
/// in no assembled area is skipped (DW0310 handles the dangling reference).
pub(super) fn plan_wave_spawns(
    plan: &Plan,
    world: &crate::compiler::nav::World,
) -> Result<(WavePlacements, WaveRings), BuildFailure> {
    // Wave mobs cannot right-click a fence gate open: seat them on the
    // no-gate-use view, where a closed gate cell is a 1.5-tall barrier — never a
    // seat, and never a doorway the seating flood spills through.
    let entity_world_owned;
    let world: &crate::compiler::nav::World = if world.has_use_gates() {
        entity_world_owned = world.without_gate_use();
        &entity_world_owned
    } else {
        world
    };
    let c = plan.campaign;
    let mut out: WavePlacements = BTreeMap::new();
    let mut rings: WaveRings = BTreeMap::new();
    for w in &c.quests.content.waves {
        let (Some(anchor), Some(area)) = (
            wave_spawn_pos(plan, w.id.as_str()),
            plan::wave_area(c, w.id.as_str()),
        ) else {
            continue;
        };
        let need = plan::wave_total(w).max(0) as usize;
        // spec-0016 §6: an aggro-edge wave is spirit-summoned at the edge of
        // perception instead of seated around its anchor, so its cells come from
        // per-mob rings across the whole ARENA rather than the anchor's room.
        if w.summon == Some(delvewright_dsl::WaveSummon::AggroEdge) {
            let (cells, centre) = plan_aggro_edge_spawns(plan, world, w, area, anchor)?;
            out.insert(w.id.as_str().to_string(), cells);
            rings.insert(w.id.as_str().to_string(), centre);
            continue;
        }
        // The room the wave's mobs must stay inside, so the placement flood-fill
        // never crosses a socket seam.
        let bounds = plan.piece_bounds(area, anchor);
        let cells = world.confined_standable_cells(anchor, bounds);
        if cells.len() < need {
            return Err(BuildFailure::Diagnostic {
                code: DW_WAVE_NO_ROOM,
                message: format!(
                    "spawn-wave `{wave}` needs {need} standable spawn cell(s) near \
                     anchor `{anchor_name}` in area `{area}`, but its room provides \
                     only {found}. Each wave mob must stand on validated footing \
                     inside the anchor's own piece (bounds {bounds:?}); the compiler \
                     will not pile mobs into blocks or spill them across a socket \
                     seam. Fix the content: shrink this wave's mob count (currently \
                     {need}) or spawn it in a larger room. Do NOT widen the piece's \
                     socket seams or move the anchor into an adjoining room — that \
                     reopens the cross-seam spill this guard prevents.",
                    wave = w.id.as_str(),
                    anchor_name = w.anchor.as_str(),
                    found = cells.len(),
                ),
            });
        }
        out.insert(
            w.id.as_str().to_string(),
            cells.into_iter().take(need).collect(),
        );
    }
    Ok((out, rings))
}

/// How far inside `follow_range` the aggro ring may reach, in blocks. A discrete
/// voxel grid rarely holds a standable cell at *exactly* `follow_range`, so the
/// ring is an annulus `[follow_range - 1, follow_range]` — one-sided on purpose:
/// a cell outside the mob's own perception summons a mob that stands there
/// (see [`crate::compiler::nav::World::annulus_standable_cells`]).
pub(super) const AGGRO_RING_TOLERANCE: f64 = 1.0;

/// Seat a `summon: aggro-edge` wave (spec-0016 §6) on the boundary of its own
/// perception: for each mob stack, the standable, reachable, line-of-sight cells
/// at that stack's `attributes.follow_range` from the defended anchor, nearest
/// first, no two mobs sharing a cell.
///
/// The party is expected at the defended anchor (that is what "defended" means),
/// so the ring around it IS the aggro boundary of the players — the mobs
/// materialize at the edge of perception and close under pure native AI. The
/// radius is per-stack because perception is per-species: a heavier mob that
/// sees further starts further out, which is exactly the read the owner asked
/// for ("spirit-summoned at the edge, never on top of the players").
///
/// `DW0387` if a stack's ring cannot seat it. `follow_range` is guaranteed
/// present by `DW0385` at validation time; a stack without one is skipped rather
/// than guessed.
pub(super) fn plan_aggro_edge_spawns(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    w: &delvewright_dsl::Wave,
    area: &str,
    anchor: [i32; 3],
) -> Result<(Vec<[i32; 3]>, [i32; 3]), BuildFailure> {
    let bounds = match plan.areas.iter().find(|a| a.area_id == area) {
        Some(a) => a.bounds(),
        None => (anchor, anchor),
    };
    let centre = world.ring_centre(anchor, bounds).unwrap_or(anchor);
    let mut used: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut cells: Vec<[i32; 3]> = Vec::new();
    for mob in &w.mobs {
        let Some(radius) = mob.attributes.and_then(|a| a.follow_range) else {
            continue;
        };
        let need = mob.count as usize;
        // Band [r-2, r-1], strictly INSIDE perception. Ladder evidence (the
        // drowned bell, runs 10 and 12): the original one-sided [r-1, r] band
        // seats mobs at the marginal edge of perceiving a defender AT the
        // anchor — vanilla target acquisition at exactly `follow_range` is a
        // coin flip, and a summoned mob that acquires nobody stands idle
        // forever, timing out the kill objective. One block of margin turns
        // "materializes at the edge of what it can sense" from fiction into
        // guaranteed engagement.
        let band_outer = (radius - 1.0).max(2.0);
        let ring = world.annulus_standable_cells(anchor, bounds, band_outer, AGGRO_RING_TOLERANCE);
        let picked: Vec<[i32; 3]> = ring
            .iter()
            .copied()
            .filter(|c| !used.contains(c))
            .take(need)
            .collect();
        if picked.len() < need {
            return Err(BuildFailure::Diagnostic {
                code: DW_AGGRO_EDGE_NO_RING,
                message: format!(
                    "`summon: aggro-edge` wave `{wave}` cannot seat {need} × `{entity}` on its \
                     perception ring: at `follow_range` {radius} (the band \
                     [{band_outer}-{AGGRO_RING_TOLERANCE}, {band_outer}], one block inside perception) around defended anchor \
                     `{anchor_name}` ({anchor:?}) in area `{area}`, only {found} \
                     cell(s) are standable, walk-reachable AND in line of sight of the anchor. \
                     The mobs must materialize at the EDGE of perception (spec-0016 §6) — the \
                     compiler will not quietly drop them on the party instead, nor spawn fewer \
                     than authored (a short wave makes a `kill` countdown that never reaches \
                     zero). Fix the content: give the arena room at that radius, lower this \
                     stack's `follow_range` to a ring the arena actually has, or move the \
                     defended anchor off the wall.",
                    wave = w.id.as_str(),
                    entity = mob.entity,
                    anchor_name = w.anchor.as_str(),
                    found = ring.iter().filter(|c| !used.contains(*c)).count(),
                ),
            });
        }
        used.extend(picked.iter().copied());
        cells.extend(picked);
    }
    Ok((cells, centre))
}

/// The absolute spawn position of a wave: the world coords of its `anchor`,
/// resolved in the area of the quest (or single-area trigger) that *spawns* it —
/// see [`plan::wave_area`]. Deliberately independent of objective type, so a
/// kill-less "live threat" wave (spec-0008 §4) resolves a spawn position exactly
/// like a wave that a `kill` objective later drains.
pub(super) fn wave_spawn_pos(plan: &Plan, wave_id: &str) -> Option<[i32; 3]> {
    let c = plan.campaign;
    let w = plan::wave_of(c, wave_id)?;
    let area = plan::wave_area(c, wave_id)?;
    plan.point(area, w.anchor.as_str())
}

/// Fail the build if any `spawn-wave` effect references a wave whose spawn
/// position cannot be resolved (`DW0310`). Such a wave emits no `spawn_<wave>`
/// function, yet the effect still emits a `function <ns>:spawn_<wave>` call — a
/// silently dangling reference that would never spawn the wave at runtime. A
/// compile-time diagnostic turns that content mistake into a loud build failure
/// instead of a missing enemy the QA hour has to notice.
pub(super) fn check_wave_spawns(plan: &Plan) -> Result<(), BuildFailure> {
    // `all_campaign_effects` is the emitter's own traversal — the one that decides
    // where a `function <ns>:spawn_<wave>` call is written. Reading the spawn sites
    // from it is what makes this check see exactly the calls that ship, including
    // the ones nested in a `sequence` step, an `on_respawn` bundle or a trap
    // payload. Scanning only the top-level chains, as this did, is how the island's
    // round-21 build shipped two `spawn_…` calls with nothing behind them.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for e in all_campaign_effects(plan.campaign) {
        if let Some(wave) = e.spawn_wave() {
            let id = wave.as_str();
            if seen.insert(id) && wave_spawn_pos(plan, id).is_none() {
                let multi_area = delvewright_dsl::Placement::of(plan.campaign)
                    == delvewright_dsl::Placement::Prefabs
                    && plan.campaign.world.content.areas.len() > 1;
                let fired_globally = plan::wave_area(plan.campaign, id).is_none()
                    && fired_only_by_global_roots(plan.campaign, id);
                let remedy = if multi_area && fired_globally {
                    "This wave is fired only from a global root (a trigger, a trap payload, an \
                     actor's kill) in a campaign of several areas, and a global root carries no \
                     area, so the compiler cannot say which area's assembly must provide the \
                     anchor. Fire it from a quest booked in the wave's area (its \
                     `on_objective_complete` or `on_complete`), or make the campaign \
                     single-area."
                } else {
                    "Ensure a quest in the wave's area fires the `spawn-wave`, or that the wave \
                     `anchor` exists in that area's prefab pool."
                };
                return Err(BuildFailure::Diagnostic {
                    code: DW_WAVE_SPAWN_UNRESOLVED,
                    message: format!(
                        "`spawn-wave` references wave `{id}`, but its spawn anchor is \
                         not placed in any assembled area — the emitted \
                         `spawn_{safe}` call would dangle and the wave never spawn. {remedy}",
                        safe = plan::safe_local(id),
                    ),
                });
            }
        }
    }
    Ok(())
}

/// The fake-player scoreboard holder marking a `respawns_on_rest` wave as
/// **seated** — set by the wave's own `spawn_<wave>` (spec-0016 §1). A bonfire
/// only re-seats waves the party has actually met; without this a rest would
/// spawn every future wave in the delve at once.
pub(super) fn wave_seated_holder(wave_id: &str) -> String {
    format!("#wseat_{}", plan::safe_local(wave_id))
}

/// The fake-player scoreboard holder counting the wave's deaths **a player was
/// credited with**, since the seating in force — written by `spawn_<wave>` (which
/// zeroes it) and by `k_reward_<wave>` (which adds one per credited kill), and
/// stated on the census line.
///
/// ## What it exists to separate
///
/// The countdown is a measurement of what still STANDS, so it clears whether a
/// body was cut down or fell into a lethal volume — which is the whole point of
/// it, and which is also why nothing downstream can tell the two apart any more.
/// The floor gate needs exactly that distinction: it reports an encounter the
/// content billed `elite`/`boss` that the UNASSISTED bot beat cold, and a
/// cohort the world killed is not a fight the bot beat. Measured on the gallery's
/// own bot ladder: of three `wave/muster` bodies one withered in `lethal/east-pit`
/// and one fell, the bot felled the third, and the advisory said the bot had
/// beaten the encounter on its first attempt.
///
/// `minecraft:player_killed_entity` is the only trigger vanilla has here, and it
/// fires exactly for the case this holder is about, so the ledger is the
/// advancement's own count and never a table the compiler invents.
///
/// ## Why the SEATING is not on the wire beside it
///
/// A reader wants "how many died with nobody credited", which is
/// `seated - standing - credited`. `standing` and `credited` are runtime facts
/// only the server holds. `seated` is not: `spawn_<wave>` writes
/// [`plan::wave_total`], a compile-time constant, and the same constant already
/// reaches the harness as `combat-plan.json`'s `count`. Putting it on the census
/// line too would ship one fact by two routes, which is a pair that can disagree
/// rather than a second measurement.
pub(super) fn wave_credited_holder(wave_id: &str) -> String {
    format!("#wcred_{}", plan::safe_local(wave_id))
}

/// Join a marker's prefix and its integer fields into one `tellraw` component,
/// rendering as a single anchored line the harness parses whole.
///
/// The grammar is the completion channel's, one token further on:
/// `[dw:<token> <campaign> <wave> <n> <n> …]`. It inherits the same three
/// unforgeability properties — player chat cannot begin with the sigil, the
/// campaign id is part of the match, and `DW0182` reserves the sigil in every
/// player-visible string — so a census line is as much an oracle as a completion
/// marker is.
pub(super) fn census_component(ns: &str, token: &str, wave_id: &str, holders: &[&str]) -> Value {
    let mut extra: Vec<Value> = Vec::new();
    for h in holders {
        extra.push(sys_score(h));
        extra.push(json!({ "text": " " }));
    }
    // The trailing separator becomes the closing bracket.
    extra.pop();
    extra.push(json!({ "text": "]" }));
    json!({
        "text": format!("[dw:{token} {ns} {wave_id} "),
        "color": "dark_gray",
        "extra": extra
    })
}

/// The census SUMMARY line: sequence, how many of the wave stand, how many of
/// those are branded (fought in a previous life), how many are below full health,
/// and how many of its deaths since the seating in force a player was credited
/// with ([`wave_credited_holder`]).
///
/// The last field is the one that is not about the bodies standing there: every
/// other number describes the survivors, and `credited` describes the fallen. It
/// is what lets a reader subtract — `count - present - credited` is how many of
/// the wave died with nobody credited, i.e. how many the WORLD killed.
pub(super) fn census_summary_component(ns: &str, wave_id: &str) -> Value {
    let credited = wave_credited_holder(wave_id);
    census_component(
        ns,
        plan::MARKER_TOKEN_CENSUS,
        wave_id,
        &["#wcen_seq", "#wcen_n", "#wcen_b", "#wcen_d", &credited],
    )
}

/// One mob's line inside a census: sequence, position and health, all ×100 so
/// they cross the chat channel as exact integers.
pub(super) fn census_mob_component(ns: &str, wave_id: &str) -> Value {
    census_component(
        ns,
        plan::MARKER_TOKEN_CENSUS_MOB,
        wave_id,
        &[
            "#wcen_seq",
            "#wcen_x",
            "#wcen_y",
            "#wcen_z",
            "#wcen_h",
            "#wcen_m",
        ],
    )
}

/// Per-objective "already announced" scoreboard (v0.3 objective-activation
/// feedback, M2 fix 4). Set once the objective's title/hint has been shown so the
/// announce fires exactly once per player.
pub(super) fn announce_score(obj_id: &str) -> String {
    format!("dw.ann_{}", plan::safe_local(obj_id))
}

/// The `dw.sys` scratch holder the per-tick wave recount writes the standing-body
/// count into, one wave at a time, immediately before copying it over that wave's
/// countdown. Shared across waves on purpose: `tick` is one atomic function call,
/// so the write and the read that consumes it cannot be interleaved with another
/// wave's — the same argument the census makes for `#wcen_*`.
pub(super) const WAVE_LIVE: &str = "#wlive";

/// The waves that HAVE runtime machinery — the one traversal both the machinery
/// emitter and every template that drives it must read.
///
/// A wave whose spawn anchor resolves in no assembled area gets no
/// `spawn_<wave>`, no census and no kill reward (`DW0310` is the standing check
/// on the dangling `spawn-wave` that leaves behind). A template loop that walked
/// the authored list instead emitted templates calling four functions that do not
/// exist — which `DW0497` refused, correctly, with the words this comment exists
/// to honour: *an emitter's call walk and its machinery walk have gone out of
/// agreement; fix the emitter so both derive from one traversal.*
///
/// This is NOT the claim's declared set and must never be confused with it. The
/// claim declares every authored wave; `check_claims` judges only the bodies that
/// exist, so an unplaceable wave is silently fine while a placed one the loop
/// skipped is a breach. Deriving `declared` from this function instead would be
/// the emitted set defining its own obligation — the exact shape the claim
/// machinery exists to prevent.
pub(super) fn wave_machinery_waves<'a>(
    plan: &'a Plan,
    wave_placements: &WavePlacements,
) -> impl Iterator<Item = &'a delvewright_dsl::Wave> {
    plan.campaign
        .quests
        .content
        .waves
        .iter()
        .filter(move |w| wave_placements.contains_key(w.id.as_str()) && plan::wave_total(w) >= 1)
}

/// Every wave's spawn function, its lane and kill-reward functions, and its `on_kill`.
pub(super) fn wave_fns(
    plan: &Plan,
    wave_placements: &WavePlacements,
    lane_routes: &crate::compiler::nav::LaneRoutes,
    item_combat: &crate::compiler::registry::ItemCombatRegistry,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    for w in &c.quests.content.waves {
        // Compiler-validated standable spawn cells near the wave anchor, in the
        // anchor's own room, one per mob. A wave whose spawn anchor
        // resolves in no assembled area gets no entry here and is skipped exactly
        // as before — DW0310 (check_wave_spawns) catches a dangling spawn-wave.
        let Some(cells) = wave_placements.get(w.id.as_str()) else {
            continue;
        };
        let mut body: Vec<String> = Vec::new();
        // spec-0016 §1: mark the wave as seated, so a bonfire rest only re-seats
        // waves the party has actually met. Emitted only for a `respawns_on_rest`
        // wave — every other campaign's `spawn_<wave>` is byte-identical.
        if w.respawns_on_rest {
            body.push(format!(
                "scoreboard players set {} dw.sys 1",
                wave_seated_holder(w.id.as_str())
            ));
        }
        body.push(format!(
            "scoreboard players set {} {} {}",
            plan::wave_counter(w.id.as_str()),
            plan::WAVE_OBJECTIVE,
            plan::wave_total(w)
        ));
        // A fresh seating starts a fresh attribution ([`wave_credited_holder`]):
        // the credited-kill ledger is about THIS cohort, so a re-seat's own
        // `kill @e[tag=…]` sweep — which credits nobody — must not be carried into
        // the count of what the party felled afterwards.
        body.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_credited_holder(w.id.as_str())
        ));
        // spec-0016 §6: a lane wave spawns as a Raider PATROL SQUAD — one leader,
        // everyone `Patrolling:1b` and pointed at the first proven waypoint. The
        // squad's own march clock starts with it. Empty for every other wave, so
        // pre-§6 `spawn_<wave>` output is byte-identical.
        let lane = w.lane.as_ref().zip(lane_routes.get(w.id.as_str()));
        let mut idx = 0i32;
        for (k, mob) in w.mobs.iter().enumerate() {
            // CustomName as a plain SNBT text component (M2 fix 1). Waves are
            // v0.3-only, so no v0.2 byte-identity concern.
            let name = match &mob.name {
                Some(n) => format!(",CustomName:{},CustomNameVisible:1b", snbt_component(n)),
                None => String::new(),
            };
            // Equipment: v0.6 explicit slots merged over the armed-mob default
            // (M2 fix 5: a summoned wither_skeleton/skeleton otherwise had no
            // weapon and was trivial). Every slot the v0.9 `drops[]` list does
            // not name keeps drop chance 0 — rank-and-file gear is never
            // lootable.
            let equip = wave_equipment(&mob.entity, mob.equipment.as_ref(), &mob.drops)
                .map(|e| format!(",{e}"))
                .unwrap_or_default();
            // v0.9: a declared quest-item drop rides the mob's own
            // death loot table. Absent on every other mob, so a wave that
            // declares no item drop keeps vanilla's own table and its exact
            // pre-0.9 summon string.
            let loot = if has_item_drop(&mob.drops) {
                format!(
                    ",DeathLootTable:\"{}\"",
                    death_loot_table(
                        ns,
                        Some(drop_loot_path("wave", &format!("{}-{k}", w.id.as_str()))),
                    )
                )
            } else {
                String::new()
            };
            // v0.4 attribute overrides (spec-0008 §4), emitted as 1.21.11
            // attribute components in the summon NBT. Empty for a plain mob. A
            // lane mob's `follow_range` is FORCED to the lane's `aggro_radius`:
            // release radius and perception radius must be the same number, or a
            // patrolling raider targets a player it cannot engage and holds
            // ground mid-lane (`DW0381` rejects a contradicting override).
            let effective_attrs = match lane {
                Some((l, _)) => Some(lane_attributes(mob.attributes, l.aggro_radius)),
                None => mob.attributes,
            };
            let attrs = attributes_snbt(effective_attrs.as_ref());
            // v0.4 permanent ambient effects: applied to this stack via a temp tag
            // after summon, so they land on exactly this mob type (not the whole
            // wave). Empty for a plain mob.
            let has_effects = !mob.effects.is_empty();
            let tmp = if has_effects { ",\"dw_tmp\"" } else { "" };
            for _ in 0..mob.count {
                // Each mob takes the next validated standable cell (ascending BFS
                // distance from the anchor); `cells` has exactly one per mob. AI is
                // left enabled (no NoAI) so the mobs fight.
                let cell = cells[idx as usize];
                let c = ent_xyz(cell);
                // spec-0016 §6 patrol NBT. `patrol_target` is the **snake_case
                // int-array** form and nothing else: 1.21.11's strict codec
                // silently DROPS the legacy `PatrolTarget:{X,Y,Z}` compound, and
                // the squad then patrols to vanilla-rolled random points — the
                // working-but-drunk failure the spike caught live. `Patrolling`
                // and `PatrolLeader` keep their camelCase names.
                let patrol = match lane {
                    Some((_, wps)) => {
                        let t = wps[0];
                        let leader = if idx == 0 { ",PatrolLeader:1b" } else { "" };
                        format!(
                            ",Patrolling:1b{leader},patrol_target:[I;{},{},{}]",
                            t[0], t[1], t[2]
                        )
                    }
                    None => String::new(),
                };
                let lead_tag = match lane {
                    Some(_) if idx == 0 => {
                        format!(",\"{}\"", lane_leader_tag(w.id.as_str()))
                    }
                    _ => String::new(),
                };
                body.push(format!(
                    "summon {} {} {} {} {{Tags:[\"{}\"{lead_tag}{tmp}],PersistenceRequired:1b{name}{equip}{loot}{attrs}{patrol}}}",
                    mob.entity,
                    c[0],
                    c[1],
                    c[2],
                    plan::wave_tag(w.id.as_str())
                ));
                idx += 1;
            }
            if has_effects {
                for eff in &mob.effects {
                    body.push(format!(
                        "effect give @e[tag=dw_tmp] {} infinite {} true",
                        eff.effect, eff.amplifier
                    ));
                }
                body.push("tag @e[tag=dw_tmp] remove dw_tmp".to_string());
            }
        }
        // spec-0016 §6: the squad marches from waypoint 0 and its clock starts
        // with it. `schedule … <n>t` is replace-mode, so a re-seat (spec-0016 §1)
        // can never double the clock up.
        if let Some((_, _)) = lane {
            let safe = plan::safe_local(w.id.as_str());
            body.push(format!(
                "scoreboard players set {} dw.sys 0",
                lane_index_holder(w.id.as_str())
            ));
            body.push(format!(
                "schedule function {ns}:lane_tick_{safe} {LANE_PERIOD_TICKS}t"
            ));
        }
        // spec-0073: the bar's max follows the bodies this function just put in
        // the world. Absent without a bar → byte-identical.
        if let Some(bar) = crate::compiler::healthbar::wave_bar(c, w.id.as_str()) {
            body.push(bar.capture_call(ns));
        }
        fns.push((
            format!("spawn_{}", plan::safe_local(w.id.as_str())),
            lines(&body),
        ));
        if let Some((l, wps)) = lane {
            fns.push(lane_tick_fn(ns, w, l, wps, &observer_guard(plan)));
        }
        // spec-0016 §1: the re-seat — clear survivors, then re-run the wave's own
        // spawn (same authored composition, same proven cells). Emitted for a
        // `respawns_on_rest` wave and for a billed elite/boss wave (whose rest
        // dispatch is guarded on the wave still standing — the undefeated
        // refresh), and for nothing else → byte-identical.
        if w.respawns_on_rest || plan.undefeated_reseat_waves().iter().any(|u| u.id == w.id) {
            let safe = plan::safe_local(w.id.as_str());
            // The standing bodies leave unseen through [`removal_lines`]: a
            // re-seat is a reset, not a death the party watches at the fire, and
            // not a kill the party earned — a declared drop is stripped first.
            let mut reseat = removal_lines(
                ns,
                &plan::wave_tag(w.id.as_str()),
                wave_declares_drops(w),
                Exit::Unseen,
            );
            reseat.push(format!("function {ns}:spawn_{safe}"));
            fns.push((format!("wave_reseat_{safe}"), lines(&reseat)));
        }
        // --- The wave CENSUS probe surface ---
        //
        // The live ladder used to answer "what is standing at this encounter?" by
        // silhouette: every entity mineflayer tracked, no distance filter, any mob
        // taller than half a block. On the drowned bell that counted five ambush
        // actors and a neighbouring wave as members of whichever wave was being
        // measured, and — since they were alive on both sides of a scripted death
        // — reported them as survivors the re-seat had failed to remove.
        // The wave tag is the only exact answer to that question and the compiler
        // owns it, so the compiler owns the census too: the harness asks these
        // functions and reads numbers, instead of guessing from shapes.
        //
        // Emitted for EVERY wave — the probe is how the ladder counts any
        // encounter, not only a re-seating one. A campaign with no waves emits
        // nothing here and is byte-identical.
        {
            let safe = plan::safe_local(w.id.as_str());
            let tag = plan::wave_tag(w.id.as_str());
            let brand = plan::wave_brand_tag(w.id.as_str());
            let wid = w.id.as_str();
            // Brand / unbrand: stamp this life's mobs, and clear the stamp. The
            // unbrand selects the BRAND, not the wave, so a mob that somehow
            // outlived its wave tag still gets cleaned up.
            fns.push((
                format!("wave_brand_{safe}"),
                lines(&[format!("tag @e[tag={tag}] add {brand}")]),
            ));
            fns.push((
                format!("wave_unbrand_{safe}"),
                lines(&[format!("tag @e[tag={brand}] remove {brand}")]),
            ));
            // Per-mob accumulation, run `as` each tagged mob. Health and its
            // maximum both come from vanilla primitives — `data get entity @s
            // Health` and `attribute @s max_health get` — so "damaged" is a fact
            // the server states, never a table the compiler invents (DW0475) and
            // never a value the client happened to be sent (a live 1.21.11 server
            // does not put an unmodified max health on the wire at all, which is
            // why the silhouette probe had to guess it from the highest health it
            // had ever seen).
            //
            // Scale 100: two decimal places carried as integers, so positions and
            // health cross the chat channel exactly, with no float formatting to
            // parse. The holders are shared across waves, which is safe because a
            // census is one atomic function call.
            fns.push((
                format!("wave_census_one_{safe}"),
                lines(&[
                    "scoreboard players add #wcen_n dw.sys 1".to_string(),
                    format!(
                        "execute if entity @s[tag={brand}] run scoreboard players add #wcen_b \
                         dw.sys 1"
                    ),
                    "execute store result score #wcen_h dw.sys run data get entity @s Health 100"
                        .to_string(),
                    "execute store result score #wcen_m dw.sys run attribute @s \
                     minecraft:max_health get 100"
                        .to_string(),
                    "execute if score #wcen_h dw.sys < #wcen_m dw.sys run scoreboard players add \
                     #wcen_d dw.sys 1"
                        .to_string(),
                    "execute store result score #wcen_x dw.sys run data get entity @s Pos[0] 100"
                        .to_string(),
                    "execute store result score #wcen_y dw.sys run data get entity @s Pos[1] 100"
                        .to_string(),
                    "execute store result score #wcen_z dw.sys run data get entity @s Pos[2] 100"
                        .to_string(),
                    format!("tellraw @a {}", census_mob_component(ns, wid)),
                ]),
            ));
            // The census itself: zero the accumulators, walk the tag, then state
            // the totals. `#wcen_seq` counts censuses so the harness can tell this
            // answer from a stale one — it never has to write a delve score to ask
            // a question.
            fns.push((
                format!("wave_census_{safe}"),
                lines(&[
                    "scoreboard players add #wcen_seq dw.sys 1".to_string(),
                    "scoreboard players set #wcen_n dw.sys 0".to_string(),
                    "scoreboard players set #wcen_b dw.sys 0".to_string(),
                    "scoreboard players set #wcen_d dw.sys 0".to_string(),
                    format!("execute as @e[tag={tag}] run function {ns}:wave_census_one_{safe}"),
                    format!("tellraw @a {}", census_summary_component(ns, wid)),
                ]),
            ));
        }
        // --- The wave MUSTER probe, and the staged removal ---
        //
        // The census counts bodies; the muster READS them. Every number a wave
        // declares — health, damage, armour, the gear it wears, the name over its
        // head — is written into one `summon` line and, until this probe, never
        // looked at again: 1.21.11 has twice silently dropped a field this
        // compiler wrote (`HandItems`, the legacy `PatrolTarget`), and both times
        // the delve booted green over a body that was not what the document said.
        // `crate::compiler::muster` derives the questions from the same
        // resolution the summon is written from, so the probe cannot verify a
        // copy of the declaration instead of the emitted one.
        //
        // `wave_strike_*` and `wave_chip_*` ride with it: the ladder does not
        // fight, it reads the bodies and then removes them, attributed to the
        // party so the wiring the kill drives actually fires.
        {
            let m = crate::compiler::muster::muster(
                w,
                &|mob| {
                    wave_equipment_slots(&mob.entity, mob.equipment.as_ref())
                        .into_iter()
                        .map(|(slot, item, _)| (slot, item.to_string()))
                        .collect()
                },
                item_combat,
                &|name| snbt_component(name),
            );
            for (name, body) in crate::compiler::muster::functions(ns, &m) {
                fns.push((name, lines(&body)));
            }
        }
        // kill reward: each slain wave mob decrements the countdown, records that
        // a PLAYER was credited with the death ([`wave_credited_holder`]), then
        // re-arms. The advancement's trigger is `player_killed_entity` over this
        // wave's tag, so reaching here IS the credit — nothing else can.
        fns.push((
            format!("k_reward_{}", plan::safe_local(w.id.as_str())),
            lines(
                &[
                    format!(
                        "scoreboard players remove {} {} 1",
                        plan::wave_counter(w.id.as_str()),
                        plan::WAVE_OBJECTIVE
                    ),
                    format!(
                        "scoreboard players add {} dw.sys 1",
                        wave_credited_holder(w.id.as_str())
                    ),
                ]
                .into_iter()
                // spec-0074: the wave's `on_kill` pays between the ledger and the
                // re-arm, as the credited player. Absent → byte-identical.
                .chain(
                    w.on_kill
                        .as_ref()
                        .map(|ok| on_kill_call(ns, delvewright_dsl::Fight::Wave(w), ok)),
                )
                .chain(std::iter::once(format!(
                    "advancement revoke @s only {ns}:k_{}",
                    plan::safe_local(w.id.as_str())
                )))
                .collect::<Vec<_>>(),
            ),
        ));
        if let Some(ok) = &w.on_kill {
            let fight = delvewright_dsl::Fight::Wave(w);
            fns.push((on_kill_function(fight), on_kill_body(plan, fight, ok)));
        }
    }
    fns
}
