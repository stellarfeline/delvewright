//! **The view distance a campaign declares, and what it serves** (spec-0091).
//!
//! `world.view_distance` is the server's `view-distance`, in chunks. The
//! engine's floor is what every proof is written against; a campaign whose far
//! views need more declares more, and the build states what the declaration
//! costs the host. The radius a body is sure to see in every direction is a
//! function of the number — [`served_radius_blocks`] — and it is the one fact
//! every far-view check reads: a sightline, a view, a showcase camera or a
//! cutscene shot aimed past it is refused under `DW0956`, because what it aims
//! at is never sent to the client.
//!
//! The document-tier shapes live here, with no world built: the declared
//! number's range, and the site plan's `sightlines[]` and `views[]`, whose two
//! ends are world coordinates the document already holds. The build-tier
//! shapes — a showcase camera's subject and a cutscene shot's aim — are the
//! compiler's, in `delvec`, against the assembled world.
//!
//! Measured on the pinned server (`tools/spike-view-distance/`, spec-0091 §2):
//! the server sends one more ring of chunks than `view-distance`, and the
//! pinned client draws `min(its render-distance option, the server's
//! view-distance)` chunks around the camera's section, so the radius a player
//! can count on is the declared number of chunks, not the server's margin.

use crate::diagnostic::Diagnostic;
use crate::envelope::Campaign;

/// The engine's floor, in chunks: what every delve is served when it declares
/// nothing, and the least a campaign may declare. 10 chunks reach the far side
/// of the largest scene built to date from any standpoint inside it, and the
/// loop kernel's reach (128 blocks) and the horizon library's vista arithmetic
/// are written against it.
pub const FLOOR: u8 = 10;
/// The most the pinned server serves (vanilla clamps `view-distance` to 32).
pub const CEILING: u8 = 32;
/// Blocks per chunk, on the horizontal axes.
pub const CHUNK_BLOCKS: u32 = 16;

/// The view distance a campaign is served, in chunks: the declared number, or
/// the floor. The one reading of `world.view_distance` every consumer takes.
pub fn chunks(c: &Campaign) -> u8 {
    c.world.content.view_distance.unwrap_or(FLOOR)
}

/// The radius, in blocks, a body standing anywhere is sure to see in every
/// direction at `chunks`: the client draws a square of `chunks` sections around
/// the camera's own section, so along an axis a camera at the far edge of its
/// section sees `16 × chunks` blocks before the first undrawn section, and on
/// every other bearing farther.
pub fn served_radius_blocks(chunks: u8) -> f64 {
    f64::from(u32::from(chunks) * CHUNK_BLOCKS)
}

/// The fewest chunks that serve a view `distance` blocks long: the number the
/// refusal prescribes.
pub fn chunks_for(distance: f64) -> u8 {
    let n = (distance / f64::from(CHUNK_BLOCKS)).ceil();
    if n < f64::from(FLOOR) {
        FLOOR
    } else if n > f64::from(CEILING) {
        CEILING
    } else {
        n as u8
    }
}

/// Euclidean distance between two world points.
pub fn distance(a: [i64; 3], b: [i64; 3]) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    ((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]) as f64).sqrt()
}

/// What the document-tier check examined, stated on every run, zeroes included.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Binding {
    /// The served view distance, in chunks.
    pub chunks: u8,
    /// Whether the campaign declared it (else the floor).
    pub declared: bool,
    /// Site-plan sightlines whose length was judged.
    pub sightlines: usize,
    /// Site-plan views whose eye-to-subject distance was judged.
    pub views: usize,
    /// Of those, the ones beyond the served radius (`DW0956`).
    pub beyond: usize,
}

impl Binding {
    /// One line, for stderr.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "view distance binding: {n} chunk(s) ({how}) serve {r} blocks in every direction; \
             {s} sightline(s) and {v} view(s) judged against it, {b} beyond it.",
            n = self.chunks,
            how = if self.declared {
                "declared"
            } else {
                "the engine's floor, undeclared"
            },
            r = served_radius_blocks(self.chunks),
            s = self.sightlines,
            v = self.views,
            b = self.beyond,
        )
    }
}

/// The document-tier shapes of `DW0956`, and the binding they state.
///
/// A declared number outside `FLOOR..=CEILING` is refused on the field, and the
/// site plan's lines are then judged against the floor, so one bad number does
/// not hide every far line behind it. A sightline's or a view's length is the
/// straight distance between its two ends; the message names the number of
/// chunks that would serve it, and that number is one edit to the one field.
pub fn checks(c: &Campaign, d: &mut Vec<Diagnostic>) -> Binding {
    let declared = c.world.content.view_distance;
    let mut b = Binding {
        chunks: FLOOR,
        declared: declared.is_some(),
        ..Binding::default()
    };
    match declared {
        Some(n) if (FLOOR..=CEILING).contains(&n) => b.chunks = n,
        Some(n) => d.push(Diagnostic::error(
            crate::diagnostic::codes::VIEW_BEYOND_SERVED,
            "world",
            "/content/view_distance".to_string(),
            format!(
                "`view_distance` = {n} chunks cannot be served: the pinned server serves at most \
                 {CEILING} chunks, and the engine's floor is {FLOOR} (every proof is written \
                 against it, and a smaller number only takes views away). Declare a value in \
                 {FLOOR}..={CEILING}, or drop the field for the floor."
            ),
        )),
        None => {}
    }
    let radius = served_radius_blocks(b.chunks);
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return b;
    };
    for (i, s) in plan.sightlines.iter().enumerate() {
        b.sightlines += 1;
        let len = distance(s.from, s.to);
        if len > radius {
            b.beyond += 1;
            d.push(Diagnostic::error(
                crate::diagnostic::codes::VIEW_BEYOND_SERVED,
                "site-plan",
                format!("/content/sightlines/{i}"),
                beyond_message(
                    &format!("the vista `{}`", s.edge),
                    len,
                    b.chunks,
                    b.declared,
                ),
            ));
        }
    }
    for (i, v) in plan.views.iter().enumerate() {
        b.views += 1;
        let len = distance(v.eye, v.look_at);
        if len > radius {
            b.beyond += 1;
            d.push(Diagnostic::error(
                crate::diagnostic::codes::VIEW_BEYOND_SERVED,
                "site-plan",
                format!("/content/views/{i}"),
                beyond_message(&format!("the view `{}`", v.id), len, b.chunks, b.declared),
            ));
        }
    }
    b
}

/// The one sentence every far-view refusal says: what is aimed, how far, what
/// is served, and the declaration that would serve it.
pub fn beyond_message(what: &str, length: f64, chunks: u8, declared: bool) -> String {
    let radius = served_radius_blocks(chunks);
    let need = chunks_for(length);
    let how = if declared {
        format!("the declared `world.view_distance` of {chunks} chunks")
    } else {
        format!("the engine's floor of {chunks} chunks (nothing declared)")
    };
    let remedy = if need > chunks {
        format!(
            "Declare `world.view_distance: {need}` (the fewest chunks that serve {length:.0} \
             blocks), or bring the two ends within {radius:.0} blocks of each other."
        )
    } else {
        format!(
            "Bring the two ends within {radius:.0} blocks of each other: {length:.0} blocks is \
             past what the pinned server can serve at any view distance ({CEILING} chunks)."
        )
    };
    format!(
        "{what} reaches {length:.1} blocks, and {how} serves {radius:.0} blocks in every \
         direction — what it looks at is never sent to the client, so the view cannot render. \
         {remedy}"
    )
}
