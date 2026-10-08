use super::*;

/// The flag a [`SCHEDULED_PROBE`] `set-flag` sets. Test-only: it exists solely in
/// the PackTest datapack, never in the shipped delve.
const SCHEDULED_PROBE_FLAG: &str = "flag/pt-sched-probe";

/// The PackTest-datapack function the scheduled-executor probe schedules.
const SCHEDULED_PROBE: &str = "pt_sched_probe";

/// PackTests for the scheduled-executor contract (AUDIT-P0).
///
/// `schedule function …` re-invokes a function with the **server** command
/// source — no executor, so every `@s`-addressed command in it silently does
/// nothing. Two templates, because one alone would not have caught the bug:
///
/// 1. `sched_executor` — **unconditional**, so every campaign (hello-world in
///    CI tier 2 included) proves the seam live on a real server. A probe
///    function in the PackTest datapack, emitted by the *real* scheduled-bundle
///    emitter ([`emit_effect_bundle`] with [`Audience::Scheduled`]) over a
///    `set-flag`, is handed to the vanilla scheduler; the test then awaits the
///    flag on its own dummy's score. Pre-fix output emits `scoreboard players
///    set @s …` here and the await times out.
/// 2. `sched_arrive_flag` — the content path, for the first `move-npc` whose
///    `on_arrive` sets a flag (the island's stealth beat). It runs the REAL
///    start function and lets the driver walk itself to the end through the
///    scheduler. The pre-existing arrive templates all call `mv_tick`/`ma_tick`
///    *inline as the dummy*, which supplies exactly the player executor the
///    scheduler does not — that is how this bug survived a green suite.
pub(super) fn emit_scheduled_executor_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let probe_score = plan::flag_score(SCHEDULED_PROBE_FLAG);

    // --- 1. the unconditional probe -------------------------------------
    // The probe body goes through the real emitter, so it carries whatever the
    // scheduled-bundle seam currently produces — this template is a live test
    // OF that seam, not a restatement of it.
    let probe = emit_effect_bundle(
        plan,
        &[delvewright_dsl::QuestEffect::from(
            delvewright_dsl::Verb::SetFlag {
                flag: delvewright_dsl::FlagId(SCHEDULED_PROBE_FLAG.to_string()),
            },
        )],
        Audience::Scheduled,
    );
    out.insert(
        format!("packtest-datapack/data/{ns}/function/{SCHEDULED_PROBE}.mcfunction"),
        lines(&probe).into_bytes(),
    );
    let mut t = packtest_header(&format!(
        "{title}: a SCHEDULED function still reaches the party (scheduled-executor contract)"
    ));
    t.push(format!("function {ns}:setup"));
    // Own init: the probe objective is test-only, so this template creates it and
    // clears its own dummy (never assume 0 on the shared batch server).
    t.push(format!("scoreboard objectives add {probe_score} dummy"));
    // spec-0018: the probe flag is party state, so the baseline and the await
    // both address `#party`. The objective is test-only (it exists solely in the
    // PackTest datapack), so this template is its sole owner in the batch — the
    // ownership `tests/packtest_batch.rs` demands of any template that awaits.
    t.push(format!(
        "scoreboard players set {} {probe_score} 0",
        plan::PARTY
    ));
    // The real scheduler, the real emitted bundle. Not an inline call: an inline
    // call would run as this test's dummy and pass even with the bug present.
    t.push(format!("schedule function {ns}:{SCHEDULED_PROBE} 2t"));
    t.push(format!(
        "await score {} {probe_score} matches 1",
        plan::PARTY
    ));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/sched_executor.mcfunction"),
        lines(&t).into_bytes(),
    );

    // --- 2. the content path: a move-npc arrival that sets a flag --------
    let arrival = moves.iter().find_map(|m| {
        all_campaign_effects(plan.campaign)
            .into_iter()
            .find_map(|e| match &e.verb {
                Verb::MoveNpc {
                    npc, to, on_arrive, ..
                } if npc.as_str() == m.npc && *to == m.to => on_arrive
                    .iter()
                    .find_map(|a| match &a.verb {
                        Verb::SetFlag { flag, .. } => Some(flag.as_str().to_string()),
                        _ => None,
                    })
                    .map(|flag| (m, flag)),
                _ => None,
            })
    });
    let Some((m, flag)) = arrival else { return };
    let bare = movenpc_bare(&m.npc, &m.to, &m.gate_key);
    let score = plan::flag_score(&flag);

    // The walk is real, so the test must outlive it: the driver reschedules
    // itself once per waypoint tick.
    let mut t = vec![
        format!(
            "#> {title}: move-npc `{}` arrival sets `{flag}` through its SCHEDULED driver",
            m.npc
        ),
        "# @dummy".to_string(),
        format!("# @timeout {}", m.ticks() + 100),
        String::new(),
    ];
    t.push(format!("function {ns}:setup"));
    // Own init: clear the party flag this template alone awaits, and release the
    // driver's re-entry latch (a sibling template may have left it armed).
    t.push(format!("scoreboard players set {} {score} 0", plan::PARTY));
    t.push(format!("scoreboard players set #mrun_{bare} dw.sys 0"));
    // The REAL start function: it schedules `mv_tick_<bare>`, which walks itself
    // to the final waypoint and fires `mv_arrive_<bare>` — every hop through the
    // scheduler, with the server command source the bug hid behind. The dummy
    // stands still throughout; nothing here supplies it as an executor.
    t.push(format!(
        "function {ns}:{}",
        movenpc_fn(&m.npc, &m.to, &m.gate_key)
    ));
    t.push(format!("await score {} {score} matches 1", plan::PARTY));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/sched_arrive_flag.mcfunction"),
        lines(&t).into_bytes(),
    );
}
