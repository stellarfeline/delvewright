//! spec-0082 — a fixed thing that can be hit and hits back.
//!
//! The fixture is the hello-world keep with one assembly standing on its exit:
//! a three-part rig whose `strike` clip lays a 3 × 3 slab over the exit and
//! whose hitbox is counted by a `strike-assembly` trigger. Three strikes set
//! `flag/struck`, which the exit's `reach-anchor` waits on, so the critical
//! path performs the trigger three times.

mod common;

use common::assembly_fixture::*;
use delvec::compiler::emit::BuildOutput;
use delvec::compiler::registry::PrefabRegistry;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry};
use delvewright_dsl::rig::Rig;
use serde_json::{Value, json};

const NS: &str = "hello-world";

/// One emitted function's body.
pub fn function(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no `{name}` emitted"))
            .clone(),
    )
    .unwrap()
}

/// Every shipped function, concatenated.
pub fn all_functions(out: &BuildOutput) -> String {
    let mut s = String::new();
    for (path, bytes) in out {
        if path.starts_with("datapack/") && path.ends_with(".mcfunction") {
            s.push_str(std::str::from_utf8(bytes).unwrap());
            s.push('\n');
        }
    }
    s
}

// ---------------------------------------------------------------------------
// AC3 — emission
// ---------------------------------------------------------------------------

/// One root, one `block_display` per part each riding the root, one hitbox; one
/// function per clip frame with one `data merge` per part; one driver. Every
/// line of it was walked against `CommandTree::v1_21_11()` by `emit::build`,
/// which refuses an invalid line (`BuildFailure::Validation`).
#[test]
fn the_body_is_a_root_its_riding_parts_and_a_hitbox() {
    let out = build_fixture();
    let summon = function(&out, "asm_summon_limb");
    let count = |needle: &str| summon.lines().filter(|l| l.contains(needle)).count();
    assert_eq!(count("summon minecraft:item_display"), 1, "{summon}");
    assert_eq!(count("summon minecraft:block_display"), 3, "{summon}");
    assert_eq!(count("ride @e[tag=dw_asm_limb_p"), 3, "{summon}");
    for i in 0..3 {
        assert!(
            summon.contains(&format!(
                "ride @e[tag=dw_asm_limb_p{i},limit=1] mount @e[tag=dw_asm_limb_root,limit=1]"
            )),
            "{summon}"
        );
    }
    assert_eq!(count("summon minecraft:interaction"), 1, "{summon}");
    assert!(summon.contains("\"dw_asm_limb_hit\""), "{summon}");
    // One function per frame, one `data merge` per part in each.
    let r = rig();
    let frames: Vec<(usize, usize)> = r
        .clips
        .values()
        .enumerate()
        .flat_map(|(k, c)| (0..c.frames.len()).map(move |f| (k, f)))
        .collect();
    assert_eq!(frames.len(), r.frame_count());
    for (k, f) in frames {
        let body = function(&out, &format!("asm_frame_limb_{k}_{f}"));
        assert_eq!(
            body.lines()
                .filter(|l| l.contains("data merge entity @s"))
                .count(),
            3,
            "{body}"
        );
    }
    // One driver.
    let tick = function(&out, "tick");
    assert_eq!(
        tick.lines()
            .filter(|l| l.contains("function hello-world:asm_tick_limb"))
            .count(),
        1,
        "{tick}"
    );
    assert!(
        function(&out, "asm_apply_limb")
            .starts_with("$function hello-world:asm_frame_limb_$(c)_$(f)")
    );
}

/// `facing: north` turns every emitted translation's `x` and `z` against
/// `south` (a half turn about the mark), and leaves `y` alone.
#[test]
fn facing_turns_every_translation_about_the_mark() {
    let south = build_fixture();
    let q = quests_with(|q| assembly(q)["facing"] = json!("north"));
    let north = try_build(&campaign(&q), &prefabs_with(rig())).expect("builds");
    let translation = |out: &BuildOutput, f: &str| -> Vec<[f64; 3]> {
        function(out, f)
            .lines()
            .filter_map(|l| {
                let t = l.split("translation:[").nth(1)?.split(']').next()?;
                let v: Vec<f64> = t
                    .split(',')
                    .map(|x| x.trim_end_matches('f').parse().unwrap())
                    .collect();
                Some([v[0], v[1], v[2]])
            })
            .collect()
    };
    // `strike`'s last frame: the slab at [-1.5, 0, -1.5].
    let k = rig().clip_index("strike").unwrap();
    let s = translation(&south, &format!("asm_frame_limb_{k}_1"));
    let n = translation(&north, &format!("asm_frame_limb_{k}_1"));
    assert_eq!(s.len(), 3);
    for (a, b) in s.iter().zip(&n) {
        assert_eq!(b[0], -a[0], "x turns: {a:?} -> {b:?}");
        assert_eq!(b[1], a[1], "y stays: {a:?} -> {b:?}");
        assert_eq!(b[2], -a[2], "z turns: {a:?} -> {b:?}");
    }
    assert!(
        s.iter().zip(&n).any(|(a, b)| a != b),
        "facing moved nothing"
    );
}

/// Two builds are byte-identical (ADR-0006).
#[test]
fn two_builds_are_byte_identical() {
    assert_eq!(build_fixture(), build_fixture());
}

/// A changed frame translation moves a `data merge` line: the rig's numbers
/// are what ship, so the element is bound by perturbation (spec-0039).
#[test]
fn perturbing_a_frame_translation_moves_a_data_merge_line() {
    let base = build_fixture();
    let mut r = rig();
    r.clips.get_mut("idle").unwrap().frames[1][0].translation[0] += 0.25;
    let moved = try_build(&campaign(&quests()), &prefabs_with(r)).expect("builds");
    let k = rig().clip_index("idle").unwrap();
    let name = format!("asm_frame_limb_{k}_1");
    assert_ne!(function(&base, &name), function(&moved, &name));
    assert!(function(&moved, &name).contains("translation:[-0.2500f,0.1000f,-0.5000f]"));
}

// ---------------------------------------------------------------------------
// AC4 — hits
// ---------------------------------------------------------------------------

/// `strike-assembly` polls and clears the assembly's own hitbox, and its
/// bundle's three effects land in declared order in one function.
#[test]
fn a_strike_reads_and_clears_the_hitbox_and_counts_in_order() {
    let out = build_fixture();
    let tick = function(&out, "tick");
    assert!(
        tick.contains(
            "if entity @e[tag=dw_asm_limb_hit,nbt={attack:{}}] run function hello-world:trig_limb_struck"
        ),
        "{tick}"
    );
    assert!(
        tick.contains("execute as @e[tag=dw_asm_limb_hit] run data remove entity @s attack"),
        "{tick}"
    );
    let trig = function(&out, "trig_limb_struck");
    let at = |needle: &str| {
        trig.lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("no `{needle}` in:\n{trig}"))
    };
    let add = at("scoreboard players add #party dw.s_hits 1");
    let cue = at("function hello-world:asm_cue_limb_");
    let flag = at("scoreboard players set #party dw.f_struck 1");
    assert!(add < cue && cue < flag, "{trig}");
    assert!(
        trig.lines()
            .nth(cue)
            .unwrap()
            .contains("dw.s_hits matches 3..")
    );
}

/// `audience: presser` on a left-click is `DW0427`; `at` on a
/// `strike-assembly` is `DW0194`.
#[test]
fn presser_and_at_are_refused_on_a_strike_assembly() {
    let q = quests_with(|q| trigger(q)["audience"] = json!("presser"));
    assert!(
        validation_codes(&q, rig())
            .iter()
            .any(|(c, p, _)| c == "DW0427" && p.ends_with("/audience")),
        "{:?}",
        validation_codes(&q, rig())
    );
    let q = quests_with(|q| trigger(q)["at"] = json!("anchor/exit"));
    let codes = validation_codes(&q, rig());
    assert!(
        codes
            .iter()
            .any(|(c, _, m)| c == "DW0194" && m.contains("assembly `assembly/limb`'s hitbox")),
        "{codes:?}"
    );
}

/// A `strike-assembly` on an assembly with no hitbox is `DW0936`.
#[test]
fn a_strike_on_an_assembly_with_no_hitbox_is_dw0936() {
    let q = quests_with(|q| {
        assembly(q).as_object_mut().unwrap().remove("hitbox");
    });
    let m = refusal(&q, rig(), "DW0936");
    assert!(m.contains("declares no `hitbox`"), "{m}");
}

// ---------------------------------------------------------------------------
// AC5 — the hitbox
// ---------------------------------------------------------------------------

/// Width 7 and height 23 are refused; width 6 and height 22 build.
#[test]
fn the_hitbox_bounds() {
    for (w, h, want) in [
        (7.0, 2.0, Some("DW0936")),
        (1.0, 23.0, Some("DW0936")),
        (6.0, 22.0, None),
    ] {
        let q = quests_with(|q| {
            assembly(q)["hitbox"] = json!({ "width": w, "height": h });
        });
        assert_eq!(build_code(&q, rig()).as_deref(), want, "{w} x {h}");
    }
}

/// A hitbox beside the parts is refused naming the cells they stand in.
#[test]
fn a_hitbox_off_the_parts_is_refused_naming_their_cells() {
    let q = quests_with(|q| {
        assembly(q)["hitbox"] = json!({ "width": 1.0, "height": 2.0, "offset": [3, 0, 0] });
    });
    let m = refusal(&q, rig(), "DW0936");
    assert!(m.contains("[5, 65, 8]") && m.contains("meets none"), "{m}");
}

/// The hitbox is in `compiler::eclipse`'s enumeration: a body posted on its
/// cell is `DW0359`.
#[test]
fn a_body_on_the_hitbox_is_dw0359() {
    let q = quests_with(|q| {
        q["content"]["actors"] = json!([
            { "id": "actor/watcher", "entity": "minecraft:villager", "anchor": "anchor/exit" }
        ]);
    });
    let m = refusal(&q, rig(), "DW0359");
    assert!(
        m.contains("assembly hitbox") || m.contains("assembly/limb"),
        "{m}"
    );
}

// ---------------------------------------------------------------------------
// AC6 — reach
// ---------------------------------------------------------------------------

/// A performed strike whose hitbox is out of every walkable eye's reach is
/// `DW0937`; the mark one cell lower builds. The bound is the constant
/// `DW0924` reads.
#[test]
fn a_performed_strike_out_of_reach_is_dw0937_and_one_cell_lower_is_not() {
    assert_eq!(delvec::compiler::strand::STRIKE_REACH, 3.0);
    // The strike is not this test's subject: a mark raised off the floor
    // lays its slab in the air, which the strike check refuses.
    let at = |dy: i32| {
        quests_with(|q| {
            assembly(q)["at"] = json!({ "anchor": "anchor/exit", "offset": [0, dy, 0] });
            assembly(q)["hitbox"] = json!({ "width": 1.0, "height": 2.0, "offset": [0, 2, 0] });
            assembly(q).as_object_mut().unwrap().remove("strikes");
        })
    };
    let m = refusal(&at(3), rig(), "DW0937");
    assert!(
        m.contains("trigger/limb-struck") && m.contains("3 blocks"),
        "{m}"
    );
    assert_eq!(build_code(&at(2), rig()), None);
}

// ---------------------------------------------------------------------------
// AC7 — the strike, end to end
// ---------------------------------------------------------------------------

/// Shape 1: a landing box one cell wider than the arming region's keep-out is
/// refused; the box inside it builds.
#[test]
fn shape_one_end_to_end() {
    let landing = |e: [u32; 3]| {
        quests_with(|q| {
            assembly(q)["strikes"]["while_in"]["extent"] = json!([0, 0, 0]);
            assembly(q)["strikes"]["pattern"][0]["on_land"][0]["in"]["extent"] = json!(e);
        })
    };
    let m = refusal(&landing([1, 0, 0]), rig(), "DW0938");
    assert!(m.contains("never wound up"), "{m}");
    assert_eq!(build_code(&landing([0, 0, 0]), rig()), None);
}

/// Shape 2: a strike clip whose last frame never comes down is refused naming
/// the cells; the clip that lays the slab builds.
#[test]
fn shape_two_end_to_end() {
    let q = quests_with(|q| {
        assembly(q)["strikes"]["pattern"][0]["strike"] = json!("windup");
    });
    let m = refusal(&q, rig(), "DW0938");
    assert!(
        m.contains("[4, 65, 7]") && m.contains("never reaches"),
        "{m}"
    );
    assert_eq!(build_code(&quests(), rig()), None);
}

/// Defect 1 end to end: the same slab laid two courses above the floor hangs
/// over the head of anybody standing there. A floor-to-three-above band read
/// it as reaching them; the standing body's space (feet and head cells) does
/// not, and the build is refused naming every caught cell.
#[test]
fn a_limb_that_never_reaches_the_body_is_refused() {
    let q = quests_with(|q| {
        assembly(q)["at"] = json!({ "anchor": "anchor/exit", "offset": [0, 2, 0] });
        assembly(q)["hitbox"] = json!({ "width": 1.0, "height": 4.0, "offset": [0, -2, 0] });
    });
    let m = refusal(&q, rig(), "DW0938");
    assert!(
        m.contains("never reaches") && m.contains("[5, 65, 8]"),
        "{m}"
    );
    assert!(m.contains("meets no standable cell's body space"), "{m}");
}

/// The fixture rig with its strike laying a slab three wide and seven long,
/// centred on the mark, its length along z.
fn long_rig() -> Rig {
    let mut r = rig();
    let last = r
        .clips
        .get_mut("strike")
        .unwrap()
        .frames
        .last_mut()
        .unwrap();
    last[0].translation = [-1.5, 0.0, -3.5];
    last[0].scale = [3.0, 0.5, 7.0];
    r
}

/// Defect 2 end to end: a one-cell landing under a seven-long limb is refused
/// naming the cells it comes down on and does not catch; the landing along the
/// limb's line, one cell in from its ends, builds.
#[test]
fn a_one_cell_landing_under_a_long_limb_is_refused() {
    let m = refusal(&quests(), long_rig(), "DW0938");
    assert!(
        m.contains("comes down on") && m.contains("does not land on"),
        "{m}"
    );
    let along = quests_with(|q| {
        let l = &mut assembly(q)["strikes"]["pattern"][0]["on_land"][0]["in"];
        *l = json!({ "anchor": "anchor/exit", "extent": [0, 0, 2] });
        assembly(q)["strikes"]["while_in"] =
            json!({ "anchor": "anchor/exit", "extent": [2, 1, 7] });
    });
    assert_eq!(build_code(&along, long_rig()), None);
}

/// An aimed pattern: the root turns to the facing nearest its target before
/// the wind-up, one landing function per facing a player can draw, each
/// tagging the turned region's players once; the blow lands one cadence after
/// the strike's last frame is applied; the record states every facing.
#[test]
fn an_aimed_strike_turns_and_lands_per_facing() {
    let q = quests_with(|q| {
        assembly(q)["strikes"]["aim"] = json!({ "facings": 4 });
        assembly(q)["strikes"]["while_in"] =
            json!({ "anchor": "anchor/exit", "extent": [2, 1, 2] });
    });
    let out = match try_build(&campaign(&q), &prefabs_with(rig())) {
        Ok(o) => o,
        Err(e) => panic!("{e:?}"),
    };
    let begin = function(&out, "asm_begin_limb");
    assert!(
        begin.contains("facing entity @a[") && begin.contains("Rotation[0] 4"),
        "{begin}"
    );
    for k in 0..4 {
        assert!(
            begin.contains(&format!("function {NS}:asm_aim_limb_{k}")),
            "{begin}"
        );
        let land = function(&out, &format!("asm_land_limb_0_{k}"));
        assert!(land.contains("add dw_asm_limb_struck"), "{land}");
        assert!(land.contains("run damage @s 4 minecraft:generic"), "{land}");
    }
    assert!(
        function(&out, "asm_aim_limb_1")
            .starts_with("tp @e[tag=dw_asm_limb_root,limit=1] 5.5 65 8.5 -90 0"),
        "{}",
        function(&out, "asm_aim_limb_1")
    );
    let tick = function(&out, "asm_tick_limb");
    assert!(
        tick.contains("if score #asm_limb_t dw.sys >= #asm_limb_tpf dw.sys run function hello-world:asm_land_limb"),
        "{tick}"
    );
    let record: Value =
        serde_json::from_slice(out.get("validation/assembly.json").unwrap()).unwrap();
    assert_eq!(record["facings"], 4, "{record:#}");
    assert_eq!(record["strike_steps"][0]["facing_count"], 4);
}

/// A one-frame wind-up, a hold of 0 and a blow of 40 build green with no
/// warning about the assembly at all: no telegraph rule and no one-shot rule
/// crept in (spec-0016 §3, applied in spec-0082 §5.4).
#[test]
fn no_telegraph_or_damage_rule_end_to_end() {
    let q = quests_with(|q| {
        let step = &mut assembly(q)["strikes"]["pattern"][0];
        step["windup"] = json!("retract");
        step["hold"] = json!(0);
        step["on_land"][0]["amount"] = json!(40);
    });
    assert_eq!(rig().clips["retract"].frames.len(), 1);
    let (out, warnings) = try_build_warned(&campaign(&q), &prefabs_with(rig())).expect("builds");
    for w in &warnings {
        assert!(
            !w.message.contains("assembly") && !w.code.starts_with("DW093"),
            "a finding about the blow: {} {}",
            w.code,
            w.message
        );
    }
    let record: Value =
        serde_json::from_slice(out.get("validation/assembly.json").unwrap()).unwrap();
    assert_eq!(record["strike_steps"][0]["hold"], 0);
    assert_eq!(record["strike_steps"][0]["amounts"], json!([40]));
    assert_eq!(record["strike_steps"][0]["windup_ticks"], 1);
}

// ---------------------------------------------------------------------------
// the path, the record and the suite
// ---------------------------------------------------------------------------

/// The count is three performances, after the beat that spawns the thing, each
/// naming the assembly and standing at its mark.
#[test]
fn the_path_strikes_three_times_after_the_spawn() {
    let out = build_fixture();
    let cp: Value = serde_json::from_slice(out.get("critical-path.json").unwrap()).unwrap();
    let steps = cp["steps"].as_array().unwrap();
    let talk = steps
        .iter()
        .position(|s| s["objective"] == "obj/talk")
        .unwrap();
    let strikes: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s["action"] == "trigger")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(strikes, vec![talk + 1, talk + 2, talk + 3], "{cp:#}");
    for i in strikes {
        assert_eq!(steps[i]["on"], "strike-assembly");
        assert_eq!(steps[i]["assembly"], "assembly/limb");
        assert_eq!(steps[i]["pos"], json!([5, 65, 8]));
        assert!(steps[i].get("anchor").is_none());
    }
}

/// The staging record states the step's numbers beside its caught cells.
#[test]
fn the_staging_record_states_the_blow() {
    let out = build_fixture();
    let record: Value =
        serde_json::from_slice(out.get("validation/assembly.json").unwrap()).unwrap();
    assert_eq!(record["declared"], 1);
    assert_eq!(record["parts"], 3);
    assert_eq!(record["hitboxes"], 1);
    let step = &record["strike_steps"][0];
    assert_eq!(step["windup_ticks"], 3);
    assert_eq!(step["hold"], 10);
    // Two frames at one tick: applied on tick 2, drawn whole on tick 3.
    assert_eq!(step["strike_ticks"], 3);
    assert_eq!(step["facing_count"], 1);
    assert_eq!(step["facings"][0]["stand"], json!([5, 65, 8]));
    assert_eq!(step["amounts"], json!([4]));
    assert_eq!(step["caught"].as_array().unwrap().len(), 9, "{record:#}");
}

/// The suite carries the three templates, and the spawn template counts the
/// root's passengers against the part count.
#[test]
fn the_generated_packtests() {
    let out = build_fixture();
    let t = |n: &str| {
        String::from_utf8(
            out.get(&format!("packtest-datapack/data/{NS}/test/{n}.mcfunction"))
                .unwrap_or_else(|| panic!("no `{n}` template"))
                .clone(),
        )
        .unwrap()
    };
    let spawn = t("asm_spawn_limb");
    assert!(spawn.contains("on passengers run scoreboard players add #asmn_limb dw.sys 1"));
    assert!(spawn.contains("assert score #asmn_limb dw.sys matches 3"));
    let hits = t("asm_hits_limb_limb_struck");
    assert!(hits.contains("{attack:{player:[I;0,0,0,1],timestamp:0L}}"));
    assert!(hits.contains("assert score #party dw.s_hits matches 1"));
    let k = rig().clip_index("retract").unwrap();
    assert!(hits.contains(&format!("assert score #asm_limb_clip dw.sys matches {k}")));
    let land = t("asm_land_limb");
    assert!(land.contains("assert score #asm_limb_lands dw.sys matches 1"));
}

// ---------------------------------------------------------------------------
// AC2 — the rig, at validation
// ---------------------------------------------------------------------------

/// Each structural defect of the rig is `DW0935` naming its field, and a clip
/// the rig lacks is `DW0935` listing the clips.
#[test]
fn a_broken_rig_is_dw0935_naming_the_field() {
    let refused = |r: Rig, field: &str| {
        let codes = validation_codes(&quests(), r);
        assert!(
            codes
                .iter()
                .any(|(c, _, m)| c == "DW0935" && m.contains(field)),
            "{field}: {codes:?}"
        );
    };
    let mut r = rig();
    r.parts[1].block = "minecraft:not_a_block".into();
    refused(r, "/parts/1/block");
    let mut r = rig();
    r.clips.get_mut("idle").unwrap().frames[0].pop();
    refused(r, "/clips/idle/frames/0");
    let mut r = rig();
    r.clips.get_mut("windup").unwrap().frames[1][2].scale = [1.0, 0.0, 1.0];
    refused(r, "/clips/windup/frames/1/2");
    for tpf in [0, 21] {
        let mut r = rig();
        r.clips.get_mut("strike").unwrap().ticks_per_frame = tpf;
        refused(r, "/clips/strike/ticks_per_frame");
    }
    let q = quests_with(|q| assembly(q)["initial"] = json!("fly"));
    let codes = validation_codes(&q, rig());
    assert!(
        codes.iter().any(|(c, p, m)| c == "DW0935"
            && p.ends_with("/initial")
            && m.contains("`idle`, `retract`, `strike`, `windup`")),
        "{codes:?}"
    );
    let q = quests_with(|q| trigger(q)["effects"][1]["clip"] = json!("fly"));
    assert!(
        validation_codes(&q, rig())
            .iter()
            .any(|(c, p, _)| c == "DW0935" && p.ends_with("/clip")),
        "{:?}",
        validation_codes(&q, rig())
    );
    let codes = validation_codes(&quests(), rig());
    assert!(codes.iter().all(|(c, _, _)| c != "DW0935"), "{codes:?}");
}

/// A rig the library does not hold is `DW0935`.
#[test]
fn a_missing_rig_is_dw0935() {
    let codes = common::validation_diagnostics(
        &campaign(&quests()),
        &FullItemRegistry::v1_21_11(),
        &PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap(),
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        codes
            .iter()
            .any(|d| d.code == "DW0935" && d.message.contains("rigs/limb/rig.json")),
        "{codes:?}"
    );
}

// ---------------------------------------------------------------------------
// AC2 / AC10 — the command line
// ---------------------------------------------------------------------------

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// A library directory holding only `rigs/limb/rig.json` (written from `r`).
fn rig_library(name: &str, r: &Rig) -> std::path::PathBuf {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let rigs = dir.join("rigs").join("limb");
    std::fs::create_dir_all(&rigs).unwrap();
    std::fs::write(
        rigs.join("rig.json"),
        serde_json::to_string_pretty(r).unwrap(),
    )
    .unwrap();
    dir
}

fn run(args: &[&str]) -> std::process::Output {
    std::process::Command::new(BIN)
        .args(args)
        .output()
        .expect("run delvec")
}

/// `delvec rig describe` prints the parts, every clip with its length in ticks
/// and per clip its last-frame footprint; two runs are byte-identical.
#[test]
fn rig_describe_prints_the_rig_and_is_deterministic() {
    let lib = rig_library("assembly_rig_describe", &rig());
    let lib = lib.to_str().unwrap();
    let a = run(&["--prefabs", lib, "rig", "describe", "rig/limb"]);
    let b = run(&["--prefabs", lib, "rig", "describe", "rig/limb"]);
    assert_eq!(
        a.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&a.stderr)
    );
    assert_eq!(a.stdout, b.stdout);
    let text = String::from_utf8(a.stdout).unwrap();
    assert!(
        text.contains("rig rig/limb: 3 part(s), 4 clip(s)"),
        "{text}"
    );
    assert!(
        text.contains("clip strike: 2 frame(s) every 1 tick(s), 2 tick(s)"),
        "{text}"
    );
    // The strike's last frame: the 3 x 3 slab round the mark.
    assert!(
        text.contains(
            "last-frame footprint, 10 cell(s) relative to the mark: [-1, 0, -1] [-1, 0, 0] [-1, 0, 1] [0, 0, -1] [0, 0, 0]"
        ),
        "{text}"
    );
    // `--facing` turns the footprint: a part one cell in front faces behind.
    let mut r = rig();
    r.clips.get_mut("retract").unwrap().frames[0][0].translation = [-0.5, 0.0, 1.5];
    let lib = rig_library("assembly_rig_describe_facing", &r);
    let north = run(&[
        "--prefabs",
        lib.to_str().unwrap(),
        "rig",
        "describe",
        "rig/limb",
        "--facing",
        "north",
    ]);
    let text = String::from_utf8(north.stdout).unwrap();
    assert!(text.contains("facing north"), "{text}");
    assert!(text.contains("[0, 0, -2]"), "{text}");
}

/// A rig that breaks a rule is refused with `DW0935` naming the field; a rig
/// the library does not hold is refused too.
#[test]
fn rig_describe_refuses_a_broken_or_missing_rig() {
    let mut r = rig();
    r.clips.get_mut("idle").unwrap().ticks_per_frame = 21;
    let lib = rig_library("assembly_rig_describe_broken", &r);
    let out = run(&[
        "--prefabs",
        lib.to_str().unwrap(),
        "rig",
        "describe",
        "rig/limb",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        said.contains("DW0935") && said.contains("/clips/idle/ticks_per_frame"),
        "{said}"
    );
    let out = run(&[
        "--prefabs",
        lib.to_str().unwrap(),
        "rig",
        "describe",
        "rig/absent",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        said.contains("DW0935") && said.contains("rigs/absent/rig.json"),
        "{said}"
    );
}

/// The binding line is printed on every build: zeroes on a campaign with no
/// assembly, one assembly on the fixture.
#[test]
fn the_binding_line_is_printed_on_every_build() {
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("assembly_binding_line");
    let _ = std::fs::remove_dir_all(&tmp);
    // The primary without the element: hello-world as shipped.
    let plain = run(&[
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        tmp.join("plain-out").to_str().unwrap(),
    ]);
    assert_eq!(
        plain.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );
    assert!(
        String::from_utf8_lossy(&plain.stderr).contains(
            "assembly binding: 0 assembl(ies) declared, 0 part(s), 0 clip(s), 0 hitbox(es) \
             examined, 0 strike step(s) checked over 0 facing(s), 0 refused"
        ),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );
    // The fixture: a library with the rig, a campaign directory with the
    // assembly.
    let lib = tmp.join("prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &lib);
    let rigs = lib.join("rigs").join("limb");
    std::fs::create_dir_all(&rigs).unwrap();
    std::fs::write(
        rigs.join("rig.json"),
        serde_json::to_string(&rig()).unwrap(),
    )
    .unwrap();
    let dir = tmp.join("campaign");
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    std::fs::write(dir.join("quests.json"), quests().to_string()).unwrap();
    let with = run(&[
        "--prefabs",
        lib.to_str().unwrap(),
        "build",
        dir.to_str().unwrap(),
        "-o",
        tmp.join("out").to_str().unwrap(),
    ]);
    let err = String::from_utf8_lossy(&with.stderr);
    assert_eq!(with.status.code(), Some(0), "{err}");
    assert!(
        err.contains(
            "assembly binding: 1 assembl(ies) declared, 3 part(s), 4 clip(s), 1 hitbox(es) \
             examined, 1 strike step(s) checked over 1 facing(s), 0 refused"
        ),
        "{err}"
    );
    assert!(
        err.contains("assembly cost: 3 display part(s) in all"),
        "{err}"
    );
}

/// The press ledger (`DW0426`'s artifact) records a `strike-assembly` as a
/// press resolved to the assembly's hitbox, so the strike is counted where
/// every other click is.
#[test]
fn the_press_ledger_records_the_strike_on_the_hitbox() {
    let out = build_fixture();
    let v: Value =
        serde_json::from_slice(out.get("validation/press-bodies.json").unwrap()).unwrap();
    assert_eq!(v["examined"], 1, "{v:#}");
    assert_eq!(v["presses"][0]["trigger"], "trigger/limb-struck");
    assert!(
        v["presses"][0]["body"]
            .as_str()
            .unwrap()
            .contains("rides assembly `assembly/limb`'s hitbox, 1 x 2 standing on [5, 65, 8]"),
        "{v:#}"
    );
}
