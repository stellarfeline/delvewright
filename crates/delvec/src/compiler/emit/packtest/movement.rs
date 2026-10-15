use super::*;

/// The cell the rig is built from: over the world's origin, over every
/// campaign's build and under the world's ceiling (`319`), so it overlaps no
/// placed piece and no other template. Absolute and small, so a `Pos` read at
/// [`SCALE`] stays inside a score — a test's own position need not.
const RIG_ORIGIN: [i32; 3] = [0, 300, 0];

/// The scale a `Pos` reading is taken at: one unit is the tolerance.
const SCALE: f64 = 10_000.0;

/// Where the dummy stands while the bodies move, relative to the rig's origin:
/// over it, outside its box.
const STAND: [i32; 3] = [7, 12, 12];

/// The function the template schedules until the rig's chunks are loaded.
const WAIT: &str = "movement_ballistics_wait";

/// spec-0113 §8.2 (A4): **the simulator held to the pinned jar's own movement
/// code.** One template for every build, because what it measures is the
/// engine's simulator, not the campaign: the rig of
/// [`delvewright_dsl::metrics::body::ballistics`] is built
/// at [`RIG_ORIGIN`] once its chunks are loaded, four `minecraft:armor_stand`
/// bodies are summoned with the throws' `Motion`,
/// and every tick for sixty ticks each body's `Pos` is asserted within the
/// instrument's tolerance of the trajectory the simulator computed. The
/// template clears its rig, its bodies and its force-load when it is done.
pub(super) fn emit_movement_packtest(plan: &Plan, out: &mut BuildOutput) {
    use delvewright_dsl::metrics::body::ballistics::{
        RIG_BOX, THROWS, TICKS, TOLERANCE, rig, trajectories,
    };
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (pin, dummy) = pin_dummy("dw_mb_dummy");
    let o = RIG_ORIGIN;
    let (lo, hi) = RIG_BOX;
    let abs = |c: [i32; 3]| [o[0] + c[0], o[1] + c[1], o[2] + c[2]];
    let (alo, ahi) = (abs(lo), abs(hi));
    let fill_air = format!(
        "fill {} {} {} {} {} {} minecraft:air",
        alo[0], alo[1], alo[2], ahi[0], ahi[1], ahi[2]
    );
    let forceload = format!("{} {} {} {}", alo[0], alo[2], ahi[0], ahi[2]);
    let mut b = vec![
        format!(
            "#> {title}: the engine's simulator holds on the pinned server (movement_ballistics)"
        ),
        "# @dummy".to_string(),
        format!("# @timeout {}", TICKS + 100),
        String::new(),
        pin,
        format!("function {ns}:setup"),
        "kill @e[tag=dw_mb]".to_string(),
        format!("forceload add {forceload}"),
        "scoreboard players set #mbl dw.sys 0".to_string(),
        format!("function {ns}:{WAIT}"),
        "await score #mbl dw.sys matches 1".to_string(),
        fill_air.clone(),
        // The dummy stands over the rig while it runs, so the rig's chunks
        // tick as the chunks round a player do.
        format!(
            "setblock {} {} {} minecraft:barrier",
            o[0] + STAND[0],
            o[1] + STAND[1] - 1,
            o[2] + STAND[2]
        ),
        format!(
            "tp {dummy} {}.5 {} {}.5",
            o[0] + STAND[0],
            o[1] + STAND[1],
            o[2] + STAND[2]
        ),
    ];
    // A chunk force-loaded on this tick is not loaded yet; the template waits
    // for both corners of the rig the way the lightning template waits for its
    // strike's chunk.
    let corners = [alo, ahi]
        .iter()
        .map(|c| format!("if loaded {} {} {}", c[0], c[1], c[2]))
        .collect::<Vec<_>>()
        .join(" ");
    let mut wait = vec![format!(
        "execute {corners} run scoreboard players set #mbl dw.sys 1"
    )];
    wait.push(format!(
        "execute if score #mbl dw.sys matches 0 run schedule function {ns}:{WAIT} 1t"
    ));
    out.insert(
        format!("packtest-datapack/data/{ns}/function/{WAIT}.mcfunction"),
        lines(&wait).into_bytes(),
    );
    for (c, block) in rig() {
        let a = abs(c);
        b.push(format!(
            "setblock {} {} {} {}",
            a[0],
            a[1],
            a[2],
            block.state()
        ));
    }
    for (i, t) in THROWS.iter().enumerate() {
        b.push(format!(
            "summon minecraft:armor_stand {} {} {} {{Tags:[\"dw_mb\",\"dw_mb{i}\"],\
             Motion:[{}d,{}d,{}d]}}",
            f64::from(o[0]) + t.start[0],
            f64::from(o[1]) + t.start[1],
            f64::from(o[2]) + t.start[2],
            t.motion[0],
            t.motion[1],
            t.motion[2]
        ));
    }
    let paths = trajectories();
    for tick in 0..TICKS {
        b.push("await delay 1t".to_string());
        for (i, path) in paths.iter().enumerate() {
            let p = path[tick];
            // A body that is gone reads as zero; say so by name instead.
            b.push(format!("assert entity @e[tag=dw_mb{i},limit=1]"));
            for (a, axis) in ["x", "y", "z"].iter().enumerate() {
                let hold = format!("#mb{i}{axis}");
                b.push(format!(
                    "execute store result score {hold} dw.sys run data get entity \
                     @e[tag=dw_mb{i},limit=1] Pos[{a}] {SCALE}"
                ));
                let v = p[a] + f64::from(o[a]);
                let lo = ((v - TOLERANCE) * SCALE).floor() as i64;
                let hi = ((v + TOLERANCE) * SCALE).floor() as i64;
                b.push(format!("assert score {hold} dw.sys matches {lo}..{hi}"));
            }
        }
    }
    b.push(fill_air);
    b.push(format!(
        "setblock {} {} {} minecraft:air",
        o[0] + STAND[0],
        o[1] + STAND[1] - 1,
        o[2] + STAND[2]
    ));
    b.push("kill @e[tag=dw_mb]".to_string());
    b.push(format!("forceload remove {forceload}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/movement_ballistics.mcfunction"),
        lines(&b).into_bytes(),
    );
}
