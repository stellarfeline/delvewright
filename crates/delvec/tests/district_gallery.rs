//! **spec-0103 on the gallery** — the site-plan point, whose layout graph
//! partitions its ten places into `district/hall`, `district/yard` and
//! `district/beacon`, with a quest, an NPC, a station and detail rows written in
//! the districts' fragments and the whole's spine handing the Yard Key across
//! the far door's arch.
//!
//! The point is materialised once per test binary, as `tools/ci/gallery_domain.py`
//! materialises it, its pieces generated and every program place detailed into a
//! prefab directory of its own — the same two acts `tools/ci/gallery-prefabs.py`
//! performs — and every test works on a copy.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn run(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn write_json(path: &Path, v: &Value) {
    std::fs::write(path, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
}

/// Every detail-plan document of a campaign: the whole's and each fragment's.
fn plan_documents(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![dir.join("detail-plan.json")];
    let mut frags: Vec<PathBuf> = std::fs::read_dir(dir.join("districts"))
        .unwrap()
        .map(|e| e.unwrap().path().join("detail-plan.json"))
        .filter(|p| p.is_file())
        .collect();
    frags.sort();
    out.extend(frags);
    out
}

/// The materialised point and its prefab directory, made once: `<root>/point`
/// and `<root>/prefabs`.
fn prepared() -> &'static (PathBuf, PathBuf) {
    static ONCE: OnceLock<(PathBuf, PathBuf)> = OnceLock::new();
    ONCE.get_or_init(|| {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("district-gallery");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let point = root.join("point");
        let prefabs = root.join("prefabs");
        common::gallery_site_plan_with_pieces(&point, &prefabs);
        // Every program place is detailed fresh: its committed row comes out
        // first, from the whole's document and from every fragment's, so no
        // place is judged against a piece this run has not made.
        let programs: BTreeSet<String> = std::fs::read_dir(point.join("programs"))
            .unwrap()
            .map(|e| {
                e.unwrap()
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        let scratch = root.join("detail");
        common::copy_dir_all(&point, &scratch);
        for doc in plan_documents(&scratch) {
            common::patch_file(&doc, |p| {
                p["content"]["details"].as_array_mut().unwrap().retain(|r| {
                    !programs.contains(r["place"].as_str().unwrap().trim_start_matches("node/"))
                });
            });
        }
        let o = run(&[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "detail",
            scratch.to_str().unwrap(),
            "--all",
        ]);
        assert_eq!(o.status.code(), Some(0), "{}", text(&o));
        (point, prefabs)
    })
}

/// A copy of the prepared point at `<root>/<tag>`, inside a repository of its
/// own whose prefab directory is a copy of the prepared one.
fn copy(tag: &str) -> (PathBuf, PathBuf) {
    let (point, prefabs) = prepared();
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("district-gallery-{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let dir = root.join("campaigns/gallery");
    common::copy_dir_all(point, &dir);
    let pre = root.join("prefabs");
    common::copy_dir_all(prefabs, &pre);
    (dir, pre)
}

fn json_lines(o: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.starts_with('{'))
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

// ---------------------------------------------------------------------------
// Criterion 4: the gallery built from fragments, and with them inlined
// ---------------------------------------------------------------------------

fn inline(dir: &Path) {
    let root = dir.join("districts");
    let mut districts: Vec<String> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    districts.sort();
    for x in &districts {
        let mut files: Vec<String> = std::fs::read_dir(root.join(x))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        files.sort();
        for f in files {
            let frag = read_json(&root.join(x).join(&f));
            let mut whole = read_json(&dir.join(&f));
            if f == "layout-graph.json" {
                for n in frag["content"]["nodes"].as_array().unwrap() {
                    let node = whole["content"]["nodes"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|w| w["id"] == n["id"])
                        .unwrap();
                    node.as_object_mut()
                        .unwrap()
                        .entry("stations")
                        .or_insert(json!([]))
                        .as_array_mut()
                        .unwrap()
                        .extend(n["stations"].as_array().unwrap().iter().cloned());
                }
            } else {
                for (k, v) in frag["content"].as_object().unwrap() {
                    whole["content"]
                        .as_object_mut()
                        .unwrap()
                        .entry(k.clone())
                        .or_insert(json!([]))
                        .as_array_mut()
                        .unwrap()
                        .extend(v.as_array().unwrap().iter().cloned());
                }
            }
            write_json(&dir.join(&f), &whole);
        }
    }
    std::fs::remove_dir_all(&root).unwrap();
}

fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(at).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.insert(
                    p.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

#[test]
fn the_gallery_from_fragments_and_inlined_builds_one_datapack_and_one_world() {
    let (dir, prefabs) = copy("determinism");
    let root = dir.parent().unwrap().parent().unwrap().to_path_buf();
    let inlined = root.join("inlined");
    common::copy_dir_all(&dir, &inlined);
    inline(&inlined);
    let mut outs = Vec::new();
    for (camp, name) in [(&dir, "from-fragments"), (&inlined, "from-whole")] {
        let out = root.join(name);
        let o = run(&[
            "--prefabs",
            prefabs.to_str().unwrap(),
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);
        assert_eq!(o.status.code(), Some(0), "{name}: {}", text(&o));
        outs.push(tree(&out));
    }
    let (a, b) = (&outs[0], &outs[1]);
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    let differ: Vec<&String> = keys
        .into_iter()
        .filter(|k| a.get(*k) != b.get(*k))
        .collect();
    assert_eq!(
        differ,
        ["manifest.json", "validation/districts.json"],
        "the same objects built different bytes"
    );
    let datapack = a.keys().filter(|k| k.starts_with("datapack/")).count();
    let world = a.keys().filter(|k| k.starts_with("server/")).count();
    assert!(
        datapack > 100 && world > 0,
        "{datapack} datapack and {world} world file(s)"
    );
    // The ledger routes the one boundary leg.
    let l: Value = serde_json::from_slice(&a["validation/districts.json"]).unwrap();
    assert!(
        l["legs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|leg| leg["from_district"] == "district/hall"
                && leg["to_district"] == "district/yard"),
        "{}",
        l["legs"]
    );
}

// ---------------------------------------------------------------------------
// Criterion 5: a worker's tree on the gallery
// ---------------------------------------------------------------------------

#[test]
fn the_hall_workers_tree_reports_the_yards_quest_under_one_line() {
    let (dir, prefabs) = copy("scope");
    std::fs::remove_file(dir.join("districts/yard/quests.json")).unwrap();
    let args = |extra: &[&str]| {
        let mut a = vec![
            "--json".to_string(),
            "--prefabs".into(),
            prefabs.to_string_lossy().into(),
            "validate".into(),
            dir.to_string_lossy().into(),
        ];
        a.extend(extra.iter().map(|s| s.to_string()));
        a
    };
    let scoped = Command::new(BIN)
        .args(args(&["--district", "district/hall"]))
        .output()
        .unwrap();
    assert_eq!(scoped.status.code(), Some(0), "{}", text(&scoped));
    let diags = json_lines(&scoped);
    let info: Vec<&Value> = diags.iter().filter(|d| d["code"] == "DW1006").collect();
    assert_eq!(info.len(), 1, "{}", text(&scoped));
    assert!(
        info[0]["message"]
            .as_str()
            .unwrap()
            .starts_with("1 planned quest(s) of 1 other district(s) are not expanded in this tree"),
        "{}",
        info[0]
    );
    let whole = Command::new(BIN).args(args(&[])).output().unwrap();
    assert_eq!(whole.status.code(), Some(1), "{}", text(&whole));
    assert!(
        json_lines(&whole)
            .iter()
            .any(|d| d["code"] == "DW0150" && d["severity"] == "error"),
        "{}",
        text(&whole)
    );
    let nobody = Command::new(BIN)
        .args(args(&["--district", "district/nobody"]))
        .output()
        .unwrap();
    assert_eq!(nobody.status.code(), Some(1));
    assert!(text(&nobody).contains("DW1009"), "{}", text(&nobody));
}

// ---------------------------------------------------------------------------
// Criterion 7: the packet
// ---------------------------------------------------------------------------

#[test]
fn the_yards_packet_carries_its_contract_its_allocations_and_its_write_set() {
    let (dir, prefabs) = copy("packet");
    let out = dir.parent().unwrap().parent().unwrap().join("packet-yard");
    let o = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "district",
        "packet",
        dir.to_str().unwrap(),
        "district/yard",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let p = read_json(&out.join("packet.json"));
    assert!(out.join("PACKET.md").is_file());

    // Every boundary edge appears once, with the seven fields of §6.3.
    let campaign = common::campaign_at(&dir);
    let graph = &campaign.layout_graph.as_ref().unwrap().content;
    let yard: BTreeSet<&str> = ["node/exit", "node/gantry", "node/perch"].into();
    let boundary: BTreeSet<String> = graph
        .edges
        .iter()
        .filter(|e| yard.contains(e.a().0.as_str()) != yard.contains(e.b().0.as_str()))
        .map(|e| e.id().0.clone())
        .collect();
    assert!(!boundary.is_empty());
    let contracts: Vec<&Value> = p["neighbours"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|n| n["contracts"].as_array().unwrap())
        .collect();
    let named: Vec<String> = contracts
        .iter()
        .map(|c| c["edge"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        named.iter().cloned().collect::<BTreeSet<_>>(),
        boundary,
        "every boundary edge has a contract"
    );
    assert_eq!(named.len(), boundary.len(), "and each exactly once");
    for c in &contracts {
        for f in [
            "edge", "class", "form", "opening", "height", "landing", "facing", "gating", "passes",
        ] {
            assert!(c.get(f).is_some(), "`{}` lacks `{f}`", c["edge"]);
        }
    }
    let arch = contracts
        .iter()
        .find(|c| c["edge"] == "edge/exit-arch")
        .expect("the far door's arch is the yard's boundary");
    assert_eq!(
        arch["passes"]["party"],
        json!([{"from": "node/far-hall", "to": "node/exit"}]),
        "the party crosses into the yard"
    );
    assert!(
        arch["passes"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["item"] == "minecraft:tripwire_hook"
                && i["given_in"] == json!(["district/hall"])
                && i["required_in"] == json!(["district/yard"])),
        "the Yard Key crosses: {}",
        arch["passes"]["items"]
    );

    // Every place's allocation is `delvec allocation`'s, byte for byte.
    let places = p["places"].as_array().unwrap();
    assert_eq!(places.len(), 3);
    for a in places {
        let place = a["place"].as_str().unwrap();
        let o = run(&["allocation", dir.to_str().unwrap(), place]);
        assert_eq!(o.status.code(), Some(0), "{}", text(&o));
        assert_eq!(
            serde_json::to_string_pretty(a).unwrap(),
            String::from_utf8_lossy(&o.stdout).trim_end(),
            "`{place}`'s allocation differs from `delvec allocation`"
        );
    }

    // Every image the packet names is under `images/`.
    for i in p["images"].as_array().unwrap() {
        assert!(
            out.join(i.as_str().unwrap()).is_file(),
            "`{i}` is named and not copied"
        );
    }

    // The write set is the derivation's.
    let ws = delvec::compiler::district::writes::write_set(
        &campaign,
        "district/yard",
        "campaigns/gallery",
        Some("prefabs"),
    );
    assert_eq!(p["write_set"], json!(ws.rules()));
    assert!(ws.holds("campaigns/gallery/districts/yard/quests.json"));
    assert!(ws.holds("campaigns/gallery/programs/perch.json"));
    assert!(ws.holds("prefabs/gallery-perch.nbt"));
    assert!(!ws.holds("campaigns/gallery/districts/hall/quests.json"));

    // A prefab directory outside the campaign's repository is refused.
    let outside = Path::new(env!("CARGO_TARGET_TMPDIR")).join("district-gallery-outside-prefabs");
    std::fs::create_dir_all(&outside).unwrap();
    let o = run(&[
        "--prefabs",
        outside.to_str().unwrap(),
        "district",
        "packet",
        dir.to_str().unwrap(),
        "district/yard",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(
        text(&o).contains("resolves outside the campaign's repository"),
        "{}",
        text(&o)
    );
    assert!(
        text(&o).contains("Take the library into the repository first"),
        "{}",
        text(&o)
    );
}

// ---------------------------------------------------------------------------
// Criterion 9: detail writes the row where the place belongs
// ---------------------------------------------------------------------------

#[test]
fn detailing_a_yard_place_writes_its_row_into_the_yards_fragment() {
    let (dir, prefabs) = copy("detail");
    // The perch's row comes out of the yard's fragment; the verb writes it back.
    common::patch_file(&dir.join("districts/yard/detail-plan.json"), |p| {
        p["content"]["details"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["place"] != "node/perch");
    });
    for f in ["gallery-perch.nbt", "gallery-perch.json"] {
        let _ = std::fs::remove_file(prefabs.join(f));
    }
    let whole_before = std::fs::read(dir.join("detail-plan.json")).unwrap();
    let o = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        dir.to_str().unwrap(),
        "node/perch",
    ]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let yard = read_json(&dir.join("districts/yard/detail-plan.json"));
    assert!(
        yard["content"]["details"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["place"] == "node/perch" && r["piece"] == "prefab/gallery-perch"),
        "{yard}"
    );
    assert!(yard["content"].get("palette").is_none(), "{yard}");
    assert_eq!(
        std::fs::read(dir.join("detail-plan.json")).unwrap(),
        whole_before,
        "the whole's detail plan is not written"
    );
    assert!(
        prefabs.join("gallery-perch.json").is_file(),
        "the piece lands under --prefabs"
    );
}
