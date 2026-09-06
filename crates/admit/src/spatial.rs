//! **The spatial contract's second door** (spec-0036 §1c): a piece nobody
//! generated, judged by the same checker over the same two inputs.
//!
//! A grammar expansion resolves its scope-bound declarations and hands the
//! checker a block grid and a resolved contract. A hand-built or ingested piece
//! has no declarations to resolve — its boxes are literal from the start — so it
//! carries the identical resolved block in its metadata and hands the checker
//! the identical pair. There is one implementation of the obligations, and this
//! module's whole job is to build its two arguments out of a `.nbt` and a
//! `.json`.
//!
//! That is not tidiness. Two checkers over one contract agree right up until
//! they do not, and the disagreement surfaces as a piece that passed admission
//! and fails at expansion — or worse, the other way round.

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::contract::{ContractReport, check};
use delvewright_grammar::geom::Box3;
use delvewright_grammar::model::VoxelModel;
use delvewright_schem::prefab::PrefabMeta;

use crate::diag::{DW_CONTRACT, DW_UNJUDGED, Diagnostic};
use crate::structure::Structure;

/// Turn a parsed structure template into the block grid the checker reads.
///
/// A cell the template does not name is air, which is what `/place template`
/// does with it.
pub fn grid(s: &Structure) -> VoxelModel {
    let size = [
        s.size[0].max(0) as u32,
        s.size[1].max(0) as u32,
        s.size[2].max(0) as u32,
    ];
    let mut model = VoxelModel::new(Box3::at_origin(size));
    for x in 0..s.size[0] {
        for y in 0..s.size[1] {
            for z in 0..s.size[2] {
                let Some(entry) = s.entry_at([x, y, z]) else {
                    continue;
                };
                let mut block = BlockState::simple(&entry.name);
                block.properties = entry.properties.clone();
                let _ = model.set([x, y, z], &block);
            }
        }
    }
    model
}

/// The anchors a metadata document declares, as the point the checker needs.
///
/// A gate anchor names a region rather than a cell; its low corner is the point
/// used, because a `posted` region's demand is that something is placed *in* it
/// and the low corner is inside it by construction.
fn anchor_points(meta: &PrefabMeta) -> BTreeMap<String, [i32; 3]> {
    meta.anchors
        .iter()
        .filter_map(|(name, a)| {
            let pos = a.pos.or_else(|| a.region.as_ref().map(|r| r.from))?;
            Some((name.clone(), pos))
        })
        .collect()
}

/// Judge a piece's declared contract against its own bytes, or `None` when it
/// declares none.
///
/// The bare judgement, for a caller that already holds both arguments and knows
/// which of the two answers it got. Everything that runs the door on files calls
/// [`door`], which is the one that cannot answer "nothing" silently.
pub fn audit(s: &Structure, meta: &PrefabMeta) -> Option<ContractReport> {
    let contract = meta.spatial_contract.as_ref()?;
    Some(check(&grid(s), contract, &anchor_points(meta)))
}

// ---------------------------------------------------------------------------
// The door, and what it says when it does not open
// ---------------------------------------------------------------------------

/// What the second door did with one piece's bytes.
///
/// The point of the type is that **there is no fourth answer and no silent
/// one**. The door either judged the blocks against a declared contract, or it
/// says which of the three ways it did not — and every arm carries the same
/// [`DoorBinding`], so a reader never has to infer from an absence what was
/// examined.
///
/// The shape it replaces was `if let … && let … && let Some(v) = audit(…)`: five
/// conditions, any of which fell through to a `contract_failed` that was still
/// `false`, and a tool that then printed `"verdict": "pass"`. A tile-set
/// manifest — which is what a composed zone IS — took the first of those five
/// every time, so the door never opened on a composed building at all.
pub enum Door {
    /// Judged: the checker's own report over these bytes.
    Judged {
        report: ContractReport,
        binding: DoorBinding,
    },
    /// There is a declaration document, it declares no contract, and nothing in
    /// it contradicts that.
    Undeclared { binding: DoorBinding },
    /// There is no declaration document beside these bytes at all.
    NoDocument { binding: DoorBinding },
    /// The door **could not** judge and refuses to be read as a pass.
    Refused {
        reason: String,
        binding: DoorBinding,
    },
}

/// What the door had in hand, and what it examined — stated on every outcome,
/// including the outcomes that examined nothing.
///
/// `files`/`cells` describe the blocks; `spaces`/`no_body`/`edges`/`anchors` the
/// declaration; `gates`/`objects` what the obligations actually bound to.
/// `resolved_anchors` is the *corroboration*'s own binding count — see
/// [`Door::open`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DoorBinding {
    /// Which of the four outcomes this is, as one machine-readable word.
    pub state: &'static str,
    /// `.nbt` files whose blocks are in the grid the door read: `1` for a single
    /// template, the tile count for a zone that ships as a tile set.
    pub files: usize,
    /// Cells of that grid.
    pub cells: u64,
    /// Declared spaces.
    pub spaces: usize,
    /// Declared out-of-walk regions.
    pub no_body: usize,
    /// Declared edges.
    pub edges: usize,
    /// Anchor points handed to the checker — or, on an outcome that judged
    /// nothing, the anchors the document declares, which is the denominator the
    /// corroboration below is counted against.
    pub anchors: usize,
    /// Obligations run.
    pub gates: usize,
    /// Objects those obligations examined, summed over the gates.
    pub objects: usize,
    /// Obligations that went red.
    pub failed_gates: usize,
    /// Anchors carrying a `resolves_to` — the drop-detector's binding count.
    pub resolved_anchors: usize,
}

/// The word a report prints for a door that was never opened.
///
/// The library-level [`audit`] and [`crate::audit::audit`] do not open it; only
/// `delvec prefab audit` does. A report that says so is not the same artifact as a
/// report that says the door opened and found nothing wrong, which is the whole
/// obligation here.
pub const UNOPENED: &str = "unopened";

/// The door not having been opened is itself a state, and it is the one a
/// report carries until something opens it.
impl Default for DoorBinding {
    fn default() -> Self {
        DoorBinding {
            state: UNOPENED,
            files: 0,
            cells: 0,
            spaces: 0,
            no_body: 0,
            edges: 0,
            anchors: 0,
            gates: 0,
            objects: 0,
            failed_gates: 0,
            resolved_anchors: 0,
        }
    }
}

impl DoorBinding {
    fn blocks(state: &'static str, files: usize, model: &VoxelModel) -> DoorBinding {
        DoorBinding {
            state,
            files,
            cells: model.region().volume(),
            ..DoorBinding::default()
        }
    }
}

impl Door {
    /// **Open the door on a grid, and answer in every case.**
    ///
    /// `meta_path` is the declaration document these blocks are judged against —
    /// a piece's sibling `.json`, or a tile set's manifest, which is the same
    /// document at zone scale (its contract and its anchors are zone-relative,
    /// so the checker's two arguments exist there exactly as they do for one
    /// template). The door reads that document **itself** rather than taking a
    /// parsed one: absent, unreadable and present are three answers a caller
    /// would otherwise classify, and the caller that classified them wrong is
    /// what this replaces.
    ///
    /// # The refusals, and why they are not "nothing to judge"
    ///
    /// * A document that exists and does not parse is not a piece without a
    ///   contract — it is a piece whose contract nobody can read. Passing it
    ///   would be reporting a verdict on evidence that was never opened.
    /// * A document that declares no contract while its anchors carry
    ///   `resolves_to` **contradicts itself**: only an exporter writes that key,
    ///   and only out of a contract. So the declaration was dropped after the
    ///   fact, which is the failure that must not look like an absence. This is
    ///   corroboration a dropped contract cannot supply — the very tool that
    ///   loses a top-level key on write keeps the anchors it modelled — and its
    ///   own binding count (`resolved_anchors`) is stated, because a document
    ///   with no anchors is one nothing here could contradict.
    pub fn open(model: &VoxelModel, files: usize, meta_path: &Path) -> Door {
        let meta = match PrefabMeta::read(meta_path) {
            Ok(Some(m)) => m,
            Ok(None) => {
                return Door::NoDocument {
                    binding: DoorBinding::blocks("no-document", files, model),
                };
            }
            Err(e) => {
                return Door::Refused {
                    reason: format!(
                        "the declaration document {} does not read: {e}. A piece whose contract \
                         nobody can parse is not a piece without one, so this door will not \
                         report a pass over it",
                        meta_path.display()
                    ),
                    binding: DoorBinding::blocks("refused", files, model),
                };
            }
        };
        let resolved_anchors = meta
            .anchors
            .values()
            .filter(|a| a.resolves_to.is_some())
            .count();

        let Some(contract) = meta.spatial_contract.as_ref() else {
            let binding = DoorBinding {
                resolved_anchors,
                anchors: meta.anchors.len(),
                ..DoorBinding::blocks("undeclared", files, model)
            };
            if resolved_anchors > 0 {
                return Door::Refused {
                    reason: format!(
                        "{} declares no `spatial_contract`, yet {resolved_anchors} of its {} \
                         anchor(s) carry the `resolves_to` an exporter writes ONLY out of a \
                         contract — so the declaration was dropped from this document rather \
                         than never made, and the piece's spaces are unjudged",
                        meta_path.display(),
                        meta.anchors.len()
                    ),
                    binding: DoorBinding {
                        state: "refused",
                        ..binding
                    },
                };
            }
            return Door::Undeclared { binding };
        };

        let anchors = anchor_points(&meta);
        let report = check(model, contract, &anchors);
        let binding = DoorBinding {
            spaces: contract.spaces.len(),
            no_body: contract.no_body.len(),
            edges: contract.edges.len(),
            anchors: anchors.len(),
            gates: report.gates.len(),
            objects: report.gates.iter().map(|g| g.bound).sum(),
            failed_gates: report.gates.iter().filter(|g| !g.passed()).count(),
            resolved_anchors,
            ..DoorBinding::blocks("judged", files, model)
        };
        Door::Judged { report, binding }
    }

    /// What the door bound to, whichever way it answered.
    pub fn binding(&self) -> &DoorBinding {
        match self {
            Door::Judged { binding, .. }
            | Door::Undeclared { binding }
            | Door::NoDocument { binding }
            | Door::Refused { binding, .. } => binding,
        }
    }

    /// True when this outcome must not be read as a pass.
    pub fn is_refusal(&self) -> bool {
        match self {
            Door::Refused { .. } => true,
            Door::Judged { report, .. } => !report.is_pass(),
            Door::Undeclared { .. } | Door::NoDocument { .. } => false,
        }
    }

    /// Everything this door has to say, as diagnostics — **including when it
    /// judged nothing**.
    ///
    /// A judged piece prints its enumeration and its findings as it always did,
    /// and one error per failed obligation. A door that did not open prints
    /// `DW0783` instead, with the count of what it did not examine: that is the
    /// line that makes "never audited" different from "audited and clean", and
    /// its absence is what let a composed zone ship unjudged.
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let b = self.binding();
        match self {
            Door::Judged { report, binding } => {
                let mut out: Vec<Diagnostic> = report
                    .enumeration
                    .iter()
                    .chain(report.findings.iter())
                    .map(|line| Diagnostic::warning(DW_CONTRACT, format!("contract: {line}")))
                    .collect();
                for gate in report.gates.iter().filter(|g| !g.passed()) {
                    out.push(Diagnostic::error(
                        DW_CONTRACT,
                        format!(
                            "{} FAILED (examined {} object(s)): {}",
                            gate.id, gate.bound, gate.detail
                        ),
                    ));
                }
                out.push(Diagnostic::warning(
                    DW_CONTRACT,
                    format!(
                        "contract: judged {} declared object(s) — {} space(s), {} out-of-walk \
                         region(s), {} edge(s), {} anchor(s) — over {} obligation(s) examining {} \
                         object(s), against {} cell(s) of blocks in {} file(s)",
                        binding.spaces + binding.no_body + binding.edges + binding.anchors,
                        binding.spaces,
                        binding.no_body,
                        binding.edges,
                        binding.anchors,
                        binding.gates,
                        binding.objects,
                        binding.cells,
                        binding.files,
                    ),
                ));
                out
            }
            Door::Undeclared { .. } => vec![Diagnostic::warning(
                DW_UNJUDGED,
                format!(
                    "the spatial contract's second door examined NOTHING here: the declaration \
                     document carries no `spatial_contract`, so ZERO obligations ran over {} \
                     cell(s) of blocks in {} file(s). Nothing about this piece's spaces, its \
                     out-of-walk regions or the way a body moves through it has been judged — \
                     which is a different fact from a piece that was judged and held. \
                     Corroboration: {} of its {} anchor(s) carry a resolved contract element, so \
                     nothing here contradicts the absence",
                    b.cells, b.files, b.resolved_anchors, b.anchors,
                ),
            )],
            Door::NoDocument { .. } => vec![Diagnostic::warning(
                DW_UNJUDGED,
                format!(
                    "the spatial contract's second door examined NOTHING here: there is no \
                     declaration document beside these bytes, so ZERO obligations ran over {} \
                     cell(s) of blocks in {} file(s), and there is nothing to corroborate that \
                     absence against. An ingested piece is audited before its metadata exists; \
                     re-run this audit after the admission steps write one",
                    b.cells, b.files,
                ),
            )],
            Door::Refused { reason, .. } => vec![Diagnostic::error(
                DW_UNJUDGED,
                format!(
                    "the spatial contract's second door COULD NOT judge these bytes, and will not \
                     report a pass over them: {reason}. Zero obligations ran over {} cell(s) of \
                     blocks in {} file(s)",
                    b.cells, b.files,
                ),
            )],
        }
    }
}
