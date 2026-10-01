//! A typed decoding of a vanilla structure template's root compound.
//!
//! The structure readers ([`crate::compiler::assembled::structure_cells`],
//! [`crate::compiler::assembled::structure_size`], the build's sentinel cell)
//! walk a dynamic [`fastnbt::Value`] tree, which allocates a hash map and an
//! owned key for every compound in the file — one per placed block. This
//! module decodes the same root straight into fixed types instead.
//!
//! It is **strict**, so that it can only ever agree with the dynamic walk:
//! every key is a declared field (`deny_unknown_fields`), every integer must be
//! an NBT `Int` ([`StrictInt`]), every list must be an NBT `List`, and the parts
//! no reader looks at (`entities`, a block's `nbt`, `DataVersion`, `palettes`)
//! are still decoded in full as [`fastnbt::Value`]. A file that decodes here
//! therefore has exactly the shape the dynamic walk reads field by field, and a
//! file that does not (any decode error) is handed to the dynamic walk, which
//! gives its answer exactly as before.

use serde::de::{self, Deserialize, Deserializer, Visitor};

/// An NBT `Int` and nothing else: a `Byte`, `Short` or `Long` is refused rather
/// than widened or narrowed, because the dynamic walk matches `Value::Int` only.
pub(crate) struct StrictInt(pub i32);

impl<'de> Deserialize<'de> for StrictInt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StrictInt;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an NBT Int")
            }
            fn visit_i32<E: de::Error>(self, v: i32) -> Result<StrictInt, E> {
                Ok(StrictInt(v))
            }
        }
        d.deserialize_i32(V)
    }
}

/// The root compound of a structure template.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Root {
    #[serde(rename = "DataVersion")]
    #[allow(dead_code)]
    pub data_version: Option<fastnbt::Value>,
    pub size: Option<fastnbt::Value>,
    pub palette: Option<Vec<PaletteEntry>>,
    #[allow(dead_code)]
    pub palettes: Option<fastnbt::Value>,
    pub blocks: Option<Vec<Block>>,
    #[allow(dead_code)]
    pub entities: Option<fastnbt::Value>,
}

/// One `palette` entry.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PaletteEntry {
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "Properties")]
    pub properties: Option<fastnbt::Value>,
}

/// One `blocks` entry, kept small: a template holds one per placed block.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Block {
    pos: Option<Pos>,
    pub state: Option<StrictInt>,
    #[allow(dead_code)]
    nbt: Option<Box<fastnbt::Value>>,
}

impl Block {
    /// The block's position, when it is a list of exactly three `Int`s — the
    /// only shape the dynamic walk accepts.
    pub fn pos3(&self) -> Option<[i32; 3]> {
        self.pos.as_ref().and_then(|p| p.0)
    }
}

/// A `pos` list of `Int`s, held without a heap allocation: `Some` when it has
/// exactly three elements. A list of any other tag is refused, as
/// [`StrictInt`] refuses one element.
struct Pos(Option<[i32; 3]>);

impl<'de> Deserialize<'de> for Pos {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Pos;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an NBT List of Int")
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Pos, A::Error> {
                let mut out = [0i32; 3];
                let mut n = 0usize;
                while let Some(StrictInt(v)) = seq.next_element::<StrictInt>()? {
                    if n < 3 {
                        out[n] = v;
                    }
                    n += 1;
                }
                Ok(Pos((n == 3).then_some(out)))
            }
        }
        d.deserialize_seq(V)
    }
}

/// Decode an already-decompressed template, or `None` when it does not have
/// the strict shape (the caller then takes the dynamic walk).
pub(crate) fn root(raw: &[u8]) -> Option<Root> {
    fastnbt::from_bytes::<Root>(raw).ok()
}

/// The `size` tag as the dynamic walk reads it: a list of exactly three `Int`s.
pub(crate) fn size_of(size: Option<&fastnbt::Value>) -> Option<[i32; 3]> {
    let fastnbt::Value::List(size) = size? else {
        return None;
    };
    if size.len() != 3 {
        return None;
    }
    let mut out = [0i32; 3];
    for (i, v) in size.iter().enumerate() {
        match v {
            fastnbt::Value::Int(n) => out[i] = *n,
            _ => return None,
        }
    }
    Some(out)
}
