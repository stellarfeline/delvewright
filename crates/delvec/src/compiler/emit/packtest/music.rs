use super::*;

/// spec-0109 §5.2: per cutscene that declares `hears`, one template
/// `music_hears_<n>` that re-establishes the configuration the build judged it
/// in — the first tick, then every repaint the forced route fires before it,
/// the lines the verb emits — and asserts the declared biome at every judged
/// camera 4-cell's 2-mod-4 sample, the cell the client reads. Music itself is
/// client-side: the server-side fact a PackTest can observe is which biome a
/// camera cell wears.
pub(super) fn emit_music_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let map = crate::compiler::horizon::biome_map(plan);
    let restore = super::atmosphere::first_tick_lines(plan);
    // Every box a first-tick or repaint paint covers: outside all of them a
    // cell stands in whatever the PackTest world's generator laid, which is
    // not the delve's ground.
    let mut painted: Vec<([i32; 3], [i32; 3])> = map.places().map(|p| p.cells).collect();
    painted.extend(
        crate::compiler::atmosphere::set_atmospheres(c)
            .into_iter()
            .filter_map(|(_, _, eff)| crate::compiler::horizon::repaint_volume(plan, eff))
            .map(|(lo, hi)| crate::compiler::atmosphere::painted_box(lo, hi)),
    );
    let inside = |cell: [i32; 3]| {
        painted
            .iter()
            .any(|(lo, hi)| (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]))
    };
    for (n, j) in crate::compiler::music::judge_cutscenes(plan, moves, actor_moves)
        .iter()
        .enumerate()
    {
        let Some(declared) = &j.declared else {
            continue;
        };
        let mut b = packtest_header(&format!(
            "{}: the cutscene at {} {} hears {declared} at every 4-cell its camera passes",
            artifact_title(c),
            j.row.stage,
            j.row.path
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(restore.iter().cloned());
        if j.row.reached {
            b.push(format!(
                "# the configuration the forced route reaches it in: {} repaint(s) before it",
                j.repaint_volumes.len()
            ));
        } else {
            b.push(
                "# the forced route never reaches this cutscene: it is judged at the first tick"
                    .to_string(),
            );
        }
        for (lo, hi, biome) in &j.repaint_volumes {
            b.extend(crate::compiler::atmosphere::fillbiome_lines(
                *lo, *hi, biome,
            ));
        }
        b.push(
            "# `stopsound` and the music a client plays are client-side: no PackTest observes \
             them; this template reads the biome each camera cell wears"
                .to_string(),
        );
        for (i, (q, _, shot)) in j.row.cells.iter().enumerate() {
            let at = crate::compiler::music::sample(*q);
            if *declared == map.ground.id && !inside(at) {
                // Outside every paint the cell stands in whatever the PackTest
                // world's generator laid, which is not the delve's ground: the
                // reading is that no atmosphere stands there.
                b.push(format!(
                    "# shot {shot} cell {q:?}: the ground, outside every paint — no atmosphere \
                     stands here"
                ));
                for (k, a) in c.world.content.atmospheres.iter().enumerate() {
                    let biome = crate::compiler::atmosphere::biome_id(ns, a.id.as_str());
                    super::atmosphere::biome_assert(
                        &format!("#mh{n}_{i}_{k}"),
                        at,
                        &biome,
                        false,
                        &mut b,
                    );
                }
                continue;
            }
            b.push(format!("# shot {shot} cell {q:?}"));
            super::atmosphere::biome_assert(&format!("#mh{n}_{i}"), at, declared, true, &mut b);
        }
        b.extend(restore.iter().cloned());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/music_hears_{n}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The hearing claim (`DW0811`): every cutscene that declares `hears` owns its
/// `music_hears_<n>` template.
pub(super) fn music_hears_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "music-hears",
        kind: crate::compiler::watch::ClaimKind::Template,
        families: vec!["music_hears_".to_string()],
        declared: crate::compiler::music::hearing_ids(plan),
    }
}
