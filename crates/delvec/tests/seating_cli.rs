//! **The pairing, measured** — `delvec prefab seating` over the shipped library,
//! on every base the schema declares (spec-0060 §6, acceptance criteria 1 and 4).
//!
//! What makes this a measurement rather than a shape:
//!
//! - The population is enumerated from the library itself — documents counted on
//!   disk, pools read out of `pools.json` — and the tool's own denominators are
//!   compared against that enumeration. A run that disagrees with what is in the
//!   directory is a red whichever way it disagrees.
//! - The bases are enumerated from `delvec schema --stage world`, so a base the
//!   engine adds is a base this test runs, rather than one somebody remembers.
//! - The verdicts name the three waterline fictions by name. A declaration-only
//!   implementation reports 5 of 5 and fails here, which is the whole reason
//!   this command opens the `.nbt`.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

mod common;

/// The figures spec-0060 recorded against the content library it was written
/// over. They are asserted as well as derived: the derivation says the tool
/// agrees with the directory, and these say the directory is still the one the
/// spec measured. A movement here is a content-library change to be recorded,
/// never a number to update quietly.
const RECORDED_POOLS: usize = 4;
const RECORDED_MEMBERS: usize = 42;
const RECORDED_DOCUMENTS: usize = 36;
const RECORDED_WATERLINES: usize = 5;
const RECORDED_BORNE_OUT: usize = 2;

/// The three declarations the bytes do not bear out (spec-0060 §1.3), by name.
const FICTIONS: [&str; 3] = [
    "island-greenfield",
    "island-greenfield-bend",
    "island-mountain",
];

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

/// The library's own enumeration: prefab documents on disk, and the pools
/// `pools.json` declares with the members each holds.
fn enumerate(dir: &Path) -> (usize, usize, usize) {
    let mut documents = 0usize;
    for entry in std::fs::read_dir(dir).expect("the prefab library is a directory") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if path.file_name().and_then(|f| f.to_str()) == Some("pools.json") {
            continue;
        }
        documents += 1;
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
    (table.len(), members, documents)
}

fn library() -> PathBuf {
    common::prefabs_dir()
}

/// **Criterion 1**: a verdict for every pool, on every base, with numerator and
/// denominator at pool, member and declaration level — and the denominators are
/// the library's own.
#[test]
fn the_seating_verdict_states_the_librarys_own_denominators_on_every_base() {
    let dir = library();
    let (pools, members, documents) = enumerate(&dir);
    assert_eq!(
        (pools, members, documents),
        (RECORDED_POOLS, RECORDED_MEMBERS, RECORDED_DOCUMENTS),
        "the library this spec was measured over has moved. That is a content-repository change \
         to record, not a number to update quietly: re-measure and say so."
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
        let out = delvec(&[
            "prefab",
            "seating",
            "--horizon",
            base,
            "--prefabs",
            dir.to_str().unwrap(),
        ]);
        let text = log(&out);
        assert!(
            text.contains(&format!(
                "seating binding: horizon base `{base}`; 0 pool(s) seatable of {pools} in this \
                 library, examined over {members} member(s); {documents} document(s) read, \
                 {documents} `.nbt` opened; {RECORDED_WATERLINES} waterline declaration(s) \
                 examined, {RECORDED_BORNE_OUT} borne out by the bytes."
            )),
            "the binding line states every denominator on `{base}`:\n{text}"
        );
        // A verdict per pool, and each one carries its own numerator.
        for pool in [
            "pool/island",
            "pool/stone-keep",
            "pool/vertical-keep",
            "pool/cave-shore",
        ] {
            assert!(
                text.contains(pool),
                "`{base}` states a verdict for `{pool}`:\n{text}"
            );
        }
        assert_eq!(
            out.status.code(),
            Some(1),
            "a library no pool of which can be seated is a refusal on `{base}`:\n{text}"
        );
    }
}

/// **Criterion 4**: the waterline check reads the BYTES. Three of the five
/// declarations in this library stand over no water at all, and a
/// declaration-only implementation reports five of five.
#[test]
fn the_waterline_verdict_names_the_three_declarations_the_bytes_do_not_bear_out() {
    let dir = library();
    let out = delvec(&[
        "prefab",
        "seating",
        "--horizon",
        "ocean",
        "--prefabs",
        dir.to_str().unwrap(),
    ]);
    let text = log(&out);
    for fiction in FICTIONS {
        assert!(
            text.contains(&format!("{fiction} ")) || text.contains(&format!("{fiction}\n")),
            "the verdict names `{fiction}`:\n{text}"
        );
    }
    assert_eq!(
        text.matches("no water block anywhere in the piece (DW0887)")
            .count(),
        FICTIONS.len(),
        "exactly the three fictions, no more and no fewer:\n{text}"
    );
    assert!(
        text.contains(&format!(
            "{RECORDED_WATERLINES} waterline declaration(s) examined, {RECORDED_BORNE_OUT} borne \
             out by the bytes"
        )),
        "and both numbers, on every run:\n{text}"
    );
}

/// The same rule through the other door, and the same numbers: `DW0887` swept
/// over a whole library by `delvec prefab audit`, from one implementation.
///
/// Two entry points reading one rule is what stops a library being clean to the
/// tool and refused by the build; this asserts they agree on the census as well
/// as on the verdict.
#[test]
fn the_library_audit_and_the_seating_verdict_are_one_implementation() {
    let dir = library();
    let out = delvec(&["prefab", "audit", dir.to_str().unwrap()]);
    let text = log(&out);
    assert_eq!(
        out.status.code(),
        Some(1),
        "a library holding a fiction is a refusal:\n{text}"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("the report is JSON on stdout");
    assert_eq!(report["code"], "DW0887");
    assert_eq!(report["verdict"], "fail");
    assert_eq!(report["documents_read"], RECORDED_DOCUMENTS);
    assert_eq!(report["nbt_opened"], RECORDED_DOCUMENTS);
    assert_eq!(report["waterlines_declared"], RECORDED_WATERLINES);
    assert_eq!(report["waterlines_borne_out"], RECORDED_BORNE_OUT);
    assert_eq!(report["waterlines_refused"], FICTIONS.len());
    let named: Vec<&str> = report["refused"]
        .as_array()
        .expect("a list of refusals")
        .iter()
        .map(|r| r["prefab_id"].as_str().expect("a prefab id"))
        .collect();
    for fiction in FICTIONS {
        assert!(
            named.contains(&format!("prefab/{fiction}").as_str()),
            "the audit names `{fiction}`: {named:?}"
        );
    }
    // The census is printed on a run that finds nothing too — the gallery's own
    // library, whose five declarations the bytes all bear out.
    let clean = delvec(&["prefab", "audit", "gallery-prefabs"]);
    if clean.status.success() {
        let text = log(&clean);
        assert!(
            text.contains("waterline declaration(s) examined")
                && text.contains("borne out by the bytes"),
            "a green sweep prints its census too:\n{text}"
        );
    }
}

/// **The vacuity guard, perturbed toward the shape it exists to catch.** A
/// library the tool cannot open the bytes of must red rather than report a
/// small clean number — so a copy with its `.nbt` removed, which is exactly what
/// a declaration-only reading of a library looks like from the outside, exits
/// non-zero and says which count fell short of which.
#[test]
fn a_library_whose_bytes_cannot_be_opened_is_a_red_and_not_a_small_number() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("seating-no-bytes");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    common::copy_dir_all(&library(), &dir);
    let mut removed = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("nbt") {
            std::fs::remove_file(&path).unwrap();
            removed += 1;
        }
    }
    assert!(removed > 0, "the perturbation must remove something");
    let out = delvec(&[
        "prefab",
        "seating",
        "--horizon",
        "ocean",
        "--prefabs",
        dir.to_str().unwrap(),
    ]);
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
