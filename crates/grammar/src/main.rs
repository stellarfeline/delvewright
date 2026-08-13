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
//! Five commands. The first four are the steps of the loop; the last asks what
//! the corpus those steps start from actually demonstrates.
//!
//! ```text
//! delve-grammar list                                  # what can be built
//! delve-grammar show --program store-room > scene.json # start from the corpus
//! delve-grammar check --file scene.json                # does the program hold together
//! delve-grammar expand --file scene.json --region 11x6x13 -o out/   # build + judge + freeze
//! delve-grammar coverage                              # what no example demonstrates
//! ```
//!
//! `--file` takes the typed JSON IR, which is the authoring form spec-0027 §3
//! names: an LLM writes that file, and nothing between it and the `.nbt` is
//! hand-assembled.
//!
//! Exit codes (mirroring `delve-schem` and `delve-render`): `0` ok · `2`
//! input/usage · `3` output · `4` a machine gate went red.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use delvewright_grammar::block::BlockState;
use delvewright_grammar::coverage;
use delvewright_grammar::gates;
use delvewright_grammar::ir::{Paint, Program};
use delvewright_grammar::{Axis, Box3, ExpandOptions, expand, export, library};

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
    fn load(&self) -> Result<(String, Program), String> {
        match (&self.program, &self.file) {
            (Some(id), None) => match library::by_id(id) {
                Some(p) => Ok((id.clone(), p)),
                None => Err(format!(
                    "no library program {id:?} — `delve-grammar list` names them all"
                )),
            },
            (None, Some(path)) => {
                let bytes =
                    std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
                let program: Program = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("parse {}: {e}", path.display()))?;
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("program")
                    .to_string();
                Ok((id, program))
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
    for (id, build) in library::PROGRAMS {
        let program = build();
        let params: Vec<String> = program
            .params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let roles: Vec<&String> = program.palette.keys().collect();
        println!("  {id}");
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
        "\nA program's minimum region is documented per rule in docs/reference/grammar.md; a \
         region too small is a refusal, never a smaller building."
    );
    ExitCode::SUCCESS
}

fn run_show(source: &Source) -> ExitCode {
    let (_, program) = match source.load() {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };
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
    let (id, program) = match source.load() {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };
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
    let (default_id, mut program) = match source.load() {
        Ok(p) => p,
        Err(e) => return bad_input(e),
    };

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
    // design never bends to satisfy it (owner ruling, 2026-08-12), so the only
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
        if let Err(e) = program.set_role(name.trim(), Paint::Block(block)) {
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
    if !report.is_pass() {
        report_to_stderr(&id, &report);
        eprintln!(
            "error: {id}: a machine gate went red; no prefab was written. The report is at {}.",
            report_path.display()
        );
        return ExitCode::from(EXIT_GATE);
    }

    // `export_zone`, never `export_prefab`: how many files the zone lands in is
    // the toolchain's arithmetic, and a region an author chose is never the
    // wrong size (owner ruling, 2026-08-12).
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
            if gate.pass { "pass" } else { "FAIL" },
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
    for finding in &report.findings {
        eprintln!("  finding: {finding}");
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::List => run_list(),
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
