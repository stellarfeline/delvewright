//! **The admitted library pieces a program places by name** (spec-0106 §3.8).
//!
//! A [`Node::Prefab`](crate::grammar::ir::Node::Prefab) names a piece of the
//! prefab library; the expansion writes its blocks, imports its anchors and
//! joins its contract. The expander never reads a file: the caller loads the
//! pieces the program names, once, into a [`Pieces`] handed in through
//! [`ExpandOptions::pieces`](crate::grammar::ExpandOptions::pieces), so an
//! expansion stays a function of its inputs (ADR-0006) and the pieces are
//! named among them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use delvewright_dsl::prefab::{Anchor, SpatialContract};
use sha2::{Digest, Sha256};

use crate::grammar::ir::{Node, Program};
use crate::grammar::model::VoxelModel;

/// One admitted piece, as an expansion reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// `prefab/<id>`.
    pub id: String,
    /// Its size `[x, y, z]`.
    pub size: [i32; 3],
    /// Its blocks, at the origin.
    pub model: VoxelModel,
    /// Its anchors, by the key its document writes.
    pub anchors: BTreeMap<String, Anchor>,
    /// Its spatial contract, when it declares one.
    pub contract: Option<SpatialContract>,
    /// Its own provenance row (ADR-0013), carried into the exported piece's.
    pub provenance: String,
    /// SHA-256 over its block grid's canonical bytes — what the exported
    /// piece's provenance names it by, so the inputs that regenerate the
    /// export include the piece's bytes, not only its name.
    pub hash: String,
    /// The local y its document declares its top water block at, when it
    /// authors a shore (`waterline_y`) — a claim the placement carries.
    pub waterline_y: Option<i32>,
}

/// Every piece a program may place, by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Pieces {
    by_id: BTreeMap<String, Piece>,
}

impl Pieces {
    /// No pieces: a program that places one is refused naming it.
    pub fn none() -> Pieces {
        Pieces::default()
    }

    /// The piece `id`, if loaded.
    pub fn get(&self, id: &str) -> Option<&Piece> {
        self.by_id.get(id)
    }

    /// Add one piece.
    pub fn insert(&mut self, piece: Piece) {
        self.by_id.insert(piece.id.clone(), piece);
    }

    /// How many pieces are loaded.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when none are.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Load every piece `program` names from the prefab library at
    /// `prefabs_dir`. A name the library does not hold is left out, and the
    /// expansion refuses it naming the piece — the one place that refusal is
    /// said.
    pub fn load_for(program: &Program, prefabs_dir: &Path) -> Result<Pieces, String> {
        let named = named(program);
        let mut out = Pieces::none();
        if named.is_empty() {
            return Ok(out);
        }
        let registry =
            crate::compiler::registry::PrefabRegistry::load_dir(prefabs_dir).map_err(|e| {
                format!(
                    "cannot read the prefab library {}: {e}",
                    prefabs_dir.display()
                )
            })?;
        for id in named {
            let Some(meta) = registry.get(&id) else {
                continue;
            };
            let model = crate::admit::spatial::library_grid(prefabs_dir, meta)?;
            let hash = format!("sha256:{:x}", Sha256::digest(model.canonical_bytes()));
            out.insert(Piece {
                id: id.clone(),
                size: meta.size(),
                model,
                anchors: meta.anchors.clone(),
                contract: meta.spatial_contract.clone(),
                provenance: meta
                    .license
                    .as_ref()
                    .map(|l| l.provenance.clone())
                    .unwrap_or_default(),
                hash,
                waterline_y: meta.waterline_y,
            });
        }
        Ok(out)
    }
}

/// Every piece id a program's rules name, whether or not the alternative
/// carrying it can be selected.
pub fn named(program: &Program) -> BTreeSet<String> {
    fn walk(node: &Node, into: &mut BTreeSet<String>) {
        match node {
            Node::Prefab { piece, .. } => {
                into.insert(piece.clone());
            }
            Node::Mark { body, .. }
            | Node::Reorient { body, .. }
            | Node::Claim { body, .. }
            | Node::Bind { body, .. } => walk(body, into),
            Node::Split(split) => split.children.iter().for_each(|c| walk(c, into)),
            Node::Void | Node::Skip | Node::Fill { .. } | Node::Call { .. } => {}
        }
    }
    let mut found = BTreeSet::new();
    for alts in program.rules.values() {
        for alt in alts {
            walk(&alt.body, &mut found);
        }
    }
    found
}

/// **The rules through which a `prefab` could be drawn by the seed** — the
/// static half of the `prefab-placed-by-design` gate (spec-0106 §3.8).
///
/// A rule with two or more alternatives that both apply unconditionally (an
/// unguarded `always`) is a weighted draw on every scope it runs in; a
/// `prefab` reachable from any of its alternatives, directly or through
/// calls, stands where the seed puts it. Each finding is `(rule, piece)`.
/// The expansion records the draws it actually took as well
/// ([`crate::grammar::Expansion::prefab_draws`]), so a draw between guarded
/// alternatives that overlapped at this region is refused too.
pub fn drawn_by_seed(program: &Program) -> Vec<(String, String)> {
    // Which pieces each rule reaches, through calls, to a fixed point.
    fn direct(node: &Node, pieces: &mut BTreeSet<String>, calls: &mut BTreeSet<String>) {
        match node {
            Node::Prefab { piece, .. } => {
                pieces.insert(piece.clone());
            }
            Node::Call { symbol } => {
                calls.insert(symbol.clone());
            }
            Node::Mark { body, .. }
            | Node::Reorient { body, .. }
            | Node::Claim { body, .. }
            | Node::Bind { body, .. } => direct(body, pieces, calls),
            Node::Split(split) => split.children.iter().for_each(|c| direct(c, pieces, calls)),
            Node::Void | Node::Skip | Node::Fill { .. } => {}
        }
    }
    let mut reach: BTreeMap<&str, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
    for (symbol, alts) in &program.rules {
        let mut pieces = BTreeSet::new();
        let mut calls = BTreeSet::new();
        for alt in alts {
            direct(&alt.body, &mut pieces, &mut calls);
        }
        reach.insert(symbol.as_str(), (pieces, calls));
    }
    loop {
        let mut grew = false;
        let snapshot: BTreeMap<&str, BTreeSet<String>> =
            reach.iter().map(|(k, (p, _))| (*k, p.clone())).collect();
        for (pieces, calls) in reach.values_mut() {
            for c in calls.iter() {
                if let Some(more) = snapshot.get(c.as_str()) {
                    for m in more {
                        grew |= pieces.insert(m.clone());
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    let mut out = Vec::new();
    for (symbol, alts) in &program.rules {
        let unguarded = alts
            .iter()
            .filter(|a| matches!(a.when, crate::grammar::ir::Cond::Always))
            .count();
        if alts.len() < 2 || unguarded < 2 {
            continue;
        }
        for piece in &reach[symbol.as_str()].0 {
            out.push((symbol.clone(), piece.clone()));
        }
    }
    out
}
