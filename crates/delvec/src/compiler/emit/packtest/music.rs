use super::*;

/// spec-0109 §5.3: per cutscene that declares its own `music`, one template
/// `cutscene_music_<n>` (`<n>` its index among the build's cutscenes) that
/// runs the cutscene's real paint, restore, interrupt and restart functions and
/// asserts, at every painted 4-cell's 2-mod-4 sample, the biome each leaves:
///
/// 1. the first tick re-established (the ground under every paint box, then the
///    ground under every repaint volume and the bootstrap), the judged
///    configuration's repaints run, and the base the configuration says;
/// 2. `cs_paint_<bare>` → the variant of that base, the latch at 1;
/// 3. `cs_unpaint_<bare>` → the base again, the latch at 0;
/// 4. the interrupt: `cs_paint_<bare>`, `#run_<bare>` cleared, `cs_unpaint_tick`
///    once → the base, the latch at 0;
/// 5. the restart: `cs_paint_<bare>` with its latch saved from before the
///    cutscene, `#run_<bare>` cleared, the real `load` → the latch at 1;
///    `cs_unpaint_tick` → the base, the latch at 0.
///
/// The cells and bases are the campaign's and the lifetime is the emitted
/// functions' runtime behaviour, which no Rust test executes. The music a
/// client plays is client-side: no PackTest observes it.
pub(super) fn emit_music_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let first = crate::compiler::horizon::biome_map(plan);
    let restore = super::atmosphere::first_tick_lines(plan);
    let (scores, _) = crate::compiler::music::scores(plan, moves, actor_moves);
    for score in &scores {
        let Some(start) = super::super::cutscene::cutscene_start_fn(score.effect) else {
            continue;
        };
        let bare = start.strip_prefix("cs_").unwrap_or(&start).to_string();
        let latch = super::super::cutscene::paint_latch(&bare);
        let n = score.index;
        let mut b = packtest_header(&format!(
            "{}: cutscene {n}'s own music is painted on every cell its camera can be in, \
             and every painted cell holds its original biome after the end, an interrupt \
             and a restart",
            artifact_title(c),
        ));
        b.push(format!("function {ns}:setup"));
        b.push(
            "# the music a client plays is client-side: no PackTest observes it; this template \
             reads the biome each painted cell wears"
                .to_string(),
        );
        // The first tick: outside every place and repaint a cell of a
        // PackTest world stands in whatever its generator laid, which is not
        // the delve's ground — so the ground is laid under every paint box
        // first, as the atmosphere templates lay it under every repaint.
        let boxes: Vec<([i32; 3], [i32; 3])> = score
            .row
            .boxes
            .iter()
            .map(|pb| {
                (
                    [pb.cells.0[0] * 4, pb.cells.0[1] * 4, pb.cells.0[2] * 4],
                    [
                        pb.cells.1[0] * 4 + 3,
                        pb.cells.1[1] * 4 + 3,
                        pb.cells.1[2] * 4 + 3,
                    ],
                )
            })
            .collect();
        let mut establish: Vec<String> = boxes
            .iter()
            .flat_map(|(lo, hi)| {
                crate::compiler::atmosphere::fillbiome_lines(*lo, *hi, &first.ground.id)
            })
            .collect();
        establish.extend(restore.iter().cloned());
        b.extend(establish.iter().cloned());
        b.push(format!(
            "# the configuration the forced route reaches it in: {} repaint(s) before it",
            score.repaint_volumes.len()
        ));
        for (lo, hi, biome) in &score.repaint_volumes {
            b.extend(crate::compiler::atmosphere::fillbiome_lines(
                *lo, *hi, biome,
            ));
        }
        b.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        let bases: Vec<([i32; 3], String, String)> = score
            .row
            .cells
            .iter()
            .filter_map(|q| {
                let at = crate::compiler::music::sample(*q);
                let base = score.configuration.at(at).0.to_string();
                let variant = score.variants.iter().find(|v| v.base == base)?;
                Some((at, base, variant.id.clone()))
            })
            .collect();
        let assert_bases = |tag: &str, b: &mut Vec<String>| {
            for (i, (at, base, _)) in bases.iter().enumerate() {
                super::atmosphere::biome_assert(&format!("#cm{n}_{tag}{i}"), *at, base, true, b);
            }
        };
        let assert_latch = |value: u8, b: &mut Vec<String>| {
            b.push(format!("assert score {latch} dw.sys matches {value}"));
        };
        b.push("# before the cutscene: every painted cell wears its base".to_string());
        assert_bases("b", &mut b);
        b.push(format!("function {ns}:cs_paint_{bare}"));
        b.push("# the paint: every painted cell wears its base's variant".to_string());
        for (i, (at, _, variant)) in bases.iter().enumerate() {
            super::atmosphere::biome_assert(&format!("#cm{n}_v{i}"), *at, variant, true, &mut b);
        }
        assert_latch(1, &mut b);
        b.push(format!("function {ns}:cs_unpaint_{bare}"));
        b.push("# the end: every painted cell holds its original biome".to_string());
        assert_bases("e", &mut b);
        assert_latch(0, &mut b);
        b.push(format!("function {ns}:cs_paint_{bare}"));
        b.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        b.push(format!("function {ns}:cs_unpaint_tick"));
        b.push("# the interrupt: the tick restores a paint its cutscene left".to_string());
        assert_bases("i", &mut b);
        assert_latch(0, &mut b);
        b.push(format!("function {ns}:cs_paint_{bare}"));
        b.push(
            "# the restart: chunks saved painted, scores saved from before the cutscene"
                .to_string(),
        );
        b.push(format!("scoreboard players set {latch} dw.sys 0"));
        b.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        b.push(format!("function {ns}:load"));
        assert_latch(1, &mut b);
        b.push(format!("function {ns}:cs_unpaint_tick"));
        assert_bases("r", &mut b);
        assert_latch(0, &mut b);
        b.extend(establish);
        out.insert(
            format!("packtest-datapack/data/{ns}/test/cutscene_music_{n}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The score claim (`DW0811`): every cutscene that declares its own `music`
/// owns its `cutscene_music_<n>` template.
pub(super) fn cutscene_music_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "cutscene-music",
        kind: crate::compiler::watch::ClaimKind::Template,
        families: vec!["cutscene_music_".to_string()],
        declared: crate::compiler::music::scored_ids(plan),
    }
}
