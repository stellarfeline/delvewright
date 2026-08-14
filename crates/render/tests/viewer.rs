//! `delve-render viewer` — the interactive review page.
//!
//! These need neither a GPU nor the (never-committed, EULA-gated) client jar:
//! the page is built from a supplied appearance table, which is exactly the
//! `--palette` seam that lets a page be built on a machine with no jar. What
//! the jar path produces is covered by `tests/gpu.rs`; what CI can prove without
//! it is that the page is self-contained, deterministic, and honest about a
//! blockstate it could not resolve.

use std::path::PathBuf;
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

/// An appearance table covering the committed keep prefabs, plus one entry the
/// table deliberately does not have, so the unresolved path is exercised.
fn palette_fixture() -> &'static str {
    r#"{
      "version": 1,
      "biome": "minecraft:plains",
      "entries": {
        "minecraft:air": {"rgb":[0,0,0],"coverage":0,"box":[0,0,0,16,16,16]},
        "minecraft:stone_bricks": {"rgb":[122,122,122],"coverage":255,"box":[0,0,0,16,16,16]},
        "minecraft:cobblestone": {"rgb":[127,127,127],"coverage":255,"box":[0,0,0,16,16,16]},
        "minecraft:stone_brick_slab[type=bottom,waterlogged=false]":
          {"rgb":[122,122,122],"coverage":255,"box":[0,0,0,16,8,16]},
        "minecraft:glass": {"rgb":[200,220,235],"coverage":60,"box":[0,0,0,16,16,16]}
      },
      "unresolved": {
        "minecraft:chain[axis=y,waterlogged=false]": "no_blockstate"
      }
    }"#
}

fn write_palette(dir: &std::path::Path) -> PathBuf {
    let p = dir.join("palette.json");
    std::fs::write(&p, palette_fixture()).unwrap();
    p
}

#[test]
fn a_page_is_self_contained_and_names_its_prefab() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("selfcontained");
    let pal = write_palette(&dir);
    let out = dir.join("page.html");

    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");

    let html = std::fs::read_to_string(&out).unwrap();
    // The Artifact CSP blocks every external host, so a page that reaches for
    // one is a page the reviewer opens to nothing.
    for probe in ["http://", "https://", "<script src", "<link ", "@import"] {
        assert!(!html.contains(probe), "page references {probe}");
    }
    assert!(html.contains("keep-gate-room"), "page names its prefab");
    assert!(html.contains("delvewright.prefab-viewer/1"));
    // The player point of view is the point of the tool; the constant must
    // reach the page rather than being reimplemented there.
    assert!(html.contains("1.62"), "player eye height reaches the page");
}

/// ADR-0006 applies to everything the pipeline emits, this page included.
#[test]
fn the_same_prefab_produces_a_byte_identical_page() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("determinism");
    let pal = write_palette(&dir);
    let mut pages = Vec::new();
    for i in 0..2 {
        let out = dir.join(format!("page-{i}.html"));
        let r = Command::new(BIN)
            .arg("viewer")
            .arg(&nbt)
            .arg("-o")
            .arg(&out)
            .arg("--palette")
            .arg(&pal)
            .output()
            .unwrap();
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
    let pal = write_palette(&dir);
    let out = dir.join("library.html");

    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let html = std::fs::read_to_string(&out).unwrap();

    // Every prefab in the directory is on the page, in the order the tool
    // promises: sorted by PATH, which is not the same as sorted by id
    // (`island-greenfield-bend.nbt` precedes `island-greenfield.nbt`, because
    // `-` sorts before `.`). The property under test is that the order is fixed
    // and derived from the input, not that it reads alphabetically.
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&src)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "nbt"))
        .collect();
    paths.sort();
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

/// A blockstate the table cannot resolve is a finding, reported with a count —
/// never a block silently drawn as if it were fine. This is the general form of
/// the `minecraft:chain` case: the id does not exist at the pinned version.
#[test]
fn an_unresolvable_blockstate_is_dw0727_with_its_cell_count() {
    let Some(nbt) = prefab("hero-temple-ruin-arch.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("unresolved");
    let pal = write_palette(&dir);
    let out = dir.join("page.html");

    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
    // Unresolved blocks are a warning, not a refusal: the reviewer still needs
    // to see the rest of the building.
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0727"), "expected DW0727: {stderr}");
    assert!(
        stderr.contains("minecraft:chain"),
        "the finding names the block: {stderr}"
    );
    // And the page itself carries it, so the owner sees it without a terminal.
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("minecraft:chain"));
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
    let pal = write_palette(&dir);
    // Copy the `.nbt` WITHOUT its `.json`, which is how a prefab that declares
    // nothing presents.
    let bare = dir.join("bare.nbt");
    std::fs::copy(&src, &bare).unwrap();
    let out = dir.join("page.html");

    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&bare)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
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
    let pal = write_palette(&dir);
    let r = Command::new(BIN)
        .arg("viewer")
        .arg(dir.join("nope.nbt"))
        .arg("-o")
        .arg(dir.join("page.html"))
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2), "{r:?}");
    let stderr = String::from_utf8_lossy(&r.stderr);
    assert!(stderr.contains("DW0721"), "expected DW0721: {stderr}");
}

#[test]
fn a_malformed_palette_is_refused_rather_than_guessed_at() {
    let Some(nbt) = prefab("keep-gate-room.nbt") else {
        eprintln!("skip: no content symlink");
        return;
    };
    let dir = tmp("bad-palette");
    let pal = dir.join("palette.json");
    std::fs::write(&pal, b"{ this is not json").unwrap();
    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(dir.join("page.html"))
        .arg("--palette")
        .arg(&pal)
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
    let pal = write_palette(&dir);
    let out = dir.join("page.html");
    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0), "{r:?}");
    let bytes = std::fs::metadata(&out).unwrap().len();
    // 36x28x42 = 42,336 cells. A JSON cube per cell would be megabytes.
    assert!(
        bytes < 1_000_000,
        "island-mountain page is {bytes} bytes; the packing has regressed"
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
    let pal = write_palette(&dir);
    let out = dir.join("page.html");

    let r = Command::new(BIN)
        .arg("viewer")
        .arg(&nbt)
        .arg("-o")
        .arg(&out)
        .arg("--palette")
        .arg(&pal)
        .output()
        .unwrap();
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
