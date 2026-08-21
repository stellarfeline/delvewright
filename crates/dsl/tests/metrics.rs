//! The metrics standard's version means something, and its two diagnostics fire
//! where they say they do (spec-0049 §2).

use delvewright_dsl::diagnostic::Severity;
use delvewright_dsl::metrics::{
    METRICS_VERSION, MetricKind, MetricValue, Metrics, Provenance, Reads, export,
};

/// The exported table's canonical bytes at [`METRICS_VERSION`].
///
/// A metrics version is not a fence — nothing grandfathers against it and no
/// document declares one — so `tools/check-version-ledger-uniqueness.py` has
/// nothing to compare and is deliberately not extended to cover it. What a
/// consumer that pins the version is actually pinning is the VALUES, and this is
/// the gate that makes the pin worth having: change any number, any note, any
/// calibration flag, and this reds until the version moves with it.
///
/// The remedy is reachable, which is the pair test `CLAUDE.md` asks for before a
/// gate that names one is trusted: bump [`METRICS_VERSION`] and update this
/// string, both in the commit that moved the table. Nothing else in the tree
/// refuses either edit — in particular the emission baseline never sees this,
/// because no build reads the building half at this version.
const CANONICAL_DIGEST: &str = "a122a79b00cacb910d96858cad9696f8e709f70bef5e1350e37d26fbcdf6d6f8";

/// A tiny SHA-256, so the digest above needs no dependency this crate does not
/// already have. `delvewright-dsl` ships serde and nothing else, and adding a
/// hash crate to the DSL's dependency tree to check its own test fixture would
/// be a real cost for a fixed 64-line function.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(v);
        }
    }
    h.iter().map(|w| format!("{w:08x}")).collect()
}

fn canonical_bytes() -> Vec<u8> {
    let table = Metrics::table();
    let mut s = serde_json::to_string_pretty(&export(&table)).expect("the table serializes");
    s.push('\n');
    s.into_bytes()
}

#[test]
fn the_version_cannot_stand_still_while_the_table_moves() {
    let got = sha256_hex(&canonical_bytes());
    assert_eq!(
        got, CANONICAL_DIGEST,
        "the metrics table's exported bytes changed. That is a new table, so bump \
         `dsl::metrics::METRICS_VERSION` (currently {METRICS_VERSION}) and set \
         CANONICAL_DIGEST to {got} in the same commit — a consumer that pins a metrics \
         version is pinning these values, and a version that stands still while they \
         move is the drift the pin was bought to prevent."
    );
}

#[test]
fn the_export_is_byte_identical_across_runs() {
    // ADR-0006: no clock, no environment, no hash order. Two builds of the table
    // in one process is the cheap half of that; the digest above is the half
    // that survives a restart.
    assert_eq!(canonical_bytes(), canonical_bytes());
}

/// `DW0812` is raised at the ONE path from a name to an entry, so a name the
/// table does not define cannot reach a check downstream.
#[test]
fn dw0812_refuses_a_name_the_table_does_not_define() {
    let m = Metrics::table();
    for (kind, bad) in [
        (MetricKind::SizeClass, "cathedral"),
        (MetricKind::Opening, "portcullis"),
        (MetricKind::Pitch, "ladder"),
        (MetricKind::Storey, "mezzanine"),
    ] {
        let err = m
            .resolve(kind, bad)
            .expect_err("the table defines no such entry");
        let d = err.diagnostic("site-plan", "/seams/0/opening");
        assert_eq!(d.code, "DW0812");
        assert_eq!(d.severity, Severity::Error);
        assert!(
            d.message.contains(bad),
            "the refusal names what was written"
        );
        assert!(
            !err.defined.is_empty(),
            "the refusal must name the defined set, or the author is sent to read the compiler"
        );
        for name in &err.defined {
            assert!(
                d.message.contains(name),
                "`{name}` is missing from the refusal"
            );
        }
    }
}

/// `DW0813` is bound to the READ, not to a call site somebody has to remember.
#[test]
fn dw0813_names_exactly_the_seeds_a_verdict_read() {
    let m = Metrics::table();
    let mut reads = Reads::new();

    // One read, of one uncalibrated entry.
    let entry = m
        .resolve(MetricKind::Opening, "arch")
        .expect("`arch` exists");
    let _ = entry.value(&mut reads);

    let binding = reads.binding();
    assert_eq!(binding.read, 1);
    assert_eq!(binding.provisional, 1);

    let d = m
        .notice(&reads, "site-plan")
        .expect("a verdict that read a seed owes the notice");
    assert_eq!(d.code, "DW0813");
    assert_eq!(d.severity, Severity::Warning, "a seed is still a number");
    assert!(d.message.contains("opening.arch"));
    assert!(
        !d.message.contains("opening.door"),
        "the notice names what was read, never the whole table — a line naming \
         everything names nothing"
    );
}

/// The self-check is what gives `DW0813` a live binding at this version, and a
/// self-check that examined nothing would be the vacuity the code exists to make
/// visible rather than the pass it looks like.
#[test]
fn the_self_check_binds_to_something_and_reports_what() {
    let m = Metrics::table();
    let check = m.self_check();
    assert!(check.failures.is_empty(), "{:?}", check.failures);
    assert!(check.binding.invariants > 0);
    assert_eq!(
        check.binding.entries,
        m.building.len(),
        "the binding count must name the whole building half, not the subset that passed"
    );
    assert_eq!(
        check.binding.reads.read,
        m.building.len(),
        "every building entry is examined, so a seed cannot hide from the notice by \
         being one nothing reads"
    );
}

/// Provenance is honest per row: a chosen number never claims to be a measured
/// one, and a player metric never claims to be provisional.
#[test]
fn no_entry_dresses_a_seed_as_a_measurement() {
    let m = Metrics::table();
    let mut reads = Reads::new();

    for (key, e) in &m.player {
        assert_ne!(
            e.provenance,
            Provenance::Provisional,
            "player metric `{key}`"
        );
        assert!(e.note.len() > 40, "player metric `{key}` states no source");
    }

    // Every entry the existing prefab library or vanilla block geometry already
    // fixes is marked `derived` and says what it was derived FROM; everything
    // else is `provisional` and says the gym decides it.
    for (key, e) in &m.building {
        assert!(
            matches!(e.provenance, Provenance::Derived | Provenance::Provisional),
            "building metric `{key}` claims to be a measured fact of the game, but a \
             standard this project fixes is not one"
        );
        assert!(!e.calibrated, "building metric `{key}`");
        assert!(
            e.note.len() > 40,
            "building metric `{key}` states no reasoning"
        );
        // The value is reachable only through a recording read.
        let _ = e.value(&mut reads);
    }
    assert_eq!(reads.binding().provisional, m.building.len());
}

/// The three-by-three passage is the one building entry taken from something
/// that already exists, and it says so — the correction that matters more than
/// the number.
#[test]
fn the_standard_passage_is_the_socket_convention_the_library_already_uses() {
    let m = Metrics::table();
    let mut reads = Reads::new();
    let e = m
        .resolve(MetricKind::Opening, "passage")
        .expect("`passage` exists");
    assert_eq!(e.provenance, Provenance::Derived);
    assert_eq!(
        *e.value(&mut reads),
        MetricValue::Opening(delvewright_dsl::metrics::Opening {
            width: 3,
            height: 3
        })
    );
}
