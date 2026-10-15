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
            strafe: h["strafe"].as_i64().expect("strafe") as i8,
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
