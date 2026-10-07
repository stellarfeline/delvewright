//! **What the declared view distance costs the host, stated by the build**
//! (spec-0091 §4).
//!
//! `world.view_distance` is a capability the campaign declares; the host never
//! caps it (CLAUDE.md, *host hardware never caps a capability*). What the engine
//! owes in return is the cost, computed from the declaration and stated where a
//! host reads it: the heap ceiling in `server/resources.properties`, which the
//! shipped image's entrypoint and the playtest server take as the JVM's
//! `MAX_MEMORY` unless an operator names one, and the binding line of every
//! build.
//!
//! ## The model, and the instrument it is fitted to
//!
//! The cost is the chunks the server keeps for its players. Measured on the
//! pinned server image with the delve's whole party online — four clients, each
//! in its own disc — by `tools/spike-view-distance/run.sh at 80f1a72135e9`, whose record is
//! `crates/delvec/tests/measured/view-distance.json` (the figures below are copied
//! from it and the test `fitted_constants_are_the_rigs` holds them equal):
//!
//! * **Chunks sent per client.** The set the server sends is the chunks whose
//!   offset `(a, b)` from the player's chunk satisfies
//!   `(max(0, |a| − 2))² + (max(0, |b| − 2))² < n²` for the effective view
//!   distance `n` — the pinned server's `ChunkTrackingView` with its edge ring
//!   — so [`sent_chunks`] counts exactly that, and the rig's per-client counts
//!   at 10, 16, 24 and 32 are the equality the test asserts.
//! * **Live heap.** The smallest heap-after-collection over a settled minute,
//!   read from the JVM's own GC log, is linear in the chunks served; the fit is
//!   [`LIVE_BASE_MIB`] + [`LIVE_PER_CHUNK_KIB`] per chunk, with the ocean
//!   backdrop (the heavier of the two a delve ships) as the per-chunk figure.
//! * **The ceiling.** The floor `versions.toml` `[server].heap_max` pins
//!   ([`HEAP_FLOOR_GIB`], held equal to the pin by
//!   `tools/tests/test_server_heap.py`) is what a delve of 84 tiles and 170
//!   horizon templates needed to load at all with the whole party served the
//!   floor's view distance: a cost of the scene, measured once, that no view
//!   distance lowers. A declared distance adds its own cost on top: the fitted
//!   live-heap increment of the declared distance over the floor at the player
//!   cap, times [`HEADROOM`] — a JVM given exactly its live set collects
//!   continuously — rounded up to a whole GiB. At the floor the statement is
//!   the pin.

use delvewright_dsl::viewdistance::{self, CEILING, FLOOR};

/// The delve's player cap (CLAUDE.md: a fixed group of 1–4 players). The cost
/// is stated for the whole party online at once.
pub const PLAYERS: u32 = 4;

/// The heap floor every server this engine starts gets, in GiB: `versions.toml`
/// `[server].heap_max`, which a Python gate holds this constant equal to.
pub const HEAP_FLOOR_GIB: u32 = 4;

/// Fitted live heap with no chunks served to anybody, MiB: the intercept of
/// the least-squares line through the rig's ocean cells (four clients at 10,
/// 16, 24 and 32 chunks).
pub const LIVE_BASE_MIB: f64 = 273.0;
/// Fitted live heap per chunk served, KiB: that line's slope. The largest
/// residual of the four cells is the 24-chunk reading, 20 % over the line;
/// the stated increment carries twice the fit ([`HEADROOM`]).
pub const LIVE_PER_CHUNK_KIB: f64 = 94.3;
/// The stated increment is this many times the fitted live increment.
pub const HEADROOM: f64 = 2.0;

/// How many chunks the pinned server sends one client at view distance `n`.
pub fn sent_chunks(n: u8) -> u32 {
    let n = i64::from(n);
    let reach = n + 2;
    let mut count = 0u32;
    for a in -reach..=reach {
        for b in -reach..=reach {
            let i = (a.abs() - 2).max(0);
            let j = (b.abs() - 2).max(0);
            if i * i + j * j < n * n {
                count += 1;
            }
        }
    }
    count
}

/// The fitted live heap, MiB, with `players` clients each served `n` chunks.
pub fn live_set_mib(n: u8, players: u32) -> f64 {
    LIVE_BASE_MIB + LIVE_PER_CHUNK_KIB * f64::from(sent_chunks(n)) * f64::from(players) / 1024.0
}

/// The heap ceiling the build states for view distance `n`, in whole GiB: the
/// floor plus the declared distance's fitted live increment over the floor at
/// the player cap, with headroom.
pub fn heap_max_gib(n: u8) -> u32 {
    let increment = (live_set_mib(n, PLAYERS) - live_set_mib(FLOOR, PLAYERS)).max(0.0);
    let needed = (f64::from(HEAP_FLOOR_GIB) * 1024.0 + increment * HEADROOM) / 1024.0;
    (needed.ceil() as u32).max(HEAP_FLOOR_GIB)
}

/// The ceiling as itzg's `MAX_MEMORY` spells it (`6G`).
pub fn heap_max_label(n: u8) -> String {
    format!("{}G", heap_max_gib(n))
}

/// What the build examined and stated, printed on every build, zeroes included.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Binding {
    /// The served view distance, in chunks.
    pub chunks: u8,
    /// Whether the campaign declared it (else the floor).
    pub declared: bool,
    /// Showcase cameras whose subject was judged against the served radius.
    pub showcase_cameras: usize,
    /// Cutscene shots whose keyframes were judged against it.
    pub cutscene_shots: usize,
}

impl Binding {
    /// One line, for stderr: the distance, what was judged against it (a
    /// refusal having stopped the build, everything counted is within it), and
    /// the cost stated to the host.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "view distance binding: {n} chunk(s) ({how}) serve {r} blocks in every direction; \
             {c} showcase camera(s) and {s} cutscene shot(s) judged against it, 0 beyond it; \
             stated to the host: heap-max {heap} for {p} players × {k} chunks each \
             (server/resources.properties).",
            n = self.chunks,
            how = if self.declared {
                "declared"
            } else {
                "the engine's floor, undeclared"
            },
            r = viewdistance::served_radius_blocks(self.chunks),
            c = self.showcase_cameras,
            s = self.cutscene_shots,
            heap = heap_max_label(self.chunks),
            p = PLAYERS,
            k = sent_chunks(self.chunks),
        )
    }
}

/// `server/resources.properties`: what this delve asks of its host, in the
/// key=value form the image's entrypoint already reads `server.properties` in.
pub fn resources_properties(namespace: &str, n: u8) -> String {
    format!(
        "# Generated by delvec for campaign {namespace} (spec-0091): what this delve asks of its host.\n\
         # heap-max is the JVM ceiling the shipped image and the playtest server start with unless\n\
         # the operator names one (MEMORY / MAX_MEMORY / --memory); computed for {p} players at\n\
         # view-distance {n} ({k} chunks each), never below the engine's floor.\n\
         heap-max={heap}\n\
         players={p}\n",
        p = PLAYERS,
        k = sent_chunks(n),
        heap = heap_max_label(n),
    )
}

/// The range a declaration may take, for the README.
pub fn range() -> (u8, u8) {
    (FLOOR, CEILING)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chunk count is the pinned server's own set, at every distance the
    /// rig measured (`crates/delvec/tests/measured/view-distance.json`,
    /// `sent_chunks[].chunks` of each cell with the bots declaring 32).
    #[test]
    fn sent_chunks_is_the_rigs_count() {
        let record: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../tests/measured/view-distance.json"
        ))
        .unwrap();
        let mut judged = 0;
        for cell in record["cells"].as_array().unwrap() {
            if cell["client_view_distance"].as_u64() != Some(32) {
                continue;
            }
            let n = cell["view_distance"].as_u64().unwrap() as u8;
            let served = n.min(CEILING);
            for client in cell["sent_chunks"].as_array().unwrap() {
                assert_eq!(
                    client["chunks"].as_u64().unwrap() as u32,
                    sent_chunks(served),
                    "view-distance {n}"
                );
                judged += 1;
            }
        }
        assert!(judged >= 16, "{judged} client readings judged");
    }

    /// The heap never falls below the floor, and grows with the distance.
    #[test]
    fn heap_is_floored_and_monotone() {
        assert_eq!(heap_max_gib(FLOOR), HEAP_FLOOR_GIB);
        let mut last = 0;
        for n in FLOOR..=CEILING {
            let g = heap_max_gib(n);
            assert!(g >= last && g >= HEAP_FLOOR_GIB, "{n}: {g}");
            last = g;
        }
        assert!(heap_max_gib(CEILING) > HEAP_FLOOR_GIB);
    }
}
