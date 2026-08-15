//! Reading a grammar program **document**: the one place a file becomes a
//! [`Program`], and therefore the one place an `include` is resolved.
//!
//! # Why this module exists
//!
//! [`crate::compose::include_renaming`] composes one `Program` into another and
//! has done since the first zone was written. It is a Rust API, and a `&Program`
//! is obtainable only from a Rust constructor — spec-0040 §1.2 measured the
//! consequence: there is no document or CLI surface for include, so composition
//! happens only in engine source, and a creator authoring JSON, which is the
//! only surface a creator has, cannot compose two program files at all. Every
//! campaign is therefore stuck at one program per zone.
//!
//! spec-0040 §6.2 names the capability that closes it and allows two forms — a
//! manifest-driven compose step (CLI), **or** an include block in the `Program`
//! document fenced at the next `Program` version (ADR-0018 §7). This is the
//! second: a manifest step would put composition outside the artifact of record,
//! and only the document form composes *any* program into *any* program. The
//! spec's conditions on it are that the refusals, the anchor-rename rule and the
//! seam byte-identity promise are the ones `compose` already has, which is why
//! nothing below reimplements any of them.
//!
//! The missing piece is not a composition mechanism. It is the **file half**: an
//! `include` names another document, resolving it reads that document, and
//! reading a file is exactly what [`Program::validate`] must never do. So the
//! split is:
//!
//! ```text
//!   ir.rs         the `include` list — data, no I/O, refused if unresolved
//!   compose.rs    Program × Program → Program — no I/O, no RNG, no environment
//!   document.rs   path → Program — the only I/O, and the only recursion
//! ```
//!
//! Everything a composition *means* is [`crate::compose`]'s, unchanged: the same
//! prefix rewrite, the same anchor rule (a prefix never touches an anchor; a
//! rename is explicit and per-stem), the same refusals, and the same seam
//! promise pinned by `tests/compose.rs`. This module reads files, orders the
//! work, and refuses what only a file can be wrong about.
//!
//! # What only a file can be wrong about
//!
//! * an **absolute path**, which builds on one machine and no other;
//! * a **`\` separator**, which is not a path on the platform the pinned server
//!   image runs;
//! * a **cycle**, which a pure composition cannot express and a file reference
//!   can — refused with the whole chain named, never as a stack overflow;
//! * a **repeated prefix**, which `compose` would meet as a clash on whichever
//!   rule name happened to sort first, blaming a rule for a decision the include
//!   list made.
//!
//! # What is deliberately NOT checked here: the two documents' versions
//!
//! A composed document is validated **against the version it declares**, before
//! any of it is copied, so its own declared number stays honest — a document
//! that says `1.0.0` and writes a `bind` is refused where it is written, and
//! composition cannot launder it. What is not checked is the *relation* between
//! the two numbers, because the existing fence already decides it: a composition
//! is one program carrying the destination's `version`, and
//! [`Program::validate`] walks it at that version, so a construct the
//! destination may not write is refused by name wherever it came from. A second
//! rule comparing the two numbers would be a weaker private copy of that — it
//! would name a version instead of the construct — and today it could not fire
//! at all, since an `include` requires the newest version the ledger has.
//!
//! # Determinism
//!
//! Nothing here draws from the RNG or reads the environment. Includes are
//! applied in document order; every name a composition carries lands in a
//! `BTreeMap`, so the resolved program is independent of that order and the same
//! documents resolve to the same bytes (ADR-0006). The one thing order decides
//! is which of two colliding claims is reported first.
//!
//! # The seam's bound, stated because a test must not assume the unbounded form
//!
//! An included program expanded over the same box builds byte-identically to the
//! program alone **when nothing was drawn from the stream before it**. The
//! seeded stream is one sequential splitmix64 consumed in traversal order
//! (`crate::rng`), so a sibling that draws earlier shifts every later draw:
//! geometry from mutually exclusive guards is unaffected, weighted mixes
//! re-texture. spec-0040 §1.4 established that by probe — two programs identical
//! except that an earlier sibling draws produce different bytes inside the same
//! called piece — and §5 records what it costs: texture is composition-relative,
//! so a per-zone accepted render certifies that zone's geometry, palette and
//! distribution but not its exact texture bytes, and the review that certifies
//! appearance is the composed one.
//!
//! A composition is therefore byte-identical to its parts only in the shape the
//! seam test pins — a host that draws nothing before the call — and
//! `tests/document_include.rs` demonstrates both halves rather than asserting
//! the promise it would like to be true.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::compose::{self, AnchorRenames, ComposeError};
use crate::ir::{Include, Program, ProgramError};

/// A document that could not be turned into a program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    /// The file could not be read.
    Read {
        /// The path, as resolved.
        path: PathBuf,
        /// Who asked for it — `"the command line"` or a composing document.
        asked_by: String,
        /// The I/O failure, as text (`std::io::Error` is not `Clone`).
        detail: String,
    },
    /// The file is not a grammar program document.
    Parse {
        /// The path.
        path: PathBuf,
        /// What `serde_json` said.
        detail: String,
    },
    /// An include names a path that is not portable.
    UnusablePath {
        /// The document that wrote it.
        by: PathBuf,
        /// The prefix that include declares.
        prefix: String,
        /// The path as written.
        program: String,
        /// Why it cannot be used.
        why: &'static str,
    },
    /// Two includes of one document claim the same prefix.
    PrefixTwice {
        /// The document that wrote both.
        by: PathBuf,
        /// The prefix.
        prefix: String,
    },
    /// A document composes itself, directly or through others.
    Cycle {
        /// The chain, from the document that started it back to itself.
        chain: Vec<PathBuf>,
    },
    /// A document in the tree is not a valid program on its own terms.
    Program {
        /// The document.
        path: PathBuf,
        /// What `validate` refused.
        detail: Box<ProgramError>,
    },
    /// The composition itself was refused.
    Compose {
        /// The composing document.
        by: PathBuf,
        /// The composed document.
        source: PathBuf,
        /// The prefix.
        prefix: String,
        /// What `compose` refused.
        detail: Box<ComposeError>,
    },
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::Read {
                path,
                asked_by,
                detail,
            } => write!(
                f,
                "read {} (asked for by {asked_by}): {detail}",
                path.display()
            ),
            DocumentError::Parse { path, detail } => {
                write!(f, "parse {}: {detail}", path.display())
            }
            DocumentError::UnusablePath {
                by,
                prefix,
                program,
                why,
            } => write!(
                f,
                "{} composes {program:?} under the prefix {prefix:?}, and that path {why}. An \
                 include is resolved against the document that writes it, so the path has to be \
                 one every checkout of this campaign can follow",
                by.display()
            ),
            DocumentError::PrefixTwice { by, prefix } => write!(
                f,
                "{} composes two documents under the prefix {prefix:?}. A prefix is what tells \
                 one composed vocabulary from another, so two pieces sharing one would redefine \
                 each other name by name — refused here, where the include list is, rather than \
                 as a clash on whichever rule name happened to sort first",
                by.display()
            ),
            DocumentError::Cycle { chain } => write!(
                f,
                "a document composes itself: {}. An include copies a program in, so a cycle has \
                 no fixed point — it is refused with the chain named rather than followed until \
                 the stack runs out",
                chain
                    .iter()
                    .map(|p| format!("{}", p.display()))
                    .collect::<Vec<_>>()
                    .join(" → ")
            ),
            DocumentError::Program { path, detail } => {
                write!(f, "{}: {detail}", path.display())
            }
            DocumentError::Compose {
                by,
                source,
                prefix,
                detail,
            } => write!(
                f,
                "{} composes {} under the prefix {prefix:?}: {detail}",
                by.display(),
                source.display()
            ),
        }
    }
}

impl std::error::Error for DocumentError {}

/// One composition the loader performed, in the order it performed it.
///
/// The binding count of the include surface, in the form a report can print: a
/// composition that resolved nothing says so with a zero, and a zero is a
/// statement about the document rather than an absence of output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composition {
    /// The document that wrote the include.
    pub by: PathBuf,
    /// The document composed in.
    pub source: PathBuf,
    /// The `name` that document declares.
    pub name: String,
    /// The prefix its vocabulary took.
    pub prefix: String,
    /// How deep in the include tree this composition sits; `0` is an include
    /// written by the document that was loaded.
    pub depth: usize,
}

/// A loaded document: the program every include has been resolved into, and what
/// was resolved to get there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The program. Its `include` list is empty — resolution consumed it.
    pub program: Program,
    /// Every composition performed, innermost first within each include.
    pub compositions: Vec<Composition>,
}

impl Loaded {
    /// The include surface's binding count for this document: how many program
    /// documents were composed in, at every depth.
    pub fn includes(&self) -> usize {
        self.compositions.len()
    }
}

/// Read a program document and resolve every `include` it writes, recursively.
///
/// The result is an ordinary [`Program`] — the same value the equivalent chain
/// of [`crate::compose::include_renaming`] calls produces — so everything
/// downstream (`validate`, `expand`, `judge`, `export`) is untouched by the fact
/// that composition happened.
///
/// It does **not** call [`Program::validate`] on the result: that is the
/// caller's step, and keeping it there means `check` reports a composed
/// program's reference errors exactly as it reports a hand-written one's.
pub fn load(path: &Path) -> Result<Loaded, DocumentError> {
    let mut stack = Vec::new();
    let mut compositions = Vec::new();
    let program = resolve_file(path, "the command line", &mut stack, &mut compositions, 0)?;
    Ok(Loaded {
        program,
        compositions,
    })
}

/// [`load`], for a document already in hand rather than on disk.
///
/// `base` is the directory the document's includes are relative to — the
/// directory the document itself lives in. `label` is what a refusal calls it.
pub fn resolve(program: Program, base: &Path, label: &Path) -> Result<Loaded, DocumentError> {
    let mut stack = vec![label.to_path_buf()];
    let mut compositions = Vec::new();
    let program = resolve_program(program, label, base, &mut stack, &mut compositions, 0)?;
    Ok(Loaded {
        program,
        compositions,
    })
}

fn resolve_file(
    path: &Path,
    asked_by: &str,
    stack: &mut Vec<PathBuf>,
    compositions: &mut Vec<Composition>,
    depth: usize,
) -> Result<Program, DocumentError> {
    let key = normalise(path);
    if let Some(from) = stack.iter().position(|p| *p == key) {
        let mut chain: Vec<PathBuf> = stack[from..].to_vec();
        chain.push(key);
        return Err(DocumentError::Cycle { chain });
    }
    let bytes = std::fs::read(path).map_err(|e| DocumentError::Read {
        path: path.to_path_buf(),
        asked_by: asked_by.to_string(),
        detail: e.to_string(),
    })?;
    let program: Program = serde_json::from_slice(&bytes).map_err(|e| DocumentError::Parse {
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    let base = path.parent().unwrap_or(Path::new("")).to_path_buf();
    stack.push(key);
    let resolved = resolve_program(program, path, &base, stack, compositions, depth)?;
    stack.pop();
    Ok(resolved)
}

fn resolve_program(
    mut program: Program,
    label: &Path,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    compositions: &mut Vec<Composition>,
    depth: usize,
) -> Result<Program, DocumentError> {
    // The fence FIRST, and here rather than only at `validate`: resolution
    // consumes the include list, so a loader that composed and left the fence
    // to a later call would have built the very document the fence refuses and
    // destroyed the evidence on the way.
    program
        .check_include_fence()
        .map_err(|e| DocumentError::Program {
            path: label.to_path_buf(),
            detail: Box::new(e),
        })?;
    let includes = std::mem::take(&mut program.include);
    if includes.is_empty() {
        return Ok(program);
    }
    let mut prefixes: BTreeSet<&str> = BTreeSet::new();
    for include in &includes {
        if !prefixes.insert(include.prefix.as_str()) {
            return Err(DocumentError::PrefixTwice {
                by: label.to_path_buf(),
                prefix: include.prefix.clone(),
            });
        }
    }

    let mut composed = program;
    for include in &includes {
        let source_path = base.join(check_path(label, include)?);
        let asked_by = format!("{} (prefix {:?})", label.display(), include.prefix);
        let source = resolve_file(&source_path, &asked_by, stack, compositions, depth + 1)?;

        // The composed document is a program in its own right and is judged as
        // one, against the version IT declares, before any of it is copied. A
        // reference error inside a piece names the piece's own file that way,
        // instead of surfacing later as an unresolvable `z0/…` symbol in a
        // document that did not write it.
        source.validate().map_err(|e| DocumentError::Program {
            path: source_path.clone(),
            detail: Box::new(e),
        })?;

        let renames: AnchorRenames<'_> = include
            .rename_anchors
            .iter()
            .map(|(from, to)| (from.as_str(), to.as_str()))
            .collect();
        let name = source.name.clone();
        composed = compose::include_renaming(composed, &source, &include.prefix, &renames)
            .map_err(|e| DocumentError::Compose {
                by: label.to_path_buf(),
                source: source_path.clone(),
                prefix: include.prefix.clone(),
                detail: Box::new(e),
            })?;
        compositions.push(Composition {
            by: label.to_path_buf(),
            source: source_path,
            name,
            prefix: include.prefix.clone(),
            depth,
        });
    }
    Ok(composed)
}

/// Refuse a path a document cannot portably name, and hand back the one it can.
fn check_path<'a>(label: &Path, include: &'a Include) -> Result<&'a Path, DocumentError> {
    let unusable = |why| {
        Err(DocumentError::UnusablePath {
            by: label.to_path_buf(),
            prefix: include.prefix.clone(),
            program: include.program.clone(),
            why,
        })
    };
    if include.program.is_empty() {
        return unusable("is empty, so it names no document at all");
    }
    if include.program.contains('\\') {
        return unusable(
            "separates its segments with `\\`, which is not a path separator on the platform \
             this toolchain builds on — write `/`, which every platform reads",
        );
    }
    let path = Path::new(&include.program);
    if path.is_absolute() || matches!(path.components().next(), Some(Component::Prefix(_))) {
        return unusable(
            "is absolute, so it names a document on one machine and on no other; ADR-0006 \
             forbids an absolute path in emitted output and an input that carries one has the \
             same defect one layer up",
        );
    }
    Ok(path)
}

/// The key two spellings of one path compare equal under — `a/./b.json` and
/// `a/b.json`, `a/c/../b.json` and `a/b.json`.
///
/// Public because a caller that has to ask "is this file the one that document
/// composed?" must ask it the same way the loader answered it. Two different
/// normalisations is how the answer comes out `no` for a file that plainly is.
///
/// A path key that compares two spellings of one file without touching the disk.
///
/// `canonicalize` would be stronger and is deliberately not used: it fails on a
/// path that does not exist, which turns a missing include — the refusal a
/// creator actually meets — into a cycle-detection error about the wrong thing.
/// This collapses `.` and resolves `..` textually, which is what separates
/// `a/../b.json` from `b.json`; a symlink that makes two different paths one
/// file is left to become an ordinary name clash from `compose`.
pub fn normalised_path(path: &Path) -> PathBuf {
    normalise(path)
}

fn normalise(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(Component::ParentDir);
                }
            }
            other => out.push(other),
        }
    }
    out
}
