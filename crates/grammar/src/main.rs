//! `delve-grammar` — drive the box-split grammar back end from the command line
//! (spec-0027 §3).
//!
//! # Why this binary exists
//!
//! spec-0027 was approved on 2026-08-04 and `crates/grammar` was built: the
//! expander, the rule library, the `.nbt` export. What was never built is a way
//! to *run* it. Its only callers were `cargo test`, so the division of labour
//! the spec is entirely about — **the model authors rules, the deterministic
//! expander does geometry, machine gates judge the result** — had no entry point
//! at all, and the only way to produce a prefab from a grammar program was to
//! write a Rust test. A back end a creator cannot invoke is a library, not a
//! back end.
//!
//! Six commands. The first four are the steps of the loop; the last two ask
//! what the corpus those steps start from demonstrates, and whether every
//! member of it still holds up.
//!
//! ```text
//! delve-grammar list                                  # what can be built
//! delve-grammar show --program store-room > scene.json # start from the corpus
//! delve-grammar check --file scene.json                # does the program hold together
//! delve-grammar expand --file scene.json --region 11x6x13 -o out/   # build + judge + freeze
//! delve-grammar coverage                              # what no example demonstrates
//! delve-grammar audit --library --campaign-root ../content  # judge EVERY program
//! ```
//!
//! `expand` judges the one program an operator names, which leaves the corpus
//! itself judged only when somebody remembers to walk it — and a campaign's
//! zone programs, the artifacts of record, had nothing walking them at all.
//! `audit` is that walk, and it is what CI runs.
//!
//! `--file` takes the typed JSON IR, which is the authoring form spec-0027 §3
//! names: an LLM writes that file, and nothing between it and the `.nbt` is
//! hand-assembled.
//!
//! Exit codes (mirroring `delve-schem` and `delve-render`): `0` ok · `2`
//! input/usage · `3` output · `4` a machine gate went red.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use delvewright_grammar::block::BlockState;
use delvewright_grammar::coverage;
use delvewright_grammar::document::{self, Loaded};
use delvewright_grammar::gates;
use delvewright_grammar::ir::{Paint, Program};
use delvewright_grammar::{Axis, Box3, ExpandOptions, Expansion, expand, export, library};

const EXIT_INPUT: u8 = 2;
const EXIT_OUTPUT: u8 = 3;
const EXIT_GATE: u8 = 4;

#[derive(Parser)]
#[command(
    name = "delve-grammar",
    about = "Box-split grammar prefab back end: list programs, check one, expand it into a prefab",
    version,
    disable_version_flag = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List every program in the rule library, with its knobs.
    List,
    /// Validate a program's structure without expanding it.
    ///
    /// Fast, and the right first call when a program was just written: every
    /// `Program::validate` refusal (unknown rule, unknown role, a split whose
    /// children do not match its pieces, an unmatchable guard) is found here,
    /// with no region and no seed involved.
    Check {
        #[command(flatten)]
        source: Source,
    },
    /// Print a program as the typed JSON IR — the authoring form.
    ///
    /// The library is a **few-shot corpus we legally own** (spec-0027 §2), and
    /// this is how an author reaches it: start from the nearest rule, edit the
    /// JSON, `check` it. Without this the corpus is Rust source, which is not
    /// the form anything downstream consumes.
    Show {
        #[command(flatten)]
        source: Source,
    },
    /// Report which IR constructs the rule library demonstrates, and which
    /// none of it does.
    ///
    /// `prefab-procedure.md` §3 sends an author to the corpus, not the schema,
    /// so a construct no example writes does not exist in practice. This counts
    /// every `Node` kind, every `Cond` kind and every palette paint kind over
    /// the programs `list` names, and calls a zero what it is.
    ///
    /// It measures **demonstration, not expressiveness**: a pass means no part
    /// of the language is left undemonstrated. It is not evidence that an
    /// author can build any particular thing.
    Coverage {
        /// Also write the report as JSON to this path.
        #[arg(long, value_name = "PATH")]
        json: Option<PathBuf>,
    },
    /// Expand and judge **every** program of a corpus, and say what bound.
    ///
    /// `expand` judges one program an operator names. That leaves the corpus
    /// itself unjudged unless somebody remembers to walk it, and a campaign's
    /// zone programs — the artifacts of record — had nothing walking them at
    /// all. This is the walk: it enumerates a corpus, expands every member at
    /// the expansion the corpus itself declares, runs the same `gates::judge`
    /// `expand` runs, and reds if any gate fails or if the corpus it was
    /// pointed at was empty.
    ///
    /// A gate that bound to zero reds here too — but as an ordinary gate
    /// failure, decided by `gates::seal_zero_bindings` before the report
    /// reaches this command, and therefore identically through `expand`. This
    /// command used to apply its own zero-binding rule on top, which made it a
    /// second authority that disagreed with the creator's own door.
    ///
    /// It never writes a prefab. Judging is the whole job.
    Audit {
        /// Audit the built-in rule library (`list`), at the region and seed
        /// each entry declares.
        #[arg(long)]
        library: bool,
        /// Audit every campaign zone program under this content-repo root —
        /// `<root>/campaigns/<campaign>/design/programs/`. Repeatable.
        #[arg(long = "campaign-root", value_name = "PATH")]
        campaign_roots: Vec<PathBuf>,
        /// A JSON file recording programs that are KNOWN red, with the exact
        /// diagnostic codes each must fail with.
        ///
        /// It inverts the assertion, it never removes it: a recorded program is
        /// still expanded and still judged, and it is a finding if it passes,
        /// if it fails with a different code, or if it fails with one more. A
        /// plain skip would be satisfied by the very defect it excuses. Absent
        /// flag means no exclusions, which is the strict reading.
        #[arg(long, value_name = "PATH")]
        exclusions: Option<PathBuf>,
    },
    /// Expand a program over a region and freeze it as a prefab.
    Expand {
        #[command(flatten)]
        source: Source,
        /// Region to expand into, `XxYxZ` (e.g. `11x6x13`).
        #[arg(long)]
        region: String,
        /// Expansion seed.
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// Override an integer parameter: `--param head=5`. Repeatable.
        #[arg(long = "param", value_name = "NAME=VALUE")]
        params: Vec<String>,
        /// Rebind a palette role: `--role stone=minecraft:deepslate_bricks`.
        /// Repeatable.
        #[arg(long = "role", value_name = "ROLE=BLOCKSTATE")]
        roles: Vec<String>,
        /// Prefab id: lowercase letters, digits and hyphens. It names the
        /// `.nbt`, the metadata beside it and the datapack structure path.
        /// Defaults to the library program id with `--program`, and to the
        /// input file's stem with `--file` — the program's own `name` field
        /// identifies the *program* in provenance and is never the artifact's
        /// id.
        #[arg(long)]
        id: Option<String>,
        /// Also gate on the piece being walkable from its approach end to its
        /// exit end.
        #[arg(long)]
        traversable: bool,
        /// With `--traversable`, allow a fall edge — for a piece entered by
        /// stepping off a ledge.
        #[arg(long)]
        allow_falls: bool,
        /// Also gate on the piece being its own mirror image across this world
        /// axis (`x`, `y` or `z`) — the claim a shape with a mirror plane makes,
        /// and the one nothing else in the report reads.
        #[arg(long, value_name = "AXIS")]
        symmetric: Option<String>,
        /// Also gate on every piece of floor **under a roof** being walkable to
        /// from the grade entrance.
        ///
        /// The reachability numbers are measured and printed either way; this
        /// only says the piece claims a body can get everywhere indoors. Floor
        /// open to the sky is never gated — the engine cannot tell a roof from
        /// a terrace.
        #[arg(long)]
        reachable_floor: bool,
        /// Output directory. Created if absent.
        #[arg(short, long)]
        out: PathBuf,
    },
}

/// Where the program comes from. Exactly one of the two.
#[derive(clap::Args)]
#[group(required = true, multiple = false)]
struct Source {
    /// A library program id (see `list`).
    #[arg(long)]
    program: Option<String>,
    /// A grammar program as typed JSON IR — the authoring form.
    #[arg(long)]
    file: Option<PathBuf>,
}

fn bad_input(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(EXIT_INPUT)
}

impl Source {
    /// Load the program, naming what failed.
    ///
    /// `--file` goes through `document::load`, which is the **only** thing that
    /// resolves an `include`: a document that composes another names a FILE, and
    /// nothing but a loader may read one. A program that arrives with its
    /// includes unresolved is refused by `Program::validate` rather than
    /// expanded, so this is not a step a caller can quietly skip.
    fn load(&self) -> Result<(String, Loaded), String> {
        match (&self.program, &self.file) {
            (Some(id), None) => match library::by_id(id) {
                Some(program) => Ok((
                    id.clone(),
                    Loaded {
                        program,
                        compositions: Vec::new(),
                    },
                )),
                None => Err(format!(
                    "no library program {id:?} — `delve-grammar list` names them all"
                )),
            },
            (None, Some(path)) => {
                let loaded = document::load(path).map_err(|e| e.to_string())?;
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("program")
                    .to_string();
                Ok((id, loaded))
            }
            _ => Err("give exactly one of --program or --file".to_string()),
        }
    }

    /// Where the default id comes from, said the way an author can act on.
    ///
    /// The default is not obvious from anywhere the author looked: with
    /// `--file` it is the **input filename's stem**, which the program itself
    /// never mentions. So the refusal names it instead of leaving the author to
    /// guess which of the two inputs it disliked.
    fn default_id_origin(&self) -> String {
        match (&self.program, &self.file) {
            (Some(_), None) => "the library program id".to_string(),
            (None, Some(path)) => format!(
                "the stem of the input filename {:?}",
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("(unnamed)")
            ),
            _ => "the program source".to_string(),
        }
    }
}

fn parse_region(s: &str) -> Result<[u32; 3], String> {
    let parts: Vec<&str> = s.split(['x', 'X']).collect();
    if parts.len() != 3 {
        return Err(format!("region {s:?} is not XxYxZ (e.g. 11x6x13)"));
    }
    let mut out = [0u32; 3];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p
            .trim()
            .parse()
            .map_err(|_| format!("region {s:?}: {p:?} is not a positive integer"))?;
    }
    if out.contains(&0) {
        return Err(format!("region {s:?} has a zero axis"));
    }
    Ok(out)
}

fn parse_axis(s: &str) -> Result<Axis, String> {
    match s.trim() {
        "x" | "X" => Ok(Axis::X),
        "y" | "Y" => Ok(Axis::Y),
        "z" | "Z" => Ok(Axis::Z),
        other => Err(format!("{other:?} is not a world axis; give x, y or z")),
    }
}

fn split_once_eq<'a>(s: &'a str, what: &str) -> Result<(&'a str, &'a str), String> {
    s.split_once('=')
        .ok_or_else(|| format!("{what} {s:?} is not NAME=VALUE"))
}

fn run_list() -> ExitCode {
    println!(
        "{} program(s) in the rule library:",
        library::PROGRAMS.len()
    );
    for lib in library::PROGRAMS {
        let id = lib.id;
        let program = (lib.build)();
        let params: Vec<String> = program
            .params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let roles: Vec<&String> = program.palette.keys().collect();
        println!("  {id}");
        // The expansion the corpus demonstrates it at, so an author reaching
        // for a piece gets the region from the tool rather than from a page.
        println!(
            "      judged {}x{}x{}, seed {}{}",
            lib.region[0],
            lib.region[1],
            lib.region[2],
            lib.seed,
            if lib.gates.traversable {
                ", claims a route"
            } else {
                ""
            }
        );
        println!(
            "      params {}",
            if params.is_empty() {
                "(none)".to_string()
            } else {
                params.join(" ")
            }
        );
        println!(
            "      roles  {}",
            if roles.is_empty() {
                "(none)".to_string()
            } else {
                roles
                    .iter()
                    .map(|r| r.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        );
    }
    println!(
        "\nThe region above is the one the corpus demonstrates the piece at and the one \
         `delve-grammar audit --library` judges it at; a program expands over any region that \
         fits it. Minimum regions are documented per rule in docs/reference/grammar.md; a \
         region too small is a refusal, never a smaller building."
    );
    ExitCode::SUCCESS
}

/// Say what a document composed, on every command that reads one.
///
/// Printed whether the count is zero or not, and on stderr so it never lands in
/// `show`'s piped JSON. A composition is the one thing about a program that the
/// file on disk does not say — the rules arrived from somewhere else — so an
/// operator who is looking at a green verdict learns what the green was over.
fn composition_to_stderr(loaded: &Loaded) {
    if loaded.compositions.is_empty() {
        return;
    }
    eprintln!(
        "  {:<15} bound {:<6} composed program document(s)",
        "include",
        loaded.includes()
    );
    for c in &loaded.compositions {
        eprintln!(
            "      {}{} ← {} ({})",
            "  ".repeat(c.depth),
            c.prefix,
            c.source.display(),
            c.name
        );
    }
}

fn run_show(source: &Source) -> ExitCode {
    let loaded = match source.load() {
        Ok((_, l)) => l,
        Err(e) => return bad_input(e),
    };
    composition_to_stderr(&loaded);
    let program = loaded.program;
    match serde_json::to_string_pretty(&program) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal: serialise program: {e}");
            ExitCode::from(EXIT_OUTPUT)
        }
    }
}

fn run_check(source: &Source) -> ExitCode {
    let (id, loaded) = match source.load() {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };
    composition_to_stderr(&loaded);
    let program = loaded.program;
    match program.validate() {
        Ok(()) => {
            println!(
                "{id}: ok — {} rule(s), {} param(s), {} role(s). Structure only: expand it to \
                 learn whether it fits a region.",
                program.rules.len(),
                program.params.len(),
                program.palette.len()
            );
            ExitCode::SUCCESS
        }
        Err(e) => bad_input(format!("{id}: {e}")),
    }
}

fn run_coverage(json: Option<&Path>) -> ExitCode {
    let report = coverage::measure(library::PROGRAMS);

    println!(
        "demonstration coverage over {} library program(s): {}",
        report.measurements.programs, report.verdict
    );
    for c in &report.constructs {
        println!(
            "  {:<18} {}  bound {:<6} {}",
            c.id,
            if c.pass { "shown" } else { "NONE " },
            c.bound,
            match c.exempt {
                Some(why) => format!("{} [exempt: {why}]", c.detail),
                None => c.detail.clone(),
            }
        );
    }
    let m = &report.measurements;
    println!(
        "  measurements       {} rule(s) · {} alternative(s) · {} IR node(s) · {} palette role(s) \
         · {} mix(es) containing air · {} distinct block state(s)",
        m.rules,
        m.alternatives,
        m.ir_nodes,
        m.palette_roles,
        m.mixes_containing_air,
        m.distinct_block_states
    );
    for finding in &report.findings {
        println!("  finding: {finding}");
    }
    // Printed on every run, pass or fail, because the sentence exists to stop a
    // GREEN being cited for something it never measured.
    println!("\n{}", coverage::MEASURES);

    if let Some(path) = json {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            eprintln!("error: create {}: {e}", parent.display());
            return ExitCode::from(EXIT_OUTPUT);
        }
        if let Err(e) = std::fs::write(path, report.to_json()) {
            eprintln!("error: write {}: {e}", path.display());
            return ExitCode::from(EXIT_OUTPUT);
        }
    }

    if report.is_pass() {
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "error: {} construct(s) of the IR have no example in the corpus. Add one, or add an \
             exemption with its reason to `coverage::EXEMPT` — never shrink the required set.",
            report.constructs.iter().filter(|c| !c.pass).count()
        );
        ExitCode::from(EXIT_GATE)
    }
}

#[allow(clippy::too_many_arguments)]
fn run_expand(
    source: &Source,
    region: &str,
    seed: u64,
    params: &[String],
    roles: &[String],
    id_override: Option<&str>,
    options: gates::Options,
    out: &Path,
) -> ExitCode {
    let (default_id, loaded) = match source.load() {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };
    composition_to_stderr(&loaded);
    let mut program = loaded.program;

    // The id is settled and checked FIRST, before anything is expanded, judged,
    // printed or written.
    //
    // An id that cannot become a structure is a property of the inputs alone —
    // it needs no region, no seed and no expansion to detect — so checking it
    // late buys nothing and costs everything: a full gate report headed
    // `<id>: pass` reached the terminal, a `<id>.report.json` was written, and
    // the refusal came underneath it. A reader's eye stops at the word `pass`,
    // and the artifact that word appeared to be about was never written.
    let id = id_override.unwrap_or(&default_id).to_string();
    if !export::is_valid_id(&id) {
        return bad_input(format!(
            "{}\n  the id came from {}{}\n  nothing was expanded and nothing was written.",
            export::ExportError::BadId { id: id.clone() },
            if id_override.is_some() {
                "--id".to_string()
            } else {
                source.default_id_origin()
            },
            if id_override.is_some() {
                String::new()
            } else {
                "; pass --id <id> to name the prefab yourself".to_string()
            },
        ));
    }

    // The region is NOT checked against the structure-template cap here, and
    // that is deliberate: a region past it is not an error at all. `export_zone`
    // tiles it. The cap is a packaging fact about a file format and a creator's
    // design never bends to satisfy it, so the only
    // region this command refuses is a degenerate one, which `parse_region` and
    // `export_zone` between them already do.
    let size = match parse_region(region) {
        Ok(s) => s,
        Err(e) => return bad_input(e),
    };

    for spec in params {
        let (name, value) = match split_once_eq(spec, "--param") {
            Ok(p) => p,
            Err(e) => return bad_input(e),
        };
        let value: i64 = match value.trim().parse() {
            Ok(v) => v,
            Err(_) => return bad_input(format!("--param {spec:?}: {value:?} is not an integer")),
        };
        if let Err(e) = program.set_param(name.trim(), value) {
            return bad_input(format!("--param {spec:?}: {e}"));
        }
    }
    for spec in roles {
        let (name, value) = match split_once_eq(spec, "--role") {
            Ok(p) => p,
            Err(e) => return bad_input(e),
        };
        let block: BlockState = match value.trim().parse() {
            Ok(b) => b,
            Err(e) => return bad_input(format!("--role {spec:?}: {e}")),
        };
        // A `--role` override is a RESTYLE: it says which material, and the
        // syntax has no word for an axis frame. So it inherits the frame of the
        // binding it replaces — overriding a local-frame role with a
        // world-frame state would silently re-point every connection in the
        // piece, which is a different edit than the one being asked for.
        let paint = if program
            .palette
            .get(name.trim())
            .is_some_and(Paint::is_local)
        {
            Paint::local_block(block)
        } else {
            Paint::block(block)
        };
        if let Err(e) = program.set_role(name.trim(), paint) {
            return bad_input(format!("--role {spec:?}: {e}"));
        }
    }

    let opts = ExpandOptions::seeded(seed);
    let box3 = Box3::at_origin(size);

    // Judge before freezing: the report is about the expansion, and a red gate
    // must not leave a `.nbt` on disk for someone to pick up later.
    let expansion = match expand(&program, box3, &opts) {
        Ok(e) => e,
        Err(e) => return bad_input(format!("{id}: {e}")),
    };
    let report = gates::judge(&expansion, options);

    if let Err(e) = std::fs::create_dir_all(out) {
        eprintln!("error: create {}: {e}", out.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    let report_path = out.join(format!("{id}.report.json"));
    if let Err(e) = std::fs::write(&report_path, report.to_json()) {
        eprintln!("error: write {}: {e}", report_path.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    // `is_fail`, not `!is_pass`: an UNDECIDED report is not a red and must not
    // refuse the artifact. The gate could not examine something at this region;
    // the program may be entirely correct, and refusing here would be the
    // "reds on ordinary correct programs" failure arriving by the back door.
    // It is still shouted — `report_to_stderr` prints the verdict, the gate's
    // state and the finding that names every undecided object.
    if report.is_fail() {
        report_to_stderr(&id, &report);
        eprintln!(
            "error: {id}: a machine gate went red; no prefab was written. The report is at {}.",
            report_path.display()
        );
        return ExitCode::from(EXIT_GATE);
    }

    // `export_zone`, never `export_prefab`: how many files the zone lands in is
    // the toolchain's arithmetic, and a region an author chose is never the
    // wrong size.
    //
    // The verdict is printed only once the artifact it is a verdict about
    // exists. Freezing can still refuse an expansion every gate passed — a
    // program that paints a block a structure template may not carry is the
    // live case — and a `pass` headline above that refusal is worse than no
    // headline: it is the line the reader stops at.
    let exported = match export::export_zone(&program, box3, &opts, &id) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("error: {id}: {e}");
            eprintln!(
                "  no prefab was written. The gate report is at {} — its gates passed; this \
                 refusal is not a gate.",
                report_path.display()
            );
            return ExitCode::from(EXIT_INPUT);
        }
    };
    if let Err(e) = exported.write_to_dir(out) {
        eprintln!("error: write into {}: {e}", out.display());
        return ExitCode::from(EXIT_OUTPUT);
    }
    report_to_stderr(&id, &report);

    let structures = exported.structure_files();
    let grid = exported.grid();
    let written = if structures.len() == 1 {
        format!("{}/{}", out.display(), structures[0])
    } else {
        // Said plainly, because the operator should be able to see that the
        // packaging happened — and that it is packaging, not a smaller zone.
        eprintln!(
            "  packaging      {} tile(s) in a {}x{}x{} grid — the zone is past the {}-per-axis \
             structure-template cap, so it ships as a tile set and one manifest. Every gate above \
             judged the whole zone.",
            structures.len(),
            grid[0],
            grid[1],
            grid[2],
            export::MAX_STRUCTURE_AXIS
        );
        format!(
            "{}/ {} tile(s) in a {}x{}x{} grid",
            out.display(),
            structures.len(),
            grid[0],
            grid[1],
            grid[2]
        )
    };
    println!(
        "{written} + {} + {} — {}x{}x{}, seed {seed}, {} filled cell(s), {} anchor(s)",
        exported.metadata_file(),
        report_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("report"),
        size[0],
        size[1],
        size[2],
        report.measurements.filled_cells,
        report.anchors.len()
    );
    ExitCode::SUCCESS
}

/// Say what was judged, and say plainly what bound to nothing.
fn report_to_stderr(id: &str, report: &gates::Report) {
    eprintln!("{id}: {}", report.verdict);
    for gate in &report.gates {
        eprintln!(
            "  {:<15} {}  bound {:<6} {}",
            gate.id,
            match gate.state {
                gates::GateState::Pass => "pass",
                gates::GateState::Fail => "FAIL",
                gates::GateState::Undecided => "UNDECIDED",
            },
            gate.bound,
            gate.detail
        );
    }
    let m = &report.measurements;
    eprintln!(
        "  measurements   filled {} / {} cells · {} distinct states · {} standable · footprint \
         {} cols, perimeter {} (complexity {:.2})",
        m.filled_cells,
        m.region_cells,
        m.distinct_states,
        m.standable_cells,
        m.footprint_area,
        m.footprint_perimeter,
        m.silhouette_complexity
    );
    for (block, share) in &m.top_blocks {
        eprintln!("      {:>5.1}%  {block}", share * 100.0);
    }
    // The settling counts, printed on every expansion whether or not either
    // rule had anything to judge. A gate is emitted only over the objects it
    // judges, so THIS line is where a reader learns that a piece holds no
    // stair and no water — the difference between "the rule held here" and
    // "the rule said nothing here", which is the whole of the vacuity rule.
    eprintln!(
        "  settling       {} stair(s) · {} fluid cell(s), {} still (waterlogged), {} run \
         direction(s) leaving the piece",
        m.stairs, m.fluid_cells, m.fluid_held_cells, m.fluid_at_edge
    );
    // Printed on every expansion, gate or no gate. This is the whole binding of
    // the reachability work: a report nobody has to ask for cannot be the report
    // nobody ran (CLAUDE.md, "a gate nothing INVOKES is not a gate").
    let r = &m.reachability;
    eprintln!(
        "  reachability   {} of {} standable cell(s) reachable on foot from {} grade entry cell(s) \
         ({:.1}%) · {} sheltered · unreachable {} sheltered + {} open to the sky, in {} pocket(s)",
        r.reachable,
        r.standable,
        r.entry_cells,
        r.reachable_share * 100.0,
        r.sheltered,
        r.unreachable_sheltered,
        r.unreachable_open,
        r.pockets
    );
    for pocket in &r.largest_pockets {
        eprintln!("      pocket  {}", pocket.describe());
    }
    // Every opt-out the contract used, by name and one per line. A count of
    // out-of-walk regions is a number a blind script can satisfy; a list saying
    // which shelf is `posted` on which anchors, and which bar the walk had to
    // open, is a thing a reviewer reads and can disagree with.
    for line in &report.enumeration {
        eprintln!("  contract: {line}");
    }
    for finding in &report.findings {
        eprintln!("  finding: {finding}");
    }
}

// ---------------------------------------------------------------------------
// audit — the sweep over a whole corpus
// ---------------------------------------------------------------------------

/// A campaign's zone manifest: `design/programs/zones.json`.
///
/// A grammar program is region-polymorphic, so a program file alone cannot be
/// expanded — something has to say at what size and seed this campaign builds
/// it. That fact lived in prose (`design/GENERATION.md` said "expands at
/// 20x10x84"), which is why nothing could check a zone program: the machine had
/// no way to know what to expand. This is that fact, in a file.
#[derive(serde::Deserialize)]
struct ZoneManifest {
    zones: Vec<ZoneEntry>,
}

/// One zone: which program file, at which expansion, claiming which gates.
#[derive(serde::Deserialize)]
struct ZoneEntry {
    /// The zone's id in the campaign — what the round summary calls it.
    id: String,
    /// The program file, relative to the manifest.
    program: String,
    /// `[X, Y, Z]`.
    region: [u32; 3],
    seed: u64,
    /// The zone claims a body walks it from its approach face to its exit face.
    #[serde(default)]
    traversable: bool,
    /// With `traversable`, the route may include a fall (a one-way descent).
    #[serde(default)]
    allow_falls: bool,
    /// The zone claims a body can reach every piece of roofed floor in it.
    #[serde(default)]
    reachable_floor: bool,
    /// The zone claims bilateral symmetry about the mid-plane of this world
    /// axis (`x`, `y` or `z`).
    ///
    /// Declared here beside the other three claims rather than left to a flag
    /// on a command line: `audit` is what runs the campaign corpus, and a claim
    /// a campaign has no way to state is a gate that binds zero over every zone
    /// there will ever be.
    #[serde(default)]
    symmetric: Option<String>,
}

/// The name a manifest must have, beside the programs it governs.
const ZONE_MANIFEST: &str = "zones.json";

/// One thing to audit: what to call it, what to expand, where, at which seed,
/// which optional gates it claims, how many program documents it composed to
/// become the program that is expanded, and which of those composed documents
/// carry a manifest row of their own.
struct AuditItem {
    label: String,
    program: Program,
    region: [u32; 3],
    seed: u64,
    gates: gates::Options,
    /// The include surface's binding count for this item.
    includes: usize,
    /// spec-0040 §3c link 4: every composed part that ALSO has its own row.
    allocations: Vec<AllocationClaim>,
}

/// **A part that is composed here and has a manifest row of its own** — the one
/// population §3c link 4 is about.
///
/// The row states the extent the part is developed, reviewed and judged at
/// standalone; the composition states the extent the whole's site plan hands it.
/// Two documents, one fact. When they disagree, the part was reviewed in a box
/// that is not the box it is built in, and the review certified a different
/// object — which is how a citadel came out of its parts at 1 : 5.5 against a
/// brief asking for a compact stepped mass (spec-0040 §1.9).
struct AllocationClaim {
    /// The prefix path the composed vocabulary carries in the resolved program.
    prefix: String,
    /// The composed document's own manifest id.
    row_id: String,
    /// The extents that row declares — the part's own frame, since a standalone
    /// expansion runs at the identity orientation.
    row_region: [u32; 3],
}

/// What one gate id totalled to across the corpus.
#[derive(Default)]
struct GateTotals {
    /// Objects examined, summed.
    objects: usize,
    /// Programs the gate ran on.
    programs: usize,
    /// Programs it went red on.
    red: usize,
    /// Objects it could not decide, summed — a binding count in its own right,
    /// and the one that says a corpus never exercises the surface.
    undecided_objects: usize,
    /// Programs holding at least one undecided object.
    undecided_programs: usize,
}

/// One audited program's outcome, so the caller can total binding counts.
struct Audited {
    label: String,
    report: gates::Report,
    /// How many [`AllocationClaim`]s this program's expansion was compared
    /// against — the compared-row count, per program.
    compared_rows: usize,
    /// Every §3c link-4 disagreement, one line each.
    allocation_reds: Vec<String>,
}

/// Programs that are KNOWN red, and the exact codes each must fail with.
#[derive(serde::Deserialize)]
struct Exclusions {
    exclusion: Vec<Exclusion>,
}

/// One recorded red.
#[derive(serde::Deserialize)]
struct Exclusion {
    /// The audit label, e.g. `the-drowned-bell-r2/z3-drowned-ward`.
    id: String,
    /// What is missing that keeps it red. An exclusion is a capability-gap
    /// record, not a permission slip, so it names the gap in words a later
    /// session can act on.
    capability_gap: String,
    /// Every diagnostic code the program must fail with — exactly this set.
    expect_codes: Vec<String>,
}

/// Which recorded codes appear in a report's failing gates.
///
/// `failed`, not `!passed`: an exclusion records what a program is RED with,
/// and an undecided gate is not red. Reading `DW0742` in here would let a
/// program hold a known-red record on a code that refuses nothing.
fn failing_codes(report: &gates::Report) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for gate in report.gates.iter().filter(|g| g.failed()) {
        for word in gate.detail.split(|c: char| !c.is_ascii_alphanumeric()) {
            if word.len() == 6
                && word.starts_with("DW")
                && word[2..].bytes().all(|b| b.is_ascii_digit())
            {
                out.insert(word.to_string());
            }
        }
    }
    out
}

fn read_exclusions(path: &Path) -> Result<Vec<Exclusion>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let parsed: Exclusions =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: parse: {e}", path.display()))?;
    for e in &parsed.exclusion {
        if e.expect_codes.is_empty() {
            return Err(format!(
                "{}: exclusion {:?} names no expect_codes, so any failure at all would satisfy \
                 it — which is a skip wearing an exclusion's clothes",
                path.display(),
                e.id
            ));
        }
        if e.capability_gap.trim().is_empty() {
            return Err(format!(
                "{}: exclusion {:?} names no capability gap. An exclusion records what is \
                 MISSING; one that records nothing is a permission slip",
                path.display(),
                e.id
            ));
        }
    }
    Ok(parsed.exclusion)
}

/// The diagnostic a §3c link-4 disagreement carries.
const DW_ALLOCATION_IDENTITY: &str = "DW0806";

fn audit_one(
    label: String,
    program: &Program,
    region: [u32; 3],
    seed: u64,
    opts: gates::Options,
    claims: &[AllocationClaim],
) -> Result<Audited, String> {
    let expansion = expand(
        program,
        Box3::at_origin(region),
        &ExpandOptions::seeded(seed),
    )
    .map_err(|e| {
        format!(
            "{label}: expand at {}x{}x{}: {e}",
            region[0], region[1], region[2]
        )
    })?;
    let allocation_reds = compare_allocations(&label, &expansion, claims);
    Ok(Audited {
        label,
        report: gates::judge(&expansion, opts),
        compared_rows: claims.len(),
        allocation_reds,
    })
}

/// **spec-0040 §3c link 4**: the part's own row names the same box.
///
/// Compares EXTENTS and never origins — a part developed standalone sits
/// wherever it likes, and where it was parked says nothing about how big it is —
/// and compares them in the **part's own frame**
/// ([`grammar::Allocation::local`]), which is what "up to the include site's
/// declared reorientation" means: a composition that turns a part a quarter-turn
/// about the vertical hands it a world box with X and Z swapped, and the part
/// still reads exactly the extents it was built at. Comparing world extents
/// would red a correct composition for having rotated something.
///
/// Every distinct box the prefix was entered at is compared, not one of them: a
/// part the composition places twice at two sizes has two allocations and at
/// most one of them can be the box its row names.
fn compare_allocations(
    label: &str,
    expansion: &Expansion,
    claims: &[AllocationClaim],
) -> Vec<String> {
    let mut reds = Vec::new();
    for claim in claims {
        let [rx, ry, rz] = claim.row_region;
        let sites = expansion.allocations.get(&claim.prefix);
        // No box at all is the strongest disagreement, not a skip: the row was
        // compared against nothing, which is the vacuous green this identity
        // exists to end. It happens when the rule that would place the part sits
        // under a guard this seed does not take — a composed part nothing places.
        let Some(sites) = sites.filter(|s| !s.is_empty()) else {
            reds.push(format!(
                "{DW_ALLOCATION_IDENTITY}: {label} composes {:?} under the prefix {:?} and no \
                 rule of that prefix is ever entered, so the composition allocates it NO box and \
                 the row's declared {rx}x{ry}x{rz} was compared against nothing. A composed part \
                 that is never placed is not a part of this map",
                claim.row_id, claim.prefix
            ));
            continue;
        };
        for site in sites {
            let [ax, ay, az] = site.local;
            if site.local == claim.row_region {
                continue;
            }
            reds.push(format!(
                "{DW_ALLOCATION_IDENTITY}: {label} allocates the prefix {:?} a box of \
                 {ax}x{ay}x{az} in the part's own frame (world box corner {},{},{} size \
                 {}x{}x{}), and zones.json declares {:?} at {rx}x{ry}x{rz}. Extent flows from \
                 the whole's brief down to the part (spec-0040 §3c), so the box the composition \
                 allocates IS the box the part is reviewed in — a row naming a different one \
                 certifies a different object. Repair the part to its allocation, or revise the \
                 site plan deliberately (the whole's own identities re-run over the revision); \
                 never grow the container to what the parts sum to",
                claim.prefix,
                site.origin[0],
                site.origin[1],
                site.origin[2],
                site.size[0],
                site.size[1],
                site.size[2],
                claim.row_id,
            ));
        }
    }
    reds
}

/// Collect every campaign zone program under a content-repo root, with the
/// expansion its campaign declares.
///
/// Reds rather than skips in three shapes, all of which are how this sweep
/// would otherwise go quietly dark: a `design/programs/` directory holding
/// programs and no manifest, a program file no manifest entry names, and a
/// manifest entry naming a file that is not there. The first is the one that
/// matters — without it, "add a zone program" and "add a zone program the gate
/// never sees" are the same action.
fn collect_campaign_zones(root: &Path) -> Result<Vec<AuditItem>, Vec<String>> {
    let mut out: Vec<AuditItem> = Vec::new();
    let mut errors = Vec::new();
    let campaigns = root.join("campaigns");
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(&campaigns) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(e) => {
            return Err(vec![format!(
                "{}: {e} — --campaign-root wants the root of a content repo (the directory \
                 holding `campaigns/`), not the campaigns directory itself",
                campaigns.display()
            )]);
        }
    };
    dirs.sort();
    for campaign in dirs {
        let programs = campaign.join("design").join("programs");
        if !programs.is_dir() {
            continue;
        }
        let name = campaign
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let mut files: Vec<PathBuf> = match std::fs::read_dir(&programs) {
            Ok(rd) => rd
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
                .filter(|p| p.file_name().and_then(|s| s.to_str()) != Some(ZONE_MANIFEST))
                .collect(),
            Err(e) => {
                errors.push(format!("{}: {e}", programs.display()));
                continue;
            }
        };
        files.sort();
        let manifest_path = programs.join(ZONE_MANIFEST);
        let manifest: ZoneManifest = match std::fs::read(&manifest_path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(m) => m,
                Err(e) => {
                    errors.push(format!("{}: parse: {e}", manifest_path.display()));
                    continue;
                }
            },
            Err(_) if files.is_empty() => continue,
            Err(e) => {
                errors.push(format!(
                    "{}: {e} — {name} holds {} zone program(s) and no manifest, so nothing \
                     states the region and seed they are built at and nothing can check them. \
                     Add `{ZONE_MANIFEST}` naming every one",
                    manifest_path.display(),
                    files.len()
                ));
                continue;
            }
        };

        // What each manifest row declares, by the program file it names — the
        // other half of §3c link 4's identity. Built before the sweep, because
        // a composition and the row of the document it composes are two entries
        // of one file and either may be written first.
        let mut rows_by_file: BTreeMap<PathBuf, Vec<(&str, [u32; 3])>> = BTreeMap::new();
        for zone in &manifest.zones {
            rows_by_file
                .entry(document::normalised_path(&programs.join(&zone.program)))
                .or_default()
                .push((zone.id.as_str(), zone.region));
        }

        let mut named: BTreeSet<PathBuf> = BTreeSet::new();
        // Every program file some manifest-named zone actually COMPOSED. A
        // composed part is judged inside its composition, so it is not itself a
        // manifest entry — but this is not an author's declaration that it is
        // covered. It is the loader's record of having read it, which a program
        // file nothing composes cannot produce. A file that no manifest names
        // and no document composes is still the red below.
        let mut composed_files: BTreeSet<PathBuf> = BTreeSet::new();
        for zone in &manifest.zones {
            let path = programs.join(&zone.program);
            named.insert(document::normalised_path(&path));
            // The same loader `--file` uses, and for the same reason: a zone
            // program may compose another program document, and `audit` is the
            // sweep that judges the campaign corpus. Reading these two entry
            // points differently is how a surface ends up checked on one path
            // and silently unresolved on the other.
            let loaded = match document::load(&path) {
                Ok(l) => l,
                Err(e) => {
                    errors.push(format!(
                        "{}: zone {:?}: {e}",
                        manifest_path.display(),
                        zone.id
                    ));
                    continue;
                }
            };
            let composed_here = loaded.includes();
            let mut allocations: Vec<AllocationClaim> = Vec::new();
            for c in &loaded.compositions {
                let key = document::normalised_path(&c.source);
                // The same normalisation the loader answered with. Two different
                // normalisations is how "is this the file that document
                // composed?" comes out `no` for a file that plainly is.
                for (row_id, row_region) in rows_by_file.get(&key).into_iter().flatten() {
                    allocations.push(AllocationClaim {
                        prefix: c.path.clone(),
                        row_id: (*row_id).to_string(),
                        row_region: *row_region,
                    });
                }
                composed_files.insert(key);
            }
            let program = loaded.program;
            let symmetric = match zone.symmetric.as_deref().map(parse_axis).transpose() {
                Ok(a) => a,
                Err(e) => {
                    errors.push(format!(
                        "{}: zone {:?}: symmetric: {e}",
                        manifest_path.display(),
                        zone.id
                    ));
                    continue;
                }
            };
            out.push(AuditItem {
                label: format!("{name}/{}", zone.id),
                program,
                region: zone.region,
                seed: zone.seed,
                gates: gates::Options {
                    traversable: zone.traversable,
                    allow_falls: zone.allow_falls,
                    symmetric,
                    reachable_floor: zone.reachable_floor,
                },
                includes: composed_here,
                allocations,
            });
        }
        for file in &files {
            let key = document::normalised_path(file);
            if named.contains(&key) || composed_files.contains(&key) {
                continue;
            }
            errors.push(format!(
                "{}: no entry in {ZONE_MANIFEST} names it and no zone program composes it, so it \
                 is a program nothing expands and nothing checks",
                file.display()
            ));
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

fn run_audit(
    library_flag: bool,
    campaign_roots: &[PathBuf],
    exclusions: Option<&Path>,
) -> ExitCode {
    if !library_flag && campaign_roots.is_empty() {
        return bad_input(
            "give --library, --campaign-root <PATH>, or both — an audit of nothing is not a pass",
        );
    }
    let recorded: Vec<Exclusion> = match exclusions {
        Some(path) => match read_exclusions(path) {
            Ok(v) => v,
            Err(e) => return bad_input(e),
        },
        None => Vec::new(),
    };

    let mut work: Vec<AuditItem> = Vec::new();
    if library_flag {
        for lib in library::PROGRAMS {
            work.push(AuditItem {
                label: format!("library/{}", lib.id),
                program: (lib.build)(),
                region: lib.region,
                seed: lib.seed,
                gates: lib.gates,
                // A library program is Rust, so it composes with
                // `compose::include` and never with a document include. Zero
                // here is a fact about the corpus, not a gap in the sweep, and
                // the summary line below says which corpus the total came from.
                includes: 0,
                // And it has no manifest at all, so no row of it can name a box.
                allocations: Vec::new(),
            });
        }
    }
    // The two corpora are counted apart, because they have different owners and
    // a zero means a different thing in each. The LIBRARY lives in this repo, so
    // its size is a fact of this tree and a zero is a defect here. The CAMPAIGN
    // corpus lives in the content repo, where an in-progress campaign sits on its
    // own development branch, so a root that carries no zone program is a fact
    // about that checkout. Summing them lets a full library carry an empty
    // campaign root to green with nothing in the output naming it, which is what
    // the pinned content did.
    let library_count = work.len();
    for root in campaign_roots {
        match collect_campaign_zones(root) {
            Ok(mut zones) => work.append(&mut zones),
            Err(errors) => {
                for e in &errors {
                    eprintln!("error: {e}");
                }
                return ExitCode::from(EXIT_GATE);
            }
        }
    }
    let campaign_count = work.len() - library_count;

    // A sweep that found nothing to sweep is the shape this whole command
    // exists to stop, so it is a red and never a silent zero.
    if work.is_empty() {
        eprintln!(
            "error: the audit found 0 programs. Pointed at {} campaign root(s){} — a corpus of \
             nothing cannot pass.",
            campaign_roots.len(),
            if library_flag {
                " plus the library"
            } else {
                ""
            }
        );
        return ExitCode::from(EXIT_GATE);
    }

    // Stated before any verdict, so the reader of a green log knows what the
    // green was over.
    if library_flag {
        println!("corpus: library {library_count} program(s)");
    }
    if !campaign_roots.is_empty() {
        println!(
            "corpus: campaign {campaign_count} program(s) over {} root(s){}",
            campaign_roots.len(),
            if campaign_count == 0 {
                " — FINDING: zero binding, no campaign zone program was examined. Which \
                 campaigns a checkout is expected to carry, and how many zone programs each \
                 declares, is enumerated per pin in the pipeline repo's \
                 .github/content-zone-corpus.json and checked against the tree there; this \
                 command judges programs, not whether the right ones are present"
            } else {
                ""
            }
        );
    }
    // The library floor, which nothing outside this repo can move: `--library`
    // asks for a corpus this tree defines, so an empty one is this tree's defect
    // and never a fact about somebody's checkout.
    if library_flag && library_count == 0 {
        eprintln!(
            "error: --library found 0 programs. The rule library is this repo's own corpus \
             (library::PROGRAMS); an empty one is a defect here, not a fact about a checkout."
        );
        return ExitCode::from(EXIT_GATE);
    }

    let mut audited: Vec<Audited> = Vec::new();
    let mut failed = false;
    let mut document_includes = 0usize;
    let mut composing_programs = 0usize;
    // The compared-row count, offered and examined. Two numbers rather than one
    // because they can only differ one way — a program whose claims were built
    // and whose expansion then failed — and a binding count that silently
    // shrinks when something upstream broke is a binding count nobody can read.
    let mut rows_offered = 0usize;
    let mut rows_examined = 0usize;
    for item in work {
        document_includes += item.includes;
        composing_programs += usize::from(item.includes > 0);
        rows_offered += item.allocations.len();
        match audit_one(
            item.label,
            &item.program,
            item.region,
            item.seed,
            item.gates,
            &item.allocations,
        ) {
            Ok(a) => {
                rows_examined += a.compared_rows;
                audited.push(a);
            }
            Err(e) => {
                eprintln!("error: {e}");
                failed = true;
            }
        }
    }

    // §3c link 4, printed with the verdicts rather than folded into a gate: the
    // identity is between a manifest row and a composition, and neither of those
    // is a thing `judge` can see from an expansion's blocks.
    for a in &audited {
        for red in &a.allocation_reds {
            failed = true;
            println!("  {:<44} FAIL", a.label);
            println!("      {:<16} FAIL  {red}", "part-allocation");
        }
    }

    // Per-gate totals across the corpus, in first-seen order: the binding count
    // is the point of the report, and a gate that examined zero objects across
    // the whole corpus is the vacuous green this project has shipped five times.
    let mut order: Vec<&'static str> = Vec::new();
    let mut bound: std::collections::BTreeMap<&'static str, GateTotals> = Default::default();
    for a in &audited {
        for g in &a.report.gates {
            if !order.contains(&g.id) {
                order.push(g.id);
            }
            let e = bound.entry(g.id).or_default();
            e.objects += g.bound;
            e.programs += 1;
            if g.failed() {
                e.red += 1;
            }
            // Counted apart from `red`, because it is a different answer. A
            // gate that could not decide anywhere in the corpus is a surface
            // the corpus never exercises — the vacuity a summed binding count
            // hides, one level down.
            e.undecided_objects += g.undecided;
            if g.undecided > 0 {
                e.undecided_programs += 1;
            }
        }
    }

    let mut held_red = 0usize;
    let mut undecided_programs = 0usize;
    for a in &audited {
        // `g.failed()` and nothing else. This used to read
        // `g.failed() || g.bound == 0`, which made the audit a SECOND authority
        // on what a zero binding means: `contract-edge-proof` had already
        // decided that one space and no interior edge is an honest zero, and
        // this line overruled it — while the door a creator runs on their own
        // machine passed the same program. The rule now lives in
        // `gates::seal_zero_bindings`, which every door reads: an unjustified
        // zero binding arrives here already red, and a justified one is not a
        // gate in this report at all.
        let bad: Vec<&gates::Gate> = a.report.gates.iter().filter(|g| g.failed()).collect();
        // Never folded into `bad`: an undecided gate refuses nothing and must
        // not red the sweep, or the third answer becomes a fail with a softer
        // name. It is printed per program, by name, so it cannot be lost.
        let unsure: Vec<&gates::Gate> = a
            .report
            .gates
            .iter()
            .filter(|g| g.state == gates::GateState::Undecided)
            .collect();
        let recorded_here = recorded.iter().find(|e| e.id == a.label);

        if let Some(entry) = recorded_here {
            // The inversion: a recorded program MUST fail, and must fail with
            // exactly the codes recorded. A pass is a finding (the record has
            // expired and the gate would otherwise stay inverted forever); a
            // different code is a finding (a second defect hiding behind the
            // first).
            let saw = failing_codes(&a.report);
            let want: BTreeSet<String> = entry.expect_codes.iter().cloned().collect();
            if saw == want && !bad.is_empty() {
                held_red += 1;
                println!(
                    "  {:<44} known-red  {}  [{}]",
                    a.label,
                    entry.expect_codes.join(" "),
                    entry.capability_gap
                );
                continue;
            }
            failed = true;
            println!(
                "  {:<44} RECORD WRONG — expected to fail with {:?}, {}",
                a.label,
                want,
                if bad.is_empty() {
                    "it PASSED: the record has expired and must be deleted".to_string()
                } else {
                    format!("it failed with {saw:?}")
                }
            );
            for g in &bad {
                println!("      {:<16} bound {:<6} {}", g.id, g.bound, g.detail);
            }
            continue;
        }

        if bad.is_empty() {
            if unsure.is_empty() {
                println!("  {:<44} pass  {} gate(s)", a.label, a.report.gates.len());
            } else {
                undecided_programs += 1;
                println!(
                    "  {:<44} UNDECIDED  {} gate(s), {} of them could not decide at this region",
                    a.label,
                    a.report.gates.len(),
                    unsure.len()
                );
                for g in &unsure {
                    println!(
                        "      {:<16} undecided {:<4} of bound {:<6} {}",
                        g.id, g.undecided, g.bound, g.detail
                    );
                }
            }
            continue;
        }
        failed = true;
        println!("  {:<44} FAIL", a.label);
        for g in bad {
            println!("      {:<16} FAIL  bound {:<6} {}", g.id, g.bound, g.detail);
        }
        for g in &unsure {
            println!(
                "      {:<16} undecided {:<4} of bound {:<6} {}",
                g.id, g.undecided, g.bound, g.detail
            );
        }
    }

    // A record naming a program the sweep never saw is stale, and a stale
    // record is how an inversion outlives the thing it inverted.
    for entry in &recorded {
        if !audited.iter().any(|a| a.label == entry.id) {
            println!(
                "  RECORD STALE — {:?} is recorded known-red and the audit did not see it",
                entry.id
            );
            failed = true;
        }
    }

    println!(
        "\naudited {} program(s), {held_red} of them held known-red, {undecided_programs} of them \
         UNDECIDED at their declared region:",
        audited.len()
    );
    // The local frame's own binding count, beside the gate whose population it
    // takes from. A zero here across a whole corpus is a finding in exactly the
    // way a zero-bound gate is: the surface exists and nothing exercises it.
    let local_frame: usize = audited
        .iter()
        .map(|a| a.report.measurements.local_frame_fills)
        .sum();
    let local_frame_programs = audited
        .iter()
        .filter(|a| a.report.measurements.local_frame_fills > 0)
        .count();
    println!(
        "  {:<16} bound {:<8} over {:<3} program(s){}",
        "local-frame",
        local_frame,
        local_frame_programs,
        if local_frame == 0 {
            " — FINDING: zero binding, no program writes a state in its own axis frame"
        } else {
            ""
        }
    );
    // The document-include surface's binding count, over the corpus where the
    // surface can exist at all.
    //
    // Scoped to the campaign corpus deliberately, and this is not an opt-out: a
    // library program is `fn() -> Program`, so it has no document to compose
    // another document FROM, and a zero over it would be a category error
    // printed as a finding. A library entry that did carry an unresolved
    // include is not quietly excused either — it reds at expansion, because
    // `validate` refuses an unresolved include by name.
    //
    // A zero over the campaign corpus is stated and is NOT a red: a campaign is
    // entitled to one program per zone. The number is how a later reader learns
    // the surface exists and that nothing here reached it.
    if !campaign_roots.is_empty() {
        println!(
            "  {:<16} bound {:<8} over {:<3} campaign program(s){}",
            "include",
            document_includes,
            composing_programs,
            if document_includes == 0 {
                " — nothing in this corpus composes another program document"
            } else {
                ""
            }
        );
    }
    // The compared-row count of §3c link 4's identity, over the corpus where a
    // manifest exists at all.
    //
    // Stated whether it is zero or not, and a zero is stated BY NAME: until this
    // line existed, the ordering that keeps a part from deciding the whole's
    // extent rested on links 1-3 alone (a brief guarded over the region, a split
    // that partitions, a part that refuses a box too small) and nothing in the
    // output said which. A zero here is not a red — a campaign is entitled to
    // compose only parts that have no row of their own, and §3c says such a part
    // is judged at its allocation and nowhere else, so the ordering is
    // structural for it. What a zero is, is a fact a reader has to be given.
    if !campaign_roots.is_empty() {
        println!(
            "  {:<16} bound {:<8} composed part(s) with a manifest row of their own{}",
            "part-allocation",
            rows_examined,
            if rows_examined == 0 {
                " — nothing in this corpus composes a part that also has its own row, so \
                 spec-0040 §3c's ordering rests here on links 1-3 alone"
                    .to_string()
            } else if rows_offered != rows_examined {
                failed = true;
                format!(
                    " — FINDING: {rows_offered} row(s) were set up for comparison and \
                     {rows_examined} were compared; the difference is a program that did not \
                     expand, and its rows were checked against nothing"
                )
            } else {
                String::new()
            }
        );
    }
    for id in &order {
        let t = &bound[id];
        // The undecided binding is printed beside the bound one, always, and
        // not only when it is non-zero — a count that appears only when it is
        // interesting is a count nobody learns to read.
        println!(
            "  {:<16} bound {:<8} undecided {:<6} over {:<3} program(s){}",
            id,
            t.objects,
            t.undecided_objects,
            t.programs,
            if t.red > 0 {
                format!(" — {} RED", t.red)
            } else if t.objects == 0 {
                " — FINDING: zero binding, this gate examined nothing".to_string()
            } else if t.undecided_programs > 0 {
                format!(
                    " — UNDECIDED in {} program(s); this corpus does not exercise the surface \
                     the gate is pointed at",
                    t.undecided_programs
                )
            } else {
                String::new()
            }
        );
        if t.objects == 0 {
            failed = true;
        }
    }

    if failed {
        eprintln!(
            "error: the zone-program audit went red. Nothing was written; the verdicts above are \
             the whole output."
        );
        ExitCode::from(EXIT_GATE)
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::List => run_list(),
        Command::Audit {
            library,
            campaign_roots,
            exclusions,
        } => run_audit(library, &campaign_roots, exclusions.as_deref()),
        Command::Show { source } => run_show(&source),
        Command::Check { source } => run_check(&source),
        Command::Coverage { json } => run_coverage(json.as_deref()),
        Command::Expand {
            source,
            region,
            seed,
            params,
            roles,
            id,
            traversable,
            allow_falls,
            symmetric,
            reachable_floor,
            out,
        } => {
            if allow_falls && !traversable {
                return bad_input("--allow-falls only means something with --traversable");
            }
            let symmetric = match symmetric.as_deref().map(parse_axis).transpose() {
                Ok(a) => a,
                Err(e) => return bad_input(e),
            };
            run_expand(
                &source,
                &region,
                seed,
                &params,
                &roles,
                id.as_deref(),
                gates::Options {
                    traversable,
                    allow_falls,
                    symmetric,
                    reachable_floor,
                },
                &out,
            )
        }
    }
}
