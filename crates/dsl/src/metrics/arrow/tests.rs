use super::*;

/// The case file the bot's aim wrote (`harness/test/shot.test.ts`).
const CASES: &str = include_str!("../../../tests/fixtures/movement/arrow-arcs-1.21.11.json");

const EPS: f64 = 1e-9;

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().expect("a number")
}

fn v3(v: &serde_json::Value) -> [f64; 3] {
    let a = v.as_array().expect("a point");
    [num(&a[0]), num(&a[1]), num(&a[2])]
}

#[test]
fn the_solver_matches_the_bots_aim() {
    let doc: serde_json::Value = serde_json::from_str(CASES).expect("the case file parses");
    assert_eq!(doc["launch_speed"].as_f64(), Some(LAUNCH_SPEED));
    assert_eq!(doc["drag"].as_f64(), Some(DRAG));
    assert_eq!(doc["gravity"].as_f64(), Some(GRAVITY));
    assert_eq!(doc["launch_drop"].as_f64(), Some(LAUNCH_DROP));
    let pitches = doc["pitches"].as_array().expect("pitch cases");
    let flights = doc["flights"].as_array().expect("flight cases");
    let (mut reached, mut unreached) = (0, 0);
    for c in pitches {
        let (d, rise) = (num(&c["distance"]), num(&c["rise"]));
        let ours = low_arc_pitch(d, rise);
        match c["pitch"].as_f64() {
            Some(p) => {
                let o = ours.unwrap_or_else(|| panic!("({d}, {rise}): the bot aims, we do not"));
                assert!(
                    (o - p).abs() < EPS,
                    "({d}, {rise}): pitch {o} against the bot's {p}"
                );
                reached += 1;
            }
            None => {
                assert!(ours.is_none(), "({d}, {rise}): we aim, the bot does not");
                unreached += 1;
            }
        }
    }
    let mut ticks = 0;
    for c in flights {
        let (eye, point) = (v3(&c["eye"]), v3(&c["point"]));
        let ours = flight(eye, point);
        match c["positions"].as_array() {
            Some(ps) => {
                let o = ours.unwrap_or_else(|| panic!("{eye:?} → {point:?}: no flight"));
                assert_eq!(o.len(), ps.len(), "{eye:?} → {point:?}: tick count");
                for (a, b) in o.iter().zip(ps) {
                    let b = v3(b);
                    for i in 0..3 {
                        assert!(
                            (a[i] - b[i]).abs() < EPS,
                            "{eye:?} → {point:?}: {a:?} against {b:?}"
                        );
                    }
                    ticks += 1;
                }
            }
            None => assert!(
                ours.is_none(),
                "{eye:?} → {point:?}: we fly, the bot does not"
            ),
        }
    }
    // The case file is the harness's; a file emptied by mistake binds nothing.
    assert!(
        reached >= 30 && unreached >= 3 && ticks >= 100,
        "{reached} {unreached} {ticks}"
    );
}

#[test]
fn the_flight_crosses_the_point() {
    let eye = [132.5, 92.62, 223.5];
    let point = [149.5, 108.5, 238.5];
    let f = flight(eye, point).expect("the belfry is in reach");
    let (a, b) = (f[f.len() - 2], f[f.len() - 1]);
    let run = |p: [f64; 3]| (p[0] - eye[0]).hypot(p[2] - eye[2]);
    let d = run(point);
    assert!(run(a) < d && run(b) >= d);
    let t = (d - run(a)) / (run(b) - run(a));
    let y = a[1] + t * (b[1] - a[1]);
    assert!((y - point[1]).abs() < 1e-6, "crosses at {y}");
}
