//! `delvec prefab seat` — an extracted prefab becomes a place (spec-0106
//! §3.9).
//!
//! The author decides two things: where the place stands (`--at`, `--floor`)
//! and which neighbour each declared opening joins (`--join`, or `--commons`
//! for an opening onto the site's open ground). Everything else the place's
//! rows say is the prefab's: the `boxes[]` row (its extent, ceiling and base
//! from the piece's cuboid and walk plane, its light, mitigation and sky from
//! the extraction), the node's `stations[]`, the `details[]` row and its
//! anchors, and each joined opening's seam — its face, offsets and size from
//! the opening's cells, its form from the opening. The would-be campaign is
//! validated before anything is written, through the same funnel `delvec
//! validate` runs, so an opening answered by nothing (`DW1022`), a station
//! the campaign already holds (`DW0870`) or a frame the piece does not fill
//! (`DW0843`) is refused where it is entered.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::ExitCode;

use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::detailplan::{Detail, DetailPlanContent};
use delvewright_dsl::layout::{LayoutGraphContent, Station};
use delvewright_dsl::prefab::{AnchorRole, PieceOpening};
use delvewright_dsl::siteplan::{Base, Face, SitePlanContent};
use delvewright_dsl::{AnchorId, Diagnostic, Envelope, NodeId, PrefabId, Stage};

use crate::EXIT_INTERNAL;
use crate::cli::campaign::{has_error, load_or_refuse, validate_loaded};
use crate::cli::report::print_one_diag;

/// The arguments of `delvec prefab seat`.
pub(crate) struct SeatArgs<'a> {
    pub(crate) campaign_dir: &'a Path,
    pub(crate) prefab: &'a str,
    pub(crate) node: &'a str,
    pub(crate) at: &'a str,
    pub(crate) floor: Option<i64>,
    pub(crate) joins: &'a [String],
    pub(crate) commons: &'a [String],
    pub(crate) renames: &'a [String],
    pub(crate) prefabs_dir: &'a Path,
    pub(crate) json: bool,
}

pub(crate) fn run_seat(args: &SeatArgs<'_>) -> ExitCode {
    match seat(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn usage(msg: impl std::fmt::Display) -> u8 {
    eprintln!("error: {msg}");
    1
}

fn pair(text: &str, what: &str) -> Result<(String, String), u8> {
    text.split_once('=')
        .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
        .filter(|(a, b)| !a.is_empty() && !b.is_empty())
        .ok_or_else(|| usage(format!("`{text}` is not `{what}`")))
}

fn parse<T: serde::de::DeserializeOwned>(text: Option<&str>, file: &str) -> Result<T, u8> {
    let Some(text) = text else {
        return Err(usage(format!(
            "the campaign carries no `{file}`; a prefab is seated as a place of the site plan, \
             so the layout graph and the site plan come first"
        )));
    };
    serde_json::from_str(text).map_err(|e| usage(format!("`{file}` does not parse: {e}")))
}

fn seat(args: &SeatArgs<'_>) -> Result<(), u8> {
    let mut loaded = load_or_refuse(args.campaign_dir, args.json)?;
    let library = PrefabRegistry::load_dir(args.prefabs_dir).map_err(|e| {
        eprintln!(
            "internal error: cannot read prefabs dir {}: {e}",
            args.prefabs_dir.display()
        );
        EXIT_INTERNAL
    })?;
    let Some(meta) = library.get(args.prefab).cloned() else {
        return Err(usage(format!(
            "the library at {} holds no `{}`",
            args.prefabs_dir.display(),
            args.prefab
        )));
    };
    let Some(x) = meta.extraction.clone() else {
        return Err(usage(format!(
            "`{}` carries no `extraction`: it was not lifted out of a campaign, so it declares \
             no openings to seat. Bind it to a place the plan already allocates with `delvec \
             detail <campaign> <place> --piece {}`",
            args.prefab, args.prefab
        )));
    };
    let Some(walk_y) = meta.walk_y else {
        return Err(usage(format!("`{}` states no `walk_y`", args.prefab)));
    };
    let node = NodeId(args.node.to_string());
    let (ax, az) = {
        let (a, b) = args
            .at
            .split_once(',')
            .ok_or_else(|| usage(format!("`--at {}` is not `x,z`", args.at)))?;
        (
            a.trim()
                .parse::<i64>()
                .map_err(|_| usage(format!("`--at {}` is not `x,z`", args.at)))?,
            b.trim()
                .parse::<i64>()
                .map_err(|_| usage(format!("`--at {}` is not `x,z`", args.at)))?,
        )
    };
    let size = meta.size();
    if size[0] < 3 || size[2] < 3 || size[1] <= walk_y {
        return Err(usage(format!(
            "`{}` is {}x{}x{} with its walk plane at {walk_y}: a place needs a play space inside \
             a one-cell ring and headroom over its floor",
            args.prefab, size[0], size[1], size[2]
        )));
    }
    let renames: BTreeMap<String, String> = args
        .renames
        .iter()
        .map(|r| pair(r, "FROM=TO"))
        .collect::<Result<_, _>>()?;
    let joins: Vec<(String, String)> = args
        .joins
        .iter()
        .map(|j| pair(j, "OPENING=EDGE"))
        .collect::<Result<_, _>>()?;
    for name in joins.iter().map(|(o, _)| o).chain(args.commons.iter()) {
        if !x.openings.iter().any(|o| &o.name == name) {
            let known: Vec<&str> = x.openings.iter().map(|o| o.name.as_str()).collect();
            return Err(usage(format!(
                "`{}` declares no opening `{name}`; it declares {}",
                args.prefab,
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            )));
        }
    }

    let mut graph: Envelope<LayoutGraphContent> =
        parse(loaded.raw.layout_graph.as_deref(), "layout-graph.json")?;
    let mut plan: Envelope<SitePlanContent> =
        parse(loaded.raw.site_plan.as_deref(), "site-plan.json")?;
    let mut details: Envelope<DetailPlanContent> = match loaded.raw.detail_plan.as_deref() {
        Some(t) => serde_json::from_str(t)
            .map_err(|e| usage(format!("`detail-plan.json` does not parse: {e}")))?,
        None => Envelope {
            dsl_version: delvec::compiler::DSL_VERSION.to_string(),
            campaign_id: plan.campaign_id.clone(),
            stage: Stage::DetailPlan,
            content: DetailPlanContent {
                palette: None,
                details: Vec::new(),
            },
        },
    };
    if !graph.content.nodes.iter().any(|n| n.id == node) {
        return Err(usage(format!(
            "the layout graph declares no `{node}`: a seated prefab becomes a place the graph \
             already names — declare the node (its intent, and the connections its openings \
             join) first"
        )));
    }

    // ---- where it stands ----
    let floor = match args.floor {
        Some(y) => y,
        None => {
            let first_commons = x
                .openings
                .iter()
                .find(|o| args.commons.iter().any(|c| c == &o.name));
            default_floor(
                &loaded,
                &plan.content,
                [ax, az],
                size,
                walk_y,
                first_commons,
            )?
        }
    };
    let frame_lo = [ax, floor - i64::from(walk_y), az];

    // ---- the stations, renamed where asked ----
    let others: BTreeSet<&str> = graph
        .content
        .nodes
        .iter()
        .filter(|n| n.id != node)
        .flat_map(|n| n.stations.iter().map(|s| s.anchor.as_str()))
        .collect();
    let mut seated: Vec<(Station, String)> = Vec::new();
    let mut taken: Vec<String> = Vec::new();
    for st in &x.stations {
        let key = st.anchor.as_str().to_string();
        let name = renames.get(&key).cloned().unwrap_or_else(|| key.clone());
        if others.contains(name.as_str()) {
            taken.push(name.clone());
        }
        let mut s = st.clone();
        s.anchor = AnchorId(name);
        seated.push((s, key));
    }
    if !taken.is_empty() {
        let d = Diagnostic::error(
            delvewright_dsl::layout::DW_STATION_DUPLICATE,
            "layout-graph",
            format!("/content/nodes/{node}/stations"),
            format!(
                "seating `{}` as `{node}` declares {n} station(s) another place of this campaign \
                 already holds: {list}. A station's name is unique across the campaign. Seat it \
                 under other names — `--rename {first}=<new>` for each — and the details row \
                 binds each new name to the piece's own anchor. Nothing was written.",
                args.prefab,
                n = taken.len(),
                list = taken
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                first = taken[0],
            ),
        );
        print_one_diag(&d, args.json);
        return Err(1);
    }
    let gnode = graph
        .content
        .nodes
        .iter_mut()
        .find(|n| n.id == node)
        .expect("checked above");
    for (s, _) in &seated {
        if !gnode.stations.iter().any(|t| t.anchor == s.anchor) {
            gnode.stations.push(s.clone());
        }
    }

    // ---- the box ----
    let base = match x.ground {
        Base::Aloft(_) => Base::Aloft((walk_y - 1) as u32),
        _ => Base::Owned((walk_y - 1) as u32),
    };
    let mut boxv = serde_json::json!({
        "node": node.0,
        "min": [ax + 1, az + 1],
        "extent": [size[0] - 2, size[2] - 2],
        "floor": {"y": floor},
        "ceiling": {"open": size[1] - walk_y},
        "base": base,
    });
    if let Some(l) = &x.relight {
        boxv["lighting"] = serde_json::to_value(l).expect("lighting");
    }
    if let Some(m) = &x.mitigation {
        boxv["mitigation"] = serde_json::to_value(m).expect("mitigation");
    }
    if let Some(a) = &x.atmosphere {
        boxv["atmosphere"] = serde_json::Value::String(a.0.clone());
    }
    if !args.commons.is_empty() {
        boxv["commons"] = serde_json::to_value(args.commons).expect("names");
    }
    let new_box: delvewright_dsl::siteplan::PlanBox =
        serde_json::from_value(boxv).map_err(|e| usage(format!("the box does not form: {e}")))?;
    match plan.content.boxes.iter().position(|b| b.node == node) {
        Some(i) => plan.content.boxes[i] = new_box,
        None => plan.content.boxes.push(new_box),
    }

    // ---- the seams, one per joined opening ----
    for (oname, edge) in &joins {
        let o = x
            .openings
            .iter()
            .find(|o| &o.name == oname)
            .expect("checked above");
        let seam = seam_for(&graph.content, &plan, &node, o, edge, frame_lo)?;
        plan.content.seams.retain(|s| s.edge != seam.edge);
        plan.content.seams.push(seam);
    }

    // ---- the details row ----
    let mut judged = delvewright_dsl::parse_campaign(&{
        let mut raw = loaded.raw.clone();
        raw.layout_graph = Some(canonical(&graph));
        raw.site_plan = Some(canonical(&plan));
        raw
    })
    .map(|mut c| {
        c.heightmap = loaded.heightmap.clone();
        c
    })
    .map_err(|d| {
        for x in &d {
            print_one_diag(x, args.json);
        }
        1u8
    })?;
    let entry = meta
        .anchors
        .iter()
        .find(|(_, a)| a.role == Some(AnchorRole::Entry))
        .map(|(k, _)| k.clone());
    let by_name: BTreeMap<&str, &str> = seated
        .iter()
        .map(|(s, k)| (s.anchor.as_str(), k.as_str()))
        .collect();
    let node_anchor = delvewright_dsl::siteplan::node_anchor(&node);
    let mut anchors: BTreeMap<String, String> = BTreeMap::new();
    for owed in delvewright_dsl::owed_anchors(&judged, &node) {
        if let Some(k) = by_name.get(owed.as_str()) {
            anchors.insert(owed.clone(), (*k).to_string());
        } else if owed == delvewright_dsl::siteplan::ENTRY_ANCHOR || owed == node_anchor {
            if let Some(k) = &entry {
                anchors.insert(owed.clone(), k.clone());
            }
        } else {
            let local = owed.strip_prefix("anchor/").unwrap_or(&owed);
            if let Some(k) = [local.to_string(), format!("anchor/{local}")]
                .into_iter()
                .find(|k| meta.anchors.contains_key(k))
            {
                anchors.insert(owed.clone(), k);
            }
        }
    }
    let row = Detail {
        place: node.clone(),
        piece: PrefabId(meta.prefab_id.clone()),
        anchors,
    };
    match details.content.details.iter().position(|d| d.place == node) {
        Some(i) => details.content.details[i] = row.clone(),
        None => details.content.details.push(row.clone()),
    }

    // ---- judged before anything is written ----
    let (gtext, ptext, dtext) = (canonical(&graph), canonical(&plan), canonical(&details));
    loaded.raw.layout_graph = Some(gtext.clone());
    loaded.raw.site_plan = Some(ptext.clone());
    loaded.raw.detail_plan = Some(dtext.clone());
    let v = validate_loaded(loaded, args.prefabs_dir, args.json)?;
    if has_error(&v.diags) {
        eprintln!(
            "error: `{}` would not seat as `{node}`; nothing was written.",
            args.prefab
        );
        return Err(1);
    }
    judged = v.campaign;
    let (_, binding) = delvec::compiler::detail::check(&judged, &v.prefabs);
    let dir = args.campaign_dir;
    for (file, text) in [
        (delvec::compiler::load::LAYOUT_GRAPH_FILE, &gtext),
        (delvec::compiler::load::SITE_PLAN_FILE, &ptext),
        (delvec::compiler::load::DETAIL_PLAN_FILE, &dtext),
    ] {
        std::fs::write(dir.join(file), text).map_err(|e| {
            eprintln!(
                "internal error: cannot write {}: {e}",
                dir.join(file).display()
            );
            EXIT_INTERNAL
        })?;
    }
    eprintln!(
        "seated `{}` as `{node}`: cuboid {}x{}x{} from [{}, {}, {}], floor {floor}; {} \
         station(s) declared, {} owed name(s) bound, {} seam(s) written, {} opening(s) to the \
         commons. {}",
        args.prefab,
        size[0],
        size[1],
        size[2],
        frame_lo[0],
        frame_lo[1],
        frame_lo[2],
        seated.len(),
        row.anchors.len(),
        joins.len(),
        args.commons.len(),
        binding.line()
    );
    if !x.residue.is_empty() {
        eprintln!(
            "  the extraction left {} residue row(s); each is answered by the seated campaign's \
             own checks at `delvec build`:",
            x.residue.len()
        );
        for r in &x.residue {
            eprintln!("    {} ({}): {}", r.object, r.document, r.severed);
        }
    }
    Ok(())
}

/// A document as `delvec fmt` writes it.
fn canonical<T: serde::Serialize>(v: &T) -> String {
    delvewright_dsl::to_canonical_string(v).expect("a stage document serialises")
}

/// The walk plane `--floor` defaults to. On an `open` site where an opening
/// is disposed to the commons, the first such opening's floor meets the
/// ground outside it — a body walks out onto the commons at grade. Otherwise
/// the piece's bottom course rests on the highest terrain under its footprint
/// and ring (`top + 1 + walk_y`), and on a `solid` site on the plan region's
/// floor (`min y + walk_y`).
fn default_floor(
    loaded: &delvec::compiler::load::LoadedCampaign,
    plan: &SitePlanContent,
    at: [i64; 2],
    size: [i32; 3],
    walk_y: i32,
    commons: Option<&PieceOpening>,
) -> Result<i64, u8> {
    let campaign = delvec::compiler::load::parse_loaded(loaded)
        .map_err(|_| usage("the campaign does not parse; give the floor with `--floor <y>`"))?;
    let ground = delvewright_dsl::siteplan::Ground::of(&campaign);
    if let Some(o) = commons
        && ground.is_open()
    {
        let v = o.face.vector();
        let out = [
            at[0] + i64::from(o.cells[0][0]) + v[0],
            at[1] + i64::from(o.cells[0][2]) + v[2],
        ];
        if let Some(t) = ground.top(out[0], out[1]) {
            return Ok(t + 1 - i64::from(o.cells[0][1]) + i64::from(walk_y));
        }
    }
    let mut top: Option<i64> = None;
    for x in at[0]..at[0] + i64::from(size[0]) {
        for z in at[1]..at[1] + i64::from(size[2]) {
            if let Some(t) = ground.top(x, z) {
                top = Some(top.map_or(t, |m: i64| m.max(t)));
            }
        }
    }
    Ok(match top {
        Some(t) if ground.is_open() => t + 1 + i64::from(walk_y),
        _ => plan.region.min[1] + i64::from(walk_y),
    })
}

/// The seam that answers `o` across `edge`: its face as the edge's `a` box
/// sees it, its offsets on both boxes from the opening's world cells, its size
/// from the opening, its form from the opening.
fn seam_for(
    graph: &LayoutGraphContent,
    plan: &Envelope<SitePlanContent>,
    node: &NodeId,
    o: &PieceOpening,
    edge: &str,
    frame_lo: [i64; 3],
) -> Result<delvewright_dsl::siteplan::Seam, u8> {
    let Some(e) = graph.edges.iter().find(|e| e.id().0 == edge) else {
        return Err(usage(format!(
            "`--join {}={edge}`: the layout graph declares no connection `{edge}`",
            o.name
        )));
    };
    let (a, b) = (e.a().clone(), e.b().clone());
    let other = if &a == node {
        b.clone()
    } else if &b == node {
        a.clone()
    } else {
        return Err(usage(format!(
            "`--join {}={edge}`: `{edge}` joins `{a}` and `{b}`, not `{node}`",
            o.name
        )));
    };
    let Some(ob) = plan.content.boxes.iter().find(|bx| bx.node == other) else {
        return Err(usage(format!(
            "`--join {}={edge}`: `{other}` has no box in the site plan to meet",
            o.name
        )));
    };
    let Some(omin) = ob.min else {
        return Err(usage(format!(
            "`--join {}={edge}`: `{other}` stands where the packing puts it; pin it (`min`) so \
             the seam's cells can be stated on both boxes",
            o.name
        )));
    };
    let mine_min = [frame_lo[0] + 1, frame_lo[2] + 1];
    let lo = [
        frame_lo[0] + i64::from(o.cells[0][0]),
        frame_lo[1] + i64::from(o.cells[0][1]),
        frame_lo[2] + i64::from(o.cells[0][2]),
    ];
    let face = if &a == node { o.face } else { opposite(o.face) };
    let offset = |min: [i64; 2]| -> delvewright_dsl::siteplan::Offset {
        match o.face {
            Face::East | Face::West => delvewright_dsl::siteplan::Offset::Along(lo[2] - min[1]),
            Face::North | Face::South => delvewright_dsl::siteplan::Offset::Along(lo[0] - min[0]),
            Face::Up | Face::Down => {
                delvewright_dsl::siteplan::Offset::Plane([lo[0] - min[0], lo[2] - min[1]])
            }
        }
    };
    let (amin, bmin) = if &a == node {
        (mine_min, omin)
    } else {
        (omin, mine_min)
    };
    let stair_in = (e.class() == "stair").then(|| {
        if o.kind == "stair" {
            node.clone()
        } else {
            other.clone()
        }
    });
    let v = serde_json::json!({
        "edge": edge,
        "face": face,
        "at": offset(amin),
        "meets": offset(bmin),
        "opening": {"width": o.width, "height": o.height},
        "form": o.form,
    });
    let mut s: delvewright_dsl::siteplan::Seam =
        serde_json::from_value(v).map_err(|e| usage(format!("the seam does not form: {e}")))?;
    s.stair_in = stair_in;
    Ok(s)
}

fn opposite(f: Face) -> Face {
    match f {
        Face::East => Face::West,
        Face::West => Face::East,
        Face::North => Face::South,
        Face::South => Face::North,
        Face::Up => Face::Down,
        Face::Down => Face::Up,
    }
}
