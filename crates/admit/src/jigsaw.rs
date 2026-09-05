//! **Foreign jigsaw resolution** — neutralize the worldgen jigsaw markers a
//! community structure ships with, so an external worldgen piece can be admitted
//! as a *static* library prefab.
//!
//! Modrinth building content is overwhelmingly distributed as **worldgen
//! datapacks**: the `.nbt` templates carry `minecraft:jigsaw` blocks that vanilla
//! resolves *during generation* into their [`final_state`] block (an oak plank, a
//! stone brick, a torch, or `structure_void` = "leave whatever is there"). When we
//! place such a piece deterministically as a hero set-piece we skip worldgen
//! entirely, so those markers would otherwise remain visible in the world.
//!
//! Resolution replaces each jigsaw cell with exactly the `final_state` the vanilla
//! generator would have baked in — the intended primitive, not a workaround (no
//! raycast/heuristic; the block is read straight off the block entity). This must
//! run **at import, before** `delvec prefab socket` carves *our* sockets (our
//! sockets are jigsaw blocks too, with `final_state` = air; resolving after
//! carving would dissolve them).
//!
//! [`final_state`]: https://minecraft.wiki/w/Jigsaw_Block

use std::collections::BTreeMap;

use delvewright_schem::nbt::Nbt;

use crate::structure::{PaletteEntry, Structure};

/// One resolved jigsaw cell, for diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub pos: [i32; 3],
    /// The `final_state` name the jigsaw became.
    pub became: String,
}

/// Replace every `minecraft:jigsaw` block with its block-entity `final_state`
/// (dropping the jigsaw block entity), then prune the now-unreferenced jigsaw
/// palette entry. A jigsaw whose block entity carries no `final_state` defaults to
/// `minecraft:air` (vanilla's fallback). Returns the resolved cells in block
/// order; empty ⇒ the piece had no jigsaw markers (a no-op).
pub fn resolve(s: &mut Structure) -> Vec<Resolved> {
    // Collect first — set_cell mutates the palette, so we cannot hold a palette
    // borrow across the loop.
    let mut plan: Vec<([i32; 3], PaletteEntry, String)> = Vec::new();
    for b in &s.blocks {
        if s.palette[b.state as usize].name != "minecraft:jigsaw" {
            continue;
        }
        let final_state = b
            .nbt
            .as_ref()
            .and_then(Nbt::as_compound)
            .and_then(|c| c.get("final_state"))
            .and_then(Nbt::as_str)
            .unwrap_or("minecraft:air");
        let entry = parse_block_state(final_state);
        plan.push((b.pos, entry, final_state.to_string()));
    }
    let mut resolved = Vec::with_capacity(plan.len());
    for (pos, entry, became) in plan {
        s.set_cell(pos, entry, None);
        resolved.push(Resolved { pos, became });
    }
    if !resolved.is_empty() {
        s.prune_palette();
    }
    resolved
}

/// Parse `minecraft:oak_stairs[facing=north,half=bottom]` into a palette entry.
/// (Same grammar the schem palette reader uses; inlined to avoid widening the
/// schem crate's public surface for one caller.)
fn parse_block_state(s: &str) -> PaletteEntry {
    match s.split_once('[') {
        Some((name, rest)) => {
            let inner = rest.strip_suffix(']').unwrap_or(rest);
            let mut properties = BTreeMap::new();
            for kv in inner.split(',') {
                if let Some((k, v)) = kv.split_once('=') {
                    properties.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            PaletteEntry {
                name: name.to_string(),
                properties,
            }
        }
        None => PaletteEntry::simple(s),
    }
}
