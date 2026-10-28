//! `DW1043` (spec-0113 §5.4): a movement the reach graph does not fly is a
//! warning, never a refusal. A kit with a bottle of leaping builds, the
//! warning names the potion, the kit and `jump_boost`, the leave proof's
//! configurations from the first step are unjudged and the stranding proof
//! says so; the same campaign without it reads zero unbounded.

mod common;

fn build(dir: &std::path::Path, out: &std::path::Path) -> (bool, String) {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(dir)
        .arg("-o")
        .arg(out)
        .arg("--prefabs")
        .arg(common::prefabs_dir())
        .output()
        .expect("delvec runs");
    // The binding lines go to stderr, the grouped diagnostics to stdout: one
    // transcript of both, as a creator's terminal shows them.
    let mut all = String::from_utf8_lossy(&o.stderr).into_owned();
    all.push_str(&String::from_utf8_lossy(&o.stdout));
    (o.status.success(), all)
}

#[test]
fn a_bottle_of_leaping_builds_and_is_named() {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("movement-items");
    let _ = std::fs::remove_dir_all(&root);
    let plain = root.join("plain");
    let bottle = root.join("bottle");
    common::copy_dir_all(&common::hello_world_dir(), &plain);
    common::copy_dir_all(&common::hello_world_dir(), &bottle);
    common::patch_file(&bottle.join("classes.json"), |v| {
        v["content"]["classes"][0]["kit"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "item": "minecraft:potion", "count": 1,
                "contents": {"potion": "minecraft:leaping"}
            }));
    });

    let (ok, err) = build(&plain, &root.join("plain-out"));
    assert!(ok, "{err}");
    assert!(
        err.contains("0 a movement the reach graph does not fly")
            && err.contains("judged, 0 unbounded"),
        "the plain campaign reads zero: {err}"
    );
    assert!(!err.contains("DW1043 [warning]"), "{err}");

    let (ok, err) = build(&bottle, &root.join("bottle-out"));
    assert!(
        ok,
        "a movement the graph does not fly is not refused: {err}"
    );
    let warning = err
        .lines()
        .find(|l| l.starts_with("DW1043 [warning]"))
        .unwrap_or_else(|| panic!("no DW1043 warning: {err}"));
    for named in [
        "minecraft:leaping",
        "class `class/wanderer`'s kit",
        "jump_boost",
    ] {
        assert!(
            warning.contains(named),
            "the warning names {named}: {warning}"
        );
    }
    let line = err
        .lines()
        .find(|l| l.starts_with("movement items:"))
        .expect("the movement items line");
    assert!(
        line.contains("1 a movement the reach graph does not fly"),
        "{line}"
    );
    assert!(line.contains("0 quest configuration(s) judged"), "{line}");
    assert!(err.contains("DW0322 unjudged"), "{err}");
}
