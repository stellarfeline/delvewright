//! `delvec district list|packet|writes` (spec-0103): the verbs a planner and a
//! district's worker run when a campaign is authored in parallel.
//!
//! Exit codes: `0` ok (`list` exits 0 with every `DW1007` advisory printed) ·
//! `1` refused — a campaign that does not load or parse, a district the graph
//! does not declare, a packet whose `--prefabs` is outside the campaign's
//! repository, a path outside the write set (`DW1008`) · `10` internal.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Subcommand, ValueEnum};
use delvec::compiler::district::{self, census, packet, writes};
use delvewright_dsl::Diagnostic;

use crate::EXIT_INTERNAL;
use crate::cli::campaign::load_or_refuse;
use crate::cli::report::{print_diags, set_router};

/// Which wave a packet is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum WaveKind {
    /// Step 5: the district's quests, dialogue, NPCs and stations.
    Story,
    /// Step 9: the district's places detailed.
    Build,
    /// A finding routed to the district (`--finding` carries it).
    Repair,
}

#[derive(Subcommand, Clone, Debug)]
pub(crate) enum DistrictAction {
    /// One row per district — places (reached and scenery), seams inside,
    /// boundary seams and carries, stations, beats, planned quests (expanded
    /// of planned), places detailed, critical-path steps, acts — and `DW1007`
    /// beside a district outside the partitioning guide's bounds. Advises and
    /// never refuses: exit 0.
    List {
        /// Campaign directory.
        campaign_dir: PathBuf,
    },
    /// Write a district's handover packet to `-o <dir>`: `packet.json`,
    /// `PACKET.md` rendered from it, and `images/` holding a copy of every
    /// image it names. Derived from the documents on every invocation and an
    /// input to nothing. Refused when `--prefabs` resolves outside the
    /// campaign's repository: on the parallel route a worker's pieces travel
    /// on its branch.
    Packet {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// The district (`district/<kebab>`).
        district: String,
        /// The packet directory (created; its `images/` is replaced).
        #[arg(short, long)]
        out: PathBuf,
        /// The wave the packet is for.
        #[arg(long, value_enum, default_value = "story")]
        wave: WaveKind,
        /// The finding a `repair` wave carries, verbatim, from a file.
        #[arg(long, value_name = "FILE")]
        finding: Option<PathBuf>,
    },
    /// Read repository-relative paths, one per line, and refuse every path
    /// outside the district's write set (`DW1008`), naming its owner — the
    /// whole, or the district it belongs to. Exit 1 on any refusal, 0 when
    /// every path is inside. The one rule behind the worker's pre-commit hook
    /// and the planner's integration check.
    Writes {
        /// Campaign directory.
        campaign_dir: PathBuf,
        /// The district (`district/<kebab>`).
        district: String,
        /// A file of paths, or `-` (the default) for stdin.
        #[arg(long, value_name = "FILE", default_value = "-")]
        paths: String,
    },
}

/// Load and parse, printing the refusals; the router is installed.
fn parsed(
    campaign_dir: &Path,
    json: bool,
) -> Result<
    (
        delvewright_dsl::Campaign,
        delvec::compiler::load::LoadedCampaign,
    ),
    u8,
> {
    let loaded = load_or_refuse(campaign_dir, json)?;
    match delvec::compiler::load::parse_loaded(&loaded) {
        Ok(c) => {
            set_router(district::ledger::Attribution::of(&c, &loaded.fragments));
            Ok((c, loaded))
        }
        Err(diags) => {
            set_router(district::ledger::Attribution::from_fragments(
                &loaded.fragments,
            ));
            print_diags(&diags, json);
            Err(1)
        }
    }
}

fn unknown(c: &delvewright_dsl::Campaign, name: &str, json: bool) -> Option<u8> {
    let d: Diagnostic = district::scope::unknown(c, name)?;
    print_diags(&[d], json);
    Some(1)
}

pub(crate) fn run(action: &DistrictAction, prefabs: &Path, json: bool) -> ExitCode {
    let r = match action {
        DistrictAction::List { campaign_dir } => list(campaign_dir, json),
        DistrictAction::Packet {
            campaign_dir,
            district,
            out,
            wave,
            finding,
        } => run_packet(
            campaign_dir,
            district,
            out,
            *wave,
            finding.as_deref(),
            prefabs,
            json,
        ),
        DistrictAction::Writes {
            campaign_dir,
            district,
            paths,
        } => run_writes(campaign_dir, district, paths, prefabs, json),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn list(campaign_dir: &Path, json: bool) -> Result<(), u8> {
    let (c, _) = parsed(campaign_dir, json)?;
    let rows = census::census(&c);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&rows).expect("a census serialises")
        );
    } else {
        print!("{}", census::table(&rows));
    }
    let advice = census::advisories(&rows);
    if !advice.is_empty() {
        print_diags(&advice, json);
    }
    eprintln!("district binding: {}", census::line(&rows));
    Ok(())
}

/// The campaign directory and the prefab directory, repository-relative.
fn placed(
    campaign_dir: &Path,
    prefabs: &Path,
) -> Result<(PathBuf, String, Option<String>), String> {
    let root = writes::repo_root(campaign_dir).ok_or_else(|| {
        format!(
            "`{}` is not inside a git repository, so there is no tree for a worker's branch to \
             carry — commit the campaign to its repository first",
            campaign_dir.display()
        )
    })?;
    let campaign_rel = writes::relative(&root, campaign_dir).ok_or_else(|| {
        format!(
            "cannot place `{}` in `{}`",
            campaign_dir.display(),
            root.display()
        )
    })?;
    let prefabs_rel = if prefabs.exists() {
        writes::relative(&root, prefabs)
    } else {
        None
    };
    Ok((root, campaign_rel, prefabs_rel))
}

fn baseline(root: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn run_packet(
    campaign_dir: &Path,
    name: &str,
    out: &Path,
    wave: WaveKind,
    finding: Option<&Path>,
    prefabs: &Path,
    json: bool,
) -> Result<(), u8> {
    let (c, loaded) = parsed(campaign_dir, json)?;
    if let Some(code) = unknown(&c, name, json) {
        return Err(code);
    }
    let (root, campaign_rel, prefabs_rel) = match placed(campaign_dir, prefabs) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return Err(1);
        }
    };
    let Some(prefabs_rel) = prefabs_rel else {
        eprintln!(
            "error: `--prefabs {}` resolves outside the campaign's repository (`{}`). On the \
             parallel route a worker details into the prefab directory of its own worktree, so \
             its pieces travel on its branch and the merge brings them; a directory outside the \
             repository would leave every worker's pieces on this machine and none on a branch. \
             Take the library into the repository first (the skill's Init I6 states the cases), \
             then pass that directory as `--prefabs`.",
            prefabs.display(),
            root.display()
        );
        return Err(1);
    };
    let wave = match (wave, finding) {
        (WaveKind::Story, _) => packet::Wave::Story,
        (WaveKind::Build, _) => packet::Wave::Build,
        (WaveKind::Repair, Some(f)) => match std::fs::read_to_string(f) {
            Ok(t) => packet::Wave::Repair(t),
            Err(e) => {
                eprintln!("error: cannot read the finding {}: {e}", f.display());
                return Err(1);
            }
        },
        (WaveKind::Repair, None) => {
            eprintln!("error: a `repair` packet carries its finding: pass `--finding <file>`");
            return Err(1);
        }
    };
    let ws = writes::write_set(&c, name, &campaign_rel, Some(&prefabs_rel));
    let ctx = packet::Context {
        campaign_dir,
        prefabs_rel: &prefabs_rel,
        campaign_rel: &campaign_rel,
        wave,
        baseline: baseline(&root),
    };
    let p = match packet::derive(&c, &loaded.fragments, &loaded.design_files, name, &ctx, &ws) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return Err(1);
        }
    };
    let images = out.join("images");
    let _ = std::fs::remove_dir_all(&images);
    if let Err(e) = std::fs::create_dir_all(&images) {
        eprintln!("internal error: cannot create {}: {e}", images.display());
        return Err(EXIT_INTERNAL);
    }
    for rel in &p.images {
        let dest = images.join(rel);
        if let Some(parent) = dest.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            eprintln!("internal error: cannot create {}: {e}", parent.display());
            return Err(EXIT_INTERNAL);
        }
        if let Err(e) = std::fs::copy(campaign_dir.join(rel), &dest) {
            eprintln!("internal error: cannot copy {rel}: {e}");
            return Err(EXIT_INTERNAL);
        }
    }
    let text = serde_json::to_string_pretty(&p.doc).expect("a packet serialises") + "\n";
    let md = packet::render(&p.doc);
    for (file, body) in [("packet.json", text.as_str()), ("PACKET.md", md.as_str())] {
        if let Err(e) = std::fs::write(out.join(file), body) {
            eprintln!(
                "internal error: cannot write {}: {e}",
                out.join(file).display()
            );
            return Err(EXIT_INTERNAL);
        }
    }
    let contracts: usize = p.doc["neighbours"].as_array().map_or(0, |n| {
        n.iter()
            .map(|x| x["contracts"].as_array().map_or(0, Vec::len))
            .sum()
    });
    eprintln!(
        "packet: `{name}` written to {} — {} place(s), {} neighbour(s), {} boundary contract(s), \
         {} owed quest(s), {} image(s), {} write-set rule(s).",
        out.display(),
        p.doc["places"].as_array().map_or(0, Vec::len),
        p.doc["neighbours"].as_array().map_or(0, Vec::len),
        contracts,
        p.doc["story"]["owed"].as_array().map_or(0, Vec::len),
        p.images.len(),
        p.doc["write_set"].as_array().map_or(0, Vec::len),
    );
    Ok(())
}

fn run_writes(
    campaign_dir: &Path,
    name: &str,
    paths: &str,
    prefabs: &Path,
    json: bool,
) -> Result<(), u8> {
    let (c, _) = parsed(campaign_dir, json)?;
    if let Some(code) = unknown(&c, name, json) {
        return Err(code);
    }
    let (_, campaign_rel, prefabs_rel) = match placed(campaign_dir, prefabs) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {e}");
            return Err(1);
        }
    };
    let mut text = String::new();
    let read = if paths == "-" {
        std::io::stdin().read_to_string(&mut text).map(|_| ())
    } else {
        std::fs::read_to_string(paths).map(|t| text = t)
    };
    if let Err(e) = read {
        eprintln!("error: cannot read the paths ({paths}): {e}");
        return Err(1);
    }
    let p = district::Partition::of(&c);
    let sets: Vec<writes::WriteSet> = p
        .ids
        .iter()
        .map(|d| writes::write_set(&c, d, &campaign_rel, prefabs_rel.as_deref()))
        .collect();
    let mine = sets
        .iter()
        .find(|s| s.district == name)
        .expect("a declared district has a set");
    let lines: Vec<String> = text.lines().filter_map(writes::normalise).collect();
    let mut refused: Vec<Diagnostic> = Vec::new();
    for path in &lines {
        if mine.holds(path) {
            continue;
        }
        let owner = writes::owner(&sets, path);
        refused.push(Diagnostic::error(
            district::DW_WRITE_OUTSIDE,
            "district-writes",
            path.clone(),
            format!(
                "`{path}` is outside `{name}`'s write set; it belongs to {owner}. A district's \
                 worker writes its own fragments (`{campaign_rel}/districts/{x}/`), the programs \
                 and per-place images of its own places, and the pieces `delvec detail` writes \
                 for them — nothing else. Undo the change to this path (`git restore --staged` \
                 then `git checkout --`), and if the whole needs it changed, name it in your \
                 report as a request to the planner.",
                owner = if owner == "the whole" {
                    "the whole (the planner's documents)".to_string()
                } else {
                    format!("`{owner}`")
                },
                x = district::dir_of(name),
            ),
        ));
    }
    if !refused.is_empty() {
        print_diags(&refused, json);
    }
    eprintln!(
        "district writes: {} path(s) read, {} inside `{name}`'s write set, {} refused (DW1008).",
        lines.len(),
        lines.len() - refused.len(),
        refused.len()
    );
    if refused.is_empty() { Ok(()) } else { Err(1) }
}
