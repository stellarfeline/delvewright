//! Derive the vendored block-appearance table from a Minecraft client jar.
//!
//! ```text
//! cargo run --release -p delvec --example derive-block-appearance -- \
//!     <client.jar> <out.json> [<blocks-registry.json>]
//! ```
//!
//! **Why this is an example and not a `delvec` subcommand.** `delvec` is what an
//! authoring session runs, and every flag on it is author-facing surface that
//! owes a demo level (`tools/ci/check-demo-levels.py`, `docs/demo-levels.md`).
//! Regenerating a table this repository commits is not something a creator does:
//! they never hold the file, and the release archive they install cannot carry
//! this program at all. Its reader is whoever moves ADR-0009's Minecraft pin,
//! from a checkout, and `tools/maintenance/refresh-block-appearance.py` is the
//! step that runs it.
//!
//! **The derivation is still the only one.** It is
//! `delvec::compiler::view::blockcolor::Deriver` — what `delvec palette` and the
//! interactive viewer run against a jar — called here over the pinned block
//! registry instead of over a prefab's palette.
//!
//! The registry is read from `crates/dsl/data/blocks-1.21.11.json`, the
//! container the vendored table is derived from, at the path this file's own
//! crate sits beside. It is read rather than reached through
//! `delvewright_dsl::blocks`, because that crate's number is the campaign-format
//! version carried by every campaign document: a maintenance program is not a
//! reason to move it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::view::assets::Assets;
use delvec::compiler::view::blockcolor::{DEFAULT_BIOME, Deriver, PaletteTable};
use delvec::schem::blocks::MC_VERSION;

fn die(msg: String) -> ! {
    eprintln!("derive-block-appearance: FAIL — {msg}");
    std::process::exit(1)
}

/// `crates/delvec/data/../../dsl/data/blocks-<pin>.json`, the default container.
fn default_registry() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dsl/data")
        .join(format!("blocks-{MC_VERSION}.json"))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        die(
            "usage: derive-block-appearance <client.jar> <out.json> [<blocks-registry.json>]"
                .into(),
        );
    }
    let jar = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let registry_path = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(default_registry);

    let assets = match Assets::open(jar) {
        Ok(a) => a,
        Err(e) => die(format!("open {}: {e}", jar.display())),
    };
    // The table stands in for the jar on every machine that has none, so it may
    // only be derived FROM the pinned jar: a source that does not say which
    // version it is cannot be taken for it.
    match assets.declared_version().as_deref() {
        Some(v) if v == MC_VERSION => {}
        other => die(format!(
            "{} declares {} and this table is the {MC_VERSION} one",
            jar.display(),
            other.unwrap_or("no version at all (no version.json)")
        )),
    }

    let bytes = match std::fs::read(&registry_path) {
        Ok(b) => b,
        Err(e) => die(format!("read {}: {e}", registry_path.display())),
    };
    // Only the ids are read: what each block's properties are is the registry's
    // business, and the deriver resolves an id's default variant itself.
    let registry: BTreeMap<String, serde_json::Value> = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => die(format!("parse {}: {e}", registry_path.display())),
    };
    if registry.is_empty() {
        die(format!("{} names no block", registry_path.display()));
    }

    let deriver = Deriver::with_biome(&assets, DEFAULT_BIOME);
    let table = PaletteTable::derive(&deriver, registry.keys().map(String::as_str));

    let json = match serde_json::to_string(&table) {
        Ok(s) => s,
        Err(e) => die(format!("serialise: {e}")),
    };
    // Canonical form, through the repository's one formatter: this file is a
    // document the repository holds, and a generator that wrote a shape the
    // canonical-form gate then refused would leave a person to reformat by hand
    // what a tool produced.
    let json = match delvewright_dsl::fmt::format_text(&json) {
        Ok(s) => s,
        Err(e) => die(format!("canonicalise: {}", e.message)),
    };
    if let Err(e) = std::fs::write(out, &json) {
        die(format!("write {}: {e}", out.display()));
    }

    for (state, why) in &table.unresolved {
        eprintln!("derive-block-appearance: {state}: {why}");
    }
    eprintln!(
        "derive-block-appearance: {} — {} of {} block(s) of {} at Minecraft {}, {} unresolved, \
         {} air-like (absence, never an appearance), derived from {}",
        out.display(),
        table.entries.len(),
        registry.len(),
        registry_path.display(),
        table.mc_version.as_deref().unwrap_or("?"),
        table.unresolved.len(),
        registry.len() - table.entries.len() - table.unresolved.len(),
        jar.display(),
    );
}
