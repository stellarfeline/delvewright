//! The metrics gym (spec-0049 §2.3) — a site-plan campaign generated **from**
//! the metrics table.
//!
//! # Why it is generated rather than authored
//!
//! Building-metric values cannot be cited: nothing transfers from other engines'
//! units, and Minecraft's one-block granularity at player scale makes a
//! minimum-width choice coarser than any published standard. So they are
//! **calibrated by walking**, and the thing that gets walked has to be the table
//! itself. Generating the gym from [`Metrics::table`] is what makes it incapable
//! of drifting from what it documents: a walker's ruling edits the table entry,
//! and the bay that demonstrated it is a different size the next time anyone
//! runs this. An authored gym would be a picture of the standard as it was on
//! the day somebody typed it.
//!
//! Nothing here describes a block. The gym is four small documents plus the
//! ordinary quest layer, run through the ordinary stage-5 derivation — so the
//! calibration walk and an exercise of the whole slice are the same hour, which
//! is the dogfooding spec-0049 §2.3 asks for.
//!
//! # What it builds
//!
//! A **spine** of ten bays in a row, one per rung of the size-class ladder at
//! each of its two bounds, connected by seams that cycle every standard opening
//! the table defines — so a body walks from the smallest place the ladder admits
//! to the largest, through every doorway it admits, in one line.
//!
//! Hanging off the spine, the **vertical group**: two climbs that differ only in
//! the run their host affords, so the derivation picks the gentle pitch for one
//! and the steep one for the other and a walker compares two standards built to
//! the same rise; and a designed fall at exactly the drop policy's cap, with a
//! way back up so the pit is not a strand.
//!
//! # What it cannot build, and why that is stated rather than remembered
//!
//! The gym's whole claim is that walking it calibrates the table, so an entry it
//! never instantiates is an entry the walk cannot rule on. [`DW_GYM_UNWALKED`]
//! states which, every run, against the whole table as its denominator — a
//! count that can only be honest, because the numerator is the set of entries
//! this generator actually **read** ([`Reads`]) rather than a list somebody
//! maintains beside it. A table entry added tomorrow and reached by nothing is
//! named the first time anyone runs the gym.
//!
//! At this version three entries come back unreached, and the reason is not
//! laziness: `corridor.min-width` and `corridor.min-clearance` describe a place
//! narrower than any rung of the size-class ladder admits (the smallest is four
//! by four), and the site plan has no surface for a place that is not a box with
//! a size class — so a two-wide corridor cannot be spelled at all.
//! `pacing.walk-only-blocks-per-minute` is a ceiling for the coefficient beside
//! it and is read by no verdict. Those are findings about the vocabulary, and
//! the gym is where they surface.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_dsl::metrics::{
    Grid, MetricKind, MetricValue, Metrics, Opening, Pitch, Reads, SizeClass,
};
use delvewright_dsl::{Diagnostic, DwCode, ExitTier};
use serde_json::{Value, json};

/// `DW0840`: the gym leaves a building metric unwalked.
///
/// A warning, and it names the denominator: the gym's argument is that a walk of
/// it rules on the whole standard, so an entry no bay instantiates is a number
/// the walk cannot settle however carefully it is walked. Zero unreached entries
/// is the end state and the line does not print — which is a real end state and
/// not a vacuity, because the count is taken against every entry in the table.
pub const DW_GYM_UNWALKED: DwCode = DwCode::new("DW0840", ExitTier::Build);

/// The plane the gym opens on.
const GRADE_Y: i64 = 64;
/// How far above grade the two climbs land. Chosen as the drop policy's cap so
/// one landing serves both the climbs and the fall.
fn landing_y(table: &Metrics, reads: &mut Reads) -> i64 {
    GRADE_Y + i64::from(table.max_designed_drop_blocks(reads).unwrap_or(5))
}

/// The `dsl_version` every document this generator writes declares: the one
/// number the engine accepts, never a typed literal.
const GYM_DSL_VERSION: &str = delvewright_dsl::DSL_VERSION;

/// One bay of the spine: a rung of the size ladder at one of its bounds, or a
/// **way** at one of its instantiable widths (spec-0053 §3).
struct Bay {
    node: String,
    /// Which vocabulary [`Bay::class`] names. A bay carries the kind rather than
    /// the generator inferring it from the name, for the reason the layout graph
    /// itself does: the two classifications are different questions about a box
    /// and a reader that had to guess would guess wrong on the first name that
    /// existed in both tables.
    kind: MetricKind,
    class: &'static str,
    /// `[x, z]` of the box's low corner.
    min: [i64; 2],
    /// `[dx, dz]`.
    extent: [i64; 2],
    clearance: i64,
}

/// The documents of one generated gym, ready to write.
pub struct Gym {
    /// File name → the canonical JSON text.
    pub documents: BTreeMap<String, String>,
    /// The building-metric entries this generation read.
    pub read: BTreeSet<&'static str>,
    /// Every building entry in the table — the denominator.
    pub entries: usize,
    /// Bays on the spine.
    pub bays: usize,
    /// Seams allocated.
    pub seams: usize,
}

impl Gym {
    /// `DW0840` — the entries the gym never reached, or `None` when it reached
    /// all of them.
    #[must_use]
    pub fn unwalked(&self, table: &Metrics) -> Option<Diagnostic> {
        let missed: Vec<&str> = table
            .building
            .keys()
            .copied()
            .filter(|k| !self.read.contains(k))
            .collect();
        if missed.is_empty() {
            return None;
        }
        Some(Diagnostic::warning(
            DW_GYM_UNWALKED,
            "metrics",
            "/building",
            format!(
                "the gym instantiates {n} of the {total} building metric(s) this table defines, \
                 and leaves {m} for a walk to rule on with nothing to look at: {names}. The gym \
                 exists so that walking it settles the standard, so an entry no bay is built from \
                 is a number the walk cannot decide however carefully it is walked — that is a \
                 finding about the authoring vocabulary, not about this run. The count is taken \
                 against every entry in the table and the reached set is what this generation \
                 actually read, so an entry added later and reached by nothing is named here the \
                 first time anyone regenerates.",
                n = self.read.len(),
                total = self.entries,
                m = missed.len(),
                names = missed.join(", "),
            ),
        ))
    }
}

/// Look a way class up, recording the read (spec-0053 §3).
///
/// Through `Metrics::resolve` and `BuildingEntry::value` like every other
/// accessor here, because the coverage numerator is the read ledger: an entry
/// this generator reached any other way would be an entry the gym claims to
/// instantiate and `DW0840` cannot see it instantiate.
fn way_class(
    table: &Metrics,
    reads: &mut Reads,
    name: &'static str,
) -> delvewright_dsl::metrics::WayClass {
    let entry = table
        .resolve(MetricKind::WayClass, name)
        .expect("the way vocabulary is the table's own names");
    match entry.value(reads) {
        MetricValue::WayClass(w) => *w,
        _ => unreachable!("a way-class entry carries a way class"),
    }
}

/// Look a size class up, recording the read.
fn size_class(table: &Metrics, reads: &mut Reads, name: &'static str) -> SizeClass {
    let entry = table
        .resolve(MetricKind::SizeClass, name)
        .expect("the ladder's rungs are the table's own names");
    match entry.value(reads) {
        MetricValue::SizeClass(c) => *c,
        _ => unreachable!("a size-class entry carries a size class"),
    }
}

/// Look an opening up, recording the read.
fn opening(table: &Metrics, reads: &mut Reads, name: &'static str) -> Opening {
    let entry = table
        .resolve(MetricKind::Opening, name)
        .expect("the gym's openings are the table's own names");
    match entry.value(reads) {
        MetricValue::Opening(o) => *o,
        _ => unreachable!("an opening entry carries an opening"),
    }
}

/// Look a stair pitch up, recording the read.
fn pitch(table: &Metrics, reads: &mut Reads, name: &str) -> Pitch {
    let entry = table
        .resolve(MetricKind::Pitch, name)
        .expect("the gym's pitches are the table's own names");
    match entry.value(reads) {
        MetricValue::Pitch(p) => *p,
        _ => unreachable!("a pitch entry carries a pitch"),
    }
}

/// Look a pacing coefficient up, recording the read.
fn pacing(table: &Metrics, reads: &mut Reads, name: &str) -> i64 {
    let entry = table
        .resolve(MetricKind::Pacing, name)
        .expect("the gym's pacing coefficients are the table's own names");
    match entry.value(reads) {
        MetricValue::Count(n) => i64::from(*n),
        MetricValue::Number(n) => *n as i64,
        _ => unreachable!("a pacing entry carries a number"),
    }
}

/// Round `n` up to a whole number of `d`.
fn ceil_div(n: i64, d: i64) -> i64 {
    if d <= 0 { n } else { (n + d - 1) / d }
}

/// Look a storey height up, recording the read.
fn storey(table: &Metrics, reads: &mut Reads, name: &'static str) -> i64 {
    let entry = table
        .resolve(MetricKind::Storey, name)
        .expect("the gym's storeys are the table's own names");
    match entry.value(reads) {
        MetricValue::Count(n) => i64::from(*n),
        _ => unreachable!("a storey entry carries a cell count"),
    }
}

/// The part of an id after its kind prefix — the shape an anchor name is built
/// from, so the gym's quest layer can name what the derivation will place.
fn slug(id: &str) -> &str {
    id.split_once('/').map_or(id, |(_, rest)| rest)
}

/// Generate the gym for `table`.
///
/// Deterministic and parameterless: the same table produces the same documents,
/// byte for byte, which is what makes the gym a **regeneration** of the standard
/// rather than a second copy of it.
#[must_use]
pub fn generate(table: &Metrics, campaign_id: &str) -> Gym {
    let mut reads = Reads::new();

    // ---------------------------------------------------------------- the spine
    //
    // The ladder in table order, each rung at both of its bounds. Reading the
    // rung names from `names_of` rather than listing them is what makes a rung
    // added to the table appear here without an edit.
    let mut bays: Vec<Bay> = Vec::new();
    let mut x = 4i64;
    let z0 = 4i64;
    let storey_low = storey(table, &mut reads, "low");
    let storey_standard = storey(table, &mut reads, "standard");
    let storey_hall = storey(table, &mut reads, "hall");
    let storeys = [storey_low, storey_standard, storey_hall];

    let rungs = table.names_of(MetricKind::SizeClass);
    for (ri, rung) in rungs.iter().enumerate() {
        let c = size_class(table, &mut reads, rung);
        for (bi, foot) in [c.min_footprint, c.max_footprint].into_iter().enumerate() {
            let bound = if bi == 0 { "least" } else { "most" };
            let extent = [i64::from(foot[0]), i64::from(foot[1])];
            // The ceiling is a named storey height where the class admits one,
            // and the class's own floor where no storey reaches it. That gap is
            // real — the top rung asks for more headroom than the tallest storey
            // the table names — and it is left visible rather than papered over.
            let clearance = storeys
                .iter()
                .copied()
                .find(|s| *s >= i64::from(c.min_clearance))
                .unwrap_or(i64::from(c.min_clearance));
            bays.push(Bay {
                node: format!("node/{rung}-{bound}"),
                kind: MetricKind::SizeClass,
                class: rung,
                min: [x, z0],
                extent,
                clearance,
            });
            x += extent[0] + 1;
            let _ = ri;
        }
    }

    // Two bays are asked to host a climb, so they need headroom for one. The
    // steep host is the one whose run is too short for the gentle pitch, which
    // is the whole point of the pair.
    let rise = landing_y(table, &mut reads) - GRADE_Y;
    // The derivation picks the GENTLEST standard pitch the host affords, walking
    // the table in its own order. The gym's whole argument about pitch is a pair
    // of climbs to the same rise that come out at different pitches, so the two
    // hosts are chosen by that same rule read from that same table: one box long
    // enough for the gentlest, one too short for it and long enough for the
    // steepest. Deciding it here with a hard-coded `2 * rise` would make this
    // file a second authority on a standard the table states, and the coverage
    // count below said so — `pitch.ramp` and `pitch.stair` came back unread.
    let names = table.names_of(MetricKind::Pitch);
    let gentlest = pitch(table, &mut reads, names[0]);
    let steepest = pitch(table, &mut reads, names[names.len() - 1]);
    let run_for = |p: Pitch| ceil_div(rise * i64::from(p.run), i64::from(p.rise).max(1));
    let (gentle_run, steep_run) = (run_for(gentlest), run_for(steepest));
    let steep_host = pick_host(&bays, |b| {
        b.extent[1] >= steep_run && b.extent[1] < gentle_run
    });
    let gentle_host = pick_host(&bays, |b| b.extent[1] >= gentle_run);
    for i in [steep_host, gentle_host] {
        if bays[i].clearance < rise + storey_low {
            bays[i].clearance = rise + storey_low;
        }
    }

    // ------------------------------------------------------------- the way bays
    //
    // A way class bounds a cross-section and leaves the run free, so a bay of one
    // is a box at an instantiable WIDTH whose run exceeds the class's widest
    // cross-section — the elongation `DW0832` demands, which is what makes the
    // box a way rather than a room.
    //
    // **Instantiable** is doing work. A box's horizontal extents are multiples of
    // the kit quantum (`DW0825`), so the widths a walker can be given are the
    // multiples of `q` inside the class's range — which is fewer than the range
    // states. The corridor's inherited floor of 2 sits under a quantum of 4 and
    // is therefore not a width any plan can draw, and that is a real gap between
    // two provisional numbers rather than a laziness here: which of the two moves
    // is the walk's judgement, and the entry's own note asks for it. What this
    // generator will not do is quietly round the floor up and present the walk
    // with a bay it did not ask for.
    //
    // They are appended AFTER the climb hosts are chosen, deliberately: a way bay
    // is long by construction and would win `pick_host`'s length test, putting a
    // stair in a corridor and dissolving the pitch pair the gym exists to argue
    // about.
    let q = table
        .grid(&mut reads)
        .map_or(1, |g| i64::from(g.quantum).max(1));
    for name in table.names_of(MetricKind::WayClass) {
        let w = way_class(table, &mut reads, name);
        let (lo, hi) = (i64::from(w.min_width), i64::from(w.max_width));
        let widths: Vec<i64> = (lo..=hi).filter(|n| n % q == 0).collect();
        assert!(
            !widths.is_empty(),
            "`way-class.{name}` admits widths {lo}..{hi} and none of them is a multiple of \
             the kit quantum of {q}, so no plan can draw a way of this class at all and no \
             bay can instantiate it"
        );
        // The shortest run that both exceeds the widest cross-section and lands
        // on the grid — the least a box has to be to qualify, which is the
        // interesting end for a walk about whether a way reads as one.
        let run = ((hi + 1) + q - 1) / q * q;
        for width in widths {
            let extent = [width, run];
            let clearance = storeys
                .iter()
                .copied()
                .find(|s| *s >= i64::from(w.min_clearance))
                .unwrap_or(i64::from(w.min_clearance));
            bays.push(Bay {
                node: format!("node/{name}-{width}-wide"),
                kind: MetricKind::WayClass,
                class: name,
                min: [x, z0],
                extent,
                clearance,
            });
            x += extent[0] + 1;
        }
    }

    // ------------------------------------------------------- the vertical group
    let landing = GRADE_Y + rise;
    let steep_top = Bay {
        node: "node/steep-landing".to_string(),
        kind: MetricKind::SizeClass,
        class: "alcove",
        min: [bays[steep_host].min[0], z0 + bays[steep_host].extent[1] + 1],
        extent: [8, 8],
        clearance: storey_standard,
    };
    let gentle_top = Bay {
        node: "node/gentle-landing".to_string(),
        kind: MetricKind::SizeClass,
        class: "room",
        min: [
            bays[gentle_host].min[0],
            z0 + bays[gentle_host].extent[1] + 1,
        ],
        extent: [16, 16],
        clearance: storey_hall,
    };
    let pit = Bay {
        node: "node/pit".to_string(),
        kind: MetricKind::SizeClass,
        class: "room",
        min: [
            gentle_top.min[0],
            gentle_top.min[1] + gentle_top.extent[1] + 1,
        ],
        extent: [16, 16],
        clearance: storey_standard,
    };

    // --------------------------------------------------------- graph and plan
    let door = opening(table, &mut reads, "door");
    let arch = opening(table, &mut reads, "arch");
    let passage = opening(table, &mut reads, "passage");
    let gateway = opening(table, &mut reads, "gateway");
    let ladder_openings: [(&str, Opening); 4] = [
        ("door", door),
        ("arch", arch),
        ("passage", passage),
        ("gateway", gateway),
    ];

    // The kit grid, read rather than assumed: `DW0825` refuses a box whose
    // horizontal extents are not multiples of the quantum, and the gym's
    // footprints come from the ladder — so if a rung is ever set off-grid, the
    // generator is where that shows up rather than the checker.
    if let Some(Grid { quantum, .. }) = table.grid(&mut reads) {
        let q = i64::from(quantum).max(1);
        for b in &bays {
            assert!(
                b.extent[0] % q == 0 && b.extent[1] % q == 0,
                "the `{}` rung is {} by {}, which is not on the kit grid of {q}",
                b.class,
                b.extent[0],
                b.extent[1],
            );
        }
    }

    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut boxes: Vec<Value> = Vec::new();
    let mut seams: Vec<Value> = Vec::new();

    let node_entry = |b: &Bay, intent: &str, note: &str| {
        let field = match b.kind {
            MetricKind::WayClass => "way_class",
            _ => "size_class",
        };
        json!({
            "id": b.node,
            "intent": intent,
            "note": note,
            field: b.class,
        })
    };
    let box_entry = |b: &Bay, floor: i64| {
        json!({
            "node": b.node,
            "min": [b.min[0], b.min[1]],
            "extent": [b.extent[0], b.extent[1]],
            "floor": { "y": floor },
            "ceiling": { "clearance": b.clearance },
        })
    };

    for (i, b) in bays.iter().enumerate() {
        let (intent, note) = match b.kind {
            MetricKind::WayClass => (
                "way-class specimen",
                format!(
                    "A `{}` at a cross-section of {}: {} by {} with {} of headroom. The run \
                     exceeds the class's widest cross-section, which is the elongation that \
                     makes it a way and not a room.",
                    b.class,
                    b.extent[0].min(b.extent[1]),
                    b.extent[0],
                    b.extent[1],
                    b.clearance,
                ),
            ),
            _ => (
                "size-class specimen",
                format!(
                    "The `{}` rung at its {} bound: {} by {} with {} of headroom.",
                    b.class,
                    if i % 2 == 0 { "lower" } else { "upper" },
                    b.extent[0],
                    b.extent[1],
                    b.clearance,
                ),
            ),
        };
        nodes.push(node_entry(b, intent, &note));
        boxes.push(box_entry(b, GRADE_Y));
    }
    for (i, pair) in bays.windows(2).enumerate() {
        let (a, b) = (&pair[0], &pair[1]);
        // The widest standard opening that fits both faces, so the seams walk
        // the whole opening set as the bays grow rather than repeating one.
        let room = a.extent[1].min(b.extent[1]) - 1;
        let head = a.clearance.min(b.clearance);
        let (name, _) = ladder_openings
            .iter()
            .filter(|(_, o)| i64::from(o.width) <= room && i64::from(o.height) <= head)
            .max_by_key(|(_, o)| (o.width, o.height))
            .expect("the smallest standard opening fits the smallest rung");
        let id = format!("edge/{}-to-{}", slug(&a.node), slug(&b.node));
        edges.push(json!({ "id": id, "a": a.node, "b": b.node, "class": "walk" }));
        seams.push(json!({
            "edge": id, "face": "east", "at": [z0 + 1, GRADE_Y], "opening": name,
        }));
        let _ = i;
    }

    // The two climbs. `stair_in` is the LOWER place in both, which is the only
    // plane treads can rise off; what differs is the run that place affords, and
    // that difference is what makes the derivation choose a different pitch.
    for (host, top, gate) in [
        (steep_host, &steep_top, "door"),
        (gentle_host, &gentle_top, "arch"),
    ] {
        let h = &bays[host];
        nodes.push(node_entry(
            top,
            "climb landing",
            &format!(
                "Reached by a {rise}-block climb hosted in `{}`, which affords {} of run — the \
                 derivation picks the gentlest standard pitch that fits it.",
                h.node, h.extent[1],
            ),
        ));
        boxes.push(box_entry(top, landing));
        let id = format!("edge/{}-climb", slug(&top.node));
        edges.push(json!({ "id": id, "a": h.node, "b": top.node, "class": "stair" }));
        seams.push(json!({
            "edge": id, "face": "south", "at": [h.min[0] + 1, landing],
            "opening": gate, "stair_in": h.node,
        }));
    }

    // The designed fall, at exactly the policy cap, and the way back out of it.
    nodes.push(node_entry(
        &pit,
        "designed fall",
        &format!(
            "The floor of a {rise}-block drop — the deepest a designed one-way fall may be. The \
             stair beside it is what stops the pit being a strand.",
        ),
    ));
    boxes.push(box_entry(&pit, GRADE_Y));
    edges.push(json!({
        "id": "edge/the-fall", "a": gentle_top.node, "b": pit.node,
        "class": "drop", "falls": "a-to-b",
    }));
    seams.push(json!({
        "edge": "edge/the-fall", "face": "south",
        "at": [gentle_top.min[0] + 1, landing], "opening": "arch",
    }));
    edges.push(json!({
        "id": "edge/out-of-the-pit", "a": pit.node, "b": gentle_top.node, "class": "stair",
    }));
    seams.push(json!({
        "edge": "edge/out-of-the-pit", "face": "north",
        "at": [gentle_top.min[0] + 8, landing], "opening": "arch",
        "stair_in": pit.node,
    }));

    // ------------------------------------------------------------- the region
    //
    // Extent flows DOWN: the region is stated, and every box is inside it. It is
    // computed from the ladder because the ladder is the brief here — the gym's
    // written design IS "one place per rung at each bound" — and the identities
    // below hold the plan to that.
    let far_x = boxes
        .iter()
        .map(|b| b["min"][0].as_i64().unwrap_or(0) + b["extent"][0].as_i64().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let far_z = boxes
        .iter()
        .map(|b| b["min"][1].as_i64().unwrap_or(0) + b["extent"][1].as_i64().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let top_y = landing + gentle_top.clearance;
    let region_min = [0i64, GRADE_Y - 16, 0i64];
    let region_extent = [far_x + 8, top_y - (GRADE_Y - 16) + 8, far_z + 8];

    let entry_node = bays[0].node.clone();
    let goal_node = bays[bays.len() - 1].node.clone();
    let critical_path: Vec<Value> = bays.iter().map(|b| json!(b.node)).collect();

    // How long the gym is, in minutes, from the ladder's own nominal traverses
    // and the pacing coefficient. A typed number here would be a guess sitting
    // beside the coefficient the projection is measured against.
    let coefficient = pacing(table, &mut reads, "route-blocks-per-minute");
    // A size-class bay costs its rung's nominal traverse; a way bay costs its
    // measured RUN, because a way class bounds a cross-section and leaves the
    // run free and therefore has no nominal traverse to look up. The same rule
    // `DW0822` states, applied here rather than restated — a gym whose target
    // minutes were computed by a different rule from the projection it is walked
    // against would be arguing with the thing it exists to calibrate.
    let nominal: i64 = bays
        .iter()
        .map(|b| match b.kind {
            MetricKind::WayClass => b.extent[0].max(b.extent[1]),
            _ => i64::from(size_class(table, &mut reads, b.class).nominal_traverse_blocks),
        })
        .sum();
    let target_minutes = ceil_div(nominal, coefficient.max(1)).max(1);

    let smallest = size_class(table, &mut reads, rungs[0]);
    let largest = size_class(table, &mut reads, rungs[rungs.len() - 1]);

    let mut documents: BTreeMap<String, String> = BTreeMap::new();
    let put = |documents: &mut BTreeMap<String, String>, name: &str, v: Value| {
        let text = serde_json::to_string(&v).expect("the gym's documents serialize");
        let canonical = delvewright_dsl::fmt::format_text(&text)
            .expect("a document this generator just serialized parses");
        documents.insert(name.to_string(), canonical);
    };

    put(
        &mut documents,
        "world.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "world",
            "content": {
                "areas": [],
                "premise": "Every number a level is built to, standing side by side at the size it \
                            names. Walk it once and the standard stops being a seed.",
                "seed": 20260821,
                "target_minutes": target_minutes,
                "theme": "A gym of bays: the ladder, the doorways, the climbs and the fall.",
                "title": "The Metrics Gym",
            },
        }),
    );
    put(
        &mut documents,
        "classes.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "classes",
            "content": { "classes": [{
                "id": "class/measurer",
                "name": "Measurer",
                "blurb": "A rule, a lamp and nothing worth carrying.",
                "kit": [
                    { "count": 1, "item": "minecraft:stick", "name": "Rule" },
                    { "count": 3, "item": "minecraft:bread" },
                ],
            }] },
        }),
    );
    put(
        &mut documents,
        "npcs.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "npcs",
            "content": { "npcs": [{
                "id": "npc/invigilator",
                "name": "The Invigilator",
                "area": delvewright_dsl::SITE_AREA,
                "anchor": format!("anchor/node-{}", slug(&entry_node)),
                "base_entity": "minecraft:villager",
                "role": "quest-giver",
                "persona": {
                    "archetype": "patient examiner",
                    "backstory": "She has stood at the small end of the ladder since before any of \
                                  its rungs had numbers, and she writes down what each walker says \
                                  about them.",
                    "demeanor": "Unhurried. Asks the question and then waits.",
                    "motivation": "Get every bay walked by somebody who will say whether it is the \
                                   right size.",
                    "secret": "She has never agreed with the numbers she is asked to defend.",
                    "speech_style": "Plain, exact, faintly clerical; states measurements aloud.",
                },
            }] },
        }),
    );
    put(
        &mut documents,
        "quest-plan.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "quest-plan",
            "content": {
                "finale": "quest/walk-the-ladder",
                "quests": [{
                    "id": "quest/walk-the-ladder",
                    "act": 1,
                    "area": delvewright_dsl::SITE_AREA,
                    "depends_on": [],
                    "goal": "Walk the ladder from its smallest rung to its largest and say which \
                             sizes are wrong.",
                    "mandatory": true,
                    "npcs": ["npc/invigilator"],
                }],
            },
        }),
    );
    put(
        &mut documents,
        "dialogue.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "dialogue",
            "content": { "dialogues": [{
                "npc": "npc/invigilator",
                "root": "dlg/greeting",
                "nodes": [
                    {
                        "id": "dlg/greeting",
                        "text": "This is the smallest place the ladder allows. Walk east until it \
                                 stops getting bigger, and tell me where you stopped believing in \
                                 the sizes.",
                        "options": [
                            { "label": "What am I looking for?", "next": "dlg/what" },
                            {
                                "label": "Understood.",
                                "effects": [{ "objective": "obj/hear-the-brief", "type": "complete-objective" }],
                            },
                        ],
                    },
                    {
                        "id": "dlg/what",
                        "text": "Whether a rung feels like the rung below it. Whether a doorway is \
                                 one you would put a party through. Whether the climb is one you \
                                 would make twice.",
                        "options": [{ "label": "Back.", "next": "dlg/greeting" }],
                    },
                ],
            }] },
        }),
    );
    put(
        &mut documents,
        "quests.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "quests",
            "content": { "quests": [{
                "id": "quest/walk-the-ladder",
                "trigger": { "type": "campaign-start" },
                "happening": {
                    "subject": "npc/invigilator",
                    "verb": "arrives",
                    "text": "The party arrives at the small end of the ladder.",
                },
                "cast": { "npc/invigilator": {
                    "at": format!("anchor/node-{}", slug(&entry_node)),
                    "dialogue": "dlg/greeting",
                    "doing": "standing in the smallest bay with a rule in her hand",
                }},
                "objectives": [
                    {
                        "id": "obj/hear-the-brief",
                        "type": "talk-to",
                        "npc": "npc/invigilator",
                        "title": "Hear the brief",
                        "hint": "The Invigilator stands in the smallest bay.",
                        "happening": {
                            "subject": "npc/invigilator",
                            "verb": "learns",
                            "text": "The Invigilator explains what the walk is for.",
                        },
                    },
                    {
                        "id": "obj/reach-the-far-end",
                        "type": "reach-anchor",
                        "after": ["obj/hear-the-brief"],
                        "anchor": format!("anchor/node-{}", slug(&goal_node)),
                        "radius": 3,
                        "title": "Walk to the far end",
                        "happening": {
                            "verb": "arrives",
                            "text": "The party crosses the largest bay the ladder admits.",
                        },
                    },
                ],
                "on_complete": [{
                    "type": "campaign-complete",
                    "happening": { "verb": "departs", "text": "The ladder has been walked end to end." },
                }],
            }] },
        }),
    );
    put(
        &mut documents,
        "geometry-brief.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "geometry-brief",
            "content": { "facts": [
                {
                    "id": "fact/smallest-place-span",
                    "unit": "blocks",
                    "value": f64::from(smallest.min_footprint[0]),
                    "note": "The smallest place the size-class ladder admits. A gym whose small \
                             end is bigger than this is not showing the walker the bound they are \
                             being asked about.",
                },
                {
                    "id": "fact/largest-place-span",
                    "unit": "blocks",
                    "value": f64::from(largest.max_footprint[0]),
                    "note": "The largest. The distance between this number and the one above it \
                             is the whole ladder, and the walk is the argument about whether it \
                             has the right number of rungs.",
                },
                {
                    "id": "fact/landing-datum",
                    "unit": "blocks",
                    "value": landing as f64,
                    "note": "Where both climbs land, and the lip the designed fall goes over: the \
                             drop policy's cap above grade, so one plane demonstrates three \
                             standards.",
                },
            ] },
        }),
    );
    put(
        &mut documents,
        "layout-graph.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "layout-graph",
            "content": {
                "nodes": nodes,
                "edges": edges,
                "entry": entry_node,
                "goal": goal_node,
                "critical_path": critical_path,
                "beats": [
                    { "quest": "quest/walk-the-ladder", "objective": "obj/hear-the-brief", "node": entry_node },
                    { "quest": "quest/walk-the-ladder", "objective": "obj/reach-the-far-end", "node": goal_node },
                ],
            },
        }),
    );
    put(
        &mut documents,
        "site-plan.json",
        json!({
            "campaign_id": campaign_id,
            "dsl_version": GYM_DSL_VERSION,
            "stage": "site-plan",
            "content": {
                "region": { "min": region_min, "extent": region_extent },
                "datums": [
                    { "id": "datum/grade", "y": GRADE_Y, "note": "The plane the ladder stands on." },
                    { "id": "datum/landing", "y": landing, "note": "What both climbs reach and the fall leaves." },
                ],
                "boxes": boxes,
                "seams": seams,
                "identities": [
                    { "fact": "fact/smallest-place-span", "cmp": "eq",
                      "measure": { "of": "box-extent", "node": bays[0].node, "axis": "x" } },
                    { "fact": "fact/largest-place-span", "cmp": "eq",
                      "measure": { "of": "box-extent", "node": goal_node, "axis": "x" } },
                    { "fact": "fact/landing-datum", "cmp": "eq",
                      "measure": { "of": "datum-y", "datum": "datum/landing" } },
                ],
                "lighting": { "fixture": "torch", "min_light": 7 },
            },
        }),
    );

    Gym {
        documents,
        read: reads.read(),
        entries: table.building.len(),
        bays: bays.len(),
        seams: seams.len(),
    }
}

/// The first bay satisfying `want`, or the largest bay if none does.
///
/// A fallback rather than a panic because the hosts are chosen **from the
/// table**: change the drop cap or a rung's footprint and the pair that used to
/// straddle the gentle pitch's run may not exist. The gym still builds; what it
/// stops demonstrating is the difference between the two pitches, and the pitch
/// entry then goes unread, which is exactly what `DW0840` is for.
fn pick_host(bays: &[Bay], want: impl Fn(&Bay) -> bool) -> usize {
    bays.iter()
        .position(&want)
        .unwrap_or_else(|| bays.len().saturating_sub(1))
}

/// Write a generated gym into `dir`, creating it if needed.
///
/// # Errors
///
/// Any IO failure creating the directory or writing a document.
pub fn write(gym: &Gym, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, text) in &gym.documents {
        std::fs::write(dir.join(name), text)?;
    }
    Ok(())
}
