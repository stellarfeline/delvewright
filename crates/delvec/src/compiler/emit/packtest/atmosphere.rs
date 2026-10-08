use super::*;

/// A block inside `cells` (a 4-aligned box) whose coordinates are all 2 mod 4,
/// nearest its centre. Vanilla's biome lookup (`BiomeManager.getBiome`)
/// jitters each sample by under half a 4-cell, so a block at 2 mod 4 always
/// reads the cell it is in — the one place a reading is the cell's own.
fn quart_sample(cells: ([i32; 3], [i32; 3])) -> [i32; 3] {
    std::array::from_fn(|i| {
        let centre = (cells.0[i] + cells.1[i]).div_euclid(2);
        crate::compiler::atmosphere::quantize(centre) + 2
    })
}

/// A loaded block just outside `cells`, at 2 mod 4, beside `inside`: the first
/// of the six faces whose column a placed piece covers (world setup
/// force-loads every piece's columns) and that no box of `avoid` holds. The
/// faces are tried at the cell's own distance first, then one 4-cell further out
/// at a time, so a neighbour's paint on the near cell moves the reading rather
/// than dropping it. `None` when nothing qualifies.
fn quart_outside(
    plan: &Plan,
    cells: ([i32; 3], [i32; 3]),
    inside: [i32; 3],
    avoid: &[([i32; 3], [i32; 3])],
) -> Option<[i32; 3]> {
    let loaded = |c: [i32; 3]| {
        (crate::compiler::horizon::BUILD_MIN_Y..=crate::compiler::horizon::BUILD_MAX_Y)
            .contains(&c[1])
            && plan.placed_pieces().any(|p| {
                let (lo, hi) = p.bbox();
                lo[0] <= c[0] && c[0] <= hi[0] && lo[2] <= c[2] && c[2] <= hi[2]
            })
    };
    let held = |c: [i32; 3]| {
        avoid
            .iter()
            .any(|b| (0..3).all(|k| b.0[k] <= c[k] && c[k] <= b.1[k]))
    };
    [0, 4, 8, 16, 32].into_iter().find_map(|reach| {
        [
            (0, true),
            (0, false),
            (2, true),
            (2, false),
            (1, true),
            (1, false),
        ]
        .into_iter()
        .map(|(axis, up)| {
            let mut c = inside;
            c[axis] = if up {
                cells.1[axis] + 3 + reach
            } else {
                cells.0[axis] - 2 - reach
            };
            c
        })
        .find(|&c| loaded(c) && !held(c))
    })
}

/// A box of cells, corner to corner, inclusive.
type CellBox = ([i32; 3], [i32; 3]);

/// The lines that put the ground biome back under every repaint volume of the
/// build: the first tick, before the carried places are painted over it again.
/// Every atmosphere template starts from it, so none reads a cell a sibling's
/// paint is standing on.
fn restore_first_tick(volumes: &[(usize, CellBox)], ground: &str) -> Vec<String> {
    volumes
        .iter()
        .flat_map(|(_, (min, max))| {
            crate::compiler::atmosphere::fillbiome_lines(*min, *max, ground)
        })
        .collect()
}

/// One `execute if|unless biome` reading bridged onto a score the template
/// owns, and the assertion over it (`assert score`, the PackTest condition
/// every generated template uses).
fn biome_assert(holder: &str, cell: [i32; 3], biome: &str, holds: bool, out: &mut Vec<String>) {
    let mode = if holds { "if" } else { "unless" };
    out.push(format!("scoreboard players set {holder} dw.sys 0"));
    out.push(format!(
        "execute {mode} biome {} {} {} {biome} run scoreboard players set {holder} dw.sys 1",
        cell[0], cell[1], cell[2]
    ));
    out.push(format!("assert score {holder} dw.sys matches 1"));
}

/// spec-0080 §5.2: PackTest asserts the server's own reading of the biome map.
///
/// `atmosphere_places` paints every carried place through the bootstrap
/// function and reads, for each, a cell inside (`execute if biome`) and a cell
/// just outside (`execute unless biome`). `atmosphere_repaint_<n>` reads, for
/// each `set-atmosphere` with a resolvable volume, the first-tick biome inside
/// it, runs exactly the lines the verb emits, reads the new biome inside and the
/// unchanged one outside, and restores the first tick — every template its own
/// scores and its own init, and none awaits, so each lands inside one tick.
pub(super) fn emit_atmosphere_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let map = crate::compiler::horizon::biome_map(plan);
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };
    let places: Vec<&crate::compiler::horizon::Paint> = map.places().collect();
    let ground = map.ground.id.clone();
    // Every volume a repaint of this build paints, the writers any test can
    // meet: the repaint templates' own, and the volumes the triggers that run
    // the same beat leave painted. Keyed by the repaint's index.
    let volumes: Vec<(usize, CellBox)> = crate::compiler::atmosphere::set_atmospheres(c)
        .into_iter()
        .enumerate()
        .filter_map(|(n, (_, _, eff))| {
            let (min, max) = crate::compiler::horizon::repaint_volume(plan, eff)?;
            Some((n, (min, max)))
        })
        .collect();
    if !places.is_empty() {
        let mut b = packtest_header(&format!(
            "{}: every carried place stands in its atmosphere from the first tick",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // The first tick, re-established before anything is read: a sibling
        // template — a repaint's, or a trigger's that runs a beat which paints —
        // may have left its own biome in any repaint volume, and the templates
        // of one build run in an order this one does not choose.
        b.extend(restore_first_tick(&volumes, &ground));
        b.push(format!("function {ns}:{ATMOSPHERE_BOOTSTRAP_FN}"));
        for (i, p) in places.iter().enumerate() {
            let crate::compiler::horizon::PaintSource::Place { place, .. } = &p.source else {
                continue;
            };
            let inside = quart_sample(p.cells);
            b.push(format!("# {place}: {}", p.biome));
            biome_assert(&format!("#atm_in{i}"), inside, &p.biome, true, &mut b);
            // Outside the place: in no repaint volume (a beat's paint may stand
            // there) and in no other place's paint under the same sky (the
            // neighbour's paint is that very biome).
            let mut avoid: Vec<([i32; 3], [i32; 3])> = volumes.iter().map(|(_, v)| *v).collect();
            avoid.extend(
                places
                    .iter()
                    .enumerate()
                    .filter(|(j, o)| *j != i && o.biome == p.biome)
                    .map(|(_, o)| o.cells),
            );
            if let Some(outside) = quart_outside(plan, p.cells, inside, &avoid) {
                biome_assert(&format!("#atm_out{i}"), outside, &p.biome, false, &mut b);
            } else {
                b.push(format!(
                    "# {place}: no cell outside it stands clear of every repaint and of its \
                     neighbours under {}: no reading outside is made",
                    p.biome
                ));
            }
        }
        write("atmosphere_places", b);
    }
    for (n, (_, path, eff)) in crate::compiler::atmosphere::set_atmospheres(c)
        .into_iter()
        .enumerate()
    {
        let Some((min, max)) = crate::compiler::horizon::repaint_volume(plan, eff) else {
            continue;
        };
        let Verb::SetAtmosphere { atmosphere, .. } = &eff.verb else {
            continue;
        };
        let biome = map.biome_of(atmosphere.as_ref().map(|a| a.as_str()));
        let cells = crate::compiler::atmosphere::painted_box(min, max);
        let inside = quart_sample(cells);
        let mut b = packtest_header(&format!(
            "{}: set-atmosphere at quests {path} repaints {min:?}..{max:?} with {biome}",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // The first tick, re-established: the ground under the volume, then
        // every carried place — a sibling template may have left its own paint.
        let restore: Vec<String> = restore_first_tick(&volumes, &ground)
            .into_iter()
            .chain((!places.is_empty()).then(|| format!("function {ns}:{ATMOSPHERE_BOOTSTRAP_FN}")))
            .collect();
        b.extend(restore.iter().cloned());
        let (before, _) = map.at(inside);
        b.push("# before the beat".to_string());
        biome_assert(&format!("#atm_r{n}_before"), inside, before, true, &mut b);
        b.extend(crate::compiler::atmosphere::fillbiome_lines(
            min, max, &biome,
        ));
        b.push("# after the beat".to_string());
        biome_assert(&format!("#atm_r{n}_after"), inside, &biome, true, &mut b);
        // Outside the volume the repaint reached nothing. A carried place's
        // biome there is the bootstrap's, so it is read as itself; the ground's
        // is whatever the server's generator laid (a PackTest world is not the
        // delve's own), so there the reading is that the new biome did not
        // arrive — and when the new biome IS the ground, nothing outside can
        // tell the two apart and no reading is made.
        // The cell read stands in no OTHER repaint's volume: a template that
        // runs a beat leaves its paint standing, and this reading would meet it.
        let others: Vec<([i32; 3], [i32; 3])> = volumes
            .iter()
            .filter(|(m, _)| *m != n)
            .map(|(_, (lo, hi))| crate::compiler::atmosphere::painted_box(*lo, *hi))
            .collect();
        let outside = quart_outside(plan, cells, inside, &others);
        if outside.is_none() {
            b.push(
                "# no cell outside this volume stands clear of every other repaint's: no \
                 reading outside is made"
                    .to_string(),
            );
        }
        if let Some(outside) = outside {
            let carried = map
                .places()
                .filter(|p| {
                    (0..3).all(|i| p.cells.0[i] <= outside[i] && outside[i] <= p.cells.1[i])
                })
                .last();
            match carried {
                Some(p) => {
                    biome_assert(&format!("#atm_r{n}_kept"), outside, &p.biome, true, &mut b)
                }
                None if biome != ground => {
                    biome_assert(&format!("#atm_r{n}_kept"), outside, &biome, false, &mut b)
                }
                None => b.push(format!(
                    "# outside {outside:?} stands in the ground biome, which this repaint also \
                     paints: no reading outside can tell them apart"
                )),
            }
        }
        b.extend(restore);
        write(&format!("atmosphere_repaint_{n}"), b);
    }
}
