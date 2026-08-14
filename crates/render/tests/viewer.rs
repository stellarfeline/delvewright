//! `delve-render viewer` — the interactive review page.
//!
//! These need neither a GPU nor the (never-committed, EULA-gated) client jar:
//! `--textures` accepts an unpacked resource directory as well as a jar, so the
//! tests build a small one whose blockstates, models and textures are exactly
//! the ones the fixture prefabs name. What the real jar produces is a picture,
//! and a picture is not what CI can judge; what CI can prove is that the page is
//! self-contained, deterministic, reassembles a tiled zone, and is honest about
//! every blockstate it cannot draw as the game draws it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delve-render");

fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-viewer-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A prefab from the content repo, or `None` on a checkout without the symlink.
fn prefab(name: &str) -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../campaigns/prefabs")
        .join(name);
    p.exists().then_some(p)
}

/// One 16×16 opaque PNG, written once and shared by every fake texture. The
/// pixels do not matter to anything under test; that the file decodes does.
fn png_bytes() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([120, 120, 120, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, image::ImageOutputFormat::Png)
        .unwrap();
    out.into_inner()
}

/// Every block id a prefab's palette names.
fn blocks_in(nbt: &Path) -> BTreeSet<String> {
    let st = delvewright_render::nbt::parse_structure(nbt).expect("parse prefab");
    st.palette
        .iter()
        .filter(|s| !delvewright_render::blockcolor::is_air(s))
        .map(|s| {
            let (ns, id) = delvewright_render::blockcolor::base_id(s);
            format!("{ns}:{id}")
        })
        .collect()
}

/// Blocks the fake pack gives a `multipart` definition to, keyed on the four
/// connection flags — the shape vanilla gives a bars/fence/wall block.
///
/// One block is enough and it has to be a real one: the whole point of the
/// under-specification finding is that a definition SELECTS a model with a
/// property the palette leaves out, and a pack whose every definition is a
/// single empty variant key selects nothing with anything.
const MULTIPART_BLOCKS: &[&str] = &["minecraft:iron_bars"];

/// An unpacked resource directory covering `blocks`, minus anything in `omit`.
///
/// Each block gets a one-variant definition pointing at one full-cube model with
/// one texture — except the blocks in [`MULTIPART_BLOCKS`], which get vanilla's
/// own shape: a post plus one arm per connected side. That is a resource pack,
/// as far as everything under test is concerned, and it needs no jar.
fn fake_pack(root: &Path, blocks: &BTreeSet<String>, omit: &[&str]) {
    let states = root.join("assets/minecraft/blockstates");
    let models = root.join("assets/minecraft/models/block");
    let textures = root.join("assets/minecraft/textures/block");
    for d in [&states, &models, &textures] {
        std::fs::create_dir_all(d).unwrap();
    }
    let png = png_bytes();
    for block in blocks {
        let id = block.split_once(':').map(|(_, i)| i).unwrap_or(block);
        if omit.contains(&block.as_str()) {
            continue;
        }
        let definition = if MULTIPART_BLOCKS.contains(&block.as_str()) {
            let arms: Vec<String> = ["north", "east", "south", "west"]
                .iter()
                .map(|side| {
                    format!(
                        r#"{{"when":{{"{side}":"true"}},"apply":{{"model":"minecraft:block/{id}"}}}}"#
                    )
                })
                .collect();
            format!(
                r#"{{"multipart":[{{"apply":{{"model":"minecraft:block/{id}"}}}},{}]}}"#,
                arms.join(",")
            )
        } else {
            format!(r#"{{"variants":{{"":{{"model":"minecraft:block/{id}"}}}}}}"#)
        };
        std::fs::write(states.join(format!("{id}.json")), definition).unwrap();
        let faces = ["down", "up", "north", "south", "west", "east"]
            .map(|f| format!(r##""{f}":{{"texture":"#all"}}"##))
            .join(",");
        std::fs::write(
            models.join(format!("{id}.json")),
            format!(
                r#"{{"textures":{{"all":"minecraft:block/{id}"}},"elements":[{{"from":[0,0,0],"to":[16,16,16],"faces":{{{faces}}}}}]}}"#
            ),
        )
        .unwrap();
        std::fs::write(textures.join(format!("{id}.png")), &png).unwrap();
    }
}

/// Build a pack for one prefab and return its path.
fn pack_for(dir: &Path, nbt: &Path, omit: &[&str]) -> PathBuf {
    let root = dir.join("pack");
    fake_pack(&root, &blocks_in(nbt), omit);
    root
}

fn viewer(nbt: &Path, out: &Path, pack: &Path) -> std::process::Output {
    Command::new(BIN)
        .arg("--textures")
        .arg(pack)
        .arg("viewer")
        .arg(nbt)
        .arg("-o")
        .arg(out)
        .output()
        .unwrap()
}

#[test]
fn a_page_is_self_contained_and_names_its_prefab() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("selfcontained");
    let pack = pack_for(&dir, &nbt, &[]);
    let out = dir.join("page.html");

    let r = viewer(&nbt, &out, &pack);
    assert_eq!(r.status.code(), Some(0), "{r:?}");

    let html = std::fs::read_to_string(&out).unwrap();
    // The Artifact CSP blocks every external host, so a page that FETCHES from
    // one is a page the reviewer opens to nothing. What matters is the fetch,
    // not the letters: the vendored renderer's bundled licence notices name
    // their projects' repositories, and a URL in a comment reaches nobody.
    for probe in [
        "src=\"http",
        "href=\"http",
        "<script src",
        "<link ",
        "@import",
        "url(http",
        "fetch(",
        "XMLHttpRequest",
        "importScripts",
    ] {
        assert!(
            !html.contains(probe),
            "page reaches outside itself via {probe}"
        );
    }
    assert!(html.contains("keep-gate-room"), "page names its prefab");
    assert!(html.contains("delvewright.prefab-viewer/2"));
    // The player point of view is the point of the tool; the constant must
    // reach the page rather than being reimplemented there.
    assert!(html.contains("1.62"), "player eye height reaches the page");
    // And the renderer rides inside it, patched.
    assert!(html.contains("entity/banner_base"));
    assert!(!html.contains("entity/banner/banner_base"));
}

/// ADR-0006 applies to everything the pipeline emits, this page included.
#[test]
fn the_same_prefab_produces_a_byte_identical_page() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("determinism");
    let pack = pack_for(&dir, &nbt, &[]);
    let mut pages = Vec::new();
    for i in 0..2 {
        let out = dir.join(format!("page-{i}.html"));
        let r = viewer(&nbt, &out, &pack);
        assert_eq!(r.status.code(), Some(0), "{r:?}");
        pages.push(std::fs::read(&out).unwrap());
    }
    assert_eq!(pages[0], pages[1], "two runs differ");
    assert!(!pages[0].is_empty());
}

/// A directory of prefabs is one page, and the order cannot depend on how the
/// filesystem happened to hand them back.
#[test]
fn a_directory_of_prefabs_is_one_page_in_a_stable_order() {
    let Some(one) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let src = one.parent().unwrap().to_path_buf();
    let dir = tmp("library");
    let mut all: BTreeSet<String> = BTreeSet::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&src)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "nbt"))
        .collect();
    paths.sort();
    for p in &paths {
        all.extend(blocks_in(p));
    }
    let pack = dir.join("pack");
    fake_pack(&pack, &all, &[]);
    let out = dir.join("library.html");

    let r = Command::new(BIN)
        .arg("--textures")
        .arg(&pack)
        .arg("viewer")
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let html = std::fs::read_to_string(&out).unwrap();

    // Every prefab in the directory is on the page, in the order the tool
    // promises: sorted by PATH, which is not the same as sorted by id
    // (`island-greenfield-bend.nbt` precedes `island-greenfield.nbt`, because
    // `-` sorts before `.`). The property under test is that the order is fixed
    // and derived from the input, not that it reads alphabetically.
    assert!(paths.len() > 1, "fixture library has one prefab");
    let mut last: Option<usize> = None;
    for p in &paths {
        let id = p.file_stem().unwrap().to_string_lossy().to_string();
        let needle = format!("\"id\":\"{id}\"");
        let at = html
            .find(&needle)
            .unwrap_or_else(|| panic!("{id} missing from the page"));
        if let Some(prev) = last {
            assert!(at > prev, "{id} out of order");
        }
        last = Some(at);
    }
}

/// A blockstate the pinned version does not have is a finding, reported with a
/// cell count — never a block silently drawn as if it were fine. This is the
/// general form of the `minecraft:chain` case.
#[test]
fn an_unresolvable_blockstate_is_dw0780_with_its_cell_count() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("unresolved");
    let pack = pack_for(&dir, &nbt, &["minecraft:glowstone"]);
    let out = dir.join("page.html");

    let r = viewer(&nbt, &out, &pack);
    // Unresolved blocks are a warning, not a refusal: the reviewer still needs
    // to see the rest of the building.
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0780"), "expected DW0780: {stderr}");
    assert!(
        stderr.contains("minecraft:glowstone"),
        "the finding names the block: {stderr}"
    );
    assert!(
        stderr.contains("6 cell(s)"),
        "with its cell count: {stderr}"
    );
    // And the page itself carries it, so the owner sees it without a terminal.
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("minecraft:glowstone"));
}

/// The finding the whole rewrite exists for.
///
/// A palette entry that leaves properties unwritten is legal, and a running
/// server places the right block — but the shape then comes from the version's
/// default state rather than from the file, and the previous page drew a solid
/// cube where a wall post stands while reporting nothing at all. The page may
/// not report a clean resolution over such a palette.
#[test]
fn an_under_specified_state_is_dw0781_and_names_what_gets_filled_in() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("underspecified");
    let pack = pack_for(&dir, &nbt, &[]);
    let out = dir.join("page.html");

    let r = viewer(&nbt, &out, &pack);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0781"), "expected DW0781: {stderr}");
    assert!(
        stderr.contains("minecraft:iron_bars"),
        "the finding names the block: {stderr}"
    );
    // The properties, and what the pinned version fills them with — a message
    // that only said "incomplete" would leave the author guessing.
    for probe in ["north", "north=false", "multipart"] {
        assert!(stderr.contains(probe), "missing {probe}: {stderr}");
    }
    // The summary line cannot read as clean while this is outstanding.
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(
        stdout.contains("1 under-specified"),
        "the summary states it: {stdout}"
    );
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("under_specified"));
}

/// Binding counts, on the summary line and in the payload. A page that examined
/// nothing must not read like a page that examined everything and found nothing.
#[test]
fn the_run_states_what_each_check_examined() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("binding");
    let pack = pack_for(&dir, &nbt, &[]);
    let out = dir.join("page.html");
    let r = Command::new(BIN)
        .arg("--json")
        .arg("--textures")
        .arg(&pack)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let summary: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&r.stdout).trim()).unwrap();
    assert_eq!(summary["states"], 5, "keep-gate-room has 5 blockstates");
    assert!(
        summary["textures"].as_u64().unwrap() >= 3,
        "the fake pack gives one texture per block: {summary}"
    );
    assert!(summary["anchors"].as_u64().unwrap() > 0);
    // The page states the same numbers where the reviewer reads them.
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("special_bound"));
}

/// A prefab with no metadata sidecar has no anchors. It must still produce a
/// page — and say that the binding was zero rather than pass quietly.
#[test]
fn a_prefab_with_no_anchors_still_renders_and_reports_zero_binding() {
    let Some(src) = prefab("keep-alcove.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("no-anchors");
    // Copy the `.nbt` WITHOUT its `.json`, which is how a prefab that declares
    // nothing presents.
    let bare = dir.join("bare.nbt");
    std::fs::copy(&src, &bare).unwrap();
    let pack = pack_for(&dir, &bare, &[]);
    let out = dir.join("page.html");

    let r = viewer(&bare, &out, &pack);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("DW0726"),
        "a zero binding is a finding, not silence: {stderr}"
    );
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("\"anchors\":[]"), "empty anchor list");
}

#[test]
fn a_missing_prefab_is_dw0721_exit2() {
    let dir = tmp("missing");
    let pack = dir.join("pack");
    fake_pack(&pack, &BTreeSet::new(), &[]);
    let r = Command::new(BIN)
        .arg("--textures")
        .arg(&pack)
        .arg("viewer")
        .arg(dir.join("nope.nbt"))
        .arg("-o")
        .arg(dir.join("page.html"))
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0721"), "expected DW0721: {stderr}");
}

/// The page must stay far below the 16 MB Artifact ceiling on real geometry —
/// that is what the run-length packing buys, and a regression in it would show
/// up here long before a reviewer hit a page that would not load.
#[test]
fn a_real_prefab_page_is_small() {
    let Some(nbt) = prefab("island-mountain.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("size");
    let pack = pack_for(&dir, &nbt, &[]);
    let out = dir.join("page.html");
    let r = viewer(&nbt, &out, &pack);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let bytes = std::fs::metadata(&out).unwrap().len();
    // 36x28x42 = 42,336 cells. A JSON cube per cell would be megabytes. The
    // vendored renderer is ~290 KB of the total on its own, so the ceiling here
    // is the geometry's, not the page's.
    assert!(
        bytes < 2_000_000,
        "island-mountain page is {bytes} bytes; the packing has regressed"
    );
}

/* ------------------------------------------------------------- tiled zones -- */

/// The cross-feature interaction neither branch could test on its own.
///
/// A zone past the 48-per-axis structure-template cap ships as several `.nbt`
/// files and one manifest. `piece` and `batch` each learned to reassemble it and
/// to refuse a lone tile; this page is the third door into the same defect and
/// the one nobody would point at deliberately — a directory walk over `*.nbt`
/// would put every tile on the page as if it were a prefab, and a review of a
/// building sliced at a packaging boundary passes and means nothing.
#[test]
fn a_tiled_zone_is_one_building_on_the_page() {
    use delvewright_schem::convert::{self, DATA_VERSION};
    use delvewright_schem::schematic::{BlockState, ParsedSchematic};
    use delvewright_schem::split::{TilePart, TileSet};

    let dir = tmp("tiled");
    let zone = dir.join("zone");
    std::fs::create_dir_all(&zone).unwrap();

    let size = [6, 3, 60];
    let parts = vec![
        TilePart {
            file: "zone.x0y0z0.nbt".to_string(),
            id: "zone.x0y0z0".to_string(),
            grid_index: [0, 0, 0],
            offset: [0, 0, 0],
            size: [6, 3, 48],
        },
        TilePart {
            file: "zone.x0y0z1.nbt".to_string(),
            id: "zone.x0y0z1".to_string(),
            grid_index: [0, 0, 1],
            offset: [0, 0, 48],
            size: [6, 3, 12],
        },
    ];
    for part in &parts {
        let cells = (part.size[0] * part.size[1] * part.size[2]) as usize;
        let schem = ParsedSchematic {
            version: 3,
            source_data_version: Some(DATA_VERSION),
            size: part.size,
            offset: [0, 0, 0],
            palette: vec![BlockState {
                name: "minecraft:stone".to_string(),
                properties: std::collections::BTreeMap::new(),
            }],
            blocks: vec![0; cells],
            block_entities: Vec::new(),
        };
        let mut diagnostics = Vec::new();
        let nbt = convert::build_region(&schem, [0, 0, 0], part.size, &mut diagnostics);
        assert!(diagnostics.is_empty());
        std::fs::write(zone.join(&part.file), nbt).unwrap();
    }
    let set = TileSet {
        base: "zone".to_string(),
        size,
        part_max: 48,
        grid: [1, 1, 2],
        data_version: DATA_VERSION,
        generator: "crates/grammar".to_string(),
        parts,
    };
    std::fs::write(
        zone.join("zone.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "prefab_id": "prefab/zone",
            "structure_set": set,
            "anchors": {},
            "lighting": { "profile": "unmeasured" },
        }))
        .unwrap(),
    )
    .unwrap();

    let mut blocks = BTreeSet::new();
    blocks.insert("minecraft:stone".to_string());
    let pack = dir.join("pack");
    fake_pack(&pack, &blocks, &[]);

    // Pointed at the DIRECTORY, which is the door: the tiles are dropped in
    // favour of their manifest, and the page holds one building of the zone's
    // full size rather than two of the tiles'.
    let out = dir.join("zone.html");
    let r = Command::new(BIN)
        .arg("--json")
        .arg("--textures")
        .arg(&pack)
        .arg("viewer")
        .arg(&zone)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let summary: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&r.stdout).trim()).unwrap();
    assert_eq!(
        summary["prefabs"], 1,
        "the zone is ONE piece, not two tiles"
    );

    let html = std::fs::read_to_string(&out).unwrap();
    assert!(
        html.contains(r#""size":[6,3,60]"#),
        "the page holds the whole zone, not a tile"
    );
    assert!(html.contains(r#""tiles":2"#), "and says what it came from");
    assert!(
        !html.contains(r#""id":"zone.x0y0z0""#),
        "a tile must not appear as a prefab of its own"
    );
    // Every cell of the zone survives reassembly, cut plane included: 6*3*60.
    assert!(
        html.contains(r#""filled":1080"#),
        "the far tile's cells are on the page"
    );

    // And pointing at one tile by name is refused rather than rendered.
    let r = Command::new(BIN)
        .arg("--textures")
        .arg(&pack)
        .arg("viewer")
        .arg(zone.join("zone.x0y0z0.nbt"))
        .arg("-o")
        .arg(dir.join("fragment.html"))
        .output()
        .unwrap();
    assert_eq!(
        r.status.code(),
        Some(2),
        "a lone tile must be refused: {r:?}"
    );
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(
        stderr.contains("zone.json"),
        "and named the manifest: {stderr}"
    );
}

/// The block-entity texture table is a second copy of a table that lives,
/// private, inside the renderer — and a wrong entry in it is invisible, because
/// a texture that does not exist and a texture nobody asked for look the same in
/// a finished picture. So the ids are resolved against the asset source at build
/// time, and what an absence MEANS is decided by the source rather than chosen:
/// a jar that declares itself to be the pinned game is complete by definition,
/// so a texture it does not have means this table and that version disagree.
///
/// Both verdicts, over the same prefab and the same missing texture.
#[test]
fn a_block_entity_texture_the_pinned_version_lacks_is_dw0782() {
    let Some(nbt) = prefab("hero-galleon-oak.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("special");
    let pack = pack_for(&dir, &nbt, &[]);

    // A resource pack is entitled to be partial: the same absence is an ordinary
    // unresolved-resource warning and the page is still written.
    let out = dir.join("pack.html");
    let r = viewer(&nbt, &out, &pack);
    assert_eq!(
        r.status.code(),
        Some(0),
        "a partial pack is not a refusal: {r:?}"
    );
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0780"), "expected DW0780: {stderr}");
    assert!(
        stderr.contains("entity/chest/normal"),
        "the finding names the texture: {stderr}"
    );
    assert!(out.exists());

    // The SAME directory, now declaring itself to be the pinned game. Nothing
    // else changes, and the verdict does.
    std::fs::write(
        pack.join("version.json"),
        br#"{"id":"1.21.11","name":"1.21.11"}"#,
    )
    .unwrap();
    let out = dir.join("jar.html");
    let r = viewer(&nbt, &out, &pack);
    assert_eq!(r.status.code(), Some(10), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0782"), "expected DW0782: {stderr}");
    assert!(
        stderr.contains("entity/chest/normal"),
        "the refusal names the id: {stderr}"
    );
    assert!(
        !out.exists(),
        "no page is written from resources that disagree"
    );
}

/* ----------------------------------------------------------------- controls -- */

/// The mapping the emitted page actually ships, checked at the seam Rust owns.
///
/// What each key DOES is proved by `tests/controls.test.mjs`, which executes the
/// module; what this proves is that the module reaches the page at all, ahead of
/// the code that calls it, and that no second copy of the mapping came with it.
/// Both halves are needed: an arithmetic proof of a file the page never loads is
/// the unemitted kind of vacuous.
#[test]
fn the_page_carries_the_shared_control_table_and_only_that_one() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("controls");
    let pack = pack_for(&dir, &nbt, &[]);
    let out = dir.join("page.html");

    let r = viewer(&nbt, &out, &pack);
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let html = std::fs::read_to_string(&out).unwrap();

    let table = html
        .find("globalThis.DelveControls")
        .expect("control table absent");
    let user = html
        .find("C.walkStep(")
        .expect("page never calls the shared walk");
    assert!(
        table < user,
        "the page calls the control table before defining it"
    );

    // The physical keys, in the page, as codes — not as characters. `.key` is
    // what a Chinese IME rewrites to "Process", which is how a WASD matched on
    // characters goes dead for exactly this project's reader.
    for code in [
        "KeyW",
        "KeyA",
        "KeyS",
        "KeyD",
        "Space",
        "ShiftLeft",
        "ArrowLeft",
    ] {
        assert!(
            html.contains(code),
            "physical key {code} missing from the page"
        );
    }
    assert!(
        !html.contains("\"wasdc \".indexOf"),
        "the hand-rolled key string is back in the page"
    );
    // The reset that keeps an `[hidden]` overlay from swallowing every gesture.
    assert!(
        html.contains("[hidden] { display: none !important; }"),
        "the [hidden] reset is gone; an overlay would eat the reviewer's pointer"
    );
}

/// A gate nothing invokes is not a gate (CLAUDE.md). The control tests are
/// JavaScript, so `cargo test` cannot reach them — which is exactly the shape
/// that lets a check rot into a line in a document. This binds them to a job
/// that already has to pass, and fails if the invocation is ever removed.
#[test]
fn ci_runs_the_control_mapping_tests() {
    let ci = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/ci.yml");
    let Ok(text) = std::fs::read_to_string(&ci) else {
        panic!("cannot read {}", ci.display());
    };
    assert!(
        text.contains("node --test crates/render/tests/controls.test.mjs"),
        "ci.yml no longer runs the viewer's control mapping tests"
    );
    assert!(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/controls.test.mjs")
            .exists(),
        "the control mapping tests are gone but CI still claims to run them"
    );
}
