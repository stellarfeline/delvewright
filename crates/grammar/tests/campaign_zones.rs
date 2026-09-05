//! **The campaign corpus**, judged from the content repo's files against an
//! inventory this repo enumerates.
//!
//! Two corpora of grammar programs exist and they have different owners, which
//! is why they are two files. `zones.rs` sweeps `library::bell::*` — Rust
//! functions in this tree, so its size is a fact of this tree and a zero there
//! is a hard red with nowhere to hide. This file sweeps the CAMPAIGN corpus:
//! `campaigns/<c>/design/programs/*.json` in the content repo, which ADR-0018
//! makes the artifact of record for a zone's geometry.
//!
//! The campaign corpus is not this repo's to produce. An in-progress campaign
//! lives on its own content-repo development branch and reaches content `main`
//! only after the owner has played it, and CI checks the content out at
//! `versions.toml` `[content].sha`. So the pinned tree can legitimately carry no
//! zone program at all, and it does today.
//!
//! That makes "the sweep found nothing, so it passes" an opt-out the defect
//! itself supplies — deleting every zone program of every campaign produces
//! exactly that state. The demand here is a different one:
//! `.github/content-zone-corpus.json` **enumerates** the campaigns the pin
//! carries and how many zone programs each declares, and every number in it is
//! checked against the tree. A campaign that loses its programs reds on a count;
//! a pin that genuinely carries none passes with its inventory printed. Which
//! list an entry belongs to is decided by the pinned tree rather than by the
//! author, so the two lists are not a choice between a strong obligation and a
//! weak one.
//!
//! The content checkout is read through the `campaigns/` symlink the repo uses
//! everywhere else (CI materialises it with `./.github/actions/checkout-content`
//! at the pinned SHA). A missing checkout is a **failure**, never a skip.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use delvewright_grammar::ir::Program;
use delvewright_grammar::{Box3, ExpandOptions, expand};

// ---------------------------------------------------------------------------
// the enumerated inventory
// ---------------------------------------------------------------------------

/// One campaign the pinned content carries.
#[derive(serde::Deserialize)]
struct OnPin {
    campaign: String,
    zone_programs: usize,
    #[allow(dead_code)]
    note: String,
}

/// One campaign that is known to own zone programs somewhere this repo cannot
/// see. A queue entry, not an exemption: it asserts ABSENCE from the pinned
/// tree, and it names the branch where the obligation is discharged today.
#[derive(serde::Deserialize)]
struct OffPin {
    campaign: String,
    zone_programs: usize,
    branch: String,
    #[allow(dead_code)]
    note: String,
}

#[derive(serde::Deserialize)]
struct Record {
    content_sha: String,
    on_pin: Vec<OnPin>,
    off_pin: Vec<OffPin>,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn record_path() -> PathBuf {
    repo_root().join(".github/content-zone-corpus.json")
}

fn record() -> Record {
    let path = record_path();
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} — the campaign corpus is judged against this \
             enumeration, so an absent one is a failure and never a pass",
            path.display()
        )
    });
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// `versions.toml` `[content].sha` — the commit CI checks the content repo out
/// at. A plain line scan of the one key we need, the same way the compiler reads
/// it for `manifest.json` (`crates/delvec/src/main.rs`): no TOML dependency,
/// and the value read is the one pinned in the repo rather than live git state.
fn pinned_content_sha() -> String {
    let path = repo_root().join("versions.toml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut in_content = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            in_content = line == "[content]";
            continue;
        }
        if in_content
            && let Some(rest) = line.strip_prefix("sha")
            && let Some(rest) = rest.trim_start().strip_prefix('=')
        {
            let val = rest.split('#').next().unwrap_or(rest).trim();
            let val = val.trim_matches('"');
            if !val.is_empty() {
                return val.to_string();
            }
        }
    }
    panic!("{}: no [content].sha", path.display())
}

// ---------------------------------------------------------------------------
// the tree
// ---------------------------------------------------------------------------

fn content_campaigns() -> PathBuf {
    let root = repo_root().join("campaigns/campaigns");
    assert!(
        root.is_dir(),
        "{} is not a directory — the content checkout is missing. This test is about the \
         campaign's own files, so an absent checkout is a failure and never a skip.",
        root.display()
    );
    root
}

/// Every campaign-shaped directory in the content checkout.
///
/// A campaign is a directory holding a `world.json` (a campaign whose world is
/// authored) or a `design/` (one whose world is not yet). Anything else under
/// `campaigns/` is not a campaign — a working checkout accumulates build-output
/// directories and stray design pages there, and calling one a campaign would red
/// every local run for a reason with nothing to do with content.
///
/// **The discriminator cannot hide a zone program**, which is the property that
/// matters: zone programs live at `design/programs/`, so a directory holding one
/// necessarily holds `design/` and is necessarily in this set. And it is safe in
/// the other direction too — emptying a campaign of both takes it out of the set,
/// which reds as a missing `on_pin` entry rather than passing as an absence.
fn campaigns_on_disk() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for entry in std::fs::read_dir(content_campaigns()).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_dir() {
            continue;
        }
        if path.join("world.json").is_file() || path.join("design").is_dir() {
            out.insert(path.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    out
}

fn programs_dir(campaign: &str) -> PathBuf {
    content_campaigns().join(campaign).join("design/programs")
}

/// The zone program FILES a campaign holds, by name, manifest excluded.
fn program_files_on_disk(campaign: &str) -> Vec<String> {
    let dir = programs_dir(campaign);
    if !dir.is_dir() {
        return Vec::new();
    }
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".json") && n != "zones.json")
        .collect();
    out.sort();
    out
}

/// One zone as its campaign declares it in `design/programs/zones.json`.
#[derive(serde::Deserialize)]
struct ZoneEntry {
    id: String,
    program: String,
    region: [u32; 3],
    seed: u64,
}

#[derive(serde::Deserialize)]
struct ZoneManifest {
    zones: Vec<ZoneEntry>,
}

fn manifest_of(campaign: &str) -> Option<ZoneManifest> {
    let path = programs_dir(campaign).join("zones.json");
    if !path.is_file() {
        return None;
    }
    Some(
        serde_json::from_slice(&std::fs::read(&path).unwrap())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
}

/// The number of zone programs the pinned content is expected to carry, summed
/// over the enumeration. This is the corpus size every sweep below binds to, and
/// it comes from the record rather than from the tree — which is the whole point:
/// a tree that disagrees with it is a finding, not a new expectation.
fn expected_zone_programs() -> usize {
    record().on_pin.iter().map(|c| c.zone_programs).sum()
}

/// Every declared zone of every campaign the pin carries:
/// `(label, program, region, seed)`.
fn declared_zones() -> Vec<(String, Program, [u32; 3], u64)> {
    let mut out = Vec::new();
    for campaign in campaigns_on_disk() {
        let Some(manifest) = manifest_of(&campaign) else {
            continue;
        };
        for zone in manifest.zones {
            let path = programs_dir(&campaign).join(&zone.program);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| {
                panic!(
                    "{campaign}: zones.json declares zone {:?} as {:?}, and {}: {e}",
                    zone.id,
                    zone.program,
                    path.display()
                )
            });
            let program: Program = serde_json::from_slice(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.push((
                format!("{campaign}/{}", zone.id),
                program,
                zone.region,
                zone.seed,
            ));
        }
    }
    out
}

/// Every sweep in this file prints what it examined and which corpus that
/// examination belongs to, so a zero is named rather than inferred from silence.
fn state_binding(gate: &str, bound: usize) {
    let expected = expected_zone_programs();
    println!(
        "  {gate:<48} campaign corpus: bound {bound} of {expected} zone program(s) at content \
         pin {}{}",
        pinned_content_sha(),
        if expected == 0 {
            " — ZERO BINDING: the pinned content declares no zone program. That is a \
             fact of the pin, enumerated per campaign in .github/content-zone-corpus.json \
             and checked against the tree there; it is not this sweep passing on nothing."
        } else {
            ""
        }
    );
}

// ---------------------------------------------------------------------------
// the record binds to the pin
// ---------------------------------------------------------------------------

/// **The enumeration is about the commit CI actually builds.**
///
/// This is what binds the record to the event it guards. A content re-pin cannot
/// land without the inventory being restated at the new pin, and a restated
/// inventory is checked against the tree by the tests below — so writing a number
/// the tree disagrees with is a red rather than a shortcut. Without this, the
/// record would describe a commit nobody builds any more and every count below
/// would be measured against the wrong corpus while staying green.
///
/// It binds with no content checkout at all: two files in this repo.
#[test]
fn the_record_names_the_pin_versions_toml_declares() {
    let record = record();
    let pin = pinned_content_sha();
    assert_eq!(
        record.content_sha,
        pin,
        "{} enumerates the content at {}, but versions.toml pins {pin}. The pin moved and the \
         zone inventory did not: restate it at the new pin — every campaign the pin carries, \
         with the number of zone programs it declares.",
        record_path().display(),
        record.content_sha,
    );
}

/// A campaign is named once across the whole record.
///
/// The two lists are an obligation and a queue, and an entry in both would make
/// the effective obligation their disjunction — satisfied by the weaker half.
#[test]
fn no_campaign_is_named_twice_in_the_record() {
    let record = record();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for c in &record.on_pin {
        *seen.entry(c.campaign.clone()).or_default() += 1;
    }
    for c in &record.off_pin {
        *seen.entry(c.campaign.clone()).or_default() += 1;
    }
    let dupes: Vec<&String> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, _)| k)
        .collect();
    assert!(dupes.is_empty(), "named more than once: {dupes:?}");
    assert!(
        !record.on_pin.is_empty(),
        "{} names no campaign at all. The pinned content carries campaigns; an inventory that \
         lists none of them cannot disagree with anything.",
        record_path().display()
    );
    for c in &record.off_pin {
        assert!(
            c.zone_programs > 0,
            "off_pin entry {:?} owes no zone programs, so it records nothing — an off_pin \
             entry exists to say what a campaign owes and where that is proved today",
            c.campaign
        );
        assert!(
            !c.branch.trim().is_empty(),
            "off_pin entry {:?} names no branch, so nothing says where its obligation is \
             discharged",
            c.campaign
        );
    }
}

/// **Which list an entry belongs to is decided by the pinned tree.**
///
/// Three directions, and the third is the one that keeps the enumeration from
/// becoming a choice the author makes:
///
/// * every campaign the pin carries is named — an unnamed one is a campaign
///   nothing says whether it owes zone programs, and it would sweep as zero;
/// * every `on_pin` entry is present — a campaign emptied of both `world.json`
///   and `design/` disappears from the tree, and this is what notices;
/// * every `off_pin` entry is ABSENT — an entry that has merged to the pin must
///   move across and have its count checked, so an author cannot park a campaign
///   in the queue to avoid the count.
#[test]
fn the_pinned_tree_decides_which_list_each_campaign_is_in() {
    let record = record();
    let disk = campaigns_on_disk();
    let on_pin: BTreeSet<String> = record.on_pin.iter().map(|c| c.campaign.clone()).collect();

    let unnamed: Vec<&String> = disk.difference(&on_pin).collect();
    assert!(
        unnamed.is_empty(),
        "the content checkout carries campaign(s) the inventory does not name: {unnamed:?}. \
         Add each to on_pin in {} with the number of zone programs it declares — an unnamed \
         campaign sweeps as zero and says nothing.",
        record_path().display()
    );
    let missing: Vec<&String> = on_pin.difference(&disk).collect();
    assert!(
        missing.is_empty(),
        "the inventory names campaign(s) the content checkout does not carry: {missing:?}. \
         Either the checkout is not at content pin {} (CI checks it out there; a local \
         `campaigns/` symlink points wherever it points), or the campaign left the pin and \
         its entry must move.",
        pinned_content_sha()
    );
    for c in &record.off_pin {
        assert!(
            !disk.contains(&c.campaign),
            "{:?} is recorded off_pin on branch {:?}, but the pinned tree carries it. It has \
             landed: move it to on_pin, where its {} zone program(s) are counted against the \
             tree.",
            c.campaign,
            c.branch,
            c.zone_programs
        );
    }
    println!(
        "  content pin {}: {} campaign(s) on the pin {:?}; {} queued off-pin {:?}",
        record.content_sha,
        disk.len(),
        disk,
        record.off_pin.len(),
        record
            .off_pin
            .iter()
            .map(|c| format!("{} ({} zones, {})", c.campaign, c.zone_programs, c.branch))
            .collect::<Vec<_>>()
    );
}

/// **Every enumerated count is checked against the files.**
///
/// This is the assertion the defect cannot satisfy. A campaign whose zone
/// programs are deleted still has a number written beside its name here, and the
/// number no longer matches the directory. A campaign that gains zone programs
/// nobody enumerated fails the same way from the other side. The only way to
/// make a count green is for it to be true.
#[test]
fn every_enumerated_campaign_declares_exactly_the_zone_programs_recorded() {
    let record = record();
    let mut total = 0usize;
    for c in &record.on_pin {
        let files = program_files_on_disk(&c.campaign);
        assert_eq!(
            files.len(),
            c.zone_programs,
            "{}: the inventory records {} zone program(s); {} holds {}: {files:?}",
            c.campaign,
            c.zone_programs,
            programs_dir(&c.campaign).display(),
            files.len()
        );
        match manifest_of(&c.campaign) {
            Some(m) => assert_eq!(
                m.zones.len(),
                c.zone_programs,
                "{}: the inventory records {} zone(s) and zones.json declares {}",
                c.campaign,
                c.zone_programs,
                m.zones.len()
            ),
            None => assert_eq!(
                c.zone_programs, 0,
                "{}: the inventory records {} zone program(s) and there is no zones.json, so \
                 nothing states the region and seed they are built at",
                c.campaign, c.zone_programs
            ),
        }
        total += files.len();
    }
    state_binding("enumerated-counts", total);
    assert_eq!(total, expected_zone_programs());
}

// ---------------------------------------------------------------------------
// the sweeps over whatever the pin carries
// ---------------------------------------------------------------------------

/// The corpus is exactly the size the enumeration says.
///
/// Every sweep below iterates this list, so a list that silently changed size
/// would turn each of them into a green that examined something other than what
/// this repo believes it is examining.
#[test]
fn the_declared_zone_corpus_is_the_size_the_record_names() {
    let zones = declared_zones();
    state_binding("corpus-size", zones.len());
    assert_eq!(
        zones.len(),
        expected_zone_programs(),
        "the content checkout declares {} zone(s) and .github/content-zone-corpus.json \
         enumerates {}: {:?}",
        zones.len(),
        expected_zone_programs(),
        zones.iter().map(|z| &z.0).collect::<Vec<_>>()
    );
}

/// Every declared zone is a structurally valid program: every `call` resolves,
/// every role and param it names exists. Found here rather than at expansion,
/// with no region and no seed involved.
#[test]
fn every_declared_zone_is_structurally_valid() {
    let zones = declared_zones();
    state_binding("structurally-valid", zones.len());
    for (label, program, _, _) in &zones {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{label}: {e}"));
    }
}

/// **ADR-0006 over the campaign's own files: same program, same region, same
/// seed, byte-identical model.**
///
/// The second of the two methods this is proved by, and deliberately a different
/// code path from the first. The first runs `delvec grammar expand` twice as
/// separate processes and compares the CONTENT hash of every file each wrote —
/// the `.nbt` through the gzip writer, the metadata, the gate report. This one
/// stays in process and compares `VoxelModel::canonical_bytes`, so it touches no
/// NBT writer, no compressor and no filesystem. A nondeterminism that both miss
/// would have to survive two encodings.
///
/// The negative half is asserted too: a different seed must produce different
/// bytes for at least one zone. Without it, "identical" would also be satisfied
/// by an expander that ignored the program. That half is a claim about a
/// non-empty corpus, so over an empty one it is stated as the zero it is rather
/// than asserted about nothing — and the corpus size is itself judged above.
#[test]
fn every_declared_zone_expands_byte_identically_twice() {
    let zones = declared_zones();
    state_binding("byte-identical-twice", zones.len());
    let mut seed_sensitive = 0usize;
    for (label, program, region, seed) in &zones {
        let box3 = Box3::at_origin(*region);
        let a = expand(program, box3, &ExpandOptions::seeded(*seed))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        let b = expand(program, box3, &ExpandOptions::seeded(*seed))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "{label} is not deterministic at seed {seed}"
        );
        assert_eq!(a.anchors, b.anchors, "{label}: anchors moved");

        let other = expand(program, box3, &ExpandOptions::seeded(seed.wrapping_add(1)))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        if other.model.canonical_bytes() != a.model.canonical_bytes() {
            seed_sensitive += 1;
        }
    }
    state_binding("seed-sensitivity", seed_sensitive);
    if !zones.is_empty() {
        assert!(
            seed_sensitive > 0,
            "no declared zone changed when its seed did, so the equality above would hold \
             for an expander that read neither the seed nor the program"
        );
    }
}

/// The manifest and the directory agree in both directions.
///
/// The direction that rots is the second: a zone program dropped into
/// `design/programs/` with no manifest entry is a program nothing expands and
/// nothing checks, and it looks exactly like a program that is fine. The CLI
/// reds on it too (`delvec grammar audit`); this is the same claim inside
/// `cargo test`, because the two run in different jobs.
#[test]
fn every_program_file_is_declared_and_every_declaration_has_a_file() {
    let mut checked = 0usize;
    for campaign in campaigns_on_disk() {
        let on_disk = program_files_on_disk(&campaign);
        let dir = programs_dir(&campaign);
        let manifest = manifest_of(&campaign);
        assert!(
            manifest.is_some() || on_disk.is_empty(),
            "{} holds {} zone program(s) and no zones.json, so nothing states the region \
             and seed they are built at",
            dir.display(),
            on_disk.len()
        );
        let Some(manifest) = manifest else { continue };
        let mut declared: Vec<String> = manifest.zones.iter().map(|z| z.program.clone()).collect();
        declared.sort();
        assert_eq!(on_disk, declared, "{}", dir.display());
        for zone in &manifest.zones {
            assert!(
                dir.join(&zone.program).is_file(),
                "{}: zone {:?} names {:?}, which is not there",
                dir.display(),
                zone.id,
                zone.program
            );
        }
        checked += on_disk.len();
    }
    state_binding("manifest-bijection", checked);
    assert_eq!(checked, expected_zone_programs());
}

/// A zone's id is unique inside its campaign — the audit reports by that id, and
/// two zones sharing one would make an exclusion record ambiguous.
#[test]
fn every_declared_zone_id_is_unique() {
    let zones = declared_zones();
    state_binding("unique-ids", zones.len());
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (label, _, _, _) in zones {
        *seen.entry(label).or_default() += 1;
    }
    let dupes: Vec<&String> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, _)| k)
        .collect();
    assert!(dupes.is_empty(), "duplicate zone ids: {dupes:?}");
}
