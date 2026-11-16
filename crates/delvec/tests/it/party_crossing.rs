//! The compiler's own inter-area crossing carries the party.
//!
//! Completing an objective whose next step stands in another area crosses the
//! void to that area's entry point. The crossing is a party carry, through the
//! one body a party `teleport` uses: every member present is numbered and put
//! on a seat of their own, whoever completed the objective, and a member who
//! was away is carried once they are back (the tick's owed line). These pin
//! the emission on `keep-crawl` (one crossing on the exported path); the live
//! half is the generated `party_cross_<area>` PackTest, whose shape is pinned
//! here and which the gallery runs.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const NS: &str = "keep-crawl";
/// The keep's entry point, where completing `o/talk` crosses to.
const KEEP_ENTRY: [i32; 3] = [260, 65, 4];

fn build_keep_crawl() -> BuildOutput {
    let dir = common::keep_crawl_dir();
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("solver")).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn func<'a>(out: &'a BuildOutput, ns: &str, name: &str) -> &'a str {
    let path = format!("datapack/data/{ns}/function/{name}.mcfunction");
    std::str::from_utf8(out.get(&path).unwrap_or_else(|| panic!("missing {path}"))).unwrap()
}

/// **Where a completion's crossing puts the first member**: the cell of the
/// first seat of every `cross_<area>` the body calls (behind a branch's flags
/// or not), in call order. The one reading of a crossing's landing the tests
/// share.
pub(crate) fn crossing_landings(out: &BuildOutput, ns: &str, body: &str) -> Vec<[i32; 3]> {
    let call = format!("function {ns}:cross_");
    let mut found = Vec::new();
    for line in body.lines() {
        let Some(i) = line.find(&call) else { continue };
        let name = line[i + format!("function {ns}:").len()..].trim();
        let cross = func(out, ns, name);
        let seat = cross
            .lines()
            .find_map(|l| l.strip_prefix("tp @a[tag=!dw_cutscene,scores={dw.tpp=1}] "))
            .unwrap_or_else(|| panic!("{name} puts no member on its first seat:\n{cross}"));
        let v: Vec<f64> = seat
            .split_whitespace()
            .map(|t| t.parse().expect("a coordinate"))
            .collect();
        found.push([
            v[0].floor() as i32,
            v[1].floor() as i32,
            v[2].floor() as i32,
        ]);
    }
    found
}

#[test]
fn a_crossing_carries_every_member_not_the_one_who_acted() {
    let out = build_keep_crawl();
    let complete = func(&out, NS, "complete_o_talk");
    assert!(
        !complete.lines().any(|l| l.starts_with("teleport @s")),
        "the crossing moves no one by `@s`:\n{complete}"
    );
    assert_eq!(
        crossing_landings(&out, NS, complete),
        vec![KEEP_ENTRY],
        "{complete}"
    );
    let call = complete
        .lines()
        .find(|l| l.starts_with(&format!("function {NS}:cross_")))
        .expect("an unconditional crossing");
    let name = call.rsplit(':').next().unwrap();
    let cross = func(&out, NS, name);
    let lines: Vec<&str> = cross.lines().collect();
    // The party's count first, then the members numbered.
    assert_eq!(lines[0], "scoreboard players add #party dw.xc 1", "{cross}");
    assert!(
        lines.iter().any(|l| l.starts_with("execute as @a")
            && l.contains("unless data entity @s {Health:0.0f}")
            && l.ends_with(
                "store result score @s dw.tpp run scoreboard players add #tpp dw.sys 1"
            )),
        "every member present is numbered:\n{cross}"
    );
    // Four seats, four distinct cells, the entry point first.
    let seats: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("tp @a[tag=!dw_cutscene,scores={dw.tpp="))
        .collect();
    assert_eq!(seats.len(), 4, "{cross}");
    let cells: std::collections::BTreeSet<&str> = seats
        .iter()
        .map(|s| s.split_once("] ").unwrap().1)
        .collect();
    assert_eq!(cells.len(), 4, "no two members share a seat:\n{cross}");
    assert!(
        seats[3].starts_with("4..}]"),
        "a fifth body takes the last seat:\n{cross}"
    );
    // Every member carried takes the party's count, before the numbers go.
    let wrote = lines
        .iter()
        .position(|l| {
            *l == "scoreboard players operation @a[scores={dw.tpp=1..}] dw.xc = #party dw.xc"
        })
        .expect("the carried members take the party's count");
    assert_eq!(lines.last().unwrap(), &"scoreboard players reset @a dw.tpp");
    assert!(wrote < lines.len() - 1);
    // Declared in setup.
    let setup = func(&out, NS, "setup");
    for obj in ["dw.tpp", "dw.xc"] {
        assert!(
            setup
                .lines()
                .any(|l| l == format!("scoreboard objectives add {obj} dummy")),
            "setup declares {obj}"
        );
    }
}

#[test]
fn a_member_who_was_away_is_carried_once_back() {
    let out = build_keep_crawl();
    let tick = func(&out, NS, "tick");
    let owed = format!(
        "if entity @s[tag=!dw_cutscene] if score #party dw.xc matches 1.. unless score @s dw.xc = \
         #party dw.xc run function {NS}:cross_owed"
    );
    assert!(
        tick.lines()
            .any(|l| l.starts_with("execute as @a") && l.ends_with(&owed)),
        "the tick carries a member behind the party's crossings:\n{tick}"
    );
    let owed_fn = func(&out, NS, "cross_owed");
    let [x, y, z] = KEEP_ENTRY;
    assert!(
        owed_fn.lines().any(|l| l.ends_with(&format!(
            "if score #xto dw.sys matches 1 run tp @s {x}.5 {y} {z}.5"
        ))),
        "a late member lands on the last crossing's entry point:\n{owed_fn}"
    );
    assert_eq!(
        owed_fn.lines().last().unwrap(),
        "scoreboard players operation @s dw.xc = #party dw.xc"
    );
}

/// The generated two-member PackTest: two members far apart, one completes
/// the crossing objective, both arrive; a member spawned after is carried.
#[test]
fn the_party_crossing_packtest_drives_the_real_completion() {
    let out = build_keep_crawl();
    let (path, body) = out
        .iter()
        .find(|(p, _)| p.contains("/test/party_cross_"))
        .map(|(p, b)| (p.clone(), String::from_utf8_lossy(b).into_owned()))
        .expect("a party-crossing template");
    assert!(
        body.contains("# @environment "),
        "it runs alone: {path}\n{body}"
    );
    assert!(body.contains("dummy dwpcx0p1 spawn"), "{body}");
    assert!(
        body.contains(&format!(
            "execute as @a[tag=dw_t_pcx0,limit=1] run function {NS}:complete_o_talk"
        )),
        "the real completion is driven:\n{body}"
    );
    assert!(
        body.contains("assert score #pcx_0_near dw.sys matches 0"),
        "{body}"
    );
    assert!(
        body.contains("assert score #pcx_0_s0 dw.sys matches 1"),
        "{body}"
    );
    assert!(
        body.contains("assert score #pcx_0_s1 dw.sys matches 1"),
        "{body}"
    );
    assert!(
        body.contains(&format!("run function {NS}:cross_owed")),
        "the owed carry is driven:\n{body}"
    );
    assert!(
        body.contains("assert score #pcx_0_late dw.sys matches 1"),
        "{body}"
    );
}
