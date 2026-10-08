#[derive(Subcommand)]
enum EditAction {
    /// Replay the edit script — plus an optional `--batch` candidate — and, on
    /// a fully green replay, persist the candidate into `world-edits.json`
    /// (canonical form). Without `--batch`, replays and re-renders only.
    Apply {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// A candidate batch (one stage-7 `EditBatch` JSON object) to append to
        /// the script. Persisted only if the whole replay is green.
        #[arg(long)]
        batch: Option<PathBuf>,
        /// Directory for the per-batch snapshot PNGs + manifests.
        #[arg(short, long, default_value = "edit-shots")]
        out: PathBuf,
    },
    /// Exactly `apply`, but never writes to the campaign directory — the
    /// candidate batch is replayed, checked and rendered only.
    Preview {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// A candidate batch (one stage-7 `EditBatch` JSON object) to append to
        /// the script for this replay only.
        #[arg(long)]
        batch: Option<PathBuf>,
        /// Directory for the per-batch snapshot PNGs + manifests.
        #[arg(short, long, default_value = "edit-shots")]
        out: PathBuf,
    },
}

/// `delvec edit apply|preview` (spec-0017): the edit → replay → snapshot loop.
///
/// Replays the stage-7 edit script — with an optional `--batch` candidate
/// appended — through full validation, the deterministic replay with its
/// per-batch invariant proofs, and one auto-rendered snapshot per batch
/// (framing the batch's edited region over the final edited world). `apply`
/// additionally persists the candidate into `world-edits.json` (canonical
/// form) once the whole replay is green; `preview` never writes to the
/// campaign directory. Editing sessions leave no state outside the script.
///
/// **One proof tier, not two** (map-editor audit finding 3). The per-batch
/// invariants are a *subset* of what `build` proves — they cover gravity,
/// relight, critical-path/checkpoint walkability and boundary safety, but not
/// cutscene clipping (`DW0308`), stealth zones (`DW0327`), trap completability
/// (`DW0342`), wave seating (`DW0312`), `move-npc`/`move-actor` routability, or
/// the exported-route self-check. `apply` used to persist on that subset, so a
/// script `build` rejects could be written into the campaign. Both verbs now run
/// the **whole** build-tier proof set (`analyze` + `emit::build`, output
/// discarded) before anything is persisted; measured cost is ~0.3 s on the
/// largest content campaign against a ~0.34 s snapshot render, so there is no
/// reason for a cheaper tier to exist.
fn run_edit(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    batch: Option<&Path>,
    out_dir: &Path,
    persist: bool,
    json: bool,
) -> ExitCode {
    use delvec::compiler::load::WORLD_EDITS_FILE;
    use delvec::compiler::snapshot::{self, Camera, DEFAULT_FOV, DEFAULT_HEIGHT, DEFAULT_WIDTH};

    let mut loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };

    // Append the candidate batch to the (possibly absent) stage-7 document, in
    // memory only — nothing touches the campaign dir unless the replay is green
    // AND this is `apply`.
    if let Some(bpath) = batch {
        let src = match std::fs::read_to_string(bpath) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "internal error: cannot read --batch {}: {e}",
                    bpath.display()
                );
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        let candidate: delvewright_dsl::EditBatch = match serde_json::from_str(&src) {
            Ok(b) => b,
            Err(e) => {
                print_build_error(
                    delvewright_dsl::codes::SCHEMA,
                    &format!(
                        "--batch {} is not a stage-7 `EditBatch` object: {e}. Run `delvec \
                         schema --stage 7` for the exact shape (the file holds ONE batch \
                         object, not a whole world-edits document)",
                        bpath.display()
                    ),
                    json,
                );
                return ExitCode::from(1);
            }
        };
        let mut env: delvewright_dsl::Envelope<delvewright_dsl::WorldEditsContent> =
            match &loaded.raw.world_edits {
                Some(s) => match serde_json::from_str(s) {
                    Ok(env) => env,
                    Err(e) => {
                        print_build_error(
                            delvewright_dsl::codes::SCHEMA,
                            &format!(
                                "existing world-edits.json does not parse: {e} — fix it before \
                                 appending batches"
                            ),
                            json,
                        );
                        return ExitCode::from(1);
                    }
                },
                None => {
                    let world: serde_json::Value =
                        serde_json::from_str(&loaded.raw.world).unwrap_or_default();
                    let cid = world
                        .get("campaign_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    delvewright_dsl::Envelope {
                        dsl_version: delvewright_dsl::DSL_VERSION.to_string(),
                        campaign_id: delvewright_dsl::CampaignId(cid),
                        stage: Stage::WorldEdits,
                        content: delvewright_dsl::WorldEditsContent {
                            batches: Vec::new(),
                        },
                    }
                }
            };
        env.content.batches.push(candidate);
        let script = match delvewright_dsl::to_canonical_string(&env) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("internal error: cannot serialize world-edits: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        loaded
            .inputs
            .insert(WORLD_EDITS_FILE.to_string(), script.clone().into_bytes());
        loaded.raw.world_edits = Some(script);
    }
    let augmented_script = loaded.raw.world_edits.clone();

    let mut v = match validate_loaded(loaded, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    // `edit` proves exactly what `build` proves, so it emits the same bodies: each
    // skin carries this delve's own texture id here too, before anything reads one.
    let skin_sources = delvewright_dsl::namespace_skin_textures(&mut v.campaign);
    let plan = match Plan::build(&v.campaign, &v.prefabs)
        .map(|p| p.with_design_files(v.loaded.design_files.clone()))
    {
        Ok(p) => p,
        Err(e) => {
            // Advisories raised before the failure and explaining it (`DW0498`:
            // the pool draw behind an ambiguous-anchor `DW0305`) print first —
            // the cause above the symptom.
            print_diags(&e.warnings, json);
            print_build_error(e.failure.code, &e.failure.message, json);
            return ExitCode::from(3);
        }
    };
    let structures = match read_structures(&plan, &v.prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let replay = match delvec::compiler::edit::replay(&plan, &v.prefabs, &structures) {
        Ok(r) => r,
        Err(e) => {
            print_build_error(e.code, &e.message, json);
            return ExitCode::from(e.code.exit_tier().exit_status());
        }
    };
    let Some(replay) = replay else {
        println!("no edit batches — nothing to replay (add one with `--batch <file>`)");
        return ExitCode::SUCCESS;
    };

    // One snapshot per batch: frame the batch's edited region over the FINAL
    // edited world (a dollhouse view pulled into open air, like `--at`).
    let grid = snapshot::VoxelGrid::build(&replay.assembled.blocks);
    snapshot::report_unpainted(&grid);
    let targets = snapshot::collect_targets(&plan);
    // The solved layout, hoisted out of the per-batch loop: it is a property of
    // the plan, identical in every batch's manifest.
    let pieces = snapshot::collect_pieces(&plan);
    let opts = snapshot::FrameOpts {
        width: DEFAULT_WIDTH,
        height: DEFAULT_HEIGHT,
        sea_level: sea_level_of(&v.campaign),
        labels: true,
    };
    let mut shots: Vec<(String, String)> = Vec::new(); // (batch id, png path)
    for b in &replay.batches {
        let Some((lo, hi)) = b.bounds else { continue };
        let centre = [
            (lo[0] + hi[0]) as f64 / 2.0,
            (lo[1] + hi[1]) as f64 / 2.0,
            (lo[2] + hi[2]) as f64 / 2.0,
        ];
        let span = ((hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2])) as f64;
        let d = (span * 1.1).max(12.0);
        let eye = [
            centre[0] + d * 0.75,
            centre[1] + d * 0.65,
            centre[2] + d * 0.75,
        ];
        let cam = Camera::looking_at(pull_into_open_air(&grid, centre, eye), centre, DEFAULT_FOV);

        let mut frame = snapshot::render_frame(&grid, &cam, &opts);
        let (inside, outside) = snapshot::resolve_targets(&grid, &cam, &opts, &targets);
        snapshot::draw_labels(&mut frame, &grid, &cam, &inside);
        let png = delvec::compiler::png::encode_rgba(
            frame.canvas.width,
            frame.canvas.height,
            &frame.canvas.rgba,
        );
        let name = b.id.strip_prefix("batch/").unwrap_or(&b.id);
        let image_name = format!("{name}.png");
        let png_path = out_dir.join(&image_name);
        let doc = snapshot::manifest(
            v.campaign.world.campaign_id.as_str(),
            &image_name,
            &cam,
            &opts,
            &grid,
            snapshot::Scene {
                pieces: &pieces,
                inside: &inside,
                outside: &outside,
            },
            &frame.canvas,
        );
        let mut manifest_bytes = match serde_json::to_vec_pretty(&doc) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("internal error: cannot serialize manifest: {e}");
                return ExitCode::from(EXIT_INTERNAL);
            }
        };
        manifest_bytes.push(b'\n');
        if let Err(e) = write_file(&png_path, &png)
            .and_then(|()| write_file(&manifest_path_for(&png_path), &manifest_bytes))
        {
            eprintln!("internal error: cannot write snapshot: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
        shots.push((b.id.clone(), png_path.display().to_string()));
    }

    // ---- the FULL build-tier proof set (map-editor audit finding 3) ----
    // Everything `delvec build` proves, over the same edited model: the DW02xx
    // reachability analysis, then `emit::build` (cutscene clip DW0308, stealth
    // zones DW0327, trap completability DW0342, wave seating DW0312, move
    // routability DW0307/DW0325, exported-route + POV self-checks
    // DW0314/DW0724, entry anchor DW0345, and the emitted-command validator).
    // Output is discarded — this run exists purely so `edit` can never accept a
    // script `build` would reject.
    let adiags = analyze_campaign(&v.campaign, &v.prefabs);
    if !adiags.is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }
    let tree = CommandTree::v1_21_11();
    let skins = match read_skins(campaign_dir, &v.campaign, &skin_sources, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };
    match emit::build_with_warnings(
        &plan,
        &v.loaded.inputs,
        &structures,
        &tree,
        &v.prefabs,
        None,
        &skins,
    ) {
        Ok((_, warnings)) => print_diags(&warnings, json),
        Err(emit::BuildFailure::Validation(errors)) => {
            eprintln!(
                "build failure: {} emitted command(s) failed validation:",
                errors.len()
            );
            for e in errors.iter().take(20) {
                eprintln!("  {}: {}", e.reason, e.line);
            }
            return ExitCode::from(3);
        }
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            print_build_error(code, &message, json);
            return ExitCode::from(code.exit_tier().exit_status());
        }
    }

    // Persist the accepted candidate — `apply` only, and only now that the full
    // build-tier proof set is green. Written tmp-then-rename: a crash or a full
    // disk mid-write must never leave the campaign's script truncated, and
    // `world-edits.json` is the artifact of record (ADR-0006).
    let persisted = persist && batch.is_some();
    if persisted && let Some(script) = &augmented_script {
        let final_path = campaign_dir.join(WORLD_EDITS_FILE);
        let tmp_path = campaign_dir.join(format!("{WORLD_EDITS_FILE}.tmp"));
        if let Err(e) =
            std::fs::write(&tmp_path, script).and_then(|()| std::fs::rename(&tmp_path, &final_path))
        {
            let _ = std::fs::remove_file(&tmp_path);
            eprintln!("internal error: cannot write {WORLD_EDITS_FILE}: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    }

    if json {
        println!(
            "{}",
            serde_json::json!({
                "batches": replay.batches.iter().map(|b| &b.id).collect::<Vec<_>>(),
                "commands": replay.commands.len(),
                "snapshots": shots.iter().map(|(id, p)| serde_json::json!({
                    "batch": id, "png": p,
                })).collect::<Vec<_>>(),
                "persisted": persisted,
            })
        );
    } else {
        for (id, path) in &shots {
            println!("{id} → {path}");
        }
        println!(
            "{} batch(es) replayed green, {} runtime command(s){}",
            replay.batches.len(),
            replay.commands.len(),
            if persisted {
                format!(
                    " — persisted to {}",
                    campaign_dir.join(WORLD_EDITS_FILE).display()
                )
            } else {
                String::new()
            }
        );
    }
    ExitCode::SUCCESS
}
