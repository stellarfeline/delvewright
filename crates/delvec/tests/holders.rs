//! spec-0105, spec-0111: every holder is loot, a collect, a device, scenery or
//! exempt — the census, its
//! binding line and ledger, the arrival notice, and each refusal shape
//! (`DW1018`, `DW1019`, `DW1024`) on a fixture built through the binary.
//!
//! The fixture is hello-world furnished by a stage-7 batch
//! ([`common::container_campaign`]): hello-room carries no container of its
//! own, so every container a test counts is one the test placed.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BARREL: &str = "minecraft:barrel";

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("containers-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Build the fixture; return the exit code, what it said and the output dir.
fn build(
    tag: &str,
    blocks: &[([i32; 3], &str)],
    collect: Option<&str>,
    notice: bool,
) -> (i32, String, PathBuf) {
    let dir = scratch(tag);
    let camp = common::container_campaign(&dir.join("camp"), blocks, collect, notice);
    let out = dir.join("out");
    let prefabs = common::prefabs_dir();
    let o = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o.status.code().unwrap_or(-1), said(&o), out)
}

fn census_line(log: &str) -> &str {
    log.lines()
        .find(|l| l.starts_with("holder census:"))
        .unwrap_or_else(|| panic!("no census line in:\n{log}"))
}

fn class_apply_bodies(out: &Path) -> Vec<String> {
    let dir = out.join("datapack/data/hello-world/function");
    let mut bodies = Vec::new();
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("class_apply_") {
            bodies.push(std::fs::read_to_string(&p).unwrap());
        }
    }
    assert!(
        !bodies.is_empty(),
        "no class_apply_* under {}",
        dir.display()
    );
    bodies
}

/// spec-0105 criterion 7: a campaign with no container prints the line with
/// a measured zero, and still writes the ledger.
#[test]
fn a_campaign_with_no_container_prints_a_measured_zero() {
    let (code, log, out) = build("none", &[], None, false);
    assert_eq!(code, 0, "{log}");
    let line = census_line(&log);
    assert!(line.contains("0 holder(s)"), "{line}");
    assert!(line.contains("notice none"), "{line}");
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/holders.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["cells"], 0);
    assert!(ledger["notice"].is_null());
    // criterion 3: no notice, no line in any class apply.
    for body in class_apply_bodies(&out) {
        assert!(!body.contains("tellraw"), "{body}");
    }
}

/// `DW1018`, first arm: scenery with no notice. The notice is the remedy, and
/// with it the build is green, the ledger carries the row and the notice, and
/// every class apply tells it after `dw.classed` and after the teleport, its
/// sound played `at` the player — at the arrival, never at the command source.
#[test]
fn scenery_with_no_notice_is_dw1018_and_the_notice_builds() {
    let (code, log, _) = build("bare", &[([-3, 0, -2], BARREL)], None, false);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1018"), "{log}");
    assert!(log.contains("`minecraft:barrel` at"), "{log}");
    assert!(log.contains("area/keep"), "{log}");

    let (code, log, out) = build("told", &[([-3, 0, -2], BARREL)], None, true);
    assert_eq!(code, 0, "{log}");
    let line = census_line(&log);
    assert!(
        line.contains("1 holder(s) in the assembled world (container: 1)"),
        "{line}"
    );
    assert!(
        line.contains(
            "0 loot, 0 collect, 0 device, 1 decoration, 0 empty (frozen), 0 exempt (contents show from outside); notice declared"
        ),
        "{line}"
    );
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/holders.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["decoration"], 1);
    assert_eq!(
        ledger["notice"]["text"],
        "The keep was stripped bare before you came."
    );
    assert_eq!(ledger["notice"]["style"], "chat");
    assert_eq!(ledger["notice"]["sound"], "minecraft:block.barrel.close");
    let row = &ledger["rows"][0];
    assert_eq!(row["block"], BARREL);
    assert_eq!(row["kind"], "container");
    assert_eq!(row["slots"].as_array().unwrap().len(), 27);
    assert_eq!(row["exempt"], false);
    assert_eq!(row["place"], "area/keep");
    assert_eq!(row["claims"], serde_json::json!([]));
    assert_eq!(ledger["census_input_sha256"].as_str().unwrap().len(), 64);
    for body in class_apply_bodies(&out) {
        let lines: Vec<&str> = body.lines().collect();
        let classed = lines
            .iter()
            .position(|l| *l == "scoreboard players set @s dw.classed 1")
            .expect("dw.classed line");
        let told = lines
            .iter()
            .position(|l| l.starts_with("tellraw @s") && l.contains("stripped bare"))
            .expect("the notice line");
        let warp = lines
            .iter()
            .position(|l| l.starts_with("teleport @s"))
            .expect("teleport line");
        let sound = lines
            .iter()
            .position(|l| {
                *l == "execute as @s at @s run playsound minecraft:block.barrel.close player @s"
            })
            .expect("the notice's sound, played at the player");
        assert_eq!(
            lines.iter().filter(|l| l.contains("playsound")).count(),
            1,
            "the sound is played once, at the player, and nowhere else: {body}"
        );
        assert!(classed < warp && warp < told && told < sound, "{body}");
    }
}

/// `DW1018`, second arm: a notice over a world whose every container is
/// claimed (here a `collect`'s own chest, counted from its declaration).
#[test]
fn a_notice_over_a_stocked_world_is_dw1018() {
    let (code, log, _) = build("stocked", &[], Some("anchor/exit"), true);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1018"), "{log}");
    assert!(
        log.contains("Delete `containers.decoration_notice`"),
        "{log}"
    );
    let line = census_line(&log);
    assert!(line.contains("(container: 1)"), "{line}");
    assert!(
        line.contains("0 loot, 1 collect, 0 device, 0 decoration"),
        "{line}"
    );
}

/// `DW1024`, mixed: the collect's chest stands in the same place as a
/// scenery barrel.
#[test]
fn a_container_that_matters_among_scenery_is_dw1024() {
    let (code, log, _) = build("mixed", &[([-3, 0, -2], BARREL)], Some("anchor/exit"), true);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1024"), "{log}");
    assert!(log.contains("MIXED — `area/keep`"), "{log}");
    assert!(census_line(&log).contains("1 mixed place(s)"), "{log}");
}

/// `DW1024`, storeroom: three scenery cells in one place, over the cap of two.
#[test]
fn a_storeroom_of_nothing_is_dw1024() {
    let (code, log, _) = build(
        "storeroom",
        &[
            ([-3, 0, -2], BARREL),
            ([3, 0, -2], BARREL),
            ([-3, 0, -1], BARREL),
        ],
        None,
        true,
    );
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1024"), "{log}");
    assert!(log.contains("STOREROOM — `area/keep` holds 3"), "{log}");
    assert!(census_line(&log).contains("1 over the cap of 2"), "{log}");
}

fn validate(tag: &str, f: impl FnOnce(&mut serde_json::Value)) -> (i32, String) {
    let dir = scratch(tag);
    let camp = common::container_campaign(&dir.join("camp"), &[], None, false);
    common::patch_file(&camp.join("quests.json"), f);
    let prefabs = common::prefabs_dir();
    let o = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o.status.code().unwrap_or(-1), said(&o))
}

/// `DW1019`: a `loot` fill with no stack.
#[test]
fn an_empty_loot_fill_is_dw1019() {
    let (code, log) = validate("empty-loot", |q| {
        q["content"]["loot"] =
            serde_json::json!([{ "id": "loot/nothing", "anchor": "anchor/exit", "items": [] }]);
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(log.contains("/content/loot/0/items"), "{log}");
}

/// `DW1019`: a `containers` object that declares nothing.
#[test]
fn an_empty_containers_object_is_dw1019() {
    let (code, log) = validate("empty-containers", |q| {
        q["content"]["containers"] = serde_json::json!({});
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(log.contains("/content/containers"), "{log}");
}

/// `DW1019`: a runtime `set-block` of a container, reached inside a nested
/// effect list as well as at the top of a bundle.
#[test]
fn a_container_set_at_runtime_is_dw1019() {
    let (code, log) = validate("runtime-chest", |q| {
        let effs = q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        effs.push(serde_json::json!({
            "type": "set-block", "anchor": "anchor/exit", "block": "minecraft:smoker[facing=north]"
        }));
    });
    assert_eq!(code, 1, "{log}");
    assert!(log.contains("DW1019"), "{log}");
    assert!(
        log.contains("places `minecraft:smoker[facing=north]` at runtime"),
        "{log}"
    );
}

// ---------------------------------------------------------------------------
// spec-0111: holder entities the piece carries
// ---------------------------------------------------------------------------

use delvec::admit::Nbt;

use common::{FRAME, STAND, frame_entity, holder_entity, holder_prefabs, stand_entity};

/// Build hello-world with `quests` patched over the library `prefabs`, an
/// optional stage-7 `edits` list over `area/keep`.
fn build_holders(
    tag: &str,
    prefabs: &Path,
    quests: impl FnOnce(&mut serde_json::Value),
    edits: Option<serde_json::Value>,
) -> (i32, String, PathBuf) {
    let dir = scratch(tag);
    let camp = dir.join("camp");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |q| quests(&mut q["content"]));
    if let Some(edits) = edits {
        let doc = serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "world-edits",
            "content": {"batches": [{"area": "area/keep", "id": "batch/holders", "edits": edits}]}
        });
        std::fs::write(
            camp.join("world-edits.json"),
            serde_json::to_string_pretty(&doc).unwrap(),
        )
        .unwrap();
    }
    let out = dir.join("out");
    let o = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (o.status.code().unwrap_or(-1), said(&o), out)
}

fn loot_on_holders(c: &mut serde_json::Value) {
    c["loot"] = serde_json::json!([
        {"id": "loot/map", "anchor": "anchor/frame",
         "items": [{"item": "minecraft:map", "slot": "contents"}]},
        {"id": "loot/rack", "anchor": "anchor/rack",
         "items": [{"item": "minecraft:iron_helmet", "slot": "armor.head"},
                   {"item": "minecraft:iron_sword", "slot": "weapon.mainhand"}]}
    ]);
}

fn read(out: &Path, rel: &str) -> String {
    std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// spec-0111 criteria 4, 5, 9: the build strips the holders from the template
/// it ships, summons each with the NBT the engine owns, fills every claimed
/// slot by `item replace entity`, freezes an emptied frame, writes
/// `entity_drops`, ledgers the holders and drives each with a PackTest.
#[test]
fn holders_are_stripped_summoned_filled_and_ledgered() {
    let dir = scratch("holders-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity(), stand_entity()]);
    let (code, log, out) = build_holders("holders-green", &prefabs, loot_on_holders, None);
    assert_eq!(code, 0, "{log}");
    let line = census_line(&log);
    assert!(
        line.contains("2 holder(s) in the assembled world (armor-stand: 1, glow-item-frame: 1)"),
        "{line}"
    );
    assert!(line.contains("2 loot, 0 collect"), "{line}");
    assert!(
        line.contains("2 exempt (contents show from outside)"),
        "{line}"
    );
    assert!(line.contains("0 unsupported, 0 unreachable"), "{line}");
    // The shipped template carries no entity.
    let shipped =
        std::fs::read(out.join("datapack/data/hello-world/structure/hello-room.nbt")).unwrap();
    assert!(
        delvec::admit::structure::Structure::read(&shipped)
            .unwrap()
            .entities
            .is_empty()
    );
    let fdir = "datapack/data/hello-world/function";
    let finish = read(&out, &format!("{fdir}/setup_finish.mcfunction"));
    assert!(
        finish.contains("function hello-world:holder_frame"),
        "{finish}"
    );
    assert!(
        finish.contains("function hello-world:holder_rack"),
        "{finish}"
    );
    let frame = read(&out, &format!("{fdir}/holder_frame.mcfunction"));
    assert!(
        frame.contains("summon minecraft:glow_item_frame"),
        "{frame}"
    );
    assert!(
        frame.contains("Fixed:0b,Invulnerable:0b,ItemDropChance:1.0f")
            && frame.contains("Facing:5b,Pos:[1.5d,66.5d,2.5d],block_pos:[I;1,66,2]"),
        "{frame}"
    );
    assert!(
        frame.contains(
            "item replace entity @e[tag=dw_holder_frame,limit=1] contents with minecraft:map 1"
        ),
        "{frame}"
    );
    let rack = read(&out, &format!("{fdir}/holder_rack.mcfunction"));
    let bits = delvewright_dsl::holders::disabled_slots(&["armor.head", "weapon.mainhand"]);
    assert!(
        rack.contains(&format!(
            "ShowArms:1b,Marker:0b,Invulnerable:1b,DisabledSlots:{bits}"
        )),
        "{rack}"
    );
    assert!(
        rack.contains("weapon.mainhand with minecraft:iron_sword 1"),
        "{rack}"
    );
    let tick = read(&out, &format!("{fdir}/tick.mcfunction"));
    assert!(
        tick.contains(
            "execute as @e[type=#hello-world:frames,tag=dw_holder] unless items entity @s \
             contents * run data merge entity @s {Fixed:1b,Invulnerable:1b}"
        ),
        "{tick}"
    );
    assert!(
        read(
            &out,
            "datapack/data/hello-world/tags/entity_type/frames.json"
        )
        .contains("glow_item_frame")
    );
    assert!(read(&out, &format!("{fdir}/setup.mcfunction")).contains("gamerule entity_drops true"));
    let ledger: serde_json::Value =
        serde_json::from_str(&read(&out, "validation/holders.json")).unwrap();
    assert_eq!(ledger["holders"], 2);
    assert_eq!(ledger["exempt"], 2);
    for t in ["holder_frame", "holder_rack"] {
        let p = format!("packtest-datapack/data/hello-world/test/{t}.mcfunction");
        assert!(out.join(&p).is_file(), "{p}");
    }
    assert!(
        read(
            &out,
            "packtest-datapack/data/hello-world/test/holder_frame.mcfunction"
        )
        .contains("{Fixed:1b,Invulnerable:1b}")
    );
}

/// spec-0111 criterion 10: a `collect` adopting a frame fills it at
/// activation, unfreezes it, and its critical-path step carries `holder`.
#[test]
fn a_collect_adopting_a_frame_fills_it_at_activation_and_tells_the_bot() {
    let dir = scratch("holders-collect-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity()]);
    let (code, log, out) = build_holders(
        "holders-collect",
        &prefabs,
        |c| {
            c["quests"][0]["objectives"][1] = serde_json::json!({
                "type": "collect", "id": "obj/exit", "item": "minecraft:map", "count": 1,
                "anchor": "anchor/exit", "holder": "anchor/frame", "after": ["obj/talk"],
                "title": "Take the map", "item_name": "Keeper's Map",
                "happening": {"text": "the party takes the map", "verb": "gains"}
            });
        },
        None,
    );
    assert_eq!(code, 0, "{log}");
    let act = read(
        &out,
        "datapack/data/hello-world/function/activate_o_exit.mcfunction",
    );
    assert!(
        act.contains(
            "item replace entity @e[tag=dw_holder_frame,limit=1] contents with minecraft:map"
        ),
        "{act}"
    );
    assert!(
        act.contains(
            "data merge entity @e[tag=dw_holder_frame,limit=1] {Fixed:0b,Invulnerable:0b}"
        ),
        "{act}"
    );
    // Summoned frozen until the activation fills it.
    let frame = read(
        &out,
        "datapack/data/hello-world/function/holder_frame.mcfunction",
    );
    assert!(frame.contains("Fixed:1b,Invulnerable:1b"), "{frame}");
    let cp: serde_json::Value = serde_json::from_str(&read(&out, "critical-path.json")).unwrap();
    assert_eq!(
        cp["format_version"],
        delvec::compiler::plan::CRITICAL_PATH_FORMAT_VERSION
    );
    let step = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/exit")
        .unwrap();
    assert_eq!(step["holder"]["kind"], "glow-item-frame");
    assert_eq!(step["holder"]["slot"], "contents");
    assert!(step["holder"]["stance"].is_array(), "{step}");
    assert!(step["holder"].get("click_height").is_none(), "{step}");
    assert!(
        out.join("packtest-datapack/data/hello-world/test/obj_activate_exit.mcfunction")
            .is_file()
    );
}

/// spec-0104 §7 × spec-0111: a `highlight` on a `collect` taken from a holder
/// entity lights the holder itself — its `Glowing` flag and the highlight
/// team — for a frame and for a stand; the block map holds air at a holder,
/// and a highlight that read only blocks was dropped there with no word.
#[test]
fn a_highlight_on_a_holder_collect_lights_the_holder() {
    let dir = scratch("holders-highlight-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity(), stand_entity()]);
    for (tag, holder, extra) in [
        (
            "holders-highlight-frame",
            "anchor/frame",
            serde_json::json!({"item": "minecraft:map", "slot": "contents"}),
        ),
        (
            "holders-highlight-stand",
            "anchor/rack",
            serde_json::json!({"item": "minecraft:iron_sword", "slot": "weapon.mainhand"}),
        ),
    ] {
        let (code, log, out) = build_holders(
            tag,
            &prefabs,
            |c| {
                let mut o = serde_json::json!({
                    "type": "collect", "id": "obj/exit", "count": 1,
                    "anchor": "anchor/exit", "holder": holder, "after": ["obj/talk"],
                    "title": "Take it", "item_name": "The Thing",
                    "happening": {"text": "the party takes it", "verb": "gains"},
                    "highlight": {"within": 6}
                });
                for (k, v) in extra.as_object().unwrap() {
                    o[k] = v.clone();
                }
                c["quests"][0]["objectives"][1] = o;
            },
            None,
        );
        assert_eq!(code, 0, "{tag}: {log}");
        let stem = holder.trim_start_matches("anchor/");
        let fdir = "datapack/data/hello-world/function";
        let on = read(&out, &format!("{fdir}/hl_on_o_exit.mcfunction"));
        assert!(
            on.contains(&format!(
                "execute as @e[tag=dw_holder_{stem}] run data merge entity @s {{Glowing:1b}}"
            )) && on.contains(&format!("team join dw_hl @e[tag=dw_holder_{stem}]")),
            "{tag}: {on}"
        );
        let off = read(&out, &format!("{fdir}/hl_off_o_exit.mcfunction"));
        assert!(off.contains("{Glowing:0b}"), "{tag}: {off}");
        let poll = read(&out, &format!("{fdir}/hl_poll_o_exit.mcfunction"));
        assert!(poll.contains(&format!("as @e[tag=dw_holder_{stem}] at @s")), "{tag}: {poll}");
        let pt = read(
            &out,
            "packtest-datapack/data/hello-world/test/guide_highlight_o_exit.mcfunction",
        );
        assert!(
            pt.contains(&format!("function hello-world:holder_{stem}"))
                && pt.contains(&format!("@e[tag=dw_holder_{stem},team=dw_hl,nbt={{Glowing:1b}}]")),
            "{tag}: {pt}"
        );
    }
}

/// `DW1037`, unsupported arm: a frame whose backing block is cleared to air.
/// (Glass is legacy-solid in the pinned game — `BlockBehaviour.calculateSolid`
/// — so a frame on glass survives; air is what drops one.)
#[test]
fn a_frame_on_nothing_is_dw1037_unsupported() {
    let dir = scratch("holders-air-prefabs");
    let mut e = frame_entity();
    // Facing west: hung on the block east of it, the room's open air.
    if let Nbt::Compound(c) = &mut e
        && let Some(Nbt::Compound(n)) = c.get_mut("nbt")
    {
        n.insert("Facing".to_string(), Nbt::Byte(4));
    }
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![e]);
    let (code, log, _) = build_holders("holders-unsupported", &prefabs, |_| {}, None);
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1037"), "{log}");
    assert!(log.contains("UNSUPPORTED"), "{log}");
    assert!(
        census_line(&log).contains("1 unsupported, 0 unreachable"),
        "{log}"
    );
}

/// `DW1037`, unreachable arm: a filled frame walled in by a stage-7 batch.
#[test]
fn a_frame_behind_a_wall_is_dw1037_unreachable() {
    let dir = scratch("holders-walled-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity()]);
    let mut edits = Vec::new();
    // A shell round the frame's cell: everything in front of it, under it,
    // over it and beside it, so no standable cell sees its face.
    let shell: [([i32; 3], [i32; 3]); 5] = [
        ([1, -1, -1], [2, 2, 1]),
        ([0, -1, -1], [0, -1, 1]),
        ([0, 1, -1], [0, 2, 1]),
        ([0, 0, -1], [0, 0, -1]),
        ([0, 0, 1], [0, 0, 1]),
    ];
    for (i, (min, max)) in shell.iter().enumerate() {
        edits.push(serde_json::json!({
            "verb": "select", "name": format!("region/w{i}"),
            "shape": {"frame": {"kind": "anchor-relative", "anchor": "anchor/frame"},
                      "kind": "box", "min": min, "max": max}
        }));
        edits.push(serde_json::json!({
            "verb": "fill", "region": format!("region/w{i}"),
            "recipe": {"blocks": [{"block": "minecraft:stone", "weight": 1.0}]}
        }));
    }
    let (code, log, _) = build_holders(
        "holders-unreachable",
        &prefabs,
        |c| {
            c["loot"] = serde_json::json!([{"id": "loot/map", "anchor": "anchor/frame",
                "items": [{"item": "minecraft:map"}]}]);
        },
        Some(serde_json::Value::Array(edits)),
    );
    assert_eq!(code, 3, "{log}");
    assert!(log.contains("DW1037"), "{log}");
    assert!(log.contains("UNREACHABLE"), "{log}");
}

/// `DW1038`: a `clear-region` over a frame's backing block, and one over a
/// stand's foot block.
#[test]
fn a_runtime_edit_reaching_a_holder_is_dw1038() {
    let dir = scratch("holders-edit-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity(), stand_entity()]);
    for (tag, anchor, extent) in [
        ("holders-edit-frame", "anchor/frame", [1, 0, 0]),
        ("holders-edit-stand", "anchor/rack", [0, 1, 0]),
    ] {
        let (code, log, _) = build_holders(
            tag,
            &prefabs,
            |c| {
                loot_on_holders(c);
                c["quests"][0]["on_objective_complete"]["obj/talk"]
                    .as_array_mut()
                    .unwrap()
                    .push(serde_json::json!({
                        "type": "clear-region",
                        "region": {"anchor": anchor, "extent": extent},
                        "happening": {"text": "the wall falls", "verb": "opens"}
                    }));
            },
            None,
        );
        assert_eq!(code, 3, "{tag}: {log}");
        assert!(log.contains("DW1038"), "{tag}: {log}");
    }
}

/// The fill proofs over holder entities: a slot the holder's family lacks is
/// `DW0431`; a second stack in a frame is `DW0431`; a stand stack with no slot
/// and a `collect` padding a frame are `DW1035` at the build tier.
#[test]
fn a_fill_the_holder_cannot_take_is_refused() {
    let dir = scratch("holders-fill-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity(), stand_entity()]);
    type Patch = fn(&mut serde_json::Value);
    let cases: [(&str, &str, Patch); 4] = [
        ("holders-chest-slot-on-stand", "DW0431", |c| {
            c["loot"] = serde_json::json!([{"id": "loot/rack", "anchor": "anchor/rack",
                "items": [{"item": "minecraft:bread", "slot": "contents"}]}]);
        }),
        ("holders-second-stack", "DW0431", |c| {
            c["loot"] = serde_json::json!([{"id": "loot/map", "anchor": "anchor/frame",
                "items": [{"item": "minecraft:map"}, {"item": "minecraft:paper"}]}]);
        }),
        ("holders-stand-no-slot", "DW1035", |c| {
            c["loot"] = serde_json::json!([{"id": "loot/rack", "anchor": "anchor/rack",
                "items": [{"item": "minecraft:iron_sword"}]}]);
        }),
        ("holders-padded-frame", "DW1035", |c| {
            c["quests"][0]["objectives"][1] = serde_json::json!({
                "type": "collect", "id": "obj/exit", "item": "minecraft:map", "count": 1,
                "anchor": "anchor/exit", "holder": "anchor/frame", "after": ["obj/talk"],
                "fill_count": 2, "title": "Take the map", "item_name": "Keeper's Map",
                "happening": {"text": "the party takes the map", "verb": "gains"}
            });
        }),
    ];
    for (tag, code_want, patch) in cases {
        let (code, log, _) = build_holders(tag, &prefabs, patch, None);
        assert_eq!(code, 3, "{tag}: {log}");
        assert!(log.contains(code_want), "{tag}: {log}");
    }
}

/// `DW1035`'s armour arm reads the pinned item data: a sword in `armor.chest`.
#[test]
fn a_sword_on_a_chest_slot_is_dw1035() {
    let dir = scratch("holders-armor-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![stand_entity()]);
    let camp = dir.join("camp");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |q| {
        q["content"]["loot"] = serde_json::json!([{"id": "loot/rack", "anchor": "anchor/rack",
            "items": [{"item": "minecraft:iron_sword", "slot": "armor.chest"}]}]);
    });
    let o = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    let log = said(&o);
    assert_ne!(o.status.code(), Some(0), "{log}");
    assert!(log.contains("DW1035"), "{log}");
    assert!(log.contains("armor.chest"), "{log}");
}

/// Admission (spec-0111 §3.1, criterion 3): a holder carrying an owned key is
/// `DW1036` naming the key; any other kind is `DW0730`; a holder saying only
/// what it looks like passes.
#[test]
fn the_audit_judges_a_templates_entities() {
    let dir = scratch("holders-audit");
    let src = common::prefabs_dir().join("hello-room.nbt");
    let audit = |name: &str, entities: Vec<Nbt>| {
        let mut s =
            delvec::admit::structure::Structure::read(&std::fs::read(&src).unwrap()).unwrap();
        s.entities = entities;
        let p = dir.join(format!("{name}.nbt"));
        std::fs::write(&p, s.write()).unwrap();
        delvec(&["prefab", "audit", p.to_str().unwrap()])
    };
    let ok = audit("plain", vec![frame_entity(), stand_entity()]);
    assert_eq!(ok.status.code(), Some(0), "{}", said(&ok));
    let item = Nbt::Compound(
        [("id".to_string(), Nbt::String("minecraft:map".to_string()))]
            .into_iter()
            .collect(),
    );
    let stocked = audit(
        "stocked",
        vec![holder_entity(
            "minecraft:item_frame",
            FRAME,
            true,
            &[("Facing", Nbt::Byte(5)), ("Item", item)],
        )],
    );
    let log = said(&stocked);
    assert_ne!(stocked.status.code(), Some(0), "{log}");
    assert!(log.contains("DW1036") && log.contains("`Item`"), "{log}");
    let pig = audit(
        "pig",
        vec![holder_entity("minecraft:pig", STAND, false, &[])],
    );
    let log = said(&pig);
    assert_ne!(pig.status.code(), Some(0), "{log}");
    assert!(
        log.contains("DW0730") && log.contains("minecraft:pig"),
        "{log}"
    );
}

/// A `collect` objective on the second step of hello-world that adopts the
/// stand at `anchor/rack`, with `extra` fields merged over it.
fn sword_collect(extra: serde_json::Value) -> impl FnOnce(&mut serde_json::Value) {
    move |c| {
        let mut o = serde_json::json!({
            "type": "collect", "id": "obj/exit", "item": "minecraft:iron_sword", "count": 1,
            "anchor": "anchor/exit", "holder": "anchor/rack", "after": ["obj/talk"],
            "title": "Take the sword", "item_name": "Post Sword",
            "happening": {"text": "the party takes the sword", "verb": "gains"}
        });
        for (k, v) in extra.as_object().unwrap() {
            o[k] = v.clone();
        }
        c["quests"][0]["objectives"][1] = o;
    }
}

/// spec-0111 §3.4: a `collect` adopting a stand names the slot its item goes
/// in — the sword stands in the stand's hand, the activation fills that slot,
/// the stand shows its arms and lets only that slot be taken, and the bot is
/// told to click the hand.
#[test]
fn a_collect_puts_its_item_in_the_slot_it_names() {
    let dir = scratch("holders-slot-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![stand_entity()]);
    let (code, log, out) = build_holders(
        "holders-slot",
        &prefabs,
        sword_collect(serde_json::json!({"slot": "weapon.mainhand"})),
        None,
    );
    assert_eq!(code, 0, "{log}");
    let act = read(
        &out,
        "datapack/data/hello-world/function/activate_o_exit.mcfunction",
    );
    assert!(
        act.contains(
            "item replace entity @e[tag=dw_holder_rack,limit=1] weapon.mainhand with \
             minecraft:iron_sword"
        ),
        "{act}"
    );
    assert!(!act.contains("armor.head"), "{act}");
    let rack = read(
        &out,
        "datapack/data/hello-world/function/holder_rack.mcfunction",
    );
    let bits = delvewright_dsl::holders::disabled_slots(&["weapon.mainhand"]);
    assert!(
        rack.contains(&format!(
            "ShowArms:1b,Marker:0b,Invulnerable:1b,DisabledSlots:{bits}"
        )),
        "{rack}"
    );
    let cp: serde_json::Value = serde_json::from_str(&read(&out, "critical-path.json")).unwrap();
    let step = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/exit")
        .unwrap();
    assert_eq!(step["holder"]["kind"], "armor-stand", "{step}");
    assert_eq!(step["holder"]["slot"], "weapon.mainhand", "{step}");
    assert!(step["holder"]["click_height"].is_number(), "{step}");
}

/// spec-0111 §3.4 with §3.3's rule: a stand's slots have no order, so a
/// `collect` adopting one names its slot (`DW1035` at the build, where the
/// family is known), exactly as a `loot` stack on a stand does; a slot the
/// stand does not have is `DW0438`.
#[test]
fn a_collect_on_a_stand_names_a_slot_the_stand_has() {
    let dir = scratch("holders-slot-refused-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![stand_entity()]);
    for (tag, extra, want) in [
        ("holders-slot-none", serde_json::json!({}), "DW1035"),
        (
            "holders-slot-frame-word",
            serde_json::json!({"slot": "contents"}),
            "DW0438",
        ),
    ] {
        let (code, log, _) = build_holders(tag, &prefabs, sword_collect(extra), None);
        assert_eq!(code, 3, "{tag}: {log}");
        assert!(log.contains(want), "{tag}: {log}");
    }
}

/// spec-0111 §3.4: a `collect`'s `slot` passes the one validation a `loot`
/// stack's does — a sword in `armor.chest` is `DW1035` naming the slot — and
/// only a collect that adopts a `holder` names one.
#[test]
fn a_collect_slot_is_judged_as_a_loot_slot_is() {
    let dir = scratch("holders-slot-validate-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![stand_entity()]);
    for (tag, extra, said_too) in [
        (
            "holders-slot-chest",
            serde_json::json!({"slot": "armor.chest"}),
            "armor.chest",
        ),
        (
            "holders-slot-no-holder",
            serde_json::json!({"slot": "weapon.mainhand", "holder": null}),
            "adopts no `holder`",
        ),
        (
            "holders-slot-unknown",
            serde_json::json!({"slot": "weapon.belt"}),
            "which no holder has",
        ),
    ] {
        let camp = scratch(tag).join("camp");
        common::copy_dir_all(&common::hello_world_dir(), &camp);
        common::patch_file(&camp.join("quests.json"), |q| {
            sword_collect(extra)(&mut q["content"]);
            let o = &mut q["content"]["quests"][0]["objectives"][1];
            if o["holder"].is_null() {
                o.as_object_mut().unwrap().remove("holder");
            }
        });
        let o = delvec(&[
            "validate",
            camp.to_str().unwrap(),
            "--prefabs",
            prefabs.to_str().unwrap(),
        ]);
        let log = said(&o);
        assert_ne!(o.status.code(), Some(0), "{tag}: {log}");
        assert!(log.contains("DW1035"), "{tag}: {log}");
        assert!(log.contains(said_too), "{tag}: {log}");
    }
}

/// spec-0111 §5.2: a holder entity's own cell, and the cell of every holder
/// beside it, is no stance from which a holder is judged. The block model
/// treats a stand as air, so before this the stance nearest a stand was the
/// cell it stands in — the critical path ended inside the stand, and the walk
/// went through the stand beside it.
#[test]
fn a_holders_own_cell_and_its_neighbours_are_no_stance() {
    let dir = scratch("holders-stance-prefabs");
    let beside = [STAND[0] - 1, STAND[1], STAND[2]];
    let prefabs = holder_prefabs(
        &dir.join("prefabs"),
        vec![
            stand_entity(),
            holder_entity("minecraft:armor_stand", beside, false, &[]),
        ],
    );
    let (code, log, out) = build_holders(
        "holders-stance",
        &prefabs,
        sword_collect(serde_json::json!({"slot": "weapon.mainhand"})),
        None,
    );
    assert_eq!(code, 0, "{log}");
    let cp: serde_json::Value = serde_json::from_str(&read(&out, "critical-path.json")).unwrap();
    let step = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["objective"] == "obj/exit")
        .unwrap();
    let stance: Vec<i64> = serde_json::from_value(step["holder"]["stance"].clone())
        .unwrap_or_else(|e| panic!("{e}: {step}"));
    let pos: Vec<i64> = serde_json::from_value(step["pos"].clone()).unwrap();
    // The two stands' world cells: the step's own cell is the sword stand's.
    let off = [
        pos[0] - i64::from(STAND[0]),
        pos[1] - i64::from(STAND[1]),
        pos[2] - i64::from(STAND[2]),
    ];
    let world = |c: [i32; 3]| -> Vec<i64> { (0..3).map(|i| i64::from(c[i]) + off[i]).collect() };
    for cell in [STAND, beside] {
        assert_ne!(stance, world(cell), "stance in a stand's cell: {step}");
        let above = [cell[0], cell[1] + 1, cell[2]];
        assert_ne!(stance, world(above), "stance on a stand's head: {step}");
    }
    // The proven leg ends at that stance, so the bot replays the leg it walks:
    // the leg the holder's cell names (its `to`, the step's `pos`) has the
    // stance for its last cell, and no leg's route ends inside the stand.
    let wp: serde_json::Value =
        serde_json::from_str(&read(&out, "validation/critical-path-waypoints.json")).unwrap();
    let legs = wp["legs"].as_array().unwrap();
    let leg = legs
        .iter()
        .find(|l| l["to"] == step["pos"])
        .unwrap_or_else(|| panic!("no leg to the holder's cell: {wp}"));
    let last: Vec<i64> =
        serde_json::from_value(leg["waypoints"].as_array().unwrap().last().unwrap().clone())
            .unwrap();
    assert_eq!(last, stance, "the holder's leg ends at the stance: {wp}");
    for l in legs {
        for c in l["waypoints"].as_array().unwrap() {
            let c: Vec<i64> = serde_json::from_value(c.clone()).unwrap();
            assert_ne!(c, world(STAND), "a route crosses the stand's cell: {wp}");
        }
    }
}

/// The functions a line runs in the same tick, read the way the game runs them:
/// a bare `function` call, and an `execute if score … run function` whose every
/// condition is a `#placed` / `#init` score this walk tracks (evaluated) or a
/// `#placeok` chunk count (taken as met: the batch server has the world
/// loaded). A `schedule` runs on a later tick and is not followed; a call under
/// any other condition is not followed.
fn run_inline(
    fdir: &Path,
    ns: &str,
    lines: &[String],
    st: &mut std::collections::BTreeMap<String, i64>,
    ran: &mut Vec<String>,
) {
    let prefix = format!("function {ns}:");
    let run_prefix = format!(" run function {ns}:");
    for l in lines.iter().map(|l| l.trim()) {
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some(rest) = l.strip_prefix("scoreboard players set ") {
            let w: Vec<&str> = rest.split_whitespace().collect();
            if let [holder @ ("#placed" | "#init"), "dw.sys", v] = w[..] {
                st.insert(holder.to_string(), v.parse().unwrap());
            }
            continue;
        }
        let target = if let Some(t) = l.strip_prefix(&prefix) {
            Some(t)
        } else if let Some((head, t)) = l.split_once(&run_prefix) {
            let words: Vec<&str> = head.split_whitespace().collect();
            let mut ok = words.first() == Some(&"execute");
            let mut i = 1;
            while ok && i < words.len() {
                match &words[i..] {
                    ["if", "score", h, "dw.sys", "matches", v, ..] => {
                        ok = match *h {
                            "#placed" | "#init" => st.get(*h).copied() == v.parse().ok(),
                            "#placeok" => true,
                            _ => false,
                        };
                        i += 6;
                    }
                    _ => ok = false,
                }
            }
            ok.then_some(t)
        } else {
            None
        };
        if let Some(t) = target {
            let name = t.split_whitespace().next().unwrap().to_string();
            let p = fdir.join(format!("{name}.mcfunction"));
            if ran.len() > 256 || !p.is_file() {
                continue;
            }
            ran.push(name);
            let body: Vec<String> = std::fs::read_to_string(&p)
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
            run_inline(fdir, ns, &body, st, ran);
        }
    }
}

/// spec-0111 §4.3, the freeze test judged after its own emptying, under the
/// load chain this build emits. A campaign whose world needs no build step
/// (`place_verify` calls `setup_finish` directly — the hello-world fixture, and
/// any campaign whose blockout mass now goes down before its templates) runs
/// its whole load inside one `tick` once `setup` re-arms `#placed`. The
/// generated `holder_frame` runs `setup` and then one `tick` after emptying the
/// frame; that tick must not reach `setup_finish` (which refills the frame), or
/// the freeze line reads a full frame and the template reds on a frame the
/// engine froze correctly.
#[test]
fn a_frame_freeze_is_judged_after_its_own_emptying() {
    let dir = scratch("holders-freeze-prefabs");
    let prefabs = holder_prefabs(&dir.join("prefabs"), vec![frame_entity()]);
    let (code, log, out) = build_holders(
        "holders-freeze",
        &prefabs,
        |c| {
            c["loot"] = serde_json::json!([
                {"id": "loot/map", "anchor": "anchor/frame",
                 "items": [{"item": "minecraft:map", "slot": "contents"}]}
            ]);
        },
        None,
    );
    assert_eq!(code, 0, "{log}");
    let fdir = out.join("datapack/data/hello-world/function");
    // The shape that exposes the defect: no world-build step between the
    // placement and `setup_finish`. Should the fixture gain one, this test
    // binds nothing; it says so here rather than passing.
    let verify = read(
        &out,
        "datapack/data/hello-world/function/place_verify.mcfunction",
    );
    assert!(
        verify
            .lines()
            .last()
            .is_some_and(|l| l.ends_with("run function hello-world:setup_finish")),
        "the fixture has a world-build step, so this test binds nothing: {verify}"
    );
    let test = read(
        &out,
        "packtest-datapack/data/hello-world/test/holder_frame.mcfunction",
    );
    let lines: Vec<String> = test.lines().map(str::to_string).collect();
    let empty = lines
        .iter()
        .position(|l| l.starts_with("item replace entity") && l.ends_with("with minecraft:air"))
        .unwrap_or_else(|| panic!("the template never empties the frame: {test}"));
    let tick = empty
        + lines[empty..]
            .iter()
            .position(|l| l == "function hello-world:tick")
            .unwrap_or_else(|| panic!("no tick after the emptying: {test}"));
    assert!(
        lines[tick..]
            .iter()
            .any(|l| l.contains("Fixed:1b,Invulnerable:1b")),
        "{test}"
    );
    // The world's own load converged before any template runs.
    let mut st =
        std::collections::BTreeMap::from([("#placed".to_string(), 1), ("#init".to_string(), 1)]);
    let mut ran = Vec::new();
    run_inline(&fdir, "hello-world", &lines[..tick], &mut st, &mut ran);
    let mut in_tick = Vec::new();
    run_inline(
        &fdir,
        "hello-world",
        &lines[tick..=tick],
        &mut st,
        &mut in_tick,
    );
    for refill in ["setup_finish", "holder_frame"] {
        assert!(
            !in_tick.iter().any(|n| n == refill),
            "the tick after the emptying runs `{refill}`, refilling the frame before the \
             freeze line reads it: {in_tick:?}\n{test}"
        );
    }
}
