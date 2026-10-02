//! **A delve wears its own textures** (spec-0084): a `world.textures[]` row
//! reaches the resource pack at the vanilla path, every refusal fires on the
//! shape it names, and the build is byte-identical.
//!
//! Every campaign here is the hello-world fixture plus one row, run through the
//! binary, so what is asserted is what a creator gets.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use delvewright_dsl::DSL_VERSION;

mod common;

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("textures-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Proc::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("run delvec")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// A PNG of one colour, `w`×`h`, through the engine's own encoder.
fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
    delvec::compiler::png::encode_rgba(w, h, &px)
}

/// hello-world plus `rows` as `world.textures[]`, with `files` written into
/// `textures/`.
fn campaign(tag: &str, rows: serde_json::Value, files: &[(&str, Vec<u8>)]) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let path = camp.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!(DSL_VERSION);
    world["content"]["textures"] = rows;
    std::fs::write(&path, serde_json::to_string_pretty(&world).unwrap()).unwrap();
    if !files.is_empty() {
        std::fs::create_dir_all(camp.join("textures")).unwrap();
        for (name, bytes) in files {
            std::fs::write(camp.join("textures").join(name), bytes).unwrap();
        }
    }
    camp
}

fn moon_row(id: &str) -> serde_json::Value {
    serde_json::json!([{
        "id": id,
        "replaces": "minecraft:environment/celestial/moon/full_moon",
        "license": { "spdx": "original", "source": "original" }
    }])
}

fn validate(camp: &Path) -> (i32, String) {
    let prefabs = common::prefabs_dir();
    let r = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r))
}

fn build(camp: &Path, out: &Path) -> (i32, String) {
    let prefabs = common::prefabs_dir();
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r))
}

fn zip_names(bytes: &[u8]) -> Vec<String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().to_string())
        .collect()
}

/// spec-0084 §10 criterion 3: one row and nothing else in the pack — the pack is
/// `pack.mcmeta` and the row's vanilla path, two builds are byte-identical, and
/// the manifest records the pack, the statement and the image as an input.
#[test]
fn one_row_ships_a_pack_of_one_texture_byte_identically() {
    let red = png(32, 32, [200, 30, 30, 255]);
    let camp = campaign(
        "one",
        moon_row("red-moon"),
        &[("red-moon.png", red.clone())],
    );
    let (a, b) = (tmp("one-a"), tmp("one-b"));
    let (code, said) = build(&camp, &a);
    assert_eq!(code, 0, "{said}");
    let (code, said) = build(&camp, &b);
    assert_eq!(code, 0, "{said}");

    let zip = std::fs::read(a.join("resourcepack.zip")).unwrap();
    assert_eq!(
        zip,
        std::fs::read(b.join("resourcepack.zip")).unwrap(),
        "two builds of one campaign ship one pack (ADR-0006)"
    );
    assert_eq!(
        zip_names(&zip),
        vec![
            "assets/minecraft/textures/environment/celestial/moon/full_moon.png".to_string(),
            "pack.mcmeta".to_string()
        ]
    );
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(&zip)).unwrap();
    let mut shipped = Vec::new();
    std::io::Read::read_to_end(
        &mut z
            .by_name("assets/minecraft/textures/environment/celestial/moon/full_moon.png")
            .unwrap(),
        &mut shipped,
    )
    .unwrap();
    assert_eq!(
        shipped, red,
        "the creator's bytes, copied, never re-encoded"
    );
    let mut mcmeta = String::new();
    std::io::Read::read_to_string(&mut z.by_name("pack.mcmeta").unwrap(), &mut mcmeta).unwrap();
    let meta: serde_json::Value = serde_json::from_str(&mcmeta).unwrap();
    assert_eq!(meta["pack"]["min_format"], serde_json::json!([75, 0]));
    assert_eq!(meta["pack"]["max_format"], serde_json::json!([75, 0]));
    assert!(meta["pack"].get("pack_format").is_none(), "{mcmeta}");

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(a.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["resource_pack_sha1"],
        serde_json::json!(delvec::compiler::resourcepack::sha1_hex(&zip))
    );
    assert_eq!(
        manifest["resource_pack_overrides_vanilla"],
        serde_json::json!(true)
    );
    assert!(
        manifest["inputs"].get("textures/red-moon.png").is_some(),
        "{}",
        manifest["inputs"]
    );
    assert_eq!(
        std::fs::read(a.join("manifest.json")).unwrap(),
        std::fs::read(b.join("manifest.json")).unwrap()
    );
    let note = std::fs::read_to_string(a.join("SKINS.md")).unwrap();
    assert!(note.contains("## Textures"), "{note}");
    assert!(
        note.contains(
            "`red-moon` → `assets/minecraft/textures/environment/celestial/moon/full_moon.png` (1×)"
        ),
        "{note}"
    );
    assert!(note.contains("SERVED"), "{note}");
}

/// A changed image is a changed pack and a changed manifest: the binding the
/// gallery's coverage perturbation relies on, here over a fixture.
#[test]
fn a_changed_pixel_moves_the_pack_and_the_manifest() {
    let a_camp = campaign(
        "px-a",
        moon_row("m"),
        &[("m.png", png(32, 32, [1, 2, 3, 255]))],
    );
    let b_camp = campaign(
        "px-b",
        moon_row("m"),
        &[("m.png", png(32, 32, [1, 2, 4, 255]))],
    );
    let (a, b) = (tmp("px-out-a"), tmp("px-out-b"));
    assert_eq!(build(&a_camp, &a).0, 0);
    assert_eq!(build(&b_camp, &b).0, 0);
    assert_ne!(
        std::fs::read(a.join("resourcepack.zip")).unwrap(),
        std::fs::read(b.join("resourcepack.zip")).unwrap()
    );
}

/// A pack that carries only the engine's own namespaces states that it
/// overrides nothing — the statement is read off the archive paths, both ways.
#[test]
fn a_pack_without_a_vanilla_path_states_it_overrides_nothing() {
    let names = [
        "assets/delvewright/lang/en_us.json".to_string(),
        "assets/delve/font/art.json".to_string(),
    ];
    assert!(!delvec::compiler::textures::overrides_vanilla(names.iter()));
    let with = [
        "assets/delvewright/lang/en_us.json".to_string(),
        "assets/minecraft/textures/block/stone.png".to_string(),
    ];
    assert!(delvec::compiler::textures::overrides_vanilla(with.iter()));
}

/// spec-0084 §11 ruling 1: declared, the pack is required in the shipped file;
/// undeclared, the file is what it always was.
#[test]
fn a_required_pack_is_written_into_server_properties_only_when_declared() {
    let camp = campaign(
        "req",
        moon_row("m"),
        &[("m.png", png(32, 32, [9, 9, 9, 255]))],
    );
    let out = tmp("req-out");
    assert_eq!(build(&camp, &out).0, 0);
    let plain = std::fs::read_to_string(out.join("server/server.properties")).unwrap();
    assert!(!plain.contains("require-resource-pack"), "{plain}");

    common::patch_file(&camp.join("world.json"), |w| {
        w["content"]["require_resource_pack"] = serde_json::json!(true);
    });
    let out2 = tmp("req-out-2");
    assert_eq!(build(&camp, &out2).0, 0);
    let props = std::fs::read_to_string(out2.join("server/server.properties")).unwrap();
    assert!(props.contains("require-resource-pack=true\n"), "{props}");
}

/// One refusal case: tag, rows, files, the code, and a phrase the message carries.
type RefusalCase = (
    &'static str,
    serde_json::Value,
    Vec<(&'static str, Vec<u8>)>,
    &'static str,
    &'static str,
);

/// Every refusal of spec-0084 §6, each on the shape it names, at `validate`.
#[test]
fn every_refusal_fires_on_the_shape_it_names() {
    let moon = |id: &str| moon_row(id);
    let cases: Vec<RefusalCase> = vec![
        (
            "pre-pin-path",
            serde_json::json!([{ "id": "m", "replaces": "minecraft:environment/moon_phases",
                "license": { "spdx": "original", "source": "original" } }]),
            vec![("m.png", png(32, 32, [1, 1, 1, 255]))],
            "DW0939",
            "environment/celestial",
        ),
        (
            "textures-prefix",
            serde_json::json!([{ "id": "m", "replaces": "minecraft:textures/block/stone.png",
                "license": { "spdx": "original", "source": "original" } }]),
            vec![("m.png", png(16, 16, [1, 1, 1, 255]))],
            "DW0939",
            "without `textures/`",
        ),
        (
            "two-rows-one-texture",
            serde_json::json!([
                { "id": "a", "replaces": "minecraft:block/stone",
                  "license": { "spdx": "original", "source": "original" } },
                { "id": "b", "replaces": "minecraft:block/stone",
                  "license": { "spdx": "original", "source": "original" } }
            ]),
            vec![
                ("a.png", png(16, 16, [1, 1, 1, 255])),
                ("b.png", png(16, 16, [2, 2, 2, 255])),
            ],
            "DW0939",
            "already",
        ),
        (
            "another-shape",
            moon("m"),
            vec![("m.png", png(48, 48, [1, 1, 1, 255]))],
            "DW0940",
            "whole",
        ),
        (
            "not-a-png",
            moon("m"),
            vec![("m.png", b"GIF89a this is not a png".to_vec())],
            "DW0940",
            "not a PNG",
        ),
        (
            "bad-sidecar",
            moon("m"),
            vec![
                ("m.png", png(32, 64, [1, 1, 1, 255])),
                ("m.png.mcmeta", br#"{"animation":{"frames":[5]}}"#.to_vec()),
            ],
            "DW0940",
            "frame 5",
        ),
        ("no-file", moon("m"), vec![], "DW0309", "textures/m.png"),
        (
            "bad-id",
            moon("Red Moon"),
            vec![("Red Moon.png", png(32, 32, [1, 1, 1, 255]))],
            "DW0190",
            "kebab",
        ),
        (
            "non-commercial",
            serde_json::json!([{ "id": "m", "replaces": "minecraft:environment/celestial/moon/full_moon",
                "license": { "spdx": "CC-BY-NC-4.0", "source": "a site", "url": "https://example.org/l" } }]),
            vec![("m.png", png(32, 32, [1, 1, 1, 255]))],
            "DW0741",
            "NonCommercial",
        ),
        (
            "cc-by-without-credit",
            serde_json::json!([{ "id": "m", "replaces": "minecraft:environment/celestial/moon/full_moon",
                "license": { "spdx": "CC-BY-4.0", "source": "a site", "url": "https://example.org/l" } }]),
            vec![("m.png", png(32, 32, [1, 1, 1, 255]))],
            "DW0741",
            "attribution",
        ),
    ];
    for (tag, rows, files, code, says) in cases {
        let camp = campaign(tag, rows, &files);
        let (rc, said) = validate(&camp);
        assert_eq!(rc, 1, "{tag}: {said}");
        assert!(
            said.contains(code),
            "{tag} should be refused {code}: {said}"
        );
        assert!(said.contains(says), "{tag} should say `{says}`: {said}");
    }
}

/// An animated override is a strip of whole frames with vanilla's metadata, and
/// it ships beside the PNG at the vanilla sidecar path.
#[test]
fn an_animation_ships_its_sidecar_at_the_vanilla_path() {
    let camp = campaign(
        "anim",
        moon_row("m"),
        &[
            ("m.png", png(32, 96, [9, 9, 200, 255])),
            ("m.png.mcmeta", br#"{"animation":{"frametime":8}}"#.to_vec()),
        ],
    );
    let out = tmp("anim-out");
    let (rc, said) = build(&camp, &out);
    assert_eq!(rc, 0, "{said}");
    let names = zip_names(&std::fs::read(out.join("resourcepack.zip")).unwrap());
    assert!(
        names.contains(
            &"assets/minecraft/textures/environment/celestial/moon/full_moon.png.mcmeta"
                .to_string()
        ),
        "{names:?}"
    );
    let note = std::fs::read_to_string(out.join("SKINS.md")).unwrap();
    assert!(note.contains("(1×, 3 frame(s))"), "{note}");
}

/// The comparison sheet is a pure function of its two images (spec-0084 §5.2),
/// through `delvec textures`, when the pinned client jar is present.
#[test]
fn the_sheet_verb_writes_one_deterministic_sheet_per_row() {
    let Ok(jar) = delvec::compiler::view::cli::resolve_textures(None) else {
        eprintln!(
            "no client jar on this machine — the jar half of the sheet is not exercised here"
        );
        return;
    };
    let camp = campaign(
        "sheet",
        moon_row("m"),
        &[("m.png", png(32, 32, [200, 0, 0, 255]))],
    );
    let (a, b) = (tmp("sheet-a"), tmp("sheet-b"));
    for out in [&a, &b] {
        let r = delvec(&[
            "textures",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--textures",
            &jar,
        ]);
        assert!(r.status.success(), "{}", log(&r));
    }
    let sa = std::fs::read(a.join("m.png")).unwrap();
    assert_eq!(sa, std::fs::read(b.join("m.png")).unwrap());
    let img = image::load_from_memory(&sa).unwrap();
    assert_eq!((img.width(), img.height()), (513, 256));
}
