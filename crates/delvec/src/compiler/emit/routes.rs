//! The world block: every proof `build_with_warnings` takes over the assembled
//! world, and the routes, placements and ledgers it hands on.

use super::*;

/// What the world block proves and hands on: the routes and placements every
/// emitter reads, and the binding ledgers the build writes under
/// `validation/`.
pub(super) struct WorldProofs {
    pub(super) moves: Vec<crate::compiler::nav::MovePlan>,
    pub(super) actor_moves: Vec<crate::compiler::nav::ActorMovePlan>,
    pub(super) wave_placements: WavePlacements,
    pub(super) wave_rings: WaveRings,
    pub(super) lane_routes: crate::compiler::nav::LaneRoutes,
    pub(super) payload_plans: PayloadPlans,
    pub(super) pov_shots: Vec<crate::compiler::render_plan::PovShot>,
    pub(super) branch_waypoints: Vec<(String, Value)>,
    pub(super) branch_takes: BTreeMap<String, plan::LinkTakes>,
    pub(super) legs_carried: usize,
    pub(super) path_legs: Vec<(String, Vec<plan::Step>, Vec<crate::compiler::nav::LegRoute>)>,
    pub(super) traversal_gate: Option<crate::compiler::traversal::TraversalGate>,
    pub(super) gate_seal_ledger: Option<serde_json::Value>,
    pub(super) fluid_escape_ledger: Option<serde_json::Value>,
    pub(super) sea_seepage_ledger: Option<serde_json::Value>,
    pub(super) piece_exposure_ledger: Option<serde_json::Value>,
    pub(super) lethal_gate: Option<crate::compiler::lethal::LethalGate>,
    pub(super) loop_gate: Option<crate::compiler::r#loop::LoopBinding>,
    pub(super) firework_gate: Option<crate::compiler::firework::FireworkGate>,
    pub(super) cutscene_shots_judged: usize,
    pub(super) lightning_gate: Option<crate::compiler::lightning::LightningGate>,
    pub(super) stake_table: Option<crate::compiler::stake::StakeTable>,
    pub(super) death_plan: Option<Value>,
    pub(super) pulse_gate: Option<crate::compiler::pulse::PulseGate>,
}

/// The proofs over the assembled world — walk, waves, lanes, branches,
/// traversal, fluids, lethal volumes, loops, fireworks, strikes, stakes, the
/// death plan, payloads — in the order the build has always taken them, and
/// the ledgers they fill. A campaign that assembles no world takes none of
/// them and gets empty routes.
#[allow(clippy::too_many_arguments)]
pub(super) fn prove_world(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    assembled: &crate::compiler::assembled::Assembled,
    prefabs: &crate::compiler::registry::PrefabRegistry,
    structures: &BTreeMap<String, Vec<u8>>,
    relight: &crate::compiler::light::Relight,
    out: &mut BuildOutput,
    warnings: &mut Vec<delvewright_dsl::Diagnostic>,
    pacing: &mut Vec<delvewright_dsl::Diagnostic>,
) -> Result<WorldProofs, BuildFailure> {
    // Visual-tier player-POV shots (spec-0003): first-person cameras along the
    // proven critical-path routes. Filled inside the world block below (they need
    // the routes); empty for a campaign with no walked leg, so its render plan
    // stays byte-identical.
    let mut pov_shots: Vec<crate::compiler::render_plan::PovShot> = Vec::new();
    // spec-0025 per-branch waypoint artifacts: one
    // `validation/branch-waypoints-<branch>.json` per reachable branch, filled
    // inside the world block below (they need the assembled occupancy) and
    // emitted alongside the branch paths. Empty for a campaign with no declared
    // branch points, so nothing moves for anybody who has not opted in.
    let mut branch_waypoints: Vec<(String, Value)> = Vec::new();
    // Per branch, the links its own path takes (spec-0083 §6), decided over the
    // world below and spliced again where `branch-path-<slug>.json` is written,
    // so the bot walks the steps the branch proof judged.
    let mut branch_takes: BTreeMap<String, plan::LinkTakes> = BTreeMap::new();
    // Legs the default path crosses by a link — `teleport-gate.json`'s
    // `legs_carried`.
    let mut legs_carried = 0usize;
    // Every exported path's steps and proven routes — the exported path and each
    // reachable branch's — kept for the run-back finder, which needs the seated
    // hostiles and the lanes that are only resolved further down.
    let mut path_legs: Vec<(String, Vec<plan::Step>, Vec<crate::compiler::nav::LegRoute>)> =
        Vec::new();
    // The traversal proof's binding ledger (`compiler::traversal`), filled inside
    // the world block below. `None` for a campaign that assembles no world —
    // which is not the same fact as "examined nothing", so the artifact is
    // omitted entirely rather than emitted claiming a zero it never measured.
    let mut traversal_gate: Option<crate::compiler::traversal::TraversalGate> = None;
    // The world-load gate ledger (`compiler::assembled`,
    // playtest-methodology.md rule 1): what the completability model measured
    // about every gate the layout resolved, and how many of them it treats as
    // shut. `None` for a campaign whose layout resolves no gate anchor, so a file
    // that exists and reports `"modelled_as_sealed": 0` is a finding rather than
    // an absence.
    let mut gate_seal_ledger: Option<serde_json::Value> = None;
    // The fluid-escape proof's binding ledger (`compiler::nav`, `DW0318`): how
    // much water the assembled world holds and how much of it ended up outside
    // every placed piece, stated against the horizon. Filled by every campaign
    // that assembles a world — including a bone-dry one, which then ships a
    // ledger reading zero rather than nothing at all.
    let mut fluid_escape_ledger: Option<serde_json::Value> = None;
    // The sea-seepage proof's binding ledger (`compiler::nav`, `DW0851`): how much
    // open face the built volume presents to the ambient sea, how far the sea gets
    // in, and how much of the walk region it covers. Filled by every campaign that
    // assembles a world — a `horizon: void` one included, which then ships a
    // ledger saying `"horizon": "void"` and zeroes, rather than nothing at all.
    let mut sea_seepage_ledger: Option<serde_json::Value> = None;
    // The piece-exposure binding ledger (`compiler::burial`, `DW0885`): how much
    // solid boundary the placed pieces put on their own boxes, how much of it
    // has nothing in front of it, and how much of THAT stands in air the party
    // can be in. Filled by every campaign that assembles a world, zeroes
    // included — a build whose party never gets outdoors ships a ledger saying
    // so, which is a different artifact from one that never ran the question.
    let mut piece_exposure_ledger: Option<serde_json::Value> = None;
    // The lethal-volume proofs' binding ledger (`compiler::lethal`), filled inside
    // the world block below. `None` for a campaign that declares no volume — no
    // ledger, no artifact, no byte moved for anybody who has not opted in.
    let mut lethal_gate: Option<crate::compiler::lethal::LethalGate> = None;
    // The loop proofs' binding ledger (`compiler::loop`, spec-0086 §8). `None`
    // for a campaign that declares no loop — no line, no artifact; a ledger that
    // exists and reports zero is a finding.
    let mut loop_gate: Option<crate::compiler::r#loop::LoopBinding> = None;
    // The firework proofs' binding ledger (`compiler::firework`, spec-0068 §5),
    // filled inside the world block below. `None` for a campaign that declares no
    // firework — no ledger, no artifact, no byte moved for anybody who has not
    // opted in; a ledger that exists and reports zero columns is a finding.
    let mut firework_gate: Option<crate::compiler::firework::FireworkGate> = None;
    // spec-0091: how many cutscene shots were judged against the served view
    // distance, for the binding line printed beside the render plan.
    let mut cutscene_shots_judged = 0usize;
    // The strike proofs' binding ledger (`compiler::lightning`, spec-0092 §5),
    // filled beside the firework's: `None` for a campaign that declares no
    // strike, so nobody who has not opted in moves a byte.
    let mut lightning_gate: Option<crate::compiler::lightning::LightningGate> = None;
    // The recovery stake's compile-time placement table (`compiler::stake`), and
    // the ledger of what its proofs looked at. `None` for a campaign that declares
    // no stake, which is the whole feature's byte-identity guarantee: no table, no
    // objectives, no functions, no artifact.
    let mut stake_table: Option<crate::compiler::stake::StakeTable> = None;
    // The bot tier's contract for DYING (`compiler::deathplan`): the lethal volumes
    // it may walk into, the wording each promises, the `on_death` consequences, the
    // stake rules and the placement table's rows. `None` for a campaign that
    // declares none of the three, and for one that assembles no world — a
    // contract nobody can walk is not the same fact as an empty one.
    let mut death_plan: Option<Value> = None;
    // The pulse proofs' binding (`compiler::pulse`, spec-0102 §5.1), measured
    // over the forced route's legs below. `None` for a campaign that assembles
    // no world; the caller prints the line either way.
    let mut pulse_gate: Option<crate::compiler::pulse::PulseGate> = None;

    let (moves, actor_moves, wave_placements, wave_rings, lane_routes, payload_plans): (
        Vec<crate::compiler::nav::MovePlan>,
        Vec<crate::compiler::nav::ActorMovePlan>,
        WavePlacements,
        WaveRings,
        crate::compiler::nav::LaneRoutes,
        PayloadPlans,
    ) = if assembles_world(plan) {
        {
            // spec-0022 payload verbs need the block map (a `collapse` settles
            // real blocks), not just the occupancy view.
            let blocks: &crate::compiler::blockstate::BlockMap = &assembled.blocks;
            if world.has_gate_anchors() {
                gate_seal_ledger = Some(world.gate_seal_ledger());
            }

            // DW0322 over the FINISHED world, for every campaign that assembles
            // one.
            //
            // This proof had exactly one call site: inside the stage-7 edit
            // replay, once per batch. Stage 8 is skipped entirely for a campaign
            // with no edits — so a campaign that only places pieces never had its
            // boundary proven at all, and could ship a reachable walkable cell
            // one step from a void drop. The guarantee is a property of the
            // ASSEMBLED WORLD, not of having edited it; keying it to the edit
            // script bound it to the wrong thing.
            //
            // The per-batch call stays: it names WHICH batch broke the boundary,
            // which this one cannot. This is the floor under both — run last,
            // over the world that actually ships.
            //
            // ERROR TIER, unwindowed. A
            // reachable walkable cell one step from a bottomless column is a
            // player who leaves the world; that is not a style note the author
            // may carry for a version, so there is no deprecation window and the
            // message offers none. The per-batch call inside the edit replay is
            // an error for the same reason and stays one — it additionally names
            // WHICH batch broke the boundary, which this floor cannot.
            //
            // The fix is always GEOMETRY or the world-generator premise, never a
            // declaration: `Ambient` (spec-0013 `horizon`) states what an
            // unmodelled column contains, and `boundary`'s return clock is a
            // runtime rescue that this proof deliberately does not read — being
            // teleported back after falling out is not the guarantee.
            //
            // The walk region this proof examines is rooted at every resolved
            // anchor, SEATED INSIDE THE PIECE THAT DECLARES IT
            // (`crate::compiler::nav::AnchorRoot`). That confinement is load-bearing here
            // and was the finding this call site was born with: an unconfined
            // nearest-standable snap ignores solid geometry, so an anchor a
            // `collapse` payload must declare in the ceiling snapped UP through
            // it onto the room's ROOF — and this proof then demanded a safe edge
            // on a bare platform in a void world, which no free-standing prefab
            // can satisfy. Five `v06_trap_payloads` fixtures were red for exactly
            // that, while their twelve siblings — identical geometry, anchors one
            // block lower — were green, which is what proved the interior walk
            // region boundary-safe and the roof a disconnected component.
            // `DW0318` over the finished world: fluid that ran out of the built
            // volume. It runs BEFORE the boundary proof, and the order is
            // load-bearing. Two independent reasons, and the second is why this
            // is not a style choice:
            //
            // 1. The boundary proof cannot see this at all. It examines
            //    reachable WALKABLE cells, and a flooded cell is impassable and
            //    never floor, so escaped water is in neither the reachable set
            //    nor its neighbour scan, under either horizon.
            // 2. Worse, escaped water makes the boundary proof LIE. Its
            //    per-column fall-arrest scan (`nav::boundary_void`'s `col_min`)
            //    counts a flooded cell as arrest, so a bottomless column with a
            //    waterfall running down it reads as supported — and the proof
            //    goes quiet on precisely the columns the water escaped through.
            //    Measured on the shipped `island-beach-camp` piece placed under
            //    `horizon: void`: 9792 escaped cells, and `DW0322` silent.
            //
            // So escaping fluid is a false premise of the proof that runs next,
            // exactly as an unsettled gravity block is a false premise of
            // everything downstream of `DW0313`. Clear it first.
            //
            // The ledger is measured unconditionally and emitted below, even
            // when the verdict is a pass and even when the world holds no water
            // at all: "0 fluid cells examined" is the reading a dry campaign
            // should be able to show, and a check whose only output is silence
            // is one nobody can tell apart from a check that did not run.
            // Measured here for the LEDGER, which is owed even on a pass and
            // even by a bone-dry world. The sequencing itself is no longer this
            // call site's to hold: `verify_boundary_safety` runs the same proof
            // first, because a boundary verdict taken over a world the water is
            // still running out of is not a verdict (see its doc comment). This
            // call and that one cannot disagree — the measurement is pure and
            // reads the same two sets.
            let fluid_escape = crate::compiler::nav::measure_fluid_escape(world);
            fluid_escape_ledger = Some(fluid_escape.ledger());
            if let Some(e) = fluid_escape.finding() {
                return Err(BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                });
            }

            // Measured here for the LEDGER only, on the same terms as the fluid
            // escape above: the refusal itself lives inside
            // `verify_boundary_safety`, which owns the sequence, and this call
            // cannot disagree with it because the measurement is pure and reads
            // the same sets. A pass owes the numbers as much as a failure does —
            // `contact_face_cells: 0` is a watertight hull saying so.
            let party_walk =
                world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan));
            let seepage = crate::compiler::nav::measure_sea_seepage(world, &party_walk);
            eprintln!("{}", seepage.line());
            sea_seepage_ledger = Some(seepage.ledger());

            // **`DW0344`, second arm: no walk cell of a placed piece stands at
            // or below the sea plane.** The static half of this rule is
            // `DW0886`, asked of the library before anything is placed; this is
            // the backstop for what the documents could not know — a floor a
            // stage-7 edit script carved after placement above all. Bound here
            // because this is the first point at which the walk region and the
            // placed boxes are both in hand, and printed before it is raised,
            // for the same vacuity reason the exposure line is.
            let (sea_walk, sea_walk_findings) =
                crate::compiler::plan::check_ocean_walk_plane(plan, prefabs, &party_walk);
            eprintln!("{}", sea_walk.line());
            if let Some((first, rest)) = sea_walk_findings.split_first() {
                for extra in rest {
                    eprintln!("{} [error] build: {}", extra.code, extra.message);
                }
                return Err(BuildFailure::Diagnostic {
                    code: crate::compiler::plan::DW_OCEAN_WATERLINE,
                    message: first.message.clone(),
                });
            }

            // **`DW0885`: a piece's outside answers for itself, or the world
            // buries it.** Bound here because this is the one function that
            // turns a plan into a datapack AND the first point at which the
            // three things the question needs are all in hand: the assembled
            // blocks (what the build really writes, a valley's terrain
            // included), the placed boxes, and the walk region the party is
            // proved to stand in. `Plan::build` cannot answer it — it runs
            // before a single `.nbt` byte is read, so it knows where the boxes
            // are and nothing about which of their boundary cells are solid.
            // The binding line is printed BEFORE the refusal is raised, and the
            // ordering is the vacuity rule rather than a nicety: the run that
            // finds something owes its reader the same counts as the run that
            // finds nothing, and a check whose line appears only on a pass is one
            // whose denominator nobody can read at the moment it matters.
            let (exposure, findings) = crate::compiler::burial::check(
                plan,
                prefabs,
                blocks,
                structures,
                world,
                &party_walk,
            );
            eprintln!("{}", exposure.line());
            piece_exposure_ledger = Some(exposure.to_json());
            if let Some((first, rest)) = findings.split_first() {
                // Every piece this world stands unanswered, not only the one
                // that stops the build. The failure channel carries one message
                // (`BuildFailure`), which is the compiler's contract and does
                // not move; what would otherwise be lost is that a placement
                // defect is routinely several pieces at once, so the rest print
                // here and the first travels as the refusal.
                for extra in rest {
                    eprintln!("{} [error] build: {}", extra.code, extra.message);
                }
                return Err(BuildFailure::Diagnostic {
                    code: crate::compiler::burial::DW_PIECE_EXPOSED,
                    message: first.message.clone(),
                });
            }

            // **The surround bounds the map, proven rather than promised**
            // (`DW0854`). The generator guarantees that no surround column
            // stands exactly one block above the gap-floor datum, so the
            // floor's own walkable component is bounded above by the datum and
            // the first thing outward of it is a two-block riser, which
            // vanilla's auto-step and jump cannot take. That is an argument
            // about the generator; this is a measurement of the world.
            //
            // It has to be a second measurement, because a great deal happens
            // between the two: gravity settles, a stage-7 edit script may carve,
            // a palette may change, and any of those can put a riser on the
            // slope the generator never wrote. So the proof floods the SAME nav
            // model every route proof uses, from the surround's own gap-floor
            // cells, and asks whether anything it reaches lies outward of the
            // crest line — sharing none of the generator's arithmetic, which is
            // what makes it an observer rather than a restatement.
            //
            // Its binding is on the plan (`SurroundBinding::floor_cells`) and is
            // printed with every surround build, because a flood that started
            // from nowhere passes for free and looks exactly like this one.
            if let Some(surround) = &plan.surround
                && let Err(cell) = surround.valley.verify_unclimbable(world)
            {
                {
                    return Err(BuildFailure::Diagnostic {
                        code: crate::compiler::surround::DW_VALLEY_CLIMB,
                        message: format!(
                            "the surround's inner slope has grown a standable staircase: a walk \
                             starting on the gap floor stands at [{}, {}, {}], outward of the \
                             crest line, so the landform no longer bounds the map. The generator \
                             leaves nothing standing one block over the gap floor and keeps trees \
                             off the inner wall, so a step up from the floor was put back by \
                             something later — an edit batch, a gravity settle, or a palette \
                             whose blocks are a different height. It flooded from {} standable \
                             gap-floor cells",
                            cell[0], cell[1], cell[2], surround.binding.floor_cells,
                        ),
                    });
                }
            }

            crate::compiler::nav::verify_boundary_safety(
                world,
                &crate::compiler::edit::anchor_starts(plan),
            )
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;

            // spec-0092 §10: the boundary and the world agree (`DW0960`) — every
            // place a body is put stands inside a region that returns, and a
            // region that does not return encloses a world nobody can leave. The
            // walk region is computed only for the second shape, which reads it.
            {
                let region = playable_region_box(plan);
                let returns = boundary_returns(plan);
                let starts = crate::compiler::edit::anchor_starts(plan);
                let (reachable, sea_entry) = if region.is_some() && !returns {
                    (
                        world.reachable_walkable_rooted(&starts),
                        crate::compiler::nav::open_sea_entry(world, &starts),
                    )
                } else {
                    (BTreeSet::new(), None)
                };
                let (gate, findings) = crate::compiler::bound::judge(
                    &crate::compiler::bound::places(plan),
                    region,
                    returns,
                    &reachable,
                    sea_entry,
                );
                eprintln!("{}", gate.line());
                if let Some(first) = findings.first() {
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }

            // spec-0092 §10: a link whose root plays a cutscene before the carry
            // takes everyone or no one — `cs_end` puts every player on the cell the
            // presser stood on — so a press from a cell outside its volume strands
            // the whole party (`DW0932`, the fault "pressed from outside its
            // volume"). The route proof stands one chosen cell inside the volume;
            // this asks every cell a press reaches from.
            {
                let gathered: Vec<&crate::compiler::link::LinkPlan> = plan
                    .links
                    .iter()
                    .filter(|l| l.gathered_by.is_some())
                    .collect();
                let reachable = if gathered.is_empty() {
                    BTreeSet::new()
                } else {
                    world.reachable_walkable_rooted(&crate::compiler::edit::anchor_starts(plan))
                };
                let mut outside_cells = 0usize;
                for l in &gathered {
                    let cells = crate::compiler::nav::press_cells_outside(world, l, &reachable);
                    outside_cells += cells.len();
                    if let Some(first) = cells.first() {
                        let listed = cells
                            .iter()
                            .take(6)
                            .map(|c| format!("{c:?}"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(BuildFailure::Diagnostic {
                            code: crate::compiler::plan::DW_TELEPORT_LINK,
                            message: format!(
                                "the link `{t}` (`{p}`) carries {b} after the cutscene at `{cs}` \
                                 ends, and `cs_end` puts every player on the cell the presser \
                                 stood on — so the carry takes everyone or no one. {n} cell(s) a \
                                 body can walk to perform the trigger from outside its volume: \
                                 {listed}{more}. Pressed from {first:?}, the whole party is put \
                                 down there and nobody is carried. Fault: pressed from outside its \
                                 volume. Remedy: widen the volume over every cell the press \
                                 reaches from, or move the trigger's body where it can be reached \
                                 only from inside the volume — its own cell included, which the \
                                 volume may not cover (`DW0542`), so the body stands where no \
                                 body can stand: over a rail, a post or open water.",
                                t = l.trigger_id,
                                p = l.path,
                                b = l.box_words(),
                                cs = l.gathered_by.as_deref().unwrap_or(""),
                                n = cells.len(),
                                more = if cells.len() > 6 { ", …" } else { "" },
                            ),
                        });
                    }
                }
                eprintln!(
                    "link gathering binding: {} link(s) carried after their root's cutscene, {} \
                     press cell(s) outside their volumes over {} walk cell(s)",
                    gathered.len(),
                    outside_cells,
                    reachable.len()
                );
            }

            // Seat each wave mob on a validated standable cell near its anchor, in
            // room only (DW0312 if the room lacks the footing) — or, for a
            // `summon: aggro-edge` wave, on its perception ring (DW0387).
            //
            // **Before the proofs, because a wave's seats are posted places.** A
            // seated mob is put where it is by declaration and not by walking,
            // exactly like an NPC's anchor, so `lethal::check_respawn_seats` has
            // to be able to see it — and the cells are a measurement of the
            // assembled world (the footing the room really offers), which no
            // reading of the plan alone can produce. Everything below is a
            // consumer of this answer; nothing it needs is computed below it.
            let (waves, rings) = plan_wave_spawns(plan, world)?;

            // **spec-0068: a firework bursts in open air, clear of every posted
            // body** (`DW0899`). Asked here, immediately after the seating pass,
            // because a wave's seats ARE posted places and the reach rule reads
            // `DW0511`'s own enumeration — so the proof has to run where that
            // enumeration is complete. It reads the plan, the assembled blocks
            // and the seating, never a route, so nothing below it is lost by
            // asking it first.
            //
            // The line is printed whether or not it found anything, and before
            // the verdict is taken: a refusal owes its reader the same
            // denominators a pass does.
            {
                let (binding, findings) =
                    crate::compiler::firework::check(plan, blocks, campaign_spawn(plan), &waves);
                eprintln!("{}", binding.line());
                firework_gate = Some(binding);
                if let Some((first, rest)) = findings.split_first() {
                    for extra in rest {
                        eprintln!("{} [error] build: {}", extra.code, extra.message);
                    }
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }
            // **spec-0092: a lightning bolt strikes clear of every posted body and
            // every block it would rewrite** (`DW0958`, `DW0959`). Asked where the
            // firework is, for the firework's reason: its reach rule reads
            // `DW0511`'s enumeration, which is complete only once the seating has
            // run. The line prints before the verdict, zeroes included.
            {
                let (binding, findings) =
                    crate::compiler::lightning::check(plan, blocks, campaign_spawn(plan), &waves);
                eprintln!("{}", binding.line());
                lightning_gate = Some(binding);
                if let Some((first, rest)) = findings.split_first() {
                    for extra in rest {
                        eprintln!("{} [error] build: {}", extra.code, extra.message);
                    }
                    return Err(BuildFailure::Diagnostic {
                        code: first.code,
                        message: first.message.clone(),
                    });
                }
            }
            let (moves, actor_moves) = if crate::compiler::nav::needs_world(plan) {
                let m = crate::compiler::nav::plan_moves(plan, world)?;
                // move-actor (spec-0014): A* over the actor's footprint; DW0325 if
                // unroutable. Planned alongside move-npc from the same occupancy model.
                let am = crate::compiler::nav::plan_actor_moves(plan, world)?;
                cutscene_shots_judged =
                    crate::compiler::nav::check_cutscenes(plan, world, &m, &am)?;
                // spec-0031: the one lethal-volume obligation routing cannot see.
                // A respawn SEAT inside a volume is reached by teleport and routes
                // perfectly while killing the party on arrival, forever. The wave
                // seating goes in with it: a mob put on a cell by the seating pass
                // is put there by declaration too, and its body is not the walker
                // the footing was proven for.
                //
                // **Before the route proofs, and that ordering is a judgement.** A
                // volume that swallows a posted place usually closes the route to
                // it as well, so the two findings arrive together — and `DW0510`
                // then sends the author to "move the volume, or give the party a
                // route around it" when what is actually wrong is that their
                // Keeper is standing in the pit. The specific cause is the more
                // actionable message, and the route closure is its symptom. This
                // proof reads the plan and the seating, never the routes, so
                // nothing is lost by asking it first.
                let lethal_seats = crate::compiler::lethal::check_respawn_seats(
                    plan,
                    campaign_spawn(plan),
                    &waves,
                )?;
                // spec-0062: **danger is visible, or the engine refuses it.**
                // `DW0891`, asked BEFORE every route proof and after `DW0511`,
                // and the order is the judgement §4 records: a volume that
                // catches walked floor usually closes a route as well and often
                // sits under a reach, so asked first the refusal names the
                // cause, and `DW0510`, `DW0850` and `DW0881` then judge a volume
                // the player can see. This proof reads the plan's volumes, the
                // lethality-free population and the block map, never a route, so
                // nothing is lost by asking it here.
                let danger = if plan.lethal_volumes.is_empty() {
                    crate::compiler::lethal::DangerVisibility::default()
                } else {
                    let blocks = &assembled.blocks;
                    // `DW0922` / `DW0923`: where each seated wave's members can
                    // get to by the movement a mob has. Measured before `DW0891`,
                    // because a volume only a mob can enter is a volume a
                    // modelled body reaches and `DW0891`'s zero-binding finding
                    // has to know it; judged after `DW0891`, so a volume the
                    // player cannot see is named for that first.
                    let wave_lethal =
                        crate::compiler::lethal::wave_reach(plan, world, blocks, &waves);
                    let (mut binding, verdict) = crate::compiler::lethal::check_danger_is_visible(
                        plan,
                        world,
                        blocks,
                        campaign_spawn(plan),
                    );
                    binding.credit_waves(&wave_lethal);
                    // Stated whether it found anything or not, and before the
                    // verdict is taken: a refusal owes its reader the population
                    // it was measured against as much as a pass does.
                    eprintln!("{}", binding.line());
                    eprintln!("{}", wave_lethal.line());
                    verdict?;
                    warnings.extend(binding.findings());
                    wave_lethal.verdict()?;
                    put_json(out, "validation/wave-lethal.json", &wave_lethal.to_json());
                    binding
                };
                // spec-0085: `DW0943`, **a blinding beside a drop** — after
                // `DW0891`, so every volume it reasons about is one the player
                // could see, and before the route proofs. The binding is printed
                // whether or not the campaign grants a blinding, zeroes included.
                let (blind, blind_verdict) =
                    crate::compiler::blind::check(plan, world, campaign_spawn(plan));
                eprintln!("{}", blind.line());
                blind_verdict?;
                // spec-0086 §4: a loop's slab, its closed view, the bodies in its
                // span and its tiling, over the world as shipped (relight
                // fixtures and world-load seals included). Before the route
                // proofs, because a loop that cannot be polled or seen through is
                // the cause, and a route closed by its slab is the consequence.
                if !plan.loops.is_empty() {
                    let (binding, refusal) =
                        crate::compiler::r#loop::check(&crate::compiler::r#loop::Inputs {
                            plan,
                            world,
                            blocks: &assembled.blocks,
                            placements: &relight.placements,
                            seals: &assembled.gate_seals,
                            wave_seats: &waves,
                        });
                    eprintln!("{}", binding.line());
                    loop_gate = Some(binding);
                    if let Some(f) = refusal {
                        return Err(BuildFailure::Diagnostic {
                            code: f.code,
                            message: f.message,
                        });
                    }
                }
                // DW0311, with its binding stated whichever way it goes
                // (spec-0083 §5): every leg partitioned into walked, carried by
                // a crossing and carried by a link.
                let (route_binding, route_verdict) =
                    crate::compiler::nav::check_critical_path_bound(plan, world);
                eprintln!("{}", route_binding.line());
                route_verdict?;
                legs_carried = route_binding.carried;
                // v0.6 checkpoint no-stranding + placement proofs (spec-0012,
                // DW0315/DW0316) and stealth-zone standable/reachable proofs
                // (spec-0014, DW0327), re-rooting DW0311 reachability at each beat.
                crate::compiler::nav::check_checkpoints(plan, world)?;
                // DW0921: no place a body can reach from the route by walking,
                // falling or jumping is one it cannot leave. The binding is
                // printed before the verdict is taken, like DW0891's.
                let (leave_binding, leave_verdict) = crate::compiler::nav::check_bodies_can_leave(
                    plan,
                    world,
                    playable_region(plan).map(|r| (r.min, r.max)),
                );
                eprintln!("{}", leave_binding.line());
                leave_verdict?;
                put_json(out, "validation/leave-proof.json", &leave_binding.to_json());
                // DW0924: a body a `kill` objective waits on cannot get to a
                // place it survives and the party cannot strike it from. After
                // DW0921 because it reads the same playable region and a party
                // that can be trapped is the worse finding.
                {
                    let strand = crate::compiler::strand::check(
                        plan,
                        world,
                        &waves,
                        &crate::compiler::lethal::stands_at_roots(plan, campaign_spawn(plan)),
                        playable_region(plan).map(|r| (r.min, r.max)),
                    );
                    if strand.waves > 0 {
                        eprintln!("{}", strand.line());
                    }
                    strand.verdict()?;
                    if strand.waves > 0 {
                        put_json(out, "validation/strand.json", &strand.to_json());
                    }
                }
                if !plan.lethal_volumes.is_empty() {
                    lethal_gate = Some(crate::compiler::lethal::gate(
                        plan.campaign,
                        &plan.lethal_volumes,
                        world.lethal_cells(),
                        lethal_seats,
                        crate::compiler::nav::critical_leg_count(plan),
                        // One template per resolved volume, and a `_shut` one
                        // more per staged volume (see `emit_lethal_packtests`).
                        plan.lethal_volumes.len()
                            + plan
                                .lethal_volumes
                                .iter()
                                .filter(|v| v.staged.is_some())
                                .count(),
                        danger,
                    ));
                    if let Some(g) = lethal_gate.as_mut() {
                        g.blind = blind;
                    }
                }
                // spec-0032: the recovery stake's placement table and its proofs
                // (`DW0525` no route back, `DW0526` no safe footing). Placed after
                // the lethal-volume seat proof because it CONSUMES the volumes as
                // death regions — a volume that strands the party is a worse
                // finding, and it should be reported first.
                stake_table = crate::compiler::stake::build(plan, world, campaign_spawn(plan))?;
                // …and the contract the bot tier needs to prove any of it at
                // runtime. Built here, from the SAME table the proofs above ran
                // on, because a PackTest fake player is permanently undamageable
                // (measured 2026-08-03 and 2026-08-09) and so the whole death loop
                // is the mineflayer tier's claim to make.
                death_plan = crate::compiler::deathplan::build(
                    plan,
                    world,
                    campaign_spawn(plan),
                    stake_table.as_ref(),
                );
                crate::compiler::nav::check_stealth_zones(plan, world)?;
                // …and the onset-survivability proof on top of them (DW0355): a
                // punishing beat must be escapable in `grace_ticks` from where the
                // player provably stands when it arms, and from every checkpoint
                // that can respawn them back into it.
                crate::compiler::nav::check_stealth_onset(plan, world)?;
                // v0.6 trap completability proof (spec-0011, DW0342): every lethal
                // trap on the forced critical path must be avoidable, survivable
                // (`once`), or disarmable, else the party is provably killed or
                // soft-looped. Uses the move-npc waypoints (`m`) for the forced-path
                // cell set.
                crate::compiler::nav::check_traps(plan, world, &m)?;
                // spec-0016 §2 shortcut doors (DW0373/DW0374): the long route must
                // exist while the gate is sealed, and opening the gate must
                // genuinely shorten the crossing. The critical path above was
                // already proven with every shortcut gate SEALED (Plan::build seals
                // them at step 0), so the delve is finishable the long way.
                // `DW0425` (the wrong-side refusal) is raised with the hitbox
                // proofs above, well before this block — its own call site says
                // why it has to reach the author first.
                //
                // Every click trigger must land on something (DW0426). The
                // ledger it returns is emitted below: "how many clicks did this
                // proof resolve a body for" is the one fact that distinguishes a
                // campaign whose presses all land from one that arms none.
                put_json(
                    out,
                    "validation/press-bodies.json",
                    &check_trigger_bodies(plan)?.to_json(),
                );
                crate::compiler::nav::check_shortcuts(plan, world, campaign_spawn(plan))?;
                // spec-0016 §3 ambush counterplay (DW0376): 初见杀 is legitimate,
                // a pocket with no retreat is not.
                crate::compiler::nav::check_ambushes(plan, world, campaign_spawn(plan))?;
                // spec-0016 §4 timed gates (DW0378): a gate that punishes bad
                // timing is the point; one that punishes every timing is a slot
                // machine. At least 20% of the cycle must admit a crossing.
                crate::compiler::nav::check_timed_gates(plan, world)?;
                // The third rung. A gate's `disarm` lever must be
                // reachable while the gate is still SHUT — a jam you can only
                // pull after surviving the crossing disables nothing (DW0393).
                crate::compiler::nav::check_timed_gate_disarms(plan, world, campaign_spawn(plan))?;
                // spec-0016 §4 addendum — hazard observability (DW0388). The
                // dossier's strongest finding: what makes a periodic hazard fair
                // is not its ratio but that you can stand somewhere safe and
                // WATCH it before committing. Error tier for a souls campaign (it
                // declares a bonfire), warning tier otherwise.
                let unobserved = crate::compiler::nav::check_hazard_observability(
                    plan,
                    world,
                    campaign_spawn(plan),
                )?;
                // spec-0016 §7 pacing lints (DW0379 retry cost, DW0380 optional-
                // elite bypass). Warning tier: both are design judgements the
                // compiler can MEASURE but must not overrule — a long walk back
                // can be the authored point, and the owner's QA hour decides.
                *pacing = crate::compiler::nav::pacing_lints(plan, world);
                pacing.extend(unobserved);
                // Export the DW0311-proven critical-path routes as validation
                // metadata: thinned per-leg waypoint polylines the harness
                // replays as successive nearby goals, so no single giant mineflayer A*
                // solve strands the bot on a large open cave. NOT shipped gameplay —
                // lives under `validation/` (excluded from the delve image, like
                // packtest-datapack/). Emitted only when a walked leg exists, so a
                // campaign with none stays byte-identical to before. Uses the same
                // relight-aware `world` as the DW0311 check it exports.
                let routes = crate::compiler::nav::critical_path_routes(plan, world);
                // Structural self-check: every exported waypoint must be
                // genuinely standable in this FINAL world (settled + water-flooded +
                // fixtures). Makes it impossible to ship a waypoint the game floods
                // or walls — the water-flow / post-nav-mutation divergence class —
                // failing the build loudly (DW0314) instead of stranding the bot.
                crate::compiler::nav::verify_exported_routes(world, &routes)?;
                // spec-0065 §4.3: what the furniture exclusion bound, over the
                // same world and the same legs the proofs above walked. Printed
                // on every build that walks, zeroes included.
                let furniture =
                    crate::compiler::nav::furniture_binding(plan, world, &routes, &m, &am);
                eprintln!("{}", furniture.line());
                put_json(out, "validation/furniture-gate.json", &furniture.to_json());
                // `DW0850`: the volume that completes a `reach` and the footing
                // a body can reach it from are the same place. Bound HERE, to
                // the same build event and the same final world the waypoint
                // proof judges — the endpoint snap searches three blocks and
                // the completion cube reaches one, so a route can be proven,
                // exported and walked to a cell that never fires the objective.
                crate::compiler::reach::check_reach_completion(
                    plan,
                    world,
                    &routes,
                    campaign_spawn(plan),
                )?;
                // `DW0881`: the other direction of the same sentence. `DW0850`
                // asks whether the party can complete this at all; this asks
                // whether anybody can complete it WITHOUT arriving. The volume is
                // centred on the anchor in all three axes and vanilla tests it
                // against the whole body box, so a raised anchor whose radius
                // reaches the floor below completes from that floor and the party
                // never climbs. Bound here, to the same event and the same final
                // world, and its binding line is printed whether it found
                // anything or not — a count only says something when the run that
                // found nothing prints it too.
                let (reach_footprint, off_floor) = crate::compiler::reach::check_reach_footprint(
                    plan,
                    world,
                    campaign_spawn(plan),
                );
                eprintln!("{}", reach_footprint.line());
                off_floor?;
                // Stair-orientation proof (DW0430). Nav models a stair
                // as a full cube, so a reversed stair reads as a legal one-block
                // jump and every existing proof passes — the delve ships with a
                // staircase the player must hop up tread by tread. This is the
                // one check that reads `facing`, over the same proven routes,
                // against the same assembled world.
                if !routes.is_empty() {
                    let route_cells: Vec<Vec<[i32; 3]>> =
                        routes.iter().map(|r| r.cells.clone()).collect();
                    let blocks = &assembled.blocks;
                    crate::compiler::stairs::check_stair_orientation(
                        blocks,
                        Some(plan),
                        &route_cells,
                    )?;
                }
                // spec-0102: every pulse's derived range over the standable
                // cells of its place, and its listening stations on these
                // legs. The binding is printed before the verdict, a refusal
                // included, and on every build that walks, zeroes included.
                {
                    let (gate, verdict) = crate::compiler::pulse::measure(plan, world, &routes);
                    eprintln!("{}", gate.line());
                    verdict.map_err(|f| BuildFailure::Diagnostic {
                        code: f.code,
                        message: f.message,
                    })?;
                    warnings.extend(gate.findings());
                    pulse_gate = Some(gate);
                }
                if !routes.is_empty() {
                    put_json(
                        out,
                        "validation/critical-path-waypoints.json",
                        &crate::compiler::waypoints::waypoints_json(plan, &routes),
                    );
                }
                path_legs.push((
                    "critical-path".to_string(),
                    plan.critical_path.clone(),
                    routes.clone(),
                ));
                // Visual-tier POV cameras (spec-0003): one first-person shot per
                // corner-thinned waypoint. Their eye cells are proven clear in the
                // FINAL assembled world with every other kind's, at the one place
                // a render plan can be built (`PlannedShots::into_document`) —
                // this used to be a POV-only check here, which is exactly why the
                // identical defect on a seam camera was invisible.
                pov_shots = crate::compiler::render_plan::pov_shots(plan, &routes);
                // spec-0025 branch navigation, made first-class. The
                // DW0311 proof above quantifies over the DEFAULT playthrough
                // only, and the waypoint export followed it — so a branch run
                // walked its fork-divergent legs with no proof behind them and
                // no waypoints under them (single-goal navigation, which is
                // terrain-flaky exactly where the proven path is deterministic).
                // Here every REACHABLE branch's exported path gets both halves:
                // its own DW0311 (each walked leg routed over this same
                // assembled world, under the BRANCH's own causal gate seals, in
                // its own step space — `Plan::branch_gate_model`; default-path
                // indices belong to a different sequence and must never be
                // inherited) and its own waypoint artifact, derived from those
                // proven routes exactly as the critical path's is (same
                // thinning, same DW0314 standability self-check, same JSON
                // shape — `waypoints_json`). Deterministic: branches enumerate
                // in declaration-order (ADR-0006).
                let realized = crate::compiler::branch::realize(plan.campaign);
                if !realized.is_empty() {
                    let flow = crate::compiler::flow::Flow::new(plan.campaign);
                    for r in &realized {
                        let Some(widx) = r.world else { continue };
                        let label = |e: Failure| Failure {
                            code: e.code,
                            message: format!("branch `{}`: {}", r.branch.id, e.message),
                        };
                        let branch_fail = |e: plan::PlanError| BuildFailure::Diagnostic {
                            code: e.failure.code,
                            message: format!("branch `{}`: {}", r.branch.id, e.failure.message),
                        };
                        let unlinked = plan
                            .branch_critical_path(&flow, &flow.playthrough_in(widx))
                            .map_err(branch_fail)?;
                        // The branch's own links (spec-0083 §6): taken over its
                        // own steps and gate model, then spliced into its path.
                        let takes = if plan.links.is_empty() {
                            plan::LinkTakes::default()
                        } else {
                            let (region_events, ancestors) = plan.branch_gate_model(&unlinked);
                            let ancestor = |g: usize, s: usize| {
                                g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                            };
                            crate::compiler::nav::take_branch_links(
                                plan,
                                world,
                                campaign_spawn(plan),
                                &unlinked,
                                &region_events,
                                &ancestor,
                            )
                            .map_err(label)?
                        };
                        let cp = if takes.is_empty() {
                            unlinked
                        } else {
                            plan.branch_critical_path_linked(
                                &flow,
                                &flow.playthrough_in(widx),
                                &takes,
                            )
                            .map_err(branch_fail)?
                        };
                        branch_takes.insert(r.branch.slug.clone(), takes);
                        let (region_events, ancestors) = plan.branch_gate_model(&cp);
                        let ancestor = |g: usize, s: usize| {
                            g == 0 || ancestors.get(&s).is_some_and(|a| a.contains(&g))
                        };
                        crate::compiler::nav::check_branch_path(
                            plan,
                            world,
                            campaign_spawn(plan),
                            &cp,
                            &region_events,
                            &ancestor,
                        )
                        .map_err(label)?;
                        let branch_routes = crate::compiler::nav::branch_path_routes(
                            world,
                            campaign_spawn(plan),
                            &cp.steps,
                            &cp.transport_by_step,
                            &region_events,
                            &ancestor,
                        );
                        crate::compiler::nav::verify_exported_routes(world, &branch_routes)
                            .map_err(label)?;
                        if !branch_routes.is_empty() {
                            branch_waypoints.push((
                                r.branch.slug.clone(),
                                crate::compiler::waypoints::waypoints_json(plan, &branch_routes),
                            ));
                        }
                        path_legs.push((r.branch.slug.clone(), cp.steps.clone(), branch_routes));
                    }
                }
                (m, am)
            } else {
                (Vec::new(), Vec::new())
            };
            // Body clearance (DW0450/DW0451): no NPC or actor body may
            // occupy the same space as block geometry — not at the anchor it is
            // summoned on, and not at any tick of any walked leg. A walked
            // destination was already safe by construction (endpoint snapping +
            // passable-cell A*); a `summon` does no snapping, which is how the
            // island shipped a 2.9-tall warden inside the cliff face beside its
            // cave mouth with every other proof green. Runs after the moves are
            // planned because the walked waypoints are half of what it proves.
            warnings.extend(
                crate::compiler::clearance::check_body_clearance(plan, world, &moves, &actor_moves)
                    .map_err(|e| BuildFailure::Diagnostic {
                        code: e.code,
                        message: e.message,
                    })?,
            );
            // …and the move that got the body there must be one the body can
            // make (DW0452/DW0453, island round 21). `clearance` proves where a
            // body IS; this proves what it DID. The two island sightings it
            // exists for: eight sheep walking through a closed fence gate the
            // owner could not walk through herself, and a sheep leaving the
            // beach fold by stepping onto its wall's full-cube course instead of
            // using the pen's opening. Capabilities come from the entity, so a
            // spider routed over a wall stays silent and a sheep does not.
            let (traversal_warnings, gate) =
                crate::compiler::traversal::check_traversal(plan, world, &moves, &actor_moves)
                    .map_err(|e| BuildFailure::Diagnostic {
                        code: e.code,
                        message: e.message,
                    })?;
            warnings.extend(traversal_warnings);
            traversal_gate = Some(gate);
            // …and prove the sun is not going to fight the party's battle for it
            // (DW0496). Runs HERE because it needs the seated cells:
            // the question is whether open sky stands within one aggro radius of
            // where the mobs actually land, on ground they can walk to — not of
            // an anchor they stand around. The hollow-vigil gate yard is the
            // motivating case: roof and two walls carved off, noon pinned, and
            // two of three footmen dead to sunlight before the party could
            // engage them, with every other proof green.
            crate::compiler::daylight::check_daylight_staging(
                plan,
                world,
                &assembled.blocks,
                &waves,
            )
            .map_err(|e| BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            })?;
            // …and its mirror: prove the body will fight at all (DW0920). A
            // drowned takes no land target while the level is bright, so a choir
            // staged on dry ground under a bright hour walks to its water and
            // leaves the party a fight nobody answers — vesperhold's Undertide
            // Pool. Same seated cells, same reach, same population.
            let (engage, refused) =
                crate::compiler::engage::check_engagement(plan, world, blocks, &waves);
            eprintln!("{}", engage.line());
            if let Some(e) = refused {
                return Err(BuildFailure::Diagnostic {
                    code: e.code,
                    message: e.message,
                });
            }
            // spec-0023 §2: the winnability arithmetic. Runs here because it
            // needs the SEATED spawn cells (the exact cells the datapack will
            // summon on) as well as the campaign's declarations — a hostile the
            // party cannot reach is a property of where it actually lands, not
            // of where its anchor is.
            //
            // Gated on EVERY fight, wave-shaped or actor-shaped
            // (`combat::mandatory_fights`). It used to be gated on `kill`-a-wave
            // alone, which meant a delve whose combat is entirely actors ran none
            // of spec-0023 at all — the whole pass silently inapplicable, with
            // every board green.
            if crate::compiler::combat::mandatory_fights(plan).any() {
                warnings.extend(
                    crate::compiler::combat::check_winnability(plan, world, &waves).map_err(
                        |e| BuildFailure::Diagnostic {
                            code: e.code,
                            message: e.message,
                        },
                    )?,
                );
            }
            // The bot ladder's combat plan (spec-0023 §1): which encounters exist,
            // what the content bills each as, which checkpoint governs a death at
            // it, and — the muster — what each wave DECLARES its bodies to be, so
            // the ladder can read the live ones against it. Validation metadata
            // only: it lives under `validation/`, which `Dockerfile.delve`
            // excludes, so no shipped byte moves.
            //
            // spec-0016 §6: resolve and prove each TD lane polyline (DW0386). The
            // proven cells are what `patrol_target` carries, so the squad is only
            // ever sent somewhere it can stand and walk to.
            //
            // Before the combat plan: a run-back is measured against where the
            // hostiles actually are, and a lane wave is where it marches.
            let lanes = crate::compiler::nav::plan_lanes(plan, world)?;
            // A campaign whose only fight is an ACTOR still ships a plan. It has no
            // `encounters[]` for the ladder to read or clear, but `fights` is the
            // binding count for the whole combat pass — and a five-hostile campaign
            // that emits no plan at all reads as combat-free, which is the silence
            // the block was added for.
            if crate::compiler::combat::has_encounters(plan)
                || crate::compiler::combat::mandatory_fights(plan).any()
            {
                let mandatory = crate::compiler::combat::encounters(plan);
                // Run-backs (spec-0016 §1): a cleared `respawns_on_rest` wave a
                // rest re-seats beside a leg the path walks afterwards. Measured
                // over every exported path, against the same aggro model the
                // respawn safe zone uses.
                let sources = crate::compiler::nav::aggro_sources(plan, world, &waves, &lanes);
                let legs: Vec<crate::compiler::combat::PathLegs<'_>> = path_legs
                    .iter()
                    .map(|(label, steps, routes)| crate::compiler::combat::PathLegs {
                        label: label.clone(),
                        steps,
                        routes,
                    })
                    .collect();
                let run_backs = crate::compiler::combat::run_backs(plan, world, &legs, &sources);
                put_json(
                    out,
                    "validation/combat-plan.json",
                    &crate::compiler::combat::combat_plan_json(plan, &mandatory, &run_backs),
                );
            }
            // spec-0016 §1: the RESPAWN-POINT safe zone
            // (DW0478). Runs here because it needs both halves of where the
            // hostiles actually are — the seated spawn cells above and the lane
            // polylines just resolved — measured against every rest point. A
            // respawn point inside a hostile's aggro range is a soft-lock: rest
            // and death both deliver the party into contact on arrival.
            //
            // "Every rest point" is every `CheckpointPlan`, bonfire or plain
            // `set-checkpoint`. The ledger states how many pairs were compared,
            // because a proof that examined nothing must not read as a pass.
            let respawn_safety =
                crate::compiler::nav::check_respawn_safe_zone(plan, world, &waves, &lanes)?;
            put_json(
                out,
                "validation/respawn-safety.json",
                &respawn_safety.to_json(),
            );
            // spec-0022: resolve and prove every `volley` / `collapse`. Volley
            // coverage is proven by construction (one shot per standable
            // kill-zone cell, or DW0442 naming the cell it cannot reach), and a
            // collapse must leave the critical path completable in its SPRUNG
            // state (DW0445).
            let payloads = plan_payload_verbs(plan, world, blocks)?;
            (moves, actor_moves, waves, rings, lanes, payloads)
        }
    } else {
        (
            Vec::new(),
            Vec::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            PayloadPlans::default(),
        )
    };
    Ok(WorldProofs {
        pulse_gate,
        moves,
        actor_moves,
        wave_placements,
        wave_rings,
        lane_routes,
        payload_plans,
        pov_shots,
        branch_waypoints,
        branch_takes,
        legs_carried,
        path_legs,
        traversal_gate,
        gate_seal_ledger,
        fluid_escape_ledger,
        sea_seepage_ledger,
        piece_exposure_ledger,
        lethal_gate,
        loop_gate,
        firework_gate,
        cutscene_shots_judged,
        lightning_gate,
        stake_table,
        death_plan,
    })
}
