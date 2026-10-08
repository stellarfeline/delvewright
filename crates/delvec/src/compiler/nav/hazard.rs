// ---------------------------------------------------------------------------
// Hazard observability (spec-0016 §4 addendum, souls dossier §5.3 / §2.2 axis 5)
// ---------------------------------------------------------------------------

/// A player's eye height above the floor of the cell they stand in, in blocks —
/// the vanilla 1.21.11 standing eye offset, the same figure the player-POV camera
/// derivation uses (`DW0724`). The observability sightline starts here because
/// the question the proof asks is literally "can a player standing there see it".
const EYE_HEIGHT: f64 = 1.62;

/// The minimum distance a watch cell must keep from every cell of a hazard's
/// lethal span, in blocks (Chebyshev, box distance).
///
/// Derived rather than invented: it is **one second of sprinting** at the nav
/// model's own speed (`20 / SPRINT_TICKS_PER_BLOCK`), so the proof demands a
/// sightline from ground the player reaches a full second before the hazard could
/// have them. Sight from the very lip of the span is not observation from safety —
/// it is already the commitment. (The bell remake's portcullis bay sits six blocks
/// out, comfortably clear of this floor.)
const HAZARD_STANDOFF: i32 = (20 / SPRINT_TICKS_PER_BLOCK) as i32;

/// How far from a hazard the proof will look for a watch cell, in blocks
/// (Chebyshev, box distance). Two chunks: a bay further out than this is not a
/// bay, and the bound keeps the search over a box-garden world small and its cost
/// independent of how large the reachable region happens to be.
const HAZARD_WATCH_RANGE: i32 = 32;

/// One hazard the observability proof judges: a region whose contents become
/// lethal on a **clock the player is expected to read**.
///
/// Exactly two verbs qualify today. A `timed-gate` cycles open/closed forever; a
/// `volley` rakes its kill zone for `salvos × interval` ticks. Both ask the player
/// to time an entry, and both are therefore owed a sightline first. `collapse` is
/// deliberately NOT here: it fires once, its region is a ceiling with no standable
/// cell, and there is no cycle to watch — its fairness obligation is the
/// post-collapse completability proof (`DW0445`), not observability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimedHazard {
    /// How the diagnostic names it (`timed-gate/portcullis`, `volley into
    /// `anchor/stair-run``).
    pub id: String,
    /// The lethal span's inclusive corners, in absolute world coordinates.
    pub region: ([i32; 3], [i32; 3]),
}

/// Every timed hazard in the campaign, in deterministic content order:
/// `timed_gates[]` in declared order, then each distinct `volley` kill zone in
/// effect-traversal order. A volley declared twice with the same gallery slot and
/// the same zone is one hazard (the emitter dedupes it the same way).
pub fn timed_hazards(plan: &Plan) -> Vec<TimedHazard> {
    let mut out: Vec<TimedHazard> = plan
        .timed_gates
        .iter()
        .map(|g| TimedHazard {
            id: g.id.clone(),
            region: g.gate_region,
        })
        .collect();
    let mut seen: BTreeSet<([i32; 3], [i32; 3])> = BTreeSet::new();
    for eff in all_effects(plan) {
        let Some((_, from_anchor, kill_zone, _, _)) = eff.volley() else {
            continue;
        };
        let Some(region) = plan.zone_box(kill_zone) else {
            continue; // an unresolvable anchor is the payload planner's error
        };
        if !seen.insert(region) {
            continue;
        }
        out.push(TimedHazard {
            id: format!(
                "volley from `{from_anchor}` into `{}`",
                kill_zone.anchor.as_str()
            ),
            region,
        });
    }
    out
}

/// Prove every timed hazard can be **watched before it is committed to** —
/// [`DW_HAZARD_UNOBSERVABLE`] (`DW0388`).
///
/// For each hazard the proof asks for one **watch cell** `w` with all three of:
///
/// 1. **Outside the lethal span, by a margin.** `w` is standable and at least
///    [`HAZARD_STANDOFF`] blocks (box distance) from every cell of the span — one
///    second of sprint at the nav model's own speed. Sight from the lip of the
///    span is not observation from safety.
/// 2. **Line of sight.** The segment from `w`'s eye ([`EYE_HEIGHT`] above its
///    floor) to the player-centre-mass point of some standable hazard cell
///    ([`volley_target`], 1.0 above that cell's floor — the exact point a volley
///    aims at, so "the dangerous point of a hazard cell" has one definition in the
///    compiler) crosses no sight-blocking geometry. The predicate is the cutscene
///    clip check's `blocks_camera` walked by the same Amanatides–Woo
///    [`walk_cells`] traversal, so glass and grates are transparent to an eye
///    exactly as they are to a camera — a bay behind a grate is a bay.
/// 3. **Reached without committing.** `w` is walkable from the campaign entry over
///    the world with the span **sealed**. This is the load-bearing clause: it is
///    what makes the cell a *watch* cell rather than a cell you can only reach by
///    first surviving the hazard.
///
/// Tiering (spec-0016 §4 addendum): **error** for a campaign that declares a
/// `bonfire` — a souls campaign, where observe-before-commit is the fairness
/// contract the whole loop rests on — and **warning** everywhere else, where the
/// same geometry is a design note rather than a broken promise.
///
/// Hazards whose region holds no standable cell, and campaigns with no resolvable
/// entry, are left to the proofs that own them (`DW0444`, `DW0311`, `DW0345`)
/// rather than double-reported here.
pub fn check_hazard_observability(
    plan: &Plan,
    world: &World,
    entry: Option<[i32; 3]>,
) -> Result<Vec<Diagnostic>, Failure> {
    let hazards = timed_hazards(plan);
    let findings = verify_hazard_observability(world, &hazards, entry);
    // A campaign that places a bonfire IS a souls campaign — the same test the
    // flask obligation (`DW0476`) uses, so one campaign never sits on two
    // different answers to "is this spec-0016 content".
    hazard_tier(plan.bonfires().next().is_some(), findings)
}

/// Apply the spec-0016 §4-addendum tiering to the observability findings: a souls
/// campaign fails the build on the first one, anything else carries all of them as
/// advisory warnings. Split out from [`check_hazard_observability`] so the tier
/// rule itself is unit-testable without standing up a whole [`Plan`].
fn hazard_tier(souls: bool, findings: Vec<Diagnostic>) -> Result<Vec<Diagnostic>, Failure> {
    if !souls {
        return Ok(findings);
    }
    match findings.into_iter().next() {
        Some(d) => Err(Failure {
            code: DW_HAZARD_UNOBSERVABLE,
            message: d.message,
        }),
        None => Ok(Vec::new()),
    }
}

/// The pure core of [`check_hazard_observability`] (unit-testable against a
/// synthetic [`World`]). Reports at the advisory tier; [`hazard_tier`] decides how
/// loud that is for the campaign at hand.
fn verify_hazard_observability(
    world: &World,
    hazards: &[TimedHazard],
    entry: Option<[i32; 3]>,
) -> Vec<Diagnostic> {
    let Some(entry) = entry else {
        return Vec::new(); // DW0345 owns a campaign with no entry anchor
    };
    let mut out = Vec::new();
    for h in hazards {
        let span: BTreeSet<[i32; 3]> =
            crate::compiler::assembled::region_cells(h.region.0, h.region.1).collect();
        // What the player would be standing on inside the hazard — the cells the
        // clock actually judges, and so the cells a watcher must be able to see.
        let samples: Vec<[i32; 3]> = span
            .iter()
            .copied()
            .filter(|c| world.standable(*c))
            .collect();
        if samples.is_empty() {
            continue; // an unusable region is DW0444 / DW0311's business
        }
        // Pre-commitment ground: everywhere the player can walk from the entry
        // WITHOUT entering the span.
        let sealed = world.with_sealed(&span);
        let pre_commit = sealed.reachable_walkable(&[entry]);
        // Nearest candidate first — a real watch bay is close, so the passing case
        // costs a handful of sightlines. Ties break on cell order (ADR-0006).
        let mut candidates: Vec<(i32, [i32; 3])> = pre_commit
            .into_iter()
            .map(|c| (box_distance(c, h.region), c))
            .filter(|(d, _)| (HAZARD_STANDOFF..=HAZARD_WATCH_RANGE).contains(d))
            .collect();
        candidates.sort_unstable();
        let watch = candidates
            .into_iter()
            .find(|(_, c)| samples.iter().any(|s| sees_hazard_cell(world, *c, *s)));
        if watch.is_some() {
            continue;
        }
        out.push(Diagnostic::warning(
            DW_HAZARD_UNOBSERVABLE,
            "quests",
            format!("/content/quests/hazard/{}", h.id),
            format!(
                "hazard `{}` cannot be watched before it is committed to: no standable cell \
                 within {HAZARD_WATCH_RANGE} blocks of its span [{}, {}, {}]..[{}, {}, {}] is \
                 both at least {HAZARD_STANDOFF} blocks clear of it (one second of sprint at \
                 {SPRINT_TICKS_PER_BLOCK} t/block) and walkable from the campaign entry without \
                 entering it, with line of sight to any cell the hazard judges. The strongest \
                 rule in the souls vocabulary is observe-from-safety-before-commit (spec-0016 §4 \
                 addendum): you can stand outside Sen's Fortress and watch a blade cycle, and you \
                 cannot see inside the Capra room — a timed hazard you meet blind is a coin flip \
                 no repetition teaches, whatever its duty cycle. Give it a watch bay: open the \
                 approach so the span is visible from a few blocks back, or move the hazard off \
                 the blind side of the corner. Do NOT shorten the standoff.",
                h.id,
                h.region.0[0],
                h.region.0[1],
                h.region.0[2],
                h.region.1[0],
                h.region.1[1],
                h.region.1[2],
            ),
        ));
    }
    out
}

/// Chebyshev distance from a cell to a box, in blocks — `0` inside the box.
/// Integer arithmetic end to end: a proof never rounds (ADR-0006).
fn box_distance(c: [i32; 3], region: ([i32; 3], [i32; 3])) -> i32 {
    let (lo, hi) = region;
    (0..3)
        .map(|i| {
            let (a, b) = (lo[i].min(hi[i]), lo[i].max(hi[i]));
            (a - c[i]).max(c[i] - b).max(0)
        })
        .max()
        .unwrap_or(0)
}

/// Whether a player standing on `watch` can see the space a player standing on
/// `hazard` would occupy: the segment from eye height over one to centre mass over
/// the other, walked cell by cell through the **sight** predicate. Both endpoint
/// cells are exempt — they are the observer's own head and the target volume, both
/// standable and so both passable by construction.
fn sees_hazard_cell(world: &World, watch: [i32; 3], hazard: [i32; 3]) -> bool {
    let eye = [
        watch[0] as f64 + 0.5,
        watch[1] as f64 + EYE_HEIGHT,
        watch[2] as f64 + 0.5,
    ];
    let target = volley_target(hazard);
    let eye_cell = [watch[0], watch[1] + 1, watch[2]];
    walk_cells(eye, target, |c| {
        c != eye_cell && c != hazard && world.blocks_camera(c)
    })
    .is_none()
}

#[cfg(test)]
mod tests {

    /// A one-wide run of open columns along x at a fixed z.
    fn run_x(z: i32, x0: i32, x1: i32) -> Vec<[i32; 2]> {
        (x0..=x1).map(|x| [x, z]).collect()
    }

    /// A gate-shaped hazard: the full-height column at `(x, z)`.
    fn hazard(x: i32, z: i32) -> TimedHazard {
        TimedHazard {
            id: "timed-gate/portcullis".to_string(),
            region: ([x, WY, z], [x, WY + 1, z]),
        }
    }

    /// A straight hall with the portcullis at the far end: every cell of the
    /// approach looks right down the barrel of it, so the player can stand a good
    /// way back and watch a whole cycle before stepping in. This is the shape the
    /// dossier calls fair (§5.3 rule 1) and the proof must let it through.
    #[test]
    fn hazard_at_the_end_of_a_straight_hall_is_observable() {
        let world = carved(&run_z(0, 0, 12));
        let found = verify_hazard_observability(&world, &[hazard(0, 12)], Some([0, WY, 0]));
        assert!(
            found.is_empty(),
            "a hall you can see down is the observable case: {found:#?}"
        );
    }

    /// The seeded violation: the same portcullis put four blocks around a blind
    /// corner. Every cell far enough back to be safety is in the other leg of the
    /// L and sees rock; every cell that sees the gate is already inside the
    /// commitment radius. The Capra door — you meet the hazard for the first time
    /// with no read available. `DW0388`.
    #[test]
    fn hazard_around_a_blind_corner_is_dw0388() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        assert_eq!(found.len(), 1, "the blind corner is reported: {found:#?}");
        assert_eq!(found[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        assert!(
            found[0].message.contains("cannot be watched"),
            "the message must name the failure: {}",
            found[0].message
        );
    }

    /// The same blind corner with a watch bay: the corner leg is continued PAST
    /// the junction, so the approach opens onto ground that stands off the gate and
    /// looks straight down the hall at it. This is the bell remake's "roofed bay
    /// six blocks out" (REMAKE §7.4 entry O), and it is the fix the diagnostic
    /// prescribes — the geometry changes, never the floor.
    #[test]
    fn a_watch_bay_off_the_approach_restores_observability() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, -6, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        assert!(
            found.is_empty(),
            "a bay with a sightline is exactly what the proof asks for: {found:#?}"
        );
    }

    /// The load-bearing clause: the sightline must be reachable WITHOUT entering
    /// the hazard. One hall, one gate, one long clear view of it — but the view is
    /// all on the far side, and the near approach is too short to stand off in. A
    /// bay you can only reach by first surviving the gate is not a watch bay; the
    /// identical world entered from the far end passes, which is the whole
    /// difference.
    #[test]
    fn a_sightline_only_reachable_through_the_hazard_does_not_count() {
        let world = carved(&run_z(0, 0, 20));
        let h = [hazard(0, 4)];
        let blind = verify_hazard_observability(&world, &h, Some([0, WY, 0]));
        assert_eq!(blind.len(), 1, "entered from the short side: {blind:#?}");
        assert_eq!(blind[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        let seen = verify_hazard_observability(&world, &h, Some([0, WY, 20]));
        assert!(
            seen.is_empty(),
            "the same geometry entered from the long side is observable: {seen:#?}"
        );
    }

    /// Tiering (spec-0016 §4 addendum): the same finding fails the build for a
    /// campaign that declares a `bonfire` — souls content, where
    /// observe-before-commit is the contract the retry loop rests on — and is
    /// advisory everywhere else.
    #[test]
    fn hazard_observability_is_error_tier_only_for_souls_campaigns() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        let found = verify_hazard_observability(&world, &[hazard(4, 8)], Some([0, WY, 0]));
        let warned = hazard_tier(false, found.clone()).expect("non-souls stays advisory");
        assert_eq!(warned.len(), 1);
        assert_eq!(warned[0].code, DW_HAZARD_UNOBSERVABLE); // DW0388
        assert_eq!(warned[0].severity, delvewright_dsl::Severity::Warning);
        let err = hazard_tier(true, found).expect_err("a souls campaign fails the build");
        assert_eq!(err.code, DW_HAZARD_UNOBSERVABLE); // DW0388
    }

    /// A campaign with no resolvable entry anchor raises nothing here: `DW0345`
    /// owns that failure, and a proof that piles a second diagnostic on the same
    /// root cause sends the author chasing the wrong fix.
    #[test]
    fn no_campaign_entry_leaves_the_hazard_to_dw0345() {
        let mut open = run_z(0, 0, 8);
        open.extend(run_x(8, 0, 4));
        let world = carved(&open);
        assert!(verify_hazard_observability(&world, &[hazard(4, 8)], None).is_empty());
    }

    /// Box distance is a Chebyshev reach to the span, zero inside it, and is what
    /// both the standoff floor and the search bound are measured in.
    #[test]
    fn box_distance_is_chebyshev_to_the_span() {
        let region = ([0, 65, 0], [2, 67, 2]);
        assert_eq!(box_distance([1, 66, 1], region), 0);
        assert_eq!(box_distance([-5, 66, 1], region), 5);
        assert_eq!(box_distance([7, 66, 4], region), 5);
        assert_eq!(box_distance([0, 72, 0], region), 5);
    }
}
