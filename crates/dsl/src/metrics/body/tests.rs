use super::*;
use std::collections::BTreeMap;

/// The case file the bot's physics wrote (`harness/test/physics-cases.test.ts`).
const CASES: &str = include_str!("../../../tests/fixtures/movement/cases-1.21.11.json");

/// A world read out of one case: each cell's boxes as the library reported
/// them, its material and its fluid from the block's name and state.
struct CaseWorld {
    cells: BTreeMap<[i32; 3], (Vec<Aabb>, Material, Option<Fluid>)>,
}

impl CollisionWorld for CaseWorld {
    fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>) {
        if let Some((boxes, _, _)) = self.cells.get(&c) {
            out.extend(boxes.iter().copied());
        }
    }
    fn material(&self, c: [i32; 3]) -> Material {
        self.cells.get(&c).map_or(Material::Plain, |(_, m, _)| *m)
    }
    fn fluid(&self, c: [i32; 3]) -> Option<Fluid> {
        self.cells.get(&c).and_then(|(_, _, f)| *f)
    }
}

fn material_of(name: &str) -> Material {
    match name {
        "ice" | "packed_ice" | "frosted_ice" => Material::Ice,
        "blue_ice" => Material::BlueIce,
        "slime_block" => Material::Slime,
        "honey_block" => Material::Honey,
        "soul_sand" => Material::SoulSand,
        "cobweb" => Material::Cobweb,
        "ladder" | "vine" => Material::Climbable,
        _ => Material::Plain,
    }
}

fn fluid_of(name: &str, metadata: i64) -> Option<Fluid> {
    match name {
        "water" | "lava" => Some(Fluid {
            lava: name == "lava",
            depth: if metadata >= 8 { 0 } else { metadata as u8 },
            falling: metadata >= 8,
        }),
        _ => None,
    }
}

fn v3(v: &serde_json::Value) -> [f64; 3] {
    let a = v.as_array().expect("a 3-vector");
    [0, 1, 2].map(|i| a[i].as_f64().expect("a number"))
}

fn world_of(case: &serde_json::Value) -> CaseWorld {
    let palette: Vec<(Vec<[f64; 6]>, Material, Option<Fluid>)> = case["palette"]
        .as_array()
        .expect("a palette")
        .iter()
        .map(|p| {
            let name = p["name"].as_str().expect("a name");
            let shapes = p["shapes"]
                .as_array()
                .expect("shapes")
                .iter()
                .map(|s| {
                    let a = s.as_array().expect("a box");
                    [0, 1, 2, 3, 4, 5].map(|i| a[i].as_f64().expect("a number"))
                })
                .collect();
            (
                shapes,
                material_of(name),
                fluid_of(name, p["metadata"].as_i64().expect("metadata")),
            )
        })
        .collect();
    let mut cells = BTreeMap::new();
    for c in case["cells"].as_array().expect("cells") {
        let (at, i) = c
            .as_str()
            .expect("a cell")
            .split_once('=')
            .expect("x,y,z=i");
        let at: Vec<i32> = at.split(',').map(|v| v.parse().expect("an int")).collect();
        let (shapes, m, f) = &palette[i.parse::<usize>().expect("an index")];
        let (x, y, z) = (f64::from(at[0]), f64::from(at[1]), f64::from(at[2]));
        let boxes = shapes
            .iter()
            .map(|s| Aabb::new(s[0] + x, s[1] + y, s[2] + z, s[3] + x, s[4] + y, s[5] + z))
            .collect();
        cells.insert([at[0], at[1], at[2]], (boxes, *m, *f));
    }
    CaseWorld { cells }
}

fn inputs_of(case: &serde_json::Value) -> Vec<Input> {
    let mut out = Vec::new();
    for h in case["holds"].as_array().expect("holds") {
        let i = Input {
            forward: h["forward"].as_i64().expect("forward") as i8,
            strafe: h["strafe"].as_f64().expect("strafe"),
            jump: h["jump"].as_bool().expect("jump"),
            sprint: h["sprint"].as_bool().expect("sprint"),
            sneak: h["sneak"].as_bool().expect("sneak"),
            yaw: h["yaw"].as_f64().expect("yaw"),
            magnitude: 1.0,
        };
        for _ in 0..h["ticks"].as_u64().expect("ticks") {
            out.push(i);
        }
    }
    out
}

/// spec-0113 A2: every case the bot's physics wrote, replayed by this
/// simulator under [`Rules::Bot`], tick for tick within `1e-9`.
#[test]
fn the_simulator_matches_the_bot_physics() {
    let doc: serde_json::Value = serde_json::from_str(CASES).expect("the case file parses");
    let cases = doc["cases"].as_array().expect("cases");
    assert!(
        cases.len() >= 20,
        "{} case(s): the file must hold at least twenty",
        cases.len()
    );
    let mut ticks = 0usize;
    for case in cases {
        let name = case["name"].as_str().expect("a name");
        let world = world_of(case);
        let start = &case["start"];
        let mut st = State::standing(v3(&start["pos"]), &Attributes::PLAYER);
        st.vel = v3(&start["vel"]);
        st.on_ground = start["onGround"].as_bool().expect("onGround");
        let inputs = inputs_of(case);
        let want = case["ticks"].as_array().expect("ticks");
        assert_eq!(
            inputs.len(),
            want.len(),
            "{name}: one input per recorded tick"
        );
        for (t, (input, w)) in inputs.iter().zip(want).enumerate() {
            tick(&world, &Attributes::PLAYER, Rules::Bot, &mut st, input);
            let (pos, vel) = (v3(&w["pos"]), v3(&w["vel"]));
            for i in 0..3 {
                assert!(
                    (st.pos[i] - pos[i]).abs() <= 1e-9 && (st.vel[i] - vel[i]).abs() <= 1e-9,
                    "{name}, tick {t}: the simulator has {:?} {:?}, the bot's physics {pos:?} {vel:?}",
                    st.pos,
                    st.vel
                );
            }
            assert_eq!(
                st.on_ground,
                w["onGround"].as_bool().expect("onGround"),
                "{name}, tick {t}: the ground flag"
            );
            assert_eq!(
                st.in_water,
                w["inWater"].as_bool().expect("inWater"),
                "{name}, tick {t}: the water flag"
            );
            ticks += 1;
        }
    }
    assert!(ticks >= 20 * 20, "{ticks} tick(s) compared");
}

/// A rig of full stone cubes (and, where named, slabs).
#[derive(Default)]
struct Rig {
    cells: BTreeMap<[i32; 3], f64>,
}

impl Rig {
    fn fill(&mut self, lo: [i32; 3], hi: [i32; 3], top: f64) {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    self.cells.insert([x, y, z], top);
                }
            }
        }
    }
}

impl CollisionWorld for Rig {
    fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>) {
        if let Some(top) = self.cells.get(&c) {
            let (x, y, z) = (f64::from(c[0]), f64::from(c[1]), f64::from(c[2]));
            out.push(Aabb::new(x, y, z, x + 1.0, y + top, z + 1.0));
        }
    }
    fn material(&self, _: [i32; 3]) -> Material {
        Material::Plain
    }
    fn fluid(&self, _: [i32; 3]) -> Option<Fluid> {
        None
    }
}

/// The live spike's rig (`docs/notes/jump-arc-model.md` §1): an approach
/// platform whose top is at y = 1 ending at the plane x = 1, `gap` air columns,
/// a landing platform `rise` up, a catch floor nine below, and a stone lid
/// leaving `h` air cells over the launch surface over one phase of the rig.
fn spike_rig(gap: i32, rise: i32, lid: Option<(Phase, i32)>) -> Rig {
    let mut r = Rig::default();
    r.fill([-14, 0, -2], [0, 0, 2], 1.0);
    r.fill([1 + gap, rise, -2], [8 + gap, rise, 2], 1.0);
    r.fill([-14, -9, -2], [14, -9, 2], 1.0);
    if let Some((phase, h)) = lid {
        let (x0, x1, base) = match phase {
            Phase::Launch => (-14, 0, 1),
            Phase::Gap => (1, gap, 1),
            Phase::Landing => (1 + gap, 8 + gap, 1 + rise),
            Phase::Full => (-14, 8 + gap, 1),
        };
        let y = base + h;
        r.fill([x0, y, -2], [x1, y, 2], 1.0);
        if phase == Phase::Full && rise > 0 {
            r.fill([1 + gap, y + rise, -2], [8 + gap, y + rise, 2], 1.0);
            for x in (1 + gap)..=(8 + gap) {
                for z in -2..=2 {
                    r.cells.remove(&[x, y, z]);
                }
            }
        }
    }
    r
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Launch,
    Gap,
    Landing,
    Full,
}

/// Run the spike's attempt: from rest at `start_x`, hold forward (and sprint)
/// east, press jump on the tick that starts from the last position whose feet
/// centre is short of the edge plane `x = 1`, and say where the body came to
/// rest: `Some(x)` standing on the landing platform (the feet centre at
/// touchdown), `None` anywhere else. The press position is returned beside it.
fn spike_attempt(
    rig: &Rig,
    sprint: bool,
    start_x: f64,
    yaw: f64,
    rules: Rules,
) -> (f64, Option<f64>, f64) {
    let body = Attributes::PLAYER;
    let mut st = State::standing([start_x, 1.0, 0.5], &body);
    let mut hold = Input::forward(yaw);
    hold.sprint = sprint;
    // Run up to the edge without jumping.
    let mut trace = vec![st];
    for _ in 0..200 {
        tick(rig, &body, rules, &mut st, &hold);
        trace.push(st);
        if st.pos[0] >= 1.0 {
            break;
        }
    }
    let press = trace.len() - 2;
    let mut st = trace[press];
    let launch_x = st.pos[0];
    let mut jump = hold;
    jump.jump = true;
    let mut airborne = false;
    for _ in 0..80 {
        tick(rig, &body, rules, &mut st, &jump);
        if !st.on_ground {
            airborne = true;
        } else if airborne {
            let on_landing = st.pos[0] > 1.0 && st.pos[1] > 0.5;
            return (launch_x, on_landing.then_some(st.pos[0]), st.pos[1]);
        }
    }
    (launch_x, None, st.pos[1])
}

/// Whether any attempt of the spike's rig lands: the run-up started at every
/// phase of one tick's travel, facing within `facings` of due east.
fn spike_verdict(rig: &Rig, sprint: bool, facings: &[f64]) -> bool {
    facings.iter().any(|f| {
        (0..32).any(|k| {
            let start = -8.5 - f64::from(k) * 0.3 / 32.0;
            spike_attempt(rig, sprint, start, -90.0 + f, Rules::Bot)
                .1
                .is_some()
        })
    })
}

/// The thirty-three configurations of `docs/notes/jump-arc-model.md` §2, with
/// the verdict the live spike recorded (any of three attempts landed).
fn spike_configs() -> Vec<(String, Rig, bool, bool)> {
    let mut out = Vec::new();
    let mut add =
        |name: String, rig: Rig, sprint: bool, landed: bool| out.push((name, rig, sprint, landed));
    for (gap, landed) in [(1, true), (2, true), (3, false), (4, false)] {
        add(
            format!("walk gap={gap} rise=0"),
            spike_rig(gap, 0, None),
            false,
            landed,
        );
    }
    for (gap, landed) in [
        (1, true),
        (2, true),
        (3, true),
        (4, false),
        (5, false),
        (6, false),
    ] {
        add(
            format!("sprint gap={gap} rise=0"),
            spike_rig(gap, 0, None),
            true,
            landed,
        );
    }
    for (gap, landed) in [(0, true), (1, true), (2, false)] {
        add(
            format!("walk gap={gap} rise=1"),
            spike_rig(gap, 1, None),
            false,
            landed,
        );
    }
    for (gap, landed) in [(0, true), (1, true), (2, true), (3, false), (4, false)] {
        add(
            format!("sprint gap={gap} rise=1"),
            spike_rig(gap, 1, None),
            true,
            landed,
        );
    }
    for (phase, verdicts) in [
        (Phase::Launch, [false, true, true]),
        (Phase::Gap, [false, true, true]),
        (Phase::Landing, [true, true, true]),
        (Phase::Full, [false, true, true]),
    ] {
        for (h, landed) in [2, 3, 4].into_iter().zip(verdicts) {
            add(
                format!("sprint gap=3 rise=0, {phase:?} lid h={h}"),
                spike_rig(3, 0, Some((phase, h))),
                true,
                landed,
            );
        }
    }
    for (h, landed) in [(2, false), (3, true), (4, true)] {
        add(
            format!("sprint gap=2 rise=1, Full lid h={h}"),
            spike_rig(2, 1, Some((Phase::Full, h))),
            true,
            landed,
        );
    }
    out
}

/// spec-0113 A5: the live spike's thirty-three verdicts and its two landing
/// positions, reproduced by the simulator under the bot's rules; a facing step
/// of 0.5° adds no verdict to the 1° one.
#[test]
fn the_simulator_reproduces_the_live_spike() {
    let configs = spike_configs();
    assert_eq!(
        configs.len(),
        33,
        "the spike recorded thirty-three configurations"
    );
    let coarse: Vec<f64> = (-2..=2).map(f64::from).collect();
    let fine: Vec<f64> = (-4..=4).map(|k| f64::from(k) * 0.5).collect();
    for (name, rig, sprint, landed) in &configs {
        assert_eq!(
            spike_verdict(rig, *sprint, &coarse),
            *landed,
            "{name}: the verdict"
        );
        assert_eq!(
            spike_verdict(rig, *sprint, &fine),
            *landed,
            "{name}: a 0.5° facing step changed the verdict"
        );
    }
    // The two landing positions: pressed at the launch position the spike
    // recorded, the body touches down where it recorded.
    for (sprint, gap, launch, land) in [(true, 1, 0.984, 4.613), (false, 1, 0.954, 2.989)] {
        let rig = spike_rig(gap, 0, None);
        let hit = (0..3000)
            .map(|k| -8.5 - f64::from(k) * 0.0001)
            .map(|s| spike_attempt(&rig, sprint, s, -90.0, Rules::Bot))
            .find(|(lx, _, _)| (lx - launch).abs() < 0.0005)
            .expect("a run-up that presses at the recorded launch position");
        let x = hit.1.expect("the recorded jump lands");
        assert!(
            (x - land).abs() < 0.0015,
            "sprint={sprint}: lands at {x:.4}, the spike {land}"
        );
    }
}

/// What the simulator says a body standing on a floor `a` sixteenths tall can
/// do toward a floor `b` sixteenths tall in the next cell `dy` cells up: walk
/// onto it, jump onto it, or neither. A lid two cells over the source's
/// standing cell when `lid`.
fn simulated_step(a: i64, b: i64, dy: i32, lid: bool, rules: Rules) -> super::super::Rise {
    let mut r = Rig::default();
    r.fill([-4, 0, -1], [0, 0, 1], a as f64 / 16.0);
    r.fill([1, dy, -1], [5, dy, 1], b as f64 / 16.0);
    if lid {
        r.fill([-4, 3, -1], [0, 3, 1], 1.0);
    }
    let target = f64::from(dy) + b as f64 / 16.0;
    let body = Attributes::PLAYER;
    let reaches = |jump: bool| {
        let mut st = State::standing([-0.5, a as f64 / 16.0, 0.5], &body);
        let mut input = Input::forward(-90.0);
        input.jump = jump;
        for _ in 0..40 {
            tick(&r, &body, rules, &mut st, &input);
            if st.on_ground && st.pos[0] > 1.3 && (st.pos[1] - target).abs() < 1e-6 {
                return true;
            }
        }
        false
    };
    if reaches(false) {
        super::super::Rise::Walk
    } else if reaches(true) {
        super::super::Rise::Jump
    } else {
        super::super::Rise::Beyond
    }
}

/// spec-0113 A3: the step rule is the simulator's. For every pair of floor
/// heights in sixteenths a collision top can take (1..=16, which covers every
/// partial floor the collision table holds) and a neighbour one cell down, level
/// or up, the simulator's verdict on the one-cell step is `step_allowed`'s, with
/// the head clear and, from a full floor, under a lid two cells over the feet; and the simulator
/// admits the sprint across one column that the walk relation refuses.
#[test]
fn the_step_rule_is_the_simulators() {
    let mut pairs = 0;
    for a in 1..=16i64 {
        for b in 1..=16i64 {
            for dy in -1..=1i32 {
                let rise = i64::from(dy) * 16 + b - a;
                let want = super::super::classify_rise_16(rise);
                let open = simulated_step(a, b, dy, false, Rules::Game);
                assert_eq!(
                    open, want,
                    "floor {a}/16 to {b}/16 {dy:+} cell(s): rise {rise}/16"
                );
                // The head term is consulted for a jump only. The lid sits two
                // whole cells over the feet only on a full floor; over a partial
                // one the cell the walk relation asks about is higher than the
                // head, which is its measurement and not the rule.
                if a == 16 && want == super::super::Rise::Jump {
                    let lidded = simulated_step(a, b, dy, true, Rules::Game);
                    let allowed = super::super::step_allowed(rise, || false);
                    assert_eq!(
                        lidded != super::super::Rise::Beyond,
                        allowed,
                        "floor {a}/16 to {b}/16 {dy:+} cell(s) under a lid: rise {rise}/16, the simulator {lidded:?}"
                    );
                }
                pairs += 1;
            }
        }
    }
    assert_eq!(pairs, 16 * 16 * 3);
    // A sprint across a one-column gap with no jump: no walk step joins the
    // two floors, and the body crosses.
    let mut r = Rig::default();
    r.fill([-12, 0, -1], [0, 0, 1], 1.0);
    r.fill([2, 0, -1], [8, 0, 1], 1.0);
    // Whether it does depends on where the run-up's ticks fall against the
    // edge, so the run-up is started at each phase of one tick's travel: a body
    // reaches the far side when any phase does.
    let body = Attributes::PLAYER;
    let crossed = (0..32).filter(|k| {
        let mut st = State::standing([-8.5 - f64::from(*k) * 0.3 / 32.0, 1.0, 0.5], &body);
        let mut input = Input::forward(-90.0);
        input.sprint = true;
        for _ in 0..60 {
            tick(&r, &body, Rules::Game, &mut st, &input);
        }
        st.on_ground && st.pos[0] > 3.0 && (st.pos[1] - 1.0).abs() < 1e-9
    });
    assert!(
        crossed.count() > 0,
        "no phase of a sprint crosses one column"
    );
}

/// spec-0113 §8.2: the ballistic trajectories the one-time instrument
/// `movement_ballistics` measured the pinned server against are the
/// simulator's, committed. Rewrite with
/// `DELVEWRIGHT_WRITE_BALLISTICS=1`. Every body moves, the slime body comes
/// back up, the water body is slowed by the water, and the slab body ends on
/// the slab.
#[test]
fn the_ballistic_trajectories_are_committed() {
    use super::ballistics::{THROWS, TICKS, trajectories};
    let t = trajectories();
    assert_eq!(t.len(), 4);
    let doc = serde_json::json!({
        "rules": "game",
        "ticks": TICKS,
        "throws": THROWS.iter().zip(&t).map(|(th, tr)| serde_json::json!({
            "name": th.name,
            "start": th.start,
            "motion": th.motion,
            "pos": tr,
        })).collect::<Vec<_>>(),
    });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/movement/ballistics-1.21.11.json");
    let text = serde_json::to_string_pretty(&doc).unwrap() + "\n";
    if std::env::var("DELVEWRIGHT_WRITE_BALLISTICS").is_ok() {
        std::fs::write(&path, &text).unwrap();
    }
    let committed = std::fs::read_to_string(&path).expect("the committed trajectories");
    assert_eq!(
        committed, text,
        "the simulator no longer computes the committed trajectories"
    );
    for (th, tr) in THROWS.iter().zip(&t) {
        assert_eq!(tr.len(), TICKS);
        let moved = (0..3)
            .map(|a| (tr[TICKS - 1][a] - th.start[a]).abs())
            .fold(0.0, f64::max);
        assert!(moved > 0.1, "{} never moved", th.name);
    }
    let slime = &t[1];
    let low = slime
        .iter()
        .position(|p| p[1] < 1.01)
        .expect("the slime body lands");
    assert!(
        slime[low..].iter().any(|p| p[1] > 2.0),
        "the slime body never bounced"
    );
    let water = &t[2];
    assert!(
        water[TICKS - 1][1] < 4.0,
        "the water body did not enter the pool"
    );
    let slab = &t[3];
    assert!(
        (slab[TICKS - 1][1] - 1.5).abs() < 1e-9,
        "the slab body is not on the slab: {:?}",
        slab[TICKS - 1]
    );
}
