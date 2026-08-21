//! Process-level CLI tests: exit-code + diagnostic matrix, and the ADR-0006
//! double-build byte-identity determinism gate.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn version_line() {
    let out = delvec(&["--version"]);
    assert_eq!(code(&out), 0);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains(&format!("delvec {}", env!("CARGO_PKG_VERSION"))),
        "{s}"
    );
    // spec-0031 raised the implemented DSL to 0.10.0, in three additive parts:
    // runtime state — the stage-5 `state[]` declaration, the
    // `set-state`/`add-state`/`clear-state` verbs and the `requires_state`
    // numeric comparison carried by every gate consumer — the campaign-wide
    // `on_death` bundle, effect root R7, and the stage-5 `lethal_volumes`
    // declaration. 0.11.0 carries two surfaces and one obligation: spec-0034's
    // per-body `traversal` declaration — what a body can do when it moves —
    // carried by the stage-2 NPC and the stage-5 actor through one shared type;
    // and the press-answer lift, a `narrate` `actionbar` style (the reply strip
    // every compiler-written line already used) plus a trigger's `audience:
    // presser` (dispatch as the player who right-clicked), which together make
    // "a pressable thing answers the presser" an ordinary trigger and retire
    // `close-gate`'s private copy of it. The obligation is `DW0429`: at 0.11.0 a
    // sealed body nothing answers is an error.
    // 0.12.0 (spec-0042) adds one verb: `open-way`, a campaign opening a placed
    // piece's contingent way — the broken flight a beat repairs, the bridge a
    // beat lowers. It carries no region, no block and no sign; all three come
    // from the piece's own exported metadata, so the effect and the building
    // cannot disagree about what a way is.
    // 0.13.0 is RESERVED, not implemented: it is held for the stage-1 horizon
    // library while that change is in flight, which is why the version line
    // steps from 0.12.0 to 0.14.0 and why nothing here may treat the gap as a
    // free number.
    // 0.14.0 (spec-0049) adds two documents and no field: `geometry-brief.json`,
    // the whole map's written brief reduced to a list of named numbers, and
    // `layout-graph.json`, the campaign's space as a graph — places, the
    // connections between them, the authored critical path, and where each quest
    // beat happens. No coordinate appears in either; the embedding is a later
    // stage's document.
    assert!(s.contains("dsl 0.14.0"), "{s}");
    assert!(s.contains("mc 1.21.11"), "{s}");
}

#[test]
fn valid_campaign_validates_and_analyzes() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let v = delvec(&[
        "validate",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate: {}",
        String::from_utf8_lossy(&v.stdout)
    );
    let a = delvec(&[
        "analyze",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze: {}",
        String::from_utf8_lossy(&a.stdout)
    );
}

#[test]
fn build_is_byte_identical_across_runs() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("det-a");
    let out_b = tmp("det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            hw.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
    // Sanity: the tree actually contains the key contract outputs.
    assert!(a.contains_key("manifest.json"));
    assert!(a.contains_key("critical-path.json"));
    assert!(a.contains_key("datapack/pack.mcmeta"));
    assert!(a.contains_key("server/server.properties"));
}

#[test]
fn invalid_fixtures_exit_1_with_expected_code() {
    let dir = common::dsl_invalid_dir();
    let pf = common::prefabs_dir();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let patch: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let expect = patch["expect"].as_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let camp = tmp(&format!("inv-{stem}"));
        common::materialize(&patch, &camp);

        let out = delvec(&[
            "validate",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        let stdout = String::from_utf8_lossy(&out.stdout);

        // DW0143 is registry-dependent: the fixture's "unknown" item
        // (`minecraft:diamond_hoe`) is rejected by the DSL's 5-item vendored
        // subset but is a REAL 1.21.11 item. The compiler injects the full
        // registry, so it correctly accepts it (exit 0) — proving the full
        // registry is wired. See `full_item_registry_accepts_real_items`.
        if expect == "DW0143" {
            assert_eq!(
                code(&out),
                0,
                "DW0143 fixture should pass against the full registry"
            );
            count += 1;
            continue;
        }

        assert_eq!(
            code(&out),
            1,
            "fixture {stem} should exit 1 (validation failure)"
        );
        assert!(
            stdout.contains(expect),
            "fixture {stem}: expected code {expect} in diagnostics:\n{stdout}"
        );
        count += 1;
    }
    assert!(
        count >= 20,
        "expected the full invalid-fixture matrix, saw {count}"
    );
}

#[test]
fn keep_crawl_builds_and_double_build_is_byte_identical() {
    let kc = common::keep_crawl_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("kc-det-a");
    let out_b = tmp("kc-det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            kc.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(
            code(&r),
            0,
            "keep-crawl build: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "keep-crawl byte mismatch in {path}");
    }
    // The pool area shipped several distinct structures.
    let structures = a.keys().filter(|p| p.contains("/structure/keep-")).count();
    assert!(
        structures >= 3,
        "multiple keep pieces shipped, saw {structures}"
    );
}

/// The two `DW03xx` build/solver diagnostics: an unsatisfiable required anchor
/// (`DW0302`) and a `pieces` range too small for the required roles (`DW0303`).
/// Both pass validate + analyze (the DSL cannot see pool-area anchors) and fail
/// at build with exit 3 and their exact code in `--json`.
#[test]
fn pool_build_diagnostics_exit_3_with_dw03xx() {
    let pf = common::prefabs_dir();
    for (fixture, expect) in [
        ("keep-unsatisfiable-anchor.json", "DW0302"),
        ("keep-range-too-small.json", "DW0303"),
    ] {
        let patch: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(common::compiler_fixtures_dir().join(fixture)).unwrap(),
        )
        .unwrap();
        let camp = tmp(&format!("pool-{expect}"));
        common::materialize_from(&common::keep_crawl_dir(), &patch, &camp);

        // validate + analyze pass (pool anchors are invisible to the DSL layer).
        let v = delvec(&[
            "analyze",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&v), 0, "{fixture}: analyze should pass");

        let out = tmp(&format!("pool-{expect}-out"));
        let b = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(code(&b), 3, "{fixture}: build should exit 3");
        let stdout = String::from_utf8_lossy(&b.stdout);
        assert!(
            stdout.contains(expect),
            "{fixture}: expected {expect} in build diagnostics:\n{stdout}"
        );
    }
}

/// Gate-aware reachability (M2 fix 7, DW0306): a layout where an objective's
/// anchor is sealed behind a gate that only a later objective opens passes
/// validate + analyze (neither models sealed gates) but fails at build with exit 3
/// and DW0306. The clean keep-trial fixture (gate opened when the keeper is
/// greeted) builds fine — the same solver/seed, only the open-gate timing differs.
#[test]
fn gate_deadlock_exits_3_with_dw0305() {
    let pf = common::prefabs_dir();
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::compiler_fixtures_dir().join("keep-trial-gate-deadlock.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let camp = tmp("gate-deadlock");
    common::materialize_from(&common::keep_trial_dir(), &patch, &camp);
    // This build-error fixture alters keep-trial's strings; it is not an i18n test,
    // so drop the language declaration + sidecar (would otherwise fail coverage).
    common::make_english_only(&camp);

    // validate + analyze pass — sealed-gate reachability is invisible to them.
    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze should pass: {}",
        String::from_utf8_lossy(&a.stdout)
    );

    // build fails at DW0306.
    let out = tmp("gate-deadlock-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "gate deadlock should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0306"), "expected DW0306:\n{stdout}");

    // The clean keep-trial (gate opened at the greeting) builds fine.
    let clean = tmp("gate-clean-out");
    let r = delvec(&[
        "build",
        common::keep_trial_dir().to_str().unwrap(),
        "-o",
        clean.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&r),
        0,
        "clean keep-trial builds: {}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn unreachable_finale_exits_2_with_dw0201() {
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let camp = tmp("unreachable-finale");
    common::materialize(&patch, &camp);

    // validate passes (no DSL rule violated) ...
    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate should pass: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    // ... but analyze rejects it with exit 2 and DW0201.
    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&a), 2, "analyze should exit 2");
    let stdout = String::from_utf8_lossy(&a.stdout);
    assert!(stdout.contains("DW0201"), "expected DW0201:\n{stdout}");
    // The finale's own trigger names itself, so it never activates — the same
    // deep fixpoint that proves the finale unreachable also proves the quest
    // itself is a dead branch (DW0202): the two codes are companions here.
    assert!(stdout.contains("DW0202"), "expected DW0202:\n{stdout}");

    // build also stops at analysis (exit 2), never emitting.
    let out = tmp("unreachable-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&b), 2, "build should exit 2 on unreachable finale");
}

/// `DW0204`: a campaign whose two endings are both unconditionally completable
/// puts both on one critical path, so the first ending's `campaign-complete`
/// fires mid-path — the delve would end before the finale. `validate` passes
/// (nothing structural is wrong); only the flow model's ordered path replay sees
/// it, at exit 2 like every other `DW02xx`.
#[test]
fn incoherent_critical_path_exits_2_with_dw0204() {
    let camp = tmp("double-ending");
    let mut quests: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::compiler_fixtures_dir().join("branch-endings/quests.json"),
        )
        .unwrap(),
    )
    .unwrap();
    // Strip every flag gate: both endings become reachable in one playthrough.
    for quest in quests["content"]["quests"].as_array_mut().unwrap() {
        for obj in quest["objectives"].as_array_mut().unwrap() {
            obj.as_object_mut().unwrap().remove("requires_flags");
        }
        if let Some(map) = quest
            .get_mut("on_objective_complete")
            .and_then(|m| m.as_object_mut())
        {
            for effs in map.values_mut() {
                for e in effs.as_array_mut().unwrap() {
                    e.as_object_mut().unwrap().remove("requires_flags");
                }
            }
        }
    }
    common::materialize_from(
        &common::compiler_fixtures_dir().join("branch-endings"),
        &serde_json::json!({ "documents": { "quests": quests } }),
        &camp,
    );
    let pf = common::prefabs_dir();

    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate should pass: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&a), 2, "analyze should exit 2");
    let stdout = String::from_utf8_lossy(&a.stdout);
    assert!(stdout.contains("DW0204"), "expected DW0204:\n{stdout}");

    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        camp.join("out").to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&b), 2, "build must refuse an incoherent critical path");
}

/// `DW0203`: an objective can never complete because its own `requires_flags`
/// gate can only ever be satisfied by an effect on its *own* completion (a
/// self-cycle) — a genuinely deep reachability deadlock no static DSL rule
/// catches (see `self-flag-deadlock.json`).
#[test]
fn self_flag_deadlock_exits_2_with_dw0203() {
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("self-flag-deadlock.json"))
            .unwrap(),
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let camp = tmp("self-flag-deadlock");
    common::materialize(&patch, &camp);

    // validate passes (DW0172 does not fire: flag/loop IS produced by a real
    // set-flag effect; the dialogue-level checks don't see the self-cycle either).
    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate should pass: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&a), 2, "analyze should exit 2");
    let stdout = String::from_utf8_lossy(&a.stdout);
    assert!(stdout.contains("DW0203"), "expected DW0203:\n{stdout}");
}

/// `DW0300`: the prefab metadata resolves fine (the solver places the piece),
/// but the referenced `.nbt` structure file is missing from the prefabs dir at
/// build time — a prefab-library defect, not a campaign one. Uses a private
/// copy of the real prefabs dir (never mutate `campaigns/prefabs` itself, which
/// is a checkout of the separate content repo).
#[test]
fn missing_structure_file_exits_3_with_dw0300() {
    let prefabs_copy = tmp("dw0300-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs_copy);
    std::fs::remove_file(prefabs_copy.join("hello-room.nbt")).unwrap();

    let hw = common::hello_world_dir();
    let out = tmp("dw0300-out");
    let b = delvec(&[
        "build",
        hw.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "missing structure file should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0300"), "expected DW0300:\n{stdout}");
}

/// `DW0301`: a `prefab_pool` area needs the solver to place an `entry`-role
/// piece first, but the pool declares none — a prefab-library metadata defect.
/// keep-crawl's `pool/stone-keep` normally has exactly one (`keep-spawn-hall`);
/// this test relabels it away in a private copy of `pools.json`.
#[test]
fn pool_without_entry_piece_exits_3_with_dw0301() {
    let prefabs_copy = tmp("dw0301-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs_copy);
    let pools_path = prefabs_copy.join("pools.json");
    let mut pools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&pools_path).unwrap()).unwrap();
    let members = pools["pools"]["pool/stone-keep"]["members"]
        .as_array_mut()
        .unwrap();
    let mut relabeled = false;
    for m in members.iter_mut() {
        if m["role"] == "entry" {
            m["role"] = serde_json::json!("connector");
            relabeled = true;
        }
    }
    assert!(relabeled, "pool/stone-keep must have had an entry member");
    std::fs::write(&pools_path, serde_json::to_string_pretty(&pools).unwrap()).unwrap();

    let kc = common::keep_crawl_dir();
    let out = tmp("dw0301-out");
    let b = delvec(&[
        "build",
        kc.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "entry-less pool should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0301"), "expected DW0301:\n{stdout}");
}

/// `DW0310`: a *kill-less* `spawn-wave` (spec-0008 §4 "live threat" — no `kill`
/// objective ever drains it, exactly like `v04-showcase`'s `wave/ambush`) whose
/// `anchor` does not resolve in any assembled area is a dangling spawn — no
/// static DSL rule sees it (wave anchors are a build-time-only concept), so it
/// passes validate + analyze and fails only at build. Uses hello-world's single
/// bound prefab (not a pool): a pool area's solver collects wave anchors into
/// its own "required anchors" set and would instead reject this earlier with
/// `DW0302` (no pool piece provides it) — the dangling-*spawn* case this test
/// targets only surfaces on a single-prefab area, where nothing cross-checks a
/// wave's anchor against the bound prefab's declared anchors before build time.
#[test]
fn kill_less_dangling_spawn_wave_exits_3_with_dw0310() {
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("dangling-spawn-wave.json"))
            .unwrap(),
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let camp = tmp("dangling-spawn-wave");
    common::materialize(&patch, &camp);

    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze should pass (wave anchors are invisible to the DSL/analysis layers): {}",
        String::from_utf8_lossy(&a.stdout)
    );

    let out = tmp("dw0310-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "dangling kill-less spawn-wave should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0310"), "expected DW0310:\n{stdout}");
}

// ---------------------------------------------------------------------------
// i18n (spec-0001/0002 addendum): --lang build, l10n sidecar coverage
// ---------------------------------------------------------------------------

/// Overwrite `camp/l10n/zh-cn.json`'s `content` after `mutate` edits it, keeping
/// the rest of the envelope. Used to synthesize DW0180/DW0181 fixtures from the
/// real keep-trial sidecar.
fn mutate_sidecar(
    camp: &Path,
    mutate: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) {
    let path = camp.join("l10n/zh-cn.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let content = doc["content"].as_object_mut().unwrap();
    mutate(content);
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

/// keep-trial declares `zh-cn`; a full zh-cn sidecar makes both an `en` build and a
/// `--lang zh-cn` build succeed. The zh-cn build is deterministic, records the
/// language in the manifest, differs from `en` only in string-bearing files, and
/// keeps `critical-path.json` byte-identical (the bot contract is language-neutral).
#[test]
fn lang_build_localizes_only_strings_and_is_deterministic() {
    let kt = common::keep_trial_dir();
    let pf = common::prefabs_dir();
    let en = tmp("kt-lang-en");
    let zh_a = tmp("kt-lang-zh-a");
    let zh_b = tmp("kt-lang-zh-b");
    let build = |out: &Path, lang: Option<&str>| {
        let mut args = vec!["build", kt.to_str().unwrap(), "-o", out.to_str().unwrap()];
        args.extend(["--prefabs", pf.to_str().unwrap()]);
        if let Some(l) = lang {
            args.extend(["--lang", l]);
        }
        let r = delvec(&args);
        assert_eq!(
            code(&r),
            0,
            "build {lang:?}: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    };
    build(&en, None); // default = en
    build(&zh_a, Some("zh-cn"));
    build(&zh_b, Some("zh-cn"));

    let en_t = read_tree(&en);
    let zh = read_tree(&zh_a);
    let zh2 = read_tree(&zh_b);

    // Deterministic: same DSL + seed + lang → byte-identical.
    assert_eq!(
        zh.keys().collect::<Vec<_>>(),
        zh2.keys().collect::<Vec<_>>()
    );
    for (p, b) in &zh {
        assert_eq!(b, &zh2[p], "zh-cn double-build mismatch in {p}");
    }

    // i18n v2 (spec-0029): the DEFAULT build additionally ships the language
    // carrier — a resource pack holding one lang file per declared language plus
    // `en_us.json` — while a `--lang` bake, whose strings are already swapped,
    // ships none. keep-trial has no skins, so before v2 neither build had a pack.
    let pack_only = ["resourcepack.zip", "SKINS.md"];
    for p in pack_only {
        assert!(en_t.contains_key(p), "the default build must ship `{p}`");
        assert!(
            !zh.contains_key(p),
            "a `--lang` bake must ship no `{p}`: its strings are already baked"
        );
    }
    assert_eq!(
        en_t.keys()
            .filter(|p| !pack_only.contains(&p.as_str()))
            .collect::<Vec<_>>(),
        zh.keys().collect::<Vec<_>>()
    );
    let differing: Vec<&String> = en_t
        .iter()
        .filter(|(p, _)| !pack_only.contains(&p.as_str()))
        .filter(|(p, b)| *b != &zh[*p])
        .map(|(p, _)| p)
        .collect();
    assert!(
        differing.iter().any(|p| p.contains("/dialog/")),
        "expected localized dialog files to differ, got {differing:?}"
    );
    // Language-neutral outputs stay byte-identical between en and zh-cn.
    assert_eq!(
        en_t["critical-path.json"], zh["critical-path.json"],
        "critical-path.json must be language-neutral"
    );
    for p in &differing {
        assert!(
            p.contains("/dialog/")
                || p.contains("/function/")
                || p.contains("/advancement/")
                || p.contains("packtest-datapack/")
                // render-plan.json embeds each NPC's display name in its
                // `expect` checklist ("NPC named X faces the camera"); the vision
                // tier verifies the localized in-game name tag, so it localizes
                // with the build (unlike the bot-facing critical-path.json).
                || p.as_str() == "render-plan.json"
                || p.as_str() == "manifest.json",
            "unexpected non-string file differs between en and zh-cn: {p}"
        );
    }

    // The manifest records the non-canonical language; en's manifest does not.
    let zh_manifest = String::from_utf8(zh["manifest.json"].clone()).unwrap();
    assert!(
        zh_manifest.contains("\"language\": \"zh-cn\""),
        "{zh_manifest}"
    );
    let en_manifest = String::from_utf8(en_t["manifest.json"].clone()).unwrap();
    assert!(
        !en_manifest.contains("\"language\""),
        "en manifest must omit language"
    );
}

/// An undeclared `--lang` is a validation-class rejection of the requested build
/// (exit 1); `--lang en` always works even when the campaign declares no languages.
#[test]
fn undeclared_lang_exits_1() {
    let pf = common::prefabs_dir();
    let out = tmp("kt-lang-fr-out");
    let b = delvec(&[
        "build",
        common::keep_trial_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--lang",
        "fr",
    ]);
    assert_eq!(code(&b), 1, "undeclared --lang should exit 1");

    // --lang en on an English-only campaign is a no-op success.
    let hw_out = tmp("hw-lang-en-out");
    let r = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        hw_out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--lang",
        "en",
    ]);
    assert_eq!(code(&r), 0, "--lang en must always succeed");
}

/// A declared language whose sidecar is missing an inventory key fails validation
/// with `DW0180` (under-coverage); an orphan key fails with `DW0181`.
#[test]
fn l10n_coverage_gaps_fire_dw0180_and_dw0181() {
    let pf = common::prefabs_dir();

    // DW0180: drop a required key.
    let miss = tmp("l10n-missing");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &miss);
    mutate_sidecar(&miss, |c| {
        c.remove("world.title")
            .expect("key present in the real sidecar");
    });
    let out = delvec(&[
        "validate",
        miss.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&out), 1, "missing key should exit 1");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0180"), "expected DW0180:\n{s}");

    // DW0181: add an orphan key not in the inventory.
    let orph = tmp("l10n-orphan");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &orph);
    mutate_sidecar(&orph, |c| {
        c.insert("world.subtitle".into(), serde_json::json!("多余的键"));
    });
    let out = delvec(&[
        "validate",
        orph.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&out), 1, "orphan key should exit 1");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0181"), "expected DW0181:\n{s}");
}

/// AUDIT-P0: the machine completion-marker channel is **reserved** (`DW0182`). The
/// validation bot's only proof that an objective completed is the anchored
/// `[dw:complete <campaign> <token>]` chat line; if authored — or LLM-translated —
/// player-visible text could carry that sigil, a step could be made to pass without
/// its objective ever completing. Both halves are closed: the English source and
/// every declared language's sidecar.
#[test]
fn reserved_marker_sigil_in_player_text_fires_dw0182() {
    let pf = common::prefabs_dir();

    // Authored English: a world title carrying the sigil.
    let authored = tmp("marker-authored");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &authored);
    let world_path = authored.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    world["content"]["title"] = serde_json::json!("Keep Trial [dw:complete keep-trial obj/enter]");
    std::fs::write(&world_path, serde_json::to_string_pretty(&world).unwrap()).unwrap();
    let out = delvec(&[
        "validate",
        authored.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&out),
        1,
        "a forged marker in authored text must exit 1"
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0182"), "expected DW0182:\n{s}");

    // Translated: the English stays clean, the sidecar smuggles the sigil through.
    let translated = tmp("marker-translated");
    common::materialize_from(
        &common::keep_trial_dir(),
        &serde_json::json!({}),
        &translated,
    );
    mutate_sidecar(&translated, |c| {
        c.insert(
            "world.title".into(),
            serde_json::json!("[dw:complete keep-trial campaign]"),
        );
    });
    let out = delvec(&[
        "validate",
        translated.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&out),
        1,
        "a forged marker in a translation must exit 1"
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0182"), "expected DW0182:\n{s}");
}

/// Recursively copy a directory tree (campaign dir incl. `skins/`).
fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        let to = dst.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &to);
        } else {
            std::fs::copy(&path, &to).unwrap();
        }
    }
}

/// Copy the v04-showcase campaign into a temp dir, replacing one substring in
/// `quests.json` (a targeted patch), and return the campaign dir.
/// A copy of the v0.4 showcase with `quests.json` patched STRUCTURALLY (see
/// `common::patch_doc`: a textual splice that stops matching is a silent no-op,
/// and the test then asserts against an unpatched campaign).
fn showcase_with_quests_patch(
    name: &str,
    f: impl FnOnce(&mut serde_json::Value),
) -> std::path::PathBuf {
    let camp = tmp(name);
    copy_dir(&common::compiler_fixtures_dir().join("v04-showcase"), &camp);
    common::patch_file(&camp.join("quests.json"), f);
    camp
}

/// The showcase's one `cutscene` effect: `quests[1].on_objective_complete
/// ["obj/slay"][3]`. Panics if it moves, which is the point.
fn showcase_cutscene(d: &mut serde_json::Value) -> &mut serde_json::Value {
    let e = &mut common::objective_effects(d, 1, "obj/slay")[3];
    assert_eq!(e["type"], "cutscene", "the showcase cutscene has moved");
    e
}

/// The v0.4 showcase — exercising the collision-safe walked `move-npc` path and a
/// valid cutscene — builds and is byte-identical across a double build (ADR-0006
/// determinism gate over the new A*-planned per-tick emission).
#[test]
fn v04_showcase_double_build_is_byte_identical() {
    let dir = common::compiler_fixtures_dir().join("v04-showcase");
    let pf = common::prefabs_dir();
    let out_a = tmp("v04-det-a");
    let out_b = tmp("v04-det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(
            code(&r),
            0,
            "showcase build: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "showcase byte mismatch in {path}");
    }
    // The A*-planned walked path shipped a multi-waypoint per-tick driver.
    let tick = String::from_utf8(
        a["datapack/data/v04-showcase/function/mv_tick_keeper_objective.mcfunction"].clone(),
    )
    .unwrap();
    let tps = tick.matches("run tp @e[tag=dw_npc_keeper]").count();
    assert!(
        tps > 20,
        "expected a many-tick walked path, saw {tps} waypoints"
    );
}

/// Every per-tick `tp` of a walked `move-npc` carries the **bearing of the segment
/// it is about to walk**, so the body faces where it is going instead of gliding
/// backwards on a stale yaw (owner playtest, island round 13). Asserted against the
/// real emitted driver: each line's yaw is recomputed from that waypoint's own
/// delta, and the walk must contain a mid-path direction change — a corner turns on
/// the tick it is taken, with no smoothing.
#[test]
fn walked_move_npc_tps_carry_the_segment_bearing() {
    let dir = common::compiler_fixtures_dir().join("v04-showcase");
    let pf = common::prefabs_dir();
    let out = tmp("mv-yaw");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&r),
        0,
        "showcase build: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let tick = std::fs::read_to_string(
        out.join("datapack/data/v04-showcase/function/mv_tick_keeper_objective.mcfunction"),
    )
    .unwrap();

    // (x, y, z, yaw, pitch) per tp line, in tick order.
    let mut wp: Vec<(f64, f64, f64, i32, i32)> = Vec::new();
    for line in tick.lines() {
        let Some((_, tail)) = line.split_once("run tp @e[tag=dw_npc_keeper] ") else {
            continue;
        };
        let f: Vec<&str> = tail.split_whitespace().collect();
        assert_eq!(
            f.len(),
            5,
            "a walked tp must carry x y z yaw pitch — a bare position leaves the body's \
             stale facing: {line}"
        );
        wp.push((
            f[0].parse().unwrap(),
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
            f[4].parse().unwrap(),
        ));
    }
    assert!(wp.len() > 20, "expected a many-tick walked path");

    // The bearing of waypoint i is the bearing of the segment i -> i+1 (MC yaw:
    // 0 = +z south, atan2(-dx, dz)); a segment with no horizontal motion inherits
    // the previous bearing. The final waypoint keeps the last leg's facing.
    let mut expect = 0i32;
    let mut seeded = false;
    for (i, w) in wp.iter().enumerate() {
        assert_eq!(w.4, 0, "a level walk is emitted with pitch 0");
        if i + 1 < wp.len() {
            let (dx, dz) = (wp[i + 1].0 - w.0, wp[i + 1].2 - w.2);
            if dx.abs() >= 1e-6 || dz.abs() >= 1e-6 {
                expect = (((-dx).atan2(dz).to_degrees().round() as i32 % 360) + 360) % 360;
                seeded = true;
            }
        }
        if seeded {
            assert_eq!(
                w.3, expect,
                "tick {i} tp faces {} but its own movement bears {expect}",
                w.3
            );
        }
    }

    // A corner turns: this route is not a straight line, so the driver must show
    // more than one bearing.
    let distinct: std::collections::BTreeSet<i32> = wp.iter().map(|w| w.3).collect();
    assert!(
        distinct.len() > 1,
        "expected a direction change mid-walk, saw only yaw {distinct:?}"
    );
}

/// A `move-npc` whose destination cannot be reached over the solved geometry fails
/// the build with exit 3 and `DW0307` (spec-0008 addendum). keep-crawl places the
/// keeper in the gatehouse and `anchor/objective` in the keep — two areas across
/// the inter-area void, with no floor between — so the keeper cannot walk there.
/// (Only the quests stage is bumped to 0.4.0; the v0.4-effect gate keys off it.)
#[test]
fn move_unroutable_exits_3_with_dw0307() {
    let pf = common::prefabs_dir();
    let camp = tmp("mv-cross-void");
    copy_dir(&common::keep_crawl_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.4.0");
        common::objective_effects(d, 0, "obj/talk").push(serde_json::json!({
            "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/objective"
        }));
    });
    let out = tmp("mv-cross-void-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&b),
        3,
        "unroutable move should exit 3: {}",
        String::from_utf8_lossy(&b.stderr)
    );
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0307"), "expected DW0307:\n{stdout}");
}

/// A `move-actor` whose destination cannot be reached over the assembled geometry
/// for the actor's footprint fails the build with exit 3 and `DW0325` (spec-0014).
/// Reuses keep-crawl's cross-void geometry: an actor spawned in the gatehouse is
/// walked to `anchor/objective` in the keep — two areas across the inter-area void.
#[test]
fn move_actor_unroutable_exits_3_with_dw0325() {
    let pf = common::prefabs_dir();
    let camp = tmp("ma-cross-void");
    copy_dir(&common::keep_crawl_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        common::objective_effects(d, 0, "obj/talk").push(serde_json::json!({
            "type": "move-actor", "actor": "actor/beast", "to_anchor": "anchor/objective"
        }));
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/beast", "entity": "minecraft:zombie", "anchor": "anchor/keeper-stand" }
        ]);
    });
    let out = tmp("ma-cross-void-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&b),
        3,
        "unroutable move-actor should exit 3: {}",
        String::from_utf8_lossy(&b.stderr)
    );
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0325"), "expected DW0325:\n{stdout}");
}

/// A cutscene whose camera dolly clips a solid block fails the build with exit 3
/// and `DW0308` (spec-0008 addendum). Here the first camera waypoint is lifted
/// into the shrine ceiling.
#[test]
fn cutscene_clip_exits_3_with_dw0308() {
    let pf = common::prefabs_dir();
    // Lift the FIRST camera waypoint one block, into the shrine ceiling.
    let camp = showcase_with_quests_patch("cs-clip", |d| {
        showcase_cutscene(d)["path"][0]["offset"] = serde_json::json!([0, 3, 2]);
    });
    let out = tmp("cs-clip-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "clipping cutscene should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0308"), "expected DW0308:\n{stdout}");
}

/// A cutscene whose aim sweeps faster than the 6°/tick angular budget fails the
/// build with exit 3 and `DW0347`: the showcase's known-air dolly is
/// sped up to 1 s and aimed at a `look_at` subject passing 1 block abeam —
/// ~8.6°/tick at closest approach, a spin, not a shot.
#[test]
fn cutscene_over_angular_budget_exits_3_with_dw0347() {
    let pf = common::prefabs_dir();
    // Halve the duration and add a `look_at` (v0.6 surface) the pan cannot
    // reach inside the budget.
    let camp = showcase_with_quests_patch("cs-spin", |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        let cs = showcase_cutscene(d);
        cs["seconds"] = serde_json::json!(1);
        cs["look_at"] = serde_json::json!({ "anchor": "anchor/objective", "offset": [1, 2, 1] });
    });
    let out = tmp("cs-spin-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "over-budget pan should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0347"), "expected DW0347:\n{stdout}");
}

/// Wave-capacity guard: a `spawn-wave` whose mob count exceeds the
/// standable footing of its own room fails the build with `DW0312` and exit 2
/// (analysis-tier — a content-design capacity mistake, like reachability `DW02xx`,
/// not a compiler/geometry defect). keep-vertical's single wave is blown up past
/// any room's cell count; the diagnostic names the wave so a zero-context author
/// knows to shrink it or use a bigger room, not to touch the socket seams.
#[test]
fn oversized_wave_exits_2_with_dw0312() {
    let pf = common::prefabs_dir();
    let camp = tmp("wave-overflow");
    copy_dir(&common::keep_vertical_dir(), &camp);
    let qp = camp.join("quests.json");
    let mut quests: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&qp).unwrap()).unwrap();
    // Far more mobs than any assembled room can seat on distinct standable cells.
    quests["content"]["waves"][0]["mobs"][0]["count"] = serde_json::json!(100_000);
    std::fs::write(&qp, serde_json::to_string_pretty(&quests).unwrap()).unwrap();

    let out = tmp("wave-overflow-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&b),
        2,
        "oversized wave should exit 2 (analysis-tier capacity guard): {}",
        String::from_utf8_lossy(&b.stderr)
    );
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0312"), "expected DW0312:\n{stdout}");
    assert!(
        stdout.contains("wave/guards"),
        "the diagnostic must name the offending wave:\n{stdout}"
    );
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}

/// spec-0013: a v0.6 ocean + boundary campaign builds, is byte-identical across a
/// double build (determinism gate over the new generator-settings + boundary
/// emission), ships the ocean superflat generator-settings, wires the boundary
/// clock (`dw:cp` init, `dw:region` mirror, scheduled tick + macro return), and
/// emits the boundary PackTests. Only the stage-1 world doc is v0.6 (per-stage
/// versions; the v0.6 gate keys off `world`).
#[test]
fn v06_ocean_boundary_builds_byte_identical_and_wires_return() {
    let pf = common::prefabs_dir();
    let camp = tmp("v06-ocean");
    copy_dir(&common::hello_world_dir(), &camp);
    let world = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "boundary": { "margin": 20 },
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;
    std::fs::write(camp.join("world.json"), world).unwrap();

    let out_a = tmp("v06-ocean-a");
    let out_b = tmp("v06-ocean-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(
            code(&r),
            0,
            "ocean build: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "ocean byte mismatch in {path}");
    }

    // Ocean superflat generator-settings shipped (bedrock/stone/water).
    let props = String::from_utf8(a["server/server.properties"].clone()).unwrap();
    assert!(
        props.contains("generator-settings={\"biome\":\"minecraft:ocean\""),
        "expected ocean generator-settings, got:\n{props}"
    );
    assert!(
        props.contains("minecraft:water"),
        "ocean must layer water: {props}"
    );

    // Boundary clock wired in setup: dw:cp init + dw:region mirror + scheduled tick.
    let setup =
        String::from_utf8(a["datapack/data/hello-world/function/setup_finish.mcfunction"].clone())
            .unwrap();
    assert!(
        setup.contains("data modify storage dw:cp pos set value"),
        "dw:cp init missing: {setup}"
    );
    assert!(
        setup.contains("data modify storage dw:region bounds set value"),
        "dw:region mirror missing: {setup}"
    );
    assert!(
        setup.contains("schedule function hello-world:boundary_tick 20t"),
        "boundary clock not started: {setup}"
    );

    // The macro return teleports to the last checkpoint and shows the message.
    let ret = String::from_utf8(
        a["datapack/data/hello-world/function/boundary_return.mcfunction"].clone(),
    )
    .unwrap();
    assert!(
        ret.contains("$tp @s $(x) $(y) $(z)"),
        "macro tp missing: {ret}"
    );
    assert!(
        ret.contains("title @s actionbar"),
        "actionbar message missing: {ret}"
    );

    // The render layer must be TOLD the horizon, never guess it from blocks:
    // an ocean-horizon delve's frames need Chunky's ambient water plane, and its
    // height is the compiler's sea-level datum.
    let rp: serde_json::Value = serde_json::from_slice(&a["render-plan.json"]).unwrap();
    assert_eq!(rp["horizon"]["kind"], "ocean", "render plan horizon: {rp}");
    assert_eq!(rp["horizon"]["sea_level"], 62);

    // Boundary PackTests emitted.
    assert!(
        a.contains_key("packtest-datapack/data/hello-world/test/v06_boundary_return.mcfunction"),
        "boundary return packtest missing"
    );
    assert!(
        a.contains_key("packtest-datapack/data/hello-world/test/v06_boundary_inside.mcfunction"),
        "boundary inside packtest missing"
    );
}

/// spec-0013: absent `horizon`/`boundary` keeps a v0.5 campaign byte-identical to
/// its pre-v0.6 output — the void generator-settings is unchanged and no boundary
/// wiring leaks in. Guards the additive-superset promise.
#[test]
fn v06_absent_fields_keep_void_output_unchanged() {
    let pf = common::prefabs_dir();
    let out = tmp("v06-void");
    let r = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&r),
        0,
        "void build: {}",
        String::from_utf8_lossy(&r.stderr)
    );
    let tree = read_tree(&out);
    let props = String::from_utf8(tree["server/server.properties"].clone()).unwrap();
    assert!(
        props.contains("generator-settings={\"biome\":\"minecraft:the_void\",\"layers\":[]}"),
        "void generator-settings must be unchanged: {props}"
    );
    assert!(
        !tree.contains_key("datapack/data/hello-world/function/boundary_tick.mcfunction"),
        "no boundary function without a declared boundary"
    );
    // A void horizon emits no `horizon` key at all, so the render plan of a
    // campaign that declares nothing stays byte-identical.
    let rp: serde_json::Value = serde_json::from_slice(&tree["render-plan.json"]).unwrap();
    assert!(rp.get("horizon").is_none(), "void must not stamp a horizon");
}

/// A routable v0.6 campaign (patched hello-world) builds cleanly and the emitted
/// datapack carries the actor mechanics: a NoAI/no-loot puppet summon, a per-tick
/// move-actor tp, a `sequence` scheduling its second step at the exact tick, an
/// `unleash` that swaps the puppet for a real-AI twin, and the on-arrive vanish
/// (relocate-then-kill). Asserted against the concatenated function bodies.
#[test]
fn v06_actor_datapack_emits_the_mechanics() {
    let pf = common::prefabs_dir();
    let camp = tmp("v06-actors");
    copy_dir(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("quests.json"), |d| {
        d["dsl_version"] = serde_json::json!("0.6.0");
        common::objective_effects(d, 0, "obj/talk").extend([
            serde_json::json!({ "type": "spawn-actor", "actor": "actor/giant" }),
            serde_json::json!({
                "type": "move-actor", "actor": "actor/giant", "to_anchor": "anchor/exit",
                "on_arrive": [
                    { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" }
                ]
            }),
            serde_json::json!({ "type": "unleash-actor", "actor": "actor/giant" }),
            serde_json::json!({ "type": "sequence", "steps": [
                { "at_ticks": 0,
                  "effects": [ { "type": "spawn-actor", "actor": "actor/giant" } ] },
                { "at_ticks": 40,
                  "effects": [
                      { "type": "despawn-actor", "actor": "actor/giant", "style": "kill" }
                  ] }
            ] }),
        ]);
        d["content"]["actors"] = serde_json::json!([
            { "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
              "anchor": "anchor/keeper-stand", "facing": "east" }
        ]);
    });
    let out = tmp("v06-actors-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(
        code(&b),
        0,
        "v0.6 actor campaign should build: {}",
        String::from_utf8_lossy(&b.stdout)
    );

    let fn_dir = out.join("datapack/data/hello-world/function");
    let mut all = String::new();
    for e in std::fs::read_dir(&fn_dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) == Some("mcfunction") {
            all.push_str(&std::fs::read_to_string(&p).unwrap());
            all.push('\n');
        }
    }

    assert!(
        all.contains("summon minecraft:zombie") && all.contains("NoAI:1b"),
        "puppet is a NoAI zombie"
    );
    assert!(
        all.contains("DeathLootTable:\"minecraft:empty\""),
        "puppet drops no loot"
    );
    assert!(
        all.contains("dw_pup_giant"),
        "puppet carries its marker tag"
    );
    assert!(
        all.contains("execute unless entity @e[tag=dw_actor_giant] run summon minecraft:zombie"),
        "spawn-actor is idempotent"
    );
    assert!(
        all.contains("tp @e[tag=dw_pup_giant]"),
        "move-actor teleports the puppet"
    );
    assert!(
        all.contains("execute as @e[tag=dw_actor_giant] at @s run tp @s ~ -128 ~"),
        "on-arrive vanish relocates each actor down ITS OWN column before killing \
         (round-8: the bare `tp @e[…] ~ -128 ~` resolved against the command source, \
         dropping the body at world spawn's x/z)"
    );
    assert!(
        all.contains(" 40t"),
        "sequence schedules its second step at tick 40"
    );
    assert!(
        all.contains("execute at @e[tag=dw_pup_giant,limit=1] run summon minecraft:zombie")
            && all.contains("kill @e[tag=dw_pup_giant]"),
        "unleash summons a twin at the puppet then removes the puppet"
    );
}

/// spec-0013 sea-level datum: an `ocean` world places its areas at
/// `sea_level - island waterline` (y=60) so the island tileset's authored
/// waterline (local y=2) meets the world ocean (y=62) and its walk plane (local
/// y=3) is the vanilla-normal one block above the sea. A `void` world is
/// unchanged at y=64 — the byte-identity guarantee for every existing campaign.
#[test]
fn ocean_areas_sit_on_the_sea_level_datum_void_unchanged() {
    let pf = common::prefabs_dir();

    let place_line = |horizon: Option<&str>, name: &str| -> String {
        let camp = tmp(name);
        copy_dir(&common::hello_world_dir(), &camp);
        let mut world: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap())
                .unwrap();
        if let Some(h) = horizon {
            world["dsl_version"] = serde_json::json!("0.6.0");
            let content = world["content"].as_object_mut().unwrap();
            content.insert("horizon".into(), serde_json::json!(h));
            content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
        }
        std::fs::write(
            camp.join("world.json"),
            serde_json::to_string_pretty(&world).unwrap(),
        )
        .unwrap();
        let out = tmp(&format!("{name}-out"));
        let r = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
        std::fs::read_to_string(out.join("datapack/data/hello-world/function/place_all.mcfunction"))
            .unwrap()
    };

    let ocean = place_line(Some("ocean"), "datum-ocean");
    assert!(
        ocean.contains("place template hello-world:hello-room 0 60 0"),
        "ocean areas must sit at sea_level-2 (y=60):\n{ocean}"
    );
    let void = place_line(None, "datum-void");
    assert!(
        void.contains("place template hello-world:hello-room 0 64 0"),
        "void areas must stay at y=64 (byte-identity):\n{void}"
    );
}

/// `DW0318` on the shipped library, both horizons, one placement.
///
/// `island-beach-camp` is a real shoreline piece: its own bytes pass the
/// piece-level containment rule, and `delve-admit` counts 171 run directions
/// leaving its outer faces, which that rule deliberately does not judge —
/// *whatever this piece is placed against decides where that water goes*. This
/// test is the thing that decides it.
///
/// Placed under `horizon: ocean` the water meets the sea the piece depicts and
/// the build is green. Placed under `horizon: void` — the default, and what any
/// campaign that never declares a horizon gets — the identical geometry pours
/// thousands of water cells off the edge of the world, forever, and before this
/// check existed the build was **green** and shipped it.
fn beach_camp_campaign(name: &str, ocean: bool) -> std::path::PathBuf {
    let camp = tmp(name);
    copy_dir(&common::hello_world_dir(), &camp);
    let edit = |file: &str, f: &dyn Fn(&mut serde_json::Value)| {
        let p = camp.join(file);
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        f(&mut doc);
        if ocean {
            doc["dsl_version"] = serde_json::json!("0.6.0");
        }
        std::fs::write(&p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    };
    edit("world.json", &|d| {
        d["content"]["areas"][0]["prefab"] = serde_json::json!("prefab/island-beach-camp");
        if ocean {
            let c = d["content"].as_object_mut().unwrap();
            c.insert("horizon".into(), serde_json::json!("ocean"));
            c.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
        }
    });
    // The fixture's quest hangs on `hello-room`'s anchors; re-seat it on this
    // piece's own, and drop the gate effect (this piece declares no gate).
    edit("npcs.json", &|d| {
        d["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/crew-a");
    });
    edit("quests.json", &|d| {
        d["content"]["quests"][0]["objectives"][1]["anchor"] =
            serde_json::json!("anchor/camp-fire");
        d["content"]["quests"][0]
            .as_object_mut()
            .unwrap()
            .remove("on_objective_complete");
    });
    if ocean {
        for f in ["quest-plan.json", "classes.json", "dialogue.json"] {
            edit(f, &|_| {});
        }
    }
    camp
}

#[test]
fn a_shoreline_piece_placed_against_the_void_leaks_dw0318_and_against_the_sea_does_not() {
    let pf = common::prefabs_dir();

    // --- void: the water runs out of the world ------------------------------
    let camp = beach_camp_campaign("dw0318-void", false);
    let out = tmp("dw0318-void-out");
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(code(&r), 3, "build-tier failure:\n{log}");
    assert!(log.contains("DW0318"), "expected DW0318:\n{log}");
    assert!(
        log.contains("prefab/island-beach-camp"),
        "names the piece the water came from:\n{log}"
    );
    assert!(
        log.contains("Examined") && log.contains("fluid cell(s) across"),
        "states its binding count, not only its finding:\n{log}"
    );

    // --- ocean: the same water meets the sea --------------------------------
    let camp = beach_camp_campaign("dw0318-ocean", true);
    let out = tmp("dw0318-ocean-out");
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(code(&r), 0, "an ocean horizon holds this water:\n{log}");
    assert!(!log.contains("DW0318"), "no finding under ocean:\n{log}");

    // The binding ledger ships either way, and says what was examined.
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("validation/fluid-escape.json"))
            .expect("every assembled world emits the fluid-escape ledger"),
    )
    .unwrap();
    assert_eq!(ledger["horizon"], "ocean");
    assert_eq!(ledger["verdict"], "pass");
    assert_eq!(ledger["pieces_examined"], 1);
    let examined = ledger["fluid_cells_examined"].as_u64().unwrap();
    let outside = ledger["cells_outside_built_volume"].as_u64().unwrap();
    assert!(
        examined > 0 && outside > 0 && outside < examined,
        "the binding count is the world's water, not the finding list: {ledger}"
    );
}

/// `DW0344`: in an `ocean` world, a placed piece whose metadata declares a
/// waterline that does not land at sea level (y=62) is a build error — the piece
/// would float above the sea (an unclimbable shore) or drown under it. Nothing
/// downstream can catch this: nav, boundary, POV and PackTest all derive from the
/// very placement that is wrong. Uses a private copy of the real prefabs dir.
#[test]
fn ocean_waterline_off_sea_level_exits_3_with_dw0344() {
    let prefabs_copy = tmp("dw0344-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs_copy);
    // hello-room is not an island piece; declaring a waterline one block off the
    // convention is exactly the mis-authored-datum case the check exists for.
    let meta_path = prefabs_copy.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    meta["waterline_y"] = serde_json::json!(3);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let camp = tmp("dw0344-camp");
    copy_dir(&common::hello_world_dir(), &camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!("0.6.0");
    let content = world["content"].as_object_mut().unwrap();
    content.insert("horizon".into(), serde_json::json!("ocean"));
    content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    std::fs::write(
        camp.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();

    let out = tmp("dw0344-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "off-level waterline should exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0344"), "expected DW0344:\n{stdout}");

    // The same piece declaring the island convention (local y=2) lands its
    // waterline exactly at sea level and builds clean.
    meta["waterline_y"] = serde_json::json!(2);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let out_ok = tmp("dw0344-out-ok");
    let ok = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out_ok.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&ok),
        0,
        "convention waterline must build: {}",
        String::from_utf8_lossy(&ok.stderr)
    );

    // A `void` world is not an ocean, so the same metadata is not checked there.
    let void_camp = tmp("dw0344-void-camp");
    copy_dir(&common::hello_world_dir(), &void_camp);
    let mut m2: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    m2["waterline_y"] = serde_json::json!(3);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&m2).unwrap()).unwrap();
    let out_void = tmp("dw0344-void-out");
    let v = delvec(&[
        "build",
        void_camp.to_str().unwrap(),
        "-o",
        out_void.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "void world must ignore waterline metadata: {}",
        String::from_utf8_lossy(&v.stderr)
    );
}

/// `DW0344`'s **zero binding**: an ocean world in which the invariant examined
/// **nothing** reports under the invariant's own code, never under a second one.
///
/// This is the shape of the failure `DW0344` cannot have on its own: it is keyed
/// off an optional metadata field, so a piece that loses that field does not fail
/// the check, it silently leaves it. That is exactly what the admission tool did
/// to `waterline_y` — it read prefab metadata through a type that did not model
/// the field and wrote the document back without it — and the world it deleted
/// the field from would have gone on building green with `DW0344` binding to zero
/// pieces.
///
/// There is deliberately no discharge: the only one an author could offer
/// ("this piece needs no waterline") is the deleted declaration under another
/// name, and the only geometric one ("no piece reaches the sea") is
/// unsatisfiable while every ocean area sits at `OCEAN_BASE_Y` = 60 under a sea
/// at 62.
///
/// **The tripwire.** That last fact is asserted here rather than assumed. A
/// binding of zero earns a refusal, and the only reason this reports instead is
/// that the same global datum leaves an author no lever to satisfy one — the
/// piece really is in the water and nothing in the DSL can lift it out, so a
/// refusal would be demanding a fiction. The day a per-area datum makes a dry
/// ocean piece authorable, the sea-plane assertion below reds and the severity
/// question is reopened by this test rather than by anyone remembering a
/// comment.
///
/// Both directions, because a one-directional gate proves nothing: with the
/// declaration present the build says nothing, with it gone the build names
/// what it examined and out of how many. And a non-ocean world raises nothing
/// either way — "does not apply" and "applies and examined nothing" are
/// different states.
#[test]
fn an_ocean_world_where_nothing_declares_a_waterline_reports_dw0344_unbound() {
    let prefabs_copy = tmp("dw0364-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs_copy);
    let meta_path = prefabs_copy.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();

    let ocean_camp = tmp("dw0364-camp");
    copy_dir(&common::hello_world_dir(), &ocean_camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(ocean_camp.join("world.json")).unwrap())
            .unwrap();
    world["dsl_version"] = serde_json::json!("0.6.0");
    let content = world["content"].as_object_mut().unwrap();
    content.insert("horizon".into(), serde_json::json!("ocean"));
    content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    std::fs::write(
        ocean_camp.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();

    let build = |tag: &str, camp: &std::path::Path| -> (i32, String) {
        let out = tmp(tag);
        let r = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            prefabs_copy.to_str().unwrap(),
        ]);
        // Both streams: an advisory is written to stdout beside the build, a
        // refusal to stderr, and this test asserts across that boundary.
        (
            code(&r),
            format!(
                "{}{}",
                String::from_utf8_lossy(&r.stdout),
                String::from_utf8_lossy(&r.stderr)
            ),
        )
    };

    // Bound: the placed piece declares the convention waterline, so the datum is
    // really checked, the binding count is 1 of 1, and the build is green.
    meta["waterline_y"] = serde_json::json!(2);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let (bound_code, bound) = build("dw0344-bound-out", &ocean_camp);
    assert_eq!(
        bound_code, 0,
        "a bound ocean datum at the convention waterline must build:\n{bound}"
    );
    assert!(
        !bound.contains("the ocean-datum check examined ZERO"),
        "a check that examined a piece must not report itself unbound:\n{bound}"
    );

    // Unbound: the declaration is gone — which is precisely what an admission
    // step that did not model the field left behind. The check now binds to
    // zero pieces, and a check that examined nothing has proved nothing.
    meta.as_object_mut().unwrap().remove("waterline_y");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let (unbound_code, unbound) = build("dw0344-unbound-out", &ocean_camp);
    assert_eq!(
        unbound_code, 0,
        "the zero binding reports beside the build today:\n{unbound}"
    );
    assert!(
        unbound.contains("DW0344"),
        "the zero binding answers under the invariant's own code, not a second \
         code of its own:\n{unbound}"
    );
    assert!(
        unbound.contains("the ocean-datum check examined ZERO of 1 placed piece(s)"),
        "it must state what it examined and out of how many:\n{unbound}"
    );
    // The tripwire. This is the fact that makes a refusal undemandable rather
    // than merely unchosen: the piece really is in the water, and under the
    // single global ocean datum an author has no lever to lift it out. When a
    // per-area datum lands and a dry ocean piece becomes authorable, this
    // assertion reds — which is the point. Do not relax it; take it as the
    // signal to raise this zero binding to a refusal.
    assert!(
        unbound.contains("1 of those piece(s) stand at or below the sea plane"),
        "it must state how many pieces stand in the sea:\n{unbound}"
    );

    // A world with no ocean horizon is not in scope at all: "does not apply" and
    // "applies and examined nothing" are different states, and only the second
    // refuses.
    let void_camp = tmp("dw0344-void-camp");
    copy_dir(&common::hello_world_dir(), &void_camp);
    let (void_code, void) = build("dw0344-void-out", &void_camp);
    assert_eq!(
        void_code, 0,
        "a world with no ocean horizon has no datum to bind to:\n{void}"
    );
    assert!(
        !void.contains("the ocean-datum check examined ZERO"),
        "a non-ocean world must not report an unbound ocean datum:\n{void}"
    );
}

/// `DW0345` + the entry-anchor alias. Every campaign must resolve an ENTRY POINT —
/// the one cell that is `setworldspawn`, the class-apply teleport, the first-join
/// placement and the `dw:cp` seed. The shipped tileset library spells that anchor
/// two ways (`spawn` in the keep/cave/test tilesets, `entry` in the island one),
/// so the compiler owns the resolution; resolving NEITHER used to compile clean
/// and ship a delve with no start, which a dedicated server papers over (vanilla
/// spawn search finds the surface) and the integrated singleplayer server does
/// not (it drops the join at the build floor, inside stone).
#[test]
fn missing_entry_anchor_exits_3_with_dw0345_and_entry_is_an_alias_of_spawn() {
    let prefabs_copy = tmp("dw0345-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs_copy);
    let meta_path = prefabs_copy.join("hello-room.json");
    let read_meta = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap()
    };
    // Rename the entry anchor to the island tileset's spelling: still resolves.
    let rename_entry_anchor_to = |name: &str| {
        let mut meta = read_meta();
        let anchors = meta["anchors"].as_object_mut().unwrap();
        let v = anchors
            .remove("spawn")
            .or_else(|| anchors.remove("entry"))
            .or_else(|| anchors.remove("lobby"))
            .expect("hello-room declares an entry anchor");
        anchors.insert(name.into(), v);
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    };

    rename_entry_anchor_to("entry");
    let out_alias = tmp("dw0345-out-alias");
    let alias = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out_alias.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&alias),
        0,
        "`entry` must resolve exactly like `spawn`: {}",
        String::from_utf8_lossy(&alias.stderr)
    );
    let setup_finish = std::fs::read_to_string(
        out_alias.join("datapack/data/hello-world/function/setup_finish.mcfunction"),
    )
    .unwrap();
    assert!(
        setup_finish.contains("setworldspawn "),
        "the alias must still drive setworldspawn:\n{setup_finish}"
    );

    // Neither spelling → hard build error, not a silently start-less delve.
    rename_entry_anchor_to("lobby");
    let out = tmp("dw0345-out");
    let b = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs_copy.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&b), 3, "a campaign with no entry anchor must exit 3");
    let stdout = String::from_utf8_lossy(&b.stdout);
    assert!(stdout.contains("DW0345"), "expected DW0345:\n{stdout}");
}

// ---------------------------------------------------------------------------
// `l10n-inventory` (external-translation tooling contract, docs/reference/i18n.md)
// ---------------------------------------------------------------------------

/// Parse `l10n-inventory` stdout into its JSON document.
fn inventory_doc(campaign: &Path, lang: &str) -> serde_json::Value {
    let out = delvec(&["l10n-inventory", campaign.to_str().unwrap(), "--lang", lang]);
    assert_eq!(
        code(&out),
        0,
        "l10n-inventory: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("l10n-inventory emits one JSON document")
}

fn inventory_keys(doc: &serde_json::Value) -> BTreeSet<String> {
    doc["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["key"].as_str().unwrap().to_string())
        .collect()
}

/// The contract that makes external translation tooling safe: the key set
/// `l10n-inventory` reports is **exactly** the key set the coverage check
/// (`DW0180`) demands. Proven against the machinery itself — empty the sidecar,
/// collect every key `validate` names as missing, and compare sets. If either
/// side ever grows a key the other lacks, a translated campaign would either fail
/// validation or ship an untranslated string, and this test fails first.
#[test]
fn l10n_inventory_is_exactly_the_dw0180_coverage_set() {
    let pf = common::prefabs_dir();
    let camp = tmp("l10n-inventory-coverage");
    common::materialize_from(&common::keep_trial_dir(), &serde_json::json!({}), &camp);
    mutate_sidecar(&camp, |c| c.clear());

    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&v), 1, "an empty sidecar must fail coverage");
    let demanded: BTreeSet<String> = String::from_utf8_lossy(&v.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter(|d| d["code"] == "DW0180")
        .filter_map(|d| {
            let msg = d["message"].as_str()?;
            let (_, rest) = msg.split_once("inventory key `")?;
            let (key, _) = rest.split_once('`')?;
            Some(key.to_string())
        })
        .collect();
    assert!(!demanded.is_empty(), "expected DW0180 keys");

    let reported = inventory_keys(&inventory_doc(&camp, "zh-cn"));
    assert_eq!(
        reported, demanded,
        "l10n-inventory must report exactly the keys DW0180 demands"
    );
}

/// Inventory rows carry what a translator needs: canonical English, the NPC whose
/// dialogue tree the line belongs to (resolving to a declared NPC — the guard
/// against key-scheme drift), and the translation the current sidecar already has
/// (so a re-run only fills gaps). For a language with no sidecar, every row is
/// untranslated.
#[test]
fn l10n_inventory_carries_speakers_and_existing_translations() {
    let kt = common::keep_trial_dir();
    let doc = inventory_doc(&kt, "zh-cn");
    assert_eq!(doc["campaign_id"], "keep-trial");
    assert_eq!(doc["declared"], true);
    assert_eq!(doc["sidecar_present"], true);

    let npc_ids: BTreeSet<&str> = doc["npcs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap())
        .collect();
    assert!(npc_ids.contains("keeper"), "{npc_ids:?}");
    // Persona context a translator needs is present; plot-only fields are not.
    let keeper = doc["npcs"].as_array().unwrap()[0].clone();
    assert!(keeper["speech_style"].is_string());
    assert!(
        keeper.get("secret").is_none(),
        "persona secret must not leak"
    );

    let mut speakers = 0;
    for e in doc["entries"].as_array().unwrap() {
        let key = e["key"].as_str().unwrap();
        assert!(e["en"].is_string(), "{key} has no English source");
        assert!(
            e["existing"].is_string(),
            "{key}: the full keep-trial sidecar must round-trip"
        );
        if let Some(sp) = e["speaker"].as_str() {
            speakers += 1;
            assert!(npc_ids.contains(sp), "{key}: unknown speaker `{sp}`");
        }
        assert_eq!(
            key.starts_with("dlg.") || key.starts_with("npc."),
            e["speaker"].is_string(),
            "{key}: speaker presence must follow the key scheme"
        );
    }
    assert!(speakers > 0, "keep-trial has dialogue");

    // A language with no sidecar: same keys, nothing translated yet.
    let fresh = inventory_doc(&kt, "ja");
    assert_eq!(fresh["sidecar_present"], false);
    assert_eq!(fresh["declared"], false);
    assert_eq!(inventory_keys(&fresh), inventory_keys(&doc));
    for e in fresh["entries"].as_array().unwrap() {
        assert!(e.get("existing").is_none(), "{}", e["key"]);
    }
}
/// A warning-tier diagnostic (`DW0330`) is **reported but does not fail the run**:
/// `delvec` exits non-zero only on `Severity::Error`. This exit-code contract is what
/// makes an advisory rule possible at all — without it a warning would be an error
/// wearing a different label. Errors are unaffected (see the `DW0180`/`DW0181` test,
/// which still exits 1).
#[test]
fn dw0330_warning_reports_but_does_not_fail_the_build() {
    let pf = common::prefabs_dir();
    let dir = tmp("textfit-warning");

    let mut quests: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::hello_world_dir().join("quests.json")).unwrap(),
    )
    .unwrap();
    // `narrate` is a v0.4 effect; the hello-world fixture is v0.3.
    quests["dsl_version"] = serde_json::json!("0.6.0");
    // An on-screen title far wider than any screen renders.
    quests["content"]["quests"][0]["on_complete"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            serde_json::json!({
                "type": "narrate",
                "style": "title",
                "text": "A Title So Long That No Screen Anywhere Could Ever Hope To Show It"
            }),
        );
    common::materialize_from(
        &common::hello_world_dir(),
        &serde_json::json!({ "documents": { "quests": quests } }),
        &dir,
    );

    let out = delvec(&[
        "validate",
        dir.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("DW0330"), "expected DW0330 to be reported:\n{s}");
    assert!(
        s.contains("warning"),
        "DW0330 must render at warning severity:\n{s}"
    );
    assert_eq!(code(&out), 0, "a warning must not fail validate:\n{s}");

    // …and the same holds through a full build.
    let built = tmp("textfit-warning-out");
    let out = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        built.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&out),
        0,
        "a warning must not fail build:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// `DW0309` — a mannequin NPC declares a `skin`, but `skins/<texture_id>.png` is
/// not in the campaign directory. A build error, not a silent skip: the mannequin
/// would otherwise be summoned pointing at a texture the resource pack never
/// received, and ship as the default skin.
#[test]
fn a_missing_skin_png_is_dw0309() {
    let dir = tmp("skin-missing");
    let mut npcs: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::hello_world_dir().join("npcs.json")).unwrap(),
    )
    .unwrap();
    npcs["dsl_version"] = "0.4.0".into();
    npcs["content"]["npcs"][0]["skin"] =
        serde_json::json!({ "texture_id": "keeper", "model": "wide" });
    common::materialize_from(
        &common::hello_world_dir(),
        &serde_json::json!({ "documents": { "npcs": npcs } }),
        &dir,
    );
    // Deliberately do NOT create `skins/keeper.png`.
    let out = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        tmp("skin-missing-out").to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stderr.contains("DW0309") || stdout.contains("DW0309"),
        "expected DW0309 for a missing skin PNG:\nstderr: {stderr}\nstdout: {stdout}"
    );
    assert_ne!(code(&out), 0, "a missing skin PNG must fail the build");
}

// ---------------------------------------------------------------------------
// spec-0025 — the branch artifacts, and the v0.8 emission fence
// ---------------------------------------------------------------------------

/// The two-branch fixture builds, emits both spec-0025 artifacts under
/// `validation/`, hashes them into the manifest like every other output, and is
/// byte-identical across runs (ADR-0006).
#[test]
fn branch_artifacts_are_emitted_hashed_and_byte_identical() {
    let fx = common::compiler_fixtures_dir().join("branch-two-endings");
    let pf = common::prefabs_dir();
    let out_a = tmp("branch-a");
    let out_b = tmp("branch-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            fx.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
    for f in [
        "validation/branch-plan.json",
        "validation/branch-chronicle-hold.md",
        "validation/branch-chronicle-bolt.md",
    ] {
        assert!(a.contains_key(f), "missing {f}: {:?}", a.keys());
    }
    // Validation metadata, never shipped gameplay: nothing under `datapack/`.
    assert!(!a.keys().any(|k| k.starts_with("datapack/branch")));
    // Listed in the manifest like `critical-path-waypoints.json`.
    let manifest: serde_json::Value =
        serde_json::from_slice(&a["manifest.json"]).expect("manifest parses");
    let outputs = manifest["outputs"].as_object().expect("outputs map");
    for f in [
        "validation/branch-plan.json",
        "validation/branch-chronicle-hold.md",
    ] {
        assert!(outputs.contains_key(f), "manifest does not hash {f}");
    }
}

/// **The harness tier's input** (spec-0025 §3): one EXECUTABLE path per reachable
/// branch, in the ordinary `critical-path.json` contract, with the branch's
/// scripted dialogue choice inside its own `talk-to` step.
///
/// The identity that makes a branch run mean something: the branch the exported
/// path already walks gets a byte-identical file, so "branch coverage" is coverage
/// of the same contract the ladder has always proven — not of a second, less
/// tested one. The sibling branch differs in exactly the two ways the story does:
/// the option it takes at the fork, and the objectives it reaches afterwards.
#[test]
fn each_branch_gets_an_executable_path_in_the_critical_path_contract() {
    let fx = common::compiler_fixtures_dir().join("branch-two-endings");
    let pf = common::prefabs_dir();
    let out = tmp("branch-paths");
    let r = delvec(&[
        "build",
        fx.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    let tree = read_tree(&out);

    // The exported branch's path IS the exported path.
    assert_eq!(
        tree["validation/branch-path-hold.json"], tree["critical-path.json"],
        "the branch the critical path already walks must get the same bytes"
    );

    let bolt: serde_json::Value =
        serde_json::from_slice(&tree["validation/branch-path-bolt.json"]).unwrap();
    // Same contract the harness parses — the version fields the bot checks first.
    assert_eq!(bolt["format_version"], 2);
    assert_eq!(bolt["campaign_id"], "hello-world");
    let steps = bolt["steps"].as_array().unwrap();
    // The scripted choice rides inside the step: the bolt branch takes the option
    // the hold branch does not, and a dialog button is unclickable by a bot, so
    // the `/trigger` line the button runs is what the harness sends.
    let talk = steps
        .iter()
        .find(|s| s["action"] == "talk-to")
        .expect("a talk-to step");
    assert_eq!(talk["objective"], "obj/decide");
    assert_eq!(talk["command"], "/trigger dw.dlg_keeper set 3");
    // ...and the storyline after the fork is the bolt branch's, not the hold
    // branch's — the whole point of walking it.
    let objectives: Vec<&str> = steps
        .iter()
        .filter_map(|s| s["objective"].as_str())
        .collect();
    assert!(objectives.contains(&"obj/bolt"), "{objectives:?}");
    assert!(!objectives.contains(&"obj/watch"), "{objectives:?}");
    assert!(!objectives.contains(&"obj/walk-out"), "{objectives:?}");
    // Terminal step: the branch ends the campaign, like every proven path.
    assert_eq!(steps.last().unwrap()["action"], "assert-complete");

    // Validation metadata, hashed like the rest — never shipped gameplay.
    let manifest: serde_json::Value = serde_json::from_slice(&tree["manifest.json"]).unwrap();
    for f in [
        "validation/branch-path-hold.json",
        "validation/branch-path-bolt.json",
    ] {
        assert!(
            manifest["outputs"].as_object().unwrap().contains_key(f),
            "manifest does not hash {f}"
        );
    }
    assert!(!tree.keys().any(|k| k.starts_with("datapack/branch")));
}

/// Every REACHABLE branch gets its own waypoint artifact
/// (`validation/branch-waypoints-<slug>.json`) in the `critical-path-waypoints`
/// shape, derived from the branch's OWN path over the same assembled world its
/// per-branch DW0311 proof ran over.
///
/// Two identities pin the derivation: the branch the exported path already walks
/// gets **byte-identical** waypoints (same routes, same thinning), and a
/// fork-divergent sibling gets waypoints whose leg destinations are ITS OWN step
/// positions — never the exported path's, which is a different sequence whose
/// origins/indices must not be inherited (the same trap `Plan::branch_gate_model`
/// documents for gate fire-steps).
#[test]
fn each_reachable_branch_gets_its_own_waypoint_artifact() {
    let fx = common::compiler_fixtures_dir().join("branch-two-endings");
    let pf = common::prefabs_dir();
    let out = tmp("branch-waypoints");
    let r = delvec(&[
        "build",
        fx.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    let tree = read_tree(&out);

    // The branch the exported path walks: same routes, same bytes.
    assert_eq!(
        tree["validation/branch-waypoints-hold.json"],
        tree["validation/critical-path-waypoints.json"],
        "the branch the critical path already walks must get identical waypoints"
    );

    // The fork-divergent sibling: its legs follow ITS path, not the exported one.
    assert_ne!(
        tree["validation/branch-waypoints-bolt.json"],
        tree["validation/critical-path-waypoints.json"],
        "a fork-divergent branch must not inherit the exported path's legs"
    );
    let leg_destinations = |wp: &serde_json::Value| -> Vec<Vec<i64>> {
        wp["legs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                l["to"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_i64().unwrap())
                    .collect()
            })
            .collect()
    };
    let step_positions = |cp: &serde_json::Value| -> Vec<Vec<i64>> {
        cp["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s.get("pos"))
            .map(|p| {
                p.as_array()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_i64().unwrap())
                    .collect()
            })
            .collect()
    };
    for slug in ["hold", "bolt"] {
        let wp: serde_json::Value =
            serde_json::from_slice(&tree[&format!("validation/branch-waypoints-{slug}.json")])
                .unwrap();
        let cp: serde_json::Value =
            serde_json::from_slice(&tree[&format!("validation/branch-path-{slug}.json")]).unwrap();
        assert_eq!(wp["campaign_id"], "hello-world");
        let legs = leg_destinations(&wp);
        assert!(
            !legs.is_empty(),
            "{slug}: a branch with walked legs exports them"
        );
        let positions = step_positions(&cp);
        for to in &legs {
            assert!(
                positions.contains(to),
                "{slug}: leg destination {to:?} is not one of ITS OWN path's step \
                 positions {positions:?} — the legs must follow the branch's own path"
            );
        }
        // Every leg carries a non-empty proven polyline (the harness contract).
        for l in wp["legs"].as_array().unwrap() {
            assert!(!l["waypoints"].as_array().unwrap().is_empty());
        }
    }

    // Validation metadata, hashed like the rest — never shipped gameplay.
    let manifest: serde_json::Value = serde_json::from_slice(&tree["manifest.json"]).unwrap();
    for f in [
        "validation/branch-waypoints-hold.json",
        "validation/branch-waypoints-bolt.json",
    ] {
        assert!(
            manifest["outputs"].as_object().unwrap().contains_key(f),
            "manifest does not hash {f}"
        );
    }
    assert!(!tree.keys().any(|k| k.starts_with("datapack/branch")));
}

/// A campaign that declares no branch point emits no branch path — the whole
/// harness tier is opt-in, and hello-world's output is untouched by spec-0025.
#[test]
fn a_campaign_without_branches_emits_no_branch_path() {
    let out = tmp("no-branch-paths");
    let r = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        common::prefabs_dir().to_str().unwrap(),
    ]);
    assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    let tree = read_tree(&out);
    assert!(
        !tree.keys().any(|k| k.starts_with("validation/branch-")),
        "{:?}",
        tree.keys().collect::<Vec<_>>()
    );
}

/// **The version fence, proven on bytes.** The v0.8 surface — `branch_points`,
/// every `happening`, the named `ending` — is validation metadata with no
/// emission of its own: stripping all of it and dropping the campaign back to
/// `dsl_version 0.7.0` produces a **byte-identical `datapack/`**. That is the
/// guarantee that a 0.6/0.7 campaign cannot move by one byte because spec-0025
/// landed.
#[test]
fn the_v08_surface_changes_no_datapack_byte() {
    let fx = common::compiler_fixtures_dir().join("branch-two-endings");
    let pf = common::prefabs_dir();

    let stripped = tmp("branch-stripped-src");
    for f in common::STAGE_FILES {
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(fx.join(f)).unwrap()).unwrap();
        if v["dsl_version"] == "0.8.0" {
            v["dsl_version"] = serde_json::Value::from("0.7.0");
        }
        strip_v08(&mut v);
        std::fs::write(stripped.join(f), serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }

    let with = tmp("branch-with");
    let without = tmp("branch-without");
    for (src, out) in [(&fx, &with), (&stripped, &without)] {
        let r = delvec(&[
            "build",
            src.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    }
    let a = read_tree(&with);
    let b = read_tree(&without);
    let pack = |t: &BTreeMap<String, Vec<u8>>| -> BTreeMap<String, Vec<u8>> {
        t.iter()
            .filter(|(k, _)| k.starts_with("datapack/"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    let (pa, pb) = (pack(&a), pack(&b));
    assert!(!pa.is_empty());
    assert_eq!(
        pa.keys().collect::<Vec<_>>(),
        pb.keys().collect::<Vec<_>>(),
        "the v0.8 surface must not add or remove a datapack file"
    );
    for (path, bytes) in &pa {
        assert_eq!(bytes, &pb[path], "the v0.8 surface moved bytes in {path}");
    }
    // …and the artifacts exist only on the declaring side.
    assert!(a.contains_key("validation/branch-plan.json"));
    assert!(!b.contains_key("validation/branch-plan.json"));
}

/// Recursively drop every DSL v0.8 field from a stage document.
fn strip_v08(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.remove("happening");
            map.remove("branch_points");
            if map.get("type").and_then(|t| t.as_str()) == Some("campaign-complete") {
                map.remove("ending");
            }
            for (_, child) in map.iter_mut() {
                strip_v08(child);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_v08),
        _ => {}
    }
}
