//! `delvec validate`, `analyze`, `build` and `textures`: the subcommands that
//! act on a whole campaign, and the one loader and validation funnel every
//! subcommand that reads a campaign goes through.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::ExitCode;

use delvec::compiler::analyze::analyze_campaign;
use delvec::compiler::blockout::{Knob, Perturb};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit;
use delvec::compiler::load::{
    LoadedCampaign, load_campaign_dir, missing_stage_documents_diagnostic,
};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Diagnostic, DwCode, validate_campaign_with};

use crate::EXIT_INTERNAL;
use crate::cli::report::{print_build_error, print_diags, report_binding_notes};

/// `DW0309`: a staged **body** — a stage-2 npc or a stage-5 actor alike —
/// declares a `skin.texture_id` for which the campaign ships no
/// `skins/<texture_id>.png`. Build-tier (exit 3). One rule with a
/// `world.textures[]` row's missing file (spec-0084 §6.4), so it is declared
/// once, beside that half.
const DW_SKIN_PNG_MISSING: DwCode = delvec::compiler::textures::DW_IMAGE_MISSING;

/// The parsed campaign plus everything the CLI commands share: prefab metadata,
/// the loaded campaign directory (stage bytes + l10n sidecars), the parsed l10n
/// sidecars, and the accumulated diagnostics (schema + referential + l10n).
pub(crate) struct Validated {
    pub(crate) campaign: delvewright_dsl::Campaign,
    pub(crate) prefabs: PrefabRegistry,
    pub(crate) loaded: delvec::compiler::load::LoadedCampaign,
    pub(crate) sidecars: BTreeMap<String, delvewright_dsl::L10nDoc>,
    /// The accumulated diagnostics — the list a verdict is read off.
    pub(crate) diags: Vec<Diagnostic>,
}

/// Parse an `l10n/<code>.json` sidecar map (raw bytes) into typed [`L10nDoc`]s.
/// A malformed sidecar yields a `DW0180` diagnostic and is dropped (so coverage
/// then reports it as a missing sidecar for its declared language).
fn parse_sidecars(
    raw: &BTreeMap<String, Vec<u8>>,
    diags: &mut Vec<Diagnostic>,
) -> BTreeMap<String, delvewright_dsl::L10nDoc> {
    let mut out = BTreeMap::new();
    for (code, bytes) in raw {
        match serde_json::from_slice::<delvewright_dsl::L10nDoc>(bytes) {
            Ok(doc) => {
                out.insert(code.clone(), doc);
            }
            Err(e) => diags.push(Diagnostic::error(
                delvewright_dsl::codes::L10N_MISSING,
                "l10n",
                format!("l10n/{code}.json"),
                format!("malformed l10n sidecar: {e}"),
            )),
        }
    }
    out
}

/// **Read a campaign directory, or say which of the two things went wrong.**
///
/// The four verbs that read a campaign directory (`validate` and everything
/// built on it, `l10n-inventory`, `edit`, `allocation`) each carried their own
/// copy of one error arm, and every copy said the same thing:
/// `internal error: cannot read campaign dir: <name>`, exit 10, no `DW` code.
/// That answer is right for exactly one of the two states it covered. A campaign
/// directory holding a document this process cannot open IS an internal
/// condition worth stopping hard on; a campaign directory that does not hold all
/// six stage documents yet is an author part-way through writing one, which is
/// the state the authoring skill puts them in on purpose.
///
/// So the two are told apart here, once, and the authoring state gets an ordinary
/// coded refusal ([`DW_STAGE_DOCUMENT_MISSING`]) at the validation tier. Nothing
/// else moves: an unreadable document, a bad encoding, a path that is not a
/// directory each print exactly what they printed, at exit 10.
///
/// The order matters and is deliberate. The missing-document question is asked
/// only **after** the load has failed, so a directory that loads is never
/// probed and no verb pays for a check on its success path.
pub(crate) fn load_or_refuse(campaign_dir: &Path, json: bool) -> Result<LoadedCampaign, u8> {
    match load_campaign_dir(campaign_dir) {
        Ok(l) => Ok(l),
        Err(e) => match missing_stage_documents_diagnostic(campaign_dir) {
            Some(d) => {
                print_diags(&[d], json);
                Err(1)
            }
            None => {
                eprintln!("internal error: cannot read campaign dir: {e}");
                Err(EXIT_INTERNAL)
            }
        },
    }
}

/// `delvec textures` (spec-0084 §5.2): one comparison sheet per declared texture.
pub(crate) fn run_textures(
    campaign_dir: &Path,
    out: &Path,
    textures: Option<&str>,
    json: bool,
) -> ExitCode {
    use delvec::compiler::textures;
    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(code) => return ExitCode::from(code),
    };
    let campaign = match delvec::compiler::load::parse_loaded(&loaded) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&diags, json);
            return ExitCode::from(1);
        }
    };
    let (rows, findings) =
        textures::resolve(&campaign, |p| loaded.textures.get(p).map(Vec::as_slice));
    if !findings.is_empty() {
        let diags: Vec<Diagnostic> = findings.iter().map(|f| f.diagnostic()).collect();
        print_diags(&diags, json);
        return ExitCode::from(1);
    }
    let jar = match delvec::compiler::view::cli::resolve_textures(textures) {
        Ok(j) => j,
        Err(d) => {
            d.print(json);
            return ExitCode::from(5);
        }
    };
    let assets = match delvec::compiler::view::assets::Assets::open(Path::new(&jar)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("textures: cannot open {jar}: {e}");
            return ExitCode::from(5);
        }
    };
    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("textures: mkdir {}: {e}", out.display());
        return ExitCode::from(EXIT_INTERNAL);
    }
    for r in &rows {
        let vanilla_path = format!("assets/minecraft/textures/{}.png", r.path);
        let Some(vanilla) = assets.read(&vanilla_path) else {
            eprintln!(
                "textures: {jar} holds no `{vanilla_path}` — it is not the pinned client jar \
                 the census was derived from"
            );
            return ExitCode::from(5);
        };
        // The left half is vanilla's only if these are the bytes the census
        // measured — the same digest an override equal to vanilla is refused by.
        let got = {
            use sha2::{Digest, Sha256};
            Sha256::digest(&vanilla)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        if got != r.vanilla.sha256 {
            eprintln!(
                "textures: `{vanilla_path}` in {jar} has sha256 {got}, and the census records \
                 {} — the jar is not the pinned {} client",
                r.vanilla.sha256,
                textures::census().minecraft
            );
            return ExitCode::from(5);
        }
        let Some(sheet) = textures::sheet(&vanilla, &r.png, r.vanilla.frame(), r.k) else {
            eprintln!("textures: `{}` did not decode for its sheet", r.id);
            return ExitCode::from(EXIT_INTERNAL);
        };
        let dest = out.join(format!("{}.png", r.id));
        if let Err(e) = std::fs::write(&dest, &sheet) {
            eprintln!("textures: write {}: {e}", dest.display());
            return ExitCode::from(EXIT_INTERNAL);
        }
        println!("{} — `{}` replaces `{}`", dest.display(), r.id, r.replaces);
    }
    eprintln!(
        "textures binding: {} sheet(s) written of {} row(s) declared, from {jar}",
        rows.len(),
        campaign.world.content.textures.len()
    );
    ExitCode::SUCCESS
}

/// Validate and return the parsed campaign + shared context (prefabs, loaded dir,
/// l10n sidecars) + diagnostics; prints diagnostics. Returns `Err(exit)` on
/// internal error.
pub(crate) fn validate_stage(
    campaign_dir: &Path,
    prefabs_dir: &Path,
    json: bool,
) -> Result<Validated, u8> {
    let loaded = load_or_refuse(campaign_dir, json)?;
    validate_loaded(loaded, prefabs_dir, json)
}

/// [`validate_stage`] over an already-loaded (possibly augmented) campaign —
/// split out so `delvec edit` can validate a script with a candidate batch
/// appended before anything touches the campaign directory.
pub(super) fn validate_loaded(
    loaded: delvec::compiler::load::LoadedCampaign,
    prefabs_dir: &Path,
    json: bool,
) -> Result<Validated, u8> {
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).map_err(|e| {
        eprintln!(
            "internal error: cannot read prefabs dir {}: {e}",
            prefabs_dir.display()
        );
        EXIT_INTERNAL
    })?;
    let items = FullItemRegistry::v1_21_11();
    // v0.3 wave-mob entity validation against the full 1.21.11 entity registry
    // (157 ids, same misode/mcmeta provenance as the item registry).
    let entities = FullEntityRegistry::v1_21_11();

    match delvec::compiler::load::parse_loaded(&loaded) {
        Ok(campaign) => {
            let mut diags = validate_campaign_with(&campaign, &items, &prefabs, &entities);
            // **What this run examined, held back until the author's lines are
            // out.** These counts are not optional — a check that reports no
            // binding is one nobody can tell apart from a check that never ran,
            // and `CLAUDE.md`'s vacuity rule is why every one of them is stated
            // whether or not it is zero. What they are not is the first thing an
            // author needs, and printed as they were computed they arrived ahead
            // of the refusal. So they are collected here and emitted after
            // `print_diags`, under a heading, unchanged.
            let mut examined: Vec<String> = vec![
                // spec-0067: what the equipment fit rule (`DW0898`, raised inside
                // `validate_campaign_with` above) examined, zeroes included.
                delvewright_dsl::EquipmentBinding::of(&campaign, &items).line(),
                // spec-0071 §2: what the purchase rule (`DW0901`) examines — the
                // charges it counts, the `(list, datum)` pairs they bind, and the
                // effect lists walked as the denominator.
                delvewright_dsl::PurchaseBinding::of(&campaign).line(),
                // `DW0527`: bundles and effects the read-after-write rule walked.
                delvewright_dsl::ReadAfterWriteBinding::of(&campaign).line(),
                // spec-0073: what the health-bar rules (`DW0909`/`DW0910`/`DW0912`,
                // raised inside `validate_campaign_with` above) examined — fights
                // carrying a bar over fights declared, zeroes included.
                delvewright_dsl::HealthBarBinding::of(&campaign).line(),
                // spec-0074: what the `on_kill` rules examine — fights carrying a
                // bundle over fights declared, and how many of them come back.
                delvec::compiler::onkill::OnKillBinding::of(&campaign).line(),
            ];
            // Prefab-library load failures (DW0346): a metadata file that did
            // not parse (e.g. newer schema than this delvec) is a first-class
            // validation diagnostic, never a silent skip that resurfaces later
            // as a baffling DW0300 "prefab not found".
            diags.extend(prefabs.load_diagnostics().iter().cloned());
            // i18n l10n sidecar coverage (DW0180/DW0181) — language-independent,
            // runs on every validate/analyze/build. No-op for English-only campaigns.
            let sidecars = parse_sidecars(&loaded.l10n, &mut diags);
            diags.extend(delvewright_dsl::validate_l10n(&campaign, &sidecars));
            // The machine completion-marker channel is reserved (DW0182): no
            // authored or translated player-visible line may carry `[dw:complete`,
            // the bot's completion oracle. Runs for English-only campaigns too.
            diags.extend(delvewright_dsl::validate_marker_channel(
                &campaign, &sidecars,
            ));
            // The private-use block the i18n v2 translation tag is built from is
            // reserved too (DW0183, spec-0029): a string carrying U+E000..U+F8FF
            // could impersonate the key the compiler threads into a text
            // component — and has no glyph in any Minecraft font anyway.
            diags.extend(delvewright_dsl::validate_tr_sigil(&campaign, &sidecars));
            // spec-0096: inline style markup in every player-facing line — the
            // English and every sidecar row parse (DW0975), and a translation
            // carries exactly the English's styled spans (DW0976).
            diags.extend(delvewright_dsl::textstyle::validate_inline_styles(
                &campaign, &sidecars,
            ));
            examined.push(
                delvewright_dsl::textstyle::InlineStyleBinding::of(&campaign, &sidecars).line(),
            );
            // The compiler's own chrome namespace is reserved as well (DW0186):
            // `delvewright.*` keys are the engine's on-screen strings, shipped
            // translated with the compiler, and a sidecar row under that prefix
            // would be written into the language file and replace product chrome.
            diags.extend(delvewright_dsl::validate_chrome_namespace(&sidecars));
            // Translation provenance (DW0187/DW0188): coverage proves the sidecar's
            // key SET matches the inventory, which is silent about whether a row
            // still translates the English it renders. `source` records what each
            // row was translated FROM, so a rewritten line is detected rather than
            // audited; rows with no provenance are counted, never passed over.
            diags.extend(delvewright_dsl::validate_l10n_provenance(
                &campaign, &sidecars,
            ));
            // i18n v2: every declared language must map to a Minecraft language-file
            // code, or its `assets/delvewright/lang/<code>.json` has no name a client
            // would ask for and the language ships invisible (DW0184).
            if let Err(d) = delvewright_dsl::declared_mc_codes(&campaign) {
                diags.push(d);
            }
            // spec-0084: every `world.textures[]` row against the pinned client's
            // census (DW0939) and its own file (DW0309, DW0940). The same
            // resolution the build bakes the pack from, so a row validate admits
            // is a row the pack carries, byte for byte.
            let (texture_rows, texture_diags) =
                delvec::compiler::textures::resolve(&campaign, |p| {
                    loaded.textures.get(p).map(Vec::as_slice)
                });
            diags.extend(texture_diags.iter().map(|f| f.diagnostic()));
            examined.push(format!(
                "textures: {} row(s) resolved of {} declared, against a census of {} vanilla \
                 texture(s)",
                texture_rows.len(),
                campaign.world.content.textures.len(),
                delvec::compiler::textures::census().textures.len()
            ));
            // spec-0097: every sheet a model's boxes can judge — the mannequin
            // skins the bodies wear, and the texture rows an entity model is
            // drawn with — and the one line saying how many were judged.
            let mut sheets = delvec::compiler::skinparts::Binding::default();
            diags.extend(delvec::compiler::skinparts::check_skins(
                &campaign,
                &loaded.skins,
                &mut sheets,
            ));
            delvec::compiler::skinparts::count_texture_rows(
                &campaign,
                &texture_rows,
                &texture_diags,
                &mut sheets,
            );
            examined.push(sheets.line());
            // v0.6 sound + art-title surface (spec-0014): sound-event ids
            // (DW0326), the unsupported `play-sound at: actor` gate (DW0335), and
            // art-title glyph coverage against the `delve:art` font over the source
            // text and every declared-language sidecar (DW0328). Validation-tier
            // (exit 1) — no-op for a campaign that uses neither surface.
            diags.extend(delvec::compiler::atmos::check_sounds(&campaign));
            diags.extend(delvec::compiler::atmos::check_art(&campaign, &sidecars));
            // spec-0080: a campaign's atmospheres and its repaints, refused at
            // the document they are written in — an attribute the pinned game
            // does not accept here (DW0928), a repaint naming neither or both of
            // its volumes (DW0929), an atmosphere declared against itself or
            // against nothing (DW0930). No-op for a campaign that declares none.
            diags.extend(delvec::compiler::atmosphere::check(&campaign));
            // On-screen narrate text that overruns the title/subtitle/art width
            // budget (DW0330). Advisory tier — see `textfit` for why this warns
            // rather than rejects. Runs over the English source and every
            // declared-language sidecar rendition.
            diags.extend(delvec::compiler::textfit::check_text_fits(
                &campaign, &sidecars,
            ));
            // Dialogue option labels that overrun their dialog button (DW0331).
            // Error tier, unlike DW0330: the 150-GUI-px button is the geometry of
            // the dialog this compiler emits, not a guess about the player's
            // window, so an over-wide caption provably scrolls in game. Runs over
            // the English source and every declared-language sidecar rendition.
            diags.extend(delvec::compiler::textfit::check_option_labels(
                &campaign, &sidecars,
            ));
            // A dialogue option that asks a question leads to a line that can
            // answer it (DW0981), in the English source and every declared
            // sidecar. A document fact, refused where the option is written.
            diags.extend(delvec::compiler::telling::check_questions(
                &campaign, &sidecars,
            ));
            // A name tag marks a person: a name a crowd wears is refused on
            // every body that wears it (DW0983), with what it examined.
            {
                let (td, tbind) = delvec::compiler::telling::check_name_tags(&campaign);
                examined.push(tbind.line());
                diags.extend(td);
            }
            // v0.6 `close-gate` gate-block declaration (DW0343): the fill block is
            // prefab metadata, so this compiler-side check runs here (validation
            // tier). No-op for a campaign that uses no `close-gate`.
            diags.extend(delvec::compiler::gates::check_close_gates(
                &campaign, &prefabs,
            ));
            // spec-0083 §3.5: a `teleport` that fires while its root's cutscene
            // is still playing is undone by `cs_end` (DW0933); and §7: the
            // layout graph's `carry` edges and the campaign's links agree
            // (DW0934). Both are read off the documents alone, so they are
            // refused at validation, where the fault is entered.
            diags.extend(delvec::compiler::link::check_teleport_under_cutscene(
                &campaign,
            ));
            diags.extend(delvec::compiler::link::check_carry_realised(&campaign));
            // v0.8 seal answers (DW0423): one gate anchor, one `sealed_hint`
            // wording. No-op for a campaign that authors none.
            diags.extend(delvec::compiler::gates::check_seal_hints(&campaign));
            // spec-0073 §8.3 (DW0911): a health bar's colour and style are the
            // literals the pinned command tree lists, and that tree is this
            // crate's data, so the check runs here (validation tier). No-op for a
            // campaign that declares no bar.
            diags.extend(delvec::compiler::healthbar::check_vocabulary(
                &campaign,
                &CommandTree::v1_21_11(),
            ));
            // spec-0074 §8.2/§8.3: `on_kill.fires` is owed where a fight comes back
            // (DW0915) and inert where it does not (DW0914). Compiler-side because
            // "comes back" reads the rest points; validation tier. No-op for a
            // campaign that declares no `on_kill`.
            diags.extend(delvec::compiler::onkill::check_on_kill_fires(&campaign));
            // NPC location-continuity lint (DW0351). Advisory tier — a warning
            // names a staging discontinuity (an NPC materializing or vanishing
            // away from where it was last staged) but never fails the run:
            // narrative cover is a legitimate authorial answer.
            diags.extend(delvec::compiler::continuity::check_npc_continuity(
                &campaign,
            ));
            // The NPC scene ledger (DW0460–DW0467, spec-0020): every quest must
            // say where each live NPC is, what they are doing, and what their
            // right-click offers, and the declaration is checked against the
            // effect history. Error tier, except the staleness lint (DW0467),
            // which warns.
            diags.extend(delvec::compiler::cast::check_cast(&campaign));
            // `DW0884`: a cast row whose anchor name BOTH the beat's area and
            // the npc's own area answer to. The same finding `DW0461`'s place
            // arm refuses at the build tier from the seated pieces, refused here
            // — where the row is entered — before a cell is ever computed. Needs
            // the prefab registry (which areas answer to a name is a fact about
            // the pieces they bind), which is why it sits beside `check_cast`
            // rather than inside it. No-op for a campaign whose beats and bodies
            // share an area, which is every single-area campaign.
            diags.extend(delvec::compiler::cast::check_shared_cast_anchor(
                &campaign, &prefabs,
            ));
            // **`DW0886` / `DW0887`: a horizon and a piece set are a pair**
            // (spec-0060). Refused here rather than at the build, on `DW0855`'s
            // own precedent: the verdict is a fact about the documents and the
            // library — the declared base, the pools the world names, each
            // member's metadata and its bytes — so nothing has to be placed to
            // know it, and a creator should not spend a build to learn that the
            // pieces they chose cannot stand where they put them. It opens the
            // `.nbt`, because a verdict from declarations alone reports a
            // library of fictions as seatable. The binding line states what it
            // examined, zeroes included.
            {
                let (bind, sd) = delvec::compiler::seating::check(&campaign, &prefabs, prefabs_dir);
                examined.push(bind.line());
                diags.extend(sd);
            }
            // **`DW0889`: which anchors does this area guarantee** — asked here
            // because this is the last step that can still answer it in time. An
            // area binding a pool seats a SUBSET of that pool's members (the
            // `entry` piece, one carrier per anchor the campaign requires, and
            // `connector` fillers drawn from the seed), so an anchor declared on
            // a member nothing forces may simply not be in the built world.
            // Every diagnostic that said so needed a build, and the anchors are
            // chosen three authoring steps earlier. The whole verdict is a fact
            // about declarations — the pool's members, their roles, their
            // `anchors` maps — so nothing has to be placed to know it, on
            // `compiler::seating`'s own precedent. Advisory: the filler draw may
            // well seat the piece, and `DW0302`/`DW0360` still refuse at the
            // build if it did not. The binding line states the guarantee with
            // its denominator, zeroes included.
            {
                let (gbind, gd) = delvec::compiler::guarantee::check(&campaign, &prefabs);
                examined.push(gbind.line());
                diags.extend(gd);
            }
            // An objective keeps the promise its prompt makes (DW0860-DW0863):
            // a failure clock armed before its own prompt could be read, an
            // adopted container nothing distinguishes from the scenery beside
            // it, a hint the emitter will never show, and a fight the party is
            // given no way to find. Error tier throughout - each judges what the
            // document says. The
            // binding line states what it examined, including the zeroes.
            {
                let (pd, pbind) = delvec::compiler::promise::check(&campaign);
                examined.push(pbind.line());
                diags.extend(pd);
            }
            // spec-0050 (DSL v0.15): the detail plan. `DW0842`-`DW0845` (the
            // binding binds, the piece is the shape of its allocation, its
            // openings are the plan's seams, its anchors have standing). Bound HERE because this is the one
            // funnel every subcommand's validation goes through — `build`
            // included — so a defect cannot reach a datapack by skipping
            // `delvec validate`. No-op for a campaign with no `detail-plan`, and
            // the binding line states that zero rather than going quiet.
            {
                let (dd, dbind) = delvec::compiler::detail::check(&campaign, &prefabs);
                if campaign.detail_plan.is_some() || campaign.site_plan.is_some() {
                    examined.push(dbind.line());
                }
                diags.extend(dd);
                // spec-0098 §7: a bound piece writes no cell it does not own
                // (`DW0987`) and no fixed ring ground (`DW0990`), read off its
                // own `.nbt`, where `DW0888` already opens it.
                let (vd, vbind) =
                    delvec::compiler::detail::check_voids(&campaign, &prefabs, prefabs_dir);
                if campaign.detail_plan.is_some() {
                    examined.push(vbind.line());
                }
                diags.extend(vd);
                // spec-0098 §2: an aloft place claims no ground, so terrain
                // reaching into its claim is refused (`DW0990`, third shape),
                // read off the plan and its terrain.
                if campaign.site_plan.is_some() {
                    let (ad, abind) = delvec::compiler::detail::check_aloft(&campaign);
                    examined.push(abind.line());
                    diags.extend(ad);
                }
            }
            // spec-0025 (DSL v0.8): branch-complete narrative verification. Every
            // declared branch is enumerated and every static proof re-run under
            // its flag assignment — terminality, cast continuity, exclusive-content
            // leakage, hard event contradictions — plus the forcing function that
            // every story node says what it does to the story.
            {
                let (bd, bbind) = delvec::compiler::branch::check_branches_bound(&campaign);
                examined.push(bbind.line());
                diags.extend(bd);
            }
            // spec-0031 (DSL v0.10): a numeric gate is judged against the writes
            // the path performs before it (DW0879). The reachability model walks
            // objectives and flags; the arithmetic a `requires_state` compares
            // needs an ORDER, and the flow model's path replay is the one thing
            // in the campaign model that has one. Bound HERE for the same reason
            // the detail plan is: this is the one funnel every subcommand's
            // validation goes through, so a delve whose finale gate can never
            // open cannot reach a datapack by skipping `delvec analyze`. The
            // binding line states what it walked, including the zeroes.
            {
                let (sd, sbind) = delvec::compiler::statepath::check(&campaign);
                examined.push(sbind.line());
                diags.extend(sd);
            }
            // DW0982: a declared name reaches a dialogue line or option only
            // after the play order has told the player what it is
            // (game-writing.md §3 N1/N3). The same walk the replay proves, bound
            // here beside `statepath` for the same reason: every subcommand's
            // validation goes through this funnel.
            {
                let (nd, nbind) = delvec::compiler::telling::check_names_told_bound(&campaign);
                examined.push(nbind.line());
                diags.extend(nd);
            }
            // **`DW0890`: the approved hour is the built hour** (spec-0061).
            // Refused here rather than at the build, on `DW0855`'s precedent
            // and `DW0886`'s: the verdict is a fact about the documents and a
            // directory listing — the rows of `design.json`, the files under
            // `design/`, and the skies the campaign's own effects can reach —
            // so nothing has to be placed to know it, and a creator should not
            // spend a build and a render to learn that their night delve was
            // built at noon. Bound in the one funnel every subcommand's
            // validation goes through, so a mismatch cannot reach a datapack by
            // skipping `delvec validate`. The binding line states what it
            // examined, zeroes included: a campaign with no approved design is
            // a measured zero here and a refusal at staging.
            {
                let (dd, dbind, _) =
                    delvec::compiler::design::check(&campaign, &loaded.design_files);
                examined.push(dbind.line());
                diags.extend(dd);
            }
            // spec-0081 §5.5: every time value the campaign states, as the clock
            // it resolves to — printed on every run, zeroes included.
            examined.extend(delvec::compiler::clock::binding_lines(&campaign));
            // spec-0091: the served view distance and the site-plan lines judged
            // against it (the diagnostics were raised in `validate_campaign_with`).
            examined.push(delvewright_dsl::viewdistance::checks(&campaign, &mut Vec::new()).line());
            print_diags(&diags, json);
            report_binding_notes(&campaign, &examined);
            Ok(Validated {
                campaign,
                prefabs,
                loaded,
                sidecars,
                diags,
            })
        }
        Err(diags) => {
            print_diags(&diags, json);
            Err(1)
        }
    }
}

/// Whether any diagnostic is a hard rejection. Warnings (`Severity::Warning`) are
/// printed like errors but never fail a run: they flag things the compiler cannot
/// decide with certainty (e.g. `DW0330`, where the true limit depends on the
/// player's window size and GUI scale), so failing on them would dress a judgement
/// call as a fact. Every `Severity::Error` still exits non-zero exactly as before.
pub(crate) fn has_error(diags: &[Diagnostic]) -> bool {
    diags
        .iter()
        .any(|d| d.severity == delvewright_dsl::Severity::Error)
}

pub(crate) fn run_validate(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) if !has_error(&v.diags) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(code) => ExitCode::from(code),
    }
}

pub(crate) fn run_analyze(campaign_dir: &Path, prefabs_dir: &Path, json: bool) -> ExitCode {
    let v = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    let adiags = analyze_campaign(&v.campaign, &v.prefabs);
    print_diags(&adiags, json);
    if adiags.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

/// Read the `.nbt` bytes of every structure the plan places. Shared by `build`
/// and the spec-0015 view commands so all three see the same world.
pub(crate) fn read_structures(
    plan: &Plan,
    prefabs: &PrefabRegistry,
    prefabs_dir: &Path,
    json: bool,
) -> Result<BTreeMap<String, Vec<u8>>, u8> {
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    // The horizon surround's tiles first: the compiler GENERATED those bytes, so
    // their `structure_file` keys name nothing on disk and the loop below would
    // fail to find every one of them. Inserted first so a prefab that somehow
    // shared a filename would still win — a file an author shipped is never
    // silently replaced by one the engine made up.
    if let Some(surround) = &plan.surround {
        for (file, bytes) in &surround.structures {
            structures.insert(file.clone(), bytes.clone());
        }
    }
    // Placed pieces, plus any structure a stage-7 `fragment` verb stamps that
    // no piece placed (spec-0017) — the replay needs those bytes too.
    let mut files: Vec<String> = Vec::new();
    for area in &plan.areas {
        for template in area.pieces.iter().flat_map(|p| &p.templates) {
            files.push(template.structure_file.clone());
        }
    }
    files.extend(delvec::compiler::edit::fragment_structure_files(
        plan.campaign,
        prefabs,
    ));
    for file in files {
        {
            if structures.contains_key(&file) {
                continue;
            }
            let path = prefabs_dir.join(&file);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    structures.insert(file.clone(), bytes);
                }
                Err(e) => {
                    print_build_error(
                        delvec::compiler::plan::DW_BUILD,
                        &format!(
                            "cannot read prefab structure file `{}`: {e} — the prefab metadata \
                             points at an `.nbt` that is missing or unreadable in the prefabs dir. \
                             Restore the file or fix the metadata path (prefab-library issue)",
                            path.display()
                        ),
                        json,
                    );
                    return Err(3);
                }
            }
        }
    }
    // The templates are the size their metadata says they are (`DW0803`).
    //
    // Bound here as well as inside `emit::build_with_warnings`, and the second
    // binding is the point: this function is the ONE place every CLI consumer
    // of prefab bytes passes through — `build`, `snapshot`, `viewer` and
    // `blocking-chart` — and a review artifact drawn from a stale tile is a
    // picture that lies, which is worse than a datapack that does not build.
    // The check is pure, so running it twice on the build path costs a walk and
    // reports the same verdict.
    if let Err(delvec::compiler::emit::BuildFailure::Diagnostic { code, message }) =
        delvec::compiler::emit::check_template_extents(plan, &structures)
    {
        print_build_error(code, &message, json);
        return Err(3);
    }
    Ok(structures)
}

/// **A perturbed build is structurally unable to produce a tree.**
///
/// `--out` and `--perturb` are declared as conflicting arguments, so this pair
/// has exactly two inhabited shapes and the parser refuses the other two: an
/// ordinary build carrying a directory, or a demonstration carrying a defect and
/// no directory at all. It is a type here rather than two `Option`s and a
/// comment because the guarantee is worth stating in a form a reader cannot
/// misread: the perturbed arm has nowhere to write, so it does not decline to
/// write a tree — it has no path to write one to. No tree means no
/// `manifest.json`, and `tools/creator/staging-gate.py` fingerprints a build by hashing
/// exactly that file: *a tree with no manifest has no identity and therefore
/// cannot be admitted at all*.
enum BuildKind<'a> {
    /// The build as it ships.
    Ship(&'a Path),
    /// The derivation asked for a named defect, so its observer can be watched.
    Demonstrate(Knob, Perturb),
}

pub(crate) fn run_build(
    campaign_dir: &Path,
    out: Option<&Path>,
    perturb: &[Knob],
    perturb_place: Option<&str>,
    prefabs_dir: &Path,
    lang: &str,
    json: bool,
) -> ExitCode {
    let kind = match resolve_build_kind(out, perturb, perturb_place) {
        Ok(k) => k,
        Err(msg) => {
            eprintln!("error: {msg}");
            return ExitCode::from(1);
        }
    };

    let v = match validate_stage(campaign_dir, prefabs_dir, json) {
        Ok(v) => v,
        Err(code) => return ExitCode::from(code),
    };
    if has_error(&v.diags) {
        return ExitCode::from(1);
    }
    let Validated {
        campaign,
        prefabs,
        loaded,
        sidecars,
        ..
    } = v;
    let adiags = analyze_campaign(&campaign, &prefabs);
    if !adiags.is_empty() {
        print_diags(&adiags, json);
        return ExitCode::from(2);
    }

    // **Every approved picture is answered** (`DW0900`, spec-0070), and the
    // record's own rule read by the run that consumes it (`DW0721`). Here —
    // after validation and analysis, before a piece is seated and before
    // anything is written under `-o` — because the two documents are already in
    // hand, and a refusal at this point leaves the creator's previous build tree
    // on disk, which is what `delvec cameras --preview` needs to place the
    // camera the refusal asks for. Not at validation: every view command
    // validates first, so a validation-tier refusal would refuse the instrument.
    {
        let answered = delvec::compiler::design::answered(&campaign, &loaded.design_files);
        if !answered.is_empty() {
            print_diags(&answered, json);
            return ExitCode::from(3);
        }
    }

    // A perturbation naming a place no box declares would derive a perfectly
    // clean map, and the run would then report an observer that failed to
    // observe — a true sentence about the wrong thing. So the name is checked
    // against the site plan's own boxes, which is the one authority for what a
    // place is, before anything is derived.
    if let BuildKind::Demonstrate(knob, p) = &kind
        && let Some(place) = p.place()
    {
        let declared: Vec<&str> = campaign
            .site_plan
            .as_ref()
            .map(|sp| sp.content.boxes.iter().map(|b| b.node.0.as_str()).collect())
            .unwrap_or_default();
        if !declared.contains(&place) {
            eprintln!(
                "error: --perturb {} --perturb-place `{place}`: this campaign's site plan \
                 declares no such place. It declares {}: {}",
                knob.name(),
                declared.len(),
                if declared.is_empty() {
                    "(this campaign has no site plan, so there is no derivation to perturb)"
                        .to_string()
                } else {
                    declared.join(", ")
                }
            );
            return ExitCode::from(1);
        }
    }

    // i18n: resolve the requested build language. `en` is the implicit canonical
    // build; any other code must be declared in world.json (coverage already
    // validated above). An undeclared `--lang` is a validation-class rejection of
    // the requested build (exit 1, spec-0002).
    let is_english = lang == delvewright_dsl::CANONICAL_LANG;
    let mut campaign = campaign;
    if !is_english {
        if !campaign.world.content.languages.iter().any(|l| l == lang) {
            eprintln!(
                "error: --lang `{lang}` is not a declared language (world.json declares: {:?}); \
                 `en` is always available",
                campaign.world.content.languages
            );
            return ExitCode::from(1);
        }
        let Some(doc) = sidecars.get(lang) else {
            eprintln!("error: no l10n/{lang}.json sidecar for declared language `{lang}`");
            return ExitCode::from(1);
        };
        // Swap every player-visible string to the target language, then record the
        // sidecar as a build input (manifest provenance) for the non-en build.
        delvewright_dsl::localize(&mut campaign, &doc.content);
    }
    let build_lang = if is_english { None } else { Some(lang) };
    // i18n v2 (spec-0029): the DEFAULT build ships every declared language and lets
    // the client choose, so each authored string travels to emission carrying its
    // l10n key and becomes `{"translate": key, "fallback": english}`. A `--lang`
    // bake is the unchanged single-language artifact (spec-0029 §4): its strings
    // were already swapped above, so it is emitted as literals exactly as before.
    if is_english {
        delvewright_dsl::tag_translatables(&mut campaign);
    }
    // A baked skin lands in the client's texture space, which is shared exactly as
    // the language table is: every body's texture is rewritten to this delve's own
    // id here, once, so no emitter and no bake can ship a face under a name another
    // delve answers. Unconditional — a `--lang` build ships the same pack. The map
    // back to the authored id is what finds the PNG on disk below.
    let skin_sources = delvewright_dsl::namespace_skin_textures(&mut campaign);

    // The one caller of `Plan::build_with` outside a test, and the ordinary arm
    // is still `Plan::build` — the constructor that passes `Perturb::none()` as
    // a literal — so a build with no `--perturb` reaches the derivation through
    // exactly the code path it always did.
    let built = match &kind {
        BuildKind::Ship(_) => Plan::build(&campaign, &prefabs),
        BuildKind::Demonstrate(knob, p) => {
            eprintln!(
                "perturbed build: the derivation is asked for `{}` ({}{}); the observer this \
                 defect is documented to redden is {}. This run writes NO output tree.",
                knob.name(),
                knob.blurb(),
                p.place().map(|n| format!(" at `{n}`")).unwrap_or_default(),
                knob.documented_code(),
            );
            Plan::build_with(&campaign, &prefabs, p.clone())
        }
    };
    let plan = match built.map(|p| p.with_design_files(loaded.design_files.clone())) {
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

    // read the structure .nbt bytes referenced by placements
    let structures = match read_structures(&plan, &prefabs, prefabs_dir, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let skins = match read_skins(campaign_dir, &campaign, &skin_sources, json) {
        Ok(s) => s,
        Err(code) => return ExitCode::from(code),
    };

    let tree = CommandTree::v1_21_11();
    let output = match emit::build_with_warnings(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        build_lang,
        &skins,
    ) {
        Ok((o, warnings)) => {
            // Advisory build-tier findings (stage-7 edit replay: DW0353/DW0354).
            // Printed exactly like the validation-tier warnings, and like them
            // they never change the exit code.
            print_diags(&warnings, json);
            o
        }
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
            // The tier is the code's own ([`ExitTier`]) — an analysis-tier
            // refusal (exit 2) says the CONTENT is the defect, a build-tier one
            // (exit 3) says the compiler could not produce a tree. It used to be
            // re-derived here from the code's spelling, in a copy of the same
            // expression this verb, `edit` and `edit --check` each kept.
            print_build_error(code, &message, json);
            if let BuildKind::Demonstrate(knob, _) = &kind {
                let got = code.to_string();
                eprintln!(
                    "perturbed build: the observer REFUSED the derivation's `{}` defect. The \
                     build stops at the FIRST refusal, {got}{}. No output tree was written.",
                    knob.name(),
                    if got == knob.documented_code() {
                        ", which is the code this knob is documented to redden".to_string()
                    } else {
                        format!(
                            "; this knob is documented to redden {} — the battery's refusal \
                             line above names every rule that saw the defect, and one \
                             derivation defect is routinely seen by two of them",
                            knob.documented_code()
                        )
                    }
                );
            }
            return ExitCode::from(code.exit_tier().exit_status());
        }
    };

    // **The demonstration's own verdict, and the only place a perturbed run can
    // reach with a datapack in hand.** Getting here means the derivation built
    // the named defect and every observer passed it — which is the failure the
    // whole facility exists to be able to see, so it is a refusal rather than a
    // success, and the output is dropped unwritten. `BuildKind::Demonstrate`
    // carries no path, so this is a statement about what did not happen rather
    // than a decision not to do it.
    let out = match kind {
        BuildKind::Ship(out) => out,
        BuildKind::Demonstrate(knob, p) => {
            eprintln!(
                "perturbed build: the derivation was asked for `{}`{} and produced a datapack that \
                 NOTHING refused — {} did not fire, and neither did any other build-tier \
                 check. Either this campaign has nothing for that defect to damage, or an \
                 observer that claims to read the built bytes is reciting the arithmetic that \
                 laid them. The output was discarded; a perturbed run has no `--out` to write \
                 to.",
                knob.name(),
                p.place().map(|n| format!(" at `{n}`")).unwrap_or_default(),
                knob.documented_code(),
            );
            return ExitCode::from(3);
        }
    };

    // The directory holds exactly this build afterwards, or nothing in it moves
    // (`delvec::outdir`, `DW0967`).
    match delvec::outdir::replace(out, &output) {
        Ok(done) => {
            if let Some(line) = done.summary(out) {
                eprintln!("{line}");
            }
        }
        Err(delvec::outdir::OutError::NotOwned(why)) => {
            let code = delvec::outdir::DW_OUTPUT_NOT_OWNED;
            print_build_error(code, &why, json);
            return ExitCode::from(code.exit_tier().exit_status());
        }
        Err(delvec::outdir::OutError::Internal(why)) => {
            eprintln!("internal error: cannot write output: {why}");
            return ExitCode::from(EXIT_INTERNAL);
        }
        Err(delvec::outdir::OutError::Io(e)) => {
            eprintln!("internal error: cannot write output: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    }
    ExitCode::SUCCESS
}

/// Which of the two builds this invocation is, or why it is neither.
///
/// `clap` already refuses `--out` beside `--perturb` and refuses a knob it does
/// not know, so what is left here is the arity — `--perturb` is a `Vec` so that
/// a second one is an ERROR rather than the silent last-wins a `Option` would
/// give, which on a defect-injection flag is the shape that reports a
/// demonstration of the knob nobody asked about — and the place, which three of
/// the knobs need and the other three refuse.
fn resolve_build_kind<'a>(
    out: Option<&'a Path>,
    perturb: &[Knob],
    place: Option<&str>,
) -> Result<BuildKind<'a>, String> {
    let knob = match perturb {
        [] => {
            let out = out.ok_or_else(|| {
                "`--out` is required for a build that is not perturbed".to_string()
            })?;
            if place.is_some() {
                return Err(
                    "`--perturb-place` names the place `--perturb` damages, and this \
                            run asks for no perturbation"
                        .to_string(),
                );
            }
            return Ok(BuildKind::Ship(out));
        }
        [one] => *one,
        many => {
            return Err(format!(
                "`--perturb` takes exactly one knob and this run names {}: {}. One defect per \
                 run is what makes the refusal attributable — two at once and the code that \
                 fires says nothing about which defect it saw",
                many.len(),
                many.iter().map(|k| k.name()).collect::<Vec<_>>().join(", ")
            ));
        }
    };
    match (knob.takes_place(), place) {
        (true, None) => Err(format!(
            "`--perturb {}` damages ONE place and this run names none — add `--perturb-place \
             <place>`",
            knob.name()
        )),
        (false, Some(p)) => Err(format!(
            "`--perturb {}` damages the whole derivation, not one place, so `--perturb-place \
             {p}` would decide nothing. The knobs that take a place are: {}",
            knob.name(),
            Knob::ALL
                .iter()
                .filter(|k| k.takes_place())
                .map(|k| k.name())
                .collect::<Vec<_>>()
                .join(", ")
        )),
        (_, p) => Ok(BuildKind::Demonstrate(
            knob,
            knob.perturb(p)
                .expect("the arity was just checked against `takes_place`"),
        )),
    }
}

/// Read the skin PNGs every staged **body** references (spec-0009 bake). The PNG
/// lives in the campaign dir at `skins/<texture_id>.png`; a missing one is a
/// build error (`DW0309`), not a silent skip. Shared by `build` and by `edit`'s
/// build-tier proof run — the editor must prove exactly what `build` proves.
///
/// **Enumerated from [`delvewright_dsl::body_skin_sites`], never from one
/// stage's list.** This walked `campaign.npcs.content.npcs` by hand, so a
/// stage-5 actor's skin was read into its summon
/// (`profile:{texture:"delvewright:npc/<id>"}`), shipped in a resource pack that
/// carried no such texture, and refused by nothing: deleting an npc's PNG exited
/// 3 with `DW0309` while deleting an actor's built green. A skin is a property
/// of a body, so the walk is over bodies.
///
/// One texture is read once however many bodies name it — a character and the
/// puppet that plays it are one face.
///
/// **Keyed by the pack texture id, read from the authored one.** The campaign
/// reaching here has been through `dsl::namespace_skin_textures`, so every body's
/// `texture_id` is this delve's own id (`<campaign_id>/<authored>`) and `sources`
/// is the map back to what the creator wrote — which is what `skins/<id>.png` is
/// named after. The returned map is keyed the way the pack must write it, so the
/// archive path and the texture the summon points at are one id.
pub(crate) fn read_skins(
    campaign_dir: &Path,
    campaign: &delvewright_dsl::Campaign,
    sources: &BTreeMap<String, String>,
    json: bool,
) -> Result<BTreeMap<String, Vec<u8>>, u8> {
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for site in delvewright_dsl::body_skin_sites(campaign) {
        if skins.contains_key(&site.skin.texture_id) {
            continue;
        }
        // Total by construction: both callers rewrite before they read. The
        // identity fallback is what an un-namespaced campaign would mean, not a
        // repair of one.
        let authored = sources
            .get(&site.skin.texture_id)
            .map(String::as_str)
            .unwrap_or(site.skin.texture_id.as_str());
        let path = campaign_dir.join("skins").join(format!("{authored}.png"));
        match std::fs::read(&path) {
            Ok(bytes) => {
                skins.insert(site.skin.texture_id.clone(), bytes);
            }
            Err(e) => {
                print_build_error(
                    DW_SKIN_PNG_MISSING,
                    &format!(
                        "cannot read skin PNG `{}`: {e} — `{}` declares `skin.texture_id` \
                         `{authored}` at `{}` `{}`, but the campaign has no matching \
                         `skins/<texture_id>.png`. A body that declares a skin ships as a \
                         mannequin pointing at `delvewright:npc/{}` — this delve's own texture \
                         id — and the resource pack is where that texture comes from. Add the \
                         PNG at that path, or remove the `skin`",
                        path.display(),
                        site.body.id(),
                        site.body.stage(),
                        site.path,
                        site.skin.texture_id,
                    ),
                    json,
                );
                return Err(3);
            }
        }
    }
    Ok(skins)
}
