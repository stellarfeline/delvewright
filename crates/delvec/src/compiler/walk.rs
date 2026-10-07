//! **The walk: the owner walks the build that ships, and the record names it**
//! (spec-0049 §5.4).
//!
//! A walk is taken on the DETAILED world — real buildings, real materials — and
//! never on a blockout: a blockout somebody stood in told them almost nothing.
//! So nothing here holds detail work. Detail comes first, the walk comes after
//! it, and what the walk holds is what comes after IT: the build being shipped.
//!
//! # What the record is bound to
//!
//! `walk-record.json` names the build that was walked by three hashes, every
//! one over what the engine DERIVES rather than over a document's bytes:
//!
//! * the **grid** ([`walked_grid`]) — where every box stands and where every
//!   seam is cut;
//! * the **ways** ([`walked_ways`]) — what each seam is and what a body must
//!   hold to pass it;
//! * the **detail** ([`walked_detail`]) — which piece stands in which place,
//!   the bytes of every template of every bound piece, and where each owed
//!   anchor name was re-bound to. A blockout has a detail half too: the one
//!   that binds nothing, so a record taken on a blockout names it and is never
//!   mistaken for a walk of a detailed build.
//!
//! The engine that built it is named beside them by revision, and the massing
//! with nothing bound by [`blockout_sha256`], which the drift advisory reads.
//!
//! # What invokes each check, and what happens without it
//!
//! | check | event it is bound to |
//! |---|---|
//! | `DW0974` ([`check`]) | `validate_loaded` in `delvec`'s `main` — the one funnel every subcommand's validation goes through, `build` included |
//! | `DW0974` the advisory ([`drift`]) | the same funnel, after [`check`] |
//!
//! A record is not demanded: the campaign has none until its owner has walked
//! it, and every build before then is the build that walk needs. A record that
//! IS present must describe the build it stands beside, so a record cannot
//! survive the edit that made it a record of some other build — a layout-graph
//! edit, a moved box, a re-made place, or detail added to the blockout it was
//! taken on. The `verdict` is not read by any refusal here: a `findings` or
//! `unwalked` record of this build is a true statement about it, and what reads
//! the verdict is the release, which ships only a build whose record is
//! `passed`.

use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use delvewright_dsl::detailplan::Detail;
use delvewright_dsl::layout::{LayoutGraphContent, Node, Station};
use delvewright_dsl::metrics::Reads;
use delvewright_dsl::siteplan::{PlacedBox, PlacedSeam, SitePlanContent};
use delvewright_dsl::{Campaign, Diagnostic, DwCode, ExitTier};

use crate::compiler::registry::PrefabRegistry;

/// The stage name every diagnostic here carries — the document being judged.
const STAGE: &str = "walk-record";

delvewright_dsl::dw_code! {
    /// `DW0974`: a walk record that does not describe this build.
    pub const DW_NOT_THIS_BUILD: DwCode = DwCode::new("DW0974", ExitTier::Build);
}

// ---------------------------------------------------------------------------
// The instrument (spec-0050 §2)
// ---------------------------------------------------------------------------

/// **The engine revision, named literally.**
///
/// `CLAUDE.md`'s rule that a frozen measurement names its instrument by
/// revision, never by a version string, is the reason this is not
/// `DELVEC_VERSION`: two engines 136 commits apart report the same version.
///
/// Stamped at COMPILE time, by `crates/delvec/build.rs` (spec-0050 §2). A
/// source build reads the revision out of the checkout it is being built from,
/// suffixed `-dirty` when that tree carries uncommitted changes; a release
/// recipe or container build that has the revision and no `.git` passes
/// `DELVEC_ENGINE_REVISION` in the environment and that wins unchanged.
///
/// `unstamped` is what is left when neither can be established — a source
/// tarball such as crates.io serves, with no `.git` to read. What the engine
/// must never do is *claim* a revision it does not have, and `unstamped` is
/// that claim withheld. It is the fallback, not the normal answer: a campaign
/// author copies this field into `walk-record.json`, and a field that always
/// holds one constant is not a measurement.
#[must_use]
pub fn engine_revision() -> &'static str {
    option_env!("DELVEC_ENGINE_REVISION").unwrap_or("unstamped")
}

/// How the engine names itself in a record or a refusal: the revision, and the
/// version beside it as context rather than as the name.
#[must_use]
pub fn engine_name() -> String {
    format!(
        "{rev} (delvec {ver})",
        rev = engine_revision(),
        ver = crate::compiler::DELVEC_VERSION
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let mut s = String::with_capacity(64);
    for b in h.finalize() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// **The derived grid, canonically** — the text [`site_plan_sha256`] is taken
/// over, one line per object in derivation order.
///
/// Public because a digest is one-way. A creator whose gate re-opened diffs two
/// builds' grids to find what moved, and every test that says what this key does
/// and does not see reads the text rather than the hash.
///
/// **Every derived object is destructured exhaustively, with no `..`.** That is
/// the closure property this key has instead of whole-document bytes: a field
/// added to [`PlacedBox`] or [`PlacedSeam`] stops this crate compiling until
/// somebody decides which half of the key it belongs in. A projection nobody is
/// forced to extend is the defect the document-bytes key existed to avoid, and
/// the compiler is a stronger reminder than a comment.
///
/// What is deliberately NOT here, each because the walked whole cannot see it:
/// `datums[]` (a datum's `y` arrives as a box's `floor` and a datum nothing
/// names has no consequence), `identities[]` and `lighting` (they configure what
/// the engine CHECKS about the whole — `DW0871` and `DW0210` are their gates —
/// they do not build it), `sightlines[]` and `views[]` (a claimed line of sight
/// and a render viewpoint; neither is massing), and every `note`. A seam's
/// `class` is not here either: it is authored in the graph and it is the
/// [`walked_ways`] half that carries it, so a way that turns from arch to bar
/// re-opens the gate naming the document that was edited.
#[must_use]
pub fn walked_grid(c: &Campaign) -> Option<String> {
    let plan = c.site_plan.as_ref()?;
    let mut reads = Reads::new();
    let boxes = delvewright_dsl::siteplan::placed_boxes(c, &mut reads);
    let seams = delvewright_dsl::siteplan::placed_seams(c, &boxes, &mut reads);
    let SitePlanContent {
        region,
        datums: _,
        boxes: _,
        seams: _,
        volumes,
        identities: _,
        sightlines: _,
        views: _,
        lighting: _,
    } = &plan.content;

    let mut t = format!(
        "grid {b} box(es) {s} seam(s) {v} volume(s)\n",
        b = boxes.len(),
        s = seams.len(),
        v = volumes.len(),
    );
    t.push_str(&format!("region {region:?}\n"));
    for b in &boxes {
        let PlacedBox {
            node,
            foot,
            floor,
            clearance,
            open,
        } = b;
        t.push_str(&format!(
            "box {n} foot {foot:?} floor {floor} clearance {clearance} open {open}\n",
            n = node.0,
        ));
    }
    for s in &seams {
        // `class` goes to the ways half — see this function's own note.
        let PlacedSeam {
            edge,
            class: _,
            a,
            b,
            face,
            normal_axis,
            plane,
            opening,
            shared,
            crossing,
            rise,
            stair_in,
        } = s;
        t.push_str(&format!(
            "seam {e} {a} {b} face {face:?} axis {normal_axis} plane {plane} opening {opening:?} \
             shared {shared:?} crossing {crossing:?} rise {rise} stair_in {stair_in:?}\n",
            e = edge.0,
            a = a.0,
            b = b.0,
        ));
    }
    for v in volumes {
        t.push_str(&format!(
            "volume {id} {role:?} {region:?}\n",
            id = v.id.0,
            role = v.role,
            region = v.region,
        ));
    }
    Some(t)
}

/// **The ways a body moves by, canonically** — the text [`layout_graph_sha256`]
/// is taken over.
///
/// The grid says where the space is; this says what a body may do in it, and
/// none of it is written in the plan. An [`Edge`] is printed whole, by `Debug`,
/// because **every field an edge has is a traversal fact** — its class is the
/// variant name, and `one_way`, `falls`, `shortcut`, `gating` and `opens_from`
/// are the rest of it. There is no prose on an edge, so a field added to one
/// belongs in this key by default and lands in it without anybody remembering.
///
/// A node is destructured exhaustively instead, because a node does carry prose.
/// `intent` and `note` are the two fields the DSL itself documents as *no check
/// keys on this*; `size_class` and `way_class` are left out because their whole
/// consequence is the box's footprint and headroom, which [`walked_grid`]
/// already holds — counting them twice would move both halves of the key on one
/// edit and send the reader to the wrong repair. `stations` stay: a station is a
/// named place realized in the massing, and a `gate` station writes a bar.
#[must_use]
pub fn walked_ways(c: &Campaign) -> Option<String> {
    let graph = c.layout_graph.as_ref()?;
    let LayoutGraphContent {
        nodes,
        edges,
        entry,
        goal,
        critical_path,
        beats,
    } = &graph.content;

    let mut t = format!(
        "ways {e} edge(s) {n} node(s) {b} beat(s)\n",
        e = edges.len(),
        n = nodes.len(),
        b = beats.len(),
    );
    t.push_str(&format!("entry {}\ngoal {}\n", entry.0, goal.0));
    t.push_str(&format!(
        "critical-path {}\n",
        critical_path
            .iter()
            .map(|n| n.0.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    for e in edges {
        t.push_str(&format!("edge {e:?}\n"));
    }
    for n in nodes {
        let Node {
            id,
            intent: _,
            size_class: _,
            way_class: _,
            note: _,
            stations,
        } = n;
        for st in stations {
            let Station {
                anchor,
                kind,
                note: _,
            } = st;
            t.push_str(&format!(
                "station {node} {a} {kind:?}\n",
                node = id.0,
                a = anchor.as_str(),
            ));
        }
    }
    for b in beats {
        t.push_str(&format!(
            "beat {q} {o} {n}\n",
            q = b.quest.0,
            o = b.objective.0,
            n = b.node.0,
        ));
    }
    Some(t)
}

/// **The grid half of the freshness key**: sha256 over [`walked_grid`].
///
/// Over the DERIVED GRID, not over `site-plan.json`'s bytes. The gate asks
/// whether the whole a walker walked is the whole this campaign now builds, and
/// a document's bytes answer a narrower question that happens to correlate: they
/// move for a `dsl_version` bump, a reworded `note`, a renamed intent and a
/// re-serialization, none of which a body can feel. A key that re-opens on those
/// demands the one repair nobody can honestly perform — walk the whole again for
/// a change that cannot have changed the walk — and a gate whose remedy is
/// unperformable is discharged by hand, which is what happened to it.
///
/// So the key is the grid: every box's corner, extent, floor and headroom, and
/// every seam's cells, crossing and rise, with the whole's own volumes and the
/// region they stand in. Moving one extent by one block moves it.
#[must_use]
pub fn site_plan_sha256(c: &Campaign) -> Option<String> {
    Some(sha256_hex(walked_grid(c)?.as_bytes()))
}

/// **The ways half of the freshness key**: sha256 over [`walked_ways`].
///
/// Not a refinement of the first half: **the whole that is walked is derived
/// from two authored documents**, and only one of them is the plan. A box's
/// headroom comes from its node's `size_class`; a seam's opening is cut to air
/// or filled with the bar according to its edge's `class`; the side an
/// `anchor/unlock-…` stands on is the edge's `opens_from`; what a body must hold
/// to pass is its `gating`; and which way a fall goes is its `falls`. None of
/// that is stated anywhere in the plan, and the second and third of them move no
/// byte the grid can see.
#[must_use]
pub fn layout_graph_sha256(c: &Campaign) -> Option<String> {
    Some(sha256_hex(walked_ways(c)?.as_bytes()))
}

/// **The blockout's hash**: sha256 over the derived massing the WALK judged.
///
/// Taken over [`crate::compiler::blockout::walked_massing`] — the derivation with nothing
/// bound — for the reason that function's own note gives.
#[must_use]
pub fn blockout_sha256(c: &Campaign) -> Option<String> {
    let mut reads = Reads::new();
    let fills = crate::compiler::blockout::walked_massing(c, &mut reads)?;
    let mut text = String::new();
    for f in &fills {
        text.push_str(&format!(
            "{} {} {} {} {} {} {}\n",
            f.from[0], f.from[1], f.from[2], f.to[0], f.to[1], f.to[2], f.block
        ));
    }
    Some(sha256_hex(text.as_bytes()))
}

/// **What detailing put in the whole, canonically** — the text
/// [`detail_sha256`] is taken over.
///
/// The grid and the ways say where the space is and what a body may do in it;
/// this says what STANDS in it. One line per `details[]` row, in place order so
/// that reordering the rows moves nothing, each with:
///
/// * the place and the piece;
/// * every template the piece is built from — its id, its offset and size
///   inside the piece, and the sha256 of its `.nbt` bytes as they lie in the
///   prefab directory, so a piece re-made from an edited program is a
///   different detail even under the same name;
/// * every owed anchor name the row re-binds, with the cell and facing of the
///   piece anchor it is bound to, because that is where the quest layer's
///   bodies and completion volumes now stand.
///
/// What is deliberately NOT here: the `palette`, which is handed to a program
/// and never placed by the engine — the piece's own bytes are what a body
/// stands in, and they are here.
///
/// A campaign with no `detail-plan` has a detail half too, and it is the one
/// that binds nothing ([`BLOCKOUT_DETAIL`]). That is what lets a record taken on
/// a blockout be told apart from a record of a detailed build, rather than read
/// as one by a key that did not look.
#[must_use]
pub fn walked_detail(c: &Campaign, prefabs: &PrefabRegistry, dir: &Path) -> String {
    let rows: &[Detail] = c
        .detail_plan
        .as_ref()
        .map_or(&[], |e| e.content.details.as_slice());
    let mut sorted: Vec<&Detail> = rows.iter().collect();
    sorted.sort_by(|a, b| a.place.0.cmp(&b.place.0));
    let mut t = format!("detail {} row(s)\n", rows.len());
    for row in sorted {
        let Detail {
            place,
            piece,
            anchors,
        } = row;
        t.push_str(&format!("place {} piece {}\n", place.0, piece.0));
        let Some(meta) = prefabs.get(piece.as_str()) else {
            // `DW0842` refuses it; the key still names the absence.
            t.push_str("  piece absent from the library\n");
            continue;
        };
        for tpl in meta.templates() {
            let bytes = match std::fs::read(dir.join(tpl.file)) {
                Ok(b) => sha256_hex(&b),
                Err(_) => "unreadable".to_string(),
            };
            t.push_str(&format!(
                "  template {id} offset {off:?} size {size:?} sha256 {bytes}\n",
                id = tpl.id,
                off = tpl.offset,
                size = tpl.size,
            ));
        }
        for (name, bound_to) in anchors {
            let at = meta
                .anchors
                .get(bound_to)
                .map(|a| format!("{:?} {:?}", a.pos, a.facing))
                .unwrap_or_else(|| "absent".to_string());
            t.push_str(&format!("  anchor {name} -> {bound_to} {at}\n"));
        }
    }
    t
}

/// The detail half of a build that binds nothing — a blockout.
pub const BLOCKOUT_DETAIL: &str = "detail 0 row(s)\n";

/// **The detail half of the key**: sha256 over [`walked_detail`].
#[must_use]
pub fn detail_sha256(c: &Campaign, prefabs: &PrefabRegistry, dir: &Path) -> String {
    sha256_hex(walked_detail(c, prefabs, dir).as_bytes())
}

/// The hashes a walk record names, and the engine that produced them, printed
/// on every validation of a site-plan campaign so a record can name its
/// subject and its instrument.
///
/// Three of them are the **key** — the grid, the ways, and what detailing put
/// in the whole — and the fourth is the massing with nothing bound, which is
/// what the drift advisory reads. None is over a document's bytes: a hash a
/// creator could compute from `site-plan.json` with `sha256sum` would move on a
/// `dsl_version` bump and a reformat, which is a record going stale for a change
/// no body can feel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    /// Over the derived grid ([`walked_grid`]).
    pub site_plan: String,
    /// Over the ways a body moves by ([`walked_ways`]).
    pub layout_graph: String,
    /// Over what detailing put in the whole ([`walked_detail`]).
    pub detail: String,
    /// Over the massing with nothing bound ([`blockout_sha256`]).
    pub blockout: String,
}

impl Hashes {
    /// The hashes of `c`, or `None` for a campaign with no site plan or no
    /// layout graph — a whole with either missing is not a whole anything walks.
    #[must_use]
    pub fn of(c: &Campaign, prefabs: &PrefabRegistry, dir: &Path) -> Option<Hashes> {
        Some(Hashes {
            site_plan: site_plan_sha256(c)?,
            layout_graph: layout_graph_sha256(c)?,
            detail: detail_sha256(c, prefabs, dir),
            blockout: blockout_sha256(c)?,
        })
    }

    /// Lines for stderr, for whoever is writing the walk record.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "site plan sha256:    {sp}\nlayout graph sha256: {lg}\ndetail sha256:       {dt}\n\
             blockout sha256:     {bo}\nengine revision:     {rev}",
            sp = self.site_plan,
            lg = self.layout_graph,
            dt = self.detail,
            bo = self.blockout,
            rev = engine_name(),
        )
    }
}

// ---------------------------------------------------------------------------
// The walk record (spec-0049 §5.4)
// ---------------------------------------------------------------------------

/// What this record says happened.
///
/// **Two of these three describe a walk; the third says there was none.** With
/// only the first two, every legal record asserted a walk, and the states this
/// pipeline produces before one has happened (a build stood up and taken down,
/// a walk abandoned, a walk cut short) had no legal spelling: the truth could
/// only go into `findings[]`, which is free prose nothing reads. `Unwalked` is
/// the value that lets the document state its own subject.
///
/// Only [`Verdict::Passed`] lets the build ship, and nothing here decides
/// whether a body was in the world: that a human walked is this document's
/// author's assertion, held by operating practice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// The build was walked and may ship.
    Passed,
    /// The build was walked and something must change first.
    Findings,
    /// **Nobody walked this build.** The record exists to say so — a build
    /// stood up and taken down, a walk abandoned, a walk cut short. It is
    /// never written in place of a walk that happened, and it never becomes
    /// `passed` by anything but a walk.
    Unwalked,
}

impl Verdict {
    /// **Every spelling this closed set admits**, read off the type's own
    /// schema — the one `delvec schema --stage walk-record` exports — so a
    /// message that lists the set cannot fall behind a variant added to it.
    #[must_use]
    pub fn tokens() -> Vec<String> {
        let v = serde_json::to_value(schemars::schema_for!(Verdict))
            .expect("the verdict schema serializes to JSON");
        let from_one_of = v["oneOf"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["const"].as_str());
        let from_enum = v["enum"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(serde_json::Value::as_str);
        from_one_of.chain(from_enum).map(str::to_string).collect()
    }

    /// The token this value is spelled by.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Verdict::Passed => "passed",
            Verdict::Findings => "findings",
            Verdict::Unwalked => "unwalked",
        }
    }
}

/// One thing a walk found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WalkFinding {
    /// What it is about — a place, a seam, a view.
    pub subject: String,
    /// What was wrong, in the walker's words.
    pub note: String,
}

/// **`walk-record.json`** — the record of the owner walking one build.
///
/// It is not a stage document and carries no `dsl_version`: it is not
/// authored against a schema version, it is the record of an event.
///
/// The machine half is **that the record describes this build, and an explicit
/// verdict**, stated plainly rather than implied. That a human actually walked
/// is this document's author's assertion, held by operating practice; no engine
/// check can prove a walk happened and nothing here pretends one can.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WalkRecord {
    /// The **grid** that was walked, by [`site_plan_sha256`] — the derived space
    /// the site plan resolves to, never that document's bytes. Named for the
    /// document a creator goes to when it moves.
    pub site_plan_sha256: String,
    /// The **ways** that were walked, by [`layout_graph_sha256`]: what each seam
    /// IS — walk, stair, drop or bar, which side opens it, what a body must hold
    /// to pass — none of which the grid can see.
    pub layout_graph_sha256: String,
    /// **What stood in the whole** when it was walked, by [`detail_sha256`]:
    /// every bound place, its piece's bytes, and where its owed names were
    /// re-bound. A blockout's is the hash of a build that binds nothing.
    pub detail_sha256: String,
    /// The massing with nothing bound, by its hash — the drift advisory's.
    pub blockout_sha256: String,
    /// The engine that built it — the revision, never a version string.
    pub engine_revision: String,
    /// What this record says happened — and `unwalked` is one of the three,
    /// so a record is never forced to assert a walk in order to exist.
    pub verdict: Verdict,
    /// What was found. Present and non-empty is compatible with `passed`: a
    /// walker may note something without it holding the build back.
    #[serde(default)]
    pub findings: Vec<WalkFinding>,
}

impl WalkRecord {
    /// Parse a record, or say why it is not one.
    pub fn parse(src: &str) -> Result<WalkRecord, String> {
        serde_json::from_str(src).map_err(|e| e.to_string())
    }
}

/// The JSON Schema for `walk-record.json`, exported by
/// `delvec schema --stage walk-record`.
///
/// It is a campaign artifact, not a stage document — no `dsl_version`, no
/// `campaign_id`, no `stage` — and none of that is a reason to leave the person
/// who has to WRITE it reading prose. It is hand-authored and refused when it is
/// wrong (`DW0974`), so it is derived from [`WalkRecord`] exactly as every stage
/// schema is derived from its type: one authority for the form.
#[must_use]
pub fn walk_record_schema() -> serde_json::Value {
    let mut v = serde_json::to_value(schemars::schema_for!(WalkRecord))
        .expect("the walk-record schema serializes to JSON");
    if let Some(obj) = v.as_object_mut() {
        obj.insert(
            "title".into(),
            serde_json::Value::String("walk-record.json".into()),
        );
        obj.insert(
            "description".into(),
            serde_json::Value::String(WALK_RECORD_SCHEMA_DESCRIPTION.into()),
        );
    }
    v
}

/// What the exported schema tells its reader the document IS — stated on the
/// schema rather than only in a reference document, because the schema is what
/// the authoring step actually opens.
const WALK_RECORD_SCHEMA_DESCRIPTION: &str = "\
The record of the owner walking one build, written by hand beside the stage \
documents once the walk is done. The walk is taken on the DETAILED world, after \
detail, so the build it names is the build that ships.

A CAMPAIGN ARTIFACT, NOT A STAGE DOCUMENT. It records an event rather than \
being authored against a schema version, so it carries no `dsl_version`, no \
`campaign_id` and no `stage` — the three fields every stage document must \
have. It is not a build input either: re-recording a walk moves no emitted \
byte.

Every field but `findings` is required. The four hashes and the engine \
revision are all printed by `delvec validate` and `delvec build` on a \
site-plan campaign — copy them from the output of the build that was walked, \
which is the only place they exist: NONE of them is a hash of a document, so \
`sha256sum site-plan.json` does not produce one. `site_plan_sha256`, \
`layout_graph_sha256` and `detail_sha256` are the key, and each is over what \
the engine DERIVES: the grid (every box's corner, extent, floor and headroom, \
every seam's cells, crossing and rise, the whole's volumes and region), the \
ways a body moves by (every edge whole, the entry, the goal, the critical path, \
the beats, the stations), and what detailing put in the whole (every bound \
place, the bytes of its piece, and where its owed anchor names were re-bound — \
a blockout's is the hash of a build that binds nothing). Moving a box one \
block, changing a way, or re-making a place makes the record a record of some \
other build; a `dsl_version` bump, a reformat, a reworded note or a renamed \
intent does not.

`verdict` is one of THREE values and the third is the one to reach for when \
no walk happened. `passed` — the build was walked and may ship. `findings` — \
the build was walked and something must change first. `unwalked` — NOBODY \
WALKED IT: a build stood up and taken down, a walk abandoned, a walk cut short. \
Write `unwalked` for every one of those. It is the only value that does not \
assert a walk, and asserting one that did not happen is the thing this \
document must never be made to do; `findings[]` is free prose and nothing \
reads it, so a truth put only there changes nothing. `DW0974` refuses every \
build while this file is unparseable or names a build other than the one \
beside it, and it names which hash moved. Only a `passed` record of the build \
being shipped lets it ship.";

/// What the walk check examined, with its denominator.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WalkBinding {
    /// Records read — 0 or 1. Zero is a campaign nobody has walked yet, which
    /// is every campaign before its walk, and refuses nothing.
    pub records: usize,
    /// Key hashes compared, out of [`Self::KEYED`]. Stated with its
    /// denominator: a check that compared two of the three halves of the build
    /// is green about a smaller build than the one it claims to cover.
    pub compared: usize,
    /// The verdict a record that describes this build carries — `None` when
    /// there is no such record.
    pub verdict: Option<Verdict>,
}

impl WalkBinding {
    /// The halves the walked build is made of, and therefore the denominator
    /// of [`Self::compared`]: the grid, the ways, and what stands in them.
    pub const KEYED: usize = 3;

    /// One line, stated whether or not it is zero.
    #[must_use]
    pub fn line(&self) -> String {
        let verdict = match self.verdict {
            Some(v) => format!("a record of THIS build, verdict `{}`", v.token()),
            None if self.records == 0 => {
                "no walk record — this build has not been walked".to_string()
            }
            None => "the record does not describe this build".to_string(),
        };
        format!(
            "walk binding: {r} walk record(s) read, {c} of {d} key hash(es) compared \
             (grid, ways, detail); {verdict}.",
            r = self.records,
            c = self.compared,
            d = Self::KEYED,
        )
    }
}

/// **`DW0974`: a walk record that does not describe this build.**
///
/// Bound in validation, so every build asks it. A campaign with no record is
/// not refused — it is the campaign nobody has walked yet, and its build is the
/// build that walk needs. A record that is present must parse and must name
/// this build in all three halves of the key; the half that moved is named, both
/// of its hashes are printed, and a record taken on a blockout beside a detailed
/// build is said to be exactly that.
///
/// The refusal is on presence, never on the verdict: a `findings` or `unwalked`
/// record of this build is a true statement about it.
#[must_use]
pub fn check(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    dir: &Path,
    record: Option<&str>,
) -> (Vec<Diagnostic>, WalkBinding) {
    let mut binding = WalkBinding::default();
    let Some(src) = record else {
        return (Vec::new(), binding);
    };
    binding.records = 1;
    let refuse = |pointer: &str, message: String| {
        vec![Diagnostic::error(
            DW_NOT_THIS_BUILD,
            STAGE,
            pointer,
            message,
        )]
    };
    let Some(now) = Hashes::of(c, prefabs, dir) else {
        return (
            refuse(
                "",
                format!(
                    "`walk-record.json` names a site-plan build, and this campaign has no \
                     {missing}, so there is no build for it to describe. A walk record is \
                     written for a campaign placed by a site plan and a layout graph. Binding: \
                     1 record read, 0 of {d} key hash(es) compared.",
                    missing = if c.site_plan.is_none() {
                        "`site-plan.json`"
                    } else {
                        "`layout-graph.json`"
                    },
                    d = WalkBinding::KEYED,
                ),
            ),
            binding,
        );
    };
    let rec = match WalkRecord::parse(src) {
        Ok(r) => r,
        Err(e) => {
            return (
                refuse(
                    "",
                    format!(
                        "`walk-record.json` is not a walk record: {e}. Its form is fixed — \
                         `site_plan_sha256`, `layout_graph_sha256`, `detail_sha256`, \
                         `blockout_sha256`, `engine_revision`, `verdict` (one of {verdicts}), \
                         and `findings[]` of `{{subject, note}}`. A record that does not parse \
                         is a record nothing can be judged against, so it is a refusal rather \
                         than an absence. Binding: 1 record read, 0 of {d} key hash(es) \
                         compared.",
                        verdicts = Verdict::tokens()
                            .iter()
                            .map(|t| format!("`{t}`"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        d = WalkBinding::KEYED,
                    ),
                ),
                binding,
            );
        }
    };
    let remedy = "A record describes the one build that was walked, and this build is not \
                  that one. Walk this build and re-record from its output, or remove the record: \
                  it is a record of a build that no longer exists, and nothing is shipped without \
                  a `passed` record of the build being shipped.";
    binding.compared = 1;
    if rec.site_plan_sha256 != now.site_plan {
        return (
            refuse(
                "/site_plan_sha256",
                format!(
                    "`walk-record.json` records a walk of a DIFFERENT GRID. The record names \
                     `{recorded}`; this campaign's grid hashes to `{current}`. The hash is over \
                     the grid the engine DERIVES — every box's corner, extent, floor and \
                     headroom, every seam's cells, crossing and rise, the whole's own volumes \
                     and the region they stand in — so a reformat, a `dsl_version` bump, a \
                     reworded note or a renamed intent is not a different build, and a box that \
                     moved one block is. {remedy} Binding: 1 record read, 1 of {d} key hash(es) \
                     compared.",
                    recorded = rec.site_plan_sha256,
                    current = now.site_plan,
                    d = WalkBinding::KEYED,
                ),
            ),
            binding,
        );
    }
    binding.compared = 2;
    if rec.layout_graph_sha256 != now.layout_graph {
        return (
            refuse(
                "/layout_graph_sha256",
                format!(
                    "`walk-record.json` records a walk of DIFFERENT WAYS. The record names \
                     `{recorded}`; this campaign's ways hash to `{current}`. The grid is \
                     unchanged, and that is not enough: a seam that was an open archway and is \
                     now a quest-locked bar, a door that now opens from the other side, a fall \
                     that now falls the other way, a way that gained a flag to pass, a station \
                     that moved place or a beat that moved node is a different build standing \
                     on the same grid. The hash is over the LAYOUT GRAPH's traversal content — \
                     every edge whole, the entry, the goal, the critical path, the beats and \
                     every station — never over its bytes. {remedy} Binding: 1 record read, 2 \
                     of {d} key hash(es) compared.",
                    recorded = rec.layout_graph_sha256,
                    current = now.layout_graph,
                    d = WalkBinding::KEYED,
                ),
            ),
            binding,
        );
    }
    binding.compared = 3;
    if rec.detail_sha256 != now.detail {
        let blockout = sha256_hex(BLOCKOUT_DETAIL.as_bytes());
        let what = if rec.detail_sha256 == blockout {
            "a walk of the BLOCKOUT — the build with no place detailed — and this build is \
             detailed. A blockout walk is not a walk of the world that ships: the walk is taken \
             on the detailed world, after detail, with its real buildings and materials"
                .to_string()
        } else if now.detail == blockout {
            "a walk of a DETAILED build, and this build binds no place — the detail it was \
             walked with is gone"
                .to_string()
        } else {
            "a walk of DIFFERENT DETAIL — a place bound, unbound or re-made, a piece whose \
             bytes changed, or an owed name re-bound to another anchor since the walk"
                .to_string()
        };
        return (
            refuse(
                "/detail_sha256",
                format!(
                    "`walk-record.json` records {what}. The record names `{recorded}`; this \
                     build's detail hashes to `{current}`. The grid and the ways are unchanged. \
                     {remedy} Binding: 1 record read, 3 of {d} key hash(es) compared.",
                    recorded = rec.detail_sha256,
                    current = now.detail,
                    d = WalkBinding::KEYED,
                ),
            ),
            binding,
        );
    }
    binding.verdict = Some(rec.verdict);
    (Vec::new(), binding)
}

/// **The drift advisory**: the same walked build, different massing bytes.
///
/// A warning naming both hashes and both engine revisions, and never a refusal.
/// The derivation [`blockout_sha256`] hashes is a pure function of the site
/// plan, the layout graph, the metrics table and the engine, and everything it
/// reads out of the two documents is in [`check`]'s key — the placed boxes and
/// seams and the plan's volumes and region ([`walked_grid`]); the entry, every
/// edge whole and every station ([`walked_ways`]). This runs only where the
/// whole key compared equal, so what is left to have moved is the TOOLCHAIN,
/// and whether the build is walked again is a decision for the round summary
/// rather than a defect anyone could launder through it.
///
/// A record [`check`] refuses never reaches this text: a warning asserting an
/// unchanged whole beside a refusal saying it changed would train its reader to
/// wave the state through.
#[must_use]
pub fn drift(
    c: &Campaign,
    prefabs: &PrefabRegistry,
    dir: &Path,
    record: Option<&str>,
) -> Option<Diagnostic> {
    let rec = WalkRecord::parse(record?).ok()?;
    let now = Hashes::of(c, prefabs, dir)?;
    if rec.site_plan_sha256 != now.site_plan
        || rec.layout_graph_sha256 != now.layout_graph
        || rec.detail_sha256 != now.detail
        || rec.blockout_sha256 == now.blockout
    {
        return None;
    }
    Some(Diagnostic::warning(
        DW_NOT_THIS_BUILD,
        STAGE,
        "/blockout_sha256",
        format!(
            "the walked massing has MOVED under an unchanged grid, unchanged ways and unchanged \
             detail. The record was taken on engine `{was_rev}` and names blockout `{was}`; \
             engine `{now_rev}` derives `{now}` from the same whole. This refuses nothing. What \
             is left to have moved is the TOOLCHAIN — the engine or the metrics table — because \
             the massing is a pure function of the site plan, the layout graph, the metrics \
             table and the engine, and everything it reads out of those two documents is in the \
             key `DW0974` has just compared and found equal. Whether the build is walked again \
             is a decision for the round summary rather than a defect to repair.",
            was = rec.blockout_sha256,
            was_rev = rec.engine_revision,
            now = now.blockout,
            now_rev = engine_name(),
        ),
    ))
}
