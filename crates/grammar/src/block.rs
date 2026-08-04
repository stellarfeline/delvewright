//! Block states — the grammar's terminal vocabulary.
//!
//! Block states are first class from day one (spec-0027 §2, owner decision
//! 2026-08-04): a name-only palette cannot express stairs, slabs, panes or
//! doors, and those are exactly the micro-depth a Tier-2 build is made of.
//! Upstream carried the same information as `Block.from_string_blockstate`
//! strings behind integer material ids; we drop the id indirection and keep the
//! state.
//!
//! Properties live in a [`BTreeMap`] so both `Display` and serialisation are
//! order-stable regardless of authoring order (ADR-0006).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A block with its full state, e.g. `minecraft:oak_stairs[facing=east,half=top]`.
///
/// Serialises as the vanilla string form, not as a nested object: the IR is
/// authored by an LLM and read by a human reviewer, and
/// `"minecraft:oak_stairs[facing=east]"` is the form both already know.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BlockState {
    /// Namespaced block id. A bare id is namespaced `minecraft:` on parse.
    pub name: String,
    /// Block-state properties, sorted by key.
    pub properties: BTreeMap<String, String>,
}

impl Serialize for BlockState {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for BlockState {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        s.parse().map_err(D::Error::custom)
    }
}

/// Why a block-state string would not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStateParseError {
    /// The block id was empty.
    EmptyName,
    /// A `[` was opened and never closed.
    UnterminatedProperties,
    /// Text followed the closing `]`.
    TrailingText(String),
    /// A property was not `key=value`.
    MalformedProperty(String),
    /// The same property key appeared twice.
    DuplicateProperty(String),
}

impl fmt::Display for BlockStateParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockStateParseError::EmptyName => write!(f, "empty block id"),
            BlockStateParseError::UnterminatedProperties => {
                write!(f, "unterminated block-state property list (missing `]`)")
            }
            BlockStateParseError::TrailingText(t) => {
                write!(f, "trailing text after block state: {t:?}")
            }
            BlockStateParseError::MalformedProperty(p) => {
                write!(f, "block-state property {p:?} is not `key=value`")
            }
            BlockStateParseError::DuplicateProperty(k) => {
                write!(f, "block-state property {k:?} given twice")
            }
        }
    }
}

impl std::error::Error for BlockStateParseError {}

impl BlockState {
    /// A stateless block. `name` is namespaced `minecraft:` when bare.
    pub fn simple(name: &str) -> BlockState {
        BlockState {
            name: namespaced(name),
            properties: BTreeMap::new(),
        }
    }

    /// A block with properties, e.g. `with("oak_stairs", [("facing", "east")])`.
    pub fn with<'a, I>(name: &str, properties: I) -> BlockState
    where
        I: IntoIterator<Item = (&'a str, &'a str)>,
    {
        BlockState {
            name: namespaced(name),
            properties: properties
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    /// `minecraft:air` — what `void` writes and what an untouched cell holds.
    pub fn air() -> BlockState {
        BlockState::simple("minecraft:air")
    }

    /// True for `minecraft:air`.
    pub fn is_air(&self) -> bool {
        self.name == "minecraft:air"
    }
}

fn namespaced(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}

impl FromStr for BlockState {
    type Err = BlockStateParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (name, rest) = match s.split_once('[') {
            Some((name, rest)) => (name.trim(), Some(rest)),
            None => (s, None),
        };
        if name.is_empty() {
            return Err(BlockStateParseError::EmptyName);
        }
        let mut properties = BTreeMap::new();
        if let Some(rest) = rest {
            let Some((body, tail)) = rest.rsplit_once(']') else {
                return Err(BlockStateParseError::UnterminatedProperties);
            };
            if !tail.trim().is_empty() {
                return Err(BlockStateParseError::TrailingText(tail.to_string()));
            }
            for part in body.split(',') {
                let part = part.trim();
                if part.is_empty() {
                    continue;
                }
                let Some((k, v)) = part.split_once('=') else {
                    return Err(BlockStateParseError::MalformedProperty(part.to_string()));
                };
                let (k, v) = (k.trim(), v.trim());
                if k.is_empty() || v.is_empty() {
                    return Err(BlockStateParseError::MalformedProperty(part.to_string()));
                }
                if properties.insert(k.to_string(), v.to_string()).is_some() {
                    return Err(BlockStateParseError::DuplicateProperty(k.to_string()));
                }
            }
        }
        Ok(BlockState {
            name: namespaced(name),
            properties,
        })
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if !self.properties.is_empty() {
            write!(f, "[")?;
            for (i, (k, v)) in self.properties.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{k}={v}")?;
            }
            write!(f, "]")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_string_form() {
        for s in [
            "minecraft:stone",
            "minecraft:oak_stairs[facing=east,half=top]",
            "minecraft:oak_door[facing=north,half=lower]",
        ] {
            let parsed: BlockState = s.parse().unwrap();
            assert_eq!(parsed.to_string(), s);
        }
    }

    #[test]
    fn properties_are_sorted_not_authored_order() {
        let a: BlockState = "oak_stairs[half=top,facing=east]".parse().unwrap();
        let b: BlockState = "minecraft:oak_stairs[facing=east,half=top]"
            .parse()
            .unwrap();
        assert_eq!(a, b, "bare ids namespace, properties sort");
        assert_eq!(a.to_string(), "minecraft:oak_stairs[facing=east,half=top]");
    }

    #[test]
    fn rejects_malformed_states() {
        assert_eq!(
            "stone[facing".parse::<BlockState>(),
            Err(BlockStateParseError::UnterminatedProperties)
        );
        assert_eq!(
            "stone[facing]".parse::<BlockState>(),
            Err(BlockStateParseError::MalformedProperty("facing".into()))
        );
        assert_eq!(
            "stone[a=1,a=2]".parse::<BlockState>(),
            Err(BlockStateParseError::DuplicateProperty("a".into()))
        );
        assert_eq!(
            "stone[a=1]x".parse::<BlockState>(),
            Err(BlockStateParseError::TrailingText("x".into()))
        );
        assert_eq!(
            "".parse::<BlockState>(),
            Err(BlockStateParseError::EmptyName)
        );
    }
}
