use super::*;

/// The sealed-state test: the environment-sealing baseline (spec-0002) is
/// applied on boot.
pub(super) fn emit_sealed_state_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    // Sealed-state test: prove the environment-sealing baseline (spec-0002) is
    // applied on boot. What PackTest / vanilla 1.21.11 lets us assert in-test:
    //   * `time set noon` — the world time has a read-back path
    //     (`time query daytime` -> 6000), so it is asserted directly here.
    //   * the five gamerules — 1.21.11 gamerule *values* have NO `execute
    //     if`/predicate read-back in vanilla, so they cannot be asserted in-game.
    //     Their presence and exact 1.21.11 form is a compile-time regression
    //     instead (crates/delvec/tests/emit.rs::environment_sealing_emitted),
    //     which is the authoritative sealing assertion.
    // Verified live: `function <ns>:setup` sets daytime to 6000 and this assert
    // passes on Fabric + PackTest 2.4.0.
    let mut sealed: Vec<String> = Vec::new();
    sealed.push(format!(
        "#> {}: environment sealed on boot (spec-0002)",
        artifact_title(c)
    ));
    sealed.push("# @dummy".to_string());
    sealed.push("# @timeout 100".to_string());
    sealed.push(String::new());
    sealed.push(format!("function {ns}:setup"));
    let sealed_time = c.world.content.time;
    let sealed_clock = sealed_time.world_clock();
    let sealed_ticks = sealed_clock.daytime;
    sealed.push(format!(
        "# time set {} -> daytime {sealed_ticks} (the sole sealing command with a",
        sealed_time.token(sealed_clock)
    ));
    sealed.push("# vanilla read-back path; gamerules are asserted at compile time).".to_string());
    sealed.push(
        "execute store result score #sealtime_sealed dw.sys run time query daytime".to_string(),
    );
    sealed.push(format!(
        "assert score #sealtime_sealed dw.sys matches {sealed_ticks}"
    ));
    // spec-0081 §4.4: a celestial world states the day too — the half of the
    // clock the moon's phase is a function of — so its second read-back is
    // asserted, and, where the moon is up, the phase itself, through a
    // `time_check` predicate over the moon timeline's period. A keyword world
    // is vanilla's hour on day 0 and its bytes do not move.
    if !sealed_time.is_keyword() {
        sealed.push(format!(
            "# daytime {sealed_ticks} on day {} (dayTime {}): the moon shows {}.",
            sealed_clock.day,
            sealed_clock.absolute(),
            sealed_clock.phase().name()
        ));
        sealed.push(
            "execute store result score #sealday_sealed dw.sys run time query day".to_string(),
        );
        sealed.push(format!(
            "assert score #sealday_sealed dw.sys matches {}",
            sealed_clock.day
        ));
        if delvewright_dsl::celestial::moon_up(sealed_clock.daytime) {
            let phase = sealed_clock.phase();
            let pred = format!("moon_{}", phase.name());
            let lo = phase.index() * delvewright_dsl::celestial::DAY_TICKS;
            put_json(
                out,
                &format!("packtest-datapack/data/{ns}/predicate/{pred}.json"),
                &serde_json::json!({
                    "condition": "minecraft:time_check",
                    "period": delvewright_dsl::celestial::moon_period_ticks(),
                    "value": {
                        "min": lo,
                        "max": lo + delvewright_dsl::celestial::DAY_TICKS - 1,
                    },
                }),
            );
            sealed.push(format!(
                "execute store success score #sealmoon_sealed dw.sys if predicate {ns}:{pred}"
            ));
            sealed.push("assert score #sealmoon_sealed dw.sys matches 1".to_string());
        }
    }

    out.insert(
        format!("packtest-datapack/data/{ns}/test/sealed_state.mcfunction"),
        lines(&sealed).into_bytes(),
    );
}

/// The declared-difficulty test: the world runs at the difficulty the campaign
/// declares. Emits nothing for a campaign that declares none.
pub(super) fn emit_declared_difficulty_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    // Declared combat difficulty (v0.6): prove on a live
    // pinned server that the difficulty the campaign DECLARED is the difficulty
    // the world runs at. Unlike the gamerules, this one has a vanilla read-back:
    // the bare `/difficulty` query command returns `Difficulty#getId()`
    // (peaceful 0 / easy 1 / normal 2 / hard 3), so `execute store result` reads
    // it exactly like `time query daytime`. The assertion covers the whole chain
    // at once — the shipped `server/server.properties` (via the compose
    // profile's shared world-settings entrypoint) and the `/difficulty` in
    // `setup` must agree with the declaration, so a regression in EITHER fails
    // here. Emitted only for a campaign that declares a difficulty.
    if let Some(diff) = c.world.content.difficulty {
        let mut df: Vec<String> = Vec::new();
        df.push(format!(
            "#> {}: the world runs at the declared difficulty `{}`",
            artifact_title(c),
            diff.token()
        ));
        df.push("# @dummy".to_string());
        df.push("# @timeout 100".to_string());
        df.push(String::new());
        df.push(format!("function {ns}:setup"));
        df.push(
            "# Bare `/difficulty` is the query form: it returns Difficulty#getId()".to_string(),
        );
        df.push("# (peaceful 0 / easy 1 / normal 2 / hard 3).".to_string());
        df.push("execute store result score #difficulty dw.sys run difficulty".to_string());
        df.push(format!(
            "assert score #difficulty dw.sys matches {}",
            diff.id()
        ));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/declared_difficulty.mcfunction"),
            lines(&df).into_bytes(),
        );
    }
}
