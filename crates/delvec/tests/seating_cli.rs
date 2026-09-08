//! **The pairing, measured** — `delvec prefab seating` over the shipped library,
//! on every base the schema declares (spec-0060 §6, acceptance criteria 1 and 4).
//!
//! What makes this a measurement rather than a shape:
//!
//! - The population is enumerated from the library itself — documents counted on
//!   disk, pools read out of `pools.json`, waterline declarations read out of the
//!   documents by this file's own JSON reader — and the tool's own denominators
//!   are compared against that enumeration. A run that disagrees with what is in
//!   the directory is a red whichever way it disagrees.
//! - The bases are enumerated from `delvec schema --stage world`, so a base the
//!   engine adds is a base this test runs, rather than one somebody remembers.
//! - The **verdict** is judged over a library this file perturbs, never over
//!   whatever the content pin happens to hold.
//!
//! That last is the change this round made, and it is stated as one rather than
//! discovered later. These tests used to freeze the content library's own
//! figures — five waterline declarations, two borne out, three named fictions,
//! `0 pool(s) seatable` — and assert them back. Every one of those numbers is a
//! fact about a library this repository does not own, and the content round that
//! repaired the fictions turned the assertions into a demand that the repair be
//! undone. A **planted** fiction binds the same property, *a declaration-only
//! implementation reports N of N and fails*, against any library at all: it is
//! strictly more than the frozen figures asserted, because it holds at every pin
//! rather than at one.
//!
//! The agreement between this command and the build — the property spec-0060
//! §6.3 makes load-bearing — is a separate gate over the gallery's own library,
//! `tools/check-seating-agrees.py`, which enumerates every pool and every base
//! and perturbs toward both disagreement shapes.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

mod common;

fn delvec(args: &[&str]) -> Output {
    Proc::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// Every horizon base the engine declares, read from the schema export rather
/// than from a list somebody keeps.
fn declared_bases() -> Vec<String> {
    let out = delvec(&["schema", "--stage", "world"]);
    assert!(out.status.success(), "schema --stage world: {}", log(&out));
    let schema: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the world schema is JSON");
    let mut bases = Vec::new();
    collect_horizon_bases(&schema, &mut bases);
    bases.sort();
    bases.dedup();
    assert!(
        !bases.is_empty(),
        "the world schema declares no horizon base — enumerating zero bases is the unbound \
         vacuity mode, not a pass"
    );
    bases
}

/// The bases `HorizonBase` declares, taken from the schema's own definition of
/// it: one enumeration, reachable two ways in the document, so a base cannot
/// exist in one spelling and not the other.
///
/// Read through `$defs` by name rather than by walking for string literals: a
/// walk that matched values would also match a default, and a base this engine
/// stopped declaring would still be found.
fn collect_horizon_bases(schema: &serde_json::Value, out: &mut Vec<String>) {
    let def = &schema["$defs"]["HorizonBase"];
    if let Some(list) = def["enum"].as_array() {
        out.extend(list.iter().filter_map(|s| s.as_str()).map(str::to_string));
    }
    if let Some(variants) = def["oneOf"].as_array() {
        out.extend(
            variants
                .iter()
                .filter_map(|v| v["const"].as_str())
                .map(str::to_string),
        );
    }
}

/// What the library holds, counted by this file rather than by the tool.
///
/// A second reader sharing no configuration with the engine's registry — plain
/// JSON off the disk — so "the tool agrees with the directory" is a comparison
/// rather than a restatement of one reading.
struct Enumerated {
    pools: usize,
    members: usize,
    documents: usize,
    waterlines: usize,
}

fn enumerate(dir: &Path) -> Enumerated {
    let mut documents = 0usize;
    let mut waterlines = 0usize;
    for entry in std::fs::read_dir(dir).expect("the prefab library is a directory") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if path.file_name().and_then(|f| f.to_str()) == Some("pools.json") {
            continue;
        }
        documents += 1;
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        if doc.get("waterline_y").is_some_and(|v| !v.is_null()) {
            waterlines += 1;
        }
    }
    let pools: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("pools.json")).unwrap()).unwrap();
    let table = pools["pools"].as_object().expect("a pools table");
    let members: usize = table
        .values()
        .map(|m| {
            let mut ids: Vec<&str> = m["members"]
                .as_array()
                .expect("a pool declares its members")
                .iter()
                .map(|e| e["prefab"].as_str().expect("a member names a prefab"))
                .collect();
            ids.sort_unstable();
            ids.dedup();
            ids.len()
        })
        .sum();
    Enumerated {
        pools: table.len(),
        members,
        documents,
        waterlines,
    }
}

fn library() -> PathBuf {
    common::prefabs_dir()
}

fn seat(dir: &Path, base: &str) -> Output {
    delvec(&[
        "prefab",
        "seating",
        "--horizon",
        base,
        "--prefabs",
        dir.to_str().unwrap(),
    ])
}

/// Whether the engine's own reader finds no water anywhere in this piece — the
/// condition a planted `waterline_y` has to meet to be a fiction rather than a
/// number that happens to be right.
fn authors_no_water(nbt: &Path) -> bool {
    log(&delvec(&["prefab", "audit", nbt.to_str().unwrap()]))
        .contains("top authored water at no water in this piece")
}

/// A writable copy of the shipped library.
fn copy_of_library(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    common::copy_dir_all(&library(), &dir);
    dir
}

/// Every prefab document in a library directory, `pools.json` excluded.
fn documents_in(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension().and_then(|e| e.to_str()) == Some("json")
                && p.file_name().and_then(|f| f.to_str()) != Some("pools.json")
        })
        .collect();
    out.sort();
    out
}

/// **Criterion 1**: a verdict for every pool, on every base, with numerator and
/// denominator at pool, member and declaration level — and the denominators are
/// the library's own, counted by a reader that is not the engine's.
#[test]
fn the_seating_verdict_states_the_librarys_own_denominators_on_every_base() {
    let dir = library();
    let lib = enumerate(&dir);
    assert!(
        lib.pools > 0 && lib.members > 0 && lib.documents > 0,
        "the library at {} enumerates {} pool(s), {} member(s), {} document(s) — a verdict over \
         an empty population is the unbound vacuity mode, not a pass",
        dir.display(),
        lib.pools,
        lib.members,
        lib.documents
    );

    let bases = declared_bases();
    assert_eq!(
        bases.len(),
        3,
        "the schema declares {} base(s); every one of them is a row of the pairing table and \
         this test runs all of them: {bases:?}",
        bases.len()
    );
    for base in &bases {
        let out = seat(&dir, base);
        let text = log(&out);
        // Every denominator the tool prints is the one this file counted. The
        // NUMERATORS are deliberately not frozen here: how many pools a given
        // content revision can seat is a fact about that revision, and asserting
        // it is what turned this file into a demand that a repaired library be
        // un-repaired. What IS asserted about the numerator is that it agrees
        // with the exit code, below.
        assert!(
            text.contains(&format!("seating binding: horizon base `{base}`; ")),
            "the binding line names the base it judged on `{base}`:\n{text}"
        );
        for fragment in [
            format!("pool(s) seatable of {} in this library", lib.pools),
            format!("examined over {} member(s)", lib.members),
            format!(
                "{} document(s) read, {} `.nbt` opened",
                lib.documents, lib.documents
            ),
            format!("{} waterline declaration(s) examined", lib.waterlines),
        ] {
            assert!(
                text.contains(&fragment),
                "on `{base}` the tool's denominators must be the directory's — expected \
                 `{fragment}`:\n{text}"
            );
        }
        // A verdict per pool, by name, from the library's own table.
        let pools: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("pools.json")).unwrap())
                .unwrap();
        for pool in pools["pools"].as_object().unwrap().keys() {
            assert!(
                text.contains(pool.as_str()),
                "`{base}` states a verdict for `{pool}`:\n{text}"
            );
        }
        // The exit code is the verdict, both ways: green exactly when every pool
        // seats. A tool whose exit code and whose table disagreed would be this
        // whole round's defect one layer out.
        let all_seatable = text.contains(&format!(
            "{n} pool(s) seatable of {n} in this library",
            n = lib.pools
        ));
        assert_eq!(
            out.status.code() == Some(0),
            all_seatable,
            "on `{base}` the exit code and the verdict must be one answer:\n{text}"
        );
    }
}

/// **Criterion 4, bound against a library this test controls**: the waterline
/// check reads the BYTES.
///
/// Three declarations are planted over pieces with no water anywhere — the exact
/// shape the shipped island tileset carried, where a generator wrote its
/// authoring convention as a constant into every document including the three
/// inland pieces. A declaration-only implementation reports every one of them
/// borne out and fails here.
#[test]
fn a_planted_waterline_over_no_water_is_named_and_the_census_moves() {
    let dir = copy_of_library("seating-planted-fictions");

    // **The baseline is a RUN, not a count of documents.** How many
    // declarations a given content revision already carries, and how many of
    // them its bytes bear out, are facts about that revision; what this test
    // asserts is the DELTA the perturbation moves, which is a property of the
    // tool and holds at every pin.
    let before = census(&log(&seat(&dir, "ocean")));

    // Chosen by reading the BYTES rather than by name: the perturbation has to
    // be a fiction, and a piece that already authors water would not be one.
    let mut planted: Vec<String> = Vec::new();
    for path in documents_in(&dir) {
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        if doc.get("waterline_y").is_some_and(|v| !v.is_null()) {
            continue;
        }
        if !authors_no_water(&path.with_extension("nbt")) {
            continue;
        }
        doc["waterline_y"] = serde_json::json!(2);
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        planted.push(path.file_stem().unwrap().to_str().unwrap().to_string());
        if planted.len() == 3 {
            break;
        }
    }
    assert_eq!(
        planted.len(),
        3,
        "the perturbation planted {} fiction(s); a test whose perturbation did not happen proves \
         nothing",
        planted.len()
    );

    let text = log(&seat(&dir, "ocean"));
    let after = census(&text);
    for fiction in &planted {
        assert!(
            text.contains(&format!("{fiction} ")) || text.contains(&format!("{fiction}\n")),
            "the verdict names `{fiction}`:\n{text}"
        );
    }
    assert_eq!(
        after.0,
        before.0 + planted.len(),
        "each planted declaration is one more declaration examined:\n{text}"
    );
    assert_eq!(
        after.1, before.1,
        "and NONE of them is borne out by the bytes — a declaration-only implementation reports \
         {n} of {n} and passes here:\n{text}",
        n = after.0,
    );
    assert_eq!(
        text.matches("no water block anywhere in the piece (DW0887)")
            .count()
            - before_fictions(&dir),
        planted.len(),
        "exactly the planted fictions, no more and no fewer:\n{text}"
    );
}

/// The waterline census the binding line states: declarations examined, and how
/// many of them the bytes bear out.
fn census(text: &str) -> (usize, usize) {
    let line = text
        .lines()
        .find(|l| l.contains("waterline declaration(s) examined"))
        .unwrap_or_else(|| panic!("every run prints its waterline census:\n{text}"));
    let nth = |before: &str| -> usize {
        line.split(before)
            .next()
            .and_then(|s| s.rsplit(' ').next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or_else(|| panic!("no number before `{before}` in: {line}"))
    };
    (
        nth(" waterline declaration(s) examined"),
        nth(" borne out by the bytes"),
    )
}

/// Fictions this library already holds, so the perturbation's own count is a
/// delta rather than a claim about a content revision.
fn before_fictions(dir: &Path) -> usize {
    // Counted from a copy with the perturbation absent: the same library, read
    // through the same command, before anything was planted in it.
    let pristine = copy_of_library("seating-pristine");
    let _ = dir;
    log(&seat(&pristine, "ocean"))
        .matches("no water block anywhere in the piece (DW0887)")
        .count()
}

/// The same rule through the other two doors, and the same numbers: `DW0887`
/// swept over a whole library by `delvec prefab audit <dir>`, and asked of one
/// asset at the admission event by `delvec prefab audit <piece>.nbt`.
///
/// Three entry points reading one rule is what stops a library being clean to
/// one reader and refused by another. The per-file arm is the one that did not
/// exist: `DW0887` reached a piece only through a directory nothing ever hands
/// this command, so the same planted fiction passed every per-file audit with
/// the code appearing zero times.
#[test]
fn the_library_audit_and_the_seating_verdict_are_one_implementation() {
    let dir = copy_of_library("seating-audit-agrees");
    let lib = enumerate(&dir);
    // Again a baseline RUN: what a given content revision already declares and
    // already fails is that revision's fact, and this test is about the delta.
    let before: serde_json::Value = serde_json::from_slice(
        &delvec(&["prefab", "audit", dir.to_str().unwrap()]).stdout,
    )
    .expect("the sweep report is JSON");

    let target = documents_in(&dir)
        .into_iter()
        .find(|p| {
            let doc: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
            doc.get("waterline_y")
                .is_none_or(serde_json::Value::is_null)
                && authors_no_water(&p.with_extension("nbt"))
        })
        .expect("some piece in this library authors no water");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&target).unwrap()).unwrap();
    doc["waterline_y"] = serde_json::json!(2);
    std::fs::write(&target, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    let planted_id = format!("prefab/{}", target.file_stem().unwrap().to_str().unwrap());

    let out = delvec(&["prefab", "audit", dir.to_str().unwrap()]);
    let text = log(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a library holding a fiction is a refusal:\n{text}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the report is JSON on stdout");
    let n = |v: &serde_json::Value, k: &str| v[k].as_u64().expect("a count") as usize;
    assert_eq!(report["code"], "DW0887");
    assert_eq!(report["verdict"], "fail");
    // The denominators are the directory's, counted by this file.
    assert_eq!(n(&report, "documents_read"), lib.documents);
    assert_eq!(n(&report, "nbt_opened"), lib.documents);
    // And the numerators moved by exactly the perturbation.
    assert_eq!(
        n(&report, "waterlines_declared"),
        n(&before, "waterlines_declared") + 1
    );
    assert_eq!(
        n(&report, "waterlines_borne_out"),
        n(&before, "waterlines_borne_out")
    );
    assert_eq!(
        n(&report, "waterlines_refused"),
        n(&before, "waterlines_refused") + 1
    );
    let named: Vec<&str> = report["refused"]
        .as_array()
        .expect("a list of refusals")
        .iter()
        .map(|r| r["prefab_id"].as_str().expect("a prefab id"))
        .collect();
    assert!(
        named.contains(&planted_id.as_str()),
        "the sweep names the planted fiction: {named:?}"
    );

    let one = delvec(&[
        "prefab",
        "audit",
        target.with_extension("nbt").to_str().unwrap(),
    ]);
    let one_text = log(&one);
    assert_eq!(
        one.status.code(),
        Some(1),
        "the per-file arm refuses the same fiction:\n{one_text}"
    );
    assert!(one_text.contains("DW0887"), "{one_text}");
    assert!(
        one_text.contains("1 declaration(s) examined, 0 borne out by the bytes, 1 refused"),
        "and states its binding over the one asset:\n{one_text}"
    );
    let one_report: serde_json::Value =
        serde_json::from_slice(&one.stdout).expect("the per-file report is JSON");
    assert_eq!(one_report["verdict"], "fail");
    assert_eq!(one_report["waterline"]["state"], "checked");
    assert_eq!(one_report["waterline"]["refused"], 1);
}

/// **The per-file waterline door's four states are four different artifacts.**
///
/// The report used to say the same thing — nothing — for a piece whose
/// declaration was held to the bytes, a piece that declares none, a piece with
/// no document beside it yet, and a document nobody can parse. One silence, read
/// as the first.
#[test]
fn the_per_file_waterline_door_names_which_of_its_four_states_it_is_in() {
    let dir = copy_of_library("seating-waterline-states");
    let docs = documents_in(&dir);

    // `undeclared`: a piece stating no waterline.
    let plain = docs
        .iter()
        .find(|p| {
            let d: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
            d.get("waterline_y").is_none_or(serde_json::Value::is_null)
        })
        .expect("some piece declares no waterline")
        .clone();
    let out = delvec(&[
        "prefab",
        "audit",
        plain.with_extension("nbt").to_str().unwrap(),
    ]);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["waterline"]["state"], "undeclared");
    assert_eq!(report["waterline"]["declarations"], 0);

    // `no-document`: the bytes with no metadata beside them, which is how an
    // ingested piece arrives.
    let orphan = dir.join("orphan.nbt");
    std::fs::copy(plain.with_extension("nbt"), &orphan).unwrap();
    let out = delvec(&["prefab", "audit", orphan.to_str().unwrap()]);
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["waterline"]["state"], "no-document");

    // `unreadable`: a document nobody can parse is NOT a document with no
    // waterline, and the silence would read as the pass.
    let broken = dir.join("broken.nbt");
    std::fs::copy(plain.with_extension("nbt"), &broken).unwrap();
    std::fs::write(dir.join("broken.json"), "{ this is not json").unwrap();
    let out = delvec(&["prefab", "audit", broken.to_str().unwrap()]);
    let text = log(&out);
    assert_ne!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("DW0887"), "{text}");
    assert!(text.contains("unreadable"), "{text}");
}

/// **The vacuity guard, perturbed toward the shape it exists to catch.** A
/// library the tool cannot open the bytes of must red rather than report a
/// small clean number — so a copy with its `.nbt` removed, which is exactly what
/// a declaration-only reading of a library looks like from the outside, exits
/// non-zero and says which count fell short of which.
#[test]
fn a_library_whose_bytes_cannot_be_opened_is_a_red_and_not_a_small_number() {
    let dir = copy_of_library("seating-no-bytes");
    let mut removed = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("nbt") {
            std::fs::remove_file(&path).unwrap();
            removed += 1;
        }
    }
    assert!(removed > 0, "the perturbation must remove something");
    let out = seat(&dir, "ocean");
    let text = log(&out);
    assert_ne!(
        out.status.code(),
        Some(0),
        "a library with no bytes to read cannot be reported as examined:\n{text}"
    );
    assert!(
        text.contains("`.nbt` opened"),
        "and it says which denominator fell short:\n{text}"
    );
}

/// **`--json` is answered, not swallowed.** The flag was accepted and ignored: a
/// caller that asked for machine output got the table and no signal that it had
/// not been heard.
///
/// The object carries what the table carries — the per-pool verdicts, every
/// reason with its code and its shape, and every binding count — so what is
/// asserted is that the two forms are one answer.
#[test]
fn the_json_verdict_carries_what_the_table_carries() {
    let dir = library();
    let lib = enumerate(&dir);
    let out = delvec(&[
        "--json",
        "prefab",
        "seating",
        "--horizon",
        "ocean",
        "--prefabs",
        dir.to_str().unwrap(),
    ]);
    let doc: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("`--json` must print one JSON object: {e}\n{}", log(&out)));
    assert_eq!(doc["check"], "seating");
    assert_eq!(doc["horizon_base"], "ocean");
    assert_eq!(doc["binding"]["pools"], lib.pools);
    assert_eq!(doc["binding"]["members"], lib.members);
    assert_eq!(doc["binding"]["documents_read"], lib.documents);
    assert_eq!(doc["binding"]["nbt_opened"], lib.documents);
    assert_eq!(doc["binding"]["waterlines_declared"], lib.waterlines);
    let pools = doc["pools"].as_array().expect("a verdict per pool");
    assert_eq!(pools.len(), lib.pools);
    for p in pools {
        assert!(
            p["verdict"] == "SEATABLE" || p["verdict"] == "REFUSED",
            "a pool's verdict is one of the two: {p}"
        );
        for r in p["reasons"].as_array().expect("a reason list") {
            assert!(
                r["code"].as_str().is_some_and(|c| c.starts_with("DW")),
                "every reason names its code: {r}"
            );
            assert!(
                r["shape"].as_str().is_some_and(|s| !s.is_empty()),
                "and its SHAPE, so a caller narrowing this set says which member it means rather \
                 than matching a substring of the message: {r}"
            );
        }
    }
    // The text form's own line is carried too, so a reader of either form reads
    // the same sentence.
    assert!(
        doc["binding"]["line"]
            .as_str()
            .is_some_and(|l| l.starts_with("seating binding: horizon base `ocean`;")),
        "{doc}"
    );
}
