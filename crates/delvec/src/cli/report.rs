/// Write `bytes` to `path`, creating parent directories.
fn write_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

pub(crate) fn print_one_diag(d: &Diagnostic, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string(d).expect("diagnostic serializes")
        );
    } else {
        println!("{} [error] {} {}: {}", d.code, d.stage, d.path, d.message);
    }
}

/// Print a `DW03xx` build/solver diagnostic (exit 3), honoring `--json`. Mirrors
/// the spec-0002 one-object-per-line JSON shape used for validation diagnostics.
pub(crate) fn print_build_error(code: DwCode, message: &str, json: bool) {
    if json {
        let d = serde_json::json!({
            "code": code,
            "severity": "error",
            "stage": "build",
            "path": "",
            "message": message,
        });
        println!("{d}");
    } else {
        eprintln!("{code} [error] build: {message}");
    }
}

/// State the **binding count** of the layout-graph checks on stderr (spec-0049
/// §3.3: *every check states its binding count*).
///
/// Printed on every validate, analyze and build of a campaign that carries
/// either map-pipeline document, and printed whether or not anything was found —
/// a count only means something when the run that found nothing prints it too.
/// A campaign with neither document prints nothing at all, which is a different
/// fact from a graph that bound to zero of everything and is the reason the two
/// are distinguishable here rather than collapsed into one silence.
///
/// A **zero on a graph that exists is a finding**, and the two zeroes that can
/// occur are named rather than counted: a graph with no beats is the *graph
/// before mission* case (`DW0817` says so in its own line, at analysis tier),
/// and a graph with no traversal edges is a set of places with no space between
/// them, which no later check can catch because every one of them quantifies
/// over edges.
///
/// stderr, not stdout: `--json` reserves stdout for one diagnostic object per
/// line, and this is not a diagnostic — nothing here is wrong.
/// **The run's binding counts, under one heading, after the author's lines.**
///
/// Every count a check owes its reader is still stated, zeroes included — that
/// is the vacuity rule and nothing here relaxes it. What changed is where they
/// sit: computed as each pass ran, they printed before the first refusal, so a
/// run that refused one thing opened with four to six paragraphs about what it
/// had examined. They are the last thing a run says now, and they say it under a
/// heading so a reader can see where the answers to their own question ended.
fn report_binding_notes(campaign: &delvewright_dsl::Campaign, collected: &[String]) {
    let mut buf: Vec<String> = collected.to_vec();
    layout_binding_lines(campaign, &mut buf);
    if buf.is_empty() {
        return;
    }
    eprintln!("-- what this run examined ({} line(s))", buf.len());
    for line in &buf {
        eprintln!("{line}");
    }
}

/// The layout, plan and brief binding counts, appended rather than printed —
/// [`report_binding_notes`] is the one site that emits them, so there is one
/// place that decides when a reader sees them.
fn layout_binding_lines(campaign: &delvewright_dsl::Campaign, out: &mut Vec<String>) {
    if campaign.layout_graph.is_none()
        && campaign.geometry_brief.is_none()
        && campaign.site_plan.is_none()
    {
        return;
    }
    let b = delvewright_dsl::LayoutBinding::of(campaign);
    out.push(b.line());
    if campaign.site_plan.is_some() {
        out.push(b.plan_line());
        // The derivation, handed back: every box's corner and how it was
        // obtained, so a creator reads a corner here rather than typing one.
        out.extend(delvewright_dsl::placements(campaign));
        if b.plan.views == 0 {
            out.push(
                "site-plan binding 0: this plan names no view, so the walk has no declared \
                 vantage to judge the silhouette from and the render beside the reference sheet \
                 has nothing to frame. The plan still builds; what is missing is the picture the \
                 whole was supposed to be looked at in."
                    .to_string(),
            );
        }
        if b.plan.volumes == 0 {
            out.push(
                "site-plan binding 0: this plan declares no whole-owned volume, so the check \
                 that keeps the whole's mass out of the places examined nothing. A map made \
                 only of rooms is a legitimate map; a map with a mountain in it that forgot to \
                 say so is not, and nothing else would notice."
                    .to_string(),
            );
        }
    }
    if campaign.layout_graph.is_some() && b.traversal_edges == 0 {
        out.push(
            "layout-graph binding 0: this graph declares no traversal connection at all, so \
             every check over edges above examined nothing. A set of places with no space \
             between them is a finding, not a graph that happens to be simple."
                .to_string(),
        );
    }
    if campaign.layout_graph.is_some() && b.spine_beats == 0 {
        out.push(
            "layout-graph binding 0: no beat of this graph belongs to a quest the finale depends \
             on, so `DW0817`'s obligation to visit the mission on the way to the goal examined \
             nothing. A critical path over an unbound graph is a route through nothing."
                .to_string(),
        );
    }
    if campaign.geometry_brief.is_some() && b.brief_facts == 0 {
        out.push(
            "geometry-brief binding 0: this brief states no fact, so there is nothing for a \
             site plan's identities to bind the map to."
                .to_string(),
        );
    }
}

/// Print a diagnostic list, honoring `--json`. **Author-actionable first.**
///
/// A run's diagnostics reached the terminal in the order the passes happened to
/// produce them, which put four to six paragraphs of advisory ahead of the one
/// line the author was there to act on: on every site-plan run, `DW0813` (the
/// engine's own metric table is provisional), `DW0822` (a pacing figure that
/// "carries NO threshold and refuses nothing") and the per-run binding lines all
/// printed before the first refusal.
///
/// So the list is grouped before it is printed — refusals, then advisories about
/// this campaign, then notices about this engine — by
/// [`delvewright_dsl::Diagnostic::group`], the one authority on that order. The
/// sort is STABLE, so within a group nothing moves and every pass's own ordering
/// survives; nothing is dropped, and the grouping applies to `--json` too,
/// because a consumer reading the first line should get the actionable one for
/// the same reason a person should.
///
/// Headings are human-output only: `--json` is one JSON object per line and
/// stays that way (spec-0002).
pub(crate) fn print_diags(diags: &[Diagnostic], json: bool) {
    let mut ordered: Vec<&delvewright_dsl::Diagnostic> = diags.iter().collect();
    ordered.sort_by_key(|d| d.group());

    let mut heading: Option<delvewright_dsl::Group> = None;
    for d in &ordered {
        if json {
            println!("{}", serde_json::to_string(d).unwrap());
            continue;
        }
        let g = d.group();
        if heading != Some(g) {
            let n = ordered.iter().filter(|x| x.group() == g).count();
            println!("{}", g.heading(n));
            heading = Some(g);
        }
        let sev = match d.severity {
            delvewright_dsl::Severity::Error => "error",
            delvewright_dsl::Severity::Warning => "warning",
        };
        println!(
            "{} [{sev}] {}{}: {}",
            d.code,
            d.stage,
            if d.path.is_empty() {
                String::new()
            } else {
                format!(" {}", d.path)
            },
            d.message
        );
    }
}
