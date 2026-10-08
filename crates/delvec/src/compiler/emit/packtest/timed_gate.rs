use super::*;

/// spec-0016 §4 timed-gate PackTest: the emitted clock really alternates the gate
/// region on a live server. A fake player cannot wait out a `schedule` inside a
/// plain mcfunction, so this drives the two halves of the ping-pong directly —
/// which IS the clock's body — and asserts the region's state after each. That is
/// the machine-checkable half of "a deterministic clock over the gate region";
/// the *timing* half is the compile-time `DW0378` proof, which needs no server.
/// Emits nothing for a campaign with no timed gate.
/// Pin the jam score a disarmable gate's clock is guarded by, so a template never
/// runs against a jam a sibling left behind.
///
/// PackTest shares one server across every generated template and gives no
/// ordering guarantee, so a persistent score is shared mutable state between
/// tests. `souls_timed_gate_disarm` deliberately ends DISARMED — that is its
/// subject — and any sibling that calls `tgate_close_` afterwards finds the call
/// swallowed by the jam guard. Emitting nothing for a gate with no `disarm` keeps
/// those campaigns byte-identical.
fn pin_tgdis(b: &mut Vec<String>, g: &crate::compiler::plan::TimedGatePlan) {
    if g.disarm.is_some() {
        b.push(format!("scoreboard players set #tgdis_{} dw.sys 0", g.safe));
    }
}

pub(super) fn emit_timed_gate_packtest(plan: &Plan, out: &mut BuildOutput) {
    // EVERY declared gate, not the first. A gate's clock is emitted per gate —
    // `tgate_open_<id>`/`tgate_close_<id>` are distinct bodies over distinct
    // regions with distinct blocks and its own `crush` judgement — so watching
    // one gate says nothing whatever about the next. Binding `first()` here gave
    // a three-gate campaign whose LETHAL gate was the third exactly one runtime
    // proof, of the harmless one, and the suite was green throughout. That is the
    // unbound-gate vacuity mode with a subtler surface: it binds one object and
    // reports honestly about that one, while the set it covers has N members.
    for g in &plan.timed_gates {
        emit_one_timed_gate_packtest(plan, g, out);
        emit_timed_gate_crush_packtest(plan, g, out);
        emit_timed_gate_disarm_packtest(plan, g, out);
    }
}

/// The claim the loop above makes, judged against the shipped bytes by
/// `DW0811`. It is written from `plan.timed_gates` and NOT from whatever the
/// loop happened to walk, which is the whole point: a walk that stops at
/// `first()` still declares three gates here, so the refusal fires on exactly
/// the defect that a comment asking the next author to loop would not have
/// stopped. `DW0810` reads the same tree with no mechanic named at all and
/// stays a warning; this is the half that can refuse, because the emitter's own
/// claim is a proof obligation the defect cannot discharge.
pub(super) fn timed_gate_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "timed-gate",
        families: vec![
            "tgate_open_".to_string(),
            "tgate_close_".to_string(),
            "tgate_disarm_".to_string(),
        ],
        declared: plan.timed_gates.iter().map(|g| g.safe.clone()).collect(),
    }
}

/// One gate's alternation template. Every scratch score is suffixed with the
/// gate's own safe id: the suite is ONE batch on ONE server with no ordering
/// guarantee between templates, so a score shared across sibling gates would be
/// written by one template and asserted by another (`crate::compiler::batchstate`).
fn emit_one_timed_gate_packtest(plan: &Plan, g: &plan::TimedGatePlan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let id = &g.safe;
    let (from, to) = g.gate_region;
    let probe = from;
    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` alternates its region (spec-0016 §4)",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // Seal first: `setup` may have already run the clock's opening move, and a
    // sibling template shares this server.
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
    ));
    // …and un-jam, for the same reason the fill exists. `souls_timed_gate_disarm`
    // ends with the gate DISARMED and never restores it, `#tgdis_<id>` persists on
    // the shared server, and PackTest does not order siblings — so whenever disarm
    // runs first, this template's `tgate_close_` is swallowed by its own jam guard
    // and the re-seal assertion reads air. A template never inherits the state a
    // sibling left (the flag-leak class); it pins what it depends on.
    pin_tgdis(&mut b, g);
    b.push(format!(
        "execute store success score #tg_sealed_{id} dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push(format!("assert score #tg_sealed_{id} dw.sys matches 1"));
    b.push(format!("function {ns}:tgate_open_{id}"));
    b.push(format!(
        "execute store success score #tg_open_{id} dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push(format!("assert score #tg_open_{id} dw.sys matches 1"));
    b.push(format!("function {ns}:tgate_close_{id}"));
    b.push(format!(
        "execute store success score #tg_shut_{id} dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push(format!("assert score #tg_shut_{id} dw.sys matches 1"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate_{id}.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// The disarm PackTest: a **disarmed** gate stays open across several former cycle
/// boundaries, and its closing edge never fires again.
///
/// A fake player cannot wait out a `schedule`, so the template does what the
/// timed-gate template already does — drives the REAL clock functions directly,
/// which IS the clock's body. The proof is that after `tgate_disarm_<id>` runs,
/// calling `tgate_close_<id>` (the exact function the schedule would have
/// re-entered) leaves the span air, three former boundaries in a row. If the
/// guard were missing, the very first one would re-seal it.
///
/// A `crush: true` gate gets the sharper form for free: the same guarded body
/// carries the judgement, so a close that cannot fill also cannot damage — which
/// is why the compiler unit test asserts the damage line is *inside* the guard
/// rather than beside it.
///
/// Emits nothing unless the gate declares a `disarm`, so every other campaign's
/// PackTest suite is byte-identical.
fn emit_timed_gate_disarm_packtest(plan: &Plan, g: &plan::TimedGatePlan, out: &mut BuildOutput) {
    if g.disarm.is_none() {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (from, to) = g.gate_region;
    let probe = from;
    let id = &g.safe;
    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` stays open once disarmed",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // A sibling template shares this world: re-arm and re-seal so the fixture
    // starts from a running, shut clock whatever ran before it.
    b.push(format!("scoreboard players set #tgdis_{id} dw.sys 0"));
    b.push(format!(
        "fill {} {} {} {} {} {} {}",
        from[0], from[1], from[2], to[0], to[1], to[2], g.gate_block
    ));
    // The clock is still live: a close really does seal.
    b.push(format!("function {ns}:tgate_open_{id}"));
    b.push(format!("function {ns}:tgate_close_{id}"));
    b.push(format!(
        "execute store success score #tgd_armed_{id} dw.sys if block {} {} {} {}",
        probe[0], probe[1], probe[2], g.gate_block
    ));
    b.push(format!("assert score #tgd_armed_{id} dw.sys matches 1"));
    // Pull the lever: the span clears and the sentinel latches.
    b.push(format!("function {ns}:tgate_disarm_{id}"));
    b.push(format!(
        "execute store success score #tgd_jam_{id} dw.sys if block {} {} {} minecraft:air",
        probe[0], probe[1], probe[2]
    ));
    b.push(format!("assert score #tgd_jam_{id} dw.sys matches 1"));
    b.push(format!("assert score #tgdis_{id} dw.sys matches 1"));
    // Three former cycle boundaries. The assertion lands immediately after the
    // CLOSE — before the open half runs — because that is the only place the
    // guard is load-bearing: an unguarded close re-seals here, and an assertion
    // taken after the following open would be satisfied either way and prove
    // nothing (measured: the template that asserted after the open passed
    // against a deliberately unguarded build).
    for n in 1..=3 {
        b.push(format!("function {ns}:tgate_close_{id}"));
        b.push(format!(
            "execute store success score #tgd_c{n}_{id} dw.sys if block {} {} {} minecraft:air",
            probe[0], probe[1], probe[2]
        ));
        b.push(format!("assert score #tgd_c{n}_{id} dw.sys matches 1"));
        // …and the open half of the dead ping-pong is a harmless no-op.
        b.push(format!("function {ns}:tgate_open_{id}"));
        b.push(format!(
            "execute store success score #tgd_o{n}_{id} dw.sys if block {} {} {} minecraft:air",
            probe[0], probe[1], probe[2]
        ));
        b.push(format!("assert score #tgd_o{n}_{id} dw.sys matches 1"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate_disarm_{id}.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §4 addendum PackTest: a `crush: true` gate's closing edge selects
/// **exactly** the players standing in its region.
///
/// ## Why this asserts scoping rather than death
///
/// The obvious test — put a dummy in the gate, shut it, assert a corpse — cannot
/// be written. **PackTest fake players are immune to `/damage`** (measured live
/// on the pinned toolserver, 2026-08-03: a `# @dummy` reports
/// `playerGameType: 0` (survival), yet `damage @s 1000 minecraft:generic` leaves
/// `Health` at exactly 20.0, and an explicit `gamemode survival @s` first does
/// not change that). A lethality assertion against a dummy is therefore
/// permanently red no matter how correct the engine is. This is the same
/// limitation that already pushed the `damage-players` PackTest onto a zombie
/// dummy — and a zombie cannot stand in here, because the crush selects `@a`.
///
/// So the runtime rung proves the half it genuinely can, and the other halves are
/// proven where they can be proven honestly:
///
/// * **scoping** (here, live, on real assembled geometry) — the emitted selector
///   contains the player when they stand in the gate and excludes them when they
///   step clear. The selector string is the *same* one `tgate_close_<id>` runs.
/// * **lethality + ordering** — compiler unit tests assert the exact
///   `execute as @a[…] run damage @s 1000 minecraft:generic` and that it precedes
///   the `fill`.
/// * **end-to-end death** — verified live against a real mineflayer client on
///   pinned 1.21.11: parked two blocks clear of the region a player survives 30 s
///   of repeated closing ticks at full health, and one closing tick with the same
///   player standing inside kills them.
///
/// The test binds `@s`, not `@a`, on purpose: PackTest runs the whole suite in
/// ONE shared world, so a sibling template's dummy standing in the same fixture
/// cell would otherwise be counted. Emits nothing unless the gate opts in, so a
/// non-crushing campaign's PackTest suite is byte-identical.
fn emit_timed_gate_crush_packtest(plan: &Plan, g: &plan::TimedGatePlan, out: &mut BuildOutput) {
    if !g.crush {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let id = &g.safe;
    let (from, to) = g.gate_region;
    let selector = region_selector(from, to);
    // Feet-centred on one cell of the region: provably inside the selector box.
    let inside = crate::compiler::nav::cell_center(from);
    // Two blocks past the region's far x edge: provably outside it, and checked in
    // the same tick as the teleport so no fall or suffocation can confound it.
    let clear_x = from[0].max(to[0]) + 2;

    let mut b = packtest_header(&format!(
        "{title}: timed gate `{}` judges exactly the players in its region (spec-0016 §4)",
        g.id
    ));
    b.push(format!("function {ns}:setup"));
    // Un-jam first, for the same reason the base template does: a jam left by
    // `souls_timed_gate_disarm` swallows the `tgate_close_` this test crushes
    // with, and the crush that never happens reads as a lethality failure.
    pin_tgdis(&mut b, g);
    // Open first: a mistimed crossing leaves the player standing in an open
    // gateway, which is the position the judgement must catch.
    b.push(format!("function {ns}:tgate_open_{id}"));
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(inside[0]),
        fmt_f64(inside[1]),
        fmt_f64(inside[2])
    ));
    b.push(format!(
        "execute store success score #cr_in_{id} dw.sys if entity @s[{selector}]"
    ));
    b.push(format!("assert score #cr_in_{id} dw.sys matches 1"));
    b.push(format!(
        "tp @s {} {} {}",
        fmt_f64(f64::from(clear_x) + 0.5),
        fmt_f64(inside[1]),
        fmt_f64(inside[2])
    ));
    b.push(format!(
        "execute store success score #cr_out_{id} dw.sys if entity @s[{selector}]"
    ));
    b.push(format!("assert score #cr_out_{id} dw.sys matches 0"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_timed_gate_crush_{id}.mcfunction"),
        lines(&b).into_bytes(),
    );
}
