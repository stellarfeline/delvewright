//! Bodies: where a body stands — the one resolution rule for a summoned body's cell.

use super::*;

impl<'a> Plan<'a> {
    /// **The cell a body is summoned onto** — the one resolution rule for a
    /// [`delvewright_dsl::BodyRef`] of either class: its mark's anchor, resolved,
    /// plus the mark's offset (spec-0066).
    ///
    /// A body that declares an area is resolved in that area's table first and
    /// falls back to any placed piece; a body that declares none (an actor) is
    /// resolved across every placed piece, exactly as an `open-gate` or
    /// `move-actor` destination is. Which of the two applies is
    /// [`delvewright_dsl::BodyRef::area`]'s answer, so the rule is stated once
    /// instead of once per consumer.
    ///
    /// `None` when nothing provides the anchor. Every consumer skips such a
    /// body rather than reporting against it — `DW0325`/`DW0345`/`DW0360` own
    /// dangling references, and a geometry or occupancy finding for one would
    /// send the author to the wrong line.
    pub fn body_point(&self, body: delvewright_dsl::BodyRef<'_>) -> Option<[i32; 3]> {
        let mark = body.mark();
        self.body_anchor_site(body)
            .map(|(_, anchor_cell)| mark.cell(anchor_cell))
    }

    /// Where a body's mark's ANCHOR resolves — the area that answered and the
    /// anchor's own cell, before the offset is added. The same rule as
    /// [`Self::body_point`] (area-scoped for a body that declares an area, across
    /// every placed piece for one that does not); the area is what `DW0897` asks
    /// [`Self::piece_bounds`] about.
    pub fn body_anchor_site(
        &self,
        body: delvewright_dsl::BodyRef<'_>,
    ) -> Option<(String, [i32; 3])> {
        let mark = body.mark();
        let anchor = mark.anchor.as_str();
        if let Some(area) = body.area()
            && let Some(p) = self.point(area.as_str(), anchor)
        {
            return Some((area.as_str().to_string(), p));
        }
        self.point_any_site(anchor)
    }
}

/// **The scope a body's anchor reference resolves in.**
///
/// Two modes because there are two questions about one object, and
/// [`body_station`] answers both so that no consumer keeps a private copy of
/// either rule. The consumers used to hold one each — the emitter's world-init
/// summon read `(npc.area, npc.anchor)` strictly, the cast ledger read the
/// beat's area first — and where one anchor name was provided by both areas the
/// two answered about buildings 256 blocks apart, in the same build, with
/// nothing comparing them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyScope<'a> {
    /// **Where an NPC is declared to stand**: its stage-2 `area` and `anchor`.
    /// Strict — the declaration IS the statement of which building the body is
    /// in, so there is nothing for a wider search to add.
    Declared {
        /// The NPC's own `area`.
        area: &'a str,
    },
    /// **Where a cast ledger stations a body for one beat.** The beat's own area
    /// owns the name, then the NPC's home, then an unambiguous crossing.
    Beat {
        /// The area the beat plays in (the quest plan's `area`).
        beat: &'a str,
        /// The NPC's own `area`.
        home: &'a str,
    },
}

/// Where a body stands, as [`body_station`] resolved it.
pub enum BodyStation<'a> {
    /// The area it resolved in, and the anchor itself (cell and facing).
    At {
        /// The area the name resolved in.
        area: String,
        /// The resolved anchor.
        anchor: &'a ResolvedAnchor,
    },
    /// More than one area provides the name and the scope does not settle it.
    /// The areas, in `BTreeMap` order, for the diagnostic.
    Ambiguous(Vec<String>),
    /// No placed piece provides the name.
    Missing,
}

impl BodyStation<'_> {
    /// The cell, for a caller that only wants a position.
    pub fn pos(&self) -> Option<[i32; 3]> {
        match self {
            BodyStation::At { anchor, .. } => Some(match anchor {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            }),
            _ => None,
        }
    }

    /// `(area, cell)` — the pair that identifies a place in the world. Two names
    /// being equal says nothing; this pair being equal is what "the same place"
    /// means, and it is what `DW0461` compares.
    pub fn place(&self) -> Option<(&str, [i32; 3])> {
        match self {
            BodyStation::At { area, .. } => self.pos().map(|p| (area.as_str(), p)),
            _ => None,
        }
    }
}

/// **The one authority for where a body stands.**
///
/// Asked by the emitter's world-init summon ([`BodyScope::Declared`]), by the
/// cast ledger's per-beat station ([`BodyScope::Beat`]) and by `DW0461`, which
/// compares the two. A name is an identity within an area and nowhere wider, so
/// every answer carries the area it resolved in — the half a by-name lookup
/// throws away.
///
/// [`BodyScope::Beat`]'s order — beat's area, then home, then an unambiguous
/// crossing — is not a widening of the declared scope but the ledger's own rule:
/// a cast row says *in this quest, this body stands here*, and the quest has an
/// area. Measured on a campaign of eight zones, one NPC is declared at
/// `anchor/lampman` in his home zone and cast at `anchor/lampman` in seven
/// others, and **two** zones provide that name; reading home first made every
/// beat resolve to the home cell, so the escort's destination was the cell he
/// already stood on. Home stays as the second step because a `move-npc` may
/// station a body in an area the NPC was never declared in.
///
/// Where the beat's area and the home area BOTH provide the name they are
/// different places, and choosing between them is not this function's to make
/// silently: the choice stands only while the body is already there. `DW0461`'s
/// place arm compares this answer against the area the effect history left the
/// body in and refuses the pair that disagrees, so a row can win the NAME but
/// never move a BODY.
pub fn body_station<'a>(
    anchors: &'a AnchorTable,
    scope: BodyScope<'_>,
    anchor: &str,
) -> BodyStation<'a> {
    let strict = |area: &str| -> Option<BodyStation<'a>> {
        anchors
            .get(&(area.to_string(), anchor.to_string()))
            .map(|r| BodyStation::At {
                area: area.to_string(),
                anchor: r,
            })
    };
    match scope {
        BodyScope::Declared { area } => strict(area).unwrap_or(BodyStation::Missing),
        BodyScope::Beat { beat, home } => {
            for area in [beat, home] {
                if let Some(hit) = strict(area) {
                    return hit;
                }
            }
            // Neither scope provides it: a genuine crossing, allowed while it is
            // unambiguous, through the anchor table's own authority.
            match anchors.resolve(AnchorScope::Global, anchor) {
                AnchorHit::Found { area, anchor: r } => BodyStation::At {
                    area: area.to_string(),
                    anchor: r,
                },
                AnchorHit::Ambiguous(areas) => {
                    BodyStation::Ambiguous(areas.into_iter().map(str::to_string).collect())
                }
                AnchorHit::Missing => BodyStation::Missing,
            }
        }
    }
}
