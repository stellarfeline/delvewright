//! `delvec prefab extract` — a prefab is lifted out of a built campaign
//! (spec-0106 §3.9).
//!
//! One command, one derivation, restricted to a **scope**: a place or several
//! (`node/<a>[,node/<b>…]`), a declared structure (a bound piece's contract
//! space, `node/<place>/<space>`, or a gate station's region, `anchor/<gate>`),
//! or a cuboid of coordinates (`x0,y0,z0..x1,y1,z1`). The source is the build
//! `delvec build` would ship, run in memory through the same funnel
//! ([`crate::cli::campaign::build_in_memory`]); the bytes are the world as
//! shipped inside the cuboid — the pieces, the stage-7 edits, the relight's
//! fixtures, the world-load gate seals and trigger props — and every field of
//! the document is derived. What the scope severs is printed and written as
//! the piece's `residue`; what the derivation cannot place is `DW1023`.
//!
//! Determinism (ADR-0006): the source build is deterministic, every walk here
//! is over a `BTreeMap`/`BTreeSet` or a document-order slice, and nothing
//! reads a clock, a path or an environment. Two extractions of one build write
//! the same bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use delvec::admit::light::{DEFAULT_DARK_THRESHOLD, SkyClaim};
use delvec::compiler::assembled;
use delvec::compiler::detail::{self as engine, DW_EXTRACTION_CANNOT_PLACE};
use delvec::grammar::block::BlockState as GBlock;
use delvec::grammar::geom::Box3;
use delvec::grammar::model::VoxelModel;
use delvewright_dsl::detailplan::Frame;
use delvewright_dsl::layout::Station;
use delvewright_dsl::prefab::{
    Anchor, ContractEdge, ContractFace, ContractSpace, ExtractedFrom, Extraction, License,
    PieceEntity, PieceOpening, PrefabMeta, Region, Residue, SpatialContract,
};
use delvewright_dsl::siteplan::{Base, Face};
use delvewright_dsl::{Campaign, Diagnostic, NodeId};
use sha2::{Digest, Sha256};

use crate::EXIT_INTERNAL;
use crate::cli::campaign::{Built, build_in_memory};
use crate::cli::report::print_one_diag;

/// The generator name an extracted piece's structure block carries.
pub const GENERATOR: &str = "delvec prefab extract";

/// How far from a cut face a dark cell is the cut's to report (spec-0106
/// §3.9's `<k>`): the reach of one torch's useful light, authored.
const DARK_AT_CUT: i32 = 4;

/// The scope, as given.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Scope {
    /// One place or several adjoining ones.
    Places(Vec<NodeId>),
    /// A space of a bound piece's contract.
    Claim { node: NodeId, space: String },
    /// A gate station's region.
    Gate(String),
    /// A cuboid of world coordinates, inclusive.
    Cuboid([i64; 3], [i64; 3]),
}

impl Scope {
    fn kind(&self) -> &'static str {
        match self {
            Scope::Places(_) => "place",
            Scope::Claim { .. } | Scope::Gate(_) => "structure",
            Scope::Cuboid(..) => "cut",
        }
    }
}

fn parse_scope(text: &str) -> Result<Scope, String> {
    let t = text.trim();
    if let Some((a, b)) = t.split_once("..") {
        let p = |s: &str| -> Result<[i64; 3], String> {
            let v: Vec<i64> = s
                .split(',')
                .map(|n| n.trim().parse::<i64>())
                .collect::<Result<_, _>>()
                .map_err(|_| format!("`{s}` is not three integers `x,y,z`"))?;
            match v.as_slice() {
                [x, y, z] => Ok([*x, *y, *z]),
                _ => Err(format!("`{s}` is not three integers `x,y,z`")),
            }
        };
        let (a, b) = (p(a)?, p(b)?);
        let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        return Ok(Scope::Cuboid(lo, hi));
    }
    if t.starts_with("anchor/") {
        return Ok(Scope::Gate(t.to_string()));
    }
    let parts: Vec<&str> = t.split(',').map(str::trim).collect();
    if parts.iter().any(|p| !p.starts_with("node/")) {
        return Err(format!(
            "`{t}` is not a scope. A scope is a place or several (`node/<a>[,node/<b>…]`), a \
             space of a bound place's piece (`node/<place>/<space>`), a gate station \
             (`anchor/<gate>`), or a cuboid (`x0,y0,z0..x1,y1,z1`)"
        ));
    }
    if let [one] = parts.as_slice() {
        let rest = &one["node/".len()..];
        if let Some((place, space)) = rest.split_once('/') {
            return Ok(Scope::Claim {
                node: NodeId(format!("node/{place}")),
                space: space.to_string(),
            });
        }
    }
    let mut nodes: Vec<NodeId> = Vec::new();
    for p in parts {
        let n = NodeId(p.to_string());
        if !nodes.contains(&n) {
            nodes.push(n);
        }
    }
    Ok(Scope::Places(nodes))
}

/// One bound place the cuboid reaches: its frame and its piece.
struct Source<'a> {
    node: NodeId,
    frame: Frame,
    meta: &'a PrefabMeta,
    row: &'a delvewright_dsl::detailplan::Detail,
    /// The place's box, as the plan declares it.
    plan_box: &'a delvewright_dsl::siteplan::PlanBox,
}

/// The arguments of `delvec prefab extract`.
pub(crate) struct ExtractArgs<'a> {
    pub(crate) campaign_dir: &'a Path,
    pub(crate) scope: &'a str,
    pub(crate) out: &'a Path,
    pub(crate) id: Option<&'a str>,
    pub(crate) prefabs_dir: &'a Path,
    pub(crate) json: bool,
}

pub(crate) fn run_extract(args: &ExtractArgs<'_>) -> ExitCode {
    match extract(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

/// Print a refusal; every refusal of this derivation is a build-tier one.
fn refuse(d: Diagnostic, json: bool) -> u8 {
    print_one_diag(&d, json);
    DW_EXTRACTION_CANNOT_PLACE.exit_tier().exit_status()
}

fn cannot_place(object: &str, row: &str, why: &str) -> Diagnostic {
    Diagnostic::error(
        DW_EXTRACTION_CANNOT_PLACE,
        "prefab",
        "/extraction",
        format!(
            "the extraction met `{object}` and its derivation can neither carry it nor drop it \
             by a rule: {why}. It could not fill the piece's `{row}`. The remedy is a change to \
             this derivation (spec-0106 §3.9), not a field to type: nothing was written."
        ),
    )
}

fn extract(args: &ExtractArgs<'_>) -> Result<(), u8> {
    let scope = parse_scope(args.scope).map_err(|e| {
        eprintln!("error: {e}");
        1u8
    })?;
    build_in_memory(args.campaign_dir, args.prefabs_dir, args.json, |built| {
        extract_from(args, &scope, built)
    })
}

fn extract_from(args: &ExtractArgs<'_>, scope: &Scope, built: &Built<'_>) -> Result<(), u8> {
    let scope = scope.clone();
    let campaign = built.campaign;
    let Some(plan_doc) = campaign.site_plan.as_ref().map(|p| &p.content) else {
        eprintln!(
            "error: `{}` carries no `site-plan.json`; a prefab is extracted from the places a \
             site plan holds",
            args.campaign_dir.display()
        );
        return Err(1);
    };
    let rows: BTreeMap<&str, &delvewright_dsl::detailplan::Detail> = campaign
        .detail_plan
        .as_ref()
        .map(|d| {
            d.content
                .details
                .iter()
                .map(|r| (r.place.0.as_str(), r))
                .collect()
        })
        .unwrap_or_default();
    let frames: Vec<Frame> = Frame::all(campaign).into_iter().map(|(f, _)| f).collect();
    let bound = |node: &NodeId| -> Option<Source<'_>> {
        let row = rows.get(node.0.as_str())?;
        let frame = frames.iter().find(|f| &f.node == node)?.clone();
        let meta = built.prefabs.get(row.piece.as_str())?;
        let plan_box = plan_doc.boxes.iter().find(|b| &b.node == node)?;
        Some(Source {
            node: node.clone(),
            frame,
            meta,
            row,
            plan_box,
        })
    };

    // ---- the cuboid, and which places it is cut from ----
    let (lo, hi, scope_places): ([i64; 3], [i64; 3], Vec<NodeId>) = match &scope {
        Scope::Places(nodes) => {
            let mut lo = [i64::MAX; 3];
            let mut hi = [i64::MIN; 3];
            for n in nodes {
                let Some(s) = bound(n) else {
                    eprintln!(
                        "error: `{n}` is not a bound place of this campaign: a place scope reads \
                         the piece a `details[]` row binds there, and the plan's places with a \
                         row are {}",
                        rows.keys().cloned().collect::<Vec<_>>().join(", ")
                    );
                    return Err(1);
                };
                for a in 0..3 {
                    lo[a] = lo[a].min(s.frame.lo[a]);
                    hi[a] = hi[a].max(s.frame.hi[a]);
                }
            }
            (lo, hi, nodes.clone())
        }
        Scope::Claim { node, space } => {
            let Some(s) = bound(node) else {
                eprintln!("error: `{node}` is not a bound place of this campaign");
                return Err(1);
            };
            let Some(sp) = s
                .meta
                .spatial_contract
                .as_ref()
                .and_then(|c| c.spaces.get(space))
            else {
                let known: Vec<String> = s
                    .meta
                    .spatial_contract
                    .as_ref()
                    .map(|c| c.spaces.keys().cloned().collect())
                    .unwrap_or_default();
                eprintln!(
                    "error: `{}` declares no space `{space}` in its contract; it declares: {}",
                    s.row.piece,
                    if known.is_empty() {
                        "none".to_string()
                    } else {
                        known.join(", ")
                    }
                );
                return Err(1);
            };
            let Some((l, h)) = bbox_of(&sp.boxes) else {
                eprintln!("error: `{}`'s space `{space}` holds no cell", s.row.piece);
                return Err(1);
            };
            // The space and its shell: a space is the cells a body stands
            // in, and the structure that holds it is those cells grown by one
            // course on every side — its floor, its walls, its lid — inside
            // the piece's frame.
            let lo = add(s.frame.lo, l.map(i64::from));
            let hi = add(s.frame.lo, h.map(i64::from));
            (
                [
                    (lo[0] - 1).max(s.frame.lo[0]),
                    (lo[1] - 1).max(s.frame.lo[1]),
                    (lo[2] - 1).max(s.frame.lo[2]),
                ],
                [
                    (hi[0] + 1).min(s.frame.hi[0]),
                    (hi[1] + 1).min(s.frame.hi[1]),
                    (hi[2] + 1).min(s.frame.hi[2]),
                ],
                Vec::new(),
            )
        }
        Scope::Gate(name) => {
            let hit = built.plan.anchors.iter().find_map(|((_, n), r)| match r {
                delvec::compiler::plan::ResolvedAnchor::Gate { from, to, .. } if n == name => {
                    Some((*from, *to))
                }
                _ => None,
            });
            let Some((f, t)) = hit else {
                eprintln!(
                    "error: `{name}` resolves to no gate region in this build; a structure scope \
                     by anchor names a gate station"
                );
                return Err(1);
            };
            (
                [
                    i64::from(f[0].min(t[0])),
                    i64::from(f[1].min(t[1])),
                    i64::from(f[2].min(t[2])),
                ],
                [
                    i64::from(f[0].max(t[0])),
                    i64::from(f[1].max(t[1])),
                    i64::from(f[2].max(t[2])),
                ],
                Vec::new(),
            )
        }
        Scope::Cuboid(l, h) => (*l, *h, Vec::new()),
    };
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];

    // Every bound place whose frame the cuboid reaches, in plan order.
    let sources: Vec<Source<'_>> = plan_doc
        .boxes
        .iter()
        .filter_map(|b| bound(&b.node))
        .filter(|s| match &scope {
            Scope::Places(nodes) => nodes.contains(&s.node),
            _ => (0..3).all(|a| s.frame.lo[a] <= hi[a] && s.frame.hi[a] >= lo[a]),
        })
        .collect();
    let in_scope = |c: [i64; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);

    // ---- the world as shipped, inside the cuboid and one ring around it ----
    let grow = 3;
    let clip = (
        [
            (lo[0] - grow) as i32,
            (lo[1] - grow) as i32,
            (lo[2] - grow) as i32,
        ],
        [
            (hi[0] + grow) as i32,
            (hi[1] + grow) as i32,
            (hi[2] + grow) as i32,
        ],
    );
    let shipped = shipped_world(built, clip).map_err(|d| refuse(d, args.json))?;

    // ---- block-entity data the bytes would lose ----
    if let Some(d) = block_entities_inside(built, lo, hi) {
        return Err(refuse(d, args.json));
    }

    // ---- which cells are carried, and which are a neighbour's ----
    let place_scope = matches!(scope, Scope::Places(_));
    let owner_of =
        |c: [i64; 3]| -> Option<&NodeId> { frames.iter().find(|f| f.owns(c)).map(|f| &f.node) };
    let mut voided_by: BTreeMap<String, usize> = BTreeMap::new();
    let region = Box3::new(
        [lo[0] as i32, lo[1] as i32, lo[2] as i32],
        [size[0] as u32, size[1] as u32, size[2] as u32],
    );
    let mut model = VoxelModel::new(region);
    let mut parsed: BTreeMap<&str, GBlock> = BTreeMap::new();
    let void = GBlock::simple("minecraft:structure_void");
    for pos in region.positions() {
        let c = [i64::from(pos[0]), i64::from(pos[1]), i64::from(pos[2])];
        if place_scope
            && let Some(o) = owner_of(c)
            && !scope_places.contains(o)
        {
            *voided_by.entry(o.0.clone()).or_default() += 1;
            model.set(pos, &void).map_err(|_| palette_full())?;
            continue;
        }
        let Some(state) = shipped.get(&pos) else {
            continue; // air: the model's default
        };
        let text = state.as_str();
        let block = match parsed.get(text) {
            Some(b) => b.clone(),
            None => {
                let b: GBlock = text.parse().map_err(|e| {
                    refuse(
                        cannot_place(
                            &format!("block `{text}`"),
                            "structure_set",
                            &format!("the shipped state does not parse: {e}"),
                        ),
                        args.json,
                    )
                })?;
                parsed.insert(text, b.clone());
                b
            }
        };
        model.set(pos, &block).map_err(|_| palette_full())?;
    }

    // ---- the id ----
    let id = match args.id {
        Some(id) => id.to_string(),
        None => derived_id(&scope, campaign.world.campaign_id.as_str(), lo),
    };
    if !delvec::grammar::export::is_valid_id(&id) {
        eprintln!(
            "error: `{id}` is not a prefab id (lowercase letters, digits and `-`); name it with \
             `--id`"
        );
        return Err(1);
    }
    let frozen = delvec::grammar::export::freeze_model(&model, &id, GENERATOR).map_err(|e| {
        eprintln!("error: the extracted bytes do not freeze: {e}");
        1u8
    })?;

    // ---- the walk plane, the ground ----
    let walk_y: i32 = match &scope {
        Scope::Places(_) => sources
            .iter()
            .map(|s| (s.frame.floor - lo[1]) as i32)
            .min()
            .unwrap_or(0),
        _ => {
            use delvec::schem::nav::Voxels as _;
            let oy = model.origin()[1];
            match delvec::schem::nav::standable_cells(&model)
                .iter()
                .map(|c| c[1] - oy)
                .min()
            {
                Some(w) => w,
                None => {
                    return Err(refuse(
                        cannot_place(
                            args.scope,
                            "walk_y",
                            "the cuboid holds no cell a body can stand in, so it has no walk \
                             plane to seat it by",
                        ),
                        args.json,
                    ));
                }
            }
        }
    };
    // A standable cell stands on a course of the model, and a frame holds
    // its floor course: the walk plane is never the bottom course.
    debug_assert!(walk_y >= 1, "a walk plane over at least one course");
    let ground = match (&scope, sources.as_slice()) {
        (Scope::Places(_), [one]) if one.plan_box.base.aloft().is_some() => one.plan_box.base,
        _ => Base::Owned((walk_y - 1) as u32),
    };

    // ---- anchors ----
    let mut residue: Vec<Residue> = Vec::new();
    let (anchors, stations, entry_key) =
        derive_anchors(campaign, &sources, &scope, lo, hi, &mut residue);

    // ---- openings and the contract ----
    let (openings, contract) = match &scope {
        Scope::Places(_) => place_openings(campaign, &sources, &scope_places, lo, &mut residue)
            .map_err(|d| refuse(d, args.json))?,
        _ => {
            let world = cut_world(&shipped);
            cut_openings(&world, &sources, lo, hi)
        }
    };
    let mut anchors = anchors;
    let entry_key = match entry_key {
        Some(k) => Some(k),
        None => {
            // No entry inside the scope: the first candidate opening is it.
            let first = openings.first().map(|o| o.cells[0]);
            match first {
                Some(p) => {
                    residue.push(Residue {
                        object: "entry".to_string(),
                        document: "extraction".to_string(),
                        severed: format!(
                            "entry not inside the scope: the entry is the first candidate \
                             opening, `{}`, and `anchor/cut-entry` stands in it; `DW0837` \
                             proves reach from the seated `spawn`",
                            openings[0].name
                        ),
                    });
                    let key = "anchor/cut-entry".to_string();
                    anchors.insert(
                        key.clone(),
                        Anchor {
                            pos: Some(p),
                            facing: Some(facing_in(openings[0].face).to_string()),
                            role: Some(delvewright_dsl::prefab::AnchorRole::Entry),
                            ..Anchor::default()
                        },
                    );
                    Some(key)
                }
                None => None,
            }
        }
    };
    let Some(mut contract) = contract else {
        return Err(refuse(
            cannot_place(
                args.scope,
                "spatial_contract",
                "no source piece's contract reaches the cuboid",
            ),
            args.json,
        ));
    };
    if let Some(k) = &entry_key
        && let Some(p) = anchors.get(k).and_then(|a| a.pos)
        && let Some(space) = space_at(&contract, p)
    {
        contract.entry = space;
    }
    // Every anchor names the element it lands in, where the contract's
    // spaces were renamed or clipped; one place's piece keeps its own word.
    let respaced = !(place_scope && sources.len() == 1);
    for a in anchors.values_mut().filter(|_| respaced) {
        if let Some(p) = a.pos.or_else(|| a.region.as_ref().map(|r| r.from))
            && a.resolves_to
                .as_deref()
                .is_none_or(|r| r.starts_with("space:"))
        {
            a.resolves_to = space_at(&contract, p).map(|s| format!("space:{s}"));
        }
    }
    if openings.iter().any(|o| o.source == "cut") {
        let n = openings.len();
        residue.push(Residue {
            object: "openings".to_string(),
            document: "extraction".to_string(),
            severed: format!(
                "{n} candidate opening(s) await a neighbour: each is answered at seating by a \
                 seam or by the commons (`DW1022`)"
            ),
        });
    }
    if !voided_by.is_empty() {
        for (n, cells) in &voided_by {
            residue.push(Residue {
                object: n.clone(),
                document: "site-plan.json".to_string(),
                severed: format!(
                    "{cells} cell(s) of the cuboid are `{n}`'s, outside the scope, and are held \
                     as `structure_void`"
                ),
            });
        }
    }

    // ---- light, sky, mitigation: one or dropped ----
    let one = |v: Vec<Option<String>>, what: &str, residue: &mut Vec<Residue>| -> Option<String> {
        let set: BTreeSet<Option<String>> = v.into_iter().collect();
        match set.len() {
            0 => None,
            1 => set.into_iter().next().flatten(),
            _ => {
                residue.push(Residue {
                    object: what.to_string(),
                    document: "site-plan.json".to_string(),
                    severed: format!(
                        "a {what} boundary inside the scope: the places stand under {} \
                         different declarations, and a seated piece has one, so none is carried",
                        set.len()
                    ),
                });
                None
            }
        }
    };
    let source_boxes: Vec<&delvewright_dsl::siteplan::PlanBox> = match &scope {
        Scope::Places(_) => sources.iter().map(|s| s.plan_box).collect(),
        _ => sources.iter().map(|s| s.plan_box).collect(),
    };
    let atmosphere = one(
        source_boxes
            .iter()
            .map(|b| b.atmosphere.as_ref().map(|a| a.0.clone()))
            .collect(),
        "atmosphere",
        &mut residue,
    )
    .map(delvewright_dsl::ids::AtmosphereId);
    let relight_text = one(
        source_boxes
            .iter()
            .map(|b| {
                b.lighting
                    .as_ref()
                    .map(|l| serde_json::to_string(l).expect("lighting"))
            })
            .collect(),
        "lighting",
        &mut residue,
    );
    let relight: Option<delvewright_dsl::AreaLighting> =
        relight_text.map(|t| serde_json::from_str(&t).expect("lighting round-trips"));
    let mitigation_text = one(
        source_boxes
            .iter()
            .map(|b| {
                b.mitigation
                    .as_ref()
                    .map(|m| serde_json::to_string(m).expect("mitigation"))
            })
            .collect(),
        "mitigation",
        &mut residue,
    );
    let mitigation: Option<delvewright_dsl::AreaMitigation> =
        mitigation_text.map(|t| serde_json::from_str(&t).expect("mitigation round-trips"));

    // ---- what a cut leaves unestablished, and what it severs ----
    if !place_scope {
        severed_objects(campaign, built, &sources, lo, hi, &mut residue);
        let dark = dark_at_cut(&model, &contract, &openings, relight.as_ref());
        residue.push(Residue {
            object: "lighting".to_string(),
            document: "extraction".to_string(),
            severed: format!(
                "lighting adequacy not established: {dark} dark cell(s) within {DARK_AT_CUT} of \
                 the cut; `DW0210` judges the seated place's light"
            ),
        });
    }

    // ---- bodies the bytes do not hold ----
    let entities = derive_entities(campaign, built, &anchors, lo, in_scope);

    // ---- the document ----
    let measured = {
        let sky = SkyClaim::of(Some(&contract));
        let probe = delvec::admit::light::probe(&model, DEFAULT_DARK_THRESHOLD, sky);
        if probe.is_unbound() {
            delvewright_dsl::registry::Lighting::unmeasured()
        } else {
            delvec::admit::meta::lighting_from_probe(&probe)
        }
    };
    let waterline_y = {
        let lines: BTreeSet<i64> = sources
            .iter()
            .filter_map(|s| {
                s.meta
                    .waterline_y
                    .map(|w| s.frame.lo[1] + i64::from(w) - lo[1])
            })
            .collect();
        // A waterline is a claim about the bytes: kept only where the cut
        // still holds water at it.
        let wet = |w: i64| {
            let y = region.origin[1] + w as i32;
            (region.origin[0]..region.origin[0] + region.size[0] as i32).any(|x| {
                (region.origin[2]..region.origin[2] + region.size[2] as i32).any(|z| {
                    model
                        .get([x, y, z])
                        .is_some_and(|b| b.to_string().starts_with("minecraft:water"))
                })
            })
        };
        match lines.len() {
            1 => lines
                .into_iter()
                .next()
                .filter(|w| wet(*w))
                .map(|w| w as i32),
            _ => None,
        }
    };
    let shown_faces = shown_faces(&sources, lo, hi);
    if !place_scope {
        let cross: Vec<&str> = ["north", "south", "east", "west", "up"]
            .into_iter()
            .filter(|f| !shown_faces.iter().any(|s| s == f))
            .collect();
        if !cross.is_empty() {
            meta_residue_push(
                &mut residue,
                "shown_faces",
                format!(
                    "the cut's {} side(s) are a cross-section, which `shown_faces` does not claim \
                     finished; `DW0885` judges each at seating, answered by a neighbour standing \
                     against it, the world burying it, or the author declaring it shown",
                    cross.join(", ")
                ),
            );
        }
    }
    let spdx: BTreeSet<String> = sources
        .iter()
        .filter_map(|s| s.meta.license.as_ref().map(|l| l.spdx.clone()))
        .collect();
    let held: Vec<String> = sources
        .iter()
        .map(|s| {
            let l = s.meta.license.as_ref();
            format!(
                "{} ({}, {}; {})",
                s.meta.prefab_id,
                l.map_or("no licence", |l| l.spdx.as_str()),
                l.map_or("no source", |l| l.source.as_str()),
                l.map_or("no provenance", |l| l.provenance.as_str()),
            )
        })
        .collect();
    let manifest = built
        .output
        .get(delvec::outdir::MANIFEST)
        .map(|b| sha(b))
        .unwrap_or_else(|| sha(b""));
    let documents = {
        let mut h = Sha256::new();
        for (path, bytes) in &built.loaded.inputs {
            h.update(path.as_bytes());
            h.update([0]);
            h.update(sha(bytes).as_bytes());
            h.update([0]);
        }
        format!("sha256:{}", hex(&h.finalize()))
    };
    let engine_version = format!(
        "delvec {}, dsl {}, mc {}",
        delvec::compiler::DELVEC_VERSION,
        delvec::compiler::DSL_VERSION,
        delvec::compiler::MC_VERSION
    );
    let mut meta = PrefabMeta {
        prefab_id: format!("prefab/{id}"),
        structure: frozen.structure.clone(),
        structure_set: frozen.structure_set.clone(),
        anchors,
        connectors: Vec::new(),
        lighting: Some(measured),
        license: Some(License {
            source: "derived".to_string(),
            spdx: if spdx.is_empty() {
                "GPL-3.0-or-later".to_string()
            } else {
                spdx.into_iter().collect::<Vec<_>>().join(" AND ")
            },
            note: "Extracted from a built campaign by `delvec prefab extract` (spec-0106 §3.9); \
                   every source piece's own provenance is carried below."
                .to_string(),
            provenance: format!(
                "Extracted from campaign `{}` by scope `{}` ({} scope), the world as shipped \
                 inside {}..{}; holds: {}.",
                campaign.world.campaign_id,
                args.scope,
                scope.kind(),
                fmt3(lo),
                fmt3(hi),
                if held.is_empty() {
                    "no source piece".to_string()
                } else {
                    held.join("; ")
                }
            ),
            generated_by: None,
            extracted: Some(ExtractedFrom {
                campaign: campaign.world.campaign_id.to_string(),
                commit: clean_commit(args.campaign_dir),
                documents,
                scope: args.scope.to_string(),
                engine: engine_version,
                manifest,
                digest: String::new(),
            }),
        }),
        walk_y: Some(walk_y),
        waterline_y,
        shown_faces,
        spatial_contract: Some(contract),
        extraction: Some(Extraction {
            kind: scope.kind().to_string(),
            openings,
            ground,
            relight,
            mitigation,
            atmosphere,
            stations,
            entities,
            residue,
        }),
        extra: BTreeMap::new(),
    };
    let digest = {
        let mut h = Sha256::new();
        for (file, bytes) in &frozen.files {
            h.update(file.as_bytes());
            h.update([0]);
            h.update(bytes);
        }
        h.update(meta_json(&meta).as_bytes());
        format!("sha256:{}", hex(&h.finalize()))
    };
    if let Some(l) = meta.license.as_mut()
        && let Some(x) = l.extracted.as_mut()
    {
        x.digest = digest.clone();
    }

    // ---- write ----
    std::fs::create_dir_all(args.out).map_err(|e| {
        eprintln!("internal error: cannot create {}: {e}", args.out.display());
        EXIT_INTERNAL
    })?;
    for (file, bytes) in &frozen.files {
        write(&args.out.join(file), bytes)?;
    }
    write(
        &args.out.join(format!("{id}.json")),
        meta_json(&meta).as_bytes(),
    )?;
    let x = meta.extraction.as_ref().expect("written above");
    let gates = meta.anchors.values().filter(|a| a.region.is_some()).count();
    eprintln!(
        "extracted `prefab/{id}` from `{}` by {} scope `{}`: {}x{}x{} at {}, {} tile(s); {} \
         anchor(s) ({} gate), {} station(s); {} opening(s) ({} from seams, {} candidate(s) at \
         a cut); ground {}; {} entit(y/ies); {} residue row(s){}; digest {digest}",
        campaign.world.campaign_id,
        scope.kind(),
        args.scope,
        size[0],
        size[1],
        size[2],
        fmt3(lo),
        frozen.files.len(),
        meta.anchors.len(),
        gates,
        x.stations.len(),
        x.openings.len(),
        x.openings.iter().filter(|o| o.source == "seam").count(),
        x.openings.iter().filter(|o| o.source == "cut").count(),
        serde_json::to_string(&x.ground).expect("base"),
        x.entities.len(),
        x.residue.len(),
        if x.residue.is_empty() {
            " (the scope's boundary is all seams and commons)".to_string()
        } else {
            String::new()
        },
    );
    for r in &x.residue {
        eprintln!("  residue: {} ({}): {}", r.object, r.document, r.severed);
    }
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "prefab": format!("prefab/{id}"),
                "size": size,
                "lo": lo,
                "tiles": frozen.files.len(),
                "anchors": meta.anchors.len(),
                "gate_anchors": gates,
                "stations": x.stations.len(),
                "openings": x.openings.len(),
                "residue": x.residue.len(),
                "digest": digest,
            })
        );
    }
    Ok(())
}

fn meta_residue_push(residue: &mut Vec<Residue>, object: &str, severed: String) {
    residue.push(Residue {
        object: object.to_string(),
        document: "extraction".to_string(),
        severed,
    });
}

fn palette_full() -> u8 {
    eprintln!(
        "error: the cuboid holds more distinct block states than one structure palette can; \
         extract a smaller scope"
    );
    1
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), u8> {
    std::fs::write(path, bytes).map_err(|e| {
        eprintln!("internal error: cannot write {}: {e}", path.display());
        EXIT_INTERNAL
    })
}

/// The metadata as the library writes every piece: pretty, newline-ended.
fn meta_json(meta: &PrefabMeta) -> String {
    serde_json::to_string_pretty(meta).expect("prefab metadata serialises") + "\n"
}

fn sha(bytes: &[u8]) -> String {
    format!("sha256:{}", hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fmt3(v: [i64; 3]) -> String {
    format!("[{}, {}, {}]", v[0], v[1], v[2])
}

fn add(a: [i64; 3], b: [i64; 3]) -> [i64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn bbox_of(boxes: &[Region]) -> Option<([i32; 3], [i32; 3])> {
    let mut it = boxes.iter();
    let first = it.next()?;
    let mut lo = [0; 3];
    let mut hi = [0; 3];
    for a in 0..3 {
        lo[a] = first.from[a].min(first.to[a]);
        hi[a] = first.from[a].max(first.to[a]);
    }
    for r in it {
        for a in 0..3 {
            lo[a] = lo[a].min(r.from[a].min(r.to[a]));
            hi[a] = hi[a].max(r.from[a].max(r.to[a]));
        }
    }
    Some((lo, hi))
}

/// The id a scope names when `--id` does not: the place's stem, the first and
/// last places' stems, the place and its space, the gate, or the campaign and
/// the cut's low corner.
fn derived_id(scope: &Scope, campaign: &str, lo: [i64; 3]) -> String {
    let stem = |n: &NodeId| n.0.trim_start_matches("node/").to_string();
    match scope {
        Scope::Places(nodes) => match nodes.as_slice() {
            [one] => stem(one),
            [first, .., last] => format!("{}-{}", stem(first), stem(last)),
            [] => campaign.to_string(),
        },
        Scope::Claim { node, space } => format!("{}-{space}", stem(node)),
        Scope::Gate(a) => format!("{campaign}-{}", a.trim_start_matches("anchor/")),
        Scope::Cuboid(..) => format!(
            "{campaign}-cut-{}-{}-{}",
            lo[0].to_string().replace('-', "m"),
            lo[1].to_string().replace('-', "m"),
            lo[2].to_string().replace('-', "m")
        ),
    }
}

/// The commit a campaign directory is checked out at, when it lies in a git
/// work tree and nothing under it differs from that commit.
fn clean_commit(dir: &Path) -> Option<String> {
    let run = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let head = run(&["rev-parse", "HEAD"])?;
    // Tracked there at all — an ignored directory inside some other work tree
    // is not checked out at that tree's commit.
    let tracked = run(&["ls-files", "--", "."])?;
    let dirty = run(&[
        "status",
        "--porcelain",
        "--ignored",
        "--untracked-files=all",
        "--",
        ".",
    ])?;
    (!tracked.is_empty() && dirty.is_empty()).then_some(head)
}

/// **The world as the piece's own bytes stand in it**, inside `clip`: the
/// assembled pieces, the stage-7 edits over them and the world-load gate
/// seals — what `delvec build` emits from, taken the way
/// `emit::build_with_warnings` takes it — **less the two derivations the
/// build makes from documents the piece does not hold**: the relight's
/// fixtures (of the places' `boxes[].lighting`, which the piece carries as
/// `extraction.relight` and a seating declares again) and the trigger props
/// (of the campaign's triggers, which a seating campaign declares for
/// itself). Each is derived again by the seated build, over the world it
/// seats the piece in. Baked, they are wrong twice over, measured on
/// Vesperhold: a fixture the source hung after every template stood hangs
/// from a block of a later tile, and the pinned server drops it where the
/// earlier tile lands (two wall torches at the x 47/48 tile seam); and a prop
/// in the bytes moves a fixture the relight derives over them.
fn shipped_world(
    built: &Built<'_>,
    clip: ([i32; 3], [i32; 3]),
) -> Result<delvec::compiler::blockstate::BlockMap, Diagnostic> {
    let plan = &built.plan;
    let mut pristine = Some(assembled::assemble(plan, &built.structures));
    let replay = delvec::compiler::edit::replay_taking(
        plan,
        built.prefabs,
        &built.structures,
        &mut pristine,
    )
    .map_err(|e| Diagnostic::error(e.code, "world-edits", "/content", e.message))?;
    let world: &assembled::Assembled = match &replay {
        Some(r) => &r.assembled,
        None => pristine
            .as_ref()
            .expect("an assembly the replay did not take"),
    };
    let (lo, hi) = clip;
    let inside = |c: [i32; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    let mut map: delvec::compiler::blockstate::BlockMap = world
        .blocks
        .range(lo..=hi)
        .filter(|(c, _)| inside(**c))
        .map(|(c, b)| (*c, *b))
        .collect();
    // Every gate the placed world authors shut, written back as setup writes it.
    for g in world.gate_seals.iter().filter(|g| g.sealed()) {
        let hit = plan.anchors.iter().find_map(|((_, n), r)| match r {
            delvec::compiler::plan::ResolvedAnchor::Gate { from, to, block } if *n == g.anchor => {
                Some((*from, *to, block.clone()))
            }
            _ => None,
        });
        let Some((from, to, block)) = hit else {
            continue;
        };
        let state = delvec::compiler::blockstate::BlockState::new(&block);
        for c in assembled::region_cells(from, to) {
            if !inside(c) {
                continue;
            }
            if delvewright_dsl::blockshape::is_air(&block) {
                map.remove(&c);
            } else {
                map.insert(c, state);
            }
        }
    }
    // **A sealed gate is written back as its anchor's bare block**, and the
    // game shapes it to its neighbours as it lands; the template the piece
    // ships already holds that shaped state, so where the template's block is
    // the seal's, the template's state is what stands there.
    let base = |s: &str| s.split('[').next().unwrap_or(s).to_string();
    let sealed: Vec<([i32; 3], [i32; 3], String)> = world
        .gate_seals
        .iter()
        .filter(|g| g.sealed())
        .filter_map(|g| {
            plan.anchors.iter().find_map(|((_, n), r)| match r {
                delvec::compiler::plan::ResolvedAnchor::Gate { from, to, block }
                    if *n == g.anchor =>
                {
                    let lo = [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])];
                    let hi = [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])];
                    Some((lo, hi, block.clone()))
                }
                _ => None,
            })
        })
        .collect();
    let within =
        |c: [i32; 3], lo: [i32; 3], hi: [i32; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let thi = [
                    t.pos[0] + t.size[0].max(t.size[2]),
                    t.pos[1] + t.size[1],
                    t.pos[2] + t.size[0].max(t.size[2]),
                ];
                let tlo = [
                    t.pos[0] - t.size[0].max(t.size[2]),
                    t.pos[1],
                    t.pos[2] - t.size[0].max(t.size[2]),
                ];
                let hits: Vec<&([i32; 3], [i32; 3], String)> = sealed
                    .iter()
                    .filter(|(l, h, _)| within(*l, clip.0, clip.1) || within(*h, clip.0, clip.1))
                    .filter(|(l, h, _)| (0..3).all(|a| l[a] <= thi[a] && h[a] >= tlo[a]))
                    .collect();
                if hits.is_empty() {
                    continue;
                }
                let Some(bytes) = built.structures.get(&t.structure_file) else {
                    continue;
                };
                for (local, name, _) in assembled::structure_cells_stateful(bytes) {
                    let w = piece.rotation.transform(local);
                    let c = [t.pos[0] + w[0], t.pos[1] + w[1], t.pos[2] + w[2]];
                    for (l, h, block) in &hits {
                        if within(c, *l, *h)
                            && base(&name) == base(block)
                            && map.get(&c).is_some_and(|b| b.as_str() == block.as_str())
                        {
                            let state = assembled::rotate_state(&name, piece.rotation);
                            map.insert(c, delvec::compiler::blockstate::BlockState::new(&state));
                        }
                    }
                }
            }
        }
    }
    Ok(map)
}

/// `DW1023` when a template cell inside the cuboid carries block-entity data:
/// the shipped model holds block states, and a chest's contents or a sign's
/// words would be lost without a word.
fn block_entities_inside(built: &Built<'_>, lo: [i64; 3], hi: [i64; 3]) -> Option<Diagnostic> {
    for area in &built.plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let tlo = t.pos;
                let thi = [
                    t.pos[0] + t.size[0] - 1,
                    t.pos[1] + t.size[1] - 1,
                    t.pos[2] + t.size[2] - 1,
                ];
                if (0..3).any(|a| i64::from(thi[a]) < lo[a] || i64::from(tlo[a]) > hi[a]) {
                    continue;
                }
                let Some(bytes) = built.structures.get(&t.structure_file) else {
                    continue;
                };
                let Ok(s) = delvec::admit::structure::Structure::read(bytes) else {
                    continue;
                };
                let n = s
                    .blocks
                    .iter()
                    .filter(|b| b.nbt.is_some())
                    .filter(|b| {
                        let w = piece.rotation.transform(b.pos);
                        let c = [
                            i64::from(t.pos[0] + w[0]),
                            i64::from(t.pos[1] + w[1]),
                            i64::from(t.pos[2] + w[2]),
                        ];
                        (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a])
                    })
                    .count();
                if n > 0 {
                    return Some(cannot_place(
                        &piece.prefab_id,
                        "structure_set",
                        &format!(
                            "{n} cell(s) of its template `{}` inside the cuboid carry block-entity \
                             data (a container's contents, a sign's words), and the shipped model \
                             the derivation reads holds block states only",
                            t.structure_file
                        ),
                    ));
                }
            }
        }
    }
    None
}

/// The facing that looks INTO a piece through an opening on `face`.
fn facing_in(face: Face) -> &'static str {
    match face {
        Face::East => "west",
        Face::West => "east",
        Face::South => "north",
        Face::North => "south",
        Face::Up | Face::Down => "north",
    }
}

/// **The anchors of the cuboid**: every source piece anchor that lies inside
/// it, rebased, keyed by the station a `details[]` row binds to it where one
/// does and by its own key otherwise (prefixed with its place's stem where two
/// pieces share a key); the stations of the source places that land inside,
/// in plan then station order; and the key of the one anchor that keeps
/// `role: entry` — the entry place's (the campaign's entry when it is in
/// scope, else the first source place in plan order that has one).
fn derive_anchors(
    campaign: &Campaign,
    sources: &[Source<'_>],
    scope: &Scope,
    lo: [i64; 3],
    hi: [i64; 3],
    residue: &mut Vec<Residue>,
) -> (BTreeMap<String, Anchor>, Vec<Station>, Option<String>) {
    let inside = |c: [i64; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    let graph = campaign.layout_graph.as_ref().map(|g| &g.content);
    let entry_node = graph.map(|g| g.entry.clone());
    let mut out: BTreeMap<String, Anchor> = BTreeMap::new();
    let mut stations: Vec<Station> = Vec::new();
    let mut entry_key: Option<String> = None;
    // Keys more than one source piece declares.
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for s in sources {
        for k in s.meta.anchors.keys() {
            *seen.entry(k.as_str()).or_default() += 1;
        }
    }
    // The entry place: the campaign's entry when in scope, else the first.
    let entry_place: Option<&NodeId> = sources
        .iter()
        .map(|s| &s.node)
        .find(|n| Some(*n) == entry_node.as_ref())
        .or_else(|| {
            sources
                .iter()
                .find(|s| {
                    s.meta
                        .anchors
                        .values()
                        .any(|a| a.role == Some(delvewright_dsl::prefab::AnchorRole::Entry))
                })
                .map(|s| &s.node)
        });
    for s in sources {
        let stem = s.node.0.trim_start_matches("node/");
        // piece key -> the station names a row binds to it
        let mut bound_to: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (owed, key) in &s.row.anchors {
            bound_to
                .entry(key.as_str())
                .or_default()
                .push(owed.as_str());
        }
        let node_stations: Vec<&Station> = graph
            .and_then(|g| g.nodes.iter().find(|n| n.id == s.node))
            .map(|n| n.stations.iter().collect())
            .unwrap_or_default();
        for (key, a) in &s.meta.anchors {
            let at = a.pos.or_else(|| a.region.as_ref().map(|r| r.from));
            let Some(p) = at else { continue };
            let w = add(s.frame.lo, p.map(i64::from));
            let whole_inside = match a.region.as_ref() {
                Some(r) => {
                    let f = add(s.frame.lo, r.from.map(i64::from));
                    let t = add(s.frame.lo, r.to.map(i64::from));
                    let fi = inside(f);
                    let ti = inside(t);
                    if fi != ti {
                        residue.push(Residue {
                            object: key.clone(),
                            document: format!(
                                "prefab/{}",
                                s.meta.prefab_id.trim_start_matches("prefab/")
                            ),
                            severed: "a gate whose region the cut crosses: its other side lies \
                                      outside the scope, so it is dropped"
                                .to_string(),
                        });
                        continue;
                    }
                    fi
                }
                None => inside(w),
            };
            if !whole_inside {
                continue;
            }
            // The station a row binds here, by the station's own name.
            let station = bound_to
                .get(key.as_str())
                .and_then(|names| {
                    names
                        .iter()
                        .find(|n| node_stations.iter().any(|st| st.anchor.as_str() == **n))
                })
                .copied();
            let name = match station {
                Some(st) => st.to_string(),
                None if seen.get(key.as_str()).copied().unwrap_or(0) > 1 => {
                    format!("anchor/{stem}-{}", key.trim_start_matches("anchor/"))
                }
                None => key.clone(),
            };
            let mut a = a.clone();
            let off = [
                (s.frame.lo[0] - lo[0]) as i32,
                (s.frame.lo[1] - lo[1]) as i32,
                (s.frame.lo[2] - lo[2]) as i32,
            ];
            if let Some(p) = a.pos.as_mut() {
                *p = [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
            }
            if let Some(r) = a.region.as_mut() {
                r.from = [r.from[0] + off[0], r.from[1] + off[1], r.from[2] + off[2]];
                r.to = [r.to[0] + off[0], r.to[1] + off[1], r.to[2] + off[2]];
            }
            if a.role == Some(delvewright_dsl::prefab::AnchorRole::Entry) {
                if Some(&s.node) == entry_place && entry_key.is_none() {
                    entry_key = Some(name.clone());
                } else {
                    a.role = None;
                }
            }
            if let Some(st) = station
                && let Some(decl) = node_stations.iter().find(|x| x.anchor.as_str() == st)
            {
                stations.push((*decl).clone());
            }
            out.insert(name, a);
        }
        // A station of a place the cut parts from the rest of it.
        if !matches!(scope, Scope::Places(_)) {
            for st in &node_stations {
                let Some(key) = s.row.anchors.get(st.anchor.as_str()) else {
                    continue;
                };
                let Some(a) = s.meta.anchors.get(key) else {
                    continue;
                };
                let Some(p) = a.pos.or_else(|| a.region.as_ref().map(|r| r.from)) else {
                    continue;
                };
                if !inside(add(s.frame.lo, p.map(i64::from))) {
                    residue.push(Residue {
                        object: st.anchor.as_str().to_string(),
                        document: "layout-graph.json".to_string(),
                        severed: format!(
                            "a station of `{}` the cut leaves outside the scope; the place's \
                             quests name it, and the seated campaign declares its own",
                            s.node
                        ),
                    });
                }
            }
        }
    }
    (out, stations, entry_key)
}

/// **The openings at a place scope's boundary** (spec-0106 §3.9): every seam
/// the plan allocates between a place of the scope and one outside it, with
/// the cells the allocation hands the scope's piece, its class and its derived
/// rise; and the contract of the pieces joined — their spaces and edges, the
/// internal seams as edges between the spaces either side, and an `exterior`
/// edge and a face for every opening.
#[allow(clippy::type_complexity)]
fn place_openings(
    campaign: &Campaign,
    sources: &[Source<'_>],
    scope: &[NodeId],
    lo: [i64; 3],
    residue: &mut Vec<Residue>,
) -> Result<(Vec<PieceOpening>, Option<SpatialContract>), Diagnostic> {
    let single = sources.len() == 1;
    let mut openings: Vec<PieceOpening> = Vec::new();
    let mut spaces: BTreeMap<String, ContractSpace> = BTreeMap::new();
    let mut no_body = BTreeMap::new();
    let mut edges: Vec<ContractEdge> = Vec::new();
    let mut faces: Vec<ContractFace> = Vec::new();
    let mut entry: Option<String> = None;
    let mut internal: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for s in sources {
        let Some(c) = s.meta.spatial_contract.as_ref() else {
            return Err(cannot_place(
                &s.meta.prefab_id,
                "spatial_contract",
                "the piece declares no spatial contract to join",
            ));
        };
        let stem = s.node.0.trim_start_matches("node/");
        let name = |n: &str| -> String {
            if single || n == "exterior" {
                n.to_string()
            } else {
                format!("{stem}-{n}")
            }
        };
        let off = [
            (s.frame.lo[0] - lo[0]) as i32,
            (s.frame.lo[1] - lo[1]) as i32,
            (s.frame.lo[2] - lo[2]) as i32,
        ];
        let rebase = |r: &Region| Region {
            from: [r.from[0] + off[0], r.from[1] + off[1], r.from[2] + off[2]],
            to: [r.to[0] + off[0], r.to[1] + off[1], r.to[2] + off[2]],
        };
        for (k, sp) in &c.spaces {
            spaces.insert(
                name(k),
                ContractSpace {
                    envelope: sp.envelope.clone(),
                    boxes: sp.boxes.iter().map(rebase).collect(),
                },
            );
        }
        for (k, nb) in &c.no_body {
            let mut nb = nb.clone();
            nb.boxes = nb.boxes.iter().map(rebase).collect();
            no_body.insert(name(k), nb);
        }
        if entry.is_none() {
            entry = Some(name(&c.entry));
        }
        let alloc = engine::allocation(campaign, &s.node);
        let seams = alloc.map(|a| a.seams).unwrap_or_default();
        // Which faces of this piece answer a seam leaving the scope.
        let mut boundary_faces: Vec<ContractFace> = Vec::new();
        for f in &c.faces {
            let fr = rebase(&f.opening);
            let answers = seams.iter().find(|sm| {
                sm.face == f.dir
                    && sm.cells[0].map(|v| v as i32)
                        == [
                            f.opening.from[0].min(f.opening.to[0]),
                            f.opening.from[1].min(f.opening.to[1]),
                            f.opening.from[2].min(f.opening.to[2]),
                        ]
                    && sm.cells[1].map(|v| v as i32)
                        == [
                            f.opening.from[0].max(f.opening.to[0]),
                            f.opening.from[1].max(f.opening.to[1]),
                            f.opening.from[2].max(f.opening.to[2]),
                        ]
            });
            match answers {
                Some(sm) if scope.iter().any(|n| n.0 == sm.other) => {
                    internal
                        .entry(sm.edge.clone())
                        .or_default()
                        .push((name(&f.space), sm.class.clone()));
                }
                _ => boundary_faces.push(ContractFace {
                    space: name(&f.space),
                    class: f.class.clone(),
                    dir: f.dir.clone(),
                    opening: fr,
                }),
            }
        }
        for e in &c.edges {
            let mut e = e.clone();
            if e.b == "exterior" || e.a == "exterior" {
                // Kept only where a boundary face still leaves this space.
                let sp = if e.a == "exterior" { &e.b } else { &e.a };
                if !boundary_faces.iter().any(|f| f.space == name(sp)) {
                    continue;
                }
            }
            e.a = name(&e.a);
            e.b = name(&e.b);
            for r in e
                .via
                .iter_mut()
                .flat_map(|v| v.boxes.iter_mut())
                .chain(e.bar.iter_mut().flat_map(|b| b.boxes.iter_mut()))
                .chain(e.way.iter_mut().flat_map(|w| w.boxes.iter_mut()))
            {
                *r = rebase(r);
            }
            edges.push(e);
        }
        faces.extend(boundary_faces);
        for sm in &seams {
            if scope.iter().any(|n| n.0 == sm.other) {
                continue;
            }
            let face: Face = serde_json::from_value(serde_json::Value::String(sm.face.clone()))
                .map_err(|_| {
                    cannot_place(
                        &sm.edge,
                        "openings",
                        "the seam leaves by no face this engine names",
                    )
                })?;
            let l = [
                sm.cells[0][0] as i32 + off[0],
                sm.cells[0][1] as i32 + off[1],
                sm.cells[0][2] as i32 + off[2],
            ];
            let h = [
                sm.cells[1][0] as i32 + off[0],
                sm.cells[1][1] as i32 + off[1],
                sm.cells[1][2] as i32 + off[2],
            ];
            let (w, ht) = in_plane(face, l, h);
            if sm.class == "barred" {
                residue.push(Residue {
                    object: delvewright_dsl::siteplan::seam_anchor(&delvewright_dsl::EdgeId(
                        sm.edge.clone(),
                    )),
                    document: "site-plan.json".to_string(),
                    severed: format!(
                        "a gate whose other side lies outside the scope (`{}` to `{}`): the \
                         opening is declared and the gate is the seated campaign's to declare",
                        s.node, sm.other
                    ),
                });
            }
            openings.push(PieceOpening {
                name: sm.edge.trim_start_matches("edge/").to_string(),
                face,
                cells: [l, h],
                kind: sm.class.clone(),
                rise: sm.rise,
                width: w,
                height: ht,
                form: sm.form.clone(),
                source: "seam".to_string(),
            });
        }
    }
    // Each internal seam is one edge between the spaces either side of it.
    for (edge, sides) in &internal {
        if let [(a, class), (b, _)] = sides.as_slice() {
            edges.push(ContractEdge {
                a: a.clone(),
                b: b.clone(),
                class: class.clone(),
                rise: None,
                via: None,
                bar: None,
                way: None,
            });
        } else {
            residue.push(Residue {
                object: edge.clone(),
                document: "site-plan.json".to_string(),
                severed: format!(
                    "a seam inside the scope that {} piece face(s) answer, where one each side \
                     joins two spaces; no contract edge is derived for it",
                    sides.len()
                ),
            });
        }
    }
    openings.sort_by(|a, b| {
        (a.face, a.cells, a.name.as_str()).cmp(&(b.face, b.cells, b.name.as_str()))
    });
    let Some(entry) = entry else {
        return Ok((openings, None));
    };
    // `exterior` edges for spaces that gained a boundary face and had none.
    for f in &faces {
        if !edges
            .iter()
            .any(|e| (e.a == f.space && e.b == "exterior") || (e.b == f.space && e.a == "exterior"))
        {
            edges.push(ContractEdge {
                a: f.space.clone(),
                b: "exterior".to_string(),
                class: f.class.clone(),
                rise: None,
                via: None,
                bar: None,
                way: None,
            });
        }
    }
    Ok((
        openings,
        Some(SpatialContract {
            entry,
            spaces,
            no_body,
            edges,
            faces,
            no_body_majority_ack: None,
        }),
    ))
}

/// The opening's clear size on its face's own two in-plane axes.
fn in_plane(face: Face, l: [i32; 3], h: [i32; 3]) -> (u32, u32) {
    let d = |a: usize| (h[a] - l[a]).unsigned_abs() + 1;
    match face {
        Face::East | Face::West => (d(2), d(1)),
        Face::North | Face::South => (d(0), d(1)),
        Face::Up | Face::Down => (d(0), d(2)),
    }
}

/// The walk model over the shipped bytes around a cut.
fn cut_world(shipped: &delvec::compiler::blockstate::BlockMap) -> delvec::compiler::nav::World {
    delvec::compiler::nav::World::from_occupancy(
        assembled::occupancy_over(shipped, &BTreeSet::new()),
        delvec::compiler::nav::Premises::geometry_only(),
    )
}

/// **The candidate openings at a cut** (spec-0106 §3.9): every run of
/// walkable floor or water the walk model finds meeting a face of the cuboid —
/// a body standing in the cuboid's boundary layer with a place to stand, step,
/// drop or swim to just outside it. Each run is one candidate: its cells, its
/// kind from the walk model, its rise. A solid face is a wall. The contract is
/// the source pieces' spaces clipped to the cuboid, with an `exterior` edge and
/// a face per candidate.
fn cut_openings(
    world: &delvec::compiler::nav::World,
    sources: &[Source<'_>],
    lo: [i64; 3],
    hi: [i64; 3],
) -> (Vec<PieceOpening>, Option<SpatialContract>) {
    let lo32 = [lo[0] as i32, lo[1] as i32, lo[2] as i32];
    let hi32 = [hi[0] as i32, hi[1] as i32, hi[2] as i32];
    let mut found: Vec<PieceOpening> = Vec::new();
    for face in [Face::East, Face::West, Face::South, Face::North] {
        let v = face.vector();
        let n = [v[0] as i32, v[1] as i32, v[2] as i32];
        let axis = if n[0] != 0 { 0 } else { 2 };
        let along = 2 - axis;
        let layer = if n[axis] > 0 { hi32[axis] } else { lo32[axis] };
        // feet cell -> (kind, rise)
        let mut cells: BTreeMap<(String, i64), BTreeSet<[i32; 2]>> = BTreeMap::new();
        for y in lo32[1]..=hi32[1] {
            for u in lo32[along]..=hi32[along] {
                let mut c = [0; 3];
                c[axis] = layer;
                c[along] = u;
                c[1] = y;
                let o = [c[0] + n[0], c[1], c[2] + n[2]];
                let class = if world.is_standable(c) {
                    if world.is_standable(o) {
                        Some(("walk".to_string(), 0))
                    } else if world.is_standable([o[0], o[1] + 1, o[2]]) {
                        Some(("stair".to_string(), 1))
                    } else if world.is_standable([o[0], o[1] - 1, o[2]]) {
                        Some(("stair".to_string(), -1))
                    } else {
                        (2..=4)
                            .find(|d| {
                                world.is_standable([o[0], o[1] - d, o[2]])
                                    && (0..*d).all(|k| world.is_clear([o[0], o[1] - k, o[2]]))
                            })
                            .map(|d| ("drop".to_string(), -i64::from(d)))
                    }
                } else if world.is_flooded(c) && world.is_flooded(o) {
                    Some(("swim".to_string(), 0))
                } else {
                    None
                };
                if let Some(k) = class {
                    cells.entry(k).or_default().insert([u, y]);
                }
            }
        }
        for ((kind, rise), set) in cells {
            for run in runs(&set) {
                let (umin, umax) = (
                    run.iter().map(|p| p[0]).min().expect("a run"),
                    run.iter().map(|p| p[0]).max().expect("a run"),
                );
                let (ymin, ymax) = (
                    run.iter().map(|p| p[1]).min().expect("a run"),
                    run.iter().map(|p| p[1]).max().expect("a run"),
                );
                let mut l = [0; 3];
                let mut h = [0; 3];
                l[axis] = layer - lo32[axis];
                h[axis] = layer - lo32[axis];
                l[along] = umin - lo32[along];
                h[along] = umax - lo32[along];
                l[1] = ymin - lo32[1];
                h[1] = (ymax + 1).min(hi32[1]) - lo32[1];
                let (w, ht) = in_plane(face, l, h);
                found.push(PieceOpening {
                    name: String::new(),
                    face,
                    cells: [l, h],
                    kind: kind.clone(),
                    rise,
                    width: w,
                    height: ht,
                    form: format!(
                        "a cut through the {} side: a body {} here",
                        face.as_str(),
                        match kind.as_str() {
                            "walk" => "walks on",
                            "stair" => "steps across",
                            "drop" => "drops out",
                            _ => "swims on",
                        }
                    ),
                    source: "cut".to_string(),
                });
            }
        }
    }
    found.sort_by_key(|a| (a.face, a.cells));
    let mut count: BTreeMap<Face, usize> = BTreeMap::new();
    for o in &mut found {
        let i = count.entry(o.face).or_default();
        *i += 1;
        o.name = format!("cut-{}-{}", o.face.as_str(), i);
    }
    // The contract: the source spaces clipped to the cuboid.
    let mut spaces: BTreeMap<String, ContractSpace> = BTreeMap::new();
    let mut edges: Vec<ContractEdge> = Vec::new();
    let mut entry: Option<String> = None;
    let single = sources.len() == 1;
    for s in sources {
        let Some(c) = s.meta.spatial_contract.as_ref() else {
            continue;
        };
        let stem = s.node.0.trim_start_matches("node/");
        let name = |n: &str| {
            if single || n == "exterior" {
                n.to_string()
            } else {
                format!("{stem}-{n}")
            }
        };
        for (k, sp) in &c.spaces {
            let boxes: Vec<Region> = sp
                .boxes
                .iter()
                .filter_map(|r| clip_region(r, s.frame.lo, lo, hi))
                .collect();
            if boxes.is_empty() {
                continue;
            }
            if entry.is_none() || name(&c.entry) == name(k) {
                entry = entry.or(Some(name(k)));
            }
            spaces.insert(
                name(k),
                ContractSpace {
                    envelope: sp.envelope.clone(),
                    boxes,
                },
            );
        }
        for e in &c.edges {
            if e.a == "exterior" || e.b == "exterior" {
                continue;
            }
            if spaces.contains_key(&name(&e.a)) && spaces.contains_key(&name(&e.b)) {
                let mut e = e.clone();
                e.a = name(&e.a);
                e.b = name(&e.b);
                e.via = None;
                e.bar = None;
                e.way = None;
                edges.push(e);
            }
        }
    }
    if spaces.is_empty() {
        spaces.insert(
            "cut".to_string(),
            ContractSpace {
                envelope: "open".to_string(),
                boxes: vec![Region {
                    from: [0, 0, 0],
                    to: [
                        (hi[0] - lo[0]) as i32,
                        (hi[1] - lo[1]) as i32,
                        (hi[2] - lo[2]) as i32,
                    ],
                }],
            },
        );
        entry = Some("cut".to_string());
    }
    let mut contract = SpatialContract {
        entry: entry.unwrap_or_else(|| "cut".to_string()),
        spaces,
        no_body: BTreeMap::new(),
        edges,
        faces: Vec::new(),
        no_body_majority_ack: None,
    };
    for o in &found {
        let class = match o.kind.as_str() {
            "swim" => "walk",
            k => k,
        }
        .to_string();
        let space = space_at(&contract, o.cells[0]).unwrap_or_else(|| contract.entry.clone());
        contract.faces.push(ContractFace {
            space: space.clone(),
            class: class.clone(),
            dir: o.face.as_str().to_string(),
            opening: Region {
                from: o.cells[0],
                to: o.cells[1],
            },
        });
        if !contract
            .edges
            .iter()
            .any(|e| e.a == space && e.b == "exterior")
        {
            contract.edges.push(ContractEdge {
                a: space,
                b: "exterior".to_string(),
                class,
                rise: None,
                via: None,
                bar: None,
                way: None,
            });
        }
    }
    (found, Some(contract))
}

/// A source region (piece-local at `frame_lo`) clipped to the cuboid and
/// rebased to it.
fn clip_region(r: &Region, frame_lo: [i64; 3], lo: [i64; 3], hi: [i64; 3]) -> Option<Region> {
    let mut f = [0; 3];
    let mut t = [0; 3];
    for a in 0..3 {
        let rl = frame_lo[a] + i64::from(r.from[a].min(r.to[a]));
        let rh = frame_lo[a] + i64::from(r.from[a].max(r.to[a]));
        let cl = rl.max(lo[a]);
        let ch = rh.min(hi[a]);
        if cl > ch {
            return None;
        }
        f[a] = (cl - lo[a]) as i32;
        t[a] = (ch - lo[a]) as i32;
    }
    Some(Region { from: f, to: t })
}

/// The 4-connected runs of a set of in-plane cells, each in cell order.
fn runs(set: &BTreeSet<[i32; 2]>) -> Vec<Vec<[i32; 2]>> {
    let mut left = set.clone();
    let mut out = Vec::new();
    while let Some(start) = left.iter().next().copied() {
        left.remove(&start);
        let mut run = vec![start];
        let mut i = 0;
        while i < run.len() {
            let p = run[i];
            for q in [
                [p[0] + 1, p[1]],
                [p[0] - 1, p[1]],
                [p[0], p[1] + 1],
                [p[0], p[1] - 1],
            ] {
                if left.remove(&q) {
                    run.push(q);
                }
            }
            i += 1;
        }
        run.sort();
        out.push(run);
    }
    out
}

/// The contract space whose boxes hold a piece-local cell.
fn space_at(c: &SpatialContract, p: [i32; 3]) -> Option<String> {
    c.spaces
        .iter()
        .find(|(_, s)| {
            s.boxes.iter().any(|r| {
                (0..3).all(|a| r.from[a].min(r.to[a]) <= p[a] && p[a] <= r.from[a].max(r.to[a]))
            })
        })
        .map(|(k, _)| k.clone())
}

/// The sides of the cuboid a source piece declares finished, where that
/// piece's own side lies on the cuboid's.
fn shown_faces(sources: &[Source<'_>], lo: [i64; 3], hi: [i64; 3]) -> Vec<String> {
    let mut out: BTreeSet<&str> = BTreeSet::new();
    for s in sources {
        for f in &s.meta.shown_faces {
            let on = match f.as_str() {
                "east" => s.frame.hi[0] == hi[0],
                "west" => s.frame.lo[0] == lo[0],
                "south" => s.frame.hi[2] == hi[2],
                "north" => s.frame.lo[2] == lo[2],
                "up" => s.frame.hi[1] == hi[1],
                "down" => s.frame.lo[1] == lo[1],
                _ => false,
            };
            if on {
                out.insert(f.as_str());
            }
        }
    }
    // The library's own order.
    ["north", "south", "east", "west", "up", "down"]
        .iter()
        .filter(|f| out.contains(**f))
        .map(|f| (*f).to_string())
        .collect()
}

/// **What a cut severs, by name** (spec-0106 §3.9): every site-plan volume,
/// assembly and pulse whose cells straddle the cuboid's boundary. Gates and
/// stations are [`derive_anchors`]'s.
fn severed_objects(
    campaign: &Campaign,
    built: &Built<'_>,
    _sources: &[Source<'_>],
    lo: [i64; 3],
    hi: [i64; 3],
    residue: &mut Vec<Residue>,
) {
    let inside = |c: [i64; 3]| (0..3).all(|a| lo[a] <= c[a] && c[a] <= hi[a]);
    let straddles = |a: [i64; 3], b: [i64; 3]| {
        let l = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
        let h = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
        let meets = (0..3).all(|i| l[i] <= hi[i] && h[i] >= lo[i]);
        let within = inside(l) && inside(h);
        meets && !within
    };
    if let Some(p) = campaign.site_plan.as_ref() {
        for v in &p.content.volumes {
            if straddles(v.region.min, v.region.max()) {
                residue.push(Residue {
                    object: v.id.0.clone(),
                    document: "site-plan.json".to_string(),
                    severed: "a volume the cut crosses: it is the whole's, and the seated \
                              campaign declares its own"
                        .to_string(),
                });
            }
        }
    }
    let quests = &campaign.quests.content;
    for a in &quests.assemblies {
        let Some(p) = built.plan.point_any(a.at.anchor.as_str()) else {
            continue;
        };
        let p = p.map(i64::from);
        let (w, h) = a
            .hitbox
            .as_ref()
            .map_or((0.0, 0.0), |hb| (hb.width, hb.height));
        let r = (w / 2.0).ceil() as i64;
        let top = h.ceil() as i64;
        if straddles([p[0] - r, p[1], p[2] - r], [p[0] + r, p[1] + top, p[2] + r]) {
            residue.push(Residue {
                object: a.id.0.clone(),
                document: "quests.json".to_string(),
                severed: "an assembly the cut crosses: its body reaches outside the scope, so \
                          it is dropped"
                    .to_string(),
            });
        }
    }
    for pl in &quests.pulses {
        let heard: Option<([i64; 3], [i64; 3])> = match (&pl.heard.region, &pl.heard.place) {
            (Some(zone), _) => built
                .plan
                .zone_box(zone)
                .map(|(a, b)| (a.map(i64::from), b.map(i64::from))),
            (None, Some(place)) => Frame::all(campaign)
                .into_iter()
                .find(|(f, _)| f.node.0 == *place)
                .map(|(f, _)| (f.lo, f.hi)),
            (None, None) => None,
        };
        if let Some((a, b)) = heard
            && straddles(a, b)
        {
            residue.push(Residue {
                object: pl.id.0.clone(),
                document: "quests.json".to_string(),
                severed: "a pulse heard across the cut: the place it is heard in lies either \
                          side of the scope's boundary, so it is dropped"
                    .to_string(),
            });
        }
    }
}

/// Dark cells within [`DARK_AT_CUT`] of a cut face, measured over the piece
/// on its own under its contract's sky, entered at its candidate openings.
fn dark_at_cut(
    model: &VoxelModel,
    contract: &SpatialContract,
    openings: &[PieceOpening],
    relight: Option<&delvewright_dsl::AreaLighting>,
) -> usize {
    let threshold = relight.map_or(DEFAULT_DARK_THRESHOLD, |l| i32::from(l.min_light));
    let origin = model.region().origin;
    let size = model.region().size;
    let doors: BTreeSet<[i32; 3]> = openings
        .iter()
        .flat_map(|o| {
            let [l, h] = o.cells;
            let mut v = Vec::new();
            for x in l[0]..=h[0] {
                for y in l[1]..=h[1] {
                    for z in l[2]..=h[2] {
                        v.push([origin[0] + x, origin[1] + y, origin[2] + z]);
                    }
                }
            }
            v
        })
        .collect();
    let dark =
        delvec::admit::light::dark_cells(model, threshold, SkyClaim::of(Some(contract)), &doors);
    let near = |c: &[i32; 3]| {
        (0..3).filter(|&a| a != 1).any(|a| {
            c[a] - origin[a] < DARK_AT_CUT || (origin[a] + size[a] as i32 - 1) - c[a] < DARK_AT_CUT
        })
    };
    dark.iter().filter(|c| near(c)).count()
}

/// **The bodies the source stands inside the cuboid** — the NPCs, actors and
/// assemblies whose anchors resolve inside it, and every furniture anchor the
/// piece carries.
fn derive_entities(
    campaign: &Campaign,
    built: &Built<'_>,
    anchors: &BTreeMap<String, Anchor>,
    _lo: [i64; 3],
    inside: impl Fn([i64; 3]) -> bool,
) -> Vec<PieceEntity> {
    let mut out: Vec<PieceEntity> = Vec::new();
    let at = |a: &str| built.plan.point_any(a).map(|p| p.map(i64::from));
    for n in &campaign.npcs.content.npcs {
        if at(n.anchor.as_str()).is_some_and(&inside) {
            out.push(PieceEntity {
                kind: "npc".to_string(),
                id: n.id.0.clone(),
                anchor: n.anchor.as_str().to_string(),
                document: "npcs.json".to_string(),
            });
        }
    }
    let quests = &campaign.quests.content;
    for a in &quests.actors {
        if at(a.anchor.as_str()).is_some_and(&inside) {
            out.push(PieceEntity {
                kind: "actor".to_string(),
                id: a.id.0.clone(),
                anchor: a.anchor.as_str().to_string(),
                document: "quests.json".to_string(),
            });
        }
    }
    for a in &quests.assemblies {
        let anchor = a.at.anchor.as_str();
        if at(anchor).is_some_and(&inside) {
            out.push(PieceEntity {
                kind: "assembly".to_string(),
                id: a.id.0.clone(),
                anchor: anchor.to_string(),
                document: "quests.json".to_string(),
            });
        }
    }
    for (k, a) in anchors {
        if a.role == Some(delvewright_dsl::prefab::AnchorRole::Furniture) {
            out.push(PieceEntity {
                kind: "furniture".to_string(),
                id: k.clone(),
                anchor: k.clone(),
                document: "prefab".to_string(),
            });
        }
    }
    out
}
