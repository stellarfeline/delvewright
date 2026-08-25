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
//! Both authored documents are named because both reach the bytes, and the
//! consequence is a gate rather than a footnote: a seam is cut to air or filled
//! with the bar by its edge's `class`, and a sky-open box takes its headroom
//! from its node's `size_class`. The walk record's freshness key is therefore
//! over both (`detail::layout_graph_sha256`); a key over the plan alone let a
//! graph-only edit move the walked massing under a record that went on reading
//! as fresh.
//!
//! # Where it enters the build
//!
//! [`crate::plan::Plan::build`] calls [`derive`] once, for every campaign that
//! carries a site plan, and pushes the result as an ordinary
//! [`AreaPlacement`](crate::plan::AreaPlacement). **That is the tooth.** There is
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
//! the compiler would need a template WRITER, which lives in `delvewright-schem`
//! and is unreachable from here (`delvec` publishes to crates.io and may depend
//! only on published crates, and `schem` is `publish = false`).
//!
//! So a piece of the blockout is a [`PiecePlacement`](crate::plan::PiecePlacement)
//! with **no templates** and its blocks in
//! [`AreaPlacement::mass`](crate::plan::AreaPlacement::mass). Nothing downstream
//! special-cases it: `bbox()` reads `pos`/`size` and answers for forceload and
//! relight exactly as before, the template loop iterates an empty list, and the
//! mass fills land in [`crate::assembled::placed_blocks`] one step ahead of the
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
//! requires and [`crate::blockout`]'s tests measure.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::StationKind;
use delvewright_dsl::metrics::{
    MetricKind, MetricValue, Metrics, Pitch, Reads, passable_width_cells,
};
use delvewright_dsl::siteplan::{
    Crossing, ENTRY_ANCHOR, PlacedBox, PlacedSeam, SITE_AREA, VolumeRole, node_anchor, seam_anchor,
    seam_unlock_anchor,
};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, NodeId};
use serde::Serialize;

use crate::plan::{AnchorRole, AreaPlacement, PiecePlacement, ResolvedAnchor};
use crate::solver::{Rotation, SealFill};

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
    /// Solid mass the whole owns — the mountain a cave system is inside.
    pub const MASSIF: &str = "minecraft:deepslate";
    /// The ground the places stand on.
    pub const GROUND: &str = "minecraft:tuff";
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
    /// Consumed once, by [`crate::plan::Plan::build`]; a second consumer would
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
}

impl Binding {
    /// One line, for stderr and for the round summary.
    ///
    /// Printed by `crate::emit::build_with_warnings`, beside the battery's, on
    /// every build of a site-plan campaign. It had no caller at all until stage 6
    /// — the observer's count was stated and the builder's was not — which is the
    /// UNRUN shape at the smallest scale: a line that is correct, reviewable, and
    /// reaches nobody.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "blockout binding: {b} place(s) massed ({de} detailed, so {un} massed by the \
             derivation), {s} seam(s) cut ({st} stair, {ba} barred), {v} whole-owned volume(s), \
             {a} anchor(s) synthesized, {f} region write(s) over {c} cell(s).",
            de = self.detailed,
            un = self.boxes.saturating_sub(self.detailed),
            b = self.boxes,
            s = self.seams,
            st = self.stairs,
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
    /// does not move by a byte — [`crate::blockout::derive`] passes an empty
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
    derive_bound(c, reads, perturb, &delvewright_dsl::bound_places(c))
}

/// **The massing the WALK judged** — the derivation with nothing bound.
///
/// This is what `blockout_sha256` hashes, and the choice is *nothing bound*
/// rather than *the massing as written*: had the hash been taken over the
/// massing as actually written, binding the first place would have moved it, and
/// the drift warning would have fired on every detailed campaign — a warning
/// that always fires is a warning nobody reads.
///
/// So the hash names the object a walker walked. What that object is a function
/// of is **the site plan, the layout graph, the metrics table and the engine** —
/// the graph included, because a seam is air or bar by its edge's `class` and a
/// sky-open box takes its headroom from its node's `size_class`. Both authored
/// documents are therefore in `DW0841`'s freshness key
/// (`detail::layout_graph_sha256`), which is what leaves *toolchain movement* as
/// the only thing spec-0050 §2's drift advisory can be reporting.
#[must_use]
pub fn walked_massing(c: &Campaign, reads: &mut Reads) -> Option<Vec<SealFill>> {
    derive_bound(c, reads, Perturb::none(), &BTreeSet::new()).map(|(a, _)| a.mass)
}

/// [`derive_with`] over an explicit set of bound places — see
/// [`walked_massing`] for the second caller and why it exists.
fn derive_bound(
    c: &Campaign,
    reads: &mut Reads,
    perturb: Perturb,
    bound: &BTreeSet<String>,
) -> Option<(AreaPlacement, Blockout)> {
    c.site_plan.as_ref()?;
    let plan = &c.site_plan.as_ref()?.content;
    let table = Metrics::table();
    let boxes = delvewright_dsl::siteplan::placed_boxes(c, reads);
    let seams = delvewright_dsl::siteplan::placed_seams(c, &boxes, reads);
    let by_node: BTreeMap<&str, &PlacedBox> =
        boxes.iter().map(|b| (b.node.0.as_str(), b)).collect();

    // The frames a binding owns, in plan document order. `Mass::holes` is what
    // the fabric split IS; everything below writes as it always did.
    let holes: Vec<([i64; 3], [i64; 3])> = boxes
        .iter()
        .filter(|b| bound.contains(b.node.0.as_str()))
        .map(|b| {
            let f = delvewright_dsl::Frame::of(b);
            (f.lo, f.hi)
        })
        .collect();
    let detailed = holes.len();
    let mut mass = Mass::new(holes);
    let mut pieces: Vec<PiecePlacement> = Vec::new();

    // (1) The whole's own mass.
    for v in &plan.volumes {
        let lo = v.region.min;
        let hi = v.region.max();
        let block = match v.role {
            VolumeRole::Massif => palette::MASSIF,
            VolumeRole::Ground => palette::GROUND,
            VolumeRole::Clearance => palette::AIR,
        };
        mass.write(lo, hi, block);
        pieces.push(piece(
            format!("blockout/{}", v.id.0),
            lo,
            [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1],
        ));
    }

    // (2) Every place's shell.
    for (i, b) in boxes.iter().enumerate() {
        let sink = perturb.drop_of(&b.node);
        let (mut lo, mut hi) = shell(b);
        lo[1] -= sink;
        hi[1] -= sink;
        // Floor course: the accent, so the colour under a body's feet names the
        // place it is standing in.
        mass.write(
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], hi[2]],
            palette::accent(i),
        );
        // Four walls, from the walk plane to the top of the play space.
        let (wy0, wy1) = (
            b.floor - sink,
            if perturb.short_walls {
                b.floor - sink
            } else {
                b.floor - sink + i64::from(b.clearance) - 1
            },
        );
        for (x0, x1, z0, z1) in [
            (lo[0], lo[0], lo[2], hi[2]),
            (hi[0], hi[0], lo[2], hi[2]),
            (lo[0] + 1, hi[0] - 1, lo[2], lo[2]),
            (lo[0] + 1, hi[0] - 1, hi[2], hi[2]),
        ] {
            mass.write([x0, wy0, z0], [x1, wy1, z1], palette::WALL);
        }
        // What closes it overhead. A sky-open place claims the ground and its
        // class's own headroom and NOTHING above that, so it gets no course.
        if !b.open {
            mass.write(
                [lo[0], hi[1], lo[2]],
                [hi[0], hi[1], hi[2]],
                palette::CEILING,
            );
        }
        pieces.push(piece(
            format!("blockout/{}", b.node.0),
            lo,
            [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1],
        ));
    }

    // (3) Every place's interior, cleared — see the ordering note above.
    for b in &boxes {
        let sink = perturb.drop_of(&b.node);
        let (mut lo, mut hi) = b.space();
        lo[1] -= sink;
        hi[1] -= sink;
        let block = if perturb.brick_up == Some(b.node.0.as_str()) {
            palette::WALL
        } else {
            palette::AIR
        };
        mass.write(lo, hi, block);
        // A ceiling laid one course into the play space. Written here rather
        // than in (2) because (2)'s course sits ABOVE the play space and this
        // pass would clear anything laid inside it — the defect has to survive
        // the clear to be a defect at all.
        if perturb.low_ceiling == Some(b.node.0.as_str()) {
            mass.write(
                [lo[0], hi[1], lo[2]],
                [hi[0], hi[1], hi[2]],
                palette::CEILING,
            );
        }
    }

    // (4) Every PORTAL's frame.
    //
    // A contact gets none, and that is what a contact IS: the boundary is
    // continuous ground and the derivation writes no wall along the span
    // (spec-0053 §4). A frame ring around a 55-cell front would be a wall drawn
    // in a second block — the exact thing the span says is not there — and it
    // would stand in every column the crossing profile is measured over.
    let mut anchors: BTreeMap<String, AnchorSpec> = BTreeMap::new();
    for s in &seams {
        if s.crossing == Crossing::Contact {
            continue;
        }
        for (flo, fhi) in frame_ring(s) {
            mass.write(flo, fhi, palette::FRAME);
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
    for s in &seams {
        let Some(host_id) = &s.stair_in else { continue };
        if bound.contains(host_id.0.as_str()) {
            continue;
        }
        let Some(host) = by_node.get(host_id.0.as_str()).copied() else {
            continue;
        };
        if tread(&mut mass, s, host, &table, reads) {
            stairs += 1;
        }
    }

    // (6) **The openings, last.** A stair arrives AT its seam, so its top course
    // sits directly under or beside the hole — and a course written after the
    // hole was cut fills it back in. Cutting last is what makes *the opening the
    // plan allocated is open* an invariant of the derivation rather than a
    // property of which pass happened to run second: the massing may do what it
    // likes, and the hole is the last word.
    for s in &seams {
        let (olo, ohi) = slide(s, perturb.slide_openings);
        if s.class == "barred" {
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
        barred: seams.iter().filter(|s| s.class == "barred").count(),
        volumes: plan.volumes.len(),
        anchors: anchors.len(),
        fills: mass.fills.len(),
        cells: mass.cells,
    };
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
            binding,
        },
    ))
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
    }
}

/// A place's shell: the play space grown by one cell on every side.
///
/// The one cell is the wall, and it is the same cell for two connected places —
/// `DW0828` allocates a seam only on a face whose two boxes stand exactly one
/// apart, so their shells share that column and the derivation writes it twice
/// with the same block rather than arbitrating between two.
fn shell(b: &PlacedBox) -> ([i64; 3], [i64; 3]) {
    let (lo, hi) = b.space();
    (
        [lo[0] - 1, lo[1] - 1, lo[2] - 1],
        [hi[0] + 1, hi[1] + 1, hi[2] + 1],
    )
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
/// Returns whether anything was laid: a seam whose two places are on one plane
/// has no climb (`DW0830` refuses that as a mislabelled walk), and a seam the
/// plan could not resolve a host for is a `DW0824`.
///
/// # The pitch, and a departure recorded where it is made
///
/// The pitch is the **gentlest standard the host affords**, chosen by the same
/// walk over the same table `DW0830` refuses with — so a plan that reached green
/// is a plan this can build, and one that could not would have been refused
/// before any block existed.
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
) -> bool {
    if s.rise == 0 {
        return false;
    }
    let (lo, hi) = host.space();
    let (olo, ohi) = s.opening;

    // **How high the treads must carry a body**, and it is read off the opening
    // rather than off the rise, because the opening is what the body has to
    // reach:
    //
    // * across a **vertical** face the body must stand with its feet at the
    //   seam's own sill and step through, so the top course's face is the sill;
    // * through a **floor or ceiling** the body must stand INSIDE the hole and
    //   step out onto the other place's floor beside it, so the top course's
    //   face is the plane the hole is cut in.
    //
    // Both come out as the floor difference on an ordinary plan; they differ
    // exactly where the plan puts a stair's sill somewhere other than the far
    // floor, and the opening is the half a body actually uses.
    let target = if s.normal_axis == 1 { s.plane } else { olo[1] };
    let climb = target - host.floor;
    if climb <= 0 {
        // The host is at or above what the stair has to reach. `DW0830` refuses
        // that plan by name — treads rise off a walk plane and the only one this
        // stair has is the lower place's — so there is nothing to build here and
        // nothing to say about it that has not been said.
        return false;
    }

    // The run is horizontal. Across a vertical face it is spent along that
    // face's normal; through a floor or a ceiling it may run either way, so the
    // host's longer horizontal axis is what it has. Same axis rule `DW0830`
    // measures the plan against.
    let run_axis = if s.normal_axis == 1 {
        let ex = host.foot[1] - host.foot[0] + 1;
        let ez = host.foot[3] - host.foot[2] + 1;
        if ex >= ez { 0usize } else { 2 }
    } else {
        s.normal_axis
    };

    // **Where the run starts, how it walks, and how much of it there really
    // is** — one decision, because the third answer depends on the first two.
    //
    // **The stair arrives AT its seam**, so the top course is the one the body
    // steps off from and the run walks back into the room. Across a vertical
    // face that is the course against the wall, and the run walks the whole
    // footprint, so the host's extent on this axis IS what it affords.
    //
    // Through a FLOOR or a CEILING it is the course under the hole, and the run
    // leaves along whichever side of the hole the host has more of — so what
    // the treads have is the room on THAT side plus the hole's own width, never
    // the host's whole extent. Measuring the extent here measures the room on
    // both sides of a stair that only ever runs down one, which is a run the
    // host does not have; a pitch chosen against it does not fit, and the
    // courses that fall off the far wall are a stair with its bottom missing.
    let (start, step, available) = if s.normal_axis == 1 {
        let (olo_r, ohi_r) = (
            olo[run_axis].max(lo[run_axis]),
            ohi[run_axis].min(hi[run_axis]),
        );
        if olo_r > ohi_r {
            return false; // the hole is not over this host — `DW0828`'s finding.
        }
        let width = ohi_r - olo_r + 1;
        let room_lo = olo_r - lo[run_axis];
        let room_hi = hi[run_axis] - ohi_r;
        if room_lo >= room_hi {
            (ohi_r, -1, room_lo + width)
        } else {
            (olo_r, 1, room_hi + width)
        }
    } else if s.plane > host.foot[if run_axis == 0 { 1 } else { 3 }] {
        (hi[run_axis], -1, hi[run_axis] - lo[run_axis] + 1)
    } else {
        (lo[run_axis], 1, hi[run_axis] - lo[run_axis] + 1)
    };

    let Some(pitch) = gentlest_pitch(table, reads, climb, available) else {
        return false; // `DW0830` refused this plan; there is no standard to build.
    };
    let courses = ceil_div(climb * i64::from(pitch.run), i64::from(pitch.rise)).max(1);

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
        return false;
    }

    // Which cells across the run the treads occupy: the opening's own width,
    // clipped to the host. A stair the width of its doorway is what a body can
    // actually walk up.
    let cross = if run_axis == 0 { 2 } else { 0 };
    let (clo, chi) = (olo[cross].max(lo[cross]), ohi[cross].min(hi[cross]));
    if clo > chi {
        return false;
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
    laid
}

/// Ceiling division over non-negative operands.
///
/// Spelled out rather than `i64::div_ceil`, which is unstable on this
/// toolchain — the same arithmetic `crate::plan`'s stair check uses, written the
/// same way, so the plan-time verdict and the built run agree by construction.
fn ceil_div(n: i64, d: i64) -> i64 {
    if d == 0 {
        n
    } else {
        n / d + i64::from(n % d != 0)
    }
}

/// The gentlest standard pitch whose run fits `available`, or `None` when the
/// table defines none that does.
///
/// The walk is over [`Metrics::names_of`] in table order and returns the first
/// that fits — the identical rule `DW0830` refuses with, read from the identical
/// table, so the plan-time verdict and the built geometry cannot be about
/// different standards.
fn gentlest_pitch(table: &Metrics, reads: &mut Reads, rise: i64, available: i64) -> Option<Pitch> {
    for name in table.names_of(MetricKind::Pitch) {
        let Ok(entry) = table.resolve(MetricKind::Pitch, name) else {
            continue;
        };
        let MetricValue::Pitch(p) = entry.value(reads) else {
            continue;
        };
        if p.rise == 0 {
            continue;
        }
        let needed = ceil_div(rise * i64::from(p.run), i64::from(p.rise));
        if needed <= available {
            return Some(*p);
        }
    }
    None
}

/// Every cell a set of region writes covers, for a caller that needs the whole
/// blockout as a cell set (the stage-5 battery's ownership map).
pub fn cells_of(lo: [i64; 3], hi: [i64; 3]) -> impl Iterator<Item = [i64; 3]> {
    (lo[1]..=hi[1]).flat_map(move |y| {
        (lo[2]..=hi[2]).flat_map(move |z| (lo[0]..=hi[0]).map(move |x| [x, y, z]))
    })
}

/// The places a cell belongs to, for the battery — see `crate::blockout::check`.
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

/// `DW0836`: a built seam disagrees with its allocation.
pub const DW_SEAM_BUILT: DwCode = DwCode::every_version("DW0836");

/// `DW0837`: a node's floor is unreached.
pub const DW_NODE_UNREACHED: DwCode = DwCode::every_version("DW0837");

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
/// Build tier (exit 3), `every_version`.
pub const DW_CONTACT_UNCROSSABLE: DwCode = DwCode::every_version("DW0877");

/// `DW0838`: a connection nothing allocated.
pub const DW_CROSSING_UNALLOCATED: DwCode = DwCode::every_version("DW0838");

/// `DW0821`: a sightline is blocked. Warning in the slice — see [`sightlines`].
pub const DW_SIGHTLINE_BLOCKED: DwCode = DwCode::every_version("DW0821");

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
    /// Shared walls examined for a wider or misplaced hole — `DW0836`.
    pub walls: usize,
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
             {ct} contact(s), {cc} crossable column(s) measured), {n} place(s) \
             proven reached, {c} standable cell(s) classified over {p} place pair(s), \
             {sl} sightline(s) walked, {i} identity(ies) re-measured ({d} declaration-only), \
             {l} critical-path leg(s) measured.",
            s = self.seams,
            ct = self.contacts,
            cc = self.contact_columns,
            w = self.walls,
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
        self.findings
            .iter()
            .find(|(_, d)| d.severity == delvewright_dsl::Severity::Error)
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
/// [`crate::emit::build_with_warnings`], on every build of a campaign whose plan
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
/// The step rule is the compiler's own ([`crate::nav::World::neighbors`]), not a
/// second one written here — see that function for why its binding was widened
/// rather than copied.
#[must_use]
pub fn check(plan: &crate::plan::Plan, blocks: &BTreeMap<[i32; 3], String>) -> Option<Battery> {
    let b = plan.blockout.as_ref()?;
    let c = plan.campaign;
    let mut findings: Vec<(DwCode, Diagnostic)> = Vec::new();
    let mut binding = BatteryBinding::default();

    // The world as a body meets it once every declared way is open. `DW0836` and
    // `DW0838` are questions about GEOMETRY — is the hole the hole the plan cut,
    // and is there a second one — so they are asked with nothing shut.
    let open = crate::nav::World::from_occupancy(crate::assembled::occupancy_of(
        blocks.clone(),
        &BTreeSet::new(),
    ));
    // The world with every way the graph's own gating closure never opens sealed
    // as the plan sealed it. The base assembled model deliberately holds gate
    // regions open (`crate::assembled`), so a reachability proof taken over it
    // would walk through a door nothing in the campaign ever unlocks.
    let sealed = crate::nav::World::from_occupancy(crate::assembled::occupancy_of(
        seal_unopened(c, b, blocks),
        &BTreeSet::new(),
    ));

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
/// open*, and it is read here rather than re-derived: `crate::blockout` has no
/// opinion about quest order and should not acquire one.
fn seal_unopened(
    c: &Campaign,
    b: &Blockout,
    blocks: &BTreeMap<[i32; 3], String>,
) -> BTreeMap<[i32; 3], String> {
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
            out.insert(narrow(cell), palette::BAR.to_string());
        }
    }
    out
}

/// **The crossing profile of a contact's span**, measured over the assembled
/// bytes (spec-0053 §4).
///
/// Returns, per column of the span, the pair of walk planes a body crossing in
/// that column stands on — `(a-side, b-side)` — for every column it can cross
/// at all. An empty result is a front nothing crosses.
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
fn contact_profile(
    s: &PlacedSeam,
    world: &crate::nav::World,
) -> Vec<(i64, i64, i64)> {
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

    // Which side a `drop` falls FROM: the higher floor. `rise` is
    // `floor(b) − floor(a)`, so a negative rise puts `a` above `b`.
    let need_both = s.class != "drop";
    let high = if s.rise <= 0 { -1i64 } else { 1i64 };

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
            let (a_side, b_side) = (side(-1), side(1));
            let crosses = if need_both {
                a_side.is_some() && b_side.is_some()
            } else if high < 0 {
                a_side.is_some()
            } else {
                b_side.is_some()
            };
            if crosses {
                // The planes a body stands on either side, where the step rule
                // reached one. A drop's far side was not walked to, so it is
                // reported as the seam's own declared plane rather than
                // measured — the number `DW0836` compares is then the one
                // `DW0837` is already responsible for.
                out.push((u, a_side.unwrap_or(c[1]), b_side.unwrap_or(c[1] + s.rise)));
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
fn widest_run(profile: &[(i64, i64, i64)]) -> usize {
    let mut best = 0usize;
    let mut run = 0usize;
    let mut prev: Option<i64> = None;
    for (u, _, _) in profile {
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
    world: &crate::nav::World,
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
            // ---- Claim 3 for a contact, PER CROSSING COLUMN.
            //
            // A front is wide enough that one number for the whole of it would
            // be a claim about its middle. A landform that tilted one side
            // leaves the two places' own walk planes agreeing and the crossing
            // disagreeing, which is precisely what an independent observer is
            // for.
            if let Some((u, pa, pb)) = profile.iter().find(|(_, pa, pb)| pb - pa != s.rise) {
                raise(
                    d,
                    DW_SEAM_BUILT,
                    Diagnostic::error(
                        DW_SEAM_BUILT,
                        "site-plan",
                        format!("/content/seams[{}]", s.edge),
                        format!(
                            "the contact for `{id}` is crossed at the wrong height. In the \
                             column at {u}, a body steps from a walk plane of {pa} to one of \
                             {pb}, a rise of {got} where the plan puts `{a}` and `{b}` \
                             {want} apart. The rise is derived from the two places' floors \
                             and is never authored, so this is the mass disagreeing with the \
                             plan: either the derivation built a course at the wrong height, \
                             or the plan gave one of the two places a floor the massing \
                             standing in it cannot honour.",
                            id = s.edge,
                            a = s.a,
                            b = s.b,
                            got = pb - pa,
                            want = s.rise,
                        ),
                    ),
                );
            }
            continue;
        }

        let blocked: Vec<[i64; 3]> = cells_of(lo, hi)
            .filter(|c| !world.is_clear(narrow(*c)))
            .collect();
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
        // Claim 3.
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
        let leaks: Vec<[i64; 3]> = cells_of(*slo, *shi)
            .filter(|c| !allowed.contains(c) && world.is_clear(narrow(*c)))
            .collect();
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
fn built_plane(b: &PlacedBox, boxes: &[PlacedBox], world: &crate::nav::World) -> Option<i64> {
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
    world: &crate::nav::World,
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
    let mut reached: BTreeSet<[i32; 3]> = BTreeSet::new();
    loop {
        let before = reached.len();
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
        if reached.len() == before {
            break; // fixpoint: no walk and no declared fall added anything.
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
    world: &crate::nav::World,
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
fn standable_near(b: &PlacedBox, world: &crate::nav::World, want: [i32; 3]) -> Option<[i32; 3]> {
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
    world: &crate::nav::World,
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
    world: &crate::nav::World,
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
/// `crate::detail::fully_detailed` — rather than set by a stage marker or an
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
/// The traversal is [`crate::nav::walk_cells`], the same exact grid walk the
/// cutscene clip is proven with — see there for why it is a DDA and not a
/// sampler.
fn sightlines(
    c: &Campaign,
    b: &Blockout,
    world: &crate::nav::World,
    binding: &mut BatteryBinding,
    d: &mut Vec<(DwCode, Diagnostic)>,
) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    for s in &plan.sightlines {
        binding.sightlines += 1;
        let eye = crate::nav::cell_center(narrow(s.from));
        let at = crate::nav::cell_center(narrow(s.to));
        let mut blocked: Vec<[i32; 3]> = Vec::new();
        crate::nav::walk_cells(eye, at, |cell| {
            if world.blocks_camera(cell) {
                blocked.push(cell);
            }
            false
        });
        if blocked.is_empty() {
            continue;
        }
        let owed = crate::detail::fully_detailed(c);
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
    world: &crate::nav::World,
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
fn built_extent(b: &PlacedBox, world: &crate::nav::World, axis: usize) -> Option<i64> {
    let (lo, hi) = built_span(b, world, axis)?;
    Some(hi - lo + 1)
}

/// The inclusive run of clear cells through a place's middle on one axis, at the
/// top course of its play space.
fn built_span(b: &PlacedBox, world: &crate::nav::World, axis: usize) -> Option<(i64, i64)> {
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
fn built_height(b: &PlacedBox, boxes: &[PlacedBox], world: &crate::nav::World) -> Option<i64> {
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
fn built_centre(b: &PlacedBox, world: &crate::nav::World) -> (f64, f64) {
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
    world: &crate::nav::World,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    pub sink: Option<&'static str>,
    /// Build every shell wall one course tall instead of to the play space's
    /// full height. Reddens `DW0838`: a body can hop the wall between two places
    /// and drop into the next, which is a way nothing allocated.
    pub short_walls: bool,
    /// Leave this place's interior solid. Reddens `DW0837`: the place exists,
    /// its seams are cut, and there is nowhere in it to stand.
    pub brick_up: Option<&'static str>,
    /// Close this place one course lower than the plan put its ceiling, leaving
    /// its floor, its walls and every opening exactly where they are. Reddens
    /// `DW0833`'s second call site on a `box-height` identity and NOTHING else —
    /// no datum moves, so `DW0836`'s realized rise is untouched, and the place
    /// stays walkable, so `DW0837` is untouched. That narrowness is the point:
    /// it is a defect only the headroom measure can see.
    pub low_ceiling: Option<&'static str>,
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
        }
    }

    /// True when this asks for nothing — what the production path passes.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Perturb::none()
    }

    /// How far this place's mass is displaced downward.
    fn drop_of(&self, node: &NodeId) -> i64 {
        i64::from(self.sink == Some(node.0.as_str()))
    }
}
