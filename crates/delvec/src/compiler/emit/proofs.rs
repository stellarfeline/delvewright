//! The proofs `build_with_warnings` takes outside the world block: the
//! assemblies, the blockout battery and the declaration proofs before it,
//! and the checks read off the finished tree after emission.

use super::*;

/// The assemblies (`DW0936`–`DW0938`), judged over the world the other proofs
/// read, before any route is derived. Returns the binding the critical path
/// carries the bot's witness of each blow into.
///
/// Asked over the world the other proofs read, before any route is derived:
/// a hitbox, its reach and where a blow lands are facts about cells, and
/// nothing below changes them. The binding and the cost the host meets are
/// printed on every build, zeroes included, before the verdict is taken.
pub(super) fn prove_assemblies(
    plan: &Plan,
    world: &crate::compiler::nav::World,
    population: &std::collections::BTreeSet<[i32; 3]>,
) -> Result<crate::compiler::assembly::AssemblyBinding, BuildFailure> {
    let entry = campaign_spawn(plan);
    let roots = crate::compiler::lethal::stands_at_roots(plan, entry);
    let returned = playable_region(plan).map(|r| (r.min, r.max));
    // Where the party can walk while each performed trigger is the next
    // beat — `DW0924`'s reading: a gate a later beat opens is shut. A
    // trigger step proves no objective, so the configuration is the one
    // the next objective step stands under: the strike is made on the way
    // to it, after every beat before it.
    let reaches = |trigger: &str, lo: [f64; 3], hi: [f64; 3]| {
        let step = plan
            .critical_path
            .iter()
            .position(|s| matches!(s, Step::Trigger { trigger_id, .. } if trigger_id == trigger))
            .map(|t| {
                (t..plan.critical_path.len())
                    .find(|&i| plan.critical_path[i].objective().is_some())
                    .unwrap_or(t)
            });
        let config = step.and_then(|s| crate::compiler::nav::world_while_next(plan, world, s));
        let ground = config.as_ref().unwrap_or(world);
        ground
            .reachable_walkable(&roots)
            .into_iter()
            .filter(|p| !crate::compiler::nav::returned_from(returned, *p))
            .any(|p| crate::compiler::strand::eye_reaches_box(ground, p, lo, hi))
    };
    let (binding, findings) = crate::compiler::assembly::check(plan, population, &reaches);
    eprintln!("{}", binding.line());
    eprintln!("{}", binding.cost_line());
    if let Some((first, rest)) = findings.split_first() {
        for extra in rest {
            eprintln!("{} [error] build: {}", extra.code, extra.message);
        }
        return Err(BuildFailure::Diagnostic {
            code: first.code,
            message: first.message.clone(),
        });
    }
    Ok(binding)
}

/// The walked population `P` (spec-0062 §2) over the lethality-free world —
/// the one population the assemblies (`DW0938`) and the watchers (`DW0997`)
/// are judged against, derived once per build.
pub(super) fn walked_population(
    plan: &Plan,
    world: &crate::compiler::nav::World,
) -> std::collections::BTreeSet<[i32; 3]> {
    let open = world.without_exclusions();
    crate::compiler::lethal::walked_population(plan, &open, campaign_spawn(plan))
}

/// The watchers (`DW0997`, spec-0101 §5.2): every watching body has a walked
/// cell within its reach. The binding line is printed on every build, zeroes
/// included, before the verdict is taken; the binding travels to the PackTests
/// (the cell each test stands its player on) and to `validation/watchers.json`.
pub(super) fn prove_watchers(
    plan: &Plan,
    population: &std::collections::BTreeSet<[i32; 3]>,
) -> Result<crate::compiler::watching::WatchBinding, BuildFailure> {
    let (binding, findings) = crate::compiler::watching::prove(plan, population);
    eprintln!("{}", binding.line());
    if let Some((first, rest)) = findings.split_first() {
        for extra in rest {
            eprintln!("{} [error] build: {}", extra.code, extra.message);
        }
        return Err(BuildFailure::Diagnostic {
            code: first.code,
            message: first.message.clone(),
        });
    }
    Ok(binding)
}

/// The stage-5 blockout battery (spec-0049 §5.3).
///
/// **Bound here, and here is the only door.** This is the one function that
/// turns a `Plan` into a datapack, so a site-plan world cannot be built,
/// packaged or shipped without being judged against the plan it was derived
/// from. There is no flag, no subcommand and no line in a document to
/// remember; a campaign with no site plan gets `None` and nothing runs, which
/// is why every other campaign's output is byte-identical.
///
/// It runs over the world model above — the same occupancy every other proof
/// in this function is taken under, edits and relight included — because a
/// battery that re-derived its own world would be judging a world nobody
/// ships. `DW0836`/`DW0837`/`DW0838` refuse (exit 3); `DW0821` and `DW0822`
/// are advisories that travel to the walk sheet, and the binding line is
/// stated whether anything was found or not.
pub(super) fn prove_blockout(
    plan: &Plan,
    assembled: &crate::compiler::assembled::Assembled,
    warnings: &mut Vec<delvewright_dsl::Diagnostic>,
) -> Result<(), BuildFailure> {
    let blocks = &assembled.blocks;
    // What the DERIVATION bound to, beside what its observer did. Printing
    // only the battery's line stated what was examined and never what was
    // built — and at stage 6 the difference is the whole reading: `detailed`
    // is how much of this map is a building and how much is still massing.
    if let Some(b) = &plan.blockout {
        eprintln!("{}", b.binding.line());
    }
    // What the ocean-datum invariant (`DW0344`) examined: how many placed
    // pieces declare a waterline, how many stand in the sea, and how many of
    // those were held to it. Printed on every build, ocean or not, because a
    // check that only speaks when it finds something cannot be told from one
    // that never ran.
    eprintln!("{}", plan.waterline.line());
    // What the horizon built, and — the half that matters — which authority
    // stated the rectangle it built around. A surround that ringed the
    // placed footprint and one that ringed the declared region look
    // identical from outside, right up until somebody details a place and
    // the mountains move. So the line names the authority, and the zeros are
    // findings rather than silences.
    if let Some(surround) = &plan.surround {
        eprintln!("{}", surround.binding.line());
        if surround.binding.floor_cells == 0 {
            eprintln!(
                "surround binding 0: the horizon built terrain but no standable gap-floor \
                 cell, so the un-climbability proof flooded from nowhere and its green means \
                 nothing. A surround with no floor is a wall around a hole."
            );
        }
    }
    if let Some(battery) = crate::compiler::blockout::check(plan, blocks) {
        eprintln!("{}", battery.binding.line());
        let refusals: Vec<&(delvewright_dsl::DwCode, delvewright_dsl::Diagnostic)> =
            battery.refusals().collect();
        if let Some(((code, refusal), rest)) = refusals.split_first() {
            // **Every rule that saw this defect, not only the one that stops
            // the build.** The failure channel carries one code and one
            // message (`BuildFailure`), which is the compiler's contract and
            // does not move; what used to be lost is that one derivation
            // defect is routinely seen by two of these rules, and a report
            // naming only the first sends a creator round the loop twice.
            // The line states the whole refusal set, and the ones after the
            // first print their own messages here — the failing one is
            // printed by the caller through the ordinary diagnostic channel,
            // so nothing is said twice.
            let mut per_code: BTreeMap<String, usize> = BTreeMap::new();
            for (c, _) in &refusals {
                *per_code.entry(c.to_string()).or_default() += 1;
            }
            eprintln!(
                "blockout battery: {} refusal(s) — {}; the build stops at the first.",
                refusals.len(),
                per_code
                    .iter()
                    .map(|(c, n)| format!("{c} ×{n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            // The same cap `BuildFailure::Validation` prints under, and for
            // the same reason: the set is what a reader needs, the whole
            // list is what a terminal loses. The count above is never
            // capped, so the cap cannot hide how much was found.
            const LISTED: usize = 20;
            for (c, d) in rest.iter().take(LISTED) {
                eprintln!("{c} [error] {}: {}", d.path, d.message);
            }
            if rest.len() > LISTED {
                eprintln!(
                    "  … and {} further refusal(s), not listed",
                    rest.len() - LISTED
                );
            }
            return Err(BuildFailure::Diagnostic {
                code: *code,
                message: refusal.message.clone(),
            });
        }
        warnings.extend(battery.advisories());
    }
    Ok(())
}

/// The proofs over resolved declarations that need no occupancy model: anchors,
/// teleport volumes, the dialogue cap, actor placement, marks, bodies on
/// affordances and on each other, shortcut sides, stake markers. Run in this
/// order before the world block; returns the teleport gate's ledger.
pub(super) fn prove_declarations(
    plan: &Plan,
    warnings: &mut Vec<delvewright_dsl::Diagnostic>,
) -> Result<crate::compiler::teleport::TeleportGate, BuildFailure> {
    // Every anchor-bearing effect, at every nesting depth, must resolve to a real
    // world position or the build stops (DW0360). This runs FIRST among the
    // referential proofs deliberately: emission fails open on an unresolved anchor
    // (it emits nothing) and the geometry proofs downstream fail *loudly but
    // wrongly* — an unresolved cutscene waypoint degrades to the world origin and
    // is then reported as a camera clipping a wall (DW0308), which sends the author
    // to move a shot that was never the problem. Name the root cause instead.
    check_effect_anchors(plan)?;

    // spec-0031: a `teleport` moves EVERYTHING inside its volume, so the volume
    // may not cover an affordance the engine bound to hardware it cannot move.
    // Runs here — right after the anchor-resolution seal and before any occupancy
    // model — because it is pure box arithmetic over resolved cells, and because
    // the alternative it replaces is a runtime type-exemption list
    // (`crate::compiler::teleport` records why that would be wrong).
    let teleport_gate = crate::compiler::teleport::check_bound_affordances(plan)?;

    // A dialogue node's conditionally-visible options are encoded as 2^n precomputed
    // variants; past the cap that is a pack-size decision, and past 32 it was a
    // compiler panic (DW0362).
    check_dialogue_variant_cap(plan)?;

    // Actor spawn anchors must resolve to a world position (spec-0014); a spawn is a
    // summon, not a walk, so this needs no occupancy model. DW0325 if one dangles.
    crate::compiler::nav::check_actor_placement(plan)?;

    // …and no two bodies that are in the world at the same time may be declared
    // on the same cell (DW0896). Runs here, with the anchor-resolution seals and
    // before any occupancy model, because it is arithmetic over resolved cells
    // and over a declaration the author can read: seven actors on one anchor
    // emitted seven identical `summon` lines and the build exited 0. Its binding
    // line prints whether or not it found anything — a count only says something
    // when the run that found nothing prints it too — and prints before the
    // refusal, so a refused run still states what it examined.
    // Every mark a body is put on stays inside the piece its anchor belongs to
    // (DW0897, spec-0066): an offset says where beside a place, never which
    // place. Runs before the occupancy rules that read those cells, and prints
    // its binding line on every run, zeroes included.
    let (marks, marks_verdict) = crate::compiler::mark::check_marks_in_piece(plan);
    eprintln!("{}", marks.line());
    marks_verdict.map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    let (one_mark, one_mark_verdict) = crate::compiler::cohabit::check_one_body_per_mark(plan);
    eprintln!("{}", one_mark.line());
    one_mark_verdict.map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // No body may stand on the affordance the party has to click (DW0359). Runs
    // right after the anchor-resolution seals and before any occupancy model:
    // it is pure box arithmetic over resolved cells, and it is the proof that the
    // island's giant — a 0.9 × 2.9 warden sharing `anchor/fire-pit` with two
    // interact objectives — was hiding a required beat behind its own hitbox.
    warnings.extend(
        crate::compiler::eclipse::check_body_eclipse(plan).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?,
    );

    // …and no OTHER affordance may contest the hitboxes a sealed gate arms to
    // answer a right-click (DW0422). Same box arithmetic, same tier:
    // two interaction entities in one cell is a ray-pick tie the client resolves
    // by iteration order, so one of them silently stops receiving clicks.
    crate::compiler::eclipse::check_seal_collisions(plan).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // A shortcut whose sealed side the geometry does not name (DW0425) is
    // refused BEFORE the affordance proof below, and that ordering is load-bearing
    // rather than tidy. A `use` trigger on such a gate cannot ride the door —
    // `pressable::body_at` only rides a shortcut whose sealed side resolved — so
    // it falls back to summoning its own box on the gate anchor, exactly where the
    // shortcut's `unlock` affordance may also stand. `DW0878` would then report a
    // hitbox tie that is a CONSEQUENCE of the undecidable side, sending the author
    // to move an anchor when what they have to fix is the door. Withhold the
    // downstream verdict; name the cause. (It also widens `DW0425` to a campaign
    // that assembles no world, which is a structural fault about the declaration
    // either way.)
    check_shortcut_sides(plan)?;

    // …and no two AFFORDANCES may stand on one cell either (DW0878). The three
    // proofs above each need one side of their pair to be something else — a
    // standing body for `DW0359`, a compiler-owned press set for `DW0422`, a cast
    // ledger entry for `DW0489` — so an `interact` objective and a `use` trigger
    // on one anchor were nobody's rule, and the engine's own gallery shipped
    // exactly that pair. Same box arithmetic, same tier, same authority
    // (`eclipse::affordances`).
    crate::compiler::eclipse::check_affordance_contests(plan).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // …and a recovery stake's marker is a PLACE, so every stake that can leave one
    // has to agree what a place looks like (DW0880). Same family, same tier, and
    // here for the same reason: one place holds ONE `minecraft:interaction`,
    // because the placement table is keyed on (seat, region) rather than on the
    // stake and the rule's common branch positions at a runtime death point — so
    // four stakes a death drops are four coincident boxes unless the hardware
    // belongs to the place. It does; this is what keeps its one face decidable.
    crate::compiler::stake::check_marker_faces(plan.campaign).map_err(|e| {
        BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        }
    })?;

    // …and no two bodies the party CLICKS may stand close enough that the
    // crosshair cannot tell them apart (DW0489). `DW0359` above compares a body
    // against an affordance and skips every walker; this reads the v0.7 cast
    // ledger, which states beat by beat who is on stage together, and measures
    // the pairs it names. It is the proof the island's terminal finding needed —
    // two crew NPCs declared on one cell at the cave mouth.
    warnings.extend(
        crate::compiler::crosshair::check_crosshair_contests(plan).map_err(|e| {
            BuildFailure::Diagnostic {
                code: e.code,
                message: e.message,
            }
        })?,
    );
    Ok(teleport_gate)
}

/// The checks read off the finished tree, in order: the command tree, the
/// affordance hardware, the fixture class, the stand-ins, the observer census,
/// the effect-root ledger, call-graph integrity, batch-state ownership, the
/// watch ledger and claims, and score seeding.
pub(super) fn check_finished_tree(
    plan: &Plan,
    tree: &CommandTree,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    out: &mut BuildOutput,
    warnings: &mut Vec<delvewright_dsl::Diagnostic>,
) -> Result<(), BuildFailure> {
    let ns = &plan.namespace;
    // ---- validate every emitted vanilla mcfunction ----
    let mut errors = Vec::new();
    for (path, bytes) in out.iter() {
        if is_vanilla_function(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    if !errors.is_empty() {
        return Err(BuildFailure::Validation(errors));
    }

    // ---- one tick's command chain stays under the game's limit (DW0984) ----
    // Every shipped function, with every function it calls in the same tick,
    // against `max_command_sequence_length`: past it the server stops the
    // function part-way and nothing reads the log line. Feature-blind, read off
    // the finished tree, run on every build.
    let chains = crate::compiler::chain::check(out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;
    eprintln!("{}", chains.binding());
    put_json(out, "validation/chain-length.json", &chains.to_json());

    // ---- affordance-hardware self-check (DW0420 / DW0421) ----
    // Every right-click target the compiler owns must be VISIBLE in the shipped
    // datapack, and only its own consumption may retire that visibility. Read
    // off the finished tree, so it judges the commands that actually ship.
    // See `crate::compiler::affordance` for the drowned-bell soft-lock this encodes.
    crate::compiler::affordance::check(&affordances(plan), out)?;

    // ---- fixture-class self-check (DW0545) ----
    // `DW0421` above is tag-keyed and asks who may DESTROY an affordance's
    // hardware. A region verb selects by BOX and MOVES what it finds, so it slips
    // past that entirely — which is how a lift carries a recovery stake's marker
    // away from the position its ledger recorded, after which `stk_gc_<s>` deletes
    // the marker and the wager with it. So the same rule is stated one verb wider,
    // over the emitted tree: every engine-summoned hitbox, mark and display
    // declares whether it is a PLACE or is carried by a BODY, and no box-narrowed
    // entity selector may reach a place. Feature-blind, so a region verb nobody
    // has written yet is covered by existing.
    let mut fixture_gate = crate::compiler::affordance::check_fixtures(out)?;
    // Counted off the shipped tree rather than reported by the emitter that wrote
    // them, so the ledger states what a reader can go and open.
    fixture_gate.packtests = out
        .keys()
        .filter(|p| p.starts_with("packtest-datapack/") && p.contains("/test/fixture_"))
        .count();
    put_json(out, "validation/fixture-gate.json", &fixture_gate.to_json());

    // ---- the party is seen in its own cutscenes (spec-0095, DW0971) ----
    // Every cutscene declared `present` places its stand-ins before the party
    // goes to spectator and removes them at its end; every `absent` one places
    // none. Read off the shipped tree against the declarations.
    let parties = cutscene_parties(plan);
    if !parties.is_empty() {
        let gate = crate::compiler::standin::check(ns, &parties, out)?;
        eprintln!("{}", gate.line());
        put_json(out, "validation/stand-in-gate.json", &gate.to_json());
    }

    // ---- a watcher is out of play everywhere (spec-0077 §5, DW0926) ----
    // A cutscene holds every player in the observation state, and a respawn wait
    // holds one while the rest play on. Every positional player selector in the
    // shipped tree must exclude the observation tag or stand at a site
    // `crate::compiler::observer::ALLOWED` names with its reason (an engine
    // self-check: see `crate::compiler::observer::check`). Feature-blind, read
    // off the shipped bytes, and run on every build.
    let census = crate::compiler::observer::check(out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;
    eprintln!("{}", census.binding());
    put_json(out, "validation/observer-census.json", &census.to_json());

    // ---- the effect-root walk's own binding ledger ----
    // Every other proof in this compiler publishes its binding as a
    // `validation/*.json`; the walk that underpins most of them published a
    // stderr STRING, so nothing downstream could assert it bound to anything.
    // A build whose effect walk reaches zero bundles is a build where every
    // effect-shaped proof is vacuous, and until this file existed that was not
    // a fact any gate could read (spec-0039 criterion 6).
    let root_binding =
        crate::compiler::plan::for_each_effect_root(plan.campaign, &mut |_site, _effs| {});
    put_json(out, "validation/effect-roots.json", &root_binding.to_json());

    // ---- call-graph integrity (DW0497) ----
    // Every `function <ns>:<name>` the compiler just wrote must point at a
    // function the compiler wrote. Vanilla resolves an unknown function to
    // nothing at all — no error, no log line — so an emitter whose call walk and
    // machinery walk disagree ships a verb that simply never happens. That is
    // exactly how the island's round-21 build lost two of its three storm waves
    // (see `crate::compiler::integrity`). Feature-blind and last, so it guards every
    // emitter, including ones not yet written.
    crate::compiler::integrity::check_tree(ns, out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;

    // ---- PackTest batch-state ownership (DW0807) ----
    // The generated suite runs as ONE batch on ONE shared server, so a template
    // that runs the real `tick` and asserts on a gated outcome must OWN every
    // `#party` term that gate reads — otherwise its verdict is decided by
    // whichever sibling ran last, and the campaign-playthrough template holds the
    // whole party ledger across ticks (see `crate::compiler::batchstate`). Feature-blind and
    // read off the shipped bytes, so it guards templates not yet written.
    let batch_binding =
        crate::compiler::batchstate::check_tree(ns, out).map_err(|e| BuildFailure::Diagnostic {
            code: e.code,
            message: e.message,
        })?;
    warnings.extend(batch_binding.finding());

    // ---- runtime-watch coverage of per-object bodies (DW0810) ----
    // A mechanic whose runtime body is emitted PER OBJECT gets one body per
    // declared object, over its own region, with its own judgement — so a suite
    // that drives one of them has proven nothing about the next. The timed-gate
    // emitter bound `first()` and shipped a three-gate level whose LETHAL gate
    // was the third with no runtime proof at all, green throughout (see
    // `crate::compiler::watch`). Read off the shipped bytes with no table of mechanics, so
    // it guards emitters not yet written.
    let watch_ids = crate::compiler::watch::declared_ids(input_bytes);
    let (watch_binding, unwatched) = crate::compiler::watch::check_tree(ns, out, &watch_ids);

    // ---- undischarged per-object watch claims (DW0811) ----
    // The refusal half, and it is drawn one step in from `DW0810` on purpose.
    // Nothing in the finished tree separates "the emitter meant to prove every
    // member and skipped some" from "the suite drives one exemplar by design" —
    // eight standing gallery families are honestly the second — so a refusal read
    // off the bytes alone would need a per-family allowlist, which is an opt-out
    // the defect can supply. The distinction lives in the EMITTER, so the emitter
    // registers its claim over the plan's own authored list and the claim is
    // judged against the shipped suite: `declared` cannot shrink when the walk
    // skips members, and `invoked` cannot be faked because it is read off bytes.
    let watch_claims = packtest::watch_claims(plan);
    let (claim_binding, breaches) = crate::compiler::watch::check_claims(ns, out, &watch_claims);

    put_json(
        out,
        "validation/watch-ledger.json",
        &watch_binding.to_json(&unwatched),
    );
    put_json(
        out,
        "validation/watch-claims.json",
        &claim_binding.to_json(&breaches),
    );

    if let Some(d) = crate::compiler::watch::claim_finding(&claim_binding, &breaches) {
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::watch::DW_CLAIM_NOT_DISCHARGED,
            message: d.message,
        });
    }
    warnings.extend(crate::compiler::watch::finding(&watch_binding, &unwatched));

    // ---- score-seeding integrity (DW0495) ----
    // Every `if score` / `unless score` / `scores={…}` the compiler just wrote
    // must read an entry the pack itself creates, or be written so a missing entry
    // cannot change its answer. On the pinned 1.21.11 server a score that was never
    // written is not zero — every comparison against it is false — which is how
    // `if score @s dw.deaths > @s dw.death_ack` silently swallowed every player's
    // FIRST death for as long as checkpoints have existed (see `crate::compiler::seeding`).
    // Feature-blind and read off the finished tree, beside the call-graph proof.
    crate::compiler::seeding::check_tree(ns, out).map_err(|e| BuildFailure::Diagnostic {
        code: e.code,
        message: e.message,
    })?;
    Ok(())
}
