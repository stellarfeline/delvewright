use super::*;

/// The campaign mechanism test: the whole objective -> quest -> campaign chain,
/// driven on one pinned dummy (one coherent drive, or one per branch phase).
pub(super) fn emit_campaign_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    // The completion objective + value the critical path asserts on.
    let (comp_obj, comp_val) = plan
        .critical_path
        .iter()
        .find_map(|s| match s {
            Step::AssertComplete { objective, value } => Some((objective.clone(), *value)),
            _ => None,
        })
        .unwrap_or_else(|| ("dw.campaign".to_string(), 1));

    // Mechanism test: on a dummy player, run the real generated init, activate the
    // campaign-start quests (as class selection does), drive each objective's
    // generated completion function (as the dialog `/trigger` and the reach
    // proximity check do), then assert the completion objective is set. This
    // proves the compiler's objective -> quest -> campaign chain end to end
    // without needing dialog-UI clicks or bot movement (verified live: passes on
    // Fabric + PackTest 2.4.0).
    //
    // Two structural facts of the campaign shape the template (the-wake):
    //
    //   * `campaign-complete` may sit at any nesting depth (spec-0025 / DW0481) —
    //     the-wake schedules it 250t into its closing `sequence`. A same-tick
    //     `assert` after the drive is then structurally unreachable ("got 0 on
    //     tick 0"), so a campaign whose ending has a scheduled tail must AWAIT
    //     the completion objective, with the timeout sized by the tail the
    //     emitter itself scheduled.
    //   * Declared `branch_points` make some terminal objectives mutually
    //     exclusive: driving every objective in one pass reaches a state no
    //     playthrough can (both endings fired in one tick). A branch campaign
    //     therefore drives one coherent per-branch path per phase, serialized
    //     through the vanilla scheduler.
    //
    // Both shapes open with the SAME full progression re-baseline
    // ([`campaign_progression_baseline`]) — see that function for the defect
    // that cost this its own intermittent red.
    let (pin, sel) = pin_dummy("dw_t_camp");
    let party = plan::PARTY;
    let branches: Vec<crate::compiler::branch::RealizedBranch> =
        crate::compiler::branch::realize(c)
            .into_iter()
            .filter(|r| r.world.is_some())
            .collect();
    if branches.is_empty() {
        // No declared branch points (or nothing reachable — already DW0482):
        // one coherent drive over every objective, exactly as before.
        let quests: BTreeSet<&str> = c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.id.as_str())
            .collect();
        let tail = quests_ending_tail(c, &quests, moves, actor_moves);
        // Baseline + drive. The baseline is the WHOLE progression surface
        // ([`campaign_progression_baseline`]), not just the score this template
        // asserts: the chain it drives is guarded, so a term a sibling left set
        // silently turns the drive into a no-op. spec-0018: the whole chain is
        // PARTY state, so the baseline, the activation and the assert all address
        // `#party`; the dummy is still what DRIVES it (`execute as {sel} run …`),
        // which is exactly the multiplayer claim — one player's action advances
        // the party.
        let mut drive: Vec<String> = campaign_progression_baseline(c, &comp_obj);
        for q in &c.quests.content.quests {
            for o in &q.objectives {
                drive.push(format!(
                    "execute as {sel} run function {ns}:complete_{}",
                    safe_obj_fn(o.id().as_str())
                ));
            }
        }
        let mut body: Vec<String> = Vec::new();
        body.push(format!(
            "#> {}: objective completions set {comp_obj} (Delvewright mechanism test)",
            artifact_title(c)
        ));
        body.push("# @dummy".to_string());
        // The baseline + drive live in `pt_camp_drive`, a suite `function/`
        // called on the drive tick, in BOTH shapes — the scheduled-ending one
        // because its template spans ticks, and the synchronous one because of
        // the pair below.
        //
        // `tests/packtest_batch.rs::party_state_across_ticks_is_owned` reads each
        // template's OWN text and demands that a `#party` score awaited across
        // ticks be touched by exactly one template. The whole-ledger baseline
        // ([`campaign_progression_baseline`]) touches every progression score, so
        // written inline it would refuse any campaign whose suite also awaits one
        // of them across ticks — `sched_arrive_flag`, emitted for a `move-npc`
        // whose `on_arrive` sets a flag, awaits exactly that. There would then be
        // NO green state: `DW0807` demands the baseline and that test refuses it.
        // Hoisting is the state that satisfies both, so it is unconditional
        // rather than a property of the ending's shape. The hoisted writes stay
        // atomic-with-the-drive either way — one mcfunction, one tick.
        let (close, timeout) = if tail == 0 {
            (
                format!("assert score {party} {comp_obj} matches {comp_val}"),
                100,
            )
        } else {
            // Scheduled ending: the ending lands `tail` ticks after the terminal
            // drive, so the template awaits it — never a weaker assert, since
            // `await` fails the test at timeout exactly as `assert` fails it on
            // the spot.
            (
                format!("await score {party} {comp_obj} matches {comp_val}"),
                100 + tail,
            )
        };
        body.push(format!("# @timeout {timeout}"));
        body.push(String::new());
        body.push(format!("function {ns}:setup"));
        // Pin this test's own dummy and drive the whole chain on it alone (see
        // `pin_dummy`): `@a`-wide quest/objective writes would land on every
        // sibling test's dummy in the batch, and the closing `@p` assert could
        // read a foreign one.
        body.push(pin);
        body.push(format!("function {ns}:pt_camp_drive"));
        body.push(close);
        out.insert(
            format!("packtest-datapack/data/{ns}/function/pt_camp_drive.mcfunction"),
            lines(&drive).into_bytes(),
        );
        out.insert(
            format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
            lines(&body).into_bytes(),
        );
    } else {
        emit_branch_campaign_packtest(
            plan,
            out,
            &branches,
            moves,
            actor_moves,
            (&comp_obj, comp_val),
            (&pin, &sel),
        );
    }
}

/// Ticks between one PackTest campaign phase's ending window closing and its
/// verdict being taken — slack for the scheduler landing the ending's last
/// function plus the completion write itself.
const CAMPAIGN_PHASE_MARGIN_TICKS: u32 = 20;

/// The campaign-playthrough template's opening baseline: **the whole party
/// progression ledger, set to the campaign's start state**, in one place for
/// both shapes of that template.
///
/// The completion objective, every declared flag, and every quest's
/// active/complete score plus every objective score go to 0; then the
/// campaign-start quests go active. What follows it is a drive of the real
/// completion functions, so the template plays the campaign from its beginning
/// rather than from wherever the batch happens to have left it.
///
/// ## Why the whole ledger, and not just the score the template asserts
///
/// The chain the drive runs is **guarded**: `check_q_<quest>` fires
/// `complete_q_<quest>` only `unless score #party dw.q_<quest> matches 1`, which
/// is right for the shipped campaign (a quest completes once) and fatal for a
/// template that re-drives it. `#party` is batch-global (spec-0018), and the
/// suite's own siblings reach that ledger through the campaign's real `tick` —
/// so one sibling calling `function <ns>:tick` while another sibling's dummies
/// stand in a `reach` volume can complete the terminal quest outright. The
/// campaign template then zeroes only `dw.campaign`, drives every objective, and
/// every `check_q_*` declines: **the drive becomes a silent no-op and the assert
/// reads 0 on tick 0**.
///
/// Measured, on the shipped bytes of `souls-bonfire`: PackTest runs the suite as
/// one batch in a RANDOMISED order, and in the order that reproduced it
/// `verb_kill` (which runs the real `tick`, completing `obj/slay` and starting
/// the closing cutscene, whose camera every dummy in the batch then spectates)
/// and `v04_interact_cleanup`/`verb_interact` (which complete `obj/door`) landed
/// before `campaign`; the next server tick completed `obj/shrine` on the
/// spectating dummies, `check_q_trial` fired, and `dw.campaign` was already
/// decided. The template that ran afterwards could not re-drive any of it.
///
/// The branch shape of the template has always done this — its phases re-run the
/// same campaign, so it met the wall on its second phase and fixed it there.
/// This is that fix reaching the shape that meets the wall through a *sibling*
/// instead of through its own second phase, and it is one function rather than
/// two spellings on purpose: a hand-rolled subset of a baseline is how a gate
/// gets a hole (`crate::compiler::batchstate`, `DW0807`).
///
/// ## Why this is not a new order-dependence
///
/// Every term zeroed here is written again by the drive that follows it, in the
/// same atomic `mcfunction` — the template body in the single-tick shape,
/// `pt_camp_drive` in the scheduled-ending shape, `pt_camp_run_<i>` in the
/// branch shape. Vanilla runs a function to completion before any other
/// function, so no sibling can observe the zeroed intermediate state.
fn campaign_progression_baseline(c: &delvewright_dsl::Campaign, comp_obj: &str) -> Vec<String> {
    let party = plan::PARTY;
    let mut b: Vec<String> = Vec::new();
    b.push(format!("scoreboard players set {party} {comp_obj} 0"));
    for f in declared_flags(c) {
        b.push(format!(
            "scoreboard players set {party} {} 0",
            plan::flag_score(&f)
        ));
    }
    for q in &c.quests.content.quests {
        b.push(format!(
            "scoreboard players set {party} {} 0",
            quest_score(q.id.as_str())
        ));
        b.push(format!(
            "scoreboard players set {party} {} 0",
            quest_active_score(q.id.as_str())
        ));
        for o in &q.objectives {
            b.push(format!(
                "scoreboard players set {party} {} 0",
                obj_score(o.id().as_str())
            ));
        }
    }
    for qid in campaign_start_quests(c) {
        b.push(format!(
            "scoreboard players set {party} {} 1",
            quest_active_score(qid)
        ));
    }
    b
}

/// The branch-aware campaign mechanism test: ONE template that
/// drives each reachable branch's coherent path as its own phase, serialized
/// through the vanilla scheduler, and awaits one verdict per phase.
///
/// Why one template rather than one per branch: every phase's verdict is the
/// shared completion objective, and a template that spans ticks must be the sole
/// owner of every `#party` score it depends on across ticks
/// (`tests/packtest_batch.rs::party_state_across_ticks_is_owned`) — two
/// concurrently-running branch templates zeroing and awaiting `dw.campaign`
/// would hand each other false verdicts in an order the compiler does not
/// control. Phases are strictly ordered by construction: phase *i*'s scheduled
/// check is what starts phase *i + 1*.
///
/// Each phase (`pt_camp_run_<i>`) opens with the WHOLE progression re-baseline
/// ([`campaign_progression_baseline`] — a prior phase's terminal quest would
/// otherwise stay `dw.q_* = 1` and its completion-guarded `on_complete` never
/// re-fire), then drives ONLY this
/// branch's path in play order. A `talk-to` step whose branch-scripted option
/// sets flags has those flags emulated immediately before its drive — the
/// option handler is UI-bound, and this is where the real playthrough sets
/// them. The phase's verdict is taken `tail + margin` ticks later
/// (`pt_camp_check_<i>`): completion objective at its expected value counts the
/// phase into `#camp_phase`, and the template's single closing `await` demands
/// every phase counted. A missed ending leaves the count short and the await
/// times out red — never weaker than the old assert, and now quantified over
/// branches.
#[allow(clippy::too_many_arguments)]
fn emit_branch_campaign_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    branches: &[crate::compiler::branch::RealizedBranch],
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    (comp_obj, comp_val): (&str, i32),
    (pin, sel): (&str, &str),
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let party = plan::PARTY;
    let n = branches.len();
    let mut timeout: u32 = 100;
    for (i, r) in branches.iter().enumerate() {
        let quests: BTreeSet<&str> = r.path.iter().map(|s| s.quest.as_str()).collect();
        let tail = quests_ending_tail(c, &quests, moves, actor_moves);
        let wait = tail + CAMPAIGN_PHASE_MARGIN_TICKS;
        timeout += wait;
        let mut run: Vec<String> = Vec::new();
        run.push(format!(
            "# Phase {i}: branch `{}` — full progression re-baseline, then this branch's",
            r.branch.id
        ));
        run.push(
            "# coherent path only (its scripted dialogue choices emulated as the flags".to_string(),
        );
        run.push("# those options set, at their real path positions).".to_string());
        run.extend(campaign_progression_baseline(c, comp_obj));
        for step in &r.path {
            if let Some(opt) = step.talk_option {
                for f in option_sets_flags(plan, &step.objective, opt) {
                    run.push(format!(
                        "scoreboard players set {party} {} 1",
                        plan::flag_score(f)
                    ));
                }
            }
            run.push(format!(
                "execute as {sel} run function {ns}:complete_{}",
                safe_obj_fn(&step.objective)
            ));
        }
        run.push(format!("schedule function {ns}:pt_camp_check_{i} {wait}t"));
        out.insert(
            format!("packtest-datapack/data/{ns}/function/pt_camp_run_{i}.mcfunction"),
            lines(&run).into_bytes(),
        );

        let mut chk = vec![format!(
            "execute if score {party} {comp_obj} matches {comp_val} run \
             scoreboard players add #camp_phase dw.sys 1"
        )];
        if i + 1 < n {
            chk.push(format!("function {ns}:pt_camp_run_{}", i + 1));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/function/pt_camp_check_{i}.mcfunction"),
            lines(&chk).into_bytes(),
        );
    }

    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "#> {}: each branch's coherent path sets {comp_obj} (Delvewright mechanism test)",
        artifact_title(c)
    ));
    body.push("# @dummy".to_string());
    body.push(format!("# @timeout {timeout}"));
    body.push(String::new());
    body.push(format!("function {ns}:setup"));
    body.push(pin.to_string());
    // Own init for the phase counter — on the shared batch server "never set"
    // is not 0, and `#camp_phase` belongs to this template alone.
    body.push("scoreboard players set #camp_phase dw.sys 0".to_string());
    // The phase chain: pt_camp_run_0 -> pt_camp_check_0 -> pt_camp_run_1 -> …
    // (each check is scheduled by its run and starts the next run).
    body.push(format!("function {ns}:pt_camp_run_0"));
    body.push(format!("await score #camp_phase dw.sys matches {n}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
        lines(&body).into_bytes(),
    );
}

/// The flags the branch-scripted dialogue option at flat index `n` (1-based, per
/// the NPC the `talk-to` objective names) sets when chosen — what the campaign
/// phase drive emulates in place of a UI click. Empty when the objective is not
/// a `talk-to` or names no such option.
fn option_sets_flags<'p>(plan: &'p Plan, objective: &str, n: usize) -> &'p [String] {
    let npc = plan
        .campaign
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find_map(|o| match o {
            Objective::TalkTo { id, npc, .. } if id.as_str() == objective => Some(npc.as_str()),
            _ => None,
        });
    npc.and_then(|npc_id| plan.npcs.iter().find(|p| p.npc_id == npc_id))
        .and_then(|p| p.options.iter().find(|o| o.n == n as i32))
        .map(|o| o.sets_flags.as_slice())
        .unwrap_or(&[])
}
