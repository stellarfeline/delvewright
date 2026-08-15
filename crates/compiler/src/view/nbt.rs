//! Vanilla-structure `.nbt` reader for the render surface (productionized from
//! the spike-render-fidelity spike).
//!
//! Nucleation 0.9 has **no importer** for the *binary gzip* vanilla structure
//! `.nbt` our prefab generator/compiler emit — its format manager only detects
//! Sponge `.schem`, litematic, Bedrock `.mcstructure`, MCEdit, world regions, and
//! *text* structure SNBT. So we gunzip the `.nbt` ourselves (fastnbt, the same
//! stack the compiler uses), read the vanilla `size`/`palette`/`blocks` schema
//! into a plain [`Structure`], and rebuild it as a `UniversalSchematic` via the
//! public `set_block` API. Textures come from a resource pack (the pinned 1.21.11
//! client jar) — that is what actually determines block fidelity.
//!
//! The parse is pure — no GPU, no nucleation — and unit tested against
//! committed prefabs, which is why it lives on this side of ADR-0021 §1's split:
//! the CPU render arms `delvec` now carries all read structures, and none of them
//! meshes one. The one nucleation-typed function that used to sit here,
//! `build_schematic`, went to `delvewright_render::render`, its only caller.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

use fastnbt::Value;
use flate2::read::GzDecoder;

/// A parsed vanilla structure template, reduced to what the renderer needs.
#[derive(Debug, Clone)]
pub struct Structure {
    /// `[sx, sy, sz]` bounding size.
    pub size: [i32; 3],
    /// Palette as `minecraft:foo[a=b,...]` block-state strings (index-aligned
    /// with the `state` indices in `blocks`).
    pub palette: Vec<String>,
    /// One entry per placed block: `(pos, palette_index)`.
    pub blocks: Vec<([i32; 3], usize)>,
}

/// Error reading/parsing a structure `.nbt`.
#[derive(Debug)]
pub struct NbtError(pub String);

impl std::fmt::Display for NbtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for NbtError {}

fn as_i32(v: &Value) -> Result<i32, NbtError> {
    Ok(match v {
        Value::Byte(b) => *b as i32,
        Value::Short(s) => *s as i32,
        Value::Int(i) => *i,
        Value::Long(l) => *l as i32,
        other => return Err(NbtError(format!("expected int-like NBT, got {other:?}"))),
    })
}

/// Build `minecraft:foo[a=b,c=d]` from a vanilla palette entry compound. Property
/// keys are sorted so the string is deterministic (order is irrelevant to
/// parsing; determinism keeps renders reproducible per input).
fn palette_state_string(entry: &HashMap<String, Value>) -> Result<String, NbtError> {
    let Some(Value::String(name)) = entry.get("Name") else {
        return Err(NbtError(format!("palette entry missing Name: {entry:?}")));
    };
    let mut s = name.clone();
    if let Some(Value::Compound(props)) = entry.get("Properties")
        && !props.is_empty()
    {
        let mut kv: Vec<(&String, &String)> = Vec::with_capacity(props.len());
        for (k, v) in props {
            let Value::String(val) = v else {
                return Err(NbtError(format!("property value not a string: {v:?}")));
            };
            kv.push((k, val));
        }
        kv.sort_by(|a, b| a.0.cmp(b.0));
        let body: Vec<String> = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
        s.push('[');
        s.push_str(&body.join(","));
        s.push(']');
    }
    Ok(s)
}

/// Read + gunzip + parse a vanilla structure `.nbt` into a [`Structure`]. Pure;
/// no GPU, no nucleation.
pub fn parse_structure(path: &Path) -> Result<Structure, NbtError> {
    let raw = std::fs::read(path).map_err(|e| NbtError(format!("read {}: {e}", path.display())))?;
    parse_structure_bytes(&raw)
}

/// [`parse_structure`] over raw file bytes (gzip-framed).
pub fn parse_structure_bytes(raw: &[u8]) -> Result<Structure, NbtError> {
    let mut gz = GzDecoder::new(raw);
    let mut buf = Vec::new();
    gz.read_to_end(&mut buf)
        .map_err(|e| NbtError(format!("gunzip: {e}")))?;
    let root: HashMap<String, Value> =
        fastnbt::from_bytes(&buf).map_err(|e| NbtError(format!("parse structure nbt: {e}")))?;

    let size = match root.get("size") {
        Some(Value::List(s)) if s.len() == 3 => [as_i32(&s[0])?, as_i32(&s[1])?, as_i32(&s[2])?],
        _ => return Err(NbtError("structure has no [x,y,z] size list".into())),
    };

    let Some(Value::List(palette)) = root.get("palette") else {
        return Err(NbtError("structure has no palette list".into()));
    };
    let mut states = Vec::with_capacity(palette.len());
    for e in palette {
        let Value::Compound(c) = e else {
            return Err(NbtError("palette entry not a compound".into()));
        };
        states.push(palette_state_string(c)?);
    }

    let Some(Value::List(blocks)) = root.get("blocks") else {
        return Err(NbtError("structure has no blocks list".into()));
    };
    let mut placed = Vec::with_capacity(blocks.len());
    for b in blocks {
        let Value::Compound(bc) = b else {
            return Err(NbtError("block not a compound".into()));
        };
        let Some(Value::List(pos)) = bc.get("pos") else {
            return Err(NbtError("block missing pos".into()));
        };
        if pos.len() != 3 {
            return Err(NbtError("block pos not [x,y,z]".into()));
        }
        let p = [as_i32(&pos[0])?, as_i32(&pos[1])?, as_i32(&pos[2])?];
        let idx = as_i32(
            bc.get("state")
                .ok_or_else(|| NbtError("block missing state".into()))?,
        )?;
        let idx = usize::try_from(idx).map_err(|_| NbtError("negative state index".into()))?;
        if idx >= states.len() {
            return Err(NbtError(format!("state index {idx} out of palette range")));
        }
        placed.push((p, idx));
    }

    Ok(Structure {
        size,
        palette: states,
        blocks: placed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn prefab(name: &str) -> PathBuf {
        // Local dev resolves the library through the `campaigns/` symlink
        // (spec-0007 Step 0). Tests that need real prefab bytes skip when it is
        // absent (fresh CI checkout without the content repo).
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../campaigns/prefabs")
            .join(name)
    }

    #[test]
    fn parses_keep_gate_room() {
        let p = prefab("keep-gate-room.nbt");
        if !p.exists() {
            eprintln!("skip: {} absent (no content symlink)", p.display());
            return;
        }
        let st = parse_structure(&p).expect("parse");
        assert_eq!(st.size, [7, 5, 9], "keep-gate-room metadata size");
        assert!(!st.palette.is_empty());
        assert!(st.palette.iter().any(|s| s.starts_with("minecraft:")));
        assert!(!st.blocks.is_empty());
        // Every block references a valid palette index.
        assert!(st.blocks.iter().all(|(_, i)| *i < st.palette.len()));
    }

    #[test]
    fn palette_props_are_sorted() {
        let mut e = HashMap::new();
        e.insert("Name".into(), Value::String("minecraft:crafter".into()));
        let mut props = HashMap::new();
        props.insert("triggered".into(), Value::String("false".into()));
        props.insert("crafting".into(), Value::String("false".into()));
        props.insert("orientation".into(), Value::String("north_up".into()));
        e.insert("Properties".into(), Value::Compound(props));
        let s = palette_state_string(&e).unwrap();
        assert_eq!(
            s,
            "minecraft:crafter[crafting=false,orientation=north_up,triggered=false]"
        );
    }
}
