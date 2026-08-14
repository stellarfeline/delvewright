//! What the shipped `server/server.properties` **contains** — every key, by
//! name and by value.
//!
//! A property the file does not write is decided by whichever host boots the
//! build, and a delve has two boot paths that do not share a default source: the
//! shipped image (`validation/Dockerfile.delve`) starts from the itzg base's own
//! `/image/server.properties` template, while the owner's playtest server
//! (`tools/playtest-server.sh`, `OVERRIDE_SERVER_PROPERTIES=false`) copies this
//! file in and lets the vanilla jar fill in the rest. Two hosts that decide a
//! world-affecting key differently are two different worlds (ADR-0006), and
//! where the two sources agree today it is a coincidence of upstream files this
//! project does not own.
//!
//! So the assertion here is the whole KEY SET, not the presence of a file: what
//! is pinned and what is left to the host is a reviewed decision, and adding or
//! dropping a key has to be written down here to pass. `crates/compiler/src/
//! emit.rs` (`DELVE_VIEW_DISTANCE`, `DELVE_SIMULATION_DISTANCE`) carries the
//! reasoning for the two chunk distances; `docs/reference/compiler.md` carries
//! the verdict on every key deliberately left unset.
//!
//! The other half of the binding is `validation/check-world-settings.sh`, which
//! proves the shipped image *derives* these keys instead of falling back to the
//! base image's ENV — a key emitted here but not derived there would be pinned
//! in a file nothing reads.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{
    self, BuildOutput, DELVE_SIMULATION_DISTANCE, DELVE_VIEW_DISTANCE,
};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// Exactly the keys a delve pins. Everything else on the pinned server version
/// is left to the host **on purpose**, with a per-key verdict recorded in
/// `docs/reference/compiler.md` (§World / build output).
const PINNED_KEYS: [&str; 15] = [
    "allow-nether",
    "difficulty",
    "force-gamemode",
    "gamemode",
    "generate-structures",
    "generator-settings",
    "level-name",
    "level-seed",
    "level-type",
    "online-mode",
    "pvp",
    "simulation-distance",
    "spawn-monsters",
    "spawn-protection",
    "view-distance",
];

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

/// `key=value` pairs of an emitted properties file, in file order.
fn pairs(out: &BuildOutput) -> Vec<(String, String)> {
    let text = std::str::from_utf8(
        out.get("server/server.properties")
            .expect("server/server.properties emitted"),
    )
    .expect("properties are utf-8");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let (k, v) = l
                .split_once('=')
                .unwrap_or_else(|| panic!("not a pair: {l}"));
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn value(out: &BuildOutput, key: &str) -> String {
    pairs(out)
        .into_iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("`{key}` is not written to server.properties"))
        .1
}

/// The key set is exactly [`PINNED_KEYS`], for every campaign, at every
/// `dsl_version`. Unlike the DSL surface this is not version-fenced: a campaign
/// that ships without a pinned view distance renders at the host's radius no
/// matter which `dsl_version` it declares.
#[test]
fn every_campaign_pins_exactly_the_reviewed_key_set() {
    let expected: BTreeSet<&str> = PINNED_KEYS.into_iter().collect();
    for dir in [
        common::hello_world_dir(),   // v0.2, void horizon, no waves
        common::keep_crawl_dir(),    // v0.2, multi-area
        common::keep_trial_dir(),    // v0.3, every gameplay verb
        common::keep_vertical_dir(), // v0.3, vertical layout
    ] {
        let out = build_dir(&dir);
        let got: BTreeSet<String> = pairs(&out).into_iter().map(|(k, _)| k).collect();
        let got_refs: BTreeSet<&str> = got.iter().map(String::as_str).collect();
        assert_eq!(
            got_refs,
            expected,
            "{} pins a different key set than the reviewed one; every key left \
             unset is decided by the host, so adding or dropping one is a \
             decision that belongs in PINNED_KEYS and in docs/reference/compiler.md",
            dir.display()
        );
    }
}

/// Both chunk distances are written, with the established values. A test that
/// only asserted the keys exist would pass on a host-shaped value.
#[test]
fn chunk_distances_carry_their_established_values() {
    let out = build_dir(&common::hello_world_dir());
    assert_eq!(
        value(&out, "view-distance"),
        DELVE_VIEW_DISTANCE.to_string(),
        "view-distance decides what the party can SEE; unwritten, the shipped \
         image takes it from the itzg base's own properties template"
    );
    assert_eq!(
        value(&out, "simulation-distance"),
        DELVE_SIMULATION_DISTANCE.to_string(),
        "simulation-distance decides which chunks TICK beyond the force-loaded \
         scene; unwritten, it is the vanilla jar's built-in default"
    );
    // 10 chunks = 160 blocks, the radius the horizon dossier and spec-0026 do
    // their vista arithmetic against. If either constant moves, that arithmetic
    // moves with it.
    assert_eq!(DELVE_VIEW_DISTANCE, 10);
    assert_eq!(DELVE_SIMULATION_DISTANCE, 10);
    // The two answer different questions; nothing may collapse them into one
    // knob, but today they agree and the shipped file must say so twice.
    assert_eq!(
        value(&out, "view-distance"),
        value(&out, "simulation-distance")
    );
}

/// Keys are written in sorted order. The table is a `BTreeMap` precisely so the
/// bytes cannot depend on iteration order (ADR-0006), and the two keys added
/// last sort into the middle and the end of the file rather than appending.
#[test]
fn keys_are_written_in_sorted_order() {
    let keys: Vec<String> = pairs(&build_dir(&common::hello_world_dir()))
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "properties must be emitted in sorted key order"
    );
}

/// The build's own `server/README.md` states the pinned distances. It is what an
/// operator reads before running the image, and a number stated there that the
/// properties file does not carry would be a promise nothing keeps.
#[test]
fn server_readme_states_the_pinned_distances() {
    let out = build_dir(&common::hello_world_dir());
    let readme = std::str::from_utf8(out.get("server/README.md").expect("README emitted")).unwrap();
    assert!(
        readme.contains(&format!("`view-distance={DELVE_VIEW_DISTANCE}`")),
        "server/README.md must state the pinned view distance:\n{readme}"
    );
    assert!(
        readme.contains(&format!(
            "`simulation-distance={DELVE_SIMULATION_DISTANCE}`"
        )),
        "server/README.md must state the pinned simulation distance:\n{readme}"
    );
}
