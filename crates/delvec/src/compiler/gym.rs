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
//! A **spine** of bays in a row, joined by one seam per standard opening the
//! table defines — so a body walks every doorway it admits, in one line.
//!
//! Hanging off the spine, the **vertical group**: two climbs that differ only in
//! the run their host affords, so the derivation picks the gentle pitch for one
//! and the steep one for the other and a walker compares two standards built to
//! the same rise of one low storey; and a designed fall of that storey, with a
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
//! At this version `pacing.walk-only-blocks-per-minute` comes back unreached: it
//! is a ceiling for the coefficient beside it and is read by no verdict. That is
//! a finding about the vocabulary, and the gym is where it surfaces.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_dsl::metrics::{MetricKind, MetricValue, Metrics, Opening, Pitch, Reads};
use delvewright_dsl::{Diagnostic, DwCode, ExitTier};
use serde_json::{Value, json};

delvewright_dsl::dw_code! {
    /// `DW0840`: the gym leaves a building metric unwalked.
    ///
    /// A warning, and it names the denominator: the gym's argument is that a walk of
    /// it rules on the whole standard, so an entry no bay instantiates is a number
    /// the walk cannot settle however carefully it is walked. Zero unreached entries
    /// is the end state and the line does not print — which is a real end state and
    /// not a vacuity, because the count is taken against every entry in the table.
    pub const DW_GYM_UNWALKED: DwCode = DwCode::new("DW0840", ExitTier::Build);
}

/// The plane the gym opens on.
const GRADE_Y: i64 = 64;
/// The `dsl_version` every document this generator writes declares: the one
/// number the engine accepts, never a typed literal.
const GYM_DSL_VERSION: &str = delvewright_dsl::DSL_VERSION;

/// One bay: a box the gym declares, at the footprint it states.
struct Bay {
    node: String,
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
    // One seam per standard opening, in table order, between consecutive bays
    // on one plane — so a body walks every doorway the table defines, in one
    // line. Reading the opening names from `names_of` rather than listing them
    // is what makes an opening added to the table appear here without an edit.
    let mut bays: Vec<Bay> = Vec::new();
    let mut x = 4i64;
    let z0 = 4i64;
    let storey_low = storey(table, &mut reads, "low");
    let storey_standard = storey(table, &mut reads, "standard");
    let storey_hall = storey(table, &mut reads, "hall");
    let storeys = [storey_low, storey_standard, storey_hall];

    let openings: Vec<(&'static str, Opening)> = table
        .names_of(MetricKind::Opening)
        .into_iter()
        .map(|n| (n, opening(table, &mut reads, n)))
        .collect();
    let widest = openings
        .iter()
        .map(|(_, o)| i64::from(o.width))
        .max()
        .unwrap_or(1);
    let tallest = openings
        .iter()
        .map(|(_, o)| i64::from(o.height))
        .max()
        .unwrap_or(2);

    // Two bays are asked to host a climb, so they need headroom for one. The
    // steep host is the one whose run is too short for the gentle pitch, which
    // is the whole point of the pair.
    // The climbs and the fall rise one low storey: a body walks up to a floor
    // of the next storey, and one landing serves both the climbs and the fall.
    let rise = storey_low;
    // The derivation picks the GENTLEST standard pitch the host affords, walking
    // the table in its own order. The gym's whole argument about pitch is a pair
    // of climbs to the same rise that come out at different pitches, so the two
    // hosts are sized by that same rule read from that same table: one box long
    // enough for the gentlest, one too short for it and long enough for the
    // steepest.
    let names = table.names_of(MetricKind::Pitch);
    let gentlest = pitch(table, &mut reads, names[0]);
    let steepest = pitch(table, &mut reads, names[names.len() - 1]);
    let run_for = |p: Pitch| ceil_div(rise * i64::from(p.run), i64::from(p.rise).max(1));
    let (gentle_run, steep_run) = (run_for(gentlest), run_for(steepest));

    // The gym's own footprints: wide enough on x for the widest opening's
    // landing, and on z the steep host's run, the gentle host's run, then a
    // square bay per remaining opening. These are the gym's design, not a
    // standard — a box is any whole number of blocks.
    let side = (widest + 3).max(steep_run + 1);
    let clearance = storeys
        .iter()
        .copied()
        .find(|s| *s > tallest)
        .unwrap_or(tallest + 1);
    let mut depths: Vec<i64> = vec![steep_run, gentle_run.max(side)];
    while depths.len() < openings.len() + 1 {
        depths.push(side);
    }
    for (i, dz) in depths.into_iter().enumerate() {
        let extent = [side, dz];
        bays.push(Bay {
            node: format!("node/bay-{}", i + 1),
            min: [x, z0],
            extent,
            clearance,
        });
        x += extent[0] + 1;
    }
    let (steep_host, gentle_host) = (0usize, 1usize);
    for i in [steep_host, gentle_host] {
        if bays[i].clearance < rise + storey_low {
            bays[i].clearance = rise + storey_low;
        }
    }

    // ------------------------------------------------------- the vertical group
    let landing = GRADE_Y + rise;
    let steep_top = Bay {
        node: "node/steep-landing".to_string(),
        min: [bays[steep_host].min[0], z0 + bays[steep_host].extent[1] + 1],
        // Narrower than its host, so it stands two cells clear of the gentle
        // landing beside it rather than one (`DW0827`).
        extent: [side - 3, 8],
        clearance: storey_standard,
    };
    let gentle_top = Bay {
        node: "node/gentle-landing".to_string(),
        min: [
            bays[gentle_host].min[0],
            z0 + bays[gentle_host].extent[1] + 1,
        ],
        extent: [16, 16],
        clearance: storey_hall,
    };
    let pit = Bay {
        node: "node/pit".to_string(),
        min: [
            gentle_top.min[0],
            gentle_top.min[1] + gentle_top.extent[1] + 1,
        ],
        extent: [16, 16],
        clearance: storey_standard,
    };

    // --------------------------------------------------------- graph and plan
    // The datum convention every bay's floor is declared under: a box's floor
    // SURFACE stands at its datum's `y`, which is what the walker reads each
    // bay standing on.
    let _ = table.datum(&mut reads);

    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut boxes: Vec<Value> = Vec::new();
    let mut seams: Vec<Value> = Vec::new();

    let node_entry = |b: &Bay, intent: &str, note: &str| {
        json!({
            "id": b.node,
            "intent": intent,
            "note": note,
        })
    };
    // The plan is relational (spec-0059): the first bay is pinned and every
    // other box is placed by its seam. `Bay::min` stays the generator's own
    // arithmetic for the region, never written to the plan.
    let entry_min = bays[0].min;
    let box_entry = |b: &Bay, floor: i64| {
        let mut v = json!({
            "node": b.node,
            "extent": [b.extent[0], b.extent[1]],
            "floor": { "y": floor },
            "ceiling": { "clearance": b.clearance },
        });
        if b.min == entry_min && b.node == bays[0].node {
            v["min"] = json!([b.min[0], b.min[1]]);
        }
        v
    };

    for b in &bays {
        let note = format!(
            "A bay {} by {} with {} of headroom, between two standard openings.",
            b.extent[0], b.extent[1], b.clearance,
        );
        nodes.push(node_entry(b, "opening specimen", &note));
        boxes.push(box_entry(b, GRADE_Y));
    }
    for (pair, (name, o)) in bays.windows(2).zip(openings.iter()) {
        let (a, b) = (&pair[0], &pair[1]);
        assert!(
            i64::from(o.width) < a.extent[1].min(b.extent[1])
                && i64::from(o.height) <= a.clearance.min(b.clearance),
            "the `{name}` opening fits the shared face of the two bays it joins"
        );
        let id = format!("edge/{}-to-{}", slug(&a.node), slug(&b.node));
        edges.push(json!({ "id": id, "a": a.node, "b": b.node, "class": "walk" }));
        // One cell in from each bay's low corner along the shared wall — the
        // same cells as ever, stated from each box's own corner.
        seams.push(json!({
            "edge": id, "face": "east", "at": 1, "meets": 1, "opening": name,
            "form": format!("a standard `{name}` between two bays, on one plane"),
        }));
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
        // The landing hangs one cell west of its host's corner, so it stands
        // two cells clear of the next bay east rather than one: two places one
        // cell apart that nothing joins would both claim the ring between them
        // (`DW0827`, spec-0098 §2 rule 3d).
        seams.push(json!({
            "edge": id, "face": "south", "at": 1, "meets": 2,
            "opening": gate, "stair_in": h.node,
            "form": format!("a standard `{gate}` at the head of a {rise}-block climb"),
        }));
    }

    // The designed fall, one low storey deep, and the way back out of it.
    nodes.push(node_entry(
        &pit,
        "designed fall",
        &format!(
            "The floor of a {rise}-block designed one-way drop. The \
             stair beside it is what stops the pit being a strand.",
        ),
    ));
    boxes.push(box_entry(&pit, GRADE_Y));
    edges.push(json!({
        "id": "edge/the-fall", "a": gentle_top.node, "b": pit.node,
        "class": "drop", "falls": "a-to-b",
    }));
    seams.push(json!({
        "edge": "edge/the-fall", "face": "south", "at": 1, "meets": 1, "opening": "arch",
        "form": format!("an arch over a {rise}-block designed fall"),
    }));
    // Named from the landing's side, as the fall is: two connections across one
    // plane that disagreed about which place comes first would leave nobody to
    // draw the wall between them (`DW0827`, spec-0098 §2 rule 3c).
    edges.push(json!({
        "id": "edge/out-of-the-pit", "a": gentle_top.node, "b": pit.node, "class": "stair",
    }));
    seams.push(json!({
        "edge": "edge/out-of-the-pit", "face": "south", "at": 8, "meets": 8, "opening": "arch",
        "stair_in": pit.node,
        "form": format!("an arch at the head of the {rise}-block stair out of the pit"),
    }));

    // ------------------------------------------------------------- the region
    //
    // Extent flows DOWN: the region is stated, and every box is inside it. It is
    // computed from the bays because the bays are the brief here.
    let all: Vec<&Bay> = bays.iter().chain([&steep_top, &gentle_top, &pit]).collect();
    let far_x = all
        .iter()
        .map(|b| b.min[0] + b.extent[0])
        .max()
        .unwrap_or(0);
    let far_z = all
        .iter()
        .map(|b| b.min[1] + b.extent[1])
        .max()
        .unwrap_or(0);
    let top_y = landing + gentle_top.clearance;
    let region_min = [0i64, GRADE_Y - 16, 0i64];
    let region_extent = [far_x + 8, top_y - (GRADE_Y - 16) + 8, far_z + 8];

    let entry_node = bays[0].node.clone();
    let goal_node = bays[bays.len() - 1].node.clone();
    let critical_path: Vec<Value> = bays.iter().map(|b| json!(b.node)).collect();

    // How long the gym is, in minutes, from the bays' own long extents and the
    // pacing coefficient — the same rule `DW0822` states, applied here rather
    // than restated.
    let coefficient = pacing(table, &mut reads, "route-blocks-per-minute");
    let nominal: i64 = bays.iter().map(|b| b.extent[0].max(b.extent[1])).sum();
    let target_minutes = ceil_div(nominal, coefficient.max(1)).max(1);

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
                "time": "noon",
                "weather": "clear",
                "theme": "A gym of bays: the doorways, the climbs and the fall.",
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
                    "backstory": "She has stood in the first bay since before any of the standards \
                                  had numbers, and she writes down what each walker says about \
                                  them.",
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
                    "goal": "Walk the bays through every standard doorway, up both climbs and \
                             down the fall, and say which standards are wrong.",
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
                        "text": "Walk east through every doorway, then up and down, and tell me \
                                 where you stopped believing in the standards.",
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
                        "text": "Whether a doorway is one you would put a party through. Whether the climb is one you \
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
                    "text": "The party arrives at the first bay.",
                },
                "cast": { "npc/invigilator": {
                    "at": format!("anchor/node-{}", slug(&entry_node)),
                    "dialogue": "dlg/greeting",
                    "doing": "standing in the first bay with a rule in her hand",
                }},
                "objectives": [
                    {
                        "id": "obj/hear-the-brief",
                        "type": "talk-to",
                        "npc": "npc/invigilator",
                        "title": "Hear the brief",
                        "hint": "The Invigilator stands in the first bay.",
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
                            "text": "The party reaches the last bay.",
                        },
                    },
                ],
                "on_complete": [{
                    "type": "campaign-complete",
                    "happening": { "verb": "departs", "text": "The bays have been walked end to end." },
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
                    "id": "fact/landing-datum",
                    "unit": "blocks",
                    "value": landing as f64,
                    "note": "Where both climbs land, and the lip the designed fall goes over: one \
                             low storey above grade, so one plane demonstrates three things.",
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
                    { "id": "datum/grade", "y": GRADE_Y, "note": "The plane the bays stand on." },
                    { "id": "datum/landing", "y": landing, "note": "What both climbs reach and the fall leaves." },
                ],
                "boxes": boxes,
                "seams": seams,
                "identities": [
                    { "fact": "fact/landing-datum", "cmp": "eq",
                      "measure": { "of": "datum-y", "datum": "datum/landing" } },
                ],
                "lighting": { "fixture": "torch", "min_light": 7 },
                // Open ground at grade between the bays, so the walker sees each
                // one stand on the same plane (spec-0098 §2b).
                "fill": {
                    "kind": "open",
                    "terrain": { "kind": "flat", "datum": "datum/grade" },
                    "surface": "minecraft:grass_block",
                    "below": "minecraft:dirt",
                },
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
