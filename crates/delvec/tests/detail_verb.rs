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

use delvewright_compiler::detail::{self, Allocation};
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
    seams: Vec<&delvewright_compiler::detail::AllocatedSeam>,
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
    let mut groups: Vec<(
        (i64, i64),
        Vec<&delvewright_compiler::detail::AllocatedSeam>,
    )> = Vec::new();
    for s in seams {
        let key = (s.cells[0][idx], s.cells[1][idx]);
        match groups.iter_mut().find(|(k, _)| *k == key) {
            Some((_, g)) => g.push(s),
            None => groups.push((key, vec![s])),
        }
    }
    groups.sort_by_key(|(k, _)| *k);
    let bound = |s: &delvewright_compiler::detail::AllocatedSeam, end: &str| {
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
    let mut cells = (0..a.extent[0]).map(|x| (x, 0));
    let mut carved = if a.seams.is_empty() {
        room()
    } else {
        carve(
            a.seams.iter().collect(),
            &[("x", int(0)), ("z", int(0)), ("y", datum.clone())],
            shift,
        )
    };
    if marks {
        for owed in &a.owed_anchors {
            let (x, z) = cells
                .by_ref()
                .find(|(x, z)| !on_a_seam(*x, *z))
                .expect("a cell no seam claims");
            let stem = owed.strip_prefix("anchor/").unwrap_or(owed);
            carved = json!({
                "op": "mark",
                "mark": {"anchor": stem, "at": "offset", "x": int(x), "y": int(0), "z": int(z)},
                "body": carved,
            });
        }
    }

    // The frame: the floor course the piece owns, and the play space over it.
    // Walls and ceiling are the whole's party planes — a course laid inside
    // the play space would move a brief identity the whole holds this place to
    // (`DW0833`) — so the place lights itself from the floor, with standing
    // lamps on a grid.
    let mut rules = serde_json::Map::new();
    rules.insert(
        "piece".into(),
        rule(split(
            "y",
            vec![abs(int(1)), rel()],
            vec![fill("floor"), call("room")],
        )),
    );
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

    json!({
        "version": "1.8.0",
        "name": format!("generated for {}", a.place),
        "start": "piece",
        "params": params,
        "palette": {
            "floor": "minecraft:stone",
            "lamp": "minecraft:sea_lantern"
        },
        "rules": rules,
        "contract": {
            "entry": "room",
            "spaces": {"room": {"envelope": "enclosed"}},
            "no_body": {},
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

/// The blockout fixture, walked, with generated programs for `nodes`.
fn fixture(root: &Path, nodes: &[&str]) -> (PathBuf, PathBuf) {
    let campaign = root.join("campaign");
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    common::copy_dir_all(&blockout_dir(), &campaign);
    common::record_walk(&campaign);
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
    // The piece: the frame's shape, the class stamped, the light measured.
    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("blockout-exit.json")).unwrap())
            .unwrap();
    assert_eq!(meta["structure"]["size"], json!([8, 5, 8]));
    assert_eq!(meta["footprint_class"], "alcove");
    assert_eq!(meta["lighting"]["profile"], "lit");
    assert_eq!(
        meta["license"]["generated_by"]["params"]["handed/seam/cell-exit/z0"],
        2
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
fn detail_refuses_without_a_walk_before_opening_the_program() {
    let tmp = tempdir("dw0841");
    let (campaign, prefabs) = fixture(&tmp, &["node/exit"]);
    std::fs::remove_file(campaign.join("walk-record.json")).unwrap();
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
    assert!(t.contains("DW0841"), "{t}");
    assert!(
        !t.contains("programs/exit.json"),
        "the program was opened: {t}"
    );
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
    assert_eq!(first["structure"]["size"], json!([8, 5, 8]));

    // The exit box shrinks inside its size class, keeping its one seam on the
    // face; the whole is re-walked; the same program re-fits the new frame.
    common::patch_file(&campaign.join("site-plan.json"), |v| {
        let boxes = v["content"]["boxes"].as_array_mut().unwrap();
        let b = boxes.iter_mut().find(|b| b["node"] == "node/exit").unwrap();
        b["extent"] = json!([8, 4]);
    });
    common::record_walk(&campaign);
    let out = delvec(&["--prefabs", ps, "detail", cs, "--all"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let second: Value =
        serde_json::from_str(&std::fs::read_to_string(prefabs.join("blockout-exit.json")).unwrap())
            .unwrap();
    assert_eq!(second["structure"]["size"], json!([8, 5, 4]));
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

    // The connection is renamed in the graph and the plan, the whole re-walked:
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
    common::record_walk(&campaign);
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
    common::record_walk(&campaign);
    let c = common::campaign_at(&campaign);
    let mut places = Vec::new();
    for a in detail::allocations(&c) {
        if answerable(&a) {
            write_program(&campaign, &a.place, &program_for(&a, None, true));
            places.push(a.place.clone());
        }
    }
    assert!(
        places.len() >= 8,
        "the gym offers {} plain-walk place(s): {places:?}",
        places.len()
    );

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
