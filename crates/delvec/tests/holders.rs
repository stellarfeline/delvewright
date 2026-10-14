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
        frame.contains("Facing:5b,Fixed:0b,Invulnerable:0b,ItemDropChance:1.0f"),
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
    assert_eq!(cp["format_version"], 5);
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
