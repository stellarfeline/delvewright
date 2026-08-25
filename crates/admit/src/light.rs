//! A **static light probe** over the space a player can actually stand in.
//!
//! The generator records `measured_min_light` from a **live 1.21.11 server probe**
//! (the gold standard). For admission machinery that must run in CI without a
//! server, this module estimates the same quantity from the piece's geometry.
//!
//! HONESTY: the emitted metadata records `method` as a *static estimate*, never a
//! live probe, so an admitted piece is never mistaken for a server-measured one.
//! A live re-probe (the generator's method) remains the gold standard for
//! borderline pieces — FLAGGED for owner review.
//!
//! # The light model is the compiler's, not this module's
//!
//! Block light, sky light, the 1.21.11 emitter table and the opacity table are
//! [`delvewright_compiler::light`]'s — the same model spec-0010 measures the
//! *assembled* world with (`DW0210`/`DW0211`). This module contributes the one
//! thing that model cannot know about a prefab: the piece stands in open air, so
//! the box handed to [`LightModel::from_blocks_within`] is a cell larger than the
//! piece on every horizontal side and above it. That ring is absent, therefore
//! air, therefore sky-open, so daylight enters through the piece's openings from
//! the side exactly as it does in the game.
//!
//! A private second copy of the model is what left this probe with **no sky term
//! at all**, and a sealed-edge box is what made that invisible: a roofed-but-open
//! building — a colonnade, a portico, a gatehouse arch, a pier under a deck, a
//! cliff overhang — measured pitch black at exit 0 while being daylit from every
//! side in the game.
//!
//! # The sky the probe assumes, and why the verdict states it
//!
//! A prefab is authored without a campaign, so nothing tells the probe which hour
//! the delve is pinned to — and a floor's light is not one number: the middle of
//! a 7×7 pavilion is bright at noon and black at midnight. Both are true, and a
//! bare `dark` says neither. So the probe floods the piece at **both ends of the
//! engine's own sky table** ([`delvewright_compiler::light::effective_sky`], never
//! a number restated here) and reports both:
//!
//! * the **profile** is taken at the darkest sky the engine models — a clear
//!   night — which is the state `darkest_effective_sky` bottoms out at and the
//!   only direction that can never call a genuinely dark interior `lit`;
//! * the **daylight** minimum is stated beside it, because *"black at night, lit
//!   by day"* is the sentence an author can act on and `dark` alone is not.
//!
//! Both numbers and the sky each was taken at go into the printed report, the
//! `DW0751` message and the `method` line written into the metadata. A light
//! level with no sky written beside it cannot be read afterwards.
//!
//! # What the minimum is taken over, and why it is not the region box
//!
//! The probe measures the minimum over the **floor cells a body can walk to from
//! outside**, and it states how many cells that was.
//!
//! Taking it over every cell of the region box instead is a gate that nobody can
//! pass and therefore nobody reads: the box holds the sealed voids between a
//! vault and its roof, which no lantern reaches and no player ever stands in. A
//! measure that cannot come out any other way is not a measure.
//!
//! Two filters, each removing cells for a reason a player would recognise:
//!
//! * **standable** — two courses of clearance over something to stand on;
//! * **reachable on foot** from a ground-level entrance ([`nav::ground_entry`]) —
//!   this is what removes the sealed voids.
//!
//! There is deliberately no third *roofed* filter ([`nav::sheltered`]). Roofedness
//! is a proxy for "indoors", and only a probe with no sky term needs one: without
//! sky, the walkable apron outside a free-standing building measures zero and every
//! such piece reports `dark` whatever its lighting design. With the sky term the
//! apron measures what the game measures — the vanilla night floor, which is above
//! the darkness threshold — so the proxy buys nothing and costs the pieces it
//! excludes. It excluded them completely: a piece with no roofed cell at all bound
//! ZERO and could carry no measured profile, which is every open-air piece in the
//! library.
//!
//! And the binding is reported, because a filter chain that removes everything
//! would otherwise read as "nothing was dark". A zero binding is a **finding**
//! (`DW0752`), never a pass: a sealed pitch-black crypt binds zero cells, and
//! the one thing this probe must never do is call that lit.
//!
//! The walk and the standability predicate are [`delvewright_schem::nav`]'s, not
//! this module's: they are the same question the grammar back end asks of an
//! expansion, and the seventh private copy of them was here.

use std::collections::BTreeMap;

use delvewright_compiler::light::{LightModel, effective_sky};
use delvewright_dsl::blockshape;
use delvewright_dsl::{WorldTime, WorldWeather};
use delvewright_schem::nav::{self, Voxels};
use delvewright_schem::split::TilePart;

use crate::structure::Structure;

/// Default block-light threshold below which a floor is `dark` (aligns with the
/// compiler's `DW0210` "floor light < 3" rule). Configurable — FLAGGED for owner
/// review (the lit/dark cutoff is policy, not mechanism).
pub const DEFAULT_DARK_THRESHOLD: i32 = 3;

/// The darkest sky the engine models: a clear night. The profile is taken here,
/// which is where [`delvewright_compiler::light::darkest_effective_sky`] bottoms
/// out — so the probe can never call an interior `lit` that the compiler's own
/// darkness proof would call dark.
///
/// Read out of [`effective_sky`] rather than written down, so a change to the
/// engine's sky table reaches this probe with nothing here to edit.
pub fn night_sky() -> i32 {
    effective_sky(WorldTime::Night, WorldWeather::Clear) as i32
}

/// Full daylight: a clear noon. The second figure the probe reports.
pub fn daylight_sky() -> i32 {
    effective_sky(WorldTime::Noon, WorldWeather::Clear) as i32
}

/// A whole zone's blocks as one grid of block names, however many files they
/// arrived in.
///
/// A tiled zone is one building; the tiling is packaging, and light crosses a
/// packaging plane exactly as it crosses any other cell. Probing tile by tile
/// would report darkness at every cut.
pub struct Zone<'a> {
    size: [i32; 3],
    names: Vec<&'a str>,
}

impl<'a> Zone<'a> {
    /// One structure template, probed on its own.
    pub fn single(s: &'a Structure) -> Zone<'a> {
        Zone::assemble(s.size, [([0, 0, 0], s)])
    }

    /// A zone reassembled from its tiles, each translated by the zone-local
    /// offset its manifest declares.
    pub fn from_tiles(size: [i32; 3], tiles: &'a [(TilePart, Structure)]) -> Zone<'a> {
        Zone::assemble(size, tiles.iter().map(|(p, s)| (p.offset, s)))
    }

    fn assemble(
        size: [i32; 3],
        tiles: impl IntoIterator<Item = ([i32; 3], &'a Structure)>,
    ) -> Zone<'a> {
        let [sx, sy, sz] = size;
        let n = (sx.max(0) as usize) * (sy.max(0) as usize) * (sz.max(0) as usize);
        // Absent cells are air: a dense template fills every cell, and a sparse
        // one means air wherever it says nothing.
        let mut names: Vec<&str> = vec!["minecraft:air"; n];
        for (offset, s) in tiles {
            for b in &s.blocks {
                let p = [
                    b.pos[0] + offset[0],
                    b.pos[1] + offset[1],
                    b.pos[2] + offset[2],
                ];
                if (0..3).all(|a| p[a] >= 0 && p[a] < size[a]) {
                    names[((p[0] * sy + p[1]) * sz + p[2]) as usize] =
                        s.palette[b.state as usize].name.as_str();
                }
            }
        }
        Zone { size, names }
    }

    /// The zone's extent.
    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    fn name(&self, pos: [i32; 3]) -> Option<&str> {
        if !(0..3).all(|a| pos[a] >= 0 && pos[a] < self.size[a]) {
            return None;
        }
        let [_, sy, sz] = self.size;
        Some(self.names[((pos[0] * sy + pos[1]) * sz + pos[2]) as usize])
    }
}

impl Voxels for Zone<'_> {
    fn origin(&self) -> [i32; 3] {
        [0, 0, 0]
    }

    fn size(&self) -> [i32; 3] {
        self.size
    }

    /// **Asked, not decided, here** — [`delvewright_dsl::blockshape`],
    /// spec-0056.
    ///
    /// This probe used to carry a nine-id list of its own: it knew a torch was
    /// walked through, which the grammar walk did not, and it called open water
    /// passable, which spec-0038 forbids. A third private answer to one question
    /// is the defect spec-0056 exists to end, so the list is gone and this reads
    /// the same table `delvec` routes with.
    fn passable(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(blockshape::passes_body)
    }

    fn floor(&self, pos: [i32; 3]) -> bool {
        self.name(pos).is_some_and(blockshape::supports_body)
    }

    /// A partial floor is measured, not read as a full cube — the same answer
    /// the grammar walk and `delvec` give.
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        self.name(support)
            .and_then(blockshape::floor_top_16)
            .map_or(delvewright_dsl::metrics::FULL_16, i64::from)
    }
}

/// The probe result — a measurement, and the binding it was taken over.
#[derive(Debug, Clone)]
pub struct LightProbe {
    /// Minimum light over the measured cells **at [`night_sky`]** — the number
    /// the profile is taken from. `None` when nothing bound.
    pub measured_min_light: Option<i32>,
    /// The same minimum at [`daylight_sky`]. Reported beside the profile so a
    /// roofed-but-open piece can say "black at night, lit by day" instead of
    /// leaving a reader to guess which of the two `dark` meant.
    pub min_light_daylight: Option<i32>,
    /// The darkest measured cell at [`night_sky`], in zone coordinates — where to
    /// put a light.
    pub darkest_cell: Option<[i32; 3]>,
    /// `"lit"` / `"dark"` / `"unbound"`.
    pub profile: &'static str,
    /// The threshold used.
    pub dark_threshold: i32,
    /// The effective sky level the profile was taken at ([`night_sky`]).
    pub sky_light: i32,
    /// The effective sky level [`Self::min_light_daylight`] was taken at.
    pub daylight_sky_light: i32,
    /// Standable cells anywhere in the region box (before any filter).
    pub standable_cells: usize,
    /// Ground-level entry cells found on the box's vertical faces.
    pub entry_cells: usize,
    /// **The binding**: standable cells a body can walk to from an entry cell —
    /// the cells the minimum was actually taken over.
    pub measured_cells: usize,
}

impl LightProbe {
    pub fn is_dark(&self) -> bool {
        self.profile == "dark"
    }

    /// Did the probe bind to nothing? A zero binding is a finding, not a pass.
    pub fn is_unbound(&self) -> bool {
        self.profile == "unbound"
    }

    /// Why the probe bound to nothing, in the words an author can act on.
    pub fn unbound_reason(&self) -> String {
        if self.standable_cells == 0 {
            "no cell in the piece has two courses of clearance over a floor — there is nowhere \
             to stand in it at all"
                .to_string()
        } else if self.entry_cells == 0 {
            format!(
                "{} standable cell(s), but no ground-level entrance on any of the four vertical \
                 faces: nothing can be walked into. A piece whose way in is a jigsaw socket must \
                 be socketed before it is probed",
                self.standable_cells
            )
        } else {
            format!(
                "{} standable cell(s) and {} entry cell(s), but nothing is reachable on foot from \
                 an entrance",
                self.standable_cells, self.entry_cells
            )
        }
    }
}

/// Run the engine's light flood over `zone` at both ends of the sky table and
/// take the minimum over player space.
pub fn probe(zone: &Zone, dark_threshold: i32) -> LightProbe {
    let model = light_model(zone);
    let (sky_light, daylight) = (night_sky(), daylight_sky());
    let night_field = model.flood(sky_light as u8);
    let day_field = model.flood(daylight as u8);

    let standable = nav::standable_cells(zone);
    let entry = nav::ground_entry(zone);
    let measured = nav::reachable_from(zone, &standable, &entry);

    let at = |f: &BTreeMap<[i32; 3], u8>, c: [i32; 3]| f.get(&c).copied().unwrap_or(0) as i32;
    let mut min_light: Option<i32> = None;
    let mut darkest: Option<[i32; 3]> = None;
    let mut min_daylight: Option<i32> = None;
    for &c in &measured {
        let l = at(&night_field, c);
        if min_light.is_none_or(|m| l < m) {
            min_light = Some(l);
            darkest = Some(c);
        }
        let d = at(&day_field, c);
        if min_daylight.is_none_or(|m| d < m) {
            min_daylight = Some(d);
        }
    }

    let profile = match min_light {
        None => "unbound",
        Some(m) if m < dark_threshold => "dark",
        Some(_) => "lit",
    };
    LightProbe {
        measured_min_light: min_light,
        min_light_daylight: min_daylight,
        darkest_cell: darkest,
        profile,
        dark_threshold,
        sky_light,
        daylight_sky_light: daylight,
        standable_cells: standable.len(),
        entry_cells: entry.len(),
        measured_cells: measured.len(),
    }
}

/// The zone as the compiler's light model, **standing in open air**.
///
/// The stated box is a cell larger than the piece on ±x, ±z and above it. Those
/// cells are absent, therefore air, therefore sky-open — so the sky seeds there
/// and floods inward through every opening in the piece's sides, which is how a
/// colonnade is lit in the game and the only way a probe can see it. Inferring
/// the box from the piece's own cells instead treats every opening as a sealed
/// edge, and a building with a roof over it can then only measure zero.
///
/// Nothing is added below the piece: what is under a prefab is the ground it
/// sits on, and no sky arrives from underneath.
fn light_model(zone: &Zone) -> LightModel {
    let [sx, sy, sz] = zone.size;
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let name = zone.names[((x * sy + y) * sz + z) as usize];
                // Absent means air to the model, so air is not worth storing.
                if !is_empty_to_light(name) {
                    blocks.insert([x, y, z], name.to_string());
                }
            }
        }
    }
    LightModel::from_blocks_within(blocks, [-1, 0, -1], [sx, sy, sz])
}

/// **Is this cell empty to the LIGHT model?**
///
/// A different question from whether a body fits through it, and it is why this
/// one predicate survived spec-0056 while the passability pair beside it did
/// not: light is stopped by opacity, and a torch cell that a body walks straight
/// through still holds a block the relight has to see. So only the three air
/// spellings are omitted from the model's block map, and everything else — torch
/// included — is handed to [`LightModel`] to decide the opacity of.
fn is_empty_to_light(name: &str) -> bool {
    delvewright_dsl::blockshape::is_air(name)
}
