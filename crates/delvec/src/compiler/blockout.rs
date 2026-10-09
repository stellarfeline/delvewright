//! **The blockout: derived, authored by no one** (spec-0049 §5) — pipeline
//! stage 5.
//!
//! The whole map's mass, as a pure function of the **site plan, the layout
//! graph**, the metrics table and the engine. There is no blockout document to
//! write, no schema, and no file an author can edit *as a blockout*: the only
//! path to blockout bytes is [`derive`], whose input is a validated site plan.
//! That is what makes *blockout before site plan* **uncompilable** rather than
//! merely forbidden (spec-0049 §7.2) — there is nothing to author early.
//!
//! Both authored documents are named because both reach the bytes: a seam is
//! cut to air or filled with the bar by its edge's `class`, and a sky-open box
//! takes its headroom from its node's `size_class`.
//!
//! # Where it enters the build
//!
//! [`crate::compiler::plan::Plan::build`] calls [`derive`] once, for every campaign that
//! carries a site plan, and pushes the result as an ordinary
//! [`AreaPlacement`](crate::compiler::plan::AreaPlacement). **That is the tooth.** There is
//! no flag, no subcommand and no second entry point: a `Plan` is the only thing
//! `build`, `analyze`, `snapshot`, `viewer`, `blocking-chart` and `edit` can
//! reach a world through, and a `Plan` built from a site-plan campaign has the
//! blockout in it. Someone who "forgot" to derive would have to have built a
//! `Plan` some other way, and there is no other way.
//!
//! Everything downstream is inherited unchanged, which is the point of entering
//! there rather than anywhere else: gravity settling, the nav occupancy model,
//! relight, boundary derivation, forceload spans, emission, the gate-seal
//! measurement and the bot export all see one more area and ask it the same
//! questions they ask a prefab-placed one.
//!
//! # Why the mass is fills and not a structure template
//!
//! A blockout box is a shell: six faces of one uniform block around a volume of
//! air. Packaged as a `.nbt` it is tens of thousands of cells, mostly air, split
//! across tiles because vanilla's structure template caps at 48 per axis — and
//! the compiler would need a template WRITER, which lives in `delvec::schem`
//! and is unreachable from here (`delvec` publishes to crates.io and may depend
//! only on published crates, and `schem` is `publish = false`).
//!
//! So a piece of the blockout is a [`PiecePlacement`](crate::compiler::plan::PiecePlacement)
//! with **no templates** and its blocks in
//! [`AreaPlacement::mass`](crate::compiler::plan::AreaPlacement::mass). Nothing downstream
//! special-cases it: `bbox()` reads `pos`/`size` and answers for forceload and
//! relight exactly as before, the template loop iterates an empty list, and the
//! mass fills land in [`crate::compiler::assembled::placed_blocks`] one step ahead of the
//! socket seals that already had that shape. A piece the prefab registry has
//! never heard of contributes no face contract and no anchors, which is correct:
//! a derived box makes no claim about mating with anything.
//!
//! # Determinism (ADR-0006)
//!
//! **No seed reaches this module.** The derivation takes the plan and the table
//! and nothing else — no RNG, no clock, no hash-order iteration, every walk over
//! a slice in document order and every map a `BTreeMap`. Changing
//! `world.seed` therefore changes no blockout byte, which spec-0049 §13.4
//! requires and [`crate::compiler::blockout`]'s tests measure.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::StationKind;
use delvewright_dsl::metrics::{MetricKind, MetricValue, Metrics, Reads, passable_width_cells};
use delvewright_dsl::siteplan::{
    Aabb, Crossing, ENTRY_ANCHOR, Ground, Owner, PlacedBox, PlacedSeam, SITE_AREA, Site,
    SitePlanContent, VolumeRole, merge_cells, node_anchor, seam_anchor, seam_unlock_anchor,
};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier, NodeId};
use serde::Serialize;

use crate::compiler::plan::{AnchorRole, AreaPlacement, PiecePlacement, ResolvedAnchor};
use crate::compiler::solver::{Rotation, SealFill};

/// The blockout's legibility palette (spec-0049 §5.1).
///
/// Distinct blocks for distinct jobs, so that a walker can see where one place
/// ends and another begins — which is the blockout's whole job, and the reason
/// the palette is fixed rather than authored. Every entry is a full opaque cube
/// that does not fall, does not burn and carries no block state, so nothing here
/// interacts with gravity settling, the fluid model or the partial-floor rule.
pub mod palette {
    /// The one-cell shell around every place.
    pub const WALL: &str = "minecraft:stone_bricks";
    /// What closes a place overhead.
    pub const CEILING: &str = "minecraft:smooth_stone";
    /// The ring of wall immediately around a seam's opening — the frame that
    /// makes a way out read as one from across the room.
    pub const FRAME: &str = "minecraft:polished_blackstone_bricks";
    /// A stair's treads, whole-block courses.
    pub const TREAD: &str = "minecraft:polished_diorite";
    /// A stair's half-courses. A bottom slab presents an 8/16 top face, which is
    /// inside the walk-up budget, so a derived stair is walked and never jumped.
    pub const TREAD_HALF: &str = "minecraft:polished_diorite_slab[type=bottom]";
    /// A declared roof zone over a place no piece has drawn yet (spec-0098
    /// §8): massed solid so the skyline is walked before it is drawn.
    pub const ROOF: &str = "minecraft:bricks";
    /// What a sealed `barred` seam stands in until content opens it.
    ///
    /// Re-exported, never restated: `DW0343` asks whether a gate anchor declares
    /// a fill block, and for a derived seam the answer is this constant, so the
    /// check and the derivation must read one definition rather than agree.
    pub const BAR: &str = delvewright_dsl::siteplan::SEAM_BAR;
    /// Air.
    pub const AIR: &str = "minecraft:air";

    /// The per-place accent, cycled deterministically over the plan's boxes in
    /// document order. A place's FLOOR is its accent, so the colour under a
    /// body's feet says which place it is standing in — the cheapest legibility
    /// a blockout can buy, and it costs no extra geometry.
    pub const ACCENTS: [&str; 16] = [
        "minecraft:white_concrete",
        "minecraft:light_gray_concrete",
        "minecraft:gray_concrete",
        "minecraft:black_concrete",
        "minecraft:brown_concrete",
        "minecraft:red_concrete",
        "minecraft:orange_concrete",
        "minecraft:yellow_concrete",
        "minecraft:lime_concrete",
        "minecraft:green_concrete",
        "minecraft:cyan_concrete",
        "minecraft:light_blue_concrete",
        "minecraft:blue_concrete",
        "minecraft:purple_concrete",
        "minecraft:magenta_concrete",
        "minecraft:pink_concrete",
    ];

    /// The accent for the `i`th place in the plan's own order.
    #[must_use]
    pub fn accent(i: usize) -> &'static str {
        ACCENTS[i % ACCENTS.len()]
    }
}

/// What the derivation built, and what the stage-5 battery judges it against.
///
/// The plan-side half ([`Blockout::boxes`], [`Blockout::seams`]) is carried here
/// **as resolved by `delvewright_dsl::siteplan`**, not as re-resolved by this
/// module: the battery reads the declaration and the bytes, and if it read a
/// declaration this module had computed for itself it would be judging the
/// derivation against the derivation's own opinion.
pub struct Blockout {
    /// The synthesized spatial vocabulary, by anchor name — see
    /// [`Blockout::anchors`].
    synthesized: Vec<(String, AnchorSpec)>,
    /// The plan's places, resolved into world cells.
    pub boxes: Vec<PlacedBox>,
    /// The plan's connections, resolved into world cells.
    pub seams: Vec<PlacedSeam>,
    /// The site's fill, resolved.
    pub ground: Ground,
    /// The places this derivation stood a stand-in in — every place no piece is
    /// bound to, by name, in plan order. A stand-in never ships (spec-0098 §8).
    pub massed: Vec<String>,
    /// What the derivation bound to.
    pub binding: Binding,
}

/// One synthesized anchor, in the shape the plan resolves it to.
///
/// A tiny mirror of [`ResolvedAnchor`] rather than the type itself, and for one
/// reason: `ResolvedAnchor` is not `Clone`, deliberately — a resolved anchor is
/// a fact about a placement and copying one is how two areas come to claim one
/// cell. The derivation produces its anchors before there is a plan to put them
/// in, so it carries the *description* and [`Blockout::anchors`] is where each
/// becomes exactly one resolved anchor.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AnchorSpec {
    /// A place to stand.
    Point([i32; 3]),
    /// A region content fills and clears.
    Gate {
        from: [i32; 3],
        to: [i32; 3],
        block: String,
    },
}

impl Blockout {
    /// Where a body stands in this place — the cell the derivation put the
    /// place's own anchor on, read off the mass it had just laid.
    ///
    /// The battery routes and seeds from THIS rather than from the plan's
    /// footprint centre, and the difference is not cosmetic: a stair the plan
    /// hosts in a box legitimately stands on that box's centre, so a proof
    /// rooted there would route from inside the massing and report a place the
    /// campaign spawns bodies in as unroutable.
    #[must_use]
    pub fn footing(&self, node: &NodeId) -> Option<[i32; 3]> {
        let want = node_anchor(node);
        self.synthesized.iter().find_map(|(name, spec)| match spec {
            AnchorSpec::Point(p) if *name == want => Some(*p),
            _ => None,
        })
    }

    /// The synthesized spatial vocabulary, ready to seat in a plan's anchor map
    /// — each name, where it is, and **what it is for** where that is a question
    /// the compiler has to answer without being told the name (spec-0046).
    ///
    /// Consumed once, by [`crate::compiler::plan::Plan::build`]; a second consumer would
    /// be a second area claiming the same cells, which is why this hands out
    /// owned values rather than a borrow anything could hold.
    ///
    /// The role travels with the anchor rather than being recovered by the
    /// consumer comparing a name against [`ENTRY_ANCHOR`]: this derivation is
    /// the producer that knows which node the graph calls its entry, and a
    /// consumer that re-derived it from a spelling would be the second place
    /// deciding what an entry is — the exact thing spec-0046 removes.
    pub fn anchors(&self) -> Vec<(&str, ResolvedAnchor, Option<AnchorRole>)> {
        self.synthesized
            .iter()
            .map(|(name, spec)| {
                let resolved = match spec {
                    AnchorSpec::Point(pos) => ResolvedAnchor::Point {
                        pos: *pos,
                        facing: None,
                    },
                    AnchorSpec::Gate { from, to, block } => ResolvedAnchor::Gate {
                        from: *from,
                        to: *to,
                        block: block.clone(),
                    },
                };
                // `derive` writes exactly one anchor under this name, and only
                // for the graph's entry node — so the comparison is reading
                // back this module's own single claim, not re-answering it.
                let role = (name == ENTRY_ANCHOR).then_some(AnchorRole::Entry);
                (name.as_str(), resolved, role)
            })
            .collect()
    }
}

/// What a run's derivation bound to — stated on every build, per the standing
/// rule that a count only means something when the run that found nothing prints
/// it too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Binding {
    /// Places the plan resolved — the denominator, not the number massed.
    ///
    /// A bound place is in here and is massed by nobody: its frame is a hole
    /// this derivation leaves for a piece. [`Binding::line`] states both, because
    /// one number that meant either would be the count nobody can read.
    pub boxes: usize,
    /// Of those, places a detail plan bound — whose frame this derivation
    /// deliberately left empty for a piece to fill (spec-0050 §3).
    pub detailed: usize,
    /// Connections cut.
    pub seams: usize,
    /// Of those, connections whose massing includes a stair **that this
    /// derivation laid**. A stair hosted in a bound box is the piece's to build,
    /// so it is not counted here — the count means what it says.
    pub stairs: usize,
    /// Cells of a floor cut over a through-floor stair's run beyond the hole the
    /// plan allocated — the stairwell [`stairwell`] measured the climb to need.
    /// Zero on a plan whose every through-floor run already climbs inside its
    /// hole, and on one with no through-floor stair at all.
    pub stairwell_cells: u64,
    /// Of those, connections sealed at world load.
    pub barred: usize,
    /// Whole-owned masses written.
    pub volumes: usize,
    /// Anchors synthesized.
    pub anchors: usize,
    /// Region writes emitted.
    pub fills: usize,
    /// World cells the writes cover.
    pub cells: u64,
    /// Cells of the rings' fixed ground the terrain pass wrote (spec-0098 §2
    /// rule 0), for every place, bound or not.
    pub fixed_cells: u64,
    /// Roof zones massed over stand-ins (spec-0098 §8).
    pub roofs: usize,
}

impl Binding {
    /// One line, for stderr and for the round summary.
    ///
    /// Printed by `crate::compiler::emit::build_with_warnings`, beside the battery's, on
    /// every build of a site-plan campaign. It had no caller at all until stage 6
    /// — the observer's count was stated and the builder's was not — which is the
    /// UNRUN shape at the smallest scale: a line that is correct, reviewable, and
    /// reaches nobody.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "blockout binding: {b} place(s) massed ({de} detailed, so {un} massed by the \
             derivation; {r} roof zone(s) massed), {fx} fixed ring ground cell(s) laid, {s} \
             seam(s) cut ({st} stair, {ba} barred), {sw} stairwell cell(s) cut over \
             through-floor runs, {v} whole-owned volume(s), {a} anchor(s) synthesized, {f} \
             region write(s) over {c} cell(s).",
            r = self.roofs,
            fx = self.fixed_cells,
            de = self.detailed,
            un = self.boxes.saturating_sub(self.detailed),
            b = self.boxes,
            s = self.seams,
            st = self.stairs,
            sw = self.stairwell_cells,
            ba = self.barred,
            v = self.volumes,
            a = self.anchors,
            f = self.fills,
            c = self.cells,
        )
    }
}

/// One accumulating region write, in the order the world applies them.
struct Mass {
    fills: Vec<SealFill>,
    cells: u64,
    /// **The frames a detail plan bound** (spec-0050 §3): inclusive world AABBs
    /// this derivation does not write inside, in plan document order.
    ///
    /// One rule rather than six special cases, and that is the whole of the
    /// fabric split. A bound piece owns its play space and the floor course
    /// under it; the derivation's floor accent, its interior clear, the ceiling
    /// of the box stacked underneath, a stair hosted in the box and a bar
    /// standing in the box's own floor course are all *writes that land inside
    /// that frame*, so all five stop by the same subtraction. A list of five
    /// exemptions is a list the sixth escapes.
    ///
    /// What stays whole-owned falls out of the same rule without being stated
    /// again: every vertical party plane, every wall, every unshared shell face
    /// and every ring of floor under a wall lie OUTSIDE the frame, so they are
    /// written exactly as they were at stage 5 whether or not the boxes beside
    /// them are detailed.
    ///
    /// **The one place the rule and spec-0050 §3's list read differently**, and
    /// it is recorded here because this is where the choice is made: that list
    /// says every seam frame stays whole-owned, and it also says the horizontal
    /// party plane between stacked boxes IS the upper box's floor course and
    /// belongs to the upper piece. For a stacked pair those two are the same
    /// cells, so a literal reading of both is unsatisfiable. The subtraction
    /// resolves it in favour of the sentence that is specific about stacked
    /// boxes: with the upper box bound, the seam frame in that plane is the
    /// piece's, like the rest of its floor. A vertical seam's frame — every seam
    /// frame in any map that does not stack — is untouched.
    ///
    /// Empty for every campaign with no detail plan, so such a campaign's output
    /// does not move by a byte — [`crate::compiler::blockout::derive`] passes an empty
    /// slice and a test measures the byte-identity.
    holes: Vec<([i64; 3], [i64; 3])>,
}

impl Mass {
    /// A fresh mass with the frames a detail plan bound; an empty slice is a
    /// campaign that details nothing, which is every campaign below 0.15.0.
    fn new(holes: Vec<([i64; 3], [i64; 3])>) -> Mass {
        Mass {
            fills: Vec::new(),
            cells: 0,
            holes,
        }
    }

    /// Write `block` over the inclusive world AABB `lo..=hi`, **split so that no
    /// single write exceeds what the game will accept**.
    ///
    /// Vanilla's `/fill` refuses a region over [`MAX_FILL_CELLS`] blocks, and it
    /// refuses it at RUN TIME, on the server, in a `setup` function whose reply
    /// nobody reads — the exact shape `CLAUDE.md` names as *a command whose
    /// response nobody reads cannot fail*. So the limit is enforced where it
    /// cannot be forgotten: a region too big to fill is not representable in this
    /// list, because the one function that appends to it splits first. The split
    /// halves the longest axis and recurses, which is deterministic and
    /// independent of how the caller happened to order the corners.
    ///
    /// Coordinates arrive as `i64` because the plan is `i64` throughout; they are
    /// narrowed here, at the one boundary, because the world model is `i32`. A
    /// plan outside `i32` cannot describe a Minecraft world at all and `DW0826`
    /// has already held every box and volume inside the declared region, so the
    /// clamp below is honest rather than a silent wrap.
    /// Write `block` over `lo..=hi`, **minus every bound frame**.
    ///
    /// The subtraction is axis-by-axis and deterministic: for each axis in
    /// order, the slab of the region below the hole is emitted, then the slab
    /// above, and what is left is the overlap, which is dropped. At most six
    /// sub-regions per hole, in one fixed order, so two runs over one plan emit
    /// the same fills in the same sequence (ADR-0006).
    fn write(&mut self, lo: [i64; 3], hi: [i64; 3], block: &str) {
        self.write_outside(lo, hi, block, 0);
    }

    fn write_outside(&mut self, lo: [i64; 3], hi: [i64; 3], block: &str, hole: usize) {
        if (0..3).any(|i| lo[i] > hi[i]) {
            return;
        }
        let Some((hlo, hhi)) = self.holes.get(hole).copied() else {
            return self.write_raw(lo, hi, block);
        };
        if (0..3).any(|i| hi[i] < hlo[i] || lo[i] > hhi[i]) {
            return self.write_outside(lo, hi, block, hole + 1); // disjoint
        }
        let (mut rlo, mut rhi) = (lo, hi);
        for axis in 0..3 {
            if rlo[axis] < hlo[axis] {
                let mut slab_hi = rhi;
                slab_hi[axis] = hlo[axis] - 1;
                self.write_outside(rlo, slab_hi, block, hole + 1);
                rlo[axis] = hlo[axis];
            }
            if rhi[axis] > hhi[axis] {
                let mut slab_lo = rlo;
                slab_lo[axis] = hhi[axis] + 1;
                self.write_outside(slab_lo, rhi, block, hole + 1);
                rhi[axis] = hhi[axis];
            }
        }
        // Whatever survived all three axes is the intersection with the frame,
        // and the frame is the piece's.
    }

    /// Write `block` over `lo..=hi`, but only in the cells of `mask` — the
    /// cells one place owns (spec-0098 §8: a stand-in is written only in its own
    /// place's claim). A non-zero `sink` is the `Perturb::sink` defect, which
    /// writes unmasked so the displaced mass lands where the defect puts it.
    fn write_within(&mut self, lo: [i64; 3], hi: [i64; 3], block: &str, mask: &[Aabb], sink: i64) {
        if sink != 0 {
            return self.write(lo, hi, block);
        }
        for (mlo, mhi) in mask {
            let a = [lo[0].max(mlo[0]), lo[1].max(mlo[1]), lo[2].max(mlo[2])];
            let b = [hi[0].min(mhi[0]), hi[1].min(mhi[1]), hi[2].min(mhi[2])];
            self.write(a, b, block);
        }
    }

    fn write_raw(&mut self, lo: [i64; 3], hi: [i64; 3], block: &str) {
        if (0..3).any(|i| lo[i] > hi[i]) {
            return;
        }
        let extent: [u64; 3] = [
            (hi[0] - lo[0] + 1) as u64,
            (hi[1] - lo[1] + 1) as u64,
            (hi[2] - lo[2] + 1) as u64,
        ];
        let n = extent[0] * extent[1] * extent[2];
        if n > MAX_FILL_CELLS {
            let axis = (0..3).max_by_key(|i| extent[*i]).unwrap_or(0);
            let mid = lo[axis] + (hi[axis] - lo[axis]) / 2;
            let mut a_hi = hi;
            a_hi[axis] = mid;
            let mut b_lo = lo;
            b_lo[axis] = mid + 1;
            self.write_raw(lo, a_hi, block);
            self.write_raw(b_lo, hi, block);
            return;
        }
        self.cells += n;
        self.fills.push(SealFill {
            from: narrow(lo),
            to: narrow(hi),
            block: block.to_string(),
        });
    }
}

impl Mass {
    /// The cells of `lo..=hi` this mass leaves **occupied**, by replaying every
    /// write in order.
    ///
    /// The derivation reads its own output for one question and it is not an
    /// optional one: **where in this place can a body actually stand?** A box's
    /// floor centre is the obvious answer and it is sometimes wrong — a stair the
    /// plan hosts in that box legitimately stands on it — so an anchor placed by
    /// arithmetic over the plan alone lands inside the massing perhaps one box in
    /// five, and `summon` does no snapping. Reading the mass is what makes the
    /// synthesized vocabulary a fact about the world rather than a hope about it.
    fn solid_in(&self, lo: [i64; 3], hi: [i64; 3]) -> BTreeSet<[i64; 3]> {
        let mut out: BTreeSet<[i64; 3]> = BTreeSet::new();
        for f in &self.fills {
            let flo = [
                i64::from(f.from[0]).max(lo[0]),
                i64::from(f.from[1]).max(lo[1]),
                i64::from(f.from[2]).max(lo[2]),
            ];
            let fhi = [
                i64::from(f.to[0]).min(hi[0]),
                i64::from(f.to[1]).min(hi[1]),
                i64::from(f.to[2]).min(hi[2]),
            ];
            if (0..3).any(|i| flo[i] > fhi[i]) {
                continue;
            }
            let air = f.block == palette::AIR;
            for c in cells_of(flo, fhi) {
                if air {
                    out.remove(&c);
                } else {
                    out.insert(c);
                }
            }
        }
        out
    }
}

impl Mass {
    /// The blocks this mass leaves in `lo..=hi`, by replaying every write in
    /// order — what the assembled world will hold there, for a question the
    /// derivation must ask the engine's own movement model rather than answer
    /// with arithmetic of its own ([`stairwell`]).
    fn blocks_in(&self, lo: [i64; 3], hi: [i64; 3]) -> BTreeMap<[i32; 3], String> {
        let mut out: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for f in &self.fills {
            let flo = [
                i64::from(f.from[0]).max(lo[0]),
                i64::from(f.from[1]).max(lo[1]),
                i64::from(f.from[2]).max(lo[2]),
            ];
            let fhi = [
                i64::from(f.to[0]).min(hi[0]),
                i64::from(f.to[1]).min(hi[1]),
                i64::from(f.to[2]).min(hi[2]),
            ];
            if (0..3).any(|i| flo[i] > fhi[i]) {
                continue;
            }
            let air = f.block == palette::AIR;
            for c in cells_of(flo, fhi) {
                if air {
                    out.remove(&narrow(c));
                } else {
                    out.insert(narrow(c), f.block.clone());
                }
            }
        }
        out
    }
}

/// The most blocks one vanilla `/fill` will write (`fill` refuses above 32768).
///
/// Held here rather than at the emitter because the emitter's job is to print a
/// command, and a region that cannot be filled is a fact about the region.
pub const MAX_FILL_CELLS: u64 = 32768;

fn narrow(c: [i64; 3]) -> [i32; 3] {
    [
        c[0].clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        c[1].clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
        c[2].clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
    ]
}

/// **Derive the whole map's mass from the site plan.**
///
/// Returns `None` for a campaign that carries no site plan — which is every
/// campaign that places pieces with `areas[]`, and is why such a campaign's
/// output does not move by a byte.
///
/// The order the writes are applied in is the whole of the derivation's
/// arbitration rule, and it is stated once here rather than discovered in six
/// places:
///
/// 1. **whole-owned volumes**, in document order — the mass the map itself owns,
///    laid before anything is cut into it;
/// 2. **every place's shell**, in document order — floor course, four walls,
///    ceiling;
/// 3. **every place's interior**, in document order — cleared to air. This is a
///    separate pass rather than part of (2) *on purpose*: a neighbour's shell
///    may legally stand in the cells over or under a place, and clearing every
///    interior after every shell is what makes "the play space the plan
///    allocated is air" an invariant of the derivation rather than a property of
///    the order two boxes happen to be written in;
/// 4. **every seam's frame**, then its **opening** — air, or the bar on a
///    `barred` way;
/// 5. **every stair's treads**, inside the box the plan said hosts them.
pub fn derive(c: &Campaign, reads: &mut Reads) -> Option<(AreaPlacement, Blockout)> {
    derive_with(c, reads, Perturb::none())
}

/// [`derive`], with a deliberate defect built in — see [`Perturb`] for why this
/// exists and why it is an argument rather than a switch.
pub fn derive_with(
    c: &Campaign,
    reads: &mut Reads,
    perturb: Perturb,
) -> Option<(AreaPlacement, Blockout)> {
    let bound = &delvewright_dsl::bound_places(c);
    c.site_plan.as_ref()?;
    let plan = &c.site_plan.as_ref()?.content;
    let table = Metrics::table();
    let boxes = delvewright_dsl::siteplan::placed_boxes(c, reads);
    let seams = delvewright_dsl::siteplan::placed_seams(c, &boxes, reads);
    let ground = delvewright_dsl::siteplan::Ground::of(c);
    let site = Site::new(&boxes, &seams, &ground);
    let by_node: BTreeMap<&str, &PlacedBox> =
        boxes.iter().map(|b| (b.node.0.as_str(), b)).collect();

    // Who owns what, once, from the one rule every reader shares.
    let owned: Vec<Vec<Aabb>> = (0..boxes.len()).map(|i| site.ownership(i).owned).collect();
    let is_bound = |i: usize| bound.contains(boxes[i].node.0.as_str());
    // The cells a binding owns, in plan document order. `Mass::holes` is what
    // the fabric split IS: nothing below writes a cell a bound piece owns.
    let holes: Vec<Aabb> = (0..boxes.len())
        .filter(|i| is_bound(*i))
        .flat_map(|i| owned[i].iter().copied())
        .collect();
    let detailed = (0..boxes.len()).filter(|i| is_bound(*i)).count();
    let mut mass = Mass::new(holes);
    let mut pieces: Vec<PiecePlacement> = Vec::new();
    let (rlo, rhi) = (plan.region.min, plan.region.max());

    // (0) What undeclared space becomes (spec-0098 §2b): the declared fill, laid
    // over the whole region before anything is placed in it.
    lay_fill(&mut mass, &ground, plan, rlo, rhi);

    // (1) The whole's own mass, each volume of the block its kind takes from
    // the fill unless it names its own.
    for v in &plan.volumes {
        let lo = v.region.min;
        let hi = v.region.max();
        match (&v.block, v.role) {
            (_, VolumeRole::Clearance) => mass.write(lo, hi, palette::AIR),
            (Some(b), _) => mass.write(lo, hi, b),
            (None, role) => {
                let top = hi[1];
                let body = ground.volume_block(role, [lo[0], top - 1, lo[2]], top);
                let cap = ground.volume_block(role, [lo[0], top, lo[2]], top);
                if top > lo[1] {
                    mass.write(lo, [hi[0], top - 1, hi[2]], body.unwrap_or(palette::AIR));
                }
                mass.write([lo[0], top, lo[2]], hi, cap.unwrap_or(palette::AIR));
            }
        }
        pieces.push(piece(
            format!("blockout/{}", v.id.0),
            lo,
            [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1],
        ));
    }

    // (2) **The stand-ins** (spec-0098 §8): the shell of every place no piece
    // is bound to, written only in the cells that place owns — never in the
    // fixed ground, never in a neighbour's cells, never in a gap.
    let mut roofs_massed = 0usize;
    for (i, b) in boxes.iter().enumerate() {
        let (slo, shi) = site.shell(i);
        pieces.push(piece(
            format!("blockout/{}", b.node.0),
            slo,
            [
                shi[0] - slo[0] + 1,
                shi[1] - slo[1] + 1,
                shi[2] - slo[2] + 1,
            ],
        ));
        if is_bound(i) {
            continue;
        }
        let sink = perturb.drop_of(&b.node);
        let mask: &[Aabb] = &owned[i];
        let down = |c: [i64; 3]| [c[0], c[1] - sink, c[2]];
        // The ring: wall from the claim's bottom to the top of the play space —
        // over the fixed ground, which the mask leaves out.
        let wall_top = if perturb.short_walls {
            b.floor - sink
        } else {
            b.top() - sink
        };
        let (x0, x1, z0, z1) = (slo[0], shi[0], slo[2], shi[2]);
        for (a, c) in [
            ([x0, slo[1], z0], [x0, wall_top, z1]),
            ([x1, slo[1], z0], [x1, wall_top, z1]),
            ([x0 + 1, slo[1], z0], [x1 - 1, wall_top, z0]),
            ([x0 + 1, slo[1], z1], [x1 - 1, wall_top, z1]),
        ] {
            mass.write_within(down(a), [c[0], c[1], c[2]], palette::WALL, mask, sink);
        }
        // Floor course: the accent, so the colour under a body's feet names the
        // place it is standing in.
        let fy = b.floor_course_y();
        mass.write_within(
            down([x0, fy, z0]),
            down([x1, fy, z1]),
            palette::accent(i),
            mask,
            sink,
        );
        if !b.open {
            if let Some((zlo, zhi)) = b.roof_zone() {
                roofs_massed += 1;
                mass.write_within(zlo, zhi, palette::ROOF, mask, 0);
            }
            let ly = b.top() + 1;
            mass.write_within(
                down([x0, ly, z0]),
                down([x1, ly, z1]),
                palette::CEILING,
                mask,
                sink,
            );
        }
    }

    // (3) Every unbound place's interior, cleared — after every shell, so the
    // play space the plan allocated is air whatever order two boxes were
    // written in.
    for b in &boxes {
        if bound.contains(b.node.0.as_str()) {
            continue;
        }
        let sink = perturb.drop_of(&b.node);
        let (mut lo, mut hi) = b.space();
        lo[1] -= sink;
        hi[1] -= sink;
        let block = if perturb.brick_up.as_deref() == Some(b.node.0.as_str()) {
            palette::WALL
        } else {
            palette::AIR
        };
        mass.write(lo, hi, block);
        // A ceiling laid one course into the play space. Written here rather
        // than in (2) because (2)'s course sits ABOVE the play space and this
        // pass would clear anything laid inside it — the defect has to survive
        // the clear to be a defect at all.
        if perturb.low_ceiling.as_deref() == Some(b.node.0.as_str()) {
            mass.write(
                [lo[0], hi[1], lo[2]],
                [hi[0], hi[1], hi[2]],
                palette::CEILING,
            );
        }
    }

    // (3b) **The ring's fixed ground** (spec-0098 §2 rule 0), for every place,
    // bound or not: the terrain continued to the plot's edge, so the plot's edge
    // is the same ground at stage 5 as in the shipped world.
    let mut by_block: BTreeMap<String, BTreeSet<[i64; 3]>> = BTreeMap::new();
    for i in 0..boxes.len() {
        for (cell, g) in site.fixed_cells(i) {
            by_block
                .entry(ground.ground_block(cell, g).to_string())
                .or_default()
                .insert(cell);
        }
    }
    let fixed_cells: u64 = by_block.values().map(|v| v.len() as u64).sum();
    for (block, cells) in &by_block {
        for (lo, hi) in merge_cells(cells) {
            mass.write(lo, hi, block);
        }
    }

    // Who owns each seam's plane, and whether a piece stands there to cut it.
    let plane_owner: Vec<Option<usize>> = seams
        .iter()
        .map(|s| match site.owner(s.opening.0) {
            Owner::Place(n) => site.index_of(&n),
            _ => None,
        })
        .collect();
    let cut_by_derivation =
        |k: usize| plane_owner[k].is_none_or(|i| !bound.contains(boxes[i].node.0.as_str()));

    // (4) Every PORTAL's frame.
    //
    // A contact gets none, and that is what a contact IS: the boundary is
    // continuous ground and the derivation writes no wall along the span
    // (spec-0053 §4). A frame ring around a 55-cell front would be a wall drawn
    // in a second block — the exact thing the span says is not there — and it
    // would stand in every column the crossing profile is measured over.
    //
    // Only where the plane's owner is a stand-in, and only in the cells it
    // owns: a bound owner's piece draws its own doorway, and the ring's fixed
    // ground under a sill is the whole's.
    let mut anchors: BTreeMap<String, AnchorSpec> = BTreeMap::new();
    for (k, s) in seams.iter().enumerate() {
        if s.crossing == Crossing::Contact || !cut_by_derivation(k) {
            continue;
        }
        let mask: &[Aabb] = plane_owner[k].map_or(&[], |i| owned[i].as_slice());
        for (flo, fhi) in frame_ring(s) {
            mass.write_within(flo, fhi, palette::FRAME, mask, 0);
        }
    }

    // (5) Every stair's treads.
    //
    // A stair hosted in a BOUND box is skipped rather than written-and-clipped,
    // and the difference is the count: `tread` reports whether it CALLED the
    // writer, and every one of those calls lands inside the hole, so counting
    // them would report stairs the derivation did not build under a field whose
    // doc says "connections whose massing includes a stair". The climb is the
    // piece's to build (spec-0050 §3) and the bytes battery proves it was built.
    let mut stairs = 0usize;
    let mut runs: Vec<(&PlacedSeam, &PlacedBox, LaidRun)> = Vec::new();
    for s in &seams {
        let Some(host_id) = &s.stair_in else { continue };
        if bound.contains(host_id.0.as_str()) {
            continue;
        }
        let Some(host) = by_node.get(host_id.0.as_str()).copied() else {
            continue;
        };
        if let Some(run) = tread(&mut mass, s, host, &table, reads) {
            stairs += 1;
            runs.push((s, host, run));
        }
    }

    // (6) **The openings, last.** A stair arrives AT its seam, so its top course
    // sits directly under or beside the hole — and a course written after the
    // hole was cut fills it back in. Cutting last is what makes *the opening the
    // plan allocated is open* an invariant of the derivation rather than a
    // property of which pass happened to run second: the massing may do what it
    // likes, and the hole is the last word.
    for (k, s) in seams.iter().enumerate() {
        let (olo, ohi) = slide(s, perturb.slide_openings);
        if !cut_by_derivation(k) && !(s.crossing == Crossing::Contact && perturb.wall_contacts) {
            // The plane is a bound piece's: its opening, and a `barred` way's
            // shut state, are the piece's to ship (spec-0098 §2).
            continue;
        }
        if s.crossing == Crossing::Contact && perturb.wall_contacts {
            // The deliberate defect: the front the plan allocated, walled.
            mass.write(olo, ohi, palette::WALL);
        } else if s.class == "barred" {
            mass.write(olo, ohi, palette::BAR);
            anchors.insert(
                seam_anchor(&s.edge),
                AnchorSpec::Gate {
                    from: narrow(olo),
                    to: narrow(ohi),
                    block: palette::BAR.to_string(),
                },
            );
        } else {
            mass.write(olo, ohi, palette::AIR);
        }
    }

    // The deliberate defect `Perturb::bury_barred` names: the far side of every
    // barred door walled flush behind its opening.
    if perturb.bury_barred {
        for s in seams
            .iter()
            .filter(|s| s.class == "barred" && s.crossing == Crossing::Portal)
        {
            let (mut olo, mut ohi) = slide(s, perturb.slide_openings);
            let b_off = s.face.vector()[s.normal_axis];
            olo[s.normal_axis] += b_off;
            ohi[s.normal_axis] += b_off;
            mass.write(olo, ohi, palette::WALL);
        }
    }

    // (7) **The stairwell over every through-floor run.** A stair up through a
    // floor starts under the hole and walks back under the floor it pierces, so
    // its upper courses stand where that floor takes a climbing body's head. The
    // hole the plan allocated is where the stair ARRIVES; the stairwell is what
    // lets a body get there. It is cut after the openings because it is measured
    // over them — see [`stairwell`].
    let mut stairwell_cells = 0u64;
    for (s, host, run) in &runs {
        stairwell_cells += stairwell(&mut mass, s, host, run, perturb.open_stairwells);
    }

    // The synthesized spatial vocabulary (spec-0049 §5.2), read off the mass
    // that has just been laid — see `Mass::solid_in` for why it cannot be read
    // off the plan.
    for b in &boxes {
        anchors.insert(
            node_anchor(&b.node),
            AnchorSpec::Point(narrow(footing(&mass, b, b.centre()))),
        );
    }
    if let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) {
        for s in &seams {
            let Some(edge) = graph.edges.iter().find(|e| e.id() == &s.edge) else {
                continue;
            };
            let delvewright_dsl::layout::Edge::Barred { opens_from, .. } = edge else {
                continue;
            };
            let side = match opens_from {
                delvewright_dsl::layout::OpensFrom::A => Some(&s.a),
                delvewright_dsl::layout::OpensFrom::B => Some(&s.b),
                delvewright_dsl::layout::OpensFrom::Either => None,
            };
            let Some(side) = side else { continue };
            let Some(host) = by_node.get(side.0.as_str()).copied() else {
                continue;
            };
            anchors.insert(
                seam_unlock_anchor(&s.edge),
                AnchorSpec::Point(narrow(footing(&mass, host, unlock_cell(s, host)))),
            );
        }
        // The entry. `ENTRY_ANCHOR` is the name it stands under; what makes it
        // the entry is the role `Blockout::anchors` hands out with it
        // (spec-0046), which is what the compiler resolves. The graph says
        // which node this is, so nothing downstream has to read a spelling.
        if let Some(b) = by_node.get(graph.entry.0.as_str()).copied() {
            anchors.insert(
                ENTRY_ANCHOR.to_string(),
                AnchorSpec::Point(narrow(footing(&mass, b, b.centre()))),
            );
        }

        // **The stations' stand-ins** (spec-0052 §5).
        //
        // A quest referencing a station of a still-massed place is the ordinary
        // mid-build state, not an edge case, so a station reference is never
        // unresolved: every station of every box is realized here, from the same
        // one authority validation resolved the name against, which is what makes
        // "a name that validated cannot fail to exist in the built world" true of
        // a massed map exactly as it is of a detailed one.
        //
        // Computed in TWO passes on purpose. The cells are all read off the mass
        // as it stands after the openings were cut (one immutable pass over every
        // box), and only then are the gate bars written. A single interleaved
        // pass would make each station's cell depend on which boxes were walked
        // before it, so the derivation would stop being a pure function of the
        // plan and start being a function of document order in a second, hidden
        // way.
        let mut station_cells: Vec<(String, [i64; 3], StationKind)> = Vec::new();
        for b in &boxes {
            let Some(node) = graph.nodes.iter().find(|n| n.id == b.node) else {
                continue;
            };
            if node.stations.is_empty() {
                continue;
            }
            // The place's own anchor is already standing on its footing, so the
            // stations start from the cell after it: two names on one cell would
            // be two places to put a body that is one place.
            let mut taken: BTreeSet<[i64; 3]> = BTreeSet::new();
            taken.insert(footing(&mass, b, b.centre()));
            for st in &node.stations {
                let cell = station_cell(&mass, b, &taken);
                taken.insert(cell);
                station_cells.push((st.anchor.as_str().to_string(), cell, st.kind));
            }
        }
        for (name, cell, kind) in station_cells {
            match kind {
                StationKind::Point => {
                    anchors.insert(name, AnchorSpec::Point(narrow(cell)));
                }
                StationKind::Gate => {
                    // A minimal sealed region of the derivation's own bar, opened
                    // and closed by the existing verbs exactly as a synthesized
                    // seam gate is. The bar is WRITTEN, so the world-load seal
                    // measures it shut like every other gate rather than taking
                    // the anchor's word for it.
                    mass.write(cell, cell, palette::BAR);
                    anchors.insert(
                        name,
                        AnchorSpec::Gate {
                            from: narrow(cell),
                            to: narrow(cell),
                            block: palette::BAR.to_string(),
                        },
                    );
                }
            }
        }
    }

    let binding = Binding {
        boxes: boxes.len(),
        detailed,
        seams: seams.len(),
        stairs,
        stairwell_cells,
        barred: seams.iter().filter(|s| s.class == "barred").count(),
        volumes: plan.volumes.len(),
        anchors: anchors.len(),
        fills: mass.fills.len(),
        cells: mass.cells,
        fixed_cells,
        roofs: roofs_massed,
    };
    let massed: Vec<String> = boxes
        .iter()
        .filter(|b| !bound.contains(b.node.0.as_str()))
        .map(|b| b.node.0.clone())
        .collect();
    Some((
        AreaPlacement {
            area_id: SITE_AREA.to_string(),
            pieces,
            seals: Vec::new(),
            mass: mass.fills,
        },
        Blockout {
            synthesized: anchors.into_iter().collect(),
            boxes,
            seams,
            ground,
            massed,
            binding,
        },
    ))
}

/// **Lay the declared fill over the whole region** (spec-0098 §2b): a `solid`
/// site's block everywhere; an `open` site's terrain column by column — the
/// `surface` block at the terrain's height, `below` under it, air above —
/// merged into rectangles of equal height so a smooth slope costs few writes.
/// Deterministic: the merge walks columns in `x`-then-`z` order (ADR-0006).
fn lay_fill(
    mass: &mut Mass,
    ground: &Ground,
    plan: &SitePlanContent,
    rlo: [i64; 3],
    rhi: [i64; 3],
) {
    let _ = plan;
    if !ground.is_declared() {
        return;
    }
    if !ground.is_open() {
        if let Some(b) = ground.fill_block(rlo) {
            let b = b.to_string();
            mass.write(rlo, rhi, &b);
        }
        return;
    }
    // Equal-height rectangles over the region's columns.
    let mut done: BTreeSet<(i64, i64)> = BTreeSet::new();
    for x in rlo[0]..=rhi[0] {
        for z in rlo[2]..=rhi[2] {
            if done.contains(&(x, z)) {
                continue;
            }
            let Some(top) = ground.top(x, z) else {
                continue;
            };
            let same =
                |xx: i64, zz: i64| !done.contains(&(xx, zz)) && ground.top(xx, zz) == Some(top);
            let mut z1 = z;
            while z1 < rhi[2] && same(x, z1 + 1) {
                z1 += 1;
            }
            let mut x1 = x;
            while x1 < rhi[0] && (z..=z1).all(|zz| same(x1 + 1, zz)) {
                x1 += 1;
            }
            for xx in x..=x1 {
                for zz in z..=z1 {
                    done.insert((xx, zz));
                }
            }
            let top = top.min(rhi[1]);
            if top < rlo[1] {
                continue;
            }
            let (Some(surface), below) = (
                ground.fill_block([x, top, z]).map(str::to_string),
                ground.fill_block([x, top - 1, z]).map(str::to_string),
            ) else {
                continue;
            };
            if top > rlo[1]
                && let Some(below) = below
            {
                mass.write([x, rlo[1], z], [x1, top - 1, z1], &below);
            }
            mass.write([x, top, z], [x1, top, z1], &surface);
        }
    }
}

/// A template-less placed piece: what the world's AABB readers (forceload,
/// relight, the stair lint's "which piece is this cell in") need, and nothing
/// more. See the module docs for why the blocks travel separately.
fn piece(prefab_id: String, lo: [i64; 3], size: [i64; 3]) -> PiecePlacement {
    PiecePlacement {
        prefab_id,
        templates: Vec::new(),
        pos: narrow(lo),
        size: narrow(size),
        rotation: Rotation::None,
        // Derived massing has no sockets: this world's ways are ALLOCATED by
        // the site plan and proved by `DW0836`, never mated.
        mated: Vec::new(),
    }
}

/// A seam's opening, displaced along its face's first in-plane axis.
///
/// `0` is the identity and is what the production path uses; anything else is a
/// [`Perturb`] asking the derivation to cut the hole somewhere the plan did not
/// allocate one.
fn slide(s: &PlacedSeam, by: i64) -> ([i64; 3], [i64; 3]) {
    let (mut lo, mut hi) = s.opening;
    if by == 0 {
        return (lo, hi);
    }
    let axis = (0..3).find(|a| *a != s.normal_axis).unwrap_or(0);
    lo[axis] += by;
    hi[axis] += by;
    (lo, hi)
}

/// The ring of wall immediately around a seam's opening, clipped to the face the
/// two boxes share.
///
/// Four rectangles rather than one hollow region, because a fill writes a box.
/// Clipping to the shared face is what stops a frame from painting itself over a
/// neighbouring seam's opening on the same wall.
fn frame_ring(s: &PlacedSeam) -> Vec<([i64; 3], [i64; 3])> {
    let (olo, ohi) = s.opening;
    let (slo, shi) = s.shared;
    let axes: Vec<usize> = (0..3).filter(|a| *a != s.normal_axis).collect();
    let (u, v) = (axes[0], axes[1]);
    let mut out = Vec::new();
    let mut push = |ulo: i64, uhi: i64, vlo: i64, vhi: i64| {
        let (ulo, uhi) = (ulo.max(slo[u]), uhi.min(shi[u]));
        let (vlo, vhi) = (vlo.max(slo[v]), vhi.min(shi[v]));
        if ulo > uhi || vlo > vhi {
            return;
        }
        let mut lo = [0i64; 3];
        let mut hi = [0i64; 3];
        lo[s.normal_axis] = s.plane;
        hi[s.normal_axis] = s.plane;
        lo[u] = ulo;
        hi[u] = uhi;
        lo[v] = vlo;
        hi[v] = vhi;
        out.push((lo, hi));
    };
    push(olo[u] - 1, ohi[u] + 1, olo[v] - 1, olo[v] - 1);
    push(olo[u] - 1, ohi[u] + 1, ohi[v] + 1, ohi[v] + 1);
    push(olo[u] - 1, olo[u] - 1, olo[v], ohi[v]);
    push(ohi[u] + 1, ohi[u] + 1, olo[v], ohi[v]);
    out
}

/// The cell a one-sided `barred` seam's far-side affordance stands on: the
/// standable cell of the openable place nearest the middle of the opening.
///
/// It is inside the box rather than in the wall, because an affordance is a
/// thing a body walks up to and presses — and the box is where the body is.
fn unlock_cell(s: &PlacedSeam, host: &PlacedBox) -> [i64; 3] {
    let (olo, ohi) = s.opening;
    let mid = [(olo[0] + ohi[0]) / 2, host.floor, (olo[2] + ohi[2]) / 2];
    let (lo, hi) = host.space();
    [
        mid[0].clamp(lo[0], hi[0]),
        host.floor,
        mid[2].clamp(lo[2], hi[2]),
    ]
}

/// Where a body actually stands in this place, nearest to `want`.
///
/// A standable cell is one whose own cell and the cell above it are clear and
/// whose support below is solid — the assembled world's own rule, applied to the
/// derivation's own output before that output becomes a world. The search is a
/// widening ring around `want` inside the play space, ordered by Chebyshev
/// distance then lexicographically, so two runs over one plan choose the same
/// cell (ADR-0006).
///
/// Falls back to `want` when the place offers no footing at all. That is not a
/// silent pass: a place with nowhere to stand is exactly what `DW0837` refuses
/// over the built bytes, and answering it here with a second refusal would be
/// two diagnostics for one defect.
fn footing(mass: &Mass, b: &PlacedBox, want: [i64; 3]) -> [i64; 3] {
    let (lo, hi) = b.space();
    let solid = mass.solid_in([lo[0], lo[1] - 1, lo[2]], [hi[0], hi[1], hi[2]]);
    let standable = |c: [i64; 3]| -> bool {
        !solid.contains(&c)
            && !solid.contains(&[c[0], c[1] + 1, c[2]])
            && solid.contains(&[c[0], c[1] - 1, c[2]])
    };
    let reach = (hi[0] - lo[0]).max(hi[2] - lo[2]).max(0);
    for r in 0..=reach {
        let mut best: Option<[i64; 3]> = None;
        for x in (want[0] - r).max(lo[0])..=(want[0] + r).min(hi[0]) {
            for z in (want[2] - r).max(lo[2])..=(want[2] + r).min(hi[2]) {
                if (x - want[0]).abs().max((z - want[2]).abs()) != r {
                    continue;
                }
                let c = [x, b.floor, z];
                if standable(c) && best.is_none_or(|w| c < w) {
                    best = Some(c);
                }
            }
        }
        if let Some(c) = best {
            return c;
        }
    }
    want
}

/// **The cell one station stands on while its place is massed** (spec-0052 §5).
///
/// The first standable cell of the box not already `taken`, in the derivation's
/// standing order — Chebyshev distance from the floor centre, then
/// lexicographically — which is the same rule [`footing`] searches by, so a
/// reader of this module learns one ordering and not two.
///
/// The author cannot state where this goes: a station has no coordinate, no
/// offset and no hint, and those are absent fields rather than optional ones.
/// The stand-in's geometry is massing, not design; the design lives in the piece,
/// where the name will land once one is bound.
///
/// Falls back to the box's centre when the place has fewer standable cells than
/// it has stations, and that is not a silent pass for the same reason
/// [`footing`]'s fallback is not: a place with nowhere to stand is what `DW0837`
/// refuses over the built bytes, and answering it here with a second refusal
/// would be two diagnostics for one defect.
fn station_cell(mass: &Mass, b: &PlacedBox, taken: &BTreeSet<[i64; 3]>) -> [i64; 3] {
    let (lo, hi) = b.space();
    let solid = mass.solid_in([lo[0], lo[1] - 1, lo[2]], [hi[0], hi[1], hi[2]]);
    let standable = |c: [i64; 3]| -> bool {
        !solid.contains(&c)
            && !solid.contains(&[c[0], c[1] + 1, c[2]])
            && solid.contains(&[c[0], c[1] - 1, c[2]])
    };
    let want = b.centre();
    let reach = (hi[0] - lo[0]).max(hi[2] - lo[2]).max(0);
    for r in 0..=reach {
        let mut best: Option<[i64; 3]> = None;
        for x in (want[0] - r).max(lo[0])..=(want[0] + r).min(hi[0]) {
            for z in (want[2] - r).max(lo[2])..=(want[2] + r).min(hi[2]) {
                if (x - want[0]).abs().max((z - want[2]).abs()) != r {
                    continue;
                }
                let c = [x, b.floor, z];
                if !taken.contains(&c) && standable(c) && best.is_none_or(|w| c < w) {
                    best = Some(c);
                }
            }
        }
        if let Some(c) = best {
            return c;
        }
    }
    want
}

/// Build one stair's treads inside the place the plan said hosts them.
///
/// Returns the run it laid, or `None` when it laid nothing: a seam whose two places are on one plane
/// has no climb (`DW0830` refuses that as a mislabelled walk), and a seam the
/// plan could not resolve a host for is a `DW0824`.
///
/// # The pitch, and a departure recorded where it is made
///
/// The pitch is the **gentlest standard the host affords**, chosen by
/// [`delvewright_dsl::siteplan::gentlest_pitch`] over the run
/// [`delvewright_dsl::siteplan::stair_run`] reports — the same two calls
/// `DW0830` makes, so a plan that reached green is a plan this can build, and
/// one that could not was refused before any block existed. That is one
/// function rather than a matching pair: the pair disagreed, and the plan it
/// disagreed about built no stair at all.
///
/// What is NOT taken from the table is [`Pitch::realization`]. The table names
/// `minecraft:*_stairs` for `pitch.stair`, and the assembled world's occupancy
/// model treats a stair block as a **full cube** — deliberately conservative,
/// because a stair's real collision is two half-steps and over-blocking a route
/// can only ever turn a proof red. Realizing a tread as a stair block would
/// therefore build a climb the engine's own navigation model reads as a
/// 16/16 jump per course, when the table's own `step_16` says the body takes two
/// 8/16 steps and never leaves the ground. The derivation builds the geometry
/// the table describes — `step_16` sixteenths per course — out of full blocks
/// and bottom slabs, whose top faces the occupancy model measures exactly. The
/// climb is then walked rather than jumped, which is what the standard claims.
fn tread(
    mass: &mut Mass,
    s: &PlacedSeam,
    host: &PlacedBox,
    table: &Metrics,
    reads: &mut Reads,
) -> Option<LaidRun> {
    if s.rise == 0 {
        return None;
    }
    let (lo, hi) = host.space();
    let (olo, ohi) = s.opening;

    // **What the run costs this host** — the climb the courses must carry, the
    // axis they walk, where course 0 stands and how much walk there really is.
    // All four come from [`delvewright_dsl::siteplan::stair_run`], which is the
    // same call `DW0830` makes: a plan that reached green is a plan this can
    // build, by construction rather than by two arithmetics agreeing. `None` is
    // a plan `DW0830` has already refused — a host at or above what the stair
    // reaches, or a hole that is not over this host (`DW0828`).
    let run = delvewright_dsl::siteplan::stair_run(
        host.floor,
        host.foot,
        s.normal_axis,
        s.plane,
        (olo, ohi),
    )?;
    let (climb, run_axis, start, step, available) =
        (run.climb, run.run_axis, run.start, run.step, run.available);

    // `None` is a plan `DW0830` refused; there is no standard to build.
    let pitch = delvewright_dsl::siteplan::gentlest_pitch(table, reads, climb, available)?;
    let courses = delvewright_dsl::siteplan::run_of(&pitch, climb).max(1);

    // **A run is laid whole or not at all.** `gentlest_pitch` was asked for a
    // standard that fits exactly this span and `courses` is that standard's own
    // arithmetic over the same climb, so this cannot fire — it is kept because
    // of what the alternative was. Laying the courses that DO fit and stopping
    // silently builds a stair whose bottom is missing, which reads as a stair to
    // every later reader and is not one: the body climbs into the place from
    // above and can never stand on its floor, and NOTHING says so, because a
    // place is "reached" the moment a body stands anywhere inside it. Refusing
    // instead leaves the climb unbuilt, which is a state the observer can see —
    // an unreached place is `DW0837`.
    if courses > available {
        return None;
    }

    // Which cells across the run the treads occupy: the opening's own width,
    // clipped to the host. A stair the width of its doorway is what a body can
    // actually walk up.
    let cross = if run_axis == 0 { 2 } else { 0 };
    let (clo, chi) = (olo[cross].max(lo[cross]), ohi[cross].min(hi[cross]));
    if clo > chi {
        return None;
    }

    // Course `k` counts back from the seam: `k = 0` is the course the body steps
    // off, and its top face stands exactly `climb` blocks over the host's own
    // walk plane. Whole blocks fill to the last whole course and a bottom slab
    // carries the 8/16 remainder a half-pitch leaves.
    let top16 = climb * 16;
    let mut laid = false;
    for k in 0..courses {
        // Height of this course's top face above the host's walk plane.
        let h16 = ((top16 * (courses - k)) / courses).clamp(0, top16);
        if h16 == 0 {
            continue;
        }
        let at = start + step * k;
        debug_assert!(
            at >= lo[run_axis] && at <= hi[run_axis],
            "the run was proven to fit the host before a block was written"
        );
        let whole = h16 / 16;
        let half = h16 % 16 != 0;
        let mut a = [0i64; 3];
        let mut b = [0i64; 3];
        a[run_axis] = at;
        b[run_axis] = at;
        a[cross] = clo;
        b[cross] = chi;
        if whole > 0 {
            a[1] = host.floor;
            b[1] = host.floor + whole - 1;
            mass.write(a, b, palette::TREAD);
            laid = true;
        }
        if half {
            a[1] = host.floor + whole;
            b[1] = host.floor + whole;
            mass.write(a, b, palette::TREAD_HALF);
            laid = true;
        }
    }
    laid.then_some(LaidRun {
        run_axis,
        cross,
        start,
        step,
        courses,
        clo,
        chi,
    })
}

/// Where one stair's courses were laid — what [`tread`] built, handed to
/// [`stairwell`] so the stairwell is cut over exactly those courses.
struct LaidRun {
    /// The horizontal axis the run walks: 0 = x, 2 = z.
    run_axis: usize,
    /// The horizontal axis across the run.
    cross: usize,
    /// Where course 0, the one the body steps off at the top, stands on `run_axis`.
    start: i64,
    /// Which way the run walks back into the host, down the climb: `+1` or `-1`.
    step: i64,
    /// How many courses were laid.
    courses: i64,
    /// The span across the run the treads occupy, inclusive.
    clo: i64,
    chi: i64,
}

/// **Cut the floor over a through-floor run wherever a body climbing it cannot
/// pass**, and return how many cells were cut.
///
/// A stair up through a floor starts under the hole the plan allocated and walks
/// back under the floor that hole pierces ([`delvewright_dsl::siteplan::stair_run`]),
/// so every course but the top few stands under that floor. A course close
/// enough to it puts a standing body's head in it, and a course a body cannot
/// stand on is a course nobody climbs: the stair is then a way down and never a
/// way up, and the place under it is one a body falls into and cannot leave
/// (`DW0921`). Stairwell headroom is measured vertically over each tread, and
/// the floor opening is sized to the run for exactly that reason — that is
/// established practice, not this engine's invention.
///
/// **Measured, not computed.** Which courses need the floor cut away is not
/// answered by arithmetic over a body's height here: that would be a second
/// model of how a body climbs, and the first is the one `DW0921` floods —
/// [`crate::compiler::nav::World::body_moves`], over the engine's occupancy
/// model of these very blocks. So, from the foot of the run to its head, each
/// step from one course (or the floor beyond the run) onto the next is asked of
/// that relation over the mass as laid; where it is not a move, the floor over
/// the course being stepped onto is cut, and if it is still not a move (a jump
/// sweeps the head through the cell over the course it leaves), the floor over
/// that one too. What is cut is only the course between the host's play space
/// and the plan's hole plane, over the columns the treads occupy — the floor
/// the plan put there, never the room above it.
///
/// A step still refused with both cut is left as it stands: nothing further up
/// the floor is the derivation's to remove, and the observers over the built
/// bytes (`DW0837`, `DW0921`) say so rather than this function guessing.
fn stairwell(
    mass: &mut Mass,
    s: &PlacedSeam,
    host: &PlacedBox,
    run: &LaidRun,
    every_course: bool,
) -> u64 {
    if s.normal_axis != 1 {
        return 0; // a stair across a wall has no floor over its run
    }
    let (lo, hi) = host.space();
    // The floor between the host's play space and the plane the hole is in.
    let (slab_lo, slab_hi) = (hi[1] + 1, s.plane);
    if slab_lo > slab_hi {
        return 0;
    }
    // The world the question is asked over: the host, its shell and the course
    // over the floor, which is all a step between two of its courses can touch.
    // It declines the campaign's premises: whether one tread can be stepped onto
    // from the one below is a fact of the laid blocks, and no gate, horizon or
    // declared hazard inside a stair's own box can change the answer.
    let wlo = [lo[0] - 1, host.floor - 1, lo[2] - 1];
    let whi = [hi[0] + 1, slab_hi + 3, hi[2] + 1];
    let world_of = |mass: &Mass| {
        crate::compiler::nav::World::from_occupancy(
            crate::compiler::assembled::occupancy_of(mass.blocks_in(wlo, whi), &BTreeSet::new()),
            crate::compiler::nav::Premises::geometry_only(),
        )
    };
    // Where a body stands in one column of the run: the first clear cell over
    // the host's walk plane — on the tread, whatever the tread is.
    let feet = |world: &crate::compiler::nav::World, along: i64, across: i64| -> [i32; 3] {
        let mut c = [0i64; 3];
        c[run.run_axis] = along;
        c[run.cross] = across;
        c[1] = host.floor;
        while c[1] <= slab_hi && !world.is_clear(narrow(c)) {
            c[1] += 1;
        }
        narrow(c)
    };
    let mut cut = 0u64;
    let cut_over = |mass: &mut Mass, along: i64| {
        let mut a = [0i64; 3];
        let mut b = [0i64; 3];
        a[run.run_axis] = along;
        b[run.run_axis] = along;
        a[run.cross] = run.clo;
        b[run.cross] = run.chi;
        a[1] = slab_lo;
        b[1] = slab_hi;
        let solid = mass.solid_in(a, b).len() as u64;
        if solid > 0 {
            mass.write(a, b, palette::AIR);
        }
        solid
    };
    if every_course {
        // `Perturb::open_stairwells`: the floor over the whole run, unmeasured.
        return (0..run.courses)
            .map(|k| cut_over(mass, run.start + run.step * k))
            .sum();
    }
    let inside = |along: i64| along >= lo[run.run_axis] && along <= hi[run.run_axis];
    // From the foot of the run to its head: the step onto course `k` is taken
    // from course `k + 1`, or from the floor beyond the run when `k` is the last.
    for k in (0..run.courses).rev() {
        let onto = run.start + run.step * k;
        let from = run.start + run.step * (k + 1);
        if !inside(from) {
            continue; // the run meets the far wall; its foot is stepped onto from beside it
        }
        for across in run.clo..=run.chi {
            for column in [onto, from] {
                let world = world_of(mass);
                if world
                    .body_moves(feet(&world, from, across))
                    .contains(&feet(&world, onto, across))
                {
                    break;
                }
                cut += cut_over(mass, column);
            }
        }
    }
    cut
}

/// Every cell a set of region writes covers, for a caller that needs the whole
/// blockout as a cell set (the stage-5 battery's ownership map).
pub fn cells_of(lo: [i64; 3], hi: [i64; 3]) -> impl Iterator<Item = [i64; 3]> {
    (lo[1]..=hi[1]).flat_map(move |y| {
        (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z]))
    })
}

/// The places a cell belongs to, for the battery — see `crate::compiler::blockout::check`.
#[must_use]
pub fn owner_of(boxes: &[PlacedBox], cell: [i32; 3]) -> Option<&NodeId> {
    boxes
        .iter()
        .find(|b| {
            let (lo, hi) = b.space();
            (0..3).all(|i| i64::from(cell[i]) >= lo[i] && i64::from(cell[i]) <= hi[i])
        })
        .map(|b| &b.node)
}

/// Every cell of every seam's opening, as a set — what a crossing is allowed to
/// pass through.
#[must_use]
pub fn seam_cells(seams: &[PlacedSeam]) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    for s in seams {
        let (lo, hi) = s.opening;
        for c in cells_of(lo, hi) {
            out.insert(narrow(c));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The stage-5 battery (spec-0049 §5.3) — the derivation's independent observer
// ---------------------------------------------------------------------------

delvewright_dsl::dw_code! {
    /// `DW0836`: a built seam disagrees with its allocation.
    pub const DW_SEAM_BUILT: DwCode = DwCode::new("DW0836", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0837`: a node's floor is unreached.
    pub const DW_NODE_UNREACHED: DwCode = DwCode::new("DW0837", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0877`: a contact nothing can cross (spec-0053 §6).
    ///
    /// The contact's measured crossing profile — the columns of its span a body
    /// crosses over the assembled bytes, under the compiler's own step rule — holds
    /// no run of body width. The author allocated a front and the massing walled it,
    /// so the graph declares a hand-off the world does not have.
    ///
    /// It is the **contact's half of `DW0836`'s first claim**, and it is a different
    /// claim rather than the same one widened. A portal is a hole and *every* cell
    /// the plan allocated must be clear; a contact is continuous ground and the
    /// massing standing on part of it is content, not a defect — a rim with a boulder
    /// on it is still a rim. So what a contact owes is not "all of it" but "somewhere
    /// along it", and asking a portal's question of a front would refuse correct
    /// content, which is exactly the failure `DW0343` already carries as a lesson.
    ///
    /// **Not a widening of the step rule**: the profile is read through
    /// `nav::World::neighbors`, the same rule every route proof in this compiler is
    /// taken under. A second step rule here would make this the one proof in the
    /// compiler taken under different physics.
    ///
    /// Build tier (exit 3).
    pub const DW_CONTACT_UNCROSSABLE: DwCode = DwCode::new("DW0877", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0986`: a portal nothing crosses.
    ///
    /// The portal's half of the crossing question `DW0877` asks of a contact. A
    /// portal's every cell can be clear (`DW0836`'s first claim holds) and the
    /// hole still lead nowhere: massing stands flush behind it on one side, so a
    /// body that opens the bar steps into air and meets a wall of treads. Every
    /// route proof then goes round by another way and stays green, which is the
    /// shape this code exists to refuse.
    ///
    /// **The quantifier**: some body standing in the opening steps, under
    /// `nav::World::neighbors` at the player's footprint, onto standable ground
    /// on **both** sides of the wall — the opening's standable cells taken as one
    /// floor a body may walk across first (with the stairwell `DW0836` admits
    /// beside a stair's hole, in the same wall), and measured over the world with
    /// every bar open. A `drop` owes only its **high** side, because the far side
    /// of a fall is what the step rule does not model (the policy `DW0877` and
    /// `DW0837` already hold); for a hole in a floor, where nothing under the
    /// opening holds a body up, the high side is the brink: the cell over the hole
    /// has room for a body and a body stands level beside it.
    ///
    /// Build tier (exit 3).
    pub const DW_PORTAL_UNCROSSABLE: DwCode = DwCode::new("DW0986", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0838`: a connection nothing allocated.
    pub const DW_CROSSING_UNALLOCATED: DwCode = DwCode::new("DW0838", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW0821`: a sightline is blocked. Warning in the slice — see [`sightlines`].
    pub const DW_SIGHTLINE_BLOCKED: DwCode = DwCode::new("DW0821", ExitTier::Build);
}

/// What the battery examined. Stated on every build, zero or not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct BatteryBinding {
    /// Seams proven against the bytes — `DW0836`.
    pub seams: usize,
    /// Of those, the ones that are CONTACTS — `DW0877`'s denominator.
    ///
    /// Stated beside the crossing columns rather than inferred from them,
    /// because zero columns over zero contacts and zero columns over three are
    /// different facts and only the pair separates them: the first is a campaign
    /// whose places all meet through doorways, and the second is a measurement
    /// that examined three fronts and found nothing crossable in any of them.
    pub contacts: usize,
    /// Columns of contact span measured crossable — `DW0877`'s numerator.
    pub contact_columns: usize,
    /// Portals whose crossing was measured — `DW0986`'s denominator. A portal
    /// `DW0836` already found solid is not counted here: its opening is not a
    /// hole to cross, and the refusal for it is already named.
    pub portals: usize,
    /// Of every portal seam, those not measured because `DW0836` found cells of
    /// the opening itself solid — stated so the denominator's gap is never silent.
    pub portals_solid: usize,
    /// Standable cells of those portals' openings (with the stairwell beside a
    /// stair's hole, in the same wall) a body could step out of — what `DW0986`
    /// measured. A portal with none is crossed only by a `drop`'s brink.
    pub portal_floor: usize,
    /// Shared walls examined for a wider or misplaced hole — `DW0836`.
    pub walls: usize,
    /// Open cells of those walls outside every allocation that claim 2 admitted
    /// as a stair's stairwell rather than refusing as a leak — see
    /// `stairwell_of`. Stated so that an admission is never silent.
    pub stairwell_cells: usize,
    /// Places proven reached — `DW0837`.
    pub nodes: usize,
    /// Standable cells classified by owner — `DW0838`.
    pub standable: usize,
    /// Unordered place pairs tested for an unallocated crossing — `DW0838`.
    pub pairs: usize,
    /// Sightlines walked — `DW0821`.
    pub sightlines: usize,
    /// Identities recomputed from the bytes — `DW0833`'s second call site.
    pub identities: usize,
    /// Of those, the ones whose measure has no byte-side referent and were
    /// therefore proven once rather than twice — see [`identities`].
    pub identities_declared_only: usize,
    /// Critical-path legs measured — `DW0822`'s second call site.
    pub legs: usize,
}

impl BatteryBinding {
    /// One line, for stderr and for the round summary.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "blockout battery binding: {s} seam(s) proven over {w} shared wall(s) (of them \
             {ct} contact(s), {cc} crossable column(s) measured; {pt} portal(s) measured \
             over {pc} standable opening cell(s), {ps} left to `DW0836` as solid; {sw} \
             unallocated open cell(s) admitted as a stair's stairwell), {n} place(s) \
             proven reached, {c} standable cell(s) classified over {p} place pair(s), \
             {sl} sightline(s) walked, {i} identity(ies) re-measured ({d} declaration-only), \
             {l} critical-path leg(s) measured.",
            s = self.seams,
            ct = self.contacts,
            cc = self.contact_columns,
            pt = self.portals,
            pc = self.portal_floor,
            ps = self.portals_solid,
            w = self.walls,
            sw = self.stairwell_cells,
            n = self.nodes,
            c = self.standable,
            p = self.pairs,
            sl = self.sightlines,
            i = self.identities,
            d = self.identities_declared_only,
            l = self.legs,
        )
    }
}

/// What the battery found, and what it bound to.
///
/// One list rather than "the refusal" and "the advisories", because severity
/// already carries that distinction and splitting it would let a caller report
/// one half. Every check runs even when an earlier one has failed: a run that
/// stops at the first red states a binding that counts only what it reached,
/// which is the truncation-fakes-coverage shape.
pub struct Battery {
    /// Everything found, in check order, each beside the rule that raised it.
    /// Errors refuse the build (exit 3); warnings (`DW0821`, `DW0822`) never do.
    ///
    /// The [`DwCode`] travels with the [`Diagnostic`] rather than being looked
    /// back up from `Diagnostic::code`, which is a `String`: a lookup table from
    /// code strings to rules is a second registry somebody has to remember to
    /// extend, and the whole reason `DwCode` pairs an id with its scope is that
    /// the scope should travel to every site that raises it.
    pub findings: Vec<(DwCode, Diagnostic)>,
    /// What was examined.
    pub binding: BatteryBinding,
}

impl Battery {
    /// The first refusal, if the build must stop.
    #[must_use]
    pub fn refusal(&self) -> Option<&(DwCode, Diagnostic)> {
        self.refusals().next()
    }

    /// **Every** refusal, in the order the battery raised them.
    ///
    /// A build stops at the first — `emit::BuildFailure` carries one code and
    /// one message, as every check in this compiler does — but one defect is
    /// routinely seen by more than one of these rules, and a report that names
    /// only the first is a report about a smaller world than the battery
    /// examined. Walls built a course tall are the standing example: the wall is
    /// then open above every allocation (`DW0836`) *and* a body hops between two
    /// places nothing connected (`DW0838`), and a creator who only ever sees the
    /// first fixes it, rebuilds, and meets the second.
    pub fn refusals(&self) -> impl Iterator<Item = &(DwCode, Diagnostic)> {
        self.findings
            .iter()
            .filter(|(_, d)| d.severity == delvewright_dsl::Severity::Error)
    }

    /// The advisories, for the build's own warning list.
    #[must_use]
    pub fn advisories(&self) -> Vec<Diagnostic> {
        self.findings
            .iter()
            .filter(|(_, d)| d.severity != delvewright_dsl::Severity::Error)
            .map(|(_, d)| d.clone())
            .collect()
    }
}

/// Raise one finding, keeping its rule beside it.
fn raise(d: &mut Vec<(DwCode, Diagnostic)>, code: DwCode, diag: Diagnostic) {
    d.push((code, diag));
}

/// **Judge the built blockout against the plan it was derived from.**
///
/// # What invokes this, and what happens without it
///
/// [`crate::compiler::emit::build_with_warnings`], on every build of a campaign whose plan
/// carries a blockout — the same place the gravity gate, the relight gate and
/// the critical-path proofs are bound, and for the same reason: it is the one
/// function that turns a `Plan` into a datapack, so nothing can ship a world
/// that went round it. A campaign with no site plan returns `None` and nothing
/// runs; there is no flag, no subcommand and no checklist line. Someone who
/// built a site-plan world without this would have had to emit a datapack
/// without `build_with_warnings`, and there is no such path.
///
/// # Why it is an independent observer and not a replay
///
/// Every verdict below compares **what the plan declares** with **what the
/// assembled bytes are**. Nothing here re-derives the mass: it does not know
/// where the derivation put a floor course, how it chose a pitch, or which cells
/// it cleared. It knows the plan — resolved by `delvewright_dsl::siteplan`, the
/// same resolution the stage-4 checks judged — and it knows the world. A
/// derivation that builds something else therefore disagrees with it, which is
/// what spec-0049's acceptance criterion 8 asks for and what the perturbation
/// tests demonstrate: reddening these codes requires perturbing the
/// **derivation**, never hand-authoring bytes.
///
/// The step rule is the compiler's own ([`crate::compiler::nav::World::neighbors`]), not a
/// second one written here — see that function for why its binding was widened
/// rather than copied.
#[must_use]
pub fn check(
    plan: &crate::compiler::plan::Plan,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> Option<Battery> {
    let b = plan.blockout.as_ref()?;
    let c = plan.campaign;
    let mut findings: Vec<(DwCode, Diagnostic)> = Vec::new();
    let mut binding = BatteryBinding::default();

    // **Both worlds here are geometry alone** (`nav::Premises::geometry_only`),
    // and the decline is this battery's own subject matter rather than an
    // oversight. The blockout battery judges a MASSING against the site plan
    // that produced it — is the hole the hole the plan cut, is there a second
    // one, does a body fit through the seam — and its two worlds carry their own
    // sealing authority (`seal_unopened`, from the quest graph's monotone
    // closure). Handing them the campaign's world-load gate seals as well would
    // put two answers to "what is shut" in one world.
    //
    // The one premise that would sharpen rather than duplicate is the declared
    // lethal volumes: a blockout node reachable only through a kill box is not
    // reached. That is a real gap and it is a finding, not a repair made here —
    // it changes what `DW0837` refuses, which is spec-0049's stage-5 contract
    // and belongs to a round that can judge the campaigns it would newly red.
    //
    // The world as a body meets it once every declared way is open. `DW0836` and
    // `DW0838` are questions about GEOMETRY — is the hole the hole the plan cut,
    // and is there a second one — so they are asked with nothing shut.
    let open = crate::compiler::nav::World::from_occupancy(
        crate::compiler::assembled::occupancy_over(blocks, &BTreeSet::new()),
        crate::compiler::nav::Premises::geometry_only(),
    );
    // The world with every way the graph's own gating closure never opens sealed
    // as the plan sealed it. The base assembled model deliberately holds gate
    // regions open (`crate::compiler::assembled`), so a reachability proof taken over it
    // would walk through a door nothing in the campaign ever unlocks.
    let sealed = crate::compiler::nav::World::from_occupancy(
        crate::compiler::assembled::occupancy_over(&seal_unopened(c, b, blocks), &BTreeSet::new()),
        crate::compiler::nav::Premises::geometry_only(),
    );

    seams_built(b, &open, &mut binding, &mut findings);
    nodes_reached(c, b, &sealed, &mut binding, &mut findings);
    crossings(c, b, &open, &mut binding, &mut findings);
    sightlines(c, b, &open, &mut binding, &mut findings);
    identities(c, b, &open, &mut binding, &mut findings);
    pacing(c, b, &open, &mut binding, &mut findings);
    Some(Battery { findings, binding })
}

/// The assembled blocks with every `barred` seam the graph's closure never opens
/// standing in its bar again.
///
/// The graph's monotone closure is the campaign's own answer to *which ways ever
/// open*, and it is read here rather than re-derived: `crate::compiler::blockout` has no
/// opinion about quest order and should not acquire one.
fn seal_unopened(
    c: &Campaign,
    b: &Blockout,
    blocks: &crate::compiler::blockstate::BlockMap,
) -> crate::compiler::blockstate::BlockMap {
    let mut out = blocks.clone();
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return out;
    };
    let grants = delvewright_dsl::layout::Grants::of(c, graph);
    let closure = delvewright_dsl::layout::Closure::run(graph, &grants);
    for s in &b.seams {
        if s.class != "barred" {
            continue;
        }
        let Some(edge) = graph.edges.iter().find(|e| e.id() == &s.edge) else {
            continue;
        };
        if delvewright_dsl::layout::Closure::satisfied(edge.gating(), &closure.obtained) {
            continue; // the campaign opens it; the base world's clear stands.
        }
        let (lo, hi) = s.opening;
        for cell in cells_of(lo, hi) {
            out.insert(
                narrow(cell),
                crate::compiler::blockstate::BlockState::new(palette::BAR),
            );
        }
    }
    out
}

/// **The crossing profile of a contact's span**, measured over the assembled
/// bytes (spec-0053 §4).
///
/// Returns the columns of the span a body can cross in. An empty result is a
/// front nothing crosses.
///
/// # What it deliberately does NOT report, and why
///
/// An earlier form returned the walk planes on either side per column, so that
/// `DW0836`'s rise claim could be taken per column as spec-0053 §4 describes it.
/// That claim is **not sound and is withdrawn**: measured on this repository's
/// own blockout fixture, an ordinary correct map falsifies it. A `stair` seam
/// hosted in one of the two places arrives at the very wall the contact spans,
/// so in the stair's own columns the two sides are level and the measured rise
/// is 0 while the plan declares -5 — and nothing about that map is wrong. A
/// seam's `rise` is `floor(b) − floor(a)`, a fact about the two places' FLOORS,
/// never a promise about every column of a wide front. The whole-place form of
/// the claim, which every seam already owed and which a contact now runs too, is
/// the sound one.
///
/// # What a column crossing MEANS, and the one asymmetry
///
/// A body stands on a standable cell of the span at the wall plane and steps out
/// of it. For a `walk` contact it must be able to step out on **both** sides:
/// walking ground is two-way and a front a body can only enter is not a
/// hand-off. For a `drop` contact only the **high** side is required, because
/// the far side of a fall is precisely what the step rule does not model — a
/// router that could fall would prove routes a body cannot come back from — and
/// `DW0837` already treats a declared drop by seeding rather than walking. The
/// same policy, in the same words, so this engine has one answer about drops and
/// not two.
///
/// The step rule is `nav::World::neighbors`, unmodified and unwidened.
fn contact_profile(s: &PlacedSeam, world: &crate::compiler::nav::World) -> Vec<i64> {
    // The face's two in-plane axes: the one columns run along, and the one
    // scanned within a column. On a vertical face the column axis is the
    // horizontal one, because a column is what a body walks past; on a
    // horizontal face x is columns and z is the scan.
    let col_axis = if s.normal_axis == 1 {
        0
    } else {
        (0..3)
            .find(|a| *a != s.normal_axis && *a != 1)
            .expect("a vertical face has one horizontal in-plane axis")
    };
    let scan_axis = (0..3)
        .find(|a| *a != s.normal_axis && *a != col_axis)
        .expect("a face has two in-plane axes");

    // **Which side of the wall plane `a` is on is a fact of the FACE**, not a
    // constant. The seam names a face OF `a`, so `a` sits on the side the face's
    // normal points AWAY from: on a `west` face the plane is `a`'s low corner
    // minus one and `a` is at `plane + 1`, and on an `east` face it is the other
    // way round. Read from `Face::vector`, which is where `shared_face` reads it
    // too, rather than assumed — a constant here would be right on half the
    // faces and quietly wrong on the rest.
    let n_dir = s.face.vector()[s.normal_axis];
    let (a_off, b_off) = (-n_dir, n_dir);

    // Which side a `drop` falls FROM: the higher floor. `rise` is
    // `floor(b) − floor(a)`, so a negative rise puts `a` above `b`.
    let need_both = s.class != "drop";
    let high_off = if s.rise <= 0 { a_off } else { b_off };

    let (lo, hi) = s.opening;
    let mut out = Vec::new();
    for u in lo[col_axis]..=hi[col_axis] {
        for v in lo[scan_axis]..=hi[scan_axis] {
            let mut c = [0i64; 3];
            c[s.normal_axis] = s.plane;
            c[col_axis] = u;
            c[scan_axis] = v;
            if !world.is_standable(narrow(c)) {
                continue;
            }
            let n = world.neighbors(narrow(c));
            let side = |off: i64| {
                n.iter()
                    .find(|x| i64::from(x[s.normal_axis]) == s.plane + off)
                    .map(|x| i64::from(x[1]))
            };
            let (on_a, on_b) = (side(a_off), side(b_off));
            let crosses = if need_both {
                on_a.is_some() && on_b.is_some()
            } else {
                side(high_off).is_some()
            };
            if crosses {
                out.push(u);
                break;
            }
        }
    }
    out
}

/// The longest unbroken run of consecutive columns in a crossing profile.
///
/// A body needs `passable_width_cells()` columns SIDE BY SIDE, not that many
/// scattered along the front — two crossable columns forty blocks apart do not
/// make a two-wide way. The profile is produced in column order, so this is one
/// pass.
fn widest_run(profile: &[i64]) -> usize {
    let mut best = 0usize;
    let mut run = 0usize;
    let mut prev: Option<i64> = None;
    for u in profile {
        run = if prev == Some(u - 1) { run + 1 } else { 1 };
        best = best.max(run);
        prev = Some(*u);
    }
    best
}

/// How many columns a seam's span has — the denominator a crossing profile is
/// stated against, so a profile of zero over a span of zero is distinguishable
/// from a profile of zero over a span of fifty-five.
fn column_count(s: &PlacedSeam) -> usize {
    let col_axis = if s.normal_axis == 1 {
        0
    } else {
        (0..3)
            .find(|a| *a != s.normal_axis && *a != 1)
            .expect("a vertical face has one horizontal in-plane axis")
    };
    let (lo, hi) = s.opening;
    usize::try_from(hi[col_axis] - lo[col_axis] + 1).unwrap_or(0)
}

/// `DW0836`: the hole in the wall is the hole the plan cut — no narrower, no
/// wider, nowhere else — and the climb it spans is the climb the plan declared.
///
/// Three claims, and the second is the one that could not be made at stage 4:
///
/// 1. **every allocated cell is passable.** A seam whose opening the derivation
///    failed to clear is a connection the graph declares and the world does not
///    have.
/// 2. **no other cell of the shared wall is passable.** Asked per *wall* rather
///    than per seam, because two connections may legitimately pierce one wall —
///    the union of their openings is what the wall is allowed to have.
/// 3. **the realized rise equals the declared rise.** Measured from the bytes as
///    the lowest cell a body can stand on inside each place, so a floor course
///    laid at the wrong height disagrees with the plan that put the two places
///    at those datums.
///
/// **A contact answers claim 1 differently, and `DW0877` is that answer.** A
/// portal is a hole and every cell of it must be clear; a contact is continuous
/// ground and the massing standing on part of it is content, so what it owes is
/// a crossable run of body width somewhere along the span. Claim 2 is unchanged
/// — wall outside the span, as ever — and claim 3 is taken **per crossing
/// column**, because one number for a fifty-five-cell front would be a claim
/// about its middle.
fn seams_built(
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    // Claim 1, and the realized walk plane every rise is measured against.
    let planes: BTreeMap<&str, Option<i64>> = b
        .boxes
        .iter()
        .map(|x| (x.node.0.as_str(), built_plane(x, &b.boxes, world)))
        .collect();
    for s in &b.seams {
        binding.seams += 1;
        let (lo, hi) = s.opening;

        // ---- A CONTACT's half of claim 1 (spec-0053 §4).
        //
        // A front is continuous ground, not a hole, so what it owes is a run of
        // body width somewhere along it rather than every cell of it. Massing
        // standing on part of a rim is content; a rim nothing can cross is a
        // hand-off the graph declares and the world does not have.
        if s.crossing == Crossing::Contact {
            binding.contacts += 1;
            let profile = contact_profile(s, world);
            let need = usize::try_from(passable_width_cells()).unwrap_or(1).max(1);
            let widest = widest_run(&profile);
            binding.contact_columns += profile.len();
            if widest < need {
                raise(
                    d,
                    DW_CONTACT_UNCROSSABLE,
                    Diagnostic::error(
                        DW_CONTACT_UNCROSSABLE,
                        "site-plan",
                        format!("/content/seams[{}]", s.edge),
                        format!(
                            "nothing crosses the contact the plan allocated for `{id}`. Of the \
                             {cols} column(s) of the front between `{a}` and `{b}` at x \
                             {x0}..{x1} y {y0}..{y1} z {z0}..{z1}, {n} are crossable and the \
                             longest unbroken run of them is {widest}, where a body needs \
                             {need}. The graph declares a hand-off here and the massing has \
                             walled it. Nobody wrote these blocks, so this is the derivation \
                             disagreeing with the plan it was derived from rather than an \
                             authoring mistake: the repair is in the compiler, not in the \
                             campaign. What the plan can say about it is where the front is — \
                             move `at`, or widen `contact.extent`, so the span lies where the \
                             two places actually meet.",
                            id = s.edge,
                            a = s.a,
                            b = s.b,
                            cols = column_count(s),
                            n = profile.len(),
                            x0 = lo[0],
                            x1 = hi[0],
                            y0 = lo[1],
                            y1 = hi[1],
                            z0 = lo[2],
                            z1 = hi[2],
                        ),
                    ),
                );
            }
            // No `continue`: the whole-place rise claim further down is owed by
            // every seam of either kind, and on a contact it is the one that
            // covers a drop's far side.
        }

        // ---- A PORTAL's half of claim 1: EVERY allocated cell is passable.
        //
        // A contact's cells are deliberately not asked this. A front is
        // continuous ground and massing standing on part of it is content, so
        // `DW0877` above asks the question a front can answer; asking a portal's
        // question of a front would refuse correct content.
        let blocked: Vec<[i64; 3]> = if s.crossing == Crossing::Portal {
            cells_of(lo, hi)
                .filter(|c| !world.is_clear(narrow(*c)))
                .collect()
        } else {
            Vec::new()
        };
        if !blocked.is_empty() {
            raise(
                d,
                DW_SEAM_BUILT,
                Diagnostic::error(
                    DW_SEAM_BUILT,
                    "site-plan",
                    format!("/content/seams[{}]", s.edge),
                    format!(
                        "the built world does not have the opening the plan allocated for `{id}`. Of \
                     the {n} cell(s) between `{a}` and `{b}` at x {x0}..{x1} y {y0}..{y1} z \
                     {z0}..{z1}, {k} are still solid — the first at {f:?}. The graph declares a \
                     way here and the world does not have one. Nobody wrote these blocks, so this \
                     is the derivation disagreeing with the plan it was derived from rather than \
                     an authoring mistake: the repair is in the compiler, not in the campaign.",
                        id = s.edge,
                        a = s.a,
                        b = s.b,
                        n = cells_of(lo, hi).count(),
                        x0 = lo[0],
                        x1 = hi[0],
                        y0 = lo[1],
                        y1 = hi[1],
                        z0 = lo[2],
                        z1 = hi[2],
                        k = blocked.len(),
                        f = blocked[0],
                    ),
                ),
            );
        }
        // ---- A PORTAL's crossing (`DW0986`): the hole, clear, leads a body
        // through. A portal `DW0836` has just found solid is left to it — its
        // opening is not a hole to cross, and counted so the gap is stated.
        if s.crossing == Crossing::Portal {
            if blocked.is_empty() {
                portal_crossing(s, world, binding, d);
            } else {
                binding.portals_solid += 1;
            }
        }
        // ---- Claim 3, over the two places' own walk planes. Owed by every
        // seam of either kind: on a contact it is what covers a drop's far side,
        // which the step rule does not walk to and the per-column claim above
        // therefore does not measure.
        let (Some(Some(pa)), Some(Some(pb))) = (
            planes.get(s.a.0.as_str()).copied(),
            planes.get(s.b.0.as_str()).copied(),
        ) else {
            continue; // a place with no footing at all is `DW0837`'s finding.
        };
        if pb - pa != s.rise {
            raise(
                d,
                DW_SEAM_BUILT,
                Diagnostic::error(
                    DW_SEAM_BUILT,
                    "site-plan",
                    format!("/content/seams[{}]", s.edge),
                    format!(
                        "`{id}` spans a climb of {got} block(s) in the built world and the plan puts \
                     its two places {want} apart. A body's feet land at y {pa} in `{a}` and at y \
                     {pb} in `{b}`, measured as the lowest cell each place offers to stand on. A \
                     rise is not authored — it is the consequence of where the plan put the two \
                     places — so a built rise that differs from it means mass was laid at a \
                     height the plan did not choose, and every proof taken over this world is \
                     about a map the site plan does not describe.",
                        id = s.edge,
                        a = s.a,
                        b = s.b,
                        got = pb - pa,
                        want = s.rise,
                    ),
                ),
            );
        }
    }

    // Claim 2, per shared wall.
    let mut walls: BTreeMap<([i64; 3], [i64; 3]), Vec<&PlacedSeam>> = BTreeMap::new();
    for s in &b.seams {
        walls.entry(s.shared).or_default().push(s);
    }
    for ((slo, shi), group) in &walls {
        binding.walls += 1;
        let allowed: BTreeSet<[i64; 3]> = group
            .iter()
            .flat_map(|s| cells_of(s.opening.0, s.opening.1))
            .collect();
        let open: Vec<[i64; 3]> = cells_of(*slo, *shi)
            .filter(|c| !allowed.contains(c) && world.is_clear(narrow(*c)))
            .collect();
        let well = stairwell_of(&b.boxes, group, &open, world);
        binding.stairwell_cells += well.len();
        let leaks: Vec<[i64; 3]> = open.into_iter().filter(|c| !well.contains(c)).collect();
        if leaks.is_empty() {
            continue;
        }
        let names: Vec<String> = group.iter().map(|s| s.edge.0.clone()).collect();
        raise(
            d,
            DW_SEAM_BUILT,
            Diagnostic::error(
                DW_SEAM_BUILT,
                "site-plan",
                "/content/seams",
                format!(
                    "the wall at x {x0}..{x1} y {y0}..{y1} z {z0}..{z1} is open in {k} cell(s) the \
                 plan allocated no seam for — the first at {f:?}. The plan cuts {n} opening(s) \
                 through this wall ({names}), covering {a} cell(s); everything else on it is \
                 wall. An opening wider than its allocation, or somewhere else entirely, is a way \
                 the site plan never agreed to and nothing downstream would ever have named.",
                    x0 = slo[0],
                    x1 = shi[0],
                    y0 = slo[1],
                    y1 = shi[1],
                    z0 = slo[2],
                    z1 = shi[2],
                    k = leaks.len(),
                    f = leaks[0],
                    n = group.len(),
                    names = names.join(", "),
                    a = allowed.len(),
                ),
            ),
        );
    }
}

/// `DW0986`: a portal's opening leads a body from one side to the other.
///
/// See [`DW_PORTAL_UNCROSSABLE`] for the quantifier. Taken over the world with
/// every way open, because the question is whether the hole leads anywhere once
/// it is opened, not whether it is open.
///
/// # Why a floor of the opening and not a column, as a contact measures
///
/// A body crosses a one-cell wall by standing in it: the step rule moves one
/// cell horizontally and at most one vertically, so every walk from one side to
/// the other stands on a cell of the wall's plane. But it need not step in and
/// out from the SAME cell. A stair through a floor arrives in the hole on one
/// tread and leaves it from the next — measured on the gallery's undercroft
/// stair, no single cell of its hole has a step both down and up, and the stair
/// is perfectly climbable. So the opening's standable cells are taken as one
/// floor: a body may walk across it (and into the stairwell `DW0836` admits
/// beside a stair's hole, which lies in the same wall) before it steps out. The
/// body's width is the step rule's own footprint, so no second width is asked.
fn portal_crossing(
    s: &PlacedSeam,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    binding.portals += 1;
    let n_dir = s.face.vector()[s.normal_axis];
    let (a_off, b_off) = (-n_dir, n_dir);
    // The higher place, which is all a `drop` owes (see `contact_profile`).
    let high_off = if s.rise <= 0 { a_off } else { b_off };
    let drop = s.class == "drop";
    let (slo, shi) = s.shared;
    let in_wall = |c: [i32; 3]| {
        let c = [i64::from(c[0]), i64::from(c[1]), i64::from(c[2])];
        c[s.normal_axis] == s.plane && (0..3).all(|i| c[i] >= slo[i] && c[i] <= shi[i])
    };
    let side_of = |x: &[i32; 3]| i64::from(x[s.normal_axis]) - s.plane;

    // The floor of the opening: its standable cells, and every standable cell of
    // the same wall a body walks to from them without leaving the wall's plane.
    let (lo, hi) = s.opening;
    let mut floor: BTreeSet<[i32; 3]> = cells_of(lo, hi)
        .map(narrow)
        .filter(|c| world.is_standable(*c))
        .collect();
    let mut frontier: Vec<[i32; 3]> = floor.iter().copied().collect();
    while let Some(c) = frontier.pop() {
        for n in world.neighbors(c) {
            if in_wall(n) && floor.insert(n) {
                frontier.push(n);
            }
        }
    }
    let (mut onto_a, mut onto_b) = (0usize, 0usize);
    for c in &floor {
        let n = world.neighbors(*c);
        onto_a += usize::from(n.iter().any(|x| side_of(x) == a_off));
        onto_b += usize::from(n.iter().any(|x| side_of(x) == b_off));
    }
    // A hole in a floor under a `drop`: nothing under the opening holds a body,
    // so the high side is the brink — the cell over the hole has room for a
    // body, and a body stands beside it, level, to walk in from.
    let brinks = if drop && s.normal_axis == 1 {
        cells_of(lo, hi)
            .filter(|c| {
                let over = [c[0], c[1] + high_off, c[2]];
                let head = [over[0], over[1] + 1, over[2]];
                world.is_clear(narrow(over))
                    && world.is_clear(narrow(head))
                    && [(-1, 0), (1, 0), (0, -1), (0, 1)].iter().any(|(dx, dz)| {
                        world.is_standable(narrow([over[0] + dx, over[1], over[2] + dz]))
                    })
            })
            .count()
    } else {
        0
    };
    let onto_high = if high_off == a_off { onto_a } else { onto_b };
    let crosses = if drop {
        onto_high > 0 || brinks > 0
    } else {
        onto_a > 0 && onto_b > 0
    };
    binding.portal_floor += floor.len();
    if crosses {
        return;
    }
    let owed = if drop {
        format!(
            "a `drop` owes only its high side, `{h}`: a body walks into the opening from it, or, \
             through a floor, to the brink over the hole ({brinks} brink cell(s))",
            h = if high_off == a_off { &s.a } else { &s.b },
        )
    } else {
        "a body standing in it must step onto standable ground on BOTH sides".to_string()
    };
    raise(
        d,
        DW_PORTAL_UNCROSSABLE,
        Diagnostic::error(
            DW_PORTAL_UNCROSSABLE,
            "site-plan",
            format!("/content/seams[{}]", s.edge),
            format!(
                "nothing crosses the {class} opening the plan allocated for `{id}`. Every cell of \
                 it is clear, at x {x0}..{x1} y {y0}..{y1} z {z0}..{z1} between `{a}` and `{b}`, \
                 and {owed}. Of the {k} cell(s) a body can stand on in the opening (with the \
                 stairwell beside it in the same wall), {na} step into `{a}` and {nb} into `{b}` \
                 — measured under the compiler's step rule with every bar open. What stands \
                 past the hole is massing or a fall a body cannot walk, so a party that opens \
                 this way meets a wall, and where another way joins these places every route \
                 proof goes round by it and stays green. The repair is in the plan: move this seam along \
                 its face (`at`), or move what stands flush behind it — a stair laid in either \
                 place moves with its own edge's `at`/`meets`, and a detailed place's piece is \
                 re-detailed — so that the opening meets floor on the side that has none.",
                class = s.class,
                id = s.edge,
                a = s.a,
                b = s.b,
                k = floor.len(),
                na = onto_a,
                nb = onto_b,
                x0 = lo[0],
                x1 = hi[0],
                y0 = lo[1],
                y1 = hi[1],
                z0 = lo[2],
                z1 = hi[2],
            ),
        ),
    );
}

/// **The part of a floor's unallocated opening that is a stair's stairwell** —
/// claim 2's one admission, read off the bytes.
///
/// A stair up through a floor walks back under the floor it pierces, so the
/// derivation cuts that floor wherever a body climbing the run needs it gone
/// (`blockout::stairwell`). Those cells are outside the hole the plan allocated
/// and they are the same way, not a second one. This is **not** a replay of the
/// derivation: it does not know where the treads are or which pitch was chosen.
/// It admits an open cell of the wall only when both hold over the built world:
///
/// - **a body on the stair uses it**: a body stands with its feet in it, or its
///   head, or it is the cell a body standing on a raised tread two under it
///   sweeps when it jumps. The raised-tread condition is what keeps a hole over
///   bare floor a leak: a cell two over a place's walk plane is ceiling, not
///   headroom.
/// - **it opens off a stair's own hole**: it is joined, across the wall, through
///   cells that also hold the first condition, to the opening of a seam in this
///   wall that the plan hosts a stair for.
///
/// A floor cut wider than the climb needs — over a course a body's head never
/// reaches — holds neither, and stays a leak.
fn stairwell_of(
    boxes: &[PlacedBox],
    group: &[&PlacedSeam],
    open: &[[i64; 3]],
    world: &crate::compiler::nav::World,
) -> BTreeSet<[i64; 3]> {
    let wells: Vec<&PlacedSeam> = group
        .iter()
        .copied()
        .filter(|s| s.normal_axis == 1 && s.stair_in.is_some())
        .collect();
    if wells.is_empty() {
        return BTreeSet::new();
    }
    // The walk plane of the place under the floor: the plan's, not the
    // derivation's — a body standing above it is standing on something raised.
    let under: BTreeMap<&str, i64> = boxes.iter().map(|b| (b.node.0.as_str(), b.floor)).collect();
    let plane = wells
        .iter()
        .filter_map(|s| s.stair_in.as_ref().and_then(|h| under.get(h.0.as_str())))
        .copied()
        .min();
    let used = |c: [i64; 3]| -> bool {
        let at = |dy: i64| narrow([c[0], c[1] - dy, c[2]]);
        world.is_standable(at(0))
            || world.is_standable(at(1))
            || (world.is_standable(at(2)) && plane.is_some_and(|p| c[1] - 2 > p))
    };
    let candidates: BTreeSet<[i64; 3]> = open.iter().copied().filter(|c| used(*c)).collect();
    let mut well: BTreeSet<[i64; 3]> = BTreeSet::new();
    let mut frontier: Vec<[i64; 3]> = wells
        .iter()
        .flat_map(|s| cells_of(s.opening.0, s.opening.1))
        .collect();
    while let Some(c) = frontier.pop() {
        for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let n = [c[0] + dx, c[1], c[2] + dz];
            if candidates.contains(&n) && well.insert(n) {
                frontier.push(n);
            }
        }
    }
    well
}

/// **A place's walk plane, as built** — the byte-side reading of where a body's
/// feet land in it.
///
/// A measurement, not a lookup: the plan says where the floor was meant to be
/// and this says where it is. Two rules, and the second is what makes the first
/// safe.
///
/// 1. **Inside the declared play space, the plane is the LOWEST level a body can
///    stand at.** That is what a floor is: treads, plinths and whatever else the
///    massing puts in a room stand above it, never below.
/// 2. **Only when the declared space offers no footing at all** does the search
///    look below it, and then it takes the standable level NEAREST the declared
///    floor rather than the lowest one. That case is the one the rule exists
///    for — a floor course laid a block or two low leaves a body standing just
///    under the declaration, and a window that stopped at the declaration would
///    find nothing, report the place as *unreachable* (`DW0837`), and never run
///    the check that names the real defect (`DW0836`'s realized rise), because
///    that check skips a place with no plane. One defect would produce the wrong
///    diagnostic and silence the right one.
///
/// Taking the nearest rather than the lowest is what stops the search falling
/// through the map. The whole's own `ground` volume is a walkable surface under
/// every place that stands on it, so a lowest-wins search over a downward margin
/// reads the ground as the room's floor: on the gallery's own plan the far hall
/// came back four blocks under its datum and `DW0836` reported a nine-block
/// climb nobody built. Cells belonging to ANOTHER place are excluded outright
/// for the same family of reason — the plan is entitled to put a place directly
/// under another (`DW0827` refuses overlap, and two boxes at different datums
/// over one footprint do not overlap), and that place's floor is not this one's.
fn built_plane(
    b: &PlacedBox,
    boxes: &[PlacedBox],
    world: &crate::compiler::nav::World,
) -> Option<i64> {
    let (lo, hi) = b.space();
    let standable = |c: [i64; 3]| !owned_by_other(boxes, b, c) && world.is_standable(narrow(c));
    if let Some(y) = cells_of(lo, hi)
        .filter(|c| standable(*c))
        .map(|c| c[1])
        .min()
    {
        return Some(y);
    }
    let margin = i64::from(b.clearance);
    cells_of([lo[0], lo[1] - margin, lo[2]], [hi[0], lo[1] - 1, hi[2]])
        .filter(|c| standable(*c))
        .map(|c| c[1])
        .max()
}

/// Is this cell inside some OTHER place's declared play space?
///
/// See [`built_plane`] for why the question is asked at all.
fn owned_by_other(boxes: &[PlacedBox], me: &PlacedBox, cell: [i64; 3]) -> bool {
    boxes.iter().any(|other| {
        if other.node == me.node {
            return false;
        }
        let (lo, hi) = other.space();
        (0..3).all(|i| cell[i] >= lo[i] && cell[i] <= hi[i])
    })
}

/// `DW0837`: every place the graph declares has a floor a body can reach.
///
/// The graph's `DW0816` proved this over topology, before any coordinate
/// existed. This proves the derivation preserved it in blocks — over the
/// compiler's own step rule, from the cell the campaign really spawns a body in,
/// through openings really cut, with every way the campaign never opens really
/// shut.
///
/// # The declared fall, and why it is seeded rather than walked
///
/// The step rule is a WALK: cardinal, one cell of rise or fall, gated on the
/// physical rise between two standing surfaces. It models no free fall, and
/// deliberately — a router that could fall would prove routes a body cannot
/// come back from. A `drop` seam is exactly such a fall, and it is *designed*:
/// the plan allocated it, `DW0831` held its depth under the policy cap, and
/// `DW0836` has just proved the hole is where the plan cut it. So the closure
/// below seeds the far side of a drop whose near side is already reached, and
/// iterates. That is the graph's own declaration carried into the bytes, the
/// same way the gating closure is — never a widening of the step rule, which
/// stays exactly what every other proof in this compiler is taken under.
fn nodes_reached(
    c: &Campaign,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return;
    };
    let by_node: BTreeMap<&str, &PlacedBox> =
        b.boxes.iter().map(|x| (x.node.0.as_str(), x)).collect();
    let Some(entry) = by_node.get(graph.entry.0.as_str()).copied() else {
        return; // `DW0824` refused the plan; there is no body to start.
    };

    let bound = delvewright_dsl::bound_places(c);
    let seat = |x: &PlacedBox| seat_in(x, b, world, &bound);
    let mut seeds: Vec<[i32; 3]> = vec![seat(entry)];
    // What the graph's monotone closure grants — the same reading
    // `seal_unopened` takes of which barred ways ever open — decides which
    // carries are ever live.
    let carry_grants = {
        let grants = delvewright_dsl::layout::Grants::of(c, graph);
        delvewright_dsl::layout::Closure::run(graph, &grants).obtained
    };
    let mut reached: BTreeSet<[i32; 3]> = BTreeSet::new();
    loop {
        let before = reached.len();
        let seeded = seeds.len();
        reached.extend(world.reachable_walkable(&seeds));
        // Every declared fall whose near side is now stood in hands the far side
        // a starting cell.
        for s in &b.seams {
            if s.class != "drop" {
                continue;
            }
            let Some(edge) = graph.edges.iter().find(|e| e.id() == &s.edge) else {
                continue;
            };
            let falls = match edge {
                delvewright_dsl::layout::Edge::Drop { falls, .. } => *falls,
                _ => continue,
            };
            let (from, to) = match falls {
                delvewright_dsl::layout::Direction::AToB => (&s.a, &s.b),
                delvewright_dsl::layout::Direction::BToA => (&s.b, &s.a),
            };
            let (Some(from), Some(to)) = (
                by_node.get(from.0.as_str()).copied(),
                by_node.get(to.0.as_str()).copied(),
            ) else {
                continue;
            };
            if !stands_in(from, &b.boxes, world, &reached) {
                continue;
            }
            let landing = seat(to);
            if !reached.contains(&landing) && !seeds.contains(&landing) {
                seeds.push(landing);
            }
        }
        // Every declared carry (spec-0083 §7) whose near side is stood in, and
        // whose gating the graph's own closure grants, hands the far side a
        // starting cell — the same seeding a declared fall gets. A carry has
        // no geometry for this battery to judge: the link that realises it is
        // the route proof's (`DW0932`), and that the graph and the links agree
        // is `DW0934`'s.
        for e in &graph.edges {
            let delvewright_dsl::layout::Edge::Carry { a, b: far, .. } = e else {
                continue;
            };
            if !delvewright_dsl::layout::Closure::satisfied(e.gating(), &carry_grants) {
                continue;
            }
            let mut ways = Vec::new();
            if e.direction() != Some(delvewright_dsl::layout::Direction::BToA) {
                ways.push((a, far));
            }
            if e.direction() != Some(delvewright_dsl::layout::Direction::AToB) {
                ways.push((far, a));
            }
            for (from, to) in ways {
                let (Some(from), Some(to)) = (
                    by_node.get(from.0.as_str()).copied(),
                    by_node.get(to.0.as_str()).copied(),
                ) else {
                    continue;
                };
                if !stands_in(from, &b.boxes, world, &reached) {
                    continue;
                }
                let landing = seat(to);
                if !reached.contains(&landing) && !seeds.contains(&landing) {
                    seeds.push(landing);
                }
            }
        }
        if reached.len() == before && seeds.len() == seeded {
            break; // fixpoint: no walk, no declared fall and no carry added anything.
        }
    }

    for x in &b.boxes {
        binding.nodes += 1;
        if stands_in(x, &b.boxes, world, &reached) {
            continue;
        }
        let (lo, hi) = x.space();
        let footing = cells_of(lo, hi)
            .filter(|cell| world.is_standable(narrow(*cell)))
            .count();
        raise(
            d,
            DW_NODE_UNREACHED,
            Diagnostic::error(
                DW_NODE_UNREACHED,
                "site-plan",
                format!("/content/boxes[{}]", x.node),
                format!(
                    "no body can reach `{node}` in the built world. The place offers {footing} \
                 standable cell(s) inside x {x0}..{x1} y {y0}..{y1} z {z0}..{z1}, and none of \
                 them is reachable from the campaign's entry over the step rule, with every way \
                 the campaign's own gating never opens shut. The layout graph proved this place \
                 reachable over topology before any coordinate existed, so what has failed is the \
                 embedding or the massing, not the design: either a seam onto it was cut \
                 somewhere a body cannot enter from, or its climb was built at a pitch a body \
                 cannot take. Of {total} place(s), {n} are reached.",
                    node = x.node,
                    x0 = lo[0],
                    x1 = hi[0],
                    y0 = lo[1],
                    y1 = hi[1],
                    z0 = lo[2],
                    z1 = hi[2],
                    total = b.boxes.len(),
                    n = b
                        .boxes
                        .iter()
                        .filter(|y| stands_in(y, &b.boxes, world, &reached))
                        .count(),
                ),
            ),
        );
    }
}

/// **Where a body stands in this place, as the BUILT world has it.**
///
/// One helper and two callers — the reachability proof and the pacing
/// measurement — because both ask the same question, and answering it twice is
/// how one of them comes to be right. It was: the seat was widened for
/// `DW0837` and not for `DW0822`, and the pacing router went on aiming at the
/// plan's centre, which in a detailed place is wherever the piece happened to
/// put its furniture. It reported a leg as unroutable while the place beside it
/// was proven reached.
///
/// The derivation's own footing is preferred, and for an unbound box it is the
/// ONLY answer — see the guard below for why that is a guarantee rather than an
/// optimisation. A DETAILED box has no derived mass inside its frame at all —
/// the piece's bytes are its floor — so the derivation's footing there is its
/// documented fallback, the plan's centre, which the piece may legitimately have
/// built a wall on. The search is what makes the answer a fact about the world
/// rather than about massing that is no longer there.
fn seat_in(
    x: &PlacedBox,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    bound: &BTreeSet<String>,
) -> [i32; 3] {
    let want = b.footing(&x.node).unwrap_or_else(|| narrow(x.centre()));
    // **Only a bound place searches**, and the guard is the claim above being
    // true rather than nearly true. For a massed box the derivation's footing is
    // standable by construction, so the search would be a no-op — but not
    // always: this battery runs over the world with EDITS and RELIGHT applied,
    // and an edit that filled the derived footing used to be a `DW0837`. Letting
    // the search run there would silently relocate the seat and turn a finding
    // into a pass, on a campaign that has no detail plan at all.
    if world.is_standable(want) || !bound.contains(x.node.0.as_str()) {
        return want;
    }
    standable_near(x, world, want).unwrap_or(want)
}

/// The standable cell of `b` nearest `want` **in the assembled world**,
/// ordered by Chebyshev distance then lexicographically so two runs over one
/// world choose the same cell (ADR-0006).
///
/// The same search `Mass::footing` runs over the derivation's own output, asked
/// of the world instead — which is what a detailed place needs, because its
/// floor arrived in a `.nbt` the derivation never saw.
fn standable_near(
    b: &PlacedBox,
    world: &crate::compiler::nav::World,
    want: [i32; 3],
) -> Option<[i32; 3]> {
    let (lo, hi) = b.space();
    let (lo, hi) = (narrow(lo), narrow(hi));
    let reach = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]).max(0);
    for r in 0..=reach {
        let mut best: Option<[i32; 3]> = None;
        for y in (want[1] - r).max(lo[1])..=(want[1] + r).min(hi[1]) {
            for x in (want[0] - r).max(lo[0])..=(want[0] + r).min(hi[0]) {
                for z in (want[2] - r).max(lo[2])..=(want[2] + r).min(hi[2]) {
                    let cell = [x, y, z];
                    let d = (x - want[0])
                        .abs()
                        .max((y - want[1]).abs())
                        .max((z - want[2]).abs());
                    if d != r || !world.is_standable(cell) {
                        continue;
                    }
                    if best.is_none_or(|bst| cell < bst) {
                        best = Some(cell);
                    }
                }
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Does the reached set contain a cell inside this place?
///
/// Over [`search_span`], not over the declaration: a body standing on a floor
/// the derivation laid one block low has reached the place, and saying otherwise
/// would answer a height question with a reachability refusal.
fn stands_in(
    b: &PlacedBox,
    boxes: &[PlacedBox],
    world: &crate::compiler::nav::World,
    reached: &BTreeSet<[i32; 3]>,
) -> bool {
    let (lo, hi) = b.space();
    // Down to the plane the derivation really laid, and no further: a body
    // standing on a floor laid one block low has reached the place, and saying
    // otherwise would answer a height question with a reachability refusal.
    let floor = built_plane(b, boxes, world).unwrap_or(lo[1]).min(lo[1]);
    reached.iter().any(|c| {
        let cell = [i64::from(c[0]), i64::from(c[1]), i64::from(c[2])];
        cell[0] >= lo[0]
            && cell[0] <= hi[0]
            && cell[2] >= lo[2]
            && cell[2] <= hi[2]
            && cell[1] >= floor
            && cell[1] <= hi[1]
            && !owned_by_other(boxes, b, cell)
    })
}

/// `DW0838`: two places are joined only through the seams the plan allocated.
///
/// # The spec says "every legal step", and a step-level rule cannot fire
///
/// spec-0049 §5.3 states this as *every legal step between a cell owned by one
/// box and a cell owned by another must lie within a declared seam's opening*.
/// Read literally that rule is **vacuous by construction**, and the reason is
/// the plan's own `DW0828`: two boxes that connect stand exactly one cell apart,
/// so no cell of one is ever a cardinal neighbour of a cell of the other. The
/// cell between them belongs to neither. A step-level rule quantifies over an
/// empty set and passes forever.
///
/// So the claim is made over PATHS instead, which is the same claim and can
/// fail: **delete every allocated opening from the world, and no two places may
/// still be walk-connected.** That catches the multi-cell crossings the
/// step-level form was reaching for and cannot see — a wall the massing left low
/// enough to climb, a corner two shells did not close, a roof one open place
/// lets a body onto and another lets it off — because none of those is a single
/// step between two owned cells either.
///
/// Departure recorded here rather than in a list, because this is where it is
/// made.
fn crossings(
    c: &Campaign,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let n = b.boxes.len();
    binding.pairs = n * n.saturating_sub(1) / 2;
    if n < 2 {
        return; // one place cannot be joined to another; the pair count says so.
    }
    let seam: BTreeSet<[i32; 3]> = seam_cells(&b.seams);
    // Every standable cell the whole map has, minus the ways the plan cut.
    let (rlo, rhi) = region_span(c);
    let mut open: BTreeSet<[i32; 3]> = BTreeSet::new();
    for cell in cells_of(rlo, rhi) {
        let cell = narrow(cell);
        if seam.contains(&cell) {
            continue;
        }
        if world.is_standable(cell) {
            open.insert(cell);
        }
    }
    binding.standable = open.len();

    // Flood each place's own cells and see who else is in the component.
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    for x in &b.boxes {
        let (lo, hi) = x.space();
        let starts: Vec<[i32; 3]> = cells_of(lo, hi)
            .map(narrow)
            .filter(|c| open.contains(c) && !seen.contains(c))
            .collect();
        if starts.is_empty() {
            continue;
        }
        let mut queue: std::collections::VecDeque<[i32; 3]> = starts.iter().copied().collect();
        let mut component: BTreeSet<[i32; 3]> = starts.iter().copied().collect();
        while let Some(cur) = queue.pop_front() {
            for next in world.neighbors(cur) {
                if open.contains(&next) && component.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        seen.extend(component.iter().copied());
        // Who else lives in this component?
        for y in &b.boxes {
            if y.node == x.node {
                continue;
            }
            let (ylo, yhi) = y.space();
            let Some(witness) = component
                .iter()
                .find(|c| (0..3).all(|i| i64::from(c[i]) >= ylo[i] && i64::from(c[i]) <= yhi[i]))
            else {
                continue;
            };
            raise(
                d,
                DW_CROSSING_UNALLOCATED,
                Diagnostic::error(
                    DW_CROSSING_UNALLOCATED,
                    "site-plan",
                    "/content/seams",
                    format!(
                        "`{a}` and `{b}` are joined by geometry the plan allocated no seam for. With \
                     every one of the {s} allocated opening(s) removed from the world, a body \
                     standing in `{a}` can still walk to {w:?}, which is inside `{b}`. **Seams \
                     are allocated, not discovered**: a way that exists because a wall came out \
                     low, a corner did not close or a roof turned out to be standable is a \
                     connection nothing in the design agreed to and nothing downstream can name — \
                     not the graph, not the pacing projection, not the bot. {n} standable cell(s) \
                     were classified over {p} place pair(s) to find this.",
                        a = x.node,
                        b = y.node,
                        s = b.seams.len(),
                        w = witness,
                        n = open.len(),
                        p = binding.pairs,
                    ),
                ),
            );
        }
    }
}

/// The cells `DW0838` sweeps: **the site plan's own declared region**, grown by
/// one cell on every side.
///
/// The region and not the boxes' own extents, and the difference is the check's
/// correctness rather than its cost. A crossing the plan did not allocate is by
/// definition somewhere the plan did not put a place — over a roof, along the
/// top of the whole's own mass, round the outside of two courtyards — so a sweep
/// bounded by the boxes would be looking only where the answer cannot be. The
/// region is the honest bound because `DW0826` has already refused anything the
/// plan places outside it, so the derivation writes no block beyond it; the one
/// cell of margin is for a body standing ON the region's topmost course, whose
/// feet are one above it.
fn region_span(c: &Campaign) -> ([i64; 3], [i64; 3]) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return ([0; 3], [-1; 3]);
    };
    let lo = plan.region.min;
    let hi = plan.region.max();
    (
        [lo[0] - 1, lo[1] - 1, lo[2] - 1],
        [hi[0] + 1, hi[1] + 1, hi[2] + 1],
    )
}

/// `DW0821`: a declared sightline is unobstructed.
///
/// **Warning while any box is unbound; a refusal once `details[]` binds every
/// graph node** (spec-0050 §7.6). The severity is computed from the artifact —
/// `crate::compiler::detail::fully_detailed` — rather than set by a stage marker or an
/// author flag, so there is nothing to set and nothing to forget, and no author
/// can choose the lenient reading.
///
/// The promotion is the whole of the reason the warning existed. Derived massing
/// has no landform shaping:
/// a vista that reads perfectly once the detail pass carves the ridge between
/// two places is blocked at blockout time by the shells standing in the way.
/// Refusing it now would force hand-shaped massing into the derivation — which
/// is exactly what §5.1's marked judgement reserves for walk evidence, not for a
/// check's convenience. So the fact travels to the walk sheet instead, naming
/// **every** blocking cell rather than the first, because a walk sheet that
/// names one cell of a wall has not said where the wall is.
///
/// The traversal is [`crate::compiler::nav::walk_cells`], the same exact grid walk the
/// cutscene clip is proven with — see there for why it is a DDA and not a
/// sampler.
fn sightlines(
    c: &Campaign,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    for s in &plan.sightlines {
        binding.sightlines += 1;
        let eye = crate::compiler::nav::cell_center(narrow(s.from));
        let at = crate::compiler::nav::cell_center(narrow(s.to));
        let mut blocked: Vec<[i32; 3]> = Vec::new();
        crate::compiler::nav::walk_cells(eye, at, |cell| {
            if world.blocks_camera(cell) {
                blocked.push(cell);
            }
            false
        });
        if blocked.is_empty() {
            continue;
        }
        let owed = crate::compiler::detail::fully_detailed(c);
        let shown: Vec<String> = blocked
            .iter()
            .take(12)
            .map(|c| format!("[{}, {}, {}]", c[0], c[1], c[2]))
            .collect();
        let _ = b;
        let tail = if owed {
            "Every place on this map is DETAILED, so nothing is left to carve: the vista was \
             declared, the pieces that would have opened it are all standing, and the line is \
             still solid. That is why this refuses here and only warns while any box is still \
             massed. The fix is a plan edit, a piece edit, or the whole's own carving through \
             the world-edit verbs — all authorable in this campaign."
        } else {
            "This is a WARNING and refuses nothing, because at least one place on this map is \
             still derived massing, and derived massing has no landform: a vista the detail pass \
             will carve a ridge for is blocked here by the shells themselves. It becomes a \
             refusal the moment `details[]` binds every node, which is a fact computed from the \
             campaign rather than a severity anyone selects."
        };
        let make = if owed {
            Diagnostic::error
        } else {
            Diagnostic::warning
        };
        raise(
            d,
            DW_SIGHTLINE_BLOCKED,
            make(
                DW_SIGHTLINE_BLOCKED,
                "site-plan",
                format!("/content/sightlines[{}]", s.edge),
                format!(
                    "the vista `{id}` does not read: the line from [{fx}, {fy}, {fz}] \
                 to [{tx}, {ty}, {tz}] passes through {n} solid cell(s) — {shown}{more}. {tail}",
                    id = s.edge,
                    fx = s.from[0],
                    fy = s.from[1],
                    fz = s.from[2],
                    tx = s.to[0],
                    ty = s.to[1],
                    tz = s.to[2],
                    n = blocked.len(),
                    shown = shown.join(", "),
                    more = if blocked.len() > shown.len() {
                        format!(", and {} more", blocked.len() - shown.len())
                    } else {
                        String::new()
                    },
                ),
            ),
        );
    }
}

/// `DW0833`'s **second call site**: the brief's numbers still hold once the
/// world exists.
///
/// The first site read the plan. This one re-measures the same identities off
/// the assembled bytes, so a derivation defect that moved a datum cannot hide
/// behind a plan-time green — a floor course laid one block low satisfies every
/// stage-4 check, because stage 4 never saw a block.
///
/// # Departure: `region-extent` is proven once, not twice
///
/// Four of the five measures have a byte-side referent — a box's built
/// footprint, its built headroom, the distance between two built places, and a
/// datum's realized walk plane. The fifth does not. A **region** is a
/// declaration the plan's contents must fit inside (`DW0826`); nothing is
/// required to reach its edges, and the derivation builds no object whose extent
/// it is. Re-measuring it as "the extent of whatever got built" would refuse
/// every plan that leaves a margin — which is every plan — so the check would be
/// refusing the thing the region exists to permit. Such an identity is therefore
/// evaluated once, at stage 4, and counted here as declaration-only in the
/// binding line rather than passed over in silence.
fn identities(
    c: &Campaign,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    let facts: BTreeMap<&str, &delvewright_dsl::layout::BriefFact> = c
        .geometry_brief
        .as_ref()
        .map(|g| {
            g.content
                .facts
                .iter()
                .map(|f| (f.id.0.as_str(), f))
                .collect()
        })
        .unwrap_or_default();
    let by_node: BTreeMap<&str, &PlacedBox> =
        b.boxes.iter().map(|x| (x.node.0.as_str(), x)).collect();

    for id in &plan.identities {
        binding.identities += 1;
        let Some(fact) = facts.get(id.fact.0.as_str()) else {
            continue; // `DW0824` refused the reference at stage 4.
        };
        let measured = match &id.measure {
            delvewright_dsl::siteplan::Measure::RegionExtent { .. } => {
                binding.identities_declared_only += 1;
                continue;
            }
            delvewright_dsl::siteplan::Measure::BoxExtent { node, axis } => {
                let axis = match axis {
                    delvewright_dsl::siteplan::PlanAxis::X => 0usize,
                    delvewright_dsl::siteplan::PlanAxis::Z => 2usize,
                };
                by_node
                    .get(node.0.as_str())
                    .and_then(|x| built_extent(x, world, axis))
                    .map(|v| v as f64)
            }
            delvewright_dsl::siteplan::Measure::BoxHeight { node } => by_node
                .get(node.0.as_str())
                .and_then(|x| built_height(x, &b.boxes, world))
                .map(|v| v as f64),
            delvewright_dsl::siteplan::Measure::DistanceXz { from, to } => {
                match (by_node.get(from.0.as_str()), by_node.get(to.0.as_str())) {
                    (Some(p), Some(q)) => {
                        let a = built_centre(p, world);
                        let e = built_centre(q, world);
                        Some(((e.0 - a.0).powi(2) + (e.1 - a.1).powi(2)).sqrt())
                    }
                    _ => None,
                }
            }
            delvewright_dsl::siteplan::Measure::DatumY { datum } => b
                .boxes
                .iter()
                .find(|x| {
                    plan.boxes.iter().any(|p| {
                        p.node == x.node
                            && matches!(&p.floor,
                                delvewright_dsl::siteplan::Floor::Datum(dd) if dd == datum)
                    })
                })
                .and_then(|x| built_plane(x, &b.boxes, world))
                .map(|v| v as f64),
        };
        let Some(measured) = measured else {
            // Nothing built answers this measure — a place with no footing at
            // all, whose own refusal is `DW0837`. One defect, one diagnostic.
            binding.identities_declared_only += 1;
            continue;
        };
        if holds(id.cmp, measured, fact.value) {
            continue;
        }
        raise(
            d,
            delvewright_dsl::siteplan::DW_IDENTITY_FALSE,
            Diagnostic::error(
                delvewright_dsl::siteplan::DW_IDENTITY_FALSE,
                "site-plan",
                format!("/content/identities[{}]", id.fact),
                format!(
                    "the BUILT world does not keep `{f}`: measured {measured}, and the brief asks for \
                 {cmp} {want}{unit}. The brief's sentence was: \"{note}\". The plan itself keeps \
                 this identity — stage 4 said so over the same comparison — so what disagrees is \
                 the MASS. Two things put mass in a place and only one of them is a defect: the \
                 derivation may have built it wrong — a course laid low, a ceiling somewhere the \
                 plan did not put it — or the PLAN may have given this place something to hold, a \
                 stair's treads most often, that stands in the space the brief's number claims. \
                 So read the measured figure against what the plan asked this place to carry \
                 before touching the derivation: where a place is paying for a run of treads it \
                 was never given the room for, the repair is in the plan — move the seam, host \
                 the stair in the other place, or give this one the footprint the run costs. This \
                 is the second of the identity's two call sites, and it exists exactly so that a \
                 derivation defect which moved a datum cannot hide behind a plan-time green.",
                    f = id.fact,
                    cmp = cmp_word(id.cmp),
                    want = fact.value,
                    unit = fact
                        .unit
                        .as_ref()
                        .map(|u| format!(" {u}"))
                        .unwrap_or_default(),
                    note = fact.note,
                ),
            ),
        );
    }
}

/// How a measurement must stand to its fact's value — the same five comparisons
/// stage 4 evaluates, so the two call sites cannot disagree about what `le`
/// means.
fn holds(cmp: delvewright_dsl::siteplan::Cmp, measured: f64, fact: f64) -> bool {
    use delvewright_dsl::siteplan::Cmp;
    match cmp {
        Cmp::Eq => (measured - fact).abs() < 1e-9,
        Cmp::Lt => measured < fact,
        Cmp::Le => measured <= fact,
        Cmp::Gt => measured > fact,
        Cmp::Ge => measured >= fact,
    }
}

fn cmp_word(cmp: delvewright_dsl::siteplan::Cmp) -> &'static str {
    use delvewright_dsl::siteplan::Cmp;
    match cmp {
        Cmp::Eq => "exactly",
        Cmp::Lt => "under",
        Cmp::Le => "at most",
        Cmp::Gt => "over",
        Cmp::Ge => "at least",
    }
}

/// A place's built interior extent on one world axis, measured at the TOP course
/// of its play space.
///
/// The top course rather than the walk plane, deliberately: a stair the plan
/// hosts in this box legitimately stands on the floor, and a measurement taken
/// there would report the room as narrower than it is. The top course is the one
/// course of the play space nothing is ever massed into.
fn built_extent(b: &PlacedBox, world: &crate::compiler::nav::World, axis: usize) -> Option<i64> {
    let (lo, hi) = built_span(b, world, axis)?;
    Some(hi - lo + 1)
}

/// The inclusive run of clear cells through a place's middle on one axis, at the
/// top course of its play space.
fn built_span(
    b: &PlacedBox,
    world: &crate::compiler::nav::World,
    axis: usize,
) -> Option<(i64, i64)> {
    let (lo, hi) = b.space();
    let mut probe = b.centre();
    probe[1] = hi[1];
    if !world.is_clear(narrow(probe)) {
        return None;
    }
    let (mut low, mut high) = (probe[axis], probe[axis]);
    for dir in [-1i64, 1] {
        let mut c = probe;
        loop {
            c[axis] += dir;
            if c[axis] < lo[axis] || c[axis] > hi[axis] || !world.is_clear(narrow(c)) {
                break;
            }
            if dir < 0 {
                low = c[axis];
            } else {
                high = c[axis];
            }
        }
    }
    Some((low, high))
}

/// A place's built headroom over its realized walk plane, capped at what the
/// plan declares: **the tallest stack of clear cells standing over that plane at
/// any column of the footprint.**
///
/// The cap is what makes a sky-open place answerable: an open place makes no
/// claim on the air above its own headroom, so counting upward past it would be
/// measuring the sky. A closed place never reaches the cap unless its ceiling is
/// where the plan put it, which is the disagreement this measure exists to find.
///
/// # Over the footprint, and not at one column
///
/// This counted upward from the box's CENTRE, which is the one cell a plan is
/// most likely to have put something in: a stair hosted here arrives at its seam
/// and walks back through the middle of the room, so on the fixture's own hall
/// the centre column is a tread and the measure answered **0** for a room whose
/// ceiling is exactly where the plan put it. Nothing about that answer was wrong
/// as arithmetic and everything about the question was — the identity asks how
/// tall the place is, and a column is not a place.
///
/// The maximum, rather than the minimum or a sample: a place this derivation
/// builds has a FLAT ceiling, so every column carrying no massing answers the
/// same number and that number IS the height. Massing the plan itself put here —
/// treads, most often — only ever answers *less*, so it cannot inflate the
/// reading, and the minimum would have the mirror-image defect of the centre
/// column with none of its luck. What the maximum does not promise is that a
/// body has this much air EVERYWHERE in the place; nothing asks that, and
/// `DW0837` is what proves a place walkable.
///
/// The sibling measure had already met this and said so: [`built_extent`] moved
/// to the top course of the play space precisely because a stair the plan hosts
/// stands on the floor. This is that reasoning arriving on the vertical axis,
/// where the top course is not available to hide in.
fn built_height(
    b: &PlacedBox,
    boxes: &[PlacedBox],
    world: &crate::compiler::nav::World,
) -> Option<i64> {
    let plane = built_plane(b, boxes, world)?;
    let cap = i64::from(b.clearance);
    let (lo, hi) = b.space();
    let mut best = 0i64;
    for z in lo[2]..=hi[2] {
        for x in lo[0]..=hi[0] {
            let mut n = 0i64;
            while n < cap && world.is_clear(narrow([x, plane + n, z])) {
                n += 1;
            }
            if n >= cap {
                return Some(cap);
            }
            best = best.max(n);
        }
    }
    Some(best)
}

/// The centre of a place's built interior, on the two horizontal axes.
fn built_centre(b: &PlacedBox, world: &crate::compiler::nav::World) -> (f64, f64) {
    let mid = |axis: usize, fallback: i64| {
        built_span(b, world, axis).map_or(fallback as f64, |(lo, hi)| (lo as f64 + hi as f64) / 2.0)
    };
    let c = b.centre();
    (mid(0, c[0]), mid(2, c[2]))
}

/// `DW0822`'s **second call site**: the route the critical path really is, in
/// blocks, measured over the built world.
///
/// The stage-3 site printed a PROJECTION — nominal traverse lengths from the
/// size-class ladder, summed and divided by an uncalibrated coefficient. This
/// prints the MEASUREMENT: the A* route a body actually walks from one place's
/// anchor to the next, over the blockout, under the compiler's own step rule. It
/// carries no threshold either, and for the same reason — the two numbers exist
/// to be set side by side, which is the only way the coefficient gets calibrated
/// at all.
fn pacing(
    c: &Campaign,
    b: &Blockout,
    world: &crate::compiler::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        return;
    };
    let by_node: BTreeMap<&str, &PlacedBox> =
        b.boxes.iter().map(|x| (x.node.0.as_str(), x)).collect();
    let bound = delvewright_dsl::bound_places(c);
    let mut blocks = 0usize;
    let mut unrouted: Vec<String> = Vec::new();
    for pair in graph.critical_path.windows(2) {
        let (Some(from), Some(to)) = (
            by_node.get(pair[0].0.as_str()).copied(),
            by_node.get(pair[1].0.as_str()).copied(),
        ) else {
            continue;
        };
        binding.legs += 1;
        let (a, z) = (
            seat_in(from, b, world, &bound),
            seat_in(to, b, world, &bound),
        );
        match world.find_path(a, z) {
            Some(path) => blocks += path.len().saturating_sub(1),
            None => unrouted.push(format!("`{}` → `{}`", pair[0], pair[1])),
        }
    }
    if binding.legs == 0 {
        return; // the zero is stated in the binding line; a count is not a fault.
    }
    let table = Metrics::table();
    let mut reads = Reads::new();
    let Ok(entry) = table.resolve(MetricKind::Pacing, "route-blocks-per-minute") else {
        return;
    };
    let MetricValue::Count(rate) = entry.value(&mut reads) else {
        return;
    };
    let rate = u64::from(*rate).max(1);
    raise(
        d,
        delvewright_dsl::layout::DW_PACING,
        Diagnostic::warning(
            delvewright_dsl::layout::DW_PACING,
            "site-plan",
            "/content/boxes",
            format!(
                "the critical path MEASURES {blocks} block(s) of route over {legs} leg(s) of the \
             built blockout, which at {rate} blocks of route per minute of play is about \
             {minutes} minute(s){un}. Like the projection printed over the graph, this figure \
             carries NO threshold and refuses nothing: the coefficient is uncalibrated until the \
             metrics gym has been walked and a full playtest has run. The two are printed so they \
             can be set side by side — the projection is what the size-class ladder says the map \
             should cost, and this is what it costs.",
                legs = binding.legs,
                minutes = (blocks as u64).div_euclid(rate)
                    + u64::from(!(blocks as u64).is_multiple_of(rate)),
                un = if unrouted.is_empty() {
                    String::new()
                } else {
                    format!(
                        ", with {} leg(s) the step rule could not route ({}) and which are therefore \
                     not in the total",
                        unrouted.len(),
                        unrouted.join(", ")
                    )
                },
            ),
        ),
    );
}

// ---------------------------------------------------------------------------
// The perturbation facility (spec-0049 §13.8)
// ---------------------------------------------------------------------------

/// **A deliberate defect the derivation is asked to build.**
///
/// This exists for one reason, and it is the reason spec-0049's acceptance
/// criterion 8 asks for: a check that replays the derivation's own arithmetic
/// agrees with it by construction, however wrong both are. `DW0836`, `DW0837`
/// and `DW0838` claim to be *independent observers* of the mass, and the only
/// way to demonstrate that claim is to make the derivation build the map wrong
/// in a named way and watch them say so. Hand-authoring the bad bytes would
/// prove something weaker — that the checks can read blocks — and would leave
/// the derivation itself untested.
///
/// # Why it is a parameter and not a hidden switch
///
/// A test-only global would be a hidden input to a function whose whole
/// property is that it is a pure function of its documents, and two tests
/// running at once would see each other's setting. So the defect is an **argument**, it is
/// public, it is documented, and the production path passes [`Perturb::none`] as
/// a literal — which `blockout_derivation_is_never_perturbed_in_production`
/// asserts, so this cannot quietly acquire a caller.
///
/// It is not an escape hatch and grants nothing: every field makes the output
/// *worse*, and the battery's whole job is to refuse the result.
///
/// # How a creator asks for one
///
/// Through [`Knob`] and `delvec build --perturb <knob>`, which is the only
/// caller outside a test. A perturbed build takes **no output directory** — the
/// parser refuses `--out` beside `--perturb` — so the demonstration cannot
/// produce a tree, cannot produce a `manifest.json`, and therefore cannot be
/// bound to a staging admission token, whatever the observer says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Perturb {
    /// Cut every seam's opening this many cells along its face's first in-plane
    /// axis, without telling the plan. Reddens `DW0836` from both directions at
    /// once: the allocated cells are still wall, and the wall is open where
    /// nothing was allocated.
    pub slide_openings: i64,
    /// Lay this place's whole shell and interior one block lower than the plan
    /// put it. Reddens `DW0836`'s realized rise and `DW0833`'s second call site,
    /// and nothing at stage 4 — which is the point: a plan-time green cannot see
    /// a datum the derivation moved.
    pub sink: Option<String>,
    /// Build every shell wall one course tall instead of to the play space's
    /// full height. Reddens `DW0838`: a body can hop the wall between two places
    /// and drop into the next, which is a way nothing allocated.
    pub short_walls: bool,
    /// Leave this place's interior solid. Reddens `DW0837`: the place exists,
    /// its seams are cut, and there is nowhere in it to stand.
    pub brick_up: Option<String>,
    /// Close this place one course lower than the plan put its ceiling, leaving
    /// its floor, its walls and every opening exactly where they are. Reddens
    /// `DW0833`'s second call site on a `box-height` identity and NOTHING else —
    /// no datum moves, so `DW0836`'s realized rise is untouched, and the place
    /// stays walkable, so `DW0837` is untouched. That narrowness is the point:
    /// it is a defect only the headroom measure can see.
    pub low_ceiling: Option<String>,
    /// Fill every **contact's** span with wall, as if the massing had closed the
    /// front the plan allocated. Reddens `DW0877`: the plan says two places meet
    /// along this span and the world has a wall there.
    ///
    /// It is the only thing that can produce that red, which is what makes it a
    /// demonstration rather than a reassurance. A portal's cells are untouched,
    /// so `DW0836`'s claim 1 stays green; the wall it writes is inside the span,
    /// so `DW0836`'s claim 2 (nothing passable OUTSIDE the allocation) stays
    /// green; and closing a way can only ever remove crossings, so `DW0838`
    /// stays green. What it can also reach is `DW0837`, and only when the front
    /// is the sole way into a place — which is a fact about the fixture, not
    /// about the knob, and the test that uses it says which.
    pub wall_contacts: bool,
    /// Cut the floor over **every** course of every through-floor run, whether a
    /// climbing body needs it gone or not. Reddens `DW0836`'s claim 2: the
    /// cells over the low courses are a hole no body on the stair uses, so they
    /// are a floor opened wider than the plan allocated and not a stairwell.
    /// It is what shows the stairwell admission refuses something — a claim 2
    /// that admitted any hole over a stair would pass it.
    pub open_stairwells: bool,
    /// Wall the cells flush behind every **barred** portal's opening, on its `b`
    /// side, over the opening's own span. Reddens `DW0986`: the bar opens onto
    /// a wall, which is the shape a stair's treads laid across a doorway take.
    ///
    /// It is the only thing that can produce that red. The opening's cells are
    /// untouched, so `DW0836`'s claim 1 stays green; the wall it writes is off
    /// the shared wall's plane, so claim 2 stays green; closing a way only
    /// removes crossings, so `DW0838` stays green. What it can also reach is
    /// `DW0837`, and only when a barred door is the sole way into its `b` place
    /// — the gallery's far hall is also entered through the annex chute.
    pub bury_barred: bool,
}

impl Perturb {
    /// The derivation as it ships: no defect at all.
    #[must_use]
    pub const fn none() -> Perturb {
        Perturb {
            slide_openings: 0,
            sink: None,
            short_walls: false,
            brick_up: None,
            low_ceiling: None,
            wall_contacts: false,
            open_stairwells: false,
            bury_barred: false,
        }
    }

    /// True when this asks for nothing — what the production path passes.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Perturb::none()
    }

    /// How far this place's mass is displaced downward.
    fn drop_of(&self, node: &NodeId) -> i64 {
        i64::from(self.sink.as_deref() == Some(node.0.as_str()))
    }

    /// The place this asks for, if it asks for one — so a caller can check the
    /// name against the plan before deriving anything.
    #[must_use]
    pub fn place(&self) -> Option<&str> {
        self.sink
            .as_deref()
            .or(self.brick_up.as_deref())
            .or(self.low_ceiling.as_deref())
    }
}

/// **Every defect the derivation can be asked for, named.**
///
/// The enumeration exists so that the surface a creator reaches — `delvec build
/// --perturb <knob>` — is derived from [`Perturb`] rather than written beside
/// it. A field of `Perturb` with no arm here is caught at COMPILE time by
/// `every_perturb_field_has_a_knob`, which destructures the struct exhaustively:
/// adding a seventh defect and forgetting to name it does not compile.
///
/// `slide_openings` is an `i64` and this offers the one-cell case, because the
/// smallest slip is the strongest demonstration — an observer that catches a
/// hole cut one cell over catches every larger miss for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// [`Perturb::slide_openings`] by one cell.
    SlideOpenings,
    /// [`Perturb::sink`] — takes a place.
    Sink,
    /// [`Perturb::short_walls`].
    ShortWalls,
    /// [`Perturb::brick_up`] — takes a place.
    BrickUp,
    /// [`Perturb::low_ceiling`] — takes a place.
    LowCeiling,
    /// [`Perturb::wall_contacts`].
    WallContacts,
    /// [`Perturb::open_stairwells`].
    OpenStairwells,
    /// [`Perturb::bury_barred`].
    BuryBarred,
}

impl Knob {
    /// Every knob, in declaration order.
    pub const ALL: [Knob; 8] = [
        Knob::SlideOpenings,
        Knob::Sink,
        Knob::ShortWalls,
        Knob::BrickUp,
        Knob::LowCeiling,
        Knob::WallContacts,
        Knob::OpenStairwells,
        Knob::BuryBarred,
    ];

    /// The kebab-case name a creator types.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Knob::SlideOpenings => "slide-openings",
            Knob::Sink => "sink",
            Knob::ShortWalls => "short-walls",
            Knob::BrickUp => "brick-up",
            Knob::LowCeiling => "low-ceiling",
            Knob::WallContacts => "wall-contacts",
            Knob::OpenStairwells => "open-stairwells",
            Knob::BuryBarred => "bury-barred",
        }
    }

    /// One line of help, for the surface that offers the knob.
    #[must_use]
    pub fn blurb(self) -> &'static str {
        match self {
            Knob::SlideOpenings => "cut every seam's opening one cell along its face",
            Knob::Sink => "lay one place's shell and interior a block low",
            Knob::ShortWalls => "build every shell wall one course tall",
            Knob::BrickUp => "leave one place's interior solid",
            Knob::LowCeiling => "close one place a course under its plan's ceiling",
            Knob::WallContacts => "wall every contact's span the plan allocated",
            Knob::OpenStairwells => "cut the floor over every course of a through-floor stair",
            Knob::BuryBarred => "wall the far side flush behind every barred door",
        }
    }

    /// Whether this defect is about ONE place, and therefore needs one named.
    #[must_use]
    pub fn takes_place(self) -> bool {
        matches!(self, Knob::Sink | Knob::BrickUp | Knob::LowCeiling)
    }

    /// Which observer this defect is DOCUMENTED to redden — the code the knob's
    /// own doc comment on [`Perturb`] names, so a surface offering the knob does
    /// not carry a second opinion about what it does.
    ///
    /// It is not a prediction of which code will stop a build. A build fails on
    /// its first refusal and one defect is routinely seen by two of these rules:
    /// walls a course tall open every wall above its allocation (`DW0836`) as
    /// well as joining two places nothing connected (`DW0838`), and `DW0836` is
    /// raised first. [`Battery::refusals`] is what says which rules actually saw
    /// it.
    #[must_use]
    pub fn documented_code(self) -> &'static str {
        match self {
            Knob::SlideOpenings => "DW0836",
            Knob::Sink => "DW0836",
            Knob::ShortWalls => "DW0838",
            Knob::BrickUp => "DW0837",
            Knob::LowCeiling => "DW0833",
            Knob::WallContacts => "DW0877",
            Knob::OpenStairwells => "DW0836",
            Knob::BuryBarred => "DW0986",
        }
    }

    /// The defect itself. `place` is required exactly when [`Self::takes_place`]
    /// is true; a caller that disagrees gets `None` rather than a silently
    /// place-less perturbation, which would derive a clean map and read as an
    /// observer that failed to observe.
    #[must_use]
    pub fn perturb(self, place: Option<&str>) -> Option<Perturb> {
        if self.takes_place() != place.is_some() {
            return None;
        }
        let p = place.map(str::to_string);
        Some(match self {
            Knob::SlideOpenings => Perturb {
                slide_openings: 1,
                ..Perturb::none()
            },
            Knob::Sink => Perturb {
                sink: p,
                ..Perturb::none()
            },
            Knob::ShortWalls => Perturb {
                short_walls: true,
                ..Perturb::none()
            },
            Knob::BrickUp => Perturb {
                brick_up: p,
                ..Perturb::none()
            },
            Knob::LowCeiling => Perturb {
                low_ceiling: p,
                ..Perturb::none()
            },
            Knob::WallContacts => Perturb {
                wall_contacts: true,
                ..Perturb::none()
            },
            Knob::OpenStairwells => Perturb {
                open_stairwells: true,
                ..Perturb::none()
            },
            Knob::BuryBarred => Perturb {
                bury_barred: true,
                ..Perturb::none()
            },
        })
    }
}

/// `--perturb`'s value set, taken from [`Knob::ALL`] rather than restated.
///
/// Written by hand rather than `#[derive(ValueEnum)]` because the derive would
/// name each value from its VARIANT identifier, which is a second spelling
/// authority beside [`Knob::name`] — and the two disagree exactly when somebody
/// renames one of them. This impl enumerates nothing of its own: the variants
/// are `Knob::ALL`, the spellings are `Knob::name`, the help lines are
/// `Knob::blurb` and `Knob::documented_code`, so a seventh defect appears in
/// `delvec build --help` the moment it exists.
///
/// It lives beside the type rather than in `main.rs` because the orphan rule
/// puts it there; `view::panorama::Bearing` already carries a `ValueEnum` in
/// this crate for the same reason.
impl clap::ValueEnum for Knob {
    fn value_variants<'a>() -> &'a [Self] {
        &Knob::ALL
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(self.name()).help(format!(
            "{} — expect {}{}",
            self.blurb(),
            self.documented_code(),
            if self.takes_place() {
                " (needs --perturb-place)"
            } else {
                ""
            }
        )))
    }
}
