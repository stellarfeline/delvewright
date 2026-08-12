//! The pinned 1.21.11 **block-state registry**, and the check every emitter of a
//! structure template owes it.
//!
//! # Why this exists
//!
//! CLAUDE.md, on the `delve-admit` finding (task #70): *an EMITTED command is
//! checked against the pinned command tree by the emitter, not by a test, because
//! the operator running the tool does not run `cargo test`.* Blocks had no such
//! rule. The consequence is measured, not hypothetical: 1.21.11 renamed
//! `minecraft:chain` to `minecraft:iron_chain`, and
//! `prefabs/tidal-keep-generator` kept placing the old id — eight cells of
//! bell-rope in `tk-bell-tower.nbt` that do not exist. A structure template
//! carrying an unknown block loads it as **air**: the generator exits 0, the
//! `.nbt` is well-formed, the byte-identity gate passes, the tower simply has no
//! ropes, and nothing anywhere says so. The same class is in the shipped
//! library: `hero-temple-ruin-arch.nbt` carries `minecraft:chain` at `[4, 9, 13]`
//! (1 of 36 prefabs, measured 2026-08-11 with `delve-admit audit`).
//!
//! So this module is the block half of the command rule. It is deliberately in
//! `delvewright-schem` — the one crate that writes structure `.nbt` bytes — so
//! that a new emitter reaches it by depending on the writer it already needs.
//!
//! # What it validates
//!
//! A full block state: the id, every property name, and every property value,
//! against `crates/compiler/data/blocks-1.21.11.json` (see that directory's
//! `PROVENANCE.md`). Nothing here is a heuristic and nothing is a warning —
//! either 1.21.11 has that state or it does not.
//!
//! It deliberately does **not** validate that a state is *sensible* (a floating
//! stair, a waterlogged block in the sky). That is craft, and craft is judged by
//! the gates over the expanded model; this is spelling.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

/// The pinned registry's JSON text, vendored beside the other 1.21.11 data.
///
/// A source include rather than a crate dependency: the file is one authority
/// shared by this crate, the grammar back end and the five out-of-workspace
/// prefab generators (`prefabs/invariants.rs`), none of which may depend on
/// `delvec`. A moved data file is a compile error, which is the loud failure.
const REGISTRY_JSON: &str = include_str!("../../compiler/data/blocks-1.21.11.json");

/// The Minecraft version this registry describes (ADR-0009).
pub const MC_VERSION: &str = "1.21.11";

/// Why a block state is not a 1.21.11 block state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockError {
    /// No block with this id exists.
    UnknownBlock {
        /// The id as written.
        name: String,
        /// The registry ids closest to it, best first — usually the rename.
        suggestions: Vec<String>,
    },
    /// The block exists but has no such property.
    UnknownProperty {
        /// The block id.
        name: String,
        /// The property as written.
        property: String,
        /// Every property the block does have.
        known: Vec<String>,
    },
    /// The property exists but not with this value.
    BadValue {
        /// The block id.
        name: String,
        /// The property.
        property: String,
        /// The value as written.
        value: String,
        /// Every legal value.
        legal: Vec<String>,
    },
}

impl fmt::Display for BlockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockError::UnknownBlock { name, suggestions } => {
                write!(f, "{name} is not a block in Minecraft {MC_VERSION}")?;
                if !suggestions.is_empty() {
                    write!(f, " — did you mean {}?", suggestions.join(", "))?;
                }
                write!(
                    f,
                    " (a structure template loads an unknown block as AIR, so this would ship a \
                     hole rather than fail)"
                )
            }
            BlockError::UnknownProperty {
                name,
                property,
                known,
            } => write!(
                f,
                "{name} has no property {property:?} in Minecraft {MC_VERSION}; it has {}",
                if known.is_empty() {
                    "none".to_string()
                } else {
                    known.join(", ")
                }
            ),
            BlockError::BadValue {
                name,
                property,
                value,
                legal,
            } => write!(
                f,
                "{name}[{property}={value}] is not legal in Minecraft {MC_VERSION}; {property} is \
                 one of {}",
                legal.join(", ")
            ),
        }
    }
}

impl std::error::Error for BlockError {}

/// Every block id in the pinned version, with every property's legal values.
pub struct BlockRegistry {
    blocks: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

impl BlockRegistry {
    /// The pinned 1.21.11 registry, parsed once per process.
    pub fn v1_21_11() -> &'static BlockRegistry {
        static REGISTRY: OnceLock<BlockRegistry> = OnceLock::new();
        REGISTRY.get_or_init(|| BlockRegistry {
            blocks: serde_json::from_str(REGISTRY_JSON)
                .expect("the vendored block registry is valid JSON"),
        })
    }

    /// How many blocks the pinned version has. A binding count: a check that
    /// reports zero here examined nothing.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Never true for the pinned registry; present because `len` is public.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// True if `name` (namespaced, e.g. `minecraft:stone`) is a block.
    pub fn has(&self, name: &str) -> bool {
        self.blocks.contains_key(name)
    }

    /// Check an id plus its properties.
    ///
    /// A non-`minecraft:` namespace is accepted without inspection: a datapack
    /// may legitimately define its own blocks, and this registry has nothing to
    /// say about them. A bare id is read as `minecraft:`-namespaced, which is
    /// how every emitter in this repo writes one.
    pub fn validate(
        &self,
        name: &str,
        properties: &BTreeMap<String, String>,
    ) -> Result<(), BlockError> {
        let namespaced = if name.contains(':') {
            name.to_string()
        } else {
            format!("minecraft:{name}")
        };
        if !namespaced.starts_with("minecraft:") {
            return Ok(());
        }
        let Some(known) = self.blocks.get(&namespaced) else {
            return Err(BlockError::UnknownBlock {
                suggestions: self.suggest(&namespaced),
                name: namespaced,
            });
        };
        for (property, value) in properties {
            let Some(legal) = known.get(property) else {
                return Err(BlockError::UnknownProperty {
                    name: namespaced,
                    property: property.clone(),
                    known: known.keys().cloned().collect(),
                });
            };
            if !legal.contains(value) {
                return Err(BlockError::BadValue {
                    name: namespaced,
                    property: property.clone(),
                    value: value.clone(),
                    legal: legal.clone(),
                });
            }
        }
        Ok(())
    }

    /// Check a `name[k=v,...]` state string.
    pub fn validate_state_string(&self, state: &str) -> Result<(), BlockError> {
        let (name, properties) = parse_state(state);
        self.validate(name, &properties)
    }

    /// Registry ids most likely to be what an unknown id meant.
    ///
    /// The rename that motivated this module (`chain` → `iron_chain`) is a
    /// **qualification**, not an edit-distance neighbour: the new id is the old
    /// one with a material word put in front of it. So candidates are ranked by
    /// how a rename actually looks — the unknown id as a whole-word *suffix*
    /// first (`iron_chain`), then as a prefix, then anywhere inside — and within
    /// a rank by how few words were added, then alphabetically so the answer is
    /// deterministic (ADR-0006). A wrong suggestion costs nothing; the error is
    /// already fatal.
    fn suggest(&self, name: &str) -> Vec<String> {
        let path = name.split_once(':').map(|(_, p)| p).unwrap_or(name);
        let words: Vec<&str> = path.split('_').collect();

        let mut ranked: Vec<(u8, usize, &String)> = Vec::new();
        for id in self.blocks.keys() {
            let candidate = id.split_once(':').map(|(_, p)| p).unwrap_or(id);
            if candidate == path {
                continue;
            }
            let candidate_words: Vec<&str> = candidate.split('_').collect();
            if candidate_words.len() <= words.len() {
                continue;
            }
            let extra = candidate_words.len() - words.len();
            let rank = if candidate_words.ends_with(words.as_slice()) {
                0
            } else if candidate_words.starts_with(words.as_slice()) {
                1
            } else if candidate_words
                .windows(words.len())
                .any(|w| w == words.as_slice())
            {
                2
            } else {
                continue;
            };
            ranked.push((rank, extra, id));
        }
        ranked.sort();
        ranked
            .into_iter()
            .take(3)
            .map(|(_, _, id)| id.clone())
            .collect()
    }
}

/// Split `name[k=v,k=v]` into its id and its properties.
///
/// Tolerant on purpose: this is a *validator's* front door, so a malformed
/// state must reach the registry and be reported as an unknown block rather
/// than be rejected by a parser with a different vocabulary.
fn parse_state(state: &str) -> (&str, BTreeMap<String, String>) {
    let Some(open) = state.find('[') else {
        return (state.trim(), BTreeMap::new());
    };
    let name = state[..open].trim();
    let inner = state[open + 1..].trim_end().trim_end_matches(']');
    let mut properties = BTreeMap::new();
    for pair in inner.split(',') {
        if let Some((k, v)) = pair.split_once('=') {
            properties.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    (name, properties)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn the_registry_binds_to_the_whole_pinned_version() {
        let reg = BlockRegistry::v1_21_11();
        assert_eq!(reg.len(), 1166, "1.21.11 has 1166 blocks");
        for id in [
            "minecraft:air",
            "minecraft:stone",
            "minecraft:water",
            "minecraft:oak_stairs",
        ] {
            assert!(reg.has(id), "{id} missing from the pinned registry");
        }
    }

    /// The finding this module exists for: 1.21.11 renamed `chain`, and eight
    /// cells of the old id shipped inside `tk-bell-tower.nbt`.
    #[test]
    fn the_renamed_chain_is_caught_and_the_rename_is_suggested() {
        let reg = BlockRegistry::v1_21_11();
        assert!(!reg.has("minecraft:chain"));
        assert!(reg.has("minecraft:iron_chain"));
        let err = reg
            .validate(
                "minecraft:chain",
                &props(&[("axis", "y"), ("waterlogged", "false")]),
            )
            .unwrap_err();
        let BlockError::UnknownBlock { suggestions, .. } = &err else {
            panic!("expected an unknown-block refusal, got {err}");
        };
        assert!(
            suggestions.contains(&"minecraft:iron_chain".to_string()),
            "the rename was not suggested: {suggestions:?}"
        );
        assert!(err.to_string().contains("loads an unknown block as AIR"));
    }

    #[test]
    fn properties_and_values_are_checked_too() {
        let reg = BlockRegistry::v1_21_11();
        assert!(
            reg.validate(
                "minecraft:oak_stairs",
                &props(&[("facing", "east"), ("half", "top")])
            )
            .is_ok()
        );
        assert_eq!(
            reg.validate("minecraft:oak_stairs", &props(&[("orientation", "east")]))
                .unwrap_err(),
            BlockError::UnknownProperty {
                name: "minecraft:oak_stairs".into(),
                property: "orientation".into(),
                known: vec![
                    "facing".into(),
                    "half".into(),
                    "shape".into(),
                    "waterlogged".into()
                ],
            }
        );
        let err = reg
            .validate("minecraft:oak_stairs", &props(&[("facing", "up")]))
            .unwrap_err();
        assert!(matches!(err, BlockError::BadValue { .. }), "{err}");
        assert!(err.to_string().contains("north"), "{err}");
    }

    #[test]
    fn a_state_string_validates_the_same_way_as_its_parts() {
        let reg = BlockRegistry::v1_21_11();
        assert!(reg.validate_state_string("minecraft:stone").is_ok());
        assert!(reg.validate_state_string("stone").is_ok());
        assert!(
            reg.validate_state_string("minecraft:iron_chain[axis=y,waterlogged=false]")
                .is_ok()
        );
        assert!(
            reg.validate_state_string("minecraft:chain[axis=y,waterlogged=false]")
                .is_err()
        );
    }

    /// A datapack's own block is not this registry's business, and refusing it
    /// would make the check a wall instead of a spell-checker.
    #[test]
    fn a_foreign_namespace_is_left_alone() {
        let reg = BlockRegistry::v1_21_11();
        assert!(
            reg.validate("delvewright:nonesuch", &BTreeMap::new())
                .is_ok()
        );
    }
}
