/// Default aggro radius for a wave mob with no declared `follow_range` — vanilla's
/// `generic.follow_range` default for the common hostiles (zombie, skeleton,
/// husk, pillager). Used by the optional-elite bypass lint when the author has
/// not tuned the attribute.
pub const DEFAULT_FOLLOW_RANGE: u32 = 16;

/// `DW0380`: every optional enemy must be walkable around.
///
/// A wave is **optional** when no `kill` objective on the critical path names it —
/// the party is never required to fight it. For each such wave, its mobs' aggro
/// spheres (the declared `follow_range`, else [`DEFAULT_FOLLOW_RANGE`]) are
/// forced solid around the wave anchor and the forced critical path is re-routed:
/// if a leg that routed before no longer does, every way forward runs through the
/// fight and "optional" is a lie.
fn optional_elite_lint(plan: &Plan, world: &World) -> Vec<Diagnostic> {
    use delvewright_dsl::Objective;
    let c = plan.campaign;
    let required: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| q.objectives.iter())
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => Some(wave.as_str()),
            _ => None,
        })
        .collect();
    let elites: Vec<(String, [i32; 3], i32)> = c
        .quests
        .content
        .waves
        .iter()
        .filter(|w| !required.contains(w.id.as_str()))
        .filter_map(|w| {
            let centre = crate::compiler::plan::point_any(&plan.anchors, w.anchor.as_str())?;
            let radius = w
                .mobs
                .iter()
                .filter_map(|m| m.attributes.and_then(|a| a.follow_range))
                .map(|r| r.max(0.0) as i32)
                .max()
                .unwrap_or(DEFAULT_FOLLOW_RANGE as i32);
            Some((w.id.as_str().to_string(), centre, radius))
        })
        .collect();
    verify_optional_elites(world, &elites, &critical_positions(plan))
}

/// The pure core of [`optional_elite_lint`] (unit-testable against a synthetic
/// [`World`]). Each elite is `(wave id, anchor cell, aggro radius)`.
fn verify_optional_elites(
    world: &World,
    elites: &[(String, [i32; 3], i32)],
    positions: &[VisitedPos],
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for (id, centre, radius) in elites {
        let r = *radius;
        let mut sphere: BTreeSet<[i32; 3]> = BTreeSet::new();
        for dx in -r..=r {
            for dy in -r..=r {
                for dz in -r..=r {
                    if dx * dx + dy * dy + dz * dz <= r * r {
                        sphere.insert([centre[0] + dx, centre[1] + dy, centre[2] + dz]);
                    }
                }
            }
        }
        let aggroed = world.with_sealed(&sphere);
        let blocked = positions.windows(2).find(|pair| {
            if pair[1].transport_before {
                return false;
            }
            let (Some(a), Some(b)) = (
                world.snap_endpoint(pair[0].pos, pair[0].talk_to),
                world.snap_endpoint(pair[1].pos, pair[1].talk_to),
            ) else {
                return false;
            };
            // A leg that goes nowhere, or that never routed in the clean world,
            // is not this lint's business (`DW0311` owns the latter).
            if a == b || world.find_path(a, b).is_none() {
                return false;
            }
            // An endpoint INSIDE the aggro sphere is not a missing bypass — the
            // party is required to stand there, so the fight is contested ground
            // by design (a "live threat" wave seated on an objective anchor is a
            // legitimate, landed pattern). This lint is about the ROUTE being
            // swallowed, not the destination being dangerous.
            if sphere.contains(&a) || sphere.contains(&b) {
                return false;
            }
            let (Some(a2), Some(b2)) = (
                aggroed.snap_endpoint(pair[0].pos, pair[0].talk_to),
                aggroed.snap_endpoint(pair[1].pos, pair[1].talk_to),
            ) else {
                return true;
            };
            aggroed.find_path(a2, b2).is_none()
        });
        if let Some(pair) = blocked {
            out.push(Diagnostic::warning(
                DW_OPTIONAL_ELITE_UNAVOIDABLE,
                "quests",
                format!("/content/waves/{id}"),
                format!(
                    "optional enemy `{id}` at {centre:?} has no bypass: with its {r}-block aggro \
                     radius blocked, the forced walk from {:?} to {:?} no longer routes — every \
                     way forward runs through the fight, so \"optional\" is a lie (spec-0016 §7). \
                     A powerful OPTIONAL enemy near the start is legitimate — the Tree Sentinel \
                     pattern — and this is its one obligation: the walk-around has to exist. \
                     Widen the room, move the wave off the corridor, or make the kill a real \
                     objective.",
                    pair[0].pos, pair[1].pos
                ),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {

    /// A non-talk-to visited position (test convenience for `route_visited`).
    /// A room `w × d` split by a wall at `z = zw` with a single doorway at
    /// `x = gx`, and (optionally) a second permanent opening at `x = bx` — the
    /// walk-around. Ceilinged, so a body standing in the doorway cannot be
    /// climbed over.
    fn two_room_world(w: i32, d: i32, y: i32, zw: i32, gx: i32, bypass: Option<i32>) -> World {
        let mut walls = Vec::new();
        for x in 0..w {
            if x == gx || Some(x) == bypass {
                continue;
            }
            walls.push([x, y, zw]);
            walls.push([x, y + 1, zw]);
        }
        floored(w, d, y, &walls)
    }

    /// A sentinel parked in the only doorway between two beats: with its aggro
    /// radius blocked, the forced walk no longer routes. There is nothing to
    /// walk around it by, so "optional" is a lie — `DW0380`, at warning tier.
    #[test]
    fn an_optional_elite_in_the_only_doorway_is_dw0380() {
        let world = two_room_world(12, 9, 65, 4, 6, None);
        let elites = vec![("wave/sentinel".to_string(), [6, 65, 4], 2)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert_eq!(diags.len(), 1, "expected one finding: {diags:#?}");
        assert_eq!(diags[0].code, DW_OPTIONAL_ELITE_UNAVOIDABLE); // DW0380
        assert_eq!(
            diags[0].severity,
            delvewright_dsl::Severity::Warning,
            "spec-0016 §7 is the design-contract section: this measures, it does not gate"
        );
        assert!(
            diags[0].message.contains("Tree Sentinel"),
            "the finding must not read as 'no optional enemies' — the pattern is legitimate, \
             only the missing walk-around is the problem: {}",
            diags[0].message
        );
    }

    /// The same sentinel with a second door far enough away to stay outside its
    /// aggro radius: the walk-around exists, so the Tree Sentinel pattern stands
    /// and nothing is reported.
    #[test]
    fn an_optional_elite_with_a_walk_around_is_legitimate() {
        let world = two_room_world(12, 9, 65, 4, 6, Some(0));
        let elites = vec![("wave/sentinel".to_string(), [6, 65, 4], 2)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert!(
            diags.is_empty(),
            "a route around the sentinel is all the engine asks for: {diags:#?}"
        );
    }

    /// A beat the party is required to STAND on, inside the aggro radius, is
    /// contested ground by design — a landed "live threat" pattern, not a missing
    /// bypass. The lint is about the route, never the destination.
    #[test]
    fn an_elite_seated_on_a_beat_is_contested_ground_not_a_missing_bypass() {
        let world = two_room_world(12, 9, 65, 4, 6, None);
        let elites = vec![("wave/threat".to_string(), [1, 65, 1], 4)];
        let diags = verify_optional_elites(
            &world,
            &elites,
            &[vp_at([1, 65, 1], 0), vp_at([1, 65, 7], 1)],
        );
        assert!(
            diags.is_empty(),
            "an objective inside the fight is design, not a defect: {diags:#?}"
        );
    }
}
