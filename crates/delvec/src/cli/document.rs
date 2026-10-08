/// `delvec fmt [--check] <path>…` — canonical form for authored Delvewright
/// JSON.
///
/// A formatter AND a check, deliberately in that order: a `--check`-only gate
/// makes authors hand-sort a 900-key sidecar, which nobody does twice, so the
/// gate ends up waived. `cargo fmt` is the shape that works.
///
/// Exit codes: `0` clean · `1` something is unformatted (`--check`), unparseable
/// (`DW0770`/`DW0771`), or matched nothing (`DW0774`) · `10` an I/O failure.
///
/// Every run states its binding count — how many files it examined. Zero is a
/// FINDING, not a pass (CLAUDE.md: a green gate that binds to nothing is
/// vacuous), because the way this gate dies quietly is a path that stops
/// matching after a directory is renamed.
fn run_fmt(paths: &[PathBuf], check: bool, json: bool) -> ExitCode {
    use delvewright_dsl::fmt;

    let mut files: Vec<PathBuf> = Vec::new();
    for root in paths {
        match fmt::discover(root) {
            Ok(found) => files.extend(found),
            Err(e) => {
                eprintln!("internal error: cannot read `{}`: {e}", root.display());
                return ExitCode::from(EXIT_INTERNAL);
            }
        }
    }
    files.sort();
    files.dedup();

    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut changed: Vec<PathBuf> = Vec::new();

    for path in &files {
        let shown = path.display().to_string();
        let original = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("internal error: cannot read `{shown}`: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        let formatted = match fmt::format_text(&original) {
            Ok(s) => s,
            Err(e) => {
                diags.push(Diagnostic::error(
                    e.code,
                    "fmt",
                    format!("{shown}:{}:{}", e.line, e.col),
                    e.message,
                ));
                continue;
            }
        };
        if formatted == original {
            continue;
        }
        changed.push(path.clone());
        if check {
            diags.push(Diagnostic::error(
                fmt::DW_FMT_UNFORMATTED,
                "fmt",
                shown.clone(),
                format!(
                    "not in canonical form (first difference at line {}). \
                     Run `delvec fmt {shown}`.",
                    first_differing_line(&original, &formatted)
                ),
            ));
        } else if let Err(e) = std::fs::write(path, &formatted) {
            eprintln!("internal error: cannot write `{shown}`: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    }

    for d in &diags {
        print_one_diag(d, json);
    }

    // Vacuity: a formatter that formatted nothing because it found nothing is
    // not a pass, and this is exactly how the CI gate would rot — a renamed
    // fixture directory, a path that no longer exists.
    if files.is_empty() {
        let d = Diagnostic::error(
            fmt::DW_FMT_NO_BINDING,
            "fmt",
            paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", "),
            "matched 0 JSON files. A formatter or a --check that binds to nothing is \
             vacuous, not a pass: check the paths (a `delvec build` output tree, a \
             dot-directory and a symlinked directory are all skipped deliberately)."
                .to_string(),
        );
        print_one_diag(&d, json);
        return ExitCode::from(1);
    }

    let unreadable = diags.len() - if check { changed.len() } else { 0 };
    if check {
        eprintln!(
            "delvec fmt --check: examined {} file(s); {} not in canonical form, {} unparseable",
            files.len(),
            changed.len(),
            unreadable
        );
    } else {
        eprintln!(
            "delvec fmt: examined {} file(s); reformatted {}, {} unparseable",
            files.len(),
            changed.len(),
            unreadable
        );
    }

    if diags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// 1-based line of the first difference, so `--check` points an author at a
/// place rather than at a file.
fn first_differing_line(a: &str, b: &str) -> usize {
    for (i, (la, lb)) in a.lines().zip(b.lines()).enumerate() {
        if la != lb {
            return i + 1;
        }
    }
    a.lines().count().min(b.lines().count()) + 1
}

/// `delvec allocation` — the handing (spec-0050 §4).
///
/// **Stdout carries the allocation and nothing else** on the success path, so an
/// authoring loop can redirect it. What it prints is an input to nothing — see
/// the note inside.
///
/// It takes no prefab directory, and that is the signature saying what the verb
/// is: an allocation is what the WHOLE hands a place, computed from the site plan
/// and the metrics table. Nothing about it depends on which pieces exist, which
/// is why it can be asked for before the piece is built — which is the only
/// moment it is any use.
fn run_allocation(campaign_dir: &Path, place: Option<&str>, all: bool, json: bool) -> ExitCode {
    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };
    // Parsed rather than fully validated, on the precedent `l10n-inventory`
    // sets: this verb's stdout is a machine-readable document an authoring loop
    // reads, and `print_diags` writes to stdout. Nothing is lost by it — an
    // allocation is derived from the plan on every invocation and is an input to
    // NOTHING, so a stale or wrong one has no vector into the build; the frame
    // is recomputed and re-judged by `DW0843` at every validation. `delvec
    // validate` is the verb that says what a campaign's state is.
    let campaign = match parse_campaign(&loaded.raw) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&diags, json);
            return ExitCode::from(1);
        }
    };
    if campaign.site_plan.is_none() {
        eprintln!(
            "error: `{}` carries no `site-plan.json`. An allocation is what the WHOLE hands a \
             place, so there is nothing to hand out until the whole exists.",
            campaign_dir.display()
        );
        return ExitCode::from(1);
    }
    let out = if all {
        serde_json::to_value(delvec::compiler::detail::allocations(&campaign))
    } else {
        let Some(place) = place else {
            eprintln!("error: name a place (`node/<kebab>`), or pass `--all`");
            return ExitCode::from(EXIT_INTERNAL);
        };
        let id = delvewright_dsl::NodeId(place.to_string());
        match delvec::compiler::detail::allocation(&campaign, &id) {
            Some(a) => serde_json::to_value(a),
            None => {
                eprintln!(
                    "error: the plan allocates no box to `{place}` — run with `--all` to see \
                     every place it does, or `delvec validate` if you expected one here"
                );
                return ExitCode::from(1);
            }
        }
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&out.expect("an allocation serializes")).unwrap()
    );
    ExitCode::SUCCESS
}

/// The `--stage` help: every document `delvec schema` answers to.
fn schema_stage_help() -> String {
    let names: Vec<String> = Stage::ALL
        .iter()
        .map(|s| format!("`{}`", s.name()))
        .collect();
    format!(
        "Which document: a campaign stage `1`..`7` or any stage by name ({}); `walk-record` \
         for the hand-written walk record (a campaign artifact, not a stage document); \
         `prefab-metadata` for a prefab library asset's sibling `<prefab-id>.json` (a \
         library asset, not a stage document); `cameras` for the showcase camera record \
         `design/cameras.json` (a campaign artifact, not a stage document); `sculpt-form` for \
         a `delvec sculpt` form (a library asset, not a stage document); or `all` for every \
         stage document at once.",
        names.join(", ")
    )
}

fn run_schema(stage: &str) -> ExitCode {
    // EVERY stage answers to its own name, the one `DW0100` prints when that
    // stage's document will not parse: a refusal that names `site-plan` and
    // then tells the author to run `--stage <1..7>` has sent them somewhere
    // their document is not. The names come from `Stage::ALL` rather than a
    // second hand-written list, so a stage added later answers here the day it
    // exists (`Stage::ALL`'s own doc comment says why).
    if let Some(s) = Stage::ALL.iter().find(|s| s.name() == stage) {
        println!(
            "{}",
            serde_json::to_string_pretty(&stage_schema(*s)).unwrap()
        );
        return ExitCode::SUCCESS;
    }
    let stages = match stage {
        // The numbered spelling of the campaign DSL's seven staged documents
        // (ADR-0002). The map-pipeline documents are NAMED and never numbered
        // into this sequence (spec-0049): that sequence is the campaign DSL's
        // staging and this is a different pipeline, so a number would assert an
        // ordering between the two that does not exist.
        "1" => vec![Stage::World],
        "2" => vec![Stage::Npcs],
        "3" => vec![Stage::Classes],
        "4" => vec![Stage::QuestPlan],
        "5" => vec![Stage::Quests],
        "6" => vec![Stage::Dialogue],
        "7" => vec![Stage::WorldEdits],
        // `walk-record.json` is not a stage document and has no `Stage` — it is
        // a campaign artifact recording an event (see `walk::walk_record_schema`).
        // It is reachable here anyway because this is the command an author is
        // told to run to see the shape of a document they must write, and the
        // walk record is one of those. The schema says what it is, so the tool
        // and the reference document agree rather than the flag's name deciding.
        "walk-record" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&delvec::compiler::walk::walk_record_schema())
                    .unwrap()
            );
            return ExitCode::SUCCESS;
        }
        // `<prefab-id>.json` is not a stage document either — it is a library
        // ASSET's metadata, reachable here for the same reason the walk record
        // is. Deliberately absent from `all`: the gallery's coverage gate
        // enumerates its units from that export, and a library-asset document
        // folded into it would demand a stage-document binding for every field
        // of a file no stage document contains (`PrefabMeta::schema`'s note).
        "prefab-metadata" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&delvewright_dsl::prefab::PrefabMeta::schema())
                    .unwrap()
            );
            return ExitCode::SUCCESS;
        }
        // `design/cameras.json` is not a stage document either — it is the showcase
        // camera record (spec-0069), a campaign artifact `delvec place-camera`
        // writes. Exported for the reason the walk record is, and absent from
        // `all` for the reason prefab metadata is; the export names the file it
        // lives at, which is how the gallery's coverage gate finds the document
        // it binds the record's units against (spec-0079 §7).
        "cameras" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&delvec::compiler::view::camera::record_schema())
                    .unwrap()
            );
            return ExitCode::SUCCESS;
        }
        // A sculpt form is not a stage document either — it is a library ASSET
        // `delvec sculpt` reads (spec-0087), exported for the reason prefab
        // metadata is and absent from `all` for the same reason. The export
        // names where forms live, which is how the gallery's coverage gate finds
        // the documents it binds the form's units against.
        "sculpt-form" => {
            println!(
                "{}",
                serde_json::to_string_pretty(&delvec::sculpt::form::schema()).unwrap()
            );
            return ExitCode::SUCCESS;
        }
        "all" => Stage::ALL.to_vec(),
        other => {
            let names: Vec<String> = Stage::ALL
                .iter()
                .map(|s| format!("`{}`", s.name()))
                .collect();
            eprintln!(
                "unknown document `{other}`. Want `1`..`7` (the campaign DSL's numbered \
                 stages), any stage by name — {names} — `walk-record` for the hand-written \
                 walk record, `prefab-metadata` for a prefab library asset's sibling \
                 `<prefab-id>.json`, `cameras` for the showcase camera record \
                 `design/cameras.json`, `sculpt-form` for a `delvec sculpt` form, or `all` for \
                 every stage document at once.",
                names = names.join(", "),
            );
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    if stages.len() == 1 {
        let schema = stage_schema(stages[0]);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    } else {
        let mut map = serde_json::Map::new();
        for s in stages {
            map.insert(s.name().to_string(), stage_schema(s));
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap()
        );
    }
    ExitCode::SUCCESS
}

/// Export the metrics standard (spec-0049 §2 — pipeline stage 0).
///
/// The table on stdout, so a tool outside the engine reads the JSON and never a
/// copy; the verdicts on stderr, so a shell pipeline gets clean JSON.
///
/// Three things are stated on stderr every run, and each is stated whether or
/// not it found anything, because a count only means something when the run that
/// found nothing prints it too:
///
/// 1. **What the table holds** — entries per half, and how many building entries
///    the metrics gym has not walked.
/// 2. **What the self-check bound to** — invariants evaluated and building
///    entries read. A run that evaluated zero invariants would be a vacuous pass
///    and is refused as an internal error, not reported as green.
/// 3. **`DW0813`**, when any verdict above rested on an uncalibrated standard.
///    This is the code's live binding at this version: no *document* reads a
///    building metric until the layout-graph and site-plan stages land, and the
///    table proving itself consistent is a real verdict resting on real seeds.
///
/// An inconsistent table exits `EXIT_INTERNAL` rather than raising a diagnostic.
/// A diagnostic is addressed to an author, and there is no author here — the
/// table is engine data, so a table that contradicts itself is a defect in
/// `dsl::metrics` and the person who has to act on it is whoever is holding the
/// compiler.
/// `delvec codes`: the DW-code registry, one JSON object per line, sorted by
/// code; the count on stderr.
fn run_codes() -> ExitCode {
    let all = delvewright_dsl::diagnostic::declared();
    let mut out = String::new();
    for entry in &all {
        out.push_str(&serde_json::to_string(entry).expect("a registry entry serializes"));
        out.push('\n');
    }
    print!("{out}");
    eprintln!("codes: {} DW code(s) declared by this binary", all.len());
    ExitCode::SUCCESS
}
