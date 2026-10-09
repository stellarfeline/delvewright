//! Placed pieces: sentinels, template extents, chunk spans.

use super::*;

/// A placement sentinel: one known solid block of a structure, used at runtime
/// to verify a `place template` actually landed (structure_file → (local pos,
/// bare block id)). Chosen as the non-air, non-void block with the lowest `(y, z, x)` —
/// deterministic per structure bytes.
pub(super) type Sentinels = BTreeMap<String, ([i32; 3], String)>;

/// Parse a gzipped vanilla structure `.nbt` and pick its sentinel block.
/// Returns `None` for unparseable or all-air structures (no runtime verify).
pub(super) fn structure_sentinel(bytes: &[u8]) -> Option<([i32; 3], String)> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut raw = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut raw).ok()?;
    // Air, and `structure_void` — a cell the shipped template does not place
    // (`admit::structure::as_placed`), so no block of the world answers for it.
    let is_air = |name: &str| {
        matches!(
            name,
            "minecraft:air"
                | "minecraft:cave_air"
                | "minecraft:void_air"
                | "minecraft:structure_void"
        )
    };
    // The lowest `(y, z, x)` non-air cell; `best` is replaced only by a
    // strictly lower key, so the first such cell in file order wins a tie.
    let pick = |best: &mut Option<([i32; 3], String)>, pos: [i32; 3], name: &String| {
        if is_air(name) {
            return;
        }
        let key = (pos[1], pos[2], pos[0]);
        let better = match &best {
            None => true,
            Some((bp, _)) => key < (bp[1], bp[2], bp[0]),
        };
        if better {
            *best = Some((pos, name.clone()));
        }
    };
    // The typed decoding first ([`crate::compiler::nbtread`]); anything it
    // refuses is walked as before.
    if let Some(root) = crate::compiler::nbtread::root(&raw) {
        let palette: Vec<Option<&String>> = root
            .palette
            .as_ref()?
            .iter()
            .map(|e| e.name.as_ref())
            .collect();
        let mut best: Option<([i32; 3], String)> = None;
        for b in root.blocks.iter().flatten() {
            let Some(pos) = b.pos3() else {
                continue;
            };
            let Some(state) = &b.state else {
                continue;
            };
            let Some(Some(name)) = palette.get(state.0 as usize) else {
                continue;
            };
            pick(&mut best, pos, name);
        }
        return best;
    }
    let root: fastnbt::Value = fastnbt::from_bytes(&raw).ok()?;
    let fastnbt::Value::Compound(root) = root else {
        return None;
    };
    let palette: Vec<Option<String>> = match root.get("palette") {
        Some(fastnbt::Value::List(entries)) => entries
            .iter()
            .map(|e| match e {
                fastnbt::Value::Compound(c) => match c.get("Name") {
                    Some(fastnbt::Value::String(s)) => Some(s.clone()),
                    _ => None,
                },
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    let mut best: Option<([i32; 3], String)> = None;
    if let Some(fastnbt::Value::List(blocks)) = root.get("blocks") {
        for b in blocks {
            let fastnbt::Value::Compound(b) = b else {
                continue;
            };
            let pos: [i32; 3] = match b.get("pos") {
                Some(fastnbt::Value::List(p)) if p.len() == 3 => {
                    let mut out = [0i32; 3];
                    let mut ok = true;
                    for (i, v) in p.iter().enumerate() {
                        match v {
                            fastnbt::Value::Int(n) => out[i] = *n,
                            _ => ok = false,
                        }
                    }
                    if !ok {
                        continue;
                    }
                    out
                }
                _ => continue,
            };
            let state = match b.get("state") {
                Some(fastnbt::Value::Int(n)) => *n as usize,
                _ => continue,
            };
            let Some(Some(name)) = palette.get(state) else {
                continue;
            };
            pick(&mut best, pos, name);
        }
    }
    best
}

/// How much of the world [`check_template_extents`] actually examined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateExtentBinding {
    /// Structure templates the plan places, tiles counted individually.
    pub placed: usize,
    /// Templates whose bytes were loaded and decoded, so their declared extent
    /// could be compared — the binding count.
    pub checked: usize,
}

impl TemplateExtentBinding {
    /// The advisory a zero binding owes its reader, or `None`.
    ///
    /// A world with placed pieces whose templates none of decoded is not a
    /// clean run of this check: it is the check examining nothing while
    /// reporting success, which is the shape a green gate takes when it has
    /// stopped binding to anything.
    pub fn finding(&self) -> Option<delvewright_dsl::Diagnostic> {
        (self.placed > 0 && self.checked == 0).then(|| {
            delvewright_dsl::Diagnostic::warning(
                DW_TEMPLATE_EXTENT,
                "build",
                "template-extent binding",
                format!(
                    "the template-extent invariant examined 0 of {} placed structure \
                     template(s): none of their `.nbt` bytes were loaded or decodable, so the \
                     check passed without comparing anything. A metadata size that disagrees \
                     with its blocks would not have been seen",
                    self.placed
                ),
            )
        })
    }
}

/// Compare every placed template's declared extent against the extent its own
/// bytes declare. See [`DW_TEMPLATE_EXTENT`].
///
/// A template whose bytes are absent from `structures` is not a finding here —
/// that is `DW0300`'s job at load — but it does not count toward the binding
/// either, which is what [`TemplateExtentBinding`] exists to say out loud.
pub fn check_template_extents(
    plan: &Plan,
    structures: &BTreeMap<String, Vec<u8>>,
) -> Result<TemplateExtentBinding, BuildFailure> {
    let mut binding = TemplateExtentBinding {
        placed: 0,
        checked: 0,
    };
    let placed: Vec<_> = plan
        .placed_pieces()
        .flat_map(|p| p.templates.iter().map(move |t| (p, t)))
        .collect();
    // Each template's own size, read in parallel; judged below in placement
    // order, so the first mismatch named is the one the loop would name.
    let sizes = crate::par::map(&placed, |(_, template)| {
        structures
            .get(&template.structure_file)
            .map(|bytes| crate::compiler::assembled::structure_size(bytes))
    });
    for ((piece, template), size) in placed.into_iter().zip(sizes) {
        {
            binding.placed += 1;
            let Some(read) = size else {
                continue;
            };
            let Some(actual) = read else {
                continue;
            };
            binding.checked += 1;
            if actual != template.size {
                let whole = if piece.templates.len() == 1 {
                    String::new()
                } else {
                    format!(
                        " It is one of {} tiles of that zone, so the rest of the zone is placed \
                         around a piece that is not the shape the manifest says it is.",
                        piece.templates.len()
                    )
                };
                return Err(BuildFailure::Diagnostic {
                    code: DW_TEMPLATE_EXTENT,
                    message: format!(
                        "prefab `{}`: structure template `{}` is {}x{}x{} in its own `.nbt`, but \
                         the prefab metadata declares it {}x{}x{}. Every pass but the placement \
                         reads the declared size — the forceload span, the piece AABB the \
                         face-contract check compares, massing's footprint — so the world would \
                         be built around a shape that is not the one whose blocks arrive.{whole} \
                         The `.nbt` and its metadata are not the same export: re-export the \
                         piece, or fix whichever of the two is stale. Do NOT adjust the declared \
                         size to match: the sizes are two claims about one fact and the fix is to \
                         make them one export again",
                        piece.prefab_id,
                        template.structure_file,
                        actual[0],
                        actual[1],
                        actual[2],
                        template.size[0],
                        template.size[1],
                        template.size[2],
                    ),
                });
            }
        }
    }
    Ok(binding)
}

/// Every `(chunk_x, chunk_z)` an inclusive block AABB covers, in ascending
/// order. Chunk coordinates use vanilla's floor division (negative block coords
/// belong to the chunk below, not toward zero — the fifth-level piece
/// straddling chunk `z=-1` that motivated the placement retry loop lives here).
pub(super) fn chunk_span(min: [i32; 3], max: [i32; 3]) -> Vec<(i32, i32)> {
    let (x0, x1) = (min[0].div_euclid(16), max[0].div_euclid(16));
    let (z0, z1) = (min[2].div_euclid(16), max[2].div_euclid(16));
    (x0..=x1)
        .flat_map(|cx| (z0..=z1).map(move |cz| (cx, cz)))
        .collect()
}
