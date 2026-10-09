//! **`delvec detail` — one verb details a place inside the allocation the
//! whole handed** (spec-0058).
//!
//! Every program here is GENERATED from the allocation and nothing else: a
//! lined room whose openings are the handed seam cells and whose marks are the
//! handed owed names, every handed value read through a `handed/…` parameter.
//! So the file doubles as the demonstration that the handing is sufficient for
//! a program — and every refusal below is produced by perturbing a program that
//! was green, never by hand-authoring a broken one.

mod common;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::compiler::detail::{self, Allocation};
use delvewright_dsl::NodeId;
use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-detail-verb-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn blockout_dir() -> PathBuf {
    common::repo_root().join("crates/delvec/tests/fixtures/blockout")
}

// ---------------------------------------------------------------------------
// The program generator: a lined room from an allocation
// ---------------------------------------------------------------------------

fn int(v: i64) -> Value {
    json!({"expr": "int", "value": v})
}
fn param(n: &str) -> Value {
    json!({"expr": "param", "name": n})
}
fn sub(a: Value, b: Value) -> Value {
    json!({"expr": "arith", "lhs": a, "op": "sub", "rhs": b})
}
fn add(a: Value, b: Value) -> Value {
    json!({"expr": "arith", "lhs": a, "op": "add", "rhs": b})
}
fn min(a: Value, b: Value) -> Value {
    json!({"expr": "arith", "lhs": a, "op": "min", "rhs": b})
}
fn max(a: Value, b: Value) -> Value {
    json!({"expr": "arith", "lhs": a, "op": "max", "rhs": b})
}
fn dim(axis: &str) -> Value {
    json!({"expr": "dim", "dim": axis})
}
fn abs(e: Value) -> Value {
    json!({"size": "absolute", "blocks": e})
}
fn rel() -> Value {
    json!({"size": "relative", "weight": int(1)})
}
fn fill(role: &str) -> Value {
    json!({"op": "fill", "material": {"role": role}})
}
fn call(s: &str) -> Value {
    json!({"op": "call", "symbol": s})
}
fn split(axis: &str, sizes: Vec<Value>, children: Vec<Value>) -> Value {
    json!({"op": "split", "axis": axis, "sizes": sizes, "children": children})
}
fn rule(body: Value) -> Value {
    json!([{"weight": 1, "body": body}])
}

/// `handed/seam/<edge stem>/<k>`.
fn seam_param(edge: &str, k: &str) -> String {
    format!(
        "handed/seam/{}/{k}",
        edge.strip_prefix("edge/").unwrap_or(edge)
    )
}

/// A piece of the room: claimed as the space, its floor row carrying the
/// lamps the place lights itself with, the rest left as air. Every absolute
/// size is clamped to the scope, because a piece of the carved room can be
/// zero cells across on any axis.
fn room() -> Value {
    json!({"op": "claim", "region": "room", "body": {
        "op": "split", "axis": "y",
        "sizes": [abs(min(int(1), dim("y"))), rel()],
        "children": [call("lamps"), {"op": "void"}]
    }})
}

/// A piece of the room above or below a way: claimed, left as air — a lamp
/// laid here would float over the way and stand a second floor in the space.
fn room_air() -> Value {
    json!({"op": "claim", "region": "room", "body": {"op": "void"}})
}

/// The seams of a room, carved as a chain of splits: along `x`, then `z`, then
/// `y`, seams sharing a range on one axis are grouped into one piece and the
/// group is carved along the next axis, until a piece holds one seam and
/// claims its way. Every other piece is claimed as the room, so the space is
/// the play space minus the ways through it — an opening is a hole through a
/// boundary, never a piece of the room it opens. Every size is an expression
/// over the handed names, so the same document carves the seams wherever the
/// plan moves them.
fn carve(
    seams: Vec<&delvec::compiler::detail::AllocatedSeam>,
    axes: &[(&str, Value)],
    shift: Option<(&str, i64)>,
) -> Value {
    let Some(((axis, base), rest)) = axes.split_first() else {
        let s = seams[0];
        let stem = s.edge.strip_prefix("edge/").unwrap_or(&s.edge);
        return json!({"op": "claim", "region": format!("way/{stem}"), "body": {"op": "void"}});
    };
    let idx = match *axis {
        "x" => 0,
        "y" => 1,
        _ => 2,
    };
    // Group by the range on this axis, ordered by its low end.
    let mut groups: Vec<((i64, i64), Vec<&delvec::compiler::detail::AllocatedSeam>)> = Vec::new();
    for s in seams {
        let key = (s.cells[0][idx], s.cells[1][idx]);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, g)) => g.push(s),
            None => groups.push((key, vec![s])),
        }
    }
    groups.sort_by_key(|(k, _)| *k);
    let bound = |s: &delvec::compiler::detail::AllocatedSeam, end: &str| {
        let p = param(&seam_param(&s.edge, &format!("{axis}{end}")));
        match shift {
            Some((e, n)) if e == s.edge && *axis == "z" => add(p, int(n)),
            _ => p,
        }
    };
    let mut sizes = Vec::new();
    let mut children = Vec::new();
    let mut prev_hi: Option<Value> = None;
    for (_, group) in groups {
        let lo = bound(group[0], "0");
        let hi = bound(group[0], "1");
        let gap = match &prev_hi {
            None => sub(lo.clone(), base.clone()),
            Some(p) => sub(sub(lo.clone(), p.clone()), int(1)),
        };
        sizes.push(abs(gap));
        children.push(if *axis == "y" { room_air() } else { room() });
        sizes.push(abs(add(sub(hi.clone(), lo), int(1))));
        children.push(carve(group, rest, shift));
        prev_hi = Some(hi);
    }
    sizes.push(rel());
    children.push(if *axis == "y" { room_air() } else { room() });
    split(axis, sizes, children)
}

/// Whether the generator can answer this allocation: every seam wants a plain
/// `walk` face on a vertical side. A place that hosts a stair, leaves by a
/// drop, bars its own floor or is entered through its floor or ceiling course
/// is a different program, and not this generator's.
fn answerable(a: &Allocation) -> bool {
    a.seams.iter().all(|s| {
        s.answer_with == ["walk"] && matches!(s.face.as_str(), "east" | "west" | "north" | "south")
    })
}

/// The program for an allocation: a room — its floor course and its play
/// space, lit from the floor — with every seam answered by a claimed way at
/// its handed cells and every owed name answered by a mark.
///
/// `shift` moves one seam's opening; `marks` withholds the owed marks. Both are
/// the perturbations the refusal tests use.
fn program_for(a: &Allocation, shift: Option<(&str, i64)>, marks: bool) -> Value {
    assert!(answerable(a), "`{}` wants more than walk faces", a.place);
    let datum = param("handed/datum-y");
    let mut params = serde_json::Map::new();
    params.insert("handed/datum-y".into(), json!(a.datum_y));
    for s in &a.seams {
        let [lo, hi] = s.cells;
        for (k, v) in [
            ("x0", lo[0]),
            ("y0", lo[1]),
            ("z0", lo[2]),
            ("x1", hi[0]),
            ("y1", hi[1]),
            ("z1", hi[2]),
            ("rise", s.rise),
        ] {
            params.insert(seam_param(&s.edge, k), json!(v));
        }
    }
    params.insert("lamp_period".into(), json!(4));

    // Marks: one per owed name, along the room's `z = 0` row — which the lamp
    // grid never reaches (it starts one cell in on both axes of every piece) —
    // on cells no seam claims, because an anchor on a way volume is not a
    // place a body stands (`DW0845`).
    let on_a_seam = |x: i64, z: i64| {
        a.seams.iter().any(|s| {
            x >= s.cells[0][0] && x <= s.cells[1][0] && z >= s.cells[0][2] && z <= s.cells[1][2]
        })
    };
    // The play space, piece-local, and how much of the place's ring the frame
    // carries on each side (one cell where the place owns its ring there, none
    // where a neighbour owns the plane).
    let [slo, shi] = a.space;
    let (rw, re) = (slo[0], a.extent[0] - 1 - shi[0]);
    let (rn, rs) = (slo[2], a.extent[2] - 1 - shi[2]);
    let clearance = shi[1] - slo[1] + 1;
    // The seams this place answers inside its play space (the plane is a
    // neighbour's), and the ones it owns and cuts in its own ring.
    let inside: Vec<_> = a.seams.iter().filter(|s| !s.owns_plane).collect();
    let mut cells = (0..shi[0] - slo[0] + 1).map(|x| (x, 0));
    let mut carved = if inside.is_empty() {
        room()
    } else {
        carve(
            inside,
            &[("x", int(slo[0])), ("z", int(slo[2])), ("y", datum.clone())],
            shift,
        )
    };
    if marks {
        for owed in &a.owed_anchors {
            let (x, z) = cells
                .by_ref()
                .find(|(x, z)| !on_a_seam(*x + slo[0], *z + slo[2]))
                .expect("a cell no seam claims");
            let stem = owed.strip_prefix("anchor/").unwrap_or(owed);
            carved = json!({
                "op": "mark",
                "mark": {"anchor": stem, "at": "offset", "x": int(x), "y": int(0), "z": int(z)},
                "body": carved,
            });
        }
    }

    // The frame: the ground and floor course under the walk plane, the storey
    // — the place's own ring walls around its play space — and the lid over
    // it. A place owns its outside (spec-0098), so its walls are its own; a
    // ring side a neighbour owns is not in the frame at all.
    let mut rules = serde_json::Map::new();
    rules.insert(
        "piece".into(),
        rule(split(
            "y",
            vec![abs(datum.clone()), abs(int(clearance)), rel()],
            vec![fill("floor"), call("storey"), call("roof")],
        )),
    );
    // The lid, and whatever of the frame stands over it: where the frame
    // reaches above the lid, the top of the lid is a roof a body could stand
    // on, declared out of walk.
    let over_lid = a.extent[1] - (a.datum_y + clearance) > 1;
    rules.insert(
        "roof".into(),
        rule(if over_lid {
            json!({"op": "claim", "region": "roof", "body": split(
                "y",
                vec![abs(min(int(1), dim("y"))), rel()],
                vec![fill("floor"), json!({"op": "void"})],
            )})
        } else {
            fill("floor")
        }),
    );
    rules.insert(
        "storey".into(),
        rule(split(
            "x",
            vec![abs(int(rw)), rel(), abs(int(re))],
            vec![call("ring_west"), call("mid"), call("ring_east")],
        )),
    );
    rules.insert(
        "mid".into(),
        rule(split(
            "z",
            vec![abs(int(rn)), rel(), abs(int(rs))],
            vec![call("ring_north"), call("room"), call("ring_south")],
        )),
    );
    // A ring side: wall, with every owned seam on that side cut through it.
    for (side, along, base) in [
        ("west", "z", 0),
        ("east", "z", 0),
        ("north", "x", rw),
        ("south", "x", rw),
    ] {
        let owned: Vec<_> = a
            .seams
            .iter()
            .filter(|s| s.owns_plane && s.face == side)
            .collect();
        let body = if owned.is_empty() {
            fill("wall")
        } else {
            let mut sizes = Vec::new();
            let mut children = Vec::new();
            let mut at = int(base);
            let idx = if along == "x" { 0 } else { 2 };
            let mut owned = owned;
            owned.sort_by_key(|s| s.cells[0][idx]);
            for s2 in owned {
                let lo = param(&seam_param(&s2.edge, &format!("{along}0")));
                let hi = param(&seam_param(&s2.edge, &format!("{along}1")));
                sizes.push(abs(sub(lo.clone(), at.clone())));
                children.push(fill("wall"));
                sizes.push(abs(add(sub(hi.clone(), lo), int(1))));
                let stem = s2
                    .edge
                    .strip_prefix("edge/")
                    .unwrap_or(&s2.edge)
                    .to_string();
                children.push(split(
                    "y",
                    vec![
                        abs(sub(param(&seam_param(&s2.edge, "y0")), datum.clone())),
                        abs(add(
                            sub(
                                param(&seam_param(&s2.edge, "y1")),
                                param(&seam_param(&s2.edge, "y0")),
                            ),
                            int(1),
                        )),
                        rel(),
                    ],
                    vec![
                        fill("wall"),
                        json!({"op": "claim", "region": format!("way/{stem}"), "body": {"op": "void"}}),
                        fill("wall"),
                    ],
                ));
                at = add(hi, int(1));
            }
            sizes.push(rel());
            children.push(fill("wall"));
            split(along, sizes, children)
        };
        rules.insert(format!("ring_{side}"), rule(body));
    }
    rules.insert("room".into(), rule(carved));
    // The lamp grid's period is clamped to the strip it is laid in, so a piece
    // narrower than one period — a corridor, the smallest alcove, the sliver
    // beside a way — still lays its lamp, or nothing, instead of overflowing.
    let gap = |axis: &str| {
        max(
            int(0),
            min(sub(param("lamp_period"), int(2)), sub(dim(axis), int(2))),
        )
    };
    // A repeating pattern must consume a cell, so a sliver too narrow for the
    // pattern's head is guarded off: the grid runs where there is room for it,
    // one cell in from the piece's edge on both axes.
    let grid = |axis: &str, child: Value| {
        json!([
            {"weight": 1,
             "when": {"cond": "cmp", "lhs": dim(axis), "op": "ge", "rhs": int(2)},
             "body": {"op": "split", "axis": axis, "repeat": true,
                      "sizes": [abs(int(1)), abs(int(1)), abs(gap(axis))],
                      "children": [{"op": "void"}, child, {"op": "void"}]}},
            {"weight": 1, "when": {"cond": "otherwise"}, "body": {"op": "void"}}
        ])
    };
    rules.insert("lamps".into(), grid("x", call("lamp_row")));
    rules.insert("lamp_row".into(), grid("z", fill("lamp")));

    let edges: Vec<Value> = a
        .seams
        .iter()
        .map(|s| {
            let stem = s.edge.strip_prefix("edge/").unwrap_or(&s.edge);
            json!({"a": "exterior", "b": "room", "class": "walk", "via": format!("way/{stem}")})
        })
        .collect();

    let mut shown: Vec<&str> = [("east", re), ("north", rn), ("south", rs), ("west", rw)]
        .into_iter()
        .filter(|(_, ring)| *ring > 0)
        .map(|(side, _)| side)
        .collect();
    shown.push("up");
    shown.sort_unstable();
    let no_body = if over_lid {
        json!({"roof": {"reason": "the place's own roof, which nobody walks"}})
    } else {
        json!({})
    };
    json!({
        "version": "1.9.0",
        "name": format!("generated for {}", a.place),
        "start": "piece",
        "params": params,
        // A place owns its outside (spec-0098), so the sides of the piece the
        // party sees from the open site are its own walls and its lid: every
        // side the frame carries the place's ring on, and the top (`DW0885`,
        // through the program-level list the grammar writes into every
        // exported prefab). Its underside stands on the ground.
        "shown_faces": shown,
        "palette": {
            "floor": "minecraft:stone",
            "wall": "minecraft:stone_bricks",
            "lamp": "minecraft:sea_lantern"
        },
        "rules": rules,
        "contract": {
            "entry": "room",
            "spaces": {"room": {"envelope": "enclosed"}},
            "no_body": no_body,
            "edges": edges
        }
    })
}

fn write_program(campaign: &Path, node: &str, program: &Value) {
    let dir = campaign.join("programs");
    std::fs::create_dir_all(&dir).unwrap();
    let stem = node.strip_prefix("node/").unwrap();
    std::fs::write(
        dir.join(format!("{stem}.json")),
        serde_json::to_string_pretty(program).unwrap() + "\n",
    )
    .unwrap();
}

/// The blockout fixture, with generated programs for `nodes`.
fn fixture(root: &Path, nodes: &[&str]) -> (PathBuf, PathBuf) {
    let campaign = root.join("campaign");
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    common::copy_dir_all(&blockout_dir(), &campaign);
    let c = common::campaign_at(&campaign);
    for node in nodes {
        let a = detail::allocation(&c, &NodeId((*node).to_string())).unwrap();
        write_program(&campaign, node, &program_for(&a, None, true));
    }
    (campaign, prefabs)
}

/// Every file under `dir` with its bytes' hash — what "nothing was written"
/// is asserted against.
fn snapshot(dir: &Path) -> BTreeSet<(String, String)> {
    use sha2::{Digest, Sha256};
    let mut out = BTreeSet::new();
    if !dir.is_dir() {
        return out;
    }
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            let h = format!("{:x}", Sha256::digest(std::fs::read(&p).unwrap()));
            out.insert((p.file_name().unwrap().to_string_lossy().to_string(), h));
        }
    }
    out
}

fn detail_plan(campaign: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(campaign.join("detail-plan.json")).unwrap())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Green: the verb writes the piece and the row, and moves no byte on a rerun
// ---------------------------------------------------------------------------

#[test]
fn one_verb_writes_the_piece_the_report_and_the_row() {
    let tmp = tempdir("green");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();

    let out = delvec(&["--prefabs", ps, "detail", cs, "node/exit"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let t = text(&out);
    assert!(
        t.contains("node/exit: `prefab/blockout-exit` written from `programs/exit.json`"),
        "{t}"
    );
    assert!(
        t.contains("1 declared face(s) answering 1 allocated seam(s)"),
        "{t}"
    );
    assert!(t.contains("1 of 1 owed name(s) bound"), "{t}");
    assert!(
        t.contains("the whole builds with the piece(s) this run wrote"),
        "{t}"
    );
    for f in [
        "blockout-exit.nbt",
        "blockout-exit.json",
        "blockout-exit.report.json",
    ] {
        assert!(prefabs.join(f).is_file(), "{f} written");
    }
    let plan = detail_plan(&campaign);
    let rows = plan["content"]["details"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["place"], "node/exit");
    assert_eq!(rows[0]["piece"], "prefab/blockout-exit");
    assert_eq!(rows[0]["anchors"]["anchor/node-exit"], "anchor/node-exit");
    // The row carries no coordinate — the document's shape is unchanged.
    assert_eq!(rows[0].as_object().unwrap().len(), 3, "{}", rows[0]);
    // The piece: the frame's shape and the light measured, and no class claim.
    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("blockout-exit.json")).unwrap())
            .unwrap();
    // The exit's claim: its ring on three sides, the floor course, four of
    // headroom and the lid (spec-0098 §2).
    assert_eq!(meta["structure"]["size"], json!([9, 6, 10]));
    assert!(meta.get("footprint_class").is_none(), "{meta}");
    assert_eq!(meta["lighting"]["profile"], "lit");
    assert_eq!(
        meta["license"]["generated_by"]["params"]["handed/seam/cell-exit/z0"], 3,
        "one cell in from the play space's edge, which the ring stands outside"
    );

    // Determinism (ADR-0006): the second run moves no byte.
    let before = (snapshot(&prefabs), snapshot(&campaign));
    let again = delvec(&["--prefabs", ps, "detail", cs, "node/exit"]);
    assert_eq!(code(&again), 0, "{}", text(&again));
    assert_eq!(before, (snapshot(&prefabs), snapshot(&campaign)));

    // And the compiler's own verdict, from disk.
    let built = delvec(&[
        "--prefabs",
        ps,
        "build",
        cs,
        "-o",
        tmp.join("out").to_str().unwrap(),
    ]);
    assert_eq!(code(&built), 0, "{}", text(&built));
}

#[test]
fn detail_all_details_every_program_in_plan_order_with_one_command() {
    let tmp = tempdir("all");
    // Every place of the fixture a plain-walk program can answer: not the hall
    // (it hosts a stair), not the loft (it leaves by a drop), not the undercroft
    // (it hosts a stair), not the cell (a stair arrives through its floor).
    let places = ["node/landing", "node/exit", "node/tunnel"];
    let (campaign, prefabs) = fixture(&tmp, &places);
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let c = common::campaign_at(&campaign);
    for p in &places {
        assert!(answerable(
            &detail::allocation(&c, &NodeId((*p).to_string())).unwrap()
        ));
    }

    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let t = text(&out);
    assert!(t.contains("detail: 3 place(s) detailed of 3 named"), "{t}");
    // Site-plan box order, which is the plan's document order.
    let order: Vec<usize> = ["node/landing:", "node/exit:", "node/tunnel:"]
        .iter()
        .map(|p| t.find(p).unwrap_or_else(|| panic!("{p} in {t}")))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    let rows = detail_plan(&campaign)["content"]["details"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(rows, 3);
    assert_eq!(
        snapshot(&prefabs)
            .iter()
            .filter(|(f, _)| f.ends_with(".nbt"))
            .count(),
        3
    );
}

// ---------------------------------------------------------------------------
// Refused where entered, and nothing written
// ---------------------------------------------------------------------------

/// Run a refusal and assert the two directories are byte-identical afterwards.
fn refused(campaign: &Path, prefabs: &Path, args: &[&str]) -> String {
    let before = (snapshot(prefabs), snapshot(campaign));
    let out = delvec(args);
    assert_ne!(code(&out), 0, "accepted: {}", text(&out));
    assert_eq!(
        before,
        (snapshot(prefabs), snapshot(campaign)),
        "a refusal wrote something"
    );
    text(&out)
}

#[test]
fn detail_refuses_a_handed_name_the_whole_does_not_hand() {
    let tmp = tempdir("dw0882");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let c = common::campaign_at(&campaign);
    let a = detail::allocation(&c, &NodeId("node/exit".into())).unwrap();
    let mut p = program_for(&a, None, true);
    p["params"]["handed/seam/no-such-edge/x0"] = json!(0);
    write_program(&campaign, "node/exit", &p);
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "node/exit",
        ],
    );
    assert!(t.contains("DW0882"), "{t}");
    assert!(t.contains("`handed/seam/no-such-edge/x0`"), "{t}");
    assert!(
        t.contains("`handed/seam/cell-exit/z0`"),
        "names what IS handed: {t}"
    );
}

#[test]
fn detail_refuses_a_seam_the_program_does_not_answer_naming_the_face() {
    let tmp = tempdir("dw0844");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let c = common::campaign_at(&campaign);
    let a = detail::allocation(&c, &NodeId("node/exit".into())).unwrap();
    // The opening one cell along the wall from where the plan cut the seam.
    write_program(
        &campaign,
        "node/exit",
        &program_for(&a, Some(("edge/cell-exit", 1)), true),
    );
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "node/exit",
        ],
    );
    assert!(t.contains("DW0844"), "{t}");
    assert!(t.contains("east side"), "names the face: {t}");
    assert!(t.contains("`edge/cell-exit`"), "names the seam: {t}");
}

#[test]
fn detail_refuses_an_owed_name_no_mark_answers() {
    let tmp = tempdir("dw0845");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let c = common::campaign_at(&campaign);
    let a = detail::allocation(&c, &NodeId("node/exit".into())).unwrap();
    write_program(&campaign, "node/exit", &program_for(&a, None, false));
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "node/exit",
        ],
    );
    assert!(t.contains("DW0845"), "{t}");
    assert!(t.contains("`anchor/node-exit`"), "{t}");
}

#[test]
fn detail_all_refuses_a_program_naming_no_place_by_name() {
    let tmp = tempdir("orphan");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    std::fs::write(campaign.join("programs/nowhere.json"), "{}\n").unwrap();
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "--all",
        ],
    );
    assert!(t.contains("`nowhere.json`"), "{t}");
    assert!(
        t.contains("name no place the site plan allocates a box to"),
        "{t}"
    );
}

#[test]
fn detail_all_over_no_program_is_a_zero_binding() {
    let tmp = tempdir("zero");
    let (campaign, prefabs) = fixture(&tmp, &[]);
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "--all",
        ],
    );
    assert!(t.contains("ZERO programs"), "{t}");
}

#[test]
fn detail_refuses_a_place_with_no_box_in_the_allocations_words() {
    let tmp = tempdir("nobox");
    let (campaign, prefabs) = fixture(&tmp, &[]);
    let t = refused(
        &campaign,
        &prefabs,
        &[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            campaign.to_str().unwrap(),
            "node/nowhere",
        ],
    );
    assert!(
        t.contains("the plan allocates no box to `node/nowhere`"),
        "{t}"
    );
}

// ---------------------------------------------------------------------------
// Regeneration: a plan edit is answered by one command
// ---------------------------------------------------------------------------

#[test]
fn after_a_plan_edit_detail_all_refits_the_piece_from_its_program() {
    let tmp = tempdir("refit");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let first: Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("blockout-exit.json")).unwrap())
            .unwrap();
    assert_eq!(first["structure"]["size"], json!([9, 6, 10]));

    // The exit box shrinks, keeping its one seam on the
    // face; the same program re-fits the new frame.
    common::patch_file(&campaign.join("site-plan.json"), |v| {
        let boxes = v["content"]["boxes"].as_array_mut().unwrap();
        let b = boxes.iter_mut().find(|b| b["node"] == "node/exit").unwrap();
        b["extent"] = json!([8, 4]);
    });
    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let second: Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("blockout-exit.json")).unwrap())
            .unwrap();
    assert_eq!(second["structure"]["size"], json!([9, 6, 6]));
    // No creator input: the program on disk is the one written before the edit.
    assert_eq!(
        first["license"]["generated_by"]["program_hash"],
        second["license"]["generated_by"]["program_hash"]
    );
}

#[test]
fn after_a_graph_edit_a_program_that_no_longer_fits_is_refused_by_name() {
    let tmp = tempdir("renamed");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));

    // The connection is renamed in the graph and the plan:
    // the program's handed names now name a seam this place no longer has.
    for doc in [
        "layout-graph.json",
        "site-plan.json",
        "quests.json",
        "quest-plan.json",
    ] {
        let p = campaign.join(doc);
        let t = std::fs::read_to_string(&p).unwrap();
        std::fs::write(&p, t.replace("edge/cell-exit", "edge/cell-gate")).unwrap();
    }
    let t = refused(
        &campaign,
        &prefabs,
        &["--prefabs", ps, "detail", cs, "--all"],
    );
    assert!(t.contains("DW0882"), "{t}");
    assert!(t.contains("`handed/seam/cell-exit/"), "{t}");
    assert!(t.contains("`handed/seam/cell-gate/z0`"), "{t}");
}

// ---------------------------------------------------------------------------
// The count property, on the metrics gym: N places, N programs, ONE command
// ---------------------------------------------------------------------------

#[test]
fn the_gym_is_detailed_by_one_command() {
    let tmp = tempdir("gym");
    let campaign = tmp.join("gym");
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let cs = campaign.to_str().unwrap();
    let ps = prefabs.to_str().unwrap();
    let generated = delvec(&["metrics", "--gym", cs]);
    assert_eq!(code(&generated), 0, "{}", text(&generated));
    let c = common::campaign_at(&campaign);
    let mut places = Vec::new();
    let allocations = detail::allocations(&c);
    for a in &allocations {
        if answerable(a) {
            write_program(&campaign, &a.place, &program_for(a, None, true));
            places.push(a.place.clone());
        }
    }
    // The census is asserted EXACTLY, not as a floor. This read `>= 8`, and a
    // floor is not a measurement: spec-0058 §9 criterion 6 carried 15 where the
    // instrument says 14 for as long as the criterion existed, and nothing could
    // redden. The gym allocates 8 places — five spine bays (one per standard
    // opening the table defines, plus one) and three in the vertical group — and
    // 4 of them want more than a walk — the two climb hosts, the drop's top and
    // the pit (§10) — so 4 are
    // answerable by a plain-walk program, and a gym that grows a place or moves
    // a seam class reds here instead of drifting.
    assert_eq!(allocations.len(), 8, "the gym's places");
    assert_eq!(places.len(), 4, "the gym's plain-walk places: {places:?}");

    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let t = text(&out);
    assert!(
        t.contains(&format!(
            "detail: {n} place(s) detailed of {n} named",
            n = places.len()
        )),
        "{t}"
    );
    let rows = detail_plan(&campaign)["content"]["details"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(rows, places.len());
    eprintln!(
        "count: {} place(s) detailed by ONE `delvec detail --all` from {} program(s)",
        places.len(),
        places.len()
    );
}

// ---------------------------------------------------------------------------
// The prefab directory: a gate report is not metadata
// ---------------------------------------------------------------------------

#[test]
fn a_gate_report_beside_a_piece_is_skipped_by_name_and_a_malformed_metadata_file_is_not() {
    let tmp = tempdir("report-json");
    let prefabs = tmp.join("prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs);
    let campaign = common::hello_world_dir();
    std::fs::write(
        prefabs.join("stray.report.json"),
        "{\"verdict\": \"pass\"}\n",
    )
    .unwrap();
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "validate",
        campaign.to_str().unwrap(),
    ]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    assert!(!text(&out).contains("DW0346"), "{}", text(&out));

    // The skip is the full `.report.json` suffix, not a `report` substring.
    std::fs::write(prefabs.join("report.json"), "{\"verdict\": \"pass\"}\n").unwrap();
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "validate",
        campaign.to_str().unwrap(),
    ]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(text(&out).contains("DW0346"), "{}", text(&out));
}

// ---------------------------------------------------------------------------
// spec-0098 §4: the handout
// ---------------------------------------------------------------------------

/// The gallery's site-plan overlay point, materialised as
/// `tools/ci/gallery_domain.py` does it.
fn gallery_site_plan(root: &Path) -> PathBuf {
    let dir = root.join("site-plan");
    common::gallery_site_plan_at(&dir);
    dir
}

/// **Criteria 24 and 25: the handout is complete, typed by nobody, and
/// carries every seam's form to both sides.** `delvec allocation --all` on the
/// gallery's site-plan overlay hands every place every field of §4; the annex —
/// a place with a roof, seams and a station — carries each of them non-empty;
/// both places a seam joins carry its form; two invocations are byte-identical;
/// and the ground values `delvec detail` binds are the handout's. Vacuous if a
/// field were optional and absent: each is asserted present, by name.
#[test]
fn the_handout_is_complete_and_hands_each_seams_form_to_both_sides() {
    let tmp = tempdir("handout");
    let dir = gallery_site_plan(&tmp);
    let ds = dir.to_str().unwrap();
    let first = delvec(&["allocation", ds, "--all"]);
    assert_eq!(code(&first), 0, "{}", text(&first));
    let second = delvec(&["allocation", ds, "--all"]);
    assert_eq!(first.stdout, second.stdout, "two invocations, one handout");
    let all: Vec<Value> = serde_json::from_slice(&first.stdout).unwrap();
    let places = serde_json::from_str::<Value>(
        &std::fs::read_to_string(dir.join("site-plan.json")).unwrap(),
    )
    .unwrap()["content"]["boxes"]
        .as_array()
        .unwrap()
        .len();
    assert!(places > 0);
    assert_eq!(all.len(), places, "one handout per place");
    const FIELDS: [&str; 14] = [
        "place",
        "brief",
        "concept",
        "sheet",
        "extent",
        "datum_y",
        "world_min",
        "space",
        "neighbours",
        "views",
        "ground",
        "seams",
        "owed_anchors",
        "voids",
    ];
    for h in &all {
        for f in FIELDS {
            assert!(h.get(f).is_some(), "`{}` lacks `{f}`", h["place"]);
        }
        for f in [
            "fill",
            "bottom_y",
            "floor_y",
            "perimeter",
            "min_y",
            "max_y",
            "fixed",
            "columns",
        ] {
            assert!(
                h["ground"].get(f).is_some(),
                "`{}` lacks `ground.{f}`",
                h["place"]
            );
        }
        let c = &h["concept"];
        assert!(c["row"].is_object() || c["absent"].is_string(), "{c}");
    }
    let annex = all.iter().find(|h| h["place"] == "node/annex").unwrap();
    assert!(annex["roof"].is_object(), "the annex declares a roof");
    assert!(!annex["seams"].as_array().unwrap().is_empty());
    assert_eq!(
        annex["brief"]["stations"],
        json!(["anchor/annex-bench (point)"])
    );
    assert!(!annex["brief"]["intent"].as_str().unwrap().is_empty());
    assert!(!annex["palette"].as_object().unwrap().is_empty());
    assert!(!annex["neighbours"].as_array().unwrap().is_empty());
    assert!(!annex["ground"]["fixed"].as_array().unwrap().is_empty());
    assert!(!annex["ground"]["columns"].as_array().unwrap().is_empty());
    assert!(!annex["voids"].as_array().unwrap().is_empty());

    // Every seam's form, on both sides of it.
    let mut joined = 0usize;
    for h in &all {
        for s in h["seams"].as_array().unwrap() {
            let form = s["form"].as_str().expect("a seam's form");
            assert!(!form.is_empty(), "{s}");
            let other = all.iter().find(|o| o["place"] == s["other"]).unwrap();
            let theirs = other["seams"]
                .as_array()
                .unwrap()
                .iter()
                .find(|t| t["edge"] == s["edge"])
                .expect("the other side carries the seam");
            assert_eq!(theirs["form"], s["form"], "both sides of `{}`", s["edge"]);
            joined += 1;
        }
    }
    assert!(
        joined >= 24,
        "{joined} seam side(s) compared — twelve seams, two sides each"
    );

    // What `delvec detail` binds under `handed/ground/` is the handout's.
    let a: Vec<Allocation> = {
        let c = common::campaign_at(&dir);
        vec![detail::allocation(&c, &NodeId("node/annex".into())).unwrap()]
    };
    let prog: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("programs/annex.json")).unwrap())
            .unwrap();
    assert_eq!(
        prog["params"]["handed/roof/lid-y"],
        json!(a[0].roof.as_ref().unwrap().lid_y)
    );
    let mut p = prog.clone();
    for (k, v) in [
        ("handed/ground/min-y", a[0].ground.min_y),
        ("handed/ground/max-y", a[0].ground.max_y),
        ("handed/ground/bottom-y", a[0].ground.bottom_y),
    ] {
        p["params"][k] = json!(v + 7); // a wrong default the verb must overwrite
    }
    std::fs::write(
        dir.join("programs/annex.json"),
        serde_json::to_string_pretty(&p).unwrap() + "\n",
    )
    .unwrap();
    // The yard's piece is the gallery generator's, which this test does not
    // run; the annex alone is detailed.
    common::patch_file(&dir.join("detail-plan.json"), |v| {
        v["content"]["details"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["place"] == "node/annex");
    });
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    // The verb binds the handed values before anything else can refuse; the
    // export's provenance records what it bound.
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        ds,
        "node/annex",
    ]);
    let t = text(&out);
    assert!(
        t.contains("of 18 handed name(s) bound") || t.contains("handed name(s) bound"),
        "{t}"
    );
    let meta = std::fs::read_to_string(prefabs.join("gallery-annex.json"));
    if let Ok(meta) = meta {
        let meta: Value = serde_json::from_str(&meta).unwrap();
        let params = &meta["license"]["generated_by"]["params"];
        assert_eq!(params["handed/ground/min-y"], json!(a[0].ground.min_y));
        assert_eq!(params["handed/ground/max-y"], json!(a[0].ground.max_y));
        assert_eq!(
            params["handed/ground/bottom-y"],
            json!(a[0].ground.bottom_y)
        );
    } else {
        panic!("the annex was not written: {t}");
    }
}

// ---------------------------------------------------------------------------
// Cross-feature pairs of the assembled map route (spec-0098): a hung place
// and the contract instruments that judge its piece.
// ---------------------------------------------------------------------------

/// A gallery program re-cut for a hung frame: the courses under the floor
/// course that its ground place stood on — the first child of the top-level
/// `y` split — are removed, the place's anchor mark moves down with them, and
/// the underside is declared shown. When `hollow` is given, the play space is
/// cut to that many courses.
fn hung_program(name: &str, hollow: Option<i64>) -> Value {
    let path = common::repo_root().join(format!("gallery/overlays/site-plan/programs/{name}.json"));
    let mut p: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let place = &mut p["rules"]["place"][0]["body"];
    let marked = place["op"] == "mark";
    let split = if marked {
        &mut place["body"]
    } else {
        &mut *place
    };
    assert_eq!(split["axis"], "y", "`{name}` stacks its courses on y");
    let under = split["sizes"][0]["blocks"]["value"].as_i64().unwrap();
    split["sizes"].as_array_mut().unwrap().remove(0);
    split["children"].as_array_mut().unwrap().remove(0);
    if let Some(h) = hollow {
        split["sizes"][1]["blocks"]["value"] = json!(h);
    }
    if marked && place["mark"]["y"]["expr"] == "int" {
        let y = place["mark"]["y"]["value"].as_i64().unwrap();
        place["mark"]["y"]["value"] = json!(y - under);
    }
    let faces = p["shown_faces"].as_array_mut().unwrap();
    if !faces.contains(&json!("down")) {
        faces.push(json!("down"));
    }
    p
}

/// **A climb inside one place survives the place being hung, and hung
/// scenery with no floor states its zero.** The gallery's site-plan point,
/// with the causeway — whose program raises a lookout reached by a ladder,
/// declared as a `climb` edge in its spatial contract — hung from its floor
/// course (`base: {"aloft": 0}`), and the beacon, a scenery place
/// (`reached: false`), hung at y 70 and cut to a one-course hollow so no cell
/// of it is stood in. Only those two places are detailed (every other place
/// stands in), each from its own program re-cut for the hung frame.
/// `delvec detail --all` writes both and the whole builds with them: the
/// causeway's contract proves its one interior edge over the hung frame, and
/// the beacon's contract states, by name and count, the zero standable cells
/// scenery owes nothing for. Vacuous unless both are hung: the run's aloft
/// binding names them.
#[test]
fn a_climb_inside_a_hung_place_proves_and_hung_scenery_states_its_zero() {
    let tmp = tempdir("hung-pair");
    let dir = gallery_site_plan(&tmp);
    common::patch_file(&dir.join("site-plan.json"), |v| {
        for b in v["content"]["boxes"].as_array_mut().unwrap() {
            if b["node"] == "node/causeway" {
                b["base"] = json!({"aloft": 0});
            }
            if b["node"] == "node/beacon" {
                b["base"] = json!({"aloft": 0});
                b["floor"] = json!({"y": 70});
            }
        }
    });
    common::patch_file(&dir.join("detail-plan.json"), |v| {
        v["content"]["details"] = json!([]);
    });
    let programs = dir.join("programs");
    std::fs::remove_dir_all(&programs).unwrap();
    std::fs::create_dir_all(&programs).unwrap();
    for (name, hollow) in [("causeway", None), ("beacon", Some(1))] {
        std::fs::write(
            programs.join(format!("{name}.json")),
            serde_json::to_string_pretty(&hung_program(name, hollow)).unwrap(),
        )
        .unwrap();
    }
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        dir.to_str().unwrap(),
        "--all",
    ]);
    let t = text(&out);
    assert_eq!(code(&out), 0, "{t}");
    assert!(t.contains("detail: 2 place(s) detailed of 2 named"), "{t}");
    assert!(
        t.contains("detail: the whole builds with the piece(s) this run wrote."),
        "{t}"
    );
    assert!(
        t.contains("aloft binding: 4 aloft place(s) of 10"),
        "the causeway and the beacon hang beside the gantry and the perch: {t}"
    );

    let report = |piece: &str| -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(prefabs.join(format!("{piece}.report.json"))).unwrap(),
        )
        .unwrap()
    };
    let gate = |r: &Value, id: &str| -> Option<Value> {
        r["gates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["id"] == id)
            .cloned()
    };

    let causeway = report("gallery-causeway");
    assert_eq!(causeway["verdict"], "pass");
    let proof = gate(&causeway, "contract-edge-proof").expect("the edge proof is emitted");
    assert_eq!(proof["state"], "pass", "{proof}");
    assert_eq!(
        proof["bound"], 2,
        "the two climbs of the ladder stack, proved: {proof}"
    );
    let reach = gate(&causeway, "contract-reachability").expect("reachability is emitted");
    assert_eq!(reach["state"], "pass", "{reach}");
    assert!(reach["bound"].as_u64().unwrap() > 0, "{reach}");
    // The causeway encloses nothing — every space is `open` — and the verb
    // passes it with closure's zero stated — the gate withheld and its count
    // in the enumeration — never refused as vacuous (spec-0098 §6b).
    assert!(
        gate(&causeway, "contract-closure").is_none(),
        "closure over nothing is withheld, not red: {causeway}"
    );
    assert!(
        causeway["enumeration"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .any(|l| l.contains("contract-closure")
                && l.contains("declare an envelope closure examines; every space is open")),
        "{causeway}"
    );

    let beacon = report("gallery-beacon");
    assert_eq!(beacon["verdict"], "pass");
    assert!(
        gate(&beacon, "contract-reachability").is_none(),
        "no floor, no reachability gate: {beacon}"
    );
    let closure = gate(&beacon, "contract-closure").expect("closure judges the shell");
    assert!(closure["bound"].as_u64().unwrap() > 0, "{closure}");
    let zero: BTreeSet<&str> = beacon["enumeration"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|l| l.contains("is scenery") && l.contains("0 standable cell(s)"))
        .filter_map(|l| l.strip_prefix("gate `")?.split('`').next())
        .collect();
    assert_eq!(
        zero,
        BTreeSet::from([
            "contract-coverage",
            "contract-no-body",
            "contract-no-body-majority",
            "contract-reachability",
        ]),
        "the four floor gates each state the scenery zero: {}",
        beacon["enumeration"]
    );
}

/// The gallery beacon re-cut as a crown of leaves: five courses of ground,
/// two of leaves, three of air over them — so every leaf top is a standable
/// cell — and a contract whose one space is claimed in the top course of air
/// and holds none of them.
fn leafy_beacon() -> Value {
    let beacon: Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("gallery/overlays/site-plan/programs/beacon.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let fill =
        |role: &str| json!([{"body": {"material": {"role": role}, "op": "fill"}, "weight": 1}]);
    let abs = |n: i64| json!({"blocks": {"expr": "int", "value": n}, "size": "absolute"});
    let rest = json!({"size": "relative", "weight": {"expr": "int", "value": 1}});
    json!({
        "contract": {"edges": [], "entry": "crown", "spaces": {"crown": {"envelope": "open"}}},
        "name": "gallery-beacon",
        "palette": {
            "ground": "minecraft:polished_blackstone_bricks",
            "leaves": "minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]"
        },
        "params": {},
        "shown_faces": ["east", "north", "south", "west"],
        "start": "place",
        "version": beacon["version"],
        "rules": {
            "ground": fill("ground"),
            "leaves": fill("leaves"),
            "air": [{"body": {"axis": "y", "op": "split",
                "children": [{"op": "void"}, {"body": {"op": "void"}, "op": "claim", "region": "crown"}],
                "sizes": [abs(2), rest.clone()]}, "weight": 1}],
            "place": [{"body": {"axis": "y", "op": "split",
                "children": [
                    {"op": "call", "symbol": "ground"},
                    {"op": "call", "symbol": "leaves"},
                    {"op": "call", "symbol": "leaves"},
                    {"op": "call", "symbol": "air"}
                ],
                "sizes": [abs(5), abs(1), abs(1), rest]}, "weight": 1}]
        }
    })
}

/// **Scenery whose leaves can be stood on owes no floor and no light**
/// (spec-0098 §14, departure 36). The gallery's site-plan point with its
/// scenery beacon (`reached: false`) re-cut as a crown of leaves with 100
/// standable leaf tops, no light source, and a contract declaring none of
/// them. `delvec detail` writes it and the whole builds: the contract's floor
/// gates excuse the leaf tops with their count, and the light probe states
/// the cells it excludes with theirs instead of grading them. The same piece
/// with `reached: false` taken off is a place a body visits, and the verb
/// judges its floor again — the leaf tops nothing declares and the way in
/// nothing claims — and writes nothing.
#[test]
fn a_leafy_scenery_crown_owes_no_floor_and_no_light_and_a_reached_one_is_judged() {
    let tmp = tempdir("leafy-crown");
    let dir = gallery_site_plan(&tmp);
    common::patch_file(&dir.join("detail-plan.json"), |v| {
        v["content"]["details"] = json!([]);
    });
    let programs = dir.join("programs");
    std::fs::remove_dir_all(&programs).unwrap();
    std::fs::create_dir_all(&programs).unwrap();
    std::fs::write(
        programs.join("beacon.json"),
        serde_json::to_string_pretty(&leafy_beacon()).unwrap(),
    )
    .unwrap();
    let detail_into = |prefabs: &Path| {
        std::fs::create_dir_all(prefabs).unwrap();
        delvec(&[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            dir.to_str().unwrap(),
            "--all",
        ])
    };

    let prefabs = tmp.join("prefabs");
    let out = detail_into(&prefabs);
    let t = text(&out);
    assert_eq!(code(&out), 0, "{t}");
    assert!(
        t.contains(
            "node/beacon: scenery (`reached: false`) — the light probe excludes the piece's 100 \
             standable cell(s): none is stood in, and none is owed light."
        ),
        "{t}"
    );
    assert!(
        t.contains("detail: the whole builds with the piece(s) this run wrote."),
        "{t}"
    );
    let report: Value = serde_json::from_str(
        &std::fs::read_to_string(prefabs.join("gallery-beacon.report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["verdict"], "pass");
    let coverage = report["gates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["id"] == "contract-coverage")
        .cloned()
        .expect("coverage is emitted");
    assert_eq!(coverage["state"], "pass", "{coverage}");
    assert_eq!(coverage["bound"], 100, "{coverage}");
    assert!(
        coverage["detail"]
            .as_str()
            .unwrap()
            .contains("none of the piece's 100 standable cell(s) is stood in"),
        "{coverage}"
    );

    common::patch_file(&dir.join("layout-graph.json"), |v| {
        for n in v["content"]["nodes"].as_array_mut().unwrap() {
            if n["id"] == "node/beacon" {
                n.as_object_mut().unwrap().remove("reached");
            }
        }
    });
    let reached = tmp.join("prefabs-reached");
    let out = detail_into(&reached);
    let t = text(&out);
    assert_eq!(code(&out), 1, "{t}");
    assert!(
        t.contains("contract-coverage FAIL") && t.contains("contract-reachability FAIL"),
        "a reached crown's floor is judged again: {t}"
    );
    assert!(!t.contains("the light probe excludes"), "{t}");
    assert!(
        !reached.join("gallery-beacon.nbt").exists(),
        "nothing written"
    );
}

/// **Scenery owes no light, so no proof floods from inside it** (spec-0098
/// §14, departure 36). The gallery's sealed scenery beacon with its lamp
/// swapped for leaves: a closed room of stone and oak leaves with no light
/// source anywhere in it. Scenery has no place a body stands, so the blockout
/// synthesizes no `anchor/node-beacon` (the one authority,
/// `synthesized_anchor_kinds`, names none), and the darkness survey
/// (`DW0210`), which floods from every anchor of an area, never starts inside
/// it: the whole builds. Before, the anchor stood on the room's floor and the
/// survey refused the unlit scenery with `DW0210`.
#[test]
fn an_unlit_leafy_scenery_room_is_never_surveyed_for_light() {
    let tmp = tempdir("unlit-scenery");
    let dir = gallery_site_plan(&tmp);
    common::patch_file(&dir.join("detail-plan.json"), |v| {
        v["content"]["details"] = json!([]);
    });
    common::patch_file(&dir.join("programs/beacon.json"), |v| {
        v["palette"]["lamp"] =
            json!("minecraft:oak_leaves[distance=1,persistent=true,waterlogged=false]");
    });
    let c = common::campaign_at(&dir);
    assert!(
        !delvewright_dsl::synthesized_anchors(&c).contains("anchor/node-beacon"),
        "scenery has no place anchor"
    );
    assert!(
        delvewright_dsl::synthesized_anchors(&c).contains("anchor/node-annex"),
        "a reached place keeps its own"
    );
    let prefabs = tmp.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let out = delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        dir.to_str().unwrap(),
        "node/beacon",
    ]);
    let t = text(&out);
    assert_eq!(code(&out), 0, "{t}");
    assert!(!t.contains("DW0210"), "{t}");
    assert!(
        t.contains("detail: the whole builds with the piece(s) this run wrote."),
        "{t}"
    );
    let bytes = std::fs::read(prefabs.join("gallery-beacon.nbt")).unwrap();
    let s = delvec::admit::structure::Structure::read(&bytes).unwrap();
    let names = s.block_names();
    assert!(
        names.contains("minecraft:oak_leaves") && !names.contains("minecraft:sea_lantern"),
        "the piece is the unlit leafy room: {names:?}"
    );
}
