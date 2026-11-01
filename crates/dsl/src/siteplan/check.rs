//! The site plan's validation driver and the checks that read the graph and the
//! plan together: the binding ledger, `DW0824`'s agreement, `DW0839`'s one
//! placement authority, the stage-6 deferral clause and the lighting range.

use super::*;

// ---------------------------------------------------------------------------
// The binding ledger's site-plan half
// ---------------------------------------------------------------------------

/// What a run's site-plan checks bound to.
///
/// Carried inside [`crate::LayoutBinding`] rather than as a second ledger: the
/// map pipeline's documents are counted by one struct with one constructor, so
/// the number the CLI prints and the number a diagnostic quotes cannot disagree
/// about how many places there are.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct PlanBinding {
    /// Places embedded — what `DW0826` examines.
    pub boxes: usize,
    /// Unordered box pairs — what `DW0827` examines. Zero at one box, which is
    /// the honest count and not a pass.
    pub box_pairs: usize,
    /// Connections allocated — what `DW0828` and `DW0829` examine.
    pub seams: usize,
    /// Of those, seams whose edge is a `stair` — what `DW0830` examines.
    pub stair_seams: usize,
    /// Of those, seams whose edge is a `drop` — what `DW0831` examines.
    pub drop_seams: usize,
    /// Named ground planes.
    pub datums: usize,
    /// Whole-owned masses — what `DW0835` examines.
    pub volumes: usize,
    /// Guarded comparisons against the brief — what `DW0833` examines.
    pub identities: usize,
    /// Sightlines embedded.
    pub sightlines: usize,
    /// Named exterior vantages the walk judges the silhouette from.
    pub views: usize,
    /// Boxes whose corner the author pinned (spec-0059 §3).
    pub pinned: usize,
    /// Boxes whose corner a seam derived.
    pub derived: usize,
    /// Connected components of the seam graph.
    pub components: usize,
    /// Boxes declaring a `roof` — what `DW0988` examines (spec-0098).
    pub roofs: usize,
    /// Cells two claims share that a rule awarded — what `DW0827`'s widened
    /// quantifier examined and found owned (spec-0098 §2).
    pub contested_awarded: usize,
    /// The fill's kind, `solid` or `open` (spec-0098 §2b).
    pub fill: &'static str,
    /// Connections the world builds shut ([`Edge::shuts`]) that carry a seam
    /// here — what `DW1027` and `DW1028` examine (spec-0107 §8).
    pub gated: usize,
    /// Of those, connections whose seam is a front (`DW1027` refuses each).
    pub gated_fronts: usize,
    /// Of `gated`, connections an `open-gate` opens.
    pub opened_by_effect: usize,
    /// Of `gated`, connections no `open-gate` opens and a `shortcuts[]` entry
    /// does.
    pub opened_by_shortcut: usize,
    /// Of `gated`, connections neither of those opens and a `timed_gates[]`
    /// entry's clock does.
    pub opened_by_clock: usize,
    /// Of `gated`, connections nothing opens (`DW1028` refuses each).
    pub opened_by_nothing: usize,
}

/// How a shut connection is opened, first match in this order — the partition
/// [`PlanBinding`] counts and `DW1028` judges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opener {
    Effect,
    Shortcut,
    Clock,
    Nothing,
}

/// Every connection the world builds shut and carrying a seam in this plan, in
/// graph order: its index, the edge, whether its seam is a front, and what
/// opens it.
fn gated_connections<'g>(
    c: &Campaign,
    plan: &SitePlanContent,
    graph: &'g LayoutGraphContent,
) -> Vec<(usize, &'g Edge, bool, Opener)> {
    // Every gate region the campaign opens, however it opens it: an
    // `open-gate` at any nesting depth, a `shortcut` whose far side lifts the
    // bar, a timed gate's clock.
    let mut by_effect: BTreeSet<&str> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_, _, eff| {
        eff.visit_deep(&mut |e| {
            if let Some(a) = e.open_gate_anchor() {
                by_effect.insert(a.0.as_str());
            }
        });
    });
    let by_shortcut: BTreeSet<&str> = c
        .quests
        .content
        .shortcuts
        .iter()
        .map(|s| s.gate.0.as_str())
        .collect();
    let by_clock: BTreeSet<&str> = c
        .quests
        .content
        .timed_gates
        .iter()
        .map(|g| g.gate.0.as_str())
        .collect();
    graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.shuts())
        .filter_map(|(i, e)| {
            let seam = plan.seams.iter().find(|s| &s.edge == e.id())?;
            let want = seam_anchor(e.id());
            let opener = if by_effect.contains(want.as_str()) {
                Opener::Effect
            } else if by_shortcut.contains(want.as_str()) {
                Opener::Shortcut
            } else if by_clock.contains(want.as_str()) {
                Opener::Clock
            } else {
                Opener::Nothing
            };
            Some((i, e, seam.contact.is_some(), opener))
        })
        .collect()
}

impl PlanBinding {
    /// Count what a campaign's site plan offers the checks.
    #[must_use]
    pub fn of(c: &Campaign) -> PlanBinding {
        let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
            return PlanBinding::default();
        };
        let classes: BTreeMap<&str, &'static str> = c
            .layout_graph
            .as_ref()
            .map(|g| {
                g.content
                    .edges
                    .iter()
                    .map(|e| (e.id().0.as_str(), e.class()))
                    .collect()
            })
            .unwrap_or_default();
        let n = plan.boxes.len();
        let (pinned, derived, components) = match c.layout_graph.as_ref() {
            Some(g) => {
                let table = Metrics::table();
                let mut reads = Reads::new();
                let mut sink = Vec::new();
                let (_, packed) = resolve(plan, &g.content, &table, &mut reads, &mut sink);
                (packed.pinned, packed.derived, packed.components)
            }
            None => (0, 0, 0),
        };
        let gated = c
            .layout_graph
            .as_ref()
            .map(|g| gated_connections(c, plan, &g.content))
            .unwrap_or_default();
        let opened = |o: Opener| gated.iter().filter(|r| r.3 == o).count();
        PlanBinding {
            gated: gated.len(),
            gated_fronts: gated.iter().filter(|r| r.2).count(),
            opened_by_effect: opened(Opener::Effect),
            opened_by_shortcut: opened(Opener::Shortcut),
            opened_by_clock: opened(Opener::Clock),
            opened_by_nothing: opened(Opener::Nothing),
            pinned,
            derived,
            components,
            boxes: n,
            box_pairs: n * n.saturating_sub(1) / 2,
            seams: plan.seams.len(),
            stair_seams: plan
                .seams
                .iter()
                .filter(|s| classes.get(s.edge.0.as_str()) == Some(&"stair"))
                .count(),
            drop_seams: plan
                .seams
                .iter()
                .filter(|s| classes.get(s.edge.0.as_str()) == Some(&"drop"))
                .count(),
            datums: plan.datums.len(),
            volumes: plan.volumes.len(),
            identities: plan.identities.len(),
            sightlines: plan.sightlines.len(),
            views: plan.views.len(),
            roofs: plan.boxes.iter().filter(|b| b.roof.is_some()).count(),
            contested_awarded: SitePlan::of(c).site().contests().1,
            fill: match plan.fill {
                Fill::Solid { .. } => "solid",
                Fill::Open { .. } => "open",
            },
        }
    }

    /// The site-plan half of the binding line.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "site-plan binding: {b} box(es) ({p} pair(s) compared; {pn} pinned, {dv} derived, \
             in {cc} component(s); {r} roofed; {ca} shared claim cell(s) awarded), {s} seam(s) \
             ({st} stair, {sd} drop), {d} datum(s), {v} whole-owned volume(s), {i} \
             identity(ies), {sl} sightline(s), {w} view(s); fill `{fill}`. DW1027 binding: {g} \
             gated connection(s), {gf} of them fronts. DW1028 binding: {g} gated connection(s), \
             {ge} opened by an `open-gate`, {gs} by a shortcut, {gc} by a clock, {gn} by \
             nothing.",
            g = self.gated,
            gf = self.gated_fronts,
            ge = self.opened_by_effect,
            gs = self.opened_by_shortcut,
            gc = self.opened_by_clock,
            gn = self.opened_by_nothing,
            r = self.roofs,
            ca = self.contested_awarded,
            fill = if self.fill.is_empty() {
                "none"
            } else {
                self.fill
            },
            pn = self.pinned,
            dv = self.derived,
            cc = self.components,
            b = self.boxes,
            p = self.box_pairs,
            s = self.seams,
            st = self.stair_seams,
            sd = self.drop_seams,
            d = self.datums,
            v = self.volumes,
            i = self.identities,
            sl = self.sightlines,
            w = self.views,
        )
    }
}

// ---------------------------------------------------------------------------
// Validation (spec-0049 §4.3) — every rule, all upstream of any geometry
// ---------------------------------------------------------------------------

/// Every check the site plan owes, at **validation** tier (exit 1).
///
/// # What invokes this, and what happens without it
///
/// [`crate::validate::validate_campaign_with`], whenever the campaign directory
/// holds a `site-plan.json` — the same event-bound shape stages 2, 3 and 7 use.
/// That function is what **every** `delvec` subcommand's validation stage calls,
/// so there is no path from a campaign directory to a verdict, a world or a
/// datapack that goes round it: `validate`, `analyze` and `build` all enter
/// through it, and a caller who somehow skipped it would have no `Campaign` to
/// hand anything downstream. There is no flag to pass, no step in a document to
/// remember, and no second entry point.
///
/// # Why every rule is here and none is at analysis tier
///
/// Round 2 put the layout graph's *reachability* proofs at analysis tier because
/// reachability is a question about a whole graph, the tier its quest-graph
/// siblings answer at. Nothing here is that question. Every rule below is a
/// property of the document in front of it — does this box fit, do these two
/// boxes touch, does this number match the brief's — so all of it is validation,
/// and a plan that is wrong is wrong before anything is analyzed.
///
/// # What is deliberately NOT here, and where it went
///
/// Three obligations of this stage are only decidable once the blockout exists,
/// and each is named rather than approximated:
///
/// * whether a built seam is the opening the plan allocated, whether every node's
///   floor is reached, and whether any crossing was *discovered* outside a seam;
/// * `DW0833`'s second call site, the identities recomputed from assembled bytes;
/// * whether a declared sightline is unobstructed.
///
/// All three read blocks. Writing a version of them here that read the plan
/// instead would be the derivation's arithmetic replayed against itself — the
/// opposite of an independent observer — so they belong to the round that builds
/// the blockout, and this module states the plan-side half they will be checked
/// against.
pub fn check(c: &Campaign, reads: &mut Reads, d: &mut Vec<Diagnostic>) {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return;
    };
    let table = Metrics::table();

    // Plan-internal wellformedness first: the ids every rule below quotes.
    ids(plan, d);
    one_authority(c, d);

    // ------------------------------------------------------------- the tooth
    // A site plan validates ONLY against a layout graph and a geometry brief.
    // Both are refused by name, and the graph's absence returns: every rule
    // below reads node ids, and without a graph each would be answering about a
    // place nothing declared.
    let brief_missing = c.geometry_brief.is_none();
    if brief_missing {
        d.push(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            "",
            "this campaign carries a site plan and no `geometry-brief.json`. The plan is the \
             embedding of a design, and the brief is where that design's numbers are written \
             down; with no brief there is nothing for `identities[]` to hold the map to, and \
             the region's extent is a number with no author. Write the brief first — a plan \
             cannot reach green ahead of it."
                .to_string(),
        ));
    }
    let Some(graph) = c.layout_graph.as_ref().map(|g| &g.content) else {
        d.push(Diagnostic::error(
            DW_PLAN_AGREEMENT,
            "site-plan",
            "",
            "this campaign carries a site plan and no `layout-graph.json`. A site plan is the \
             geometric embedding OF a layout graph: every box names a place and every seam \
             names a connection, so with no graph there is nothing being embedded and every \
             name in this document resolves to nothing. This is the only line this state \
             raises: the map's anchor vocabulary is derived from the graph too, so every \
             anchor a `npcs`, `quests` or effect document names is left unjudged here rather \
             than refused against an empty set, and is judged the moment the graph exists. \
             Author the graph first — that ordering \
             is what this refusal exists to make uncompilable rather than merely advised."
                .to_string(),
        ));
        return;
    };

    openers(c, plan, graph, d);
    let (placed, packed) = resolve(plan, graph, &table, reads, d);
    agreement(plan, graph, &placed, d);
    region(plan, &placed, d);
    disjoint(&placed, d);
    roofs(&placed, d);
    fillcheck::fill(c, plan, d);
    fillcheck::claims(c, plan, d);
    seams(plan, graph, &placed, &packed, &table, reads, d);
    ways(plan, d);
    volumes_outside_boxes(plan, &placed, d);
    identities(c, plan, &placed, d);
    lighting(plan, d);
    plots_cannot_hold(plan, graph, d);
}

/// `DW1073`: **a plot cannot hold what it declares** (spec-0125 §3.4) — one
/// code, three shapes, each named in the message: the plot is under a lid,
/// its parts' cells sum past its play space, or a structure's `height` is
/// past its `open` courses. Judged on the declarations only; a plot whose
/// parts fit is asked nothing more, whatever its coverage.
pub fn plots_cannot_hold(
    plan: &SitePlanContent,
    graph: &crate::layout::LayoutGraphContent,
    d: &mut Vec<Diagnostic>,
) {
    for n in &graph.nodes {
        if n.holds.is_empty() {
            continue;
        }
        let Some((bi, b)) = plan.boxes.iter().enumerate().find(|(_, b)| b.node == n.id) else {
            continue; // `DW0824` reported the missing box.
        };
        let names = |ps: &[&crate::layout::Part]| {
            ps.iter()
                .map(|p| format!("`{}`", p.id))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let all: Vec<&crate::layout::Part> = n.holds.iter().collect();
        match b.ceiling {
            Ceiling::Clearance(c) => {
                d.push(Diagnostic::error(
                    DW_PLOT_CANNOT_HOLD,
                    "site-plan",
                    format!("/content/boxes/{bi}/ceiling"),
                    format!(
                        "under a lid: `{node}` is a plot — it declares {k} part(s) ({names}) — and \
                         its box's ceiling is `{{\"clearance\": {c}}}`. A lid is one structure, and \
                         a plot's structures stand under the sky between them. Give the box an \
                         `open` ceiling clearing the tallest ridge, or remove `holds[]` and build \
                         the place as rooms.",
                        node = n.id,
                        k = n.holds.len(),
                        names = names(&all),
                    ),
                ));
            }
            Ceiling::Open(air) => {
                for p in n
                    .holds
                    .iter()
                    .filter(|p| p.kind == crate::layout::PartKind::Structure)
                {
                    let Some(h) = p.height else { continue };
                    if h > air.get() {
                        d.push(Diagnostic::error(
                            DW_PLOT_CANNOT_HOLD,
                            "site-plan",
                            format!("/content/boxes/{bi}/ceiling/open"),
                            format!(
                                "too low: `{node}`'s part `{part}` ({use_}) declares a height of {h} \
                                 course(s) and the box claims {air} course(s) of air over its \
                                 walk plane, {over} short. Raise the box's `open` to at least {h}, \
                                 or lower the part's `height`.",
                                node = n.id,
                                part = p.id,
                                use_ = p.r#use,
                                air = air.get(),
                                over = h - air.get(),
                            ),
                        ));
                    }
                }
            }
        }
        let sum: u64 = n.holds.iter().map(|p| u64::from(p.cells)).sum();
        let area = u64::from(b.extent[0].get()) * u64::from(b.extent[1].get());
        if sum > area {
            d.push(Diagnostic::error(
                DW_PLOT_CANNOT_HOLD,
                "site-plan",
                format!("/content/boxes/{bi}/extent"),
                format!(
                    "too small: `{node}`'s {k} part(s) declare {sum} cell(s) of play space and its \
                     box's play space is {x} × {z} = {area} cell(s), {short} short — before any \
                     wall or way between them. Widen the box's `extent` to hold the parts and \
                     their ways, or lower the parts' `cells`.",
                    node = n.id,
                    k = n.holds.len(),
                    x = b.extent[0].get(),
                    z = b.extent[1].get(),
                    short = sum - area,
                ),
            ));
        }
    }
}

/// `DW0839`: a campaign has ONE placement authority.
///
/// `areas[]` places pieces on the fixed stride; the site plan places the whole
/// map in its own region. A world carrying both has two owners for one question
/// and no rule to pick between them, so the answer is not to arbitrate but to
/// refuse. Both surfaces stay legal at 0.14.0 — one per campaign.
fn one_authority(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let n = c.world.content.areas.len();
    if n == 0 {
        return;
    }
    d.push(Diagnostic::error(
        DW_TWO_AUTHORITIES,
        "world",
        "/content/areas",
        format!(
            "this campaign declares {n} `areas[]` entry(ies) AND a site plan. Those are two \
             placement authorities for one world: `areas[]` seats prefab pieces on the compiler's \
             fixed stride, and the site plan seats the derived blockout inside its own declared \
             `region` — so every question about where something is has two answers and nothing \
             says which. Keep one. A campaign that places pieces keeps `areas[]` and drops \
             `site-plan.json`; a campaign whose map is the site plan declares an empty `areas` \
             list and lets the plan own the space. Both surfaces are legal — what is not legal \
             is one \
             campaign holding both."
        ),
    ));
}

/// **`DW1028`: a gated connection nothing opens** (spec-0107 §8.2).
///
/// The graph half stands in [`crate::layout`]: `DW0818` holds a `gating` to a
/// flag some effect really sets or a quest that really exists. That says the way
/// is *meant* to open; it does not say anything in the campaign ever opens
/// **this** way, because at stage 3 the region such an effect would target does
/// not exist yet. It exists here: the derivation synthesizes [`seam_anchor`]
/// over every shut seam's bar, so "something opens `anchor/seam-<edge>`" is a
/// question with a subject — an `open-gate` at any nesting depth, a shortcut's
/// gate, or a timed gate's clock.
///
/// Raised against the quest documents, the stage that owns the opener. Only
/// reachable in a site-plan campaign, which is exactly the campaign in which the
/// seam anchor exists to be named.
fn openers(
    c: &Campaign,
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    d: &mut Vec<Diagnostic>,
) {
    let unwritten = if c.quests.content.quests.is_empty() {
        " Stage 5 declares no quests at all here, so nothing in it can open anything yet, and \
         this line is that state rather than the finding: see `DW0150`, which names it. This \
         clears when stage 5 is written."
    } else {
        ""
    };
    for (_, e, _, opener) in gated_connections(c, plan, graph) {
        if opener != Opener::Nothing {
            continue;
        }
        let want = seam_anchor(e.id());
        let terms = e.gating().map(|g| g.words()).unwrap_or_default();
        d.push(Diagnostic::error(
            DW_GATED_UNOPENED,
            "quests",
            "/content",
            format!(
                "`{id}` (a `{class}`) is gated on {terms}, so the world builds it shut, and nothing \
                 in this campaign opens it: no `open-gate`, `shortcuts[].gate` or \
                 `timed_gates[].gate` names `{want}`. Shut from world load and shut forever, it is \
                 a wall carrying a claim about order. Write the opener — an `open-gate` naming \
                 `{want}` at the beat whose completion the gating names — or remove the `gating` \
                 from `{id}` in the layout graph.{unwritten}",
                id = e.id(),
                class = e.class(),
            ),
        ));
    }
}

/// `datum/`, `volume/` and `view/` ids: well formed and unique, like every other
/// id in the DSL. An id is an id, so these are the ordinary `DW0110`/`DW0111`.
fn ids(plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut check_id = |ok: bool, id: String, kind: &str, path: String, d: &mut Vec<Diagnostic>| {
        if !ok {
            d.push(Diagnostic::error(
                crate::codes::ID_SYNTAX,
                "site-plan",
                path.clone(),
                format!("malformed {kind} id `{id}` — expected `{kind}/<kebab-case>`."),
            ));
        }
        if !seen.insert(format!("{kind}:{id}")) {
            d.push(Diagnostic::error(
                crate::codes::ID_DUPLICATE,
                "site-plan",
                path,
                format!(
                    "duplicate {kind} id `{id}` — rename one, because anything naming it would \
                     otherwise name both."
                ),
            ));
        }
    };
    for (i, dat) in plan.datums.iter().enumerate() {
        check_id(
            dat.id.is_valid_syntax(),
            dat.id.0.clone(),
            "datum",
            format!("/content/datums/{i}/id"),
            d,
        );
    }
    for (i, v) in plan.volumes.iter().enumerate() {
        check_id(
            v.id.is_valid_syntax(),
            v.id.0.clone(),
            "volume",
            format!("/content/volumes/{i}/id"),
            d,
        );
    }
    for (i, v) in plan.views.iter().enumerate() {
        check_id(
            v.id.is_valid_syntax(),
            v.id.0.clone(),
            "view",
            format!("/content/views/{i}/id"),
            d,
        );
    }
}

/// `DW0824`: the graph and the plan agree **exactly**, in both directions.
///
/// Six correspondences and three references, all one claim: everything the graph
/// declares is embedded exactly once, and everything the plan embeds is
/// something the graph declared.
///
/// This check is also the **two-artifact question's instrument** (spec-0049
/// §10): how often it fires *alone* — a graph edit with no plan edit or the
/// reverse — is the measurable evidence that decides whether the graph and the
/// plan stay two documents or merge into one.
///
/// It reads `plan.boxes` rather than the resolved list, deliberately: a box
/// whose floor named no datum is still a box the author wrote, and reporting it
/// as a place with no box as well would answer a question nobody asked.
fn agreement(
    plan: &SitePlanContent,
    graph: &LayoutGraphContent,
    placed: &[Placed<'_>],
    d: &mut Vec<Diagnostic>,
) {
    let fault = |path: String, msg: String, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(DW_PLAN_AGREEMENT, "site-plan", path, msg));
    };

    // -------------------------------------------------------------- places
    let nodes: BTreeSet<&str> = graph.nodes.iter().map(|n| n.id.0.as_str()).collect();
    let mut boxed: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, b) in plan.boxes.iter().enumerate() {
        if !nodes.contains(b.node.0.as_str()) {
            fault(
                format!("/content/boxes/{i}/node"),
                format!(
                    "this box embeds `{n}`, which the layout graph declares no place for. A box \
                     is the geometry OF a place; one that names nothing is a room the map has \
                     no reason to contain. Declare the place, or delete the box.",
                    n = b.node,
                ),
                d,
            );
            continue;
        }
        boxed.entry(b.node.0.as_str()).or_default().push(i);
    }
    for (i, n) in graph.nodes.iter().enumerate() {
        match boxed.get(n.id.0.as_str()).map_or(0, Vec::len) {
            1 => {}
            0 => fault(
                "/content/boxes".to_string(),
                format!(
                    "place `{id}` has no box. Every place the graph declares is embedded exactly \
                     once — an unembedded place is a room the plan forgot, and nothing later can \
                     notice it, because every geometric rule quantifies over the boxes that \
                     exist. (Graph node {i} of {total}.)",
                    id = n.id,
                    total = graph.nodes.len(),
                ),
                d,
            ),
            k => fault(
                "/content/boxes".to_string(),
                format!(
                    "place `{id}` has {k} boxes. A place is one space; two boxes for it make \
                     every rule below pick one of them and no rule says which.",
                    id = n.id,
                ),
                d,
            ),
        }
    }

    // --------------------------------------------------------- connections
    let edges: BTreeMap<&str, &Edge> = graph.edges.iter().map(|e| (e.id().0.as_str(), e)).collect();
    let mut seamed: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in plan.seams.iter().enumerate() {
        match edges.get(s.edge.0.as_str()) {
            None => fault(
                format!("/content/seams/{i}/edge"),
                format!(
                    "this seam allocates `{e}`, which the layout graph declares no connection \
                     for. A seam is an opening cut for a connection; one that names nothing is a \
                     hole in a wall for no reason.",
                    e = s.edge,
                ),
                d,
            ),
            Some(Edge::Vision { .. }) => fault(
                format!("/content/seams/{i}/edge"),
                format!(
                    "`{e}` is a `vision` connection and carries a **sightline**, not a seam. A \
                     seam is an opening on a shared face, and a vista's two ends are routinely \
                     not adjacent — a tower seen from a shore shares no face with it — so the \
                     seam construct cannot state the one thing a vision connection asserts. Move \
                     it to `sightlines[]`.",
                    e = s.edge,
                ),
                d,
            ),
            Some(Edge::Carry { .. }) => fault(
                format!("/content/seams/{i}/edge"),
                format!(
                    "`{e}` is a `carry` connection: a body crosses it by being put down on the \
                     far side by a link, so no opening is cut for it. Remove this seam.",
                    e = s.edge,
                ),
                d,
            ),
            Some(_) => {
                seamed.entry(s.edge.0.as_str()).or_default().push(i);
            }
        }
    }
    let mut sighted: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in plan.sightlines.iter().enumerate() {
        match edges.get(s.edge.0.as_str()) {
            None => fault(
                format!("/content/sightlines/{i}/edge"),
                format!(
                    "this sightline embeds `{e}`, which the layout graph declares no connection \
                     for.",
                    e = s.edge,
                ),
                d,
            ),
            Some(Edge::Carry { .. }) => fault(
                format!("/content/sightlines/{i}/edge"),
                format!(
                    "`{e}` is a `carry` connection: a body crosses it by being put down on the \
                     far side by a link, so nothing is embedded for it. Remove this sightline.",
                    e = s.edge,
                ),
                d,
            ),
            Some(e) if !matches!(e, Edge::Vision { .. }) => fault(
                format!("/content/sightlines/{i}/edge"),
                format!(
                    "`{id}` is a `{class}` connection: a body passes along it, so it is allocated \
                     a **seam** on a shared face rather than a line of sight. Move it to \
                     `seams[]`.",
                    id = s.edge,
                    class = e.class(),
                ),
                d,
            ),
            Some(_) => {
                sighted.entry(s.edge.0.as_str()).or_default().push(i);
            }
        }
    }
    for (i, e) in graph.edges.iter().enumerate() {
        // A carry is owed neither: a body crosses it by being put down on the
        // far side, and the geometry has nothing to allocate for that.
        if matches!(e, Edge::Carry { .. }) {
            continue;
        }
        let (what, held, other) = if e.is_traversal() {
            ("seam", &seamed, "seams")
        } else {
            ("sightline", &sighted, "sightlines")
        };
        match held.get(e.id().0.as_str()).map_or(0, Vec::len) {
            1 => {}
            0 => fault(
                format!("/content/{other}"),
                format!(
                    "connection `{id}` ({class}) has no {what}. **Seams are allocated, not \
                     discovered**: two places connect because the plan cut an opening between \
                     them while both were still free to move, never because a wall happened to \
                     be low somewhere. A connection with nothing allocated is a promise the \
                     geometry has not been asked to keep. (Graph edge {i} of {total}.)",
                    id = e.id(),
                    class = e.class(),
                    total = graph.edges.len(),
                ),
                d,
            ),
            k => fault(
                format!("/content/{other}"),
                format!(
                    "connection `{id}` has {k} {what}s. One connection is one way through; two \
                     openings for it are two ways, and the graph declared one.",
                    id = e.id(),
                ),
                d,
            ),
        }
    }

    // ---------------------------------- what a seam says ABOUT its connection
    let by_node: BTreeMap<&str, &Placed<'_>> =
        placed.iter().map(|p| (p.plan.node.0.as_str(), p)).collect();
    for (i, s) in plan.seams.iter().enumerate() {
        let Some(e) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        let Some(host) = &s.stair_in else {
            continue;
        };
        if !matches!(e, Edge::Stair { .. }) {
            fault(
                format!("/content/seams/{i}/stair_in"),
                format!(
                    "this seam declares stair massing in `{host}`, and `{id}` is a `{class}` \
                     connection. Only a stair is built out of treads; on anything else the \
                     declaration is a fact about the geometry that the graph contradicts.",
                    id = s.edge,
                    class = e.class(),
                ),
                d,
            );
        } else if host != e.a() && host != e.b() {
            fault(
                format!("/content/seams/{i}/stair_in"),
                format!(
                    "this seam hosts its stair in `{host}`, which is neither end of `{id}` \
                     (`{a}` and `{b}`). A stair stands in one of the two places it joins.",
                    id = s.edge,
                    a = e.a(),
                    b = e.b(),
                ),
                d,
            );
        }
    }

    // ------------------------- what a sightline says ABOUT its two places
    for (i, s) in plan.sightlines.iter().enumerate() {
        let Some(e) = edges.get(s.edge.0.as_str()) else {
            continue;
        };
        for (end, point, node) in [("from", s.from, e.a()), ("to", s.to, e.b())] {
            let Some(p) = by_node.get(node.0.as_str()) else {
                continue;
            };
            if contains_point(p, point) {
                continue;
            }
            fault(
                format!("/content/sightlines/{i}/{end}"),
                format!(
                    "`{id}` is a line of sight between `{a}` and `{b}`, and its `{end}` end \
                     `[{x}, {y}, {z}]` is not inside `{node}`. The stage-5 proof walks exactly \
                     this segment and calls the result the vista's; a segment whose ends are \
                     somewhere else would be proving a different claim, green or red.",
                    id = s.edge,
                    a = e.a(),
                    b = e.b(),
                    x = point[0],
                    y = point[1],
                    z = point[2],
                ),
                d,
            );
        }
    }
}

/// Is a world cell inside a place's play space?
fn contains_point(p: &Placed<'_>, at: [i64; 3]) -> bool {
    if at[0] < p.x0() || at[0] > p.x1() || at[2] < p.z0() || at[2] > p.z1() {
        return false;
    }
    let (lo, hi) = p.y_span();
    at[1] >= lo && at[1] <= hi
}

/// **The clause a stage-6 verdict owes when the allocation it measured against
/// is one this plan has already refused** — the DEFER shape of
/// [`crate::diagnostic`]'s "one cause, one line".
///
/// A `details[]` row is judged against a FRAME and a SEAM SET, and both are
/// computed from the site plan. When the plan's own refusals have already
/// touched them, the stage-6 line is a true, separate finding measured against a
/// number the map does not really have — and, worse, the primary is in another
/// document, so the reader has no way to see the relation.
///
/// So the stage-6 verdicts keep their own lines — each still names a real
/// mismatch, and suppressing them is how fixing one thing produces a fresh crop
/// of refusals nobody was shown — and gain a clause saying what they are
/// downstream of. What can be already-refused is a seam the plan DECLARES on
/// this place and did not resolve — a face the two boxes do not share
/// (`DW0828`) or a loop the packing could not close (`DW0883`) — so the allocated
/// seam set this place answers is short of what the author wrote.
///
/// Empty when the plan settled every seam, which is the ordinary case and costs one
/// pass over the plan's seams.
#[must_use]
pub fn refused_upstream(c: &Campaign, node: &NodeId, resolved: &[PlacedSeam]) -> String {
    let Some(plan) = c.site_plan.as_ref().map(|p| &p.content) else {
        return String::new();
    };
    let mut parts: Vec<String> = Vec::new();

    // A seam the plan wrote and did not resolve: the graph edge names this
    // place, the plan has a seam row for it, and nothing came out the other end.
    let graph_edges: BTreeMap<&str, &Edge> = c
        .layout_graph
        .as_ref()
        .map(|g| {
            g.content
                .edges
                .iter()
                .map(|e| (e.id().0.as_str(), e))
                .collect()
        })
        .unwrap_or_default();
    let unresolved: Vec<String> = plan
        .seams
        .iter()
        .filter(|s| {
            graph_edges
                .get(s.edge.0.as_str())
                .is_some_and(|e| e.a() == node || e.b() == node)
                && !resolved.iter().any(|r| r.edge == s.edge)
        })
        .map(|s| format!("`{}`", s.edge))
        .collect();
    if !unresolved.is_empty() {
        parts.push(format!(
            "the plan writes {n} seam(s) on this place that it does not resolve ({list}), which \
             `DW0828` or `DW0883` has already refused, so the allocation this piece is answering \
             is short of what the plan says",
            n = unresolved.len(),
            list = unresolved.join(", "),
        ));
    }

    if parts.is_empty() {
        return String::new();
    }
    format!(
        " This measurement stands downstream of a site-plan refusal: {parts}. Repair the plan \
         first — this line moves with it.",
        parts = parts.join("; "),
    )
}

/// The plan's one lighting setting, range-checked exactly as an area's is — the
/// same code, because it is the same object being asked the same question.
fn lighting(plan: &SitePlanContent, d: &mut Vec<Diagnostic>) {
    let Some(l) = &plan.lighting else { return };
    if (1..=14).contains(&l.min_light) {
        return;
    }
    d.push(Diagnostic::error(
        crate::codes::LIGHTING_RANGE,
        "site-plan",
        "/content/lighting/min_light",
        format!(
            "`lighting.min_light` = {} is out of range — set it to a value in 1..=14 (7 is the \
             default)",
            l.min_light
        ),
    ));
}
